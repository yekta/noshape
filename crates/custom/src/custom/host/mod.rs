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
mod sm_flat;
mod legacy;
mod loft;
#[cfg(test)]
mod native_args;
mod edge_change;
mod offsets;
pub mod packed;
mod ops;
mod pattern;
pub mod query;
mod sketch;
mod status;
pub(crate) use status::{manipulator_value, quantity_of};

pub use convert::*;
pub use query::{Ent, entity_type_of};
pub use status::{feature_list_refs, feature_list_value};

use crate::reference::{BodyId, FeatureId, Reference, Role, TopoName};
use crate::regen::context::{Body, Context};
use crate::regen::eval::{EvalCtx, FeatureError};
use glam::{DMat4, DVec3};
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

/// The bodies an `opSplitPart` left, and its tool plane (origin, normal).
pub type SplitRecord = (Vec<BodyId>, Option<(DVec3, DVec3)>);

/// Mutable modeling state of one feature evaluation.
/// [`CtxState::face_creators`]: by a face name's address, the name and its creator (operation number, id).
pub type FaceCreators = HashMap<usize, (Arc<TopoName>, Option<(u32, FsId)>)>;

/// Something worked out from a body, by the body's address (the body held, so the address stays its own).
pub type ByBody<T> = Mutex<HashMap<usize, (Arc<crate::regen::context::Body>, Arc<T>)>>;
/// [`CtxState::created_cache`]: by (feature id, entity type), the answer and what it was worked out from.
pub type CreatedCache = HashMap<(FsId, Option<query::EType>), (query::CreatedSeen, Vec<query::Ent>)>;

