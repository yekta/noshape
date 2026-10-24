//! Part Studio regeneration: evaluates the feature list into bodies,
//! sketches, planes and mate connectors, with per-feature status, and builds
//! the kernel-free [`StudioDisplay`] for the UI.
//!
//! Incremental: per-feature results are cached and keyed by a hash chain of
//! (upstream key, effective feature after configuration), so editing feature
//! *k* re-evaluates from *k* onward. A failed feature is skipped (later
//! features see the context before it), like Onshape. Tessellations are
//! cached per shape, so unchanged bodies are never re-meshed.

pub mod context;
pub mod derived;
mod display;
pub mod runs;
pub use display::flat_view_display;
pub use display::{PART_COLORS, angle_display_mesh, set_angle_display_mesh, tessellate_shape};
pub mod eval;
pub mod features;
pub mod implicit;
pub mod measure;

use crate::configuration::ConfigValues;
use crate::display::{FeatureState, FeatureStatus, MeshBuffers, StudioDisplay};
use crate::partstudio::PartStudio;
use crate::reference::FeatureId;
use context::Context;
use eval::EvalCtx;
use noshape_units::{Quantity, UnitPrefs};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct RegenOptions {
    /// Evaluate only the first `n` features (overrides the rollback bar).
    pub upto: Option<usize>,
    /// Sketch being edited: displayed with constraint states.
    pub editing_sketch: Option<FeatureId>,
    /// Tessellation chord tolerance relative to the body's size.
    pub relative_deflection: f64,
    /// Angular deflection (radians).
    pub angular_deflection: f64,
    /// Variables from the document's Variable Studios (visible everywhere).
    pub document_variables: BTreeMap<String, Quantity>,
    /// Coarse meshes for immediate feedback; a later `Final` pass refines.
    pub preview_meshes: bool,
    /// Access to other Part Studios of the document (Derived features).
    pub derived: Option<std::sync::Arc<dyn derived::DerivedProvider>>,
    /// Compiled Feature Studios of the document (custom features).
    pub custom_features: Option<std::sync::Arc<dyn crate::custom::CustomFeatureProvider>>,
    /// Lets the caller stop this run when a newer edit makes it stale.
    pub cancel: Option<RegenCancel>,
}

/// Stops a running regeneration that a newer edit made stale (live
/// "Pending state": the newest edit wins; Onshape has no visible cancel).
/// `supersede`: finish the features (their results are cached, so the next
/// run reuses them) but skip the display. `abort`: stop at the next feature
/// or pattern instance; nothing half-done is cached.
#[derive(Clone, Debug, Default)]
pub struct RegenCancel(Arc<CancelFlags>);

#[derive(Debug, Default)]
struct CancelFlags {
    superseded: std::sync::atomic::AtomicBool,
    aborted: std::sync::atomic::AtomicBool,
}

