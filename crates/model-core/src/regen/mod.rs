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
pub mod custom_features;
pub mod crash_guard;
pub mod derived;
mod display;
pub mod runs;
pub mod std_route;
pub use display::flat_view_display;
pub use display::{PART_COLORS, angle_display_mesh, keep_shown, set_angle_display_mesh, shown_vertices, tessellate_shape};
pub mod eval;
pub mod features;
pub mod implicit;
pub mod measure;

use crate::configured_properties::{configured_value, parse_color};
use crate::in_context::ContextBodies;
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
    pub custom_features: Option<std::sync::Arc<dyn custom_features::CustomFeatureProvider>>,
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
    hook: std::sync::OnceLock<EvaluateHook>,
}

/// [`RegenCancel::on_evaluate`]'s callback.
struct EvaluateHook(Arc<dyn Fn(FeatureId) + Send + Sync>);

impl std::fmt::Debug for EvaluateHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EvaluateHook")
    }
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

    /// Test hook: `f` runs on the regenerating thread before each feature
    /// this run evaluates (not the cached ones), e.g. to hold the run at a
    /// barrier while a newer request retires it. Set once.
    pub fn on_evaluate(&self, f: impl Fn(FeatureId) + Send + Sync + 'static) {
        let _ = self.0.hook.set(EvaluateHook(Arc::new(f)));
    }

    fn evaluating(&self, id: FeatureId) {
        if let Some(h) = self.0.hook.get() {
            (h.0)(id);
        }
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
    /// Features this run evaluated (the others came from the cache).
    pub evaluated: Vec<FeatureId>,
}

struct CacheEntry {
    key: u64,
    ctx: Arc<Context>,
    state: FeatureState,
    /// The run that last used it ([`Regenerator::run`]).
    last_run: u64,
    /// The feature's own key (its definition and the run's settings, not what came before it), its input and what
    /// it read of it ([`ReadSet`]; `None`: never reused): incremental regeneration.
    local: u64,
    input: Option<Arc<Context>>,
    reads: Option<Arc<ReadSet>>,
}

/// What an evaluation read of its input's bodies: bodies by address (the entry's input keeps them alive) and the
/// name lookups it made.
struct ReadSet {
    bodies: Vec<usize>,
    lookups: Vec<(context::LookupKind, crate::reference::TopoName)>,
    bounds: Option<Option<(glam::DVec3, glam::DVec3)>>,
    scopes: Vec<(Vec<noshape_kernel::Shape>, Vec<crate::reference::BodyId>)>,
}