pub struct CtxState {
    pub ctx: Context,
    /// The Part Studio feature being evaluated.
    pub feature: FeatureId,
    pub feature_name: String,
    pub units: UnitPrefs,
    pub outer_vars: BTreeMap<String, Quantity>,
    pub xform: Option<DMat4>,
    /// A Part Studio loaded for a derive (buildPartStudio): its sketch
    /// bodies are modifiable there (std derive keeps a form studio's flat
    /// sketch with its tools).
    pub loaded: bool,
    transient_ids: HashMap<Arc<str>, Reference>,
    /// Transient ids earlier features of the regeneration handed out ([`Context::fs_transient_next`]): this one's
    /// count on from there.
    transient_base: usize,
    transient_of: HashMap<Reference, Arc<str>>,
    /// Transient ids kept from feature to feature ([`Context::fs_transients`]).
    pub persistent: Vec<(Arc<str>, Reference)>,
    /// Marker feature id -> operation.
    pub ops: HashMap<FeatureId, OpInfo>,
    /// Bodies merged by an operation (union, intersection of tools) are
    /// also "created by" the creators of each merged body and by the
    /// merging operation (std query.fs, qCreatedBy).
    pub merged_creators: HashMap<BodyId, Vec<FsId>>,
    /// The operation that created each body (exact, see `body_creator`).
    pub body_ops: HashMap<BodyId, FsId>,
    /// Faces a Boolean brought into its targets (its tools' faces): created by the Boolean, as their edges with them
    /// (std rib.fs, boolean.fs: qCreatedBy(<the Boolean>, FACE / EDGE); the public Laser Joint's overcuts).
    pub adopted: HashMap<TopoName, OpInfo>,
    /// Each face name's creator (operation number, id) as `ops` and `adopted` give it, by the name's Arc (held, so
    /// its address isn't reused): qCreatedBy asks it of every face, thousands of times in a Fill pattern's loop.
    /// Cleared when `ops` or `adopted` change ([`Self::begin_op`], [`Self::adopt`], [`Self::alias_op`]).
    pub face_creators: Mutex<FaceCreators>,
    /// qCreatedBy's answers by (feature id, entity type), each kept while the context it was worked out in is the
    /// same ([`query::CreatedSeen`]): std asks the same one thousands of times in a loop (a Fill pattern's
    /// instances). Cleared with `face_creators`.
    /// Each body's topology (QueryCx's), by the body (held, so the address stays its own).
    /// Each body's name hashes (QueryCx::from_ref's prefilter), by the body (held).
    pub name_index: ByBody<query::NameIndex>,
    pub topo_cache: ByBody<query::Topo>,
    /// Each body's edges' sketch order keys ([`query::live_order`]), with the number of sketches they were found among.
    pub sketch_order_cache: ByBody<query::SketchKeys>,
    pub created_cache: Mutex<CreatedCache>,
    /// Each face's inner point ([`query::QueryCx::face_point`]), by the face (held, so its identity stays its own).
    pub face_points: Mutex<HashMap<noshape_kernel::Shape, Option<glam::DVec3>>>,
    /// Each shape's box ([`query::QueryCx::shape_box`]), by the shape (held).
    pub shape_boxes: Mutex<HashMap<noshape_kernel::Shape, Option<noshape_kernel::BoundingBox>>>,
    /// The operation that made each construction plane (`ConstructionPlane` references; qCreatedBy).
    pub plane_ops: HashMap<Reference, FsId>,
    /// Each operation's bits of its bodies' ids (unique in this feature).
    body_bits: BodyBits,
    /// The operation number after the last operation that made or changed
    /// each body (bodies from earlier features are not listed).
    pub touched: HashMap<BodyId, u32>,
    used_ids: HashSet<FsId>,
    next_seq: u32,
    /// Feature status per id (`@functionReportFeatureStatus`).
    pub status: HashMap<FsId, Value>,
    started: Vec<Started>,
    /// Every id `startFeature` started, in that order (`valuesSortedById`).
    pub start_order: Vec<FsId>,
    /// FeatureScript-only variables (values that are not quantities).
    pub variables: BTreeMap<String, Value>,
    attrs: Vec<(Reference, Value)>,
    /// `attrs`' positions by entity, built when first asked after a change ([`Self::attributes_of`]).
    attr_index: Mutex<Option<HashMap<Reference, Vec<usize>>>>,
    /// `debug()` entities as they were when debugged (std debugs points,
    /// lines and arrows of sub-features it then aborts) and their colours
    /// (sRGB), shown while the feature is edited.
    pub debug_entities: Vec<(noshape_kernel::Shape, [u8; 3])>,
    /// `setFeatureComputedParameter`: values the dialog shows (Jog's
    /// computed bend angle), handed to the regeneration's feature state.
    pub computed: Vec<(String, crate::display::ComputedValue)>,
    /// `addManipulators`: the feature's manipulators by key, handed to the
    /// regeneration's feature state (drawn while it is edited).
    pub manipulators: Vec<(String, crate::display::FeatureManipulator)>,
    /// Sketches being built (`newSketch` .. `skSolve`), by sketch id.
    pub sketches: HashMap<FsId, sketch::SketchBuild>,
    /// String entity ids of FeatureScript sketches.
    pub sketch_names: sketch::SketchNames,
    /// Geometry of entities when they got their transient ids (TRACKING
    /// queries find what they became after an operation renamed them).
    pub snapshots: HashMap<Arc<str>, query::Snapshot>,
    /// Bodies hidden from the Part Studio (std sheet metal definition
    /// sheets): skipped by `qEverything` & co., found by attribute queries.
    pub hidden: HashSet<BodyId>,
    /// Holes built by each `opHole` (`qOpHoleFace` / `qOpHoleProfile`).
    pub holes: HashMap<FsId, Vec<hole::HoleRecord>>,
    /// What each operation was made from (`qDependency`: an extruded body's
    /// profile faces or sketch edges).
    pub dependencies: HashMap<FsId, Vec<Reference>>,
    /// Bodies each `opSplitPart` left and its tool plane (`qSplitBy`).
    pub splits: HashMap<FsId, SplitRecord>,
    /// Where each attributed face, edge or vertex was (Parasolid attributes
    /// follow the topology; our references follow names, which an operation
    /// can change: [`CtxState::remap_attributes`]).
    pub attr_snapshots: HashMap<Reference, (BodyId, query::Snapshot)>,
    /// Bumped each time an operation changes a body (cached transient ids
    /// of its edges and vertices are checked again: names get reused).
    body_versions: HashMap<BodyId, u64>,
    /// The body version each transient id was last checked at.
    transient_versions: HashMap<Arc<str>, u64>,
    /// Each body's topology version: bumped when an operation changes its entities' names (a transform or an edge
    /// move keeps them), so an edge's or vertex's name, an ordinal among its neighbours', may then stand for another.
    topo_versions: HashMap<BodyId, u64>,
    /// The owner's topology version when each edge's or vertex's transient id was taken.
    transient_topo: HashMap<Arc<str>, u64>,
    /// The edges each `opSplitEdges` split (`qSplitBy` finds their pieces).
    pub split_edges: HashMap<FsId, Vec<(BodyId, noshape_kernel::Shape)>>,
    /// The bodies each `opPattern` instance made, by (operation id, instance name) (`qPatternInstances`).
    pub pattern_instances: HashMap<(FsId, String), Vec<BodyId>>,
    /// The Part Studio features a FeatureList parameter of this feature picked (their functions re-run them).
    pub feature_defs: Vec<crate::regen::eval::EffectiveFeature>,
    /// `setFeaturePatternInstanceData`: each instance id's transform while std's pattern re-runs features under it.
    pub instance_transforms: HashMap<FsId, DMat4>,
    /// Sketches a re-run instance solved again: the originals, put back when the instance is done.
    pub replaced_sketches: HashMap<FeatureId, Arc<crate::regen::context::SketchResult>>,
    /// `opSplitFace` planar tools: a point and normal of each tool's plane
    /// and the midpoints of the edges it cut (tracking a tool face finds
    /// them, std's partial dependency: sheet metal Bend's boundaries).
    pub split_tools: Vec<(DVec3, DVec3, Vec<DVec3>)>,
    /// The face pieces each `opSplitFace` left (their snapshot points; `qSplitBy` faces).
    pub split_face_pieces: HashMap<FsId, Vec<DVec3>>,
    /// The bodies each `opSplitFace` / `opSplitPart` cut, the midpoints of the edges it made there and its
    /// operation number, in order: those edges are `qCreatedBy` it, whatever later operations rename them (an
    /// `opDeleteFace` leaving them open), also on a body touched since (a split sheet's piece got a new id).
    pub split_face_edges: Vec<(FsId, Vec<BodyId>, Vec<DVec3>, u32)>,
    /// Edges `opEdgeChange` moved: their start, middle and end before and after, in order (a tracked edge is
    /// found where it went).
    pub edge_moves: Vec<(Vec<DVec3>, Vec<DVec3>)>,
    /// The same for the vertices an operation made where its new edges end (a localized opBoolean: the corner
    /// overcut's corner breaks go on `qCreatedBy(<its Boolean>, VERTEX)`), by their points.
    pub made_vertices: Vec<(FsId, Vec<BodyId>, Vec<DVec3>)>,
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
        let persistent = std::mem::take(&mut ctx.fs_transients);
        let transient_base = ctx.fs_transient_next;
        let cx = Arc::new(FsContext {
            serial: next_serial(),
            state: Mutex::new(CtxState {
                ctx,
                feature,
                feature_name,
                units,
                outer_vars,
                xform,
                loaded: false,
                transient_ids: HashMap::new(),
                transient_base,
                transient_of: HashMap::new(),
                persistent,
                ops: HashMap::new(),
                merged_creators: HashMap::new(),
                body_ops: HashMap::new(),
                adopted: HashMap::new(),
                face_creators: Mutex::new(HashMap::new()),
                created_cache: Mutex::new(HashMap::new()),
                face_points: Mutex::new(HashMap::new()),
                shape_boxes: Mutex::new(HashMap::new()),
                topo_cache: Mutex::new(HashMap::new()),
                sketch_order_cache: Mutex::new(HashMap::new()),
                name_index: Mutex::new(HashMap::new()),
                plane_ops: HashMap::new(),
                body_bits: BodyBits::default(),
                touched: HashMap::new(),
                used_ids: HashSet::new(),
                next_seq: 1,
                status: HashMap::new(),
                started: Vec::new(),
                start_order: Vec::new(),
                variables: BTreeMap::new(),
                attrs: attributes,
                attr_index: Mutex::new(None),
                debug_entities: Vec::new(),
                computed: Vec::new(),
                manipulators: Vec::new(),
                sketches: HashMap::new(),
                sketch_names: HashMap::new(),
                hidden,
                snapshots: HashMap::new(),
                holes: HashMap::new(),
                dependencies: HashMap::new(),
                splits: HashMap::new(),
                split_edges: HashMap::new(),
                pattern_instances: HashMap::new(),
                feature_defs: Vec::new(),
                instance_transforms: HashMap::new(),
                replaced_sketches: HashMap::new(),
                split_tools: Vec::new(),
                split_face_pieces: HashMap::new(),
                split_face_edges: Vec::new(),
                edge_moves: Vec::new(),
                made_vertices: Vec::new(),
                attr_snapshots: HashMap::new(),
                body_versions: HashMap::new(),
                transient_versions: HashMap::new(),
                topo_versions: HashMap::new(),
                transient_topo: HashMap::new(),
            }),
        });
        // Where the attributed entities are now (for renames by this feature).
        {
            let mut st = cx.state.lock();
            let snaps: HashMap<Reference, (BodyId, query::Snapshot)> = {
                let q = query::QueryCx::new(&st);
                let mut out = HashMap::new();
                for (r, _) in st.attributes() {
                    if !out.contains_key(r)
                        && let Some(s) = q.attribute_snapshot(r)
                    {
                        out.insert(r.clone(), s);
                    }
                }
                out
            };
            st.attr_snapshots = snaps;
        }
        cx
    }

    /// The regeneration context after the feature: hidden bodies and
    /// attributes move to their side stores for the next feature.
    pub fn take_context(&self) -> Context {
        let mut st = self.state.lock();
        let mut ctx = std::mem::take(&mut st.ctx);
        let (hidden, shown): (Vec<_>, Vec<_>) = std::mem::take(&mut ctx.bodies).into_iter().partition(|b| st.hidden.contains(&b.id));
        ctx.bodies = (shown).into();
        ctx.fs_hidden_bodies = hidden;
        ctx.fs_attributes = st.attributes().to_vec();
        ctx.fs_transients = st.persistent.clone();
        ctx.fs_transient_next = st.transient_base + st.transient_ids.len();
        ctx
    }

    /// The context as a FeatureScript value (tagged `Context`).
    pub fn value(self: &Arc<Self>) -> Value {
        Value::Builtin(self.clone(), std_tag("context.fs", "Context"))
    }
}

