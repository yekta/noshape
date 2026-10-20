//! The modeling runtime of FeatureScript: the `Context` builtin and the
//! `@` natives the standard library calls (queries, evaluations,
//! operations, sketches, feature status).
//!
//! A feature evaluation wraps the Part Studio's regeneration [`Context`] in
//! an [`FsContext`]. Every operation runs our regular evaluator helpers
//! ([`EvalCtx`]) with a *marker* feature id derived from the operation's
//! FeatureScript id (`id + "extrude1"`), then renames what it created so
//! names belong to the Part Studio feature again:
//! `Face { feature: <custom feature>, role: Generated(Face { feature:
//! <marker>, role }) }`. Names therefore stay deterministic (they depend on
//! the operation ids, not on the order of operations), `creator()` is the
//! custom feature, and `qCreatedBy(id + "extrude1")` finds the marker.

mod builtin;
pub(crate) mod convert;
mod evaluate;
pub mod hole;
mod sheetmetal;
mod offsets;
mod packed;
mod ops;
mod pattern;
pub mod query;
mod sketch;
mod status;
pub(crate) use status::quantity_of;

pub use convert::*;
pub use query::{Ent, entity_type_of};

use crate::reference::{BodyId, FeatureId, Reference, Role, TopoName};
use crate::regen::context::{Body, Context};
use crate::regen::eval::{EvalCtx, FeatureError};
use glam::DMat4;
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::{Builtin, Tag, Value, next_serial};
use noshape_units::{Quantity, UnitPrefs};
use parking_lot::Mutex;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Once};

/// A FeatureScript operation or feature id (`["F1a2b", "extrude1"]`).
pub type FsId = Vec<Arc<str>>;

/// Bit marking operation marker feature ids (Part Studio feature ids use 48 bits).
pub const MARKER_BIT: u64 = 1 << 63;

pub fn is_marker(f: FeatureId) -> bool {
    f.0 & MARKER_BIT != 0
}

/// Marker feature id of a FeatureScript id (stable hash of its components).
pub fn marker_of(id: &[Arc<str>]) -> FeatureId {
    // FNV-1a: stable across runs and platforms.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for c in id {
        for b in c.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    FeatureId(MARKER_BIT | (h & !MARKER_BIT))
}

/// What a FeatureScript id refers to.
#[derive(Clone, Debug)]
pub struct OpInfo {
    pub id: FsId,
    /// Order of the operation within the feature (later = newer).
    pub seq: u32,
}

/// One running feature (`@startFeature`): the context to restore on abort.
struct Started {
    id: FsId,
    snapshot: Context,
}

/// Mutable modeling state of one feature evaluation.
pub struct CtxState {
    pub ctx: Context,
    /// The Part Studio feature being evaluated.
    pub feature: FeatureId,
    pub feature_name: String,
    pub units: UnitPrefs,
    pub outer_vars: BTreeMap<String, Quantity>,
    pub xform: Option<DMat4>,
    transient_ids: HashMap<Arc<str>, Reference>,
    transient_of: HashMap<Reference, Arc<str>>,
    /// Marker feature id -> operation.
    pub ops: HashMap<FeatureId, OpInfo>,
    /// Bodies merged by an operation (union, intersection of tools) are
    /// also "created by" the creators of each merged body and by the
    /// merging operation (std query.fs, qCreatedBy).
    pub merged_creators: HashMap<BodyId, Vec<FsId>>,
    /// The operation that created each body (exact, see `body_creator`).
    pub body_ops: HashMap<BodyId, FsId>,
    used_ids: HashSet<FsId>,
    next_seq: u32,
    /// Feature status per id (`@functionReportFeatureStatus`).
    pub status: HashMap<FsId, Value>,
    started: Vec<Started>,
    /// FeatureScript-only variables (values that are not quantities).
    pub variables: BTreeMap<String, Value>,
    pub attributes: Vec<(Reference, Value)>,
    /// `debug()` entities (shown highlighted while the feature is edited).
    pub debug_entities: Vec<Reference>,
    /// Sketches being built (`newSketch` .. `skSolve`), by sketch id.
    pub sketches: HashMap<FsId, sketch::SketchBuild>,
    /// String entity ids of FeatureScript sketches.
    pub sketch_names: sketch::SketchNames,
    /// Bodies hidden from the Part Studio (std sheet metal definition
    /// sheets): skipped by `qEverything` & co., found by attribute queries.
    pub hidden: HashSet<BodyId>,
    /// Holes built by each `opHole` (`qOpHoleFace` / `qOpHoleProfile`).
    pub holes: HashMap<FsId, Vec<hole::HoleRecord>>,
}