thread_local! {
    static REUSE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Incremental regeneration on or off for this thread's regenerations (`None`: as NOSHAPE_REGEN_REUSE says, on
/// unless it is `0`): a feature whose definition and the inputs it read are unchanged takes its last result instead of
/// running again. Returns the thread's setting before.
pub fn set_reuse(on: Option<bool>) -> Option<bool> {
    REUSE.with(|r| r.replace(on))
}

fn reuse_on() -> bool {
    static ENV: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    REUSE.with(|r| r.get()).unwrap_or_else(|| *ENV.get_or_init(|| !std::env::var("NOSHAPE_REGEN_REUSE").is_ok_and(|v| v.trim() == "0")))
}

/// `e`'s result for `input` when nothing `e`'s evaluation read differs: the same body ids in the same order, every
/// other part of the context the same, each body it read the very same, each name lookup it made finding the same
/// on both inputs. The result is `e`'s with the input's bodies in place of those it passed through unread.
fn reused(e: &CacheEntry, input: &Context) -> Result<Context, String> {
    let (Some(reads), Some(old)) = (e.reads.as_ref(), e.input.as_ref()) else { return Err("it read all the bodies".into()) };
    let (a, b) = (old.bodies.untracked(), input.bodies.untracked());
    if a.len() != b.len() || a.iter().zip(b).any(|(x, y)| x.id != y.id) {
        return Err("other body ids".into());
    }
    if !old.same_but_bodies(input) {
        return Err("the context besides the bodies differs".into());
    }
    let at: HashMap<usize, usize> = a.iter().enumerate().map(|(j, x)| (Arc::as_ptr(x) as usize, j)).collect();
    let mut read = vec![false; a.len()];
    for p in &reads.bodies {
        if let Some(&j) = at.get(p) {
            if !Arc::ptr_eq(&a[j], &b[j]) {
                return Err(format!("{} ({}) changed", b[j].name, b[j].id));
            }
            read[j] = true;
        }
    }
    for (k, n) in &reads.lookups {
        if old.bodies.find_untracked(*k, n) != input.bodies.find_untracked(*k, n) {
            return Err(format!("a {k:?} lookup finds other entities"));
        }
    }
    if reads.bounds.is_some_and(|bb| bb != input.bodies.bounds_untracked()) {
        return Err("the bodies' bounds differ".into());
    }
    for (tools, ids) in &reads.scopes {
        if eval::touching(b, tools).into_iter().map(|j| &b[j].id).ne(ids.iter()) {
            return Err("other bodies touch its tools".into());
        }
    }
    let by_id: HashMap<&crate::reference::BodyId, usize> = a.iter().enumerate().map(|(j, x)| (&x.id, j)).collect();
    let mut out = (*e.ctx).clone();
    let bodies: Vec<Arc<context::Body>> = out.bodies.untracked().iter().map(|x| match by_id.get(&x.id) {
        Some(&j) if !read[j] => b[j].clone(),
        _ => x.clone(),
    }).collect();
    out.bodies = bodies.into();
    features::refgeom::refresh_open_composites(&mut out);
    out.renumber_default_names();
    Ok(out)
}

/// Two solved sketches the same: their definition, plane, solve report, labels and images (what the rest of their
/// result is computed from).
fn same_sketch(a: &context::SketchResult, b: &context::SketchResult) -> bool {
    a.feature == b.feature
        && a.name == b.name
        && a.frame == b.frame
        && a.report == b.report
        && a.dimension_text == b.dimension_text
        && a.images.len() == b.images.len()
        && a.images.iter().zip(&b.images).all(|(x, y)| x.id == y.id && x.box_points == y.box_points && x.data_base64 == y.data_base64)
        && serde_json::to_vec(&a.sketch).ok().is_some_and(|x| serde_json::to_vec(&b.sketch).ok().is_some_and(|y| x == y))
}

/// Entries of features a run no longer has (deleted, or a feature list from
/// before an edit) stay this many runs, at most [`KEEP_GONE`] of them, so
/// undoing a delete finds the feature evaluated (its key still chains its
/// unchanged upstream).
pub(super) const KEEP_RUNS: u64 = 16;
/// Evaluations kept per feature: undoing or redoing an edit finds the
/// feature (and everything after it) as it was.
const VERSIONS: usize = 3;
const KEEP_GONE: usize = 8;
/// Whole results kept: undoing or redoing this many steps back is a lookup (PLAN 2.6: undo / redo ≤ 50 ms; past
/// [`VERSIONS`] edits of one feature its evaluations are gone).
const RESULTS: usize = 32;
/// And while their meshes (each counted once) stay under this many bytes.
const RESULT_MESH_BYTES: usize = 256 << 20;

/// Parsed Assembly context snapshots: (id, revision) -> bodies and their display.
type ContextCache = HashMap<(u32, u32), (Vec<Arc<context::Body>>, Vec<crate::display::BodyDisplay>)>;

#[derive(Default)]
pub struct Regenerator {
    /// Per feature its last few evaluations, newest first ([`VERSIONS`]).
    entries: HashMap<FeatureId, Vec<CacheEntry>>,
    /// Counts runs (the age of cache entries).
    run: u64,
    meshes: HashMap<u64, (u64, bool, Arc<MeshBuffers>)>,
    /// Shapes of the cached meshes and when they were last shown ([`display::build`]).
    mesh_owners: HashMap<u64, (u64, noshape_kernel::Shape)>,
    /// Each sketch's display, kept while its result is the same ([`display::build`]).
    sketch_displays: display::SketchDisplays,
    contexts: ContextCache,
    /// Features evaluated so far (not taken from the cache).
    evaluations: u64,
    /// The last runs' results by [`result_key`], newest first.
    results: Vec<(u64, RegenResult)>,
}

/// Everything a run's result depends on (the studio, its settings and the options; preview meshes aside: a full
/// result serves a preview); none for studios with Assembly contexts (their snapshots are too big to hash each run).
fn result_key(studio: &PartStudio, config: &ConfigValues, units: &UnitPrefs, opts: &RegenOptions, effective: &[eval::EffectiveFeature]) -> Option<u64> {
    if !studio.contexts.is_empty() {
        return None;
    }
    let settings = (config, units, opts.upto, opts.editing_sketch, opts.relative_deflection.to_bits(), opts.angular_deflection.to_bits(), display::angle_display_mesh());
    let mut key = hash_bytes(hash_bytes(1, &serde_json::to_vec(studio).ok()?), &serde_json::to_vec(&settings).ok()?);
    for (name, v) in &opts.document_variables {
        key = hash_bytes(hash_bytes(key, name.as_bytes()), &v.value.to_le_bytes());
    }
    for f in effective {
        if crate::features::importing::is_derived(&f.type_id)
            && let Some(p) = &opts.derived
        {
            key = hash_bytes(key, &p.version(f.params.text("partStudio")).to_le_bytes());
        }
        if let Some((tab, _)) = crate::custom::parse_type_id(&f.type_id) {
            let v = opts.custom_features.clone().unwrap_or_else(custom_features::fallback_provider).version(tab);
            key = hash_bytes(key, &v.to_le_bytes());
        }
    }
    Some(key)
}

/// NOSHAPE_KERNEL_PROFILE: the kernel's profile is printed for each feature.
fn kernel_profiling() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NOSHAPE_KERNEL_PROFILE").is_some())
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
        let _sheet_metal = std_route::LegacySheetMetal::set(studio.legacy_sheet_metal);
        let _blends = std_route::BlendKernel::set(!studio.legacy_blends);
        let _topology = std_route::LegacyTopology::set(studio.legacy_topology);
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
        let key0 = key;
        let reuse = reuse_on();
        self.contexts.retain(|k, _| studio.contexts.iter().any(|c| (c.id, c.revision) == *k));
        for c in &studio.contexts {
            self.contexts.entry((c.id, c.revision)).or_insert_with(|| {
                let bodies = c.regen_bodies();
                let display = crate::in_context::display_bodies(c, &bodies);
                (bodies, display)
            });
        }
        let mut ctx: Arc<Context> = Arc::new(Context { legacy_face_frames: studio.legacy_face_frames, ..Default::default() });
        let mut states = Vec::with_capacity(studio.features.len());
        let mut live: Vec<FeatureId> = Vec::new();
        let (mut evaluated, mut cached): (Vec<FeatureId>, usize) = (Vec::new(), 0);
        self.run += 1;
        let run = self.run;
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
        let result_key = result_key(studio, config, units, opts, &effective);
        if let Some(k) = result_key
            && let Some(i) = self.results.iter().position(|(x, r)| *x == k && (opts.preview_meshes || !r.needs_refine))
        {
            let entry = self.results.remove(i);
            let mut r = entry.1.clone();
            self.results.insert(0, entry);
            r.evaluated = Vec::new();
            r.display.regen_ms = t0.elapsed().as_secs_f32() * 1000.0;
            return r;
        }
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
            let local = hash_bytes(key0, &blob);
            // Derived features depend on another Part Studio's state.
            if crate::features::importing::is_derived(&f.type_id)
                && let Some(p) = &opts.derived
            {
                key = hash_bytes(key, &p.version(params.text("partStudio")).to_le_bytes());
            }
            // Custom features depend on their Feature Studio's source.
            if let Some((tab, _)) = crate::custom::parse_type_id(&f.type_id) {
                let v = opts.custom_features.clone().unwrap_or_else(custom_features::fallback_provider).version(tab);
                key = hash_bytes(key, &v.to_le_bytes());
            }
            // Reused when its inputs are (not custom, std-run, derived or Assembly-context features: they read more
            // than the context).
            let reusable = reuse
                && !crate::custom::is_custom(&f.type_id)
                && std_route::evaluator(&f.type_id).is_none()
                && !crate::features::importing::is_derived(&f.type_id)
                && studio.contexts.is_empty();
            live.push(f.id);
            if i >= upto {
                states.push(FeatureState { id: f.id, status: FeatureStatus::RolledBack, message: None, underdefined: false, time_ms: 0.0, faulty: Vec::new(), missing: 0, computed: Vec::new(), variable: None, touched: Vec::new(), error_mesh: None, touched_behind: Vec::new(), debug: Default::default(), toggle_points: Vec::new(), manipulators: Vec::new() });
                continue;
            }
            if let Some(versions) = self.entries.get_mut(&f.id)
                && let Some(i) = versions.iter().position(|e| e.key == key)
            {
                let mut e = versions.remove(i);
                e.last_run = run;
                ctx = e.ctx.clone();
                states.push(e.state.clone());
                versions.insert(0, e);
                cached += 1;
                continue;
            }
            if reusable
                && expr_error.is_none()
                && let Some(versions) = self.entries.get_mut(&f.id)
                && let Some((e, out)) = versions.iter().filter(|e| e.local == local).find_map(|e| match reused(e, &ctx) {
                    Ok(out) => Some((e, out)),
                    Err(why) => {
                        if context::trace_reads() {
                            eprintln!("reuse trace: {} ({}) not reused: {why}", f.name, f.type_id);
                        }
                        None
                    }
                })
            {
                let t = Instant::now();
                let (reads, mut state) = (e.reads.clone(), e.state.clone());
                state.time_ms = t.elapsed().as_secs_f32() * 1000.0;
                let input = ctx.clone();
                ctx = Arc::new(out);
                versions.insert(0, CacheEntry { key, ctx: ctx.clone(), state: state.clone(), last_run: run, local, input: Some(input), reads });
                versions.truncate(VERSIONS);
                states.push(state);
                cached += 1;
                continue;
            }
            let input = ctx.clone();
            let body_reads = reusable.then(|| Arc::new(context::BodyReads::default()));
            evaluated.push(f.id);
            let ft = Instant::now();
            let mut state = FeatureState { id: f.id, status: FeatureStatus::Ok, message: None, underdefined: false, time_ms: 0.0, faulty: Vec::new(), missing: 0, computed: Vec::new(), variable: None, touched: Vec::new(), error_mesh: None, touched_behind: Vec::new(), debug: Default::default(), toggle_points: Vec::new(), manipulators: Vec::new() };
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
                        if let Some(c) = &opts.cancel {
                            c.evaluating(f.id);
                        }
                        // A crash in here leaves the marker behind (qa DOC-POISON).
                        let _crash_guard = crash_guard::enter(f.id, &f.name);
                        let mut base = (*ctx).clone();
                        base.bodies.track(body_reads.clone());
                        // Query variables (`#name` selections) expand against the upstream context.
                        let expanded = eval::expand_query_variables(params, &base);
                        let params = expanded.as_ref().unwrap_or(params);
                        // Live (qa4 round 5 v3 s8): a sketch region that no longer
                        // exists (its sketch deleted, or moved below) is a missing
                        // selection: left out, so the feature reports its own
                        // empty-input message ("Select face or sketch region to
                        // extrude.") with "1 missing selection".
                        let own = |id: FeatureId| studio.features.iter().any(|g| g.id == id);
                        let (pruned, missing) = eval::drop_missing_regions(params, &base, &own);
                        let params = pruned.as_ref().unwrap_or(params);
                        state.missing = missing;
                        // Geometry of Assembly contexts joins the feature's context only
                        // while it is evaluated (never merged, patterned or listed).
                        let uses_context = !studio.contexts.is_empty() && crate::in_context::params_use_context(params, None);
                        if uses_context {
                            for (bodies, _) in self.contexts.values() {
                                base.bodies.extend(bodies.iter().cloned());
                            }
                        }
                        let mut ev = EvalCtx::new(f.id, &f.name, params, units, base, &outer_vars);
                        ev.cancel = opts.cancel.clone();
                        ev.editing_sketch = opts.editing_sketch;
                        ev.features = eval::Features::new(&effective, body_reads.as_deref());
                        // The sheet metal form library's studios too (`library:<id>`).
                        ev.derived = Some(Arc::new(crate::sheetmetal::forms::WithLibrary(opts.derived.clone())));
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
                                if missing > 0 && ev.warnings.is_empty() {
                                    ev.warnings.push(missing_text(missing));
                                }
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
                                state.toggle_points = std::mem::take(&mut ev.toggle_points);
                                state.manipulators = std::mem::take(&mut ev.manipulators);
                                if uses_context {
                                    ev.ctx.bodies.retain(|b| !crate::in_context::is_context_feature(b.id.feature));
                                }
                                ev.ctx.bodies.track(None);
                                features::refgeom::refresh_open_composites(&mut ev.ctx);
                                ev.ctx.renumber_default_names();
                                // A sketch solved again the same as before keeps its last result, so the features
                                // after it see the context unchanged (incremental regeneration).
                                if f.type_id == "sketch"
                                    && let Some(i) = ev.ctx.sketches.iter().position(|s| s.feature == f.id)
                                    && let Some(old) = self.entries.get(&f.id).into_iter().flatten().filter_map(|e| e.ctx.sketch(f.id)).find(|o| same_sketch(o, &ev.ctx.sketches[i]))
                                {
                                    ev.ctx.sketches[i] = old.clone();
                                }
                                ctx = Arc::new(ev.ctx);
                                if f.type_id == "assignVariable" {
                                    let name = params.text("name").trim().trim_start_matches('#').to_string();
                                    state.variable = ctx.variables.get(&name).map(|q| (name, *q));
                                }
                            }
                            Ok(Err(e)) => {
                                state.status = if e.warning { FeatureStatus::Warning } else { FeatureStatus::Error };
                                state.message = Some(e.message);
                                state.faulty = e.faulty.iter().map(|s| s.to_string()).collect();
                                state.touched = std::mem::take(&mut ev.touched);
                                state.touched_behind = std::mem::take(&mut ev.touched_behind);
                                // A failed custom feature's debug() still shows (live).
                                state.debug = std::mem::take(&mut ev.debug);
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
            if kernel_profiling() {
                noshape_kernel::kernel_profile_mark(&format!("{} {:.1} ms", f.name, state.time_ms));
            }
            if let Some(r) = &body_reads
                && let Some(t) = r.first_all.lock().take()
            {
                // The first frame of ours below the bodies list's own.
                let lines: Vec<&str> = t.lines().map(str::trim).collect();
                let at = lines
                    .iter()
                    .position(|l| !l.starts_with("at ") && l.contains("noshape_") && !l.contains("BodyReads") && !l.contains("Bodies") && !l.contains("Features") && !l.contains("Deref") && !l.contains("IntoIterator"))
                    .map_or("?".to_string(), |i| format!("{} {}", lines[i], lines.get(i + 1).copied().unwrap_or("")));
                eprintln!("reuse trace: {} ({}) reads all bodies at {at}", f.name, f.type_id);
                if let Ok(dir) = std::env::var("NOSHAPE_REUSE_TRACE_DIR") {
                    let _ = std::fs::write(format!("{dir}/{}-{}.txt", f.type_id, f.id.0), &t);
                }
            }
            let reads = body_reads.filter(|r| !r.all.load(std::sync::atomic::Ordering::Relaxed)).map(|r| Arc::new(ReadSet { bodies: r.bodies.lock().clone(), lookups: r.lookups.lock().clone(), bounds: *r.bounds.lock(), scopes: r.scopes.lock().clone() }));
            let versions = self.entries.entry(f.id).or_default();
            versions.insert(0, CacheEntry { key, ctx: ctx.clone(), state: state.clone(), last_run: run, local, input: Some(input), reads });
            versions.truncate(VERSIONS);
            states.push(state);
        }
        // Features gone from this run: the most recently used few stay a while.
        let last_run = |versions: &[CacheEntry]| versions.first().map_or(0, |e| e.last_run);
        let mut gone: Vec<(u64, FeatureId)> = self.entries.iter().filter(|(id, v)| !live.contains(id) && run - last_run(v) <= KEEP_RUNS).map(|(id, v)| (last_run(v), *id)).collect();
        gone.sort_unstable_by_key(|g| std::cmp::Reverse(g.0));
        gone.truncate(KEEP_GONE);
        self.entries.retain(|id, _| live.contains(id) || gone.iter().any(|(_, g)| g == id));
        if opts.cancel.as_ref().is_some_and(RegenCancel::superseded) {
            return cancelled();
        }
        if display::perf_enabled() {
            log::info!(target: "perf", "regen: {} features evaluated, {cached} cached, {:.0} ms (preview meshes {})", evaluated.len(), t0.elapsed().as_secs_f64() * 1e3, opts.preview_meshes);
        }
        let (mut display, needs_refine) = display::build(&ctx, studio, opts, &mut self.meshes, &mut self.mesh_owners, &mut self.sketch_displays, run);
        if aborted() {
            return cancelled();
        }
        apply_configured_names(&mut display, studio, config);
        // The shown Assembly context: its bodies, translucent and pickable.
        if let Some(active) = studio.active_context
            && let Some(c) = studio.contexts.iter().find(|c| c.id == active)
            && let Some((bodies, shown)) = self.contexts.get(&(c.id, c.revision))
        {
            display.context_bodies = shown.clone();
            display.context_shapes = bodies.clone();
        }
        display.features = states;
        display.variables = ctx.variables.clone();
        display.regen_ms = t0.elapsed().as_secs_f32() * 1000.0;
        let result = RegenResult { display, needs_refine, context: ctx, cancelled: false, evaluated };
        // (Full results only: a preview can't serve a full run, and the dialog's previews as values are typed would
        // push out the states undo goes back to.)
        if let Some(k) = result_key.filter(|_| !result.needs_refine) {
            self.results.retain(|(x, _)| *x != k);
            self.results.insert(0, (k, result.clone()));
            self.results.truncate(RESULTS);
            let mut seen = std::collections::HashSet::new();
            let mut bytes = 0;
            let keep = self
                .results
                .iter()
                .position(|(_, r)| {
                    for b in &r.display.bodies {
                        if seen.insert(Arc::as_ptr(&b.mesh)) {
                            let m = &b.mesh;
                            bytes += 12 * (m.positions.len() + m.normals.len() + m.vertices.len() + m.mesh_lines.len() + m.edges.iter().map(Vec::len).sum::<usize>()) + 4 * m.indices.len();
                        }
                    }
                    bytes > RESULT_MESH_BYTES
                })
                .map_or(self.results.len(), |i| i.max(1));
            self.results.truncate(keep);
        }
        result
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

/// Live's second line for selections that no longer resolve.
pub fn missing_text(n: usize) -> String {
    if n == 1 { "1 missing selection".into() } else { format!("{n} missing selections") }
}

/// Regeneration: a configured Name renames the part in the Parts list and
/// a configured Appearance colors it, for configurations that have a row
/// (live 644, 647).
fn apply_configured_names(display: &mut crate::display::StudioDisplay, ps: &PartStudio, active: &ConfigValues) {
    for b in &mut display.bodies {
        if let Some(n) = configured_value(ps, &b.id, "Name", active).filter(|n| !n.trim().is_empty()) {
            b.name = n;
        }
        if let Some(c) = configured_value(ps, &b.id, "Appearance", active).and_then(|v| parse_color(&v)) {
            b.color = c;
        }
    }
}