impl CtxState {
    /// Transient id string of an entity (stable within this evaluation).
    /// Every attribute: (entity, `{ name, attribute }`), in the order they were set.
    pub fn attributes(&self) -> &[(Reference, Value)] {
        &self.attrs
    }

    /// The attributes to change (their index is rebuilt when next asked).
    pub fn attributes_mut(&mut self) -> &mut Vec<(Reference, Value)> {
        *self.attr_index.get_mut() = None;
        &mut self.attrs
    }

    /// The attributes of entity `r` (`{ name, attribute }`), in order: from an index of them all (std asks this of
    /// each of thousands of entities after a pattern).
    pub fn attributes_of(&self, r: &Reference) -> Vec<&Value> {
        let mut index = self.attr_index.lock();
        let index = index.get_or_insert_with(|| {
            let mut m: HashMap<Reference, Vec<usize>> = HashMap::new();
            for (k, (e, _)) in self.attrs.iter().enumerate() {
                m.entry(e.clone()).or_default().push(k);
            }
            m
        });
        index.get(r).map(|ks| ks.iter().map(|k| &self.attrs[*k].1).collect()).unwrap_or_default()
    }

    /// Attach an attribute to `r`.
    pub fn push_attribute(&mut self, r: Reference, v: Value) {
        if !self.attr_snapshots.contains_key(&r)
            && let Some(s) = query::QueryCx::new(self).attribute_snapshot(&r)
        {
            self.attr_snapshots.insert(r.clone(), s);
        }
        self.attributes_mut().push((r, v));
    }

