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
}

impl Default for RegenOptions {
    fn default() -> Self {
        Self { upto: None, editing_sketch: None, relative_deflection: 8e-4, angular_deflection: 0.35, document_variables: BTreeMap::new(), preview_meshes: false, derived: None }
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
}

struct CacheEntry {
    key: u64,
    ctx: Arc<Context>,
    state: FeatureState,
}

#[derive(Default)]
pub struct Regenerator {
    entries: HashMap<FeatureId, CacheEntry>,
    meshes: HashMap<u64, (u64, bool, Arc<MeshBuffers>)>,
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

    pub fn regenerate(&mut self, studio: &PartStudio, config: &ConfigValues, units: &UnitPrefs, opts: &RegenOptions) -> RegenResult {
        let t0 = Instant::now();
        let upto = opts.upto.unwrap_or_else(|| studio.rollback_index()).min(studio.features.len());
        // Configuration variables are visible to every feature.
        let mut outer_vars: BTreeMap<String, Quantity> = opts.document_variables.clone();
        for (name, kind, text) in studio.configuration.variable_exprs(config) {
            let dim = match kind {
                crate::configuration::VariableKind::Length => noshape_units::Dim::LENGTH,
                crate::configuration::VariableKind::Angle => noshape_units::Dim::ANGLE,
                _ => noshape_units::Dim::UNITLESS,
            };
            if let Ok(q) = noshape_units::evaluate_as(&text, dim, units, &noshape_units::EmptyScope) {
                outer_vars.insert(name, q);
            }
        }
        let mut key = hash_bytes(0, serde_json::to_vec(&(config, units, &outer_vars.keys().collect::<Vec<_>>())).unwrap_or_default().as_slice());
        for v in outer_vars.values() {
            key = hash_bytes(key, &v.value.to_le_bytes());
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
        for (i, f) in studio.features.iter().enumerate() {
            let (params, suppressed) = (&effective[i].params, effective[i].suppressed);
            let blob = serde_json::to_vec(&(&f.type_id, &f.name, params, suppressed)).unwrap_or_default();
            key = hash_bytes(key, &blob);
            // Derived features depend on another Part Studio's state.
            if f.type_id == "importDerived"
                && let Some(p) = &opts.derived
            {
                key = hash_bytes(key, &p.version(params.text("partStudio")).to_le_bytes());
            }
            live.push(f.id);
            if i >= upto {
                states.push(FeatureState { id: f.id, status: FeatureStatus::RolledBack, message: None, underdefined: false, time_ms: 0.0 });
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
            let mut state = FeatureState { id: f.id, status: FeatureStatus::Ok, message: None, underdefined: false, time_ms: 0.0 };
            if suppressed {
                state.status = FeatureStatus::Suppressed;
            } else {
                match features::evaluator(&f.type_id) {
                    None => {
                        state.status = FeatureStatus::Error;
                        state.message = Some(format!("{} is not available in this build yet", f.feature_type().map(|t| t.name).unwrap_or(&f.type_id)));
                    }
                    Some(eval_fn) => {
                        // Query variables (`#name` selections) expand against the upstream context.
                        let expanded = eval::expand_query_variables(params, &ctx);
                        let params = expanded.as_ref().unwrap_or(params);
                        let mut ev = EvalCtx::new(f.id, &f.name, params, units, (*ctx).clone(), &outer_vars);
                        ev.editing_sketch = opts.editing_sketch;
                        ev.features = &effective;
                        ev.derived = opts.derived.clone();
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| eval_fn(&mut ev)));
                        match result {
                            Ok(Ok(())) => {
                                if !ev.warnings.is_empty() {
                                    state.status = FeatureStatus::Warning;
                                    state.message = Some(ev.warnings.join("\n"));
                                }
                                if f.type_id == "sketch"
                                    && let Some(s) = ev.ctx.sketch(f.id)
                                {
                                    state.underdefined = s.report.dof.is_some_and(|d| d > 0);
                                }
                                ctx = Arc::new(ev.ctx);
                            }
                            Ok(Err(e)) => {
                                state.status = if e.warning { FeatureStatus::Warning } else { FeatureStatus::Error };
                                state.message = Some(e.message);
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
        let (mut display, needs_refine) = display::build(&ctx, studio, opts, &mut self.meshes);
        display.features = states;
        display.variables = ctx.variables.clone();
        display.regen_ms = t0.elapsed().as_secs_f32() * 1000.0;
        RegenResult { display, needs_refine, context: ctx }
    }
}