/// The `Context` builtin.
pub struct FsContext {
    serial: u64,
    pub state: Mutex<CtxState>,
}

impl std::fmt::Debug for FsContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Context({})", self.serial)
    }
}

impl Builtin for FsContext {
    fn type_name(&self) -> &'static str {
        "Context"
    }
    fn serial(&self) -> u64 {
        self.serial
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl FsContext {
    pub fn new(mut ctx: Context, feature: FeatureId, feature_name: String, units: UnitPrefs, outer_vars: BTreeMap<String, Quantity>, xform: Option<DMat4>) -> Arc<Self> {
        // Hidden bodies and attributes of earlier features come back.
        let hidden_bodies = std::mem::take(&mut ctx.fs_hidden_bodies);
        let hidden: HashSet<BodyId> = hidden_bodies.iter().map(|b| b.id.clone()).collect();
        ctx.bodies.extend(hidden_bodies);
        let attributes = std::mem::take(&mut ctx.fs_attributes);
        Arc::new(FsContext {
            serial: next_serial(),
            state: Mutex::new(CtxState {
                ctx,
                feature,
                feature_name,
                units,
                outer_vars,
                xform,
                transient_ids: HashMap::new(),
                transient_of: HashMap::new(),
                ops: HashMap::new(),
                merged_creators: HashMap::new(),
                body_ops: HashMap::new(),
                used_ids: HashSet::new(),
                next_seq: 1,
                status: HashMap::new(),
                started: Vec::new(),
                variables: BTreeMap::new(),
                attributes,
                debug_entities: Vec::new(),
                sketches: HashMap::new(),
                sketch_names: HashMap::new(),
                hidden,
                holes: HashMap::new(),
            }),
        })
    }

    /// The regeneration context after the feature: hidden bodies and
    /// attributes move to their side stores for the next feature.
    pub fn take_context(&self) -> Context {
        let mut st = self.state.lock();
        let mut ctx = std::mem::take(&mut st.ctx);
        let (hidden, shown): (Vec<_>, Vec<_>) = std::mem::take(&mut ctx.bodies).into_iter().partition(|b| st.hidden.contains(&b.id));
        ctx.bodies = shown;
        ctx.fs_hidden_bodies = hidden;
        ctx.fs_attributes = st.attributes.clone();
        ctx
    }

    /// The context as a FeatureScript value (tagged `Context`).
    pub fn value(self: &Arc<Self>) -> Value {
        Value::Builtin(self.clone(), std_tag("context.fs", "Context"))
    }
}

impl CtxState {
    /// Transient id string of an entity (stable within this evaluation).
    pub fn transient(&mut self, r: &Reference) -> Arc<str> {
        if let Some(s) = self.transient_of.get(r) {
            return s.clone();
        }
        // Onshape-style three letter ids: JAA, JAB, ...
        let mut n = self.transient_ids.len();
        let mut s = String::new();
        loop {
            s.insert(0, (b'A' + (n % 26) as u8) as char);
            n /= 26;
            if n == 0 {
                break;
            }
        }
        while s.len() < 2 {
            s.insert(0, 'A');
        }
        let id: Arc<str> = format!("J{s}").into();
        self.transient_ids.insert(id.clone(), r.clone());
        self.transient_of.insert(r.clone(), id.clone());
        id
    }

    /// Give an entity a transient id chosen by the host (queries made
    /// before the context existed, e.g. `PartStudioData.partQuery`).
    pub fn name_transient(&mut self, id: Arc<str>, r: Reference) {
        self.transient_of.entry(r.clone()).or_insert_with(|| id.clone());
        self.transient_ids.insert(id, r);
    }

    pub fn transient_ref(&self, id: &str) -> Option<&Reference> {
        self.transient_ids.get(id)
    }

    /// Register an operation id; `None` if it was used before.
    pub fn begin_op(&mut self, id: &[Arc<str>]) -> Option<FeatureId> {
        if !self.used_ids.insert(id.to_vec()) {
            return None;
        }
        let marker = marker_of(id);
        let seq = self.next_seq;
        self.next_seq += 1;
        self.ops.insert(marker, OpInfo { id: id.to_vec(), seq });
        Some(marker)
    }

    /// The id of the operation (or feature) that created a body.
    pub fn body_creator(&self, id: &BodyId) -> Option<FsId> {
        if id.feature == self.feature {
            // Operation bodies carry the operation's marker bits.
            // Bodies an operation created are recorded when renamed.
            if let Some(op) = self.body_ops.get(id) {
                return Some(op.clone());
            }
            if let Some(op) = self.ops.iter().find(|(m, _)| marker_bits(**m) == (id.index & 0xFFFF_F000)) {
                return Some(op.1.id.clone());
            }
        }
        if is_marker(id.feature) {
            return self.ops.get(&id.feature).map(|o| o.id.clone());
        }
        Some(vec![Arc::from(id.feature.to_string())])
    }

    /// Record that operation `op` merged `merged` into the body `result`.
    pub fn record_merge(&mut self, result: &BodyId, merged: &[BodyId], op: &[Arc<str>]) {
        let mut creators: Vec<FsId> = vec![op.to_vec()];
        for b in merged {
            creators.extend(self.body_creator(b));
            creators.extend(self.merged_creators.get(b).cloned().unwrap_or_default());
        }
        creators.sort();
        creators.dedup();
        self.merged_creators.insert(result.clone(), creators);
    }

    /// The operation that created a face name (through the marker wrap),
    /// `None` for faces of other features.
    pub fn op_of_face(&self, n: &TopoName) -> Option<&OpInfo> {
        op_marker(n).and_then(|m| self.ops.get(&m))
    }
}

/// The marker inside a wrapped face name (after following splits).
pub fn op_marker(n: &TopoName) -> Option<FeatureId> {
    match n.root() {
        TopoName::Face { role: Role::Generated(inner), .. } => match inner.root() {
            TopoName::Face { feature, .. } if is_marker(*feature) => Some(*feature),
            _ => None,
        },
        TopoName::Face { feature, .. } | TopoName::Wire { feature, .. } if is_marker(*feature) => Some(*feature),
        // Wire / point body names wrapped by `wrap_name`.
        TopoName::Wire { source: crate::reference::Source::Topo(inner), .. } => match &**inner {
            TopoName::Face { feature, .. } if is_marker(*feature) => Some(*feature),
            _ => None,
        },
        _ => None,
    }
}

// ---------------------------------------------------------------- renaming

/// Rename a name created under `marker` so it belongs to `feature`.
fn wrap_name(n: &Arc<TopoName>, marker: FeatureId, feature: FeatureId) -> Arc<TopoName> {
    match &**n {
        TopoName::Face { feature: f, .. } if *f == marker => Arc::new(TopoName::face(feature, Role::Generated(n.clone()))),
        TopoName::Split { parent, by, ordinal } => {
            let p = wrap_name(parent, marker, feature);
            let by2 = if *by == marker { feature } else { *by };
            if Arc::ptr_eq(&p, parent) && by2 == *by {
                return n.clone();
            }
            Arc::new(TopoName::Split { parent: p, by: by2, ordinal: *ordinal })
        }
        TopoName::Wire { feature: f, source } if *f == marker => Arc::new(TopoName::Wire { feature, source: crate::reference::Source::Topo(Arc::new(TopoName::face(marker, Role::Generated(Arc::new(TopoName::Wire { feature: marker, source: source.clone() }))))) }),
        _ => n.clone(),
    }
}

fn mentions(n: &TopoName, marker: FeatureId) -> bool {
    match n {
        TopoName::Face { feature, .. } | TopoName::Wire { feature, .. } => *feature == marker,
        TopoName::Split { parent, by, .. } => *by == marker || mentions(parent, marker),
        _ => false,
    }
}

/// Body id for a body an operation created: stable per operation id.
fn wrap_body_id(id: &BodyId, marker: FeatureId, feature: FeatureId) -> BodyId {
    if id.feature != marker {
        return id.clone();
    }
    BodyId { feature, index: marker_bits(marker) | (id.index & 0xFFF) }
}

/// 20 well-mixed bits of an operation's marker for its bodies' indices
/// (FNV-1a's high bits hardly change between ids that differ only at the
/// end, like `id + "a"` / `id + "b"`).
fn marker_bits(marker: FeatureId) -> u32 {
    // splitmix64 finalizer.
    let mut z = marker.0;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    ((z >> 44) as u32) << 12
}

/// After an operation: give everything it created names of the custom
/// feature (see the module docs).
pub fn rename_created(ctx: &mut Context, marker: FeatureId, feature: FeatureId) {
    for b in ctx.bodies.iter_mut() {
        let touches = b.id.feature == marker || b.faces.iter().any(|f| mentions(f, marker)) || (b.faces.is_empty() && b.edges.iter().any(|e| mentions(e, marker)));
        if !touches {
            continue;
        }
        let faces: Vec<Arc<TopoName>> = b.faces.iter().map(|f| wrap_name(f, marker, feature)).collect();
        let id = wrap_body_id(&b.id, marker, feature);
        let mut nb = Body::new(id, b.name.clone(), b.kind, b.shape.clone(), faces, b.color, feature);
        // Keep what the operation made it (a composite part's members).
        nb.composite = b.composite.clone().map(|mut c| {
            c.members = c.members.iter().map(|m| wrap_body_id(m, marker, feature)).collect();
            c
        });
        if b.faces.is_empty() {
            nb.edges = b.edges.iter().map(|e| wrap_name(e, marker, feature)).collect();
            nb.vertices = b.vertices.iter().map(|e| wrap_name(e, marker, feature)).collect();
        }
        *b = Arc::new(nb);
    }
}

// ---------------------------------------------------------------- natives

thread_local! {
    /// Other Part Studios of the document for the feature running on this
    /// thread (`PartStudioData.buildFunction`).
    static DERIVED: std::cell::RefCell<Option<Arc<dyn crate::regen::derived::DerivedProvider>>> = const { std::cell::RefCell::new(None) };
}

pub fn set_derived(d: Option<Arc<dyn crate::regen::derived::DerivedProvider>>) {
    DERIVED.with(|c| *c.borrow_mut() = d);
}

pub fn derived() -> Option<Arc<dyn crate::regen::derived::DerivedProvider>> {
    DERIVED.with(|c| c.borrow().clone())
}

static REGISTER: Once = Once::new();

/// Register every modeling native with the FeatureScript runtime (once,
/// before any Feature Studio is compiled).
pub fn register() {
    REGISTER.call_once(|| {
        let mut all: Vec<(&'static str, NativeFn)> = Vec::new();
        all.extend_from_slice(status::NATIVES);
        all.extend_from_slice(query::NATIVES);
        all.extend_from_slice(evaluate::NATIVES);
        all.extend_from_slice(hole::NATIVES);
        all.extend_from_slice(sheetmetal::NATIVES);
        all.extend_from_slice(offsets::NATIVES);
        all.extend_from_slice(packed::NATIVES);
        all.extend_from_slice(ops::NATIVES);
        all.extend_from_slice(sketch::NATIVES);
        all.extend_from_slice(pattern::NATIVES);
        all.extend_from_slice(builtin::NATIVES);
        all.extend_from_slice(super::reference::NATIVES);
        noshape_featurescript::interp::register_natives(&all);

    });
}

/// The context argument of a native.
pub fn context_arg(i: &mut Interp, a: &[Value]) -> R<Arc<FsContext>> {
    if let Some(Value::Builtin(b, _)) = a.first() {
        let any: Arc<dyn std::any::Any + Send + Sync> = b.clone();
        if let Ok(c) = any.downcast::<FsContext>() {
            return Ok(c);
        }
    }
    Err(i.error("Argument 1 should be a Context"))
}

/// Run a modeling operation with id `id` against the context: the closure
/// gets an [`EvalCtx`] whose feature is the operation's marker; created
/// geometry is renamed afterwards. Failures restore the context, record the
/// error status on `id` and raise `{ message : <error> }`.
pub fn run_op<T>(i: &mut Interp, cx: &FsContext, id: &[Arc<str>], failure: &str, f: impl FnOnce(&mut EvalCtx) -> Result<T, FeatureError>) -> R<T> {
    run_op_with(i, cx, id, failure, &crate::params::ParamValues::default(), f)
}

/// [`run_op`] with parameters: an operation implemented by a built-in
/// feature's evaluator (its definition translated to that feature's
/// parameters).
pub fn run_op_with<T>(i: &mut Interp, cx: &FsContext, id: &[Arc<str>], failure: &str, params: &crate::params::ParamValues, f: impl FnOnce(&mut EvalCtx) -> Result<T, FeatureError>) -> R<T> {
    let mut st = cx.state.lock();
    let Some(marker) = st.begin_op(id) else {
        drop(st);
        return Err(raise_error(i, "DUPLICATE_ID", None));
    };
    let snapshot = st.ctx.clone();
    let ctx = std::mem::take(&mut st.ctx);
    let units = st.units;
    let outer = st.outer_vars.clone();
    let name = st.feature_name.clone();
    let mut ev = EvalCtx::new(marker, &name, params, &units, ctx, &outer);
    ev.xform = st.xform;
    let r = f(&mut ev);
    let warnings = std::mem::take(&mut ev.warnings);
    let mut ctx = ev.ctx;
    match r {
        Ok(v) => {
            let created: Vec<BodyId> = ctx.bodies.iter().filter(|b| b.id.feature == marker).map(|b| wrap_body_id(&b.id, marker, st.feature)).collect();
            rename_created(&mut ctx, marker, st.feature);
            for b in created {
                st.body_ops.insert(b, id.to_vec());
            }
            st.ctx = ctx;
            for w in warnings {
                let s = status::status_map("WARNING", "CUSTOM_ERROR", Some(&w));
                st.status.insert(id.to_vec(), s);
            }
            Ok(v)
        }
        Err(e) => {
            st.ctx = snapshot;
            // `code_error`: a specific ErrorStringEnum member.
            let (code, msg, faulty) = match e.message.strip_prefix('\u{1}') {
                Some(c) => {
                    let (c, faulty) = c.split_once('\u{2}').unwrap_or((c, ""));
                    (c.to_string(), error_text(c), faulty.split(',').filter(|f| !f.is_empty()).map(str::to_string).collect::<Vec<_>>())
                }
                None => (failure.to_string(), e.message.clone(), Vec::new()),
            };
            st.status.insert(id.to_vec(), status::status_map("ERROR", &code, Some(&msg)));
            drop(st);
            log::debug!("FeatureScript operation {} failed: {msg}", id.join("."));
            let faulty: Vec<&str> = faulty.iter().map(String::as_str).collect();
            Err(raise_with(i, &code, (code == "CUSTOM_ERROR").then_some(msg.as_str()), &faulty))
        }
    }
}

/// An operation failure reported as a specific `ErrorStringEnum` member.
pub fn code_error(code: &str) -> FeatureError {
    FeatureError::new(format!("\u{1}{code}"))
}

/// [`code_error`] naming the definition fields at fault (`faultyParameters`).
pub fn code_error_faulty(code: &str, faulty: &[&str]) -> FeatureError {
    FeatureError::new(format!("\u{1}{code}\u{2}{}", faulty.join(",")))
}

/// Raise `{ "message" : ErrorStringEnum.<name> }` (or a custom message).
pub fn raise_error(i: &mut Interp, enum_name: &str, custom: Option<&str>) -> Box<noshape_featurescript::Exception> {
    raise_with(i, enum_name, custom, &[])
}

/// [`raise_error`] naming the definition fields at fault (`faultyParameters`).
pub fn raise_faulty(i: &mut Interp, enum_name: &str, faulty: &[&str]) -> Box<noshape_featurescript::Exception> {
    raise_with(i, enum_name, None, faulty)
}

/// A regenError-style map; the notice reads "@evPlane: CANNOT_RESOLVE_ENTITIES"
/// (live) when raised by a native.
fn raise_with(i: &mut Interp, enum_name: &str, custom: Option<&str>, faulty: &[&str]) -> Box<noshape_featurescript::Exception> {
    let msg = std_enum("errorstringenum.gen.fs", "ErrorStringEnum", enum_name).unwrap_or_else(|| Value::str(enum_name));
    let mut entries = vec![("message", msg)];
    if let Some(c) = custom {
        entries.push(("customMessage", Value::str(c)));
    }
    if !faulty.is_empty() {
        entries.push(("faultyParameters", Value::array(faulty.iter().map(|f| Value::str(*f)).collect())));
    }
    let v = Value::map_from(entries);
    if i.native_name.is_empty() {
        return i.raise(v);
    }
    let notice = format!("@{}: {enum_name}", i.native_name);
    i.raise_noted(v, notice)
}

// ---------------------------------------------------------------- std types

/// Tag of a standard library type (`std_tag("vector.fs", "Vector")`).
pub fn std_tag(module: &str, name: &str) -> Tag {
    static CACHE: std::sync::LazyLock<Mutex<HashMap<(String, String), Tag>>> = std::sync::LazyLock::new(Default::default);
    let key = (module.to_string(), name.to_string());
    if let Some(t) = CACHE.lock().get(&key) {
        return *t;
    }
    let tag = noshape_featurescript::std_module(module).ok().and_then(|m| m.type_tag(name)).unwrap_or(Tag::NONE);
    if !tag.is_none() {
        CACHE.lock().insert(key, tag);
    }
    tag
}

/// An enum member of the standard library (`EntityType.FACE`).
pub fn std_enum(module: &str, enum_name: &str, member: &str) -> Option<Value> {
    let tag = std_tag(module, enum_name);
    if tag.is_none() {
        return None;
    }
    let info = noshape_featurescript::value::type_info(tag);
    info.ordinal(member).map(|o| Value::Enum(tag, o))
}

/// Name of an enum value (or plain string) argument.
pub fn enum_name(v: &Value) -> Option<Arc<str>> {
    v.as_str()
}

/// The user-facing text of an `ErrorStringEnum` member: the comment above
/// it in errorstringenum.gen.fs (what Onshape shows in the feature list).
pub fn error_text(member: &str) -> String {
    static TEXTS: std::sync::LazyLock<HashMap<String, String>> = std::sync::LazyLock::new(|| {
        let mut m = HashMap::new();
        let src = noshape_featurescript::std_source("errorstringenum.gen.fs").unwrap_or("");
        let mut last: Option<String> = None;
        for line in src.lines() {
            let t = line.trim();
            if let Some(c) = t.strip_prefix("/*").and_then(|c| c.strip_suffix("*/")) {
                last = Some(c.trim().to_string());
                continue;
            }
            let name = t.trim_end_matches(',');
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
                if let Some(text) = last.take() {
                    m.insert(name.to_string(), text);
                }
            } else if !t.starts_with('*') && !t.starts_with("/**") {
                last = None;
            }
        }
        m
    });
    TEXTS.get(member).cloned().unwrap_or_else(|| match member {
        "REGEN_ERROR" => "Error regenerating.".to_string(),
        other => other.to_string(),
    })
}