    /// Attributes whose entity an operation renamed follow it (a sheet
    /// metal definition's vertices are renamed when a flange adds faces
    /// beside them; live keeps their association attributes).
    pub fn remap_attributes(&mut self, changed: &HashSet<BodyId>) {
        // Each moved entity's new place, or its pieces when split (each
        // gets the attribute, as Parasolid's split keeps it on both).
        type Placed = (Reference, (BodyId, query::Snapshot));
        let (moves, gone): (Vec<(Reference, Vec<Placed>)>, HashSet<Reference>) = {
            let q = query::QueryCx::new(self);
            let (mut out, mut gone) = (Vec::new(), HashSet::new());
            for (r, (body, snap)) in &self.attr_snapshots {
                if !changed.contains(body) {
                    continue;
                }
                let Some(pieces) = query::relocated_pieces(&q, r, snap) else { continue };
                if pieces.is_empty() {
                    gone.insert(r.clone());
                    continue;
                }
                let to: Vec<Placed> = pieces.iter().filter_map(|e| Some((q.to_ref(e), q.attribute_snapshot(&q.to_ref(e))?))).collect();
                // Nothing that can carry it lies where it was (a sketch's point there): gone as well.
                if to.is_empty() {
                    gone.insert(r.clone());
                } else {
                    out.push((r.clone(), to));
                }
            }
            (out, gone)
        };
        // Gone entities' attributes first (by their old names: one renamed onto such a name keeps its own).
        if !gone.is_empty() {
            self.attributes_mut().retain(|(r, _)| !gone.contains(r));
            self.attr_snapshots.retain(|r, _| !gone.contains(r));
        }
        if moves.is_empty() {
            return;
        }
        // All at once: one entity's new name may be another's old one.
        let map: HashMap<Reference, Vec<Reference>> = moves.iter().map(|(o, to)| (o.clone(), to.iter().map(|(n, _)| n.clone()).collect())).collect();
        let mut copies = Vec::new();
        for (r, v) in self.attributes_mut().iter_mut() {
            if let Some(to) = map.get(r) {
                copies.extend(to[1..].iter().map(|n| (n.clone(), v.clone())));
                *r = to[0].clone();
            }
        }
        self.attributes_mut().extend(copies);
        for (old, _) in &moves {
            self.attr_snapshots.remove(old);
        }
        for (_, to) in moves {
            for (new, snap) in to {
                self.attr_snapshots.insert(new, snap);
            }
        }
    }