impl RegenCancel {
    pub fn abort(&self) {
        self.0.superseded.store(true, std::sync::atomic::Ordering::Relaxed);
        self.0.aborted.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn supersede(&self) {
        self.0.superseded.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn aborted(&self) -> bool {
        self.0.aborted.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn superseded(&self) -> bool {
        self.0.superseded.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Another handle than this one exists: a regeneration (or the task
    /// that starts its refine pass) still holds it.
    pub fn held_elsewhere(&self) -> bool {
        Arc::strong_count(&self.0) > 1
    }

    /// Run `f` (kernel work) so that an abort stops its booleans inside the
    /// kernel ([`noshape_kernel::with_cancel`]).
    pub fn scope<R>(&self, f: impl FnOnce() -> R) -> R {
        noshape_kernel::with_cancel(&self.0.aborted, f)
    }
}

impl Default for RegenOptions {
    fn default() -> Self {
        Self { upto: None, editing_sketch: None, relative_deflection: 8e-4, angular_deflection: 0.35, document_variables: BTreeMap::new(), preview_meshes: false, derived: None, custom_features: None, cancel: None }
    }
}

#[derive(Clone, Default)]
pub struct RegenResult {
    pub display: StudioDisplay,
    /// Some bodies only have preview-quality meshes; regenerate with
    /// `preview_meshes: false` to refine them.
    pub needs_refine: bool,
    /// Final context (bodies with kernel shapes), for export and assemblies.
    pub context: Arc<Context>,
    /// Stopped by [`RegenCancel`]: `display` and `context` are empty.
    pub cancelled: bool,
}

struct CacheEntry {
    key: u64,
    ctx: Arc<Context>,
    state: FeatureState,
}

/// Parsed Assembly context snapshots: (id, revision) -> bodies and their display.
type ContextCache = HashMap<(u32, u32), (Vec<Arc<context::Body>>, Vec<crate::display::BodyDisplay>)>;

#[derive(Default)]
pub struct Regenerator {
    entries: HashMap<FeatureId, CacheEntry>,
    meshes: HashMap<u64, (u64, bool, Arc<MeshBuffers>)>,
    contexts: ContextCache,
    /// Features evaluated so far (not taken from the cache).
    evaluations: u64,
}

fn hash_bytes(prev: u64, data: &[u8]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    prev.hash(&mut h);
    data.hash(&mut h);
    h.finish()
}

impl Regenerator {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many features this Regenerator has evaluated rather than taken
    /// from its cache (tests: a run reuses what it should).
    pub fn evaluations(&self) -> u64 {
        self.evaluations
    }

    pub fn regenerate(&mut self, studio: &PartStudio, config: &ConfigValues, units: &UnitPrefs, opts: &RegenOptions) -> RegenResult {
        let t0 = Instant::now();
        // Model lengths are metres; OCCT's blends are tuned for millimetre-sized parts.
        let _ = noshape_kernel::set_working_scale(1000.0);
        let upto = opts.upto.unwrap_or_else(|| studio.rollback_index()).min(studio.features.len());
        // Configuration variables are visible to every feature.
        let mut outer_vars: BTreeMap<String, Quantity> = opts.document_variables.clone();
        outer_vars.extend(studio.configuration.variable_values(config, units));
        let mut key = hash_bytes(0, serde_json::to_vec(&(config, units, &outer_vars.keys().collect::<Vec<_>>())).unwrap_or_default().as_slice());
        for v in outer_vars.values() {
            key = hash_bytes(key, &v.value.to_le_bytes());
        }
        // Assembly contexts: features referencing them regenerate when one is updated.
        for c in &studio.contexts {
            key = hash_bytes(key, &[c.id.to_le_bytes(), c.revision.to_le_bytes()].concat());
        }
        self.contexts.retain(|k, _| studio.contexts.iter().any(|c| (c.id, c.revision) == *k));
        for c in &studio.contexts {
            self.contexts.entry((c.id, c.revision)).or_insert_with(|| {
                let bodies = c.regen_bodies();
                let display = crate::in_context::display_bodies(c, &bodies);
                (bodies, display)
            });
        }
        let mut ctx: Arc<Context> = Arc::new(Context::default());
        let mut states = Vec::with_capacity(studio.features.len());
        let mut live: Vec<FeatureId> = Vec::new();
        // Effective features (configuration applied), visible to evaluators
        // that re-run other features (feature pattern "Reapply features").
        let effective: Vec<eval::EffectiveFeature> = studio
            .features
            .iter()
            .map(|f| {
                let (params, suppressed) = studio.configuration.apply(f, config);
                eval::EffectiveFeature { id: f.id, type_id: f.type_id.clone(), name: f.name.clone(), params, suppressed }
            })
            .collect();
        let aborted = || opts.cancel.as_ref().is_some_and(RegenCancel::aborted);
        let cancelled = || RegenResult { cancelled: true, ..Default::default() };
        for (i, f) in studio.features.iter().enumerate() {
            if aborted() {
                return cancelled();
            }
            let (params, mut suppressed) = (&effective[i].params, effective[i].suppressed);
            // Boolean expressions (dynamic suppression, checkboxes converted
            // to expressions) see the variables defined upstream.
            let scope = noshape_units::LayeredScope { inner: &ctx.variables, outer: &outer_vars };
            let mut expr_error: Option<String> = None;
            if let Some(e) = f.suppression_expr.as_deref().filter(|e| !e.trim().is_empty()) {
                // Onshape's "Unsuppressed if": active while the expression
                // is true, suppressed while it is false.
                match eval_bool(e, &scope) {
                    Ok(b) => suppressed = !b,
                    Err(msg) => expr_error = Some(format!("Suppression expression: {msg}")),
                }
            }
            let converted = f.feature_type().and_then(|t| resolve_bool_exprs(params, &t.params, &scope, &mut expr_error));
            let params = converted.as_ref().unwrap_or(params);
            let blob = serde_json::to_vec(&(&f.type_id, &f.name, params, suppressed)).unwrap_or_default();
            key = hash_bytes(key, &blob);
            // Derived features depend on another Part Studio's state.
            if crate::features::importing::is_derived(&f.type_id)
                && let Some(p) = &opts.derived
            {
                key = hash_bytes(key, &p.version(params.text("partStudio")).to_le_bytes());
            }
            // Custom features depend on their Feature Studio's source.
            if let Some((tab, _)) = crate::custom::parse_type_id(&f.type_id) {
                let v = opts.custom_features.clone().unwrap_or_else(crate::custom::fallback_provider).version(tab);
                key = hash_bytes(key, &v.to_le_bytes());
            }
            live.push(f.id);
            if i >= upto {
                states.push(FeatureState { id: f.id, status: FeatureStatus::RolledBack, message: None, underdefined: false, time_ms: 0.0, faulty: Vec::new(), computed: Vec::new(), touched: Vec::new(), error_mesh: None, touched_behind: Vec::new(), debug: Default::default() });
                continue;
            }
            if let Some(e) = self.entries.get(&f.id)
                && e.key == key
            {
                ctx = e.ctx.clone();
                states.push(e.state.clone());
                continue;
            }
            let ft = Instant::now();
            let mut state = FeatureState { id: f.id, status: FeatureStatus::Ok, message: None, underdefined: false, time_ms: 0.0, faulty: Vec::new(), computed: Vec::new(), touched: Vec::new(), error_mesh: None, touched_behind: Vec::new(), debug: Default::default() };
            if let Some(msg) = expr_error.take() {
                state.status = FeatureStatus::Error;
                state.message = Some(msg);
            } else if suppressed {
                state.status = FeatureStatus::Suppressed;
            } else {
                match features::evaluator(&f.type_id) {
                    None => {
                        state.status = FeatureStatus::Error;
                        state.message = Some(format!("{} is not available in this build yet", f.feature_type().map(|t| t.name).unwrap_or(&f.type_id)));
                    }
                    Some(eval_fn) => {
                        self.evaluations += 1;
                        // Query variables (`#name` selections) expand against the upstream context.
                        let expanded = eval::expand_query_variables(params, &ctx);
                        let params = expanded.as_ref().unwrap_or(params);
                        // Geometry of Assembly contexts joins the feature's context only
                        // while it is evaluated (never merged, patterned or listed).
                        let uses_context = !studio.contexts.is_empty() && crate::in_context::params_use_context(params, None);
                        let mut base = (*ctx).clone();
                        if uses_context {
                            for (bodies, _) in self.contexts.values() {
                                base.bodies.extend(bodies.iter().cloned());
                            }
                        }
                        let mut ev = EvalCtx::new(f.id, &f.name, params, units, base, &outer_vars);
                        ev.cancel = opts.cancel.clone();
                        ev.editing_sketch = opts.editing_sketch;
                        ev.features = &effective;
                        ev.derived = opts.derived.clone();
                        ev.custom = opts.custom_features.clone();
                        ev.custom_type = crate::custom::is_custom(&f.type_id).then(|| f.type_id.clone());
                        let specs = f.feature_type().map(|t| t.params.as_slice()).unwrap_or(&[]);
                        // Every boolean of the feature on this thread stops
                        // when the run is aborted (not only boolean_into's).
                        let cancel = opts.cancel.clone();
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let mut run = || {
                                ev.check_bounds(specs)?;
                                eval_fn(&mut ev)
                            };
                            match &cancel {
                                Some(c) => c.scope(run),
                                None => run(),
                            }
                        }));
                        // Stopped part-way: don't cache a half-done feature.
                        if aborted() {
                            return cancelled();
                        }
                        match result {
                            Ok(Ok(())) => {
                                if !ev.warnings.is_empty() {
                                    state.status = FeatureStatus::Warning;
                                    state.message = Some(ev.warnings.join("\n"));
                                } else if !ev.infos.is_empty() {
                                    state.status = FeatureStatus::Info;
                                    state.message = Some(ev.infos.join("\n"));
                                }
                                if f.type_id == "sketch"
                                    && let Some(s) = ev.ctx.sketch(f.id)
                                {
                                    state.underdefined = s.report.dof.is_some_and(|d| d > 0);
                                    // Live (qa5 SK1): a sketch that can't be solved is in
                                    // error (red title and feature row), its geometry kept.
                                    if !matches!(s.report.status, noshape_sketch::SolveStatus::Solved) {
                                        state.status = FeatureStatus::Error;
                                        state.message = Some(match &s.report.status {
                                            noshape_sketch::SolveStatus::Failed { .. } => "Sketch could not be solved.".to_string(),
                                            _ => state.message.clone().unwrap_or_else(|| "Sketch has conflicting constraints".into()),
                                        });
                                    }
                                }
                                state.computed = std::mem::take(&mut ev.computed);
                                state.touched = std::mem::take(&mut ev.touched);
                                state.touched_behind = std::mem::take(&mut ev.touched_behind);
                                state.debug = std::mem::take(&mut ev.debug);
                                if uses_context {
                                    ev.ctx.bodies.retain(|b| !crate::in_context::is_context_feature(b.id.feature));
                                }
                                ev.ctx.renumber_default_names();
                                ctx = Arc::new(ev.ctx);
                            }
                            Ok(Err(e)) => {
                                state.status = if e.warning { FeatureStatus::Warning } else { FeatureStatus::Error };
                                state.message = Some(e.message);
                                state.faulty = e.faulty.iter().map(|s| s.to_string()).collect();
                                state.touched = std::mem::take(&mut ev.touched);
                                state.touched_behind = std::mem::take(&mut ev.touched_behind);
                                if !ev.error_shapes.is_empty() {
                                    state.error_mesh = Some(display::tessellate_error(&noshape_kernel::Shape::compound(&ev.error_shapes), opts));
                                }
                            }
                            Err(_) => {
                                state.status = FeatureStatus::Error;
                                state.message = Some("Internal error while regenerating this feature".into());
                            }
                        }
                    }
                }
            }
            state.time_ms = ft.elapsed().as_secs_f32() * 1000.0;
            self.entries.insert(f.id, CacheEntry { key, ctx: ctx.clone(), state: state.clone() });
            states.push(state);
        }
        self.entries.retain(|id, _| live.contains(id));
        if opts.cancel.as_ref().is_some_and(RegenCancel::superseded) {
            return cancelled();
        }
        let (mut display, needs_refine) = display::build(&ctx, studio, opts, &mut self.meshes);
        if aborted() {
            return cancelled();
        }
        crate::configured_properties::apply_names(&mut display, studio, config);
        // The shown Assembly context: its bodies, translucent and pickable.
        if let Some(active) = studio.active_context
            && let Some(c) = studio.contexts.iter().find(|c| c.id == active)
            && let Some((_, shown)) = self.contexts.get(&(c.id, c.revision))
        {
            display.context_bodies = shown.clone();
        }
        display.features = states;
        display.variables = ctx.variables.clone();
        display.regen_ms = t0.elapsed().as_secs_f32() * 1000.0;
        RegenResult { display, needs_refine, context: ctx, cancelled: false }
    }
}

/// Truth value of a boolean expression (`#a > 3 in`, `#flag && #b < 2`):
/// any non-zero result is true.
pub fn eval_bool(text: &str, scope: &dyn noshape_units::VariableScope) -> Result<bool, String> {
    noshape_units::evaluate(text, scope).map(|q| q.value != 0.0).map_err(|e| e.to_string())
}

/// Checkbox parameters holding an expression (Onshape "Convert to
/// expression") evaluated to booleans; `None` when there are none.
fn resolve_bool_exprs(params: &crate::params::ParamValues, specs: &[crate::params::ParamSpec], scope: &dyn noshape_units::VariableScope, error: &mut Option<String>) -> Option<crate::params::ParamValues> {
    use crate::params::{ParamKind, ParamValue};
    let mut out: Option<crate::params::ParamValues> = None;
    for s in specs.iter().filter(|s| matches!(s.kind, ParamKind::Bool { .. })) {
        if let Some(ParamValue::Expr(e)) = params.get(s.id) {
            let b = match eval_bool(&e.text, scope) {
                Ok(b) => b,
                Err(msg) => {
                    error.get_or_insert(format!("{}: {msg}", s.label));
                    false
                }
            };
            out.get_or_insert_with(|| params.clone()).set(s.id, ParamValue::Bool(b));
        }
    }
    out
}