    pub fn transient(&mut self, r: &Reference) -> Arc<str> {
        if let Some((s, _)) = self.persistent.iter().find(|(_, x)| x == r) {
            return s.clone();
        }
        if let Some(s) = self.transient_of.get(r).cloned() {
            if !matches!(r, Reference::Edge(_) | Reference::Vertex(_)) {
                return s;
            }
            // An edge's or vertex's name may now stand for another entity
            // (ordinals shift when a body changes): keep the id only if the
            // entity is still where the id saw it.
            let (version, now) = {
                let q = query::QueryCx::new(self);
                let e = q.from_ref(r).into_iter().next();
                let version = e.as_ref().and_then(|e| q.owner(e)).and_then(|o| if let query::Ent::Body(b) = o { self.body_versions.get(&self.ctx.bodies[b].id).copied() } else { None }).unwrap_or(0);
                let unchanged = self.transient_versions.get(&s) == Some(&version);
                (version, if unchanged { None } else { Some(e.and_then(|e| q.snapshot(&e))) })
            };
            match now {
                None => return s,
                Some(snap) => {
                    let same = match (snap, self.snapshots.get(&s)) {
                        (Some(a), Some(b)) => a.etype == b.etype && a.points.len() == b.points.len() && a.points.iter().zip(&b.points).all(|(p, q)| p.distance(*q) < 1e-9),
                        _ => true,
                    };
                    if same {
                        self.transient_versions.insert(s.clone(), version);
                        return s;
                    }
                }
            }
        }
        // Onshape-style three letter ids: JAA, JAB, ..., on from the earlier features' (never one twice).
        let mut n = self.transient_base + self.transient_ids.len();
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
        // Where it is now, for tracking it through later operations.
        let snapshot = {
            let q = query::QueryCx::new(self);
            q.from_ref(r).first().and_then(|e| q.snapshot(e))
        };
        if let Some(snap) = snapshot {
            self.snapshots.insert(id.clone(), snap);
        }
        self.transient_ids.insert(id.clone(), r.clone());
        self.transient_of.insert(r.clone(), id.clone());
        let version = {
            let q = query::QueryCx::new(self);
            q.from_ref(r).first().and_then(|e| q.owner(e)).and_then(|o| if let query::Ent::Body(b) = o { self.body_versions.get(&self.ctx.bodies[b].id).copied() } else { None }).unwrap_or(0)
        };
        self.transient_versions.insert(id.clone(), version);
        if matches!(r, Reference::Edge(_) | Reference::Vertex(_)) {
            let topo = self.topo_version_of(r);
            self.transient_topo.insert(id.clone(), topo);
        }
        id
    }

    /// The topology version of the body that `r` (an edge or a vertex) is on now.
    fn topo_version_of(&self, r: &Reference) -> u64 {
        let q = query::QueryCx::new(self);
        q.from_ref(r).first().and_then(|e| q.owner(e)).and_then(|o| if let query::Ent::Body(b) = o { self.topo_versions.get(&self.ctx.bodies[b].id).copied() } else { None }).unwrap_or(0)
    }

    /// Whether transient id `id`'s edge or vertex name may still stand for what it did: its body's topology is the
    /// one it was taken on.
    pub fn same_topology(&self, id: &str, r: &Reference) -> bool {
        self.transient_topo.get(id).is_none_or(|t| *t == self.topo_version_of(r))
    }

    /// Give an entity a transient id chosen by the host (queries made
    /// before the context existed, e.g. `PartStudioData.partQuery`).
    pub fn name_transient(&mut self, id: Arc<str>, r: Reference) {
        self.transient_of.entry(r.clone()).or_insert_with(|| id.clone());
        self.transient_ids.insert(id, r);
    }

    pub fn transient_ref(&self, id: &str) -> Option<&Reference> {
        self.transient_ids.get(id).or_else(|| self.persistent.iter().find(|(s, _)| &**s == id).map(|(_, r)| r))
    }

    /// A transient id that lasts beyond this feature for `r` (outside the
    /// `J..` letters [`Self::transient`] hands out).
    pub fn persist_transient(&mut self, r: &Reference) -> Arc<str> {
        if let Some((s, _)) = self.persistent.iter().find(|(_, x)| x == r) {
            return s.clone();
        }
        let id: Arc<str> = format!("JP{}", self.persistent.len()).into();
        self.persistent.push((id.clone(), r.clone()));
        id
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
        self.face_creators.get_mut().clear();
        self.created_cache.get_mut().clear();
        self.body_bits.assign(marker);
        Some(marker)
    }

    /// Face `n` brought into its target by Boolean `op` (created by it from now on).
    pub fn adopt(&mut self, n: TopoName, op: OpInfo) {
        self.adopted.insert(n, op);
        self.face_creators.get_mut().clear();
        self.created_cache.get_mut().clear();
    }

    /// Marker `marker` names operation `op` too (copies an operation made under other ids).
    pub fn alias_op(&mut self, marker: FeatureId, op: OpInfo) {
        self.ops.insert(marker, op);
        self.face_creators.get_mut().clear();
        self.created_cache.get_mut().clear();
    }

    /// The bits of the ids of the bodies operation `marker` creates.
    pub fn op_bits(&self, marker: FeatureId) -> u32 {
        self.body_bits.of(marker)
    }

    /// The id of the operation (or feature) that created a body.
    pub fn body_creator(&self, id: &BodyId) -> Option<FsId> {
        if id.feature == self.feature {
            // Operation bodies carry the operation's marker bits.
            // Bodies an operation created are recorded when renamed.
            if let Some(op) = self.body_ops.get(id) {
                return Some(op.clone());
            }
            if let Some(op) = self.body_bits.owner(id.index & 0xFFFF_F000).and_then(|m| self.ops.get(&m)) {
                return Some(op.id.clone());
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

/// The bits of operation bodies' ids per operation of one feature. Two
/// operations whose markers share their 20 bits (about 1 feature in 100 of
/// ~150 operations, by the random feature id) gave their bodies one id, and
/// either's qCreatedBy found the other's body (the ops harness on threads: a
/// wire 1.2 m away, a duplicated offset group): the later one takes the next
/// free bits.
#[derive(Default)]
struct BodyBits {
    of: HashMap<FeatureId, u32>,
    owner: HashMap<u32, FeatureId>,
}

impl BodyBits {
    fn assign(&mut self, marker: FeatureId) -> u32 {
        if let Some(b) = self.of.get(&marker) {
            return *b;
        }
        let mut bits = marker_bits(marker);
        while self.owner.contains_key(&bits) {
            bits = bits.wrapping_add(1 << 12);
        }
        self.owner.insert(bits, marker);
        self.of.insert(marker, bits);
        bits
    }

    fn of(&self, marker: FeatureId) -> u32 {
        self.of.get(&marker).copied().unwrap_or_else(|| marker_bits(marker))
    }

    fn owner(&self, bits: u32) -> Option<FeatureId> {
        self.owner.get(&bits).copied()
    }
}

/// Body id for a body an operation created: stable per operation id.
fn wrap_body_id(id: &BodyId, marker: FeatureId, bits: u32, feature: FeatureId) -> BodyId {
    if id.feature != marker {
        return id.clone();
    }
    BodyId { feature, index: bits | (id.index & 0xFFF) }
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
pub fn rename_created(ctx: &mut Context, marker: FeatureId, bits: u32, feature: FeatureId) {
    for b in ctx.bodies.iter_mut() {
        let touches = b.id.feature == marker || b.faces.iter().any(|f| mentions(f, marker)) || (b.faces.is_empty() && b.edges.iter().any(|e| mentions(e, marker)));
        if !touches {
            continue;
        }
        let faces: Vec<Arc<TopoName>> = b.faces.iter().map(|f| wrap_name(f, marker, feature)).collect();
        let id = wrap_body_id(&b.id, marker, bits, feature);
        let mut nb = Body::new(id, b.name.clone(), b.kind, b.shape.clone(), faces, b.color, feature);
        // Keep what the operation made it (a composite part's members).
        nb.composite = b.composite.clone().map(|mut c| {
            c.members = c.members.iter().map(|m| wrap_body_id(m, marker, bits, feature)).collect();
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
    crate::register();
    REGISTER.call_once(|| {
        let mut all: Vec<(&'static str, NativeFn)> = Vec::new();
        all.extend_from_slice(status::NATIVES);
        all.extend_from_slice(query::NATIVES);
        all.extend_from_slice(evaluate::NATIVES);
        all.extend_from_slice(hole::NATIVES);
        all.extend_from_slice(sheetmetal::NATIVES);
        all.extend_from_slice(sm_flat::NATIVES);
        all.extend_from_slice(loft::NATIVES);
        all.extend_from_slice(edge_change::NATIVES);
        all.extend_from_slice(offsets::NATIVES);
        all.extend_from_slice(packed::NATIVES);
        all.extend_from_slice(ops::NATIVES);
        all.extend_from_slice(sketch::NATIVES);
        all.extend_from_slice(pattern::NATIVES);
        all.extend_from_slice(builtin::NATIVES);
        all.extend_from_slice(super::reference::NATIVES);
        noshape_featurescript::interp::register_natives(&all);
        legacy::register();

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
/// `NOSHAPE_FS_TRACE=1`: print every host operation and the bodies' face
/// counts before and after it (debugging std features).
pub(super) fn trace_ops() -> bool {
    static ON: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("NOSHAPE_FS_TRACE").is_ok_and(|v| v == "1"));
    *ON
}

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
    let mut r = f(&mut ev);
    // A cancelled regeneration's operation fails at once, what it made not named (the 45 / 45 knurl's naming took
    // a third of a second after the cancel).
    if r.is_ok() && noshape_kernel::thread_cancelled() {
        r = Err(FeatureError::new("cancelled"));
    }
    if trace_ops() {
        // Operation trace for debugging std features (stderr).
        let faces = |c: &crate::regen::context::Context| {
            c.bodies
                .iter()
                .map(|b| {
                    let bb = b.shape.bounding_box(true).map(|x| format!("[{:.3} {:.3} {:.3} .. {:.3} {:.3} {:.3}]", x.min.x * 1e3, x.min.y * 1e3, x.min.z * 1e3, x.max.x * 1e3, x.max.y * 1e3, x.max.z * 1e3)).unwrap_or_default();
                    // A small sheet: each face's box too.
                    let fb = if b.kind == crate::display::BodyKind::Surface && b.faces.len() <= 8 {
                        b.shape
                            .faces()
                            .iter()
                            .filter_map(|f| {
                                let x = f.bounding_box(true).ok()?;
                                // A planar face's outward normal (orientation matters to std).
                                let n = f.face_plane().ok().flatten().map(|_| {
                                    let [u0, u1, v0, v1] = f.uv_bounds().unwrap_or_default();
                                    f.normal_at_uv((u0 + u1) * 0.5, (v0 + v1) * 0.5).map(|n| format!(" n({:.2} {:.2} {:.2})", n.x, n.y, n.z)).unwrap_or_default()
                                });
                                Some(format!(" f[{:.3} {:.3} {:.3} .. {:.3} {:.3} {:.3}]{}", x.min.x * 1e3, x.min.y * 1e3, x.min.z * 1e3, x.max.x * 1e3, x.max.y * 1e3, x.max.z * 1e3, n.unwrap_or_default()))
                            })
                            .collect::<String>()
                    } else {
                        String::new()
                    };
                    format!("{:?}:{}{bb}{fb}", b.kind, b.faces.len())
                })
                .collect::<Vec<_>>()
                .join(" ")
        };
        let outcome = match &r {
            Ok(_) => "ok".to_string(),
            Err(e) => format!("ERR ({})", e.message),
        };
        eprintln!("[fs op] {} {failure} {} -> {outcome} | {}", id.join("."), faces(&snapshot), faces(&ev.ctx));
    }
    let warnings = std::mem::take(&mut ev.warnings);
    let infos = std::mem::take(&mut ev.infos);
    let mut ctx = ev.ctx;
    match r {
        Ok(v) => {
            let bits = st.op_bits(marker);
            let created: Vec<BodyId> = ctx.bodies.iter().filter(|b| b.id.feature == marker).map(|b| wrap_body_id(&b.id, marker, bits, st.feature)).collect();
            rename_created(&mut ctx, marker, bits, st.feature);
            // Its construction planes: the feature's next ones (listed under it as "Plane n", live 11-02).
            let feature = st.feature;
            let first = ctx.planes.iter().filter(|p| matches!(p.reference, Reference::ConstructionPlane { feature: f, .. } if f == feature)).count() as u32;
            for (index, p) in (first..).zip(ctx.planes.iter_mut().filter(|p| p.reference == Reference::FeaturePlane(marker))) {
                p.reference = Reference::ConstructionPlane { feature, index };
                p.name = format!("Plane {}", index + 1);
                st.plane_ops.insert(p.reference.clone(), id.to_vec());
            }
            let seq = st.next_seq;
            for b in created {
                st.touched.insert(b.clone(), seq);
                st.body_ops.insert(b, id.to_vec());
            }
            // Bodies the operation changed (their attributes may need to follow renames).
            let changed: HashSet<BodyId> = ctx.bodies.iter().filter(|b| snapshot.bodies.iter().any(|o| o.id == b.id && !Arc::ptr_eq(o, b))).map(|b| b.id.clone()).collect();
            for b in &changed {
                st.touched.insert(b.clone(), seq);
            }
            st.ctx = ctx;
            for b in &changed {
                *st.body_versions.entry(b.clone()).or_default() += 1;
                let names = |c: &crate::regen::context::Context| c.bodies.iter().find(|x| &x.id == b).map(|x| (x.faces.clone(), x.edges.clone(), x.vertices.clone()));
                if names(&snapshot) != names(&st.ctx) {
                    *st.topo_versions.entry(b.clone()).or_default() += 1;
                }
            }
            if !changed.is_empty() {
                st.remap_attributes(&changed);
            }
            // An operation's info (opSplitPart's SPLIT_NO_CHANGE, live): its member.
            for m in infos {
                let s = match error_code(&m) {
                    Some(c) => status::status_map("INFO", c, None),
                    None => status::status_map("INFO", "CUSTOM_ERROR", Some(&m)),
                };
                st.status.insert(id.to_vec(), s);
            }
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
                // A built-in evaluator's Onshape message: its member.
                None => match error_code(&e.message) {
                    Some(c) => (c.to_string(), e.message.clone(), e.faulty.iter().map(|f| f.to_string()).collect()),
                    // A kernel limit (memory, CPU) or a cancel is not the
                    // operation's own failure: its words, not the code's (a
                    // knurl's subtract past the memory budget read "Selected
                    // tools and targets do not intersect.", lead 10-30 11:30).
                    // (So is a Boolean noshape's declined with NOSHAPE_BOOL_FALLBACK=0: its reason shown.)
                    None if stopped_by_a_limit(&e.message) || e.message.contains("noshape's Boolean declined") => ("CUSTOM_ERROR".to_string(), e.message.clone(), Vec::new()),
                    None => (failure.to_string(), e.message.clone(), Vec::new()),
                },
            };
            // With its faulty fields: std maps them onto its own parameters
            // (processSubfeatureStatus' featureParameterMap: hole.fs opHole
            // `targets` → Merge scope, as live).
            let mut status = status::status_map("ERROR", &code, Some(&msg));
            if !faulty.is_empty()
                && let Value::Map(m, t) = &status
            {
                let mut m = (**m).clone();
                m.insert(Value::str("faultyParameters"), Value::array(faulty.iter().map(|f| Value::str(f.as_str())).collect()));
                status = Value::Map(Arc::new(m), *t);
            }
            st.status.insert(id.to_vec(), status);
            drop(st);
            log::debug!("FeatureScript operation {} failed: {msg}", id.join("."));
            let faulty: Vec<&str> = faulty.iter().map(String::as_str).collect();
            Err(raise_with(i, &code, (code == "CUSTOM_ERROR").then_some(msg.as_str()), &faulty))
        }
    }
}

/// Whether a kernel error is a limit stopping the operation (the memory or
/// CPU budget, a cancel) rather than its geometry failing.
fn stopped_by_a_limit(message: &str) -> bool {
    ["operation stopped: it needed more than", "took too long", "Cancelled", "cancelled"].iter().any(|m| message.contains(m))
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
    // By module, then name: found without an allocation (plane and point values ask it per coordinate).
    static CACHE: std::sync::LazyLock<parking_lot::RwLock<HashMap<String, HashMap<String, Tag>>>> = std::sync::LazyLock::new(Default::default);
    if let Some(t) = CACHE.read().get(module).and_then(|m| m.get(name)) {
        return *t;
    }
    let tag = noshape_featurescript::std_module(module).ok().and_then(|m| m.type_tag(name)).unwrap_or(Tag::NONE);
    if !tag.is_none() {
        CACHE.write().entry(module.to_string()).or_default().insert(name.to_string(), tag);
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

pub use crate::regen::custom_features::{error_code, error_text};

#[cfg(test)]
mod body_bits_tests {
    use super::*;

    /// Two operations whose markers share their 20 bits get bodies of their
    /// own, each found as its operation's.
    #[test]
    fn colliding_operations_get_bits_of_their_own() {
        let id = |n: &str| -> Vec<Arc<str>> { vec![Arc::from("F1234abcd"), Arc::from(n)] };
        let mut seen: HashMap<u32, String> = HashMap::new();
        let (a, b) = (0..).map(|k| format!("op{k}")).find_map(|n| seen.insert(marker_bits(marker_of(&id(&n))), n.clone()).map(|prev| (prev, n))).unwrap();
        let (ma, mb) = (marker_of(&id(&a)), marker_of(&id(&b)));
        assert_ne!(ma, mb);
        assert_eq!(marker_bits(ma), marker_bits(mb));
        let mut bits = BodyBits::default();
        let (ba, bb) = (bits.assign(ma), bits.assign(mb));
        assert_ne!(ba, bb);
        assert_eq!((bits.owner(ba), bits.owner(bb)), (Some(ma), Some(mb)));
        assert_eq!((bits.assign(ma), bits.of(mb)), (ba, bb));
        let feature = FeatureId(7);
        let body = |m: FeatureId, bits: u32| wrap_body_id(&BodyId { feature: m, index: 0 }, m, bits, feature);
        assert_ne!(body(ma, ba), body(mb, bb));
    }
}
