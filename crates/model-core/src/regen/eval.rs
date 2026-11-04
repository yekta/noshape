//! Feature evaluation context: parameter access, reference resolution,
//! profile construction, history-based naming and boolean merging.

use super::context::{Body, BodyReads, Context, LookupKind, SketchResult, face_key};
use crate::display::{BodyKind, PlaneFrame};
use crate::naming::{FaceNamingInput, HistRecord, Kind, Name, Relation, ShapeNames, name_faces};
use crate::params::{NumKind, ParamValue, ParamValues};
use crate::reference::{BodyId, DatumPlane, FeatureId, Reference, Role, Source, TopoName};
use glam::{DMat4, DVec2, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use noshape_units::{Dim, LayeredScope, Quantity, UnitPrefs, VariableScope};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct FeatureError {
    pub message: String,
    pub warning: bool,
    /// The parameters at fault (Onshape's faulty parameters: their empty
    /// selection fields turn pink and show the message).
    pub faulty: Vec<&'static str>,
}

impl FeatureError {
    pub fn new(m: impl Into<String>) -> Self {
        Self { message: m.into(), warning: false, faulty: Vec::new() }
    }

    /// An error naming the parameters at fault (their fields shown red), as [`err_at`].
    pub fn at(faulty: &[&'static str], m: impl Into<String>) -> Self {
        Self { faulty: faulty.to_vec(), ..Self::new(m) }
    }
}

impl From<k::KernelError> for FeatureError {
    fn from(e: k::KernelError) -> Self {
        FeatureError::new(format!("{KERNEL_FAILURE}{e}"))
    }
}

pub type FResult<T> = Result<T, FeatureError>;

pub fn err<T>(m: impl Into<String>) -> FResult<T> {
    Err(FeatureError::new(m))
}

/// An error caused by the given parameters (see [`FeatureError::faulty`]).
/// The prefix of a kernel failure's message (`From<KernelError>`).
pub const KERNEL_FAILURE: &str = "Geometry operation failed: ";
/// Onshape's infos for an Add / Remove that leaves its merge scope as it was.
pub const NO_GEOMETRY_CHANGE: &str = "Boolean resulted in no geometry change. The parts either do not intersect or are totally contained.";
pub const TOOLS_MISS_TARGETS: &str = "Selected tools and targets do not intersect.";

pub fn err_at<T>(faulty: &[&'static str], m: impl Into<String>) -> FResult<T> {
    Err(FeatureError { faulty: faulty.to_vec(), ..FeatureError::new(m) })
}

/// Why `v` (SI) is outside the bounds of `spec`, in user units; `None` if inside.
pub fn bounds_violation(v: f64, spec: &crate::params::NumSpec, units: &UnitPrefs) -> Option<String> {
    use crate::params::NumKind;
    let fmt = |x: f64| -> String {
        match spec.kind {
            NumKind::Length => noshape_units::format_quantity(Quantity::new(x, Dim::LENGTH), units),
            NumKind::Angle => noshape_units::format_quantity(Quantity::new(x, Dim::ANGLE), units),
            NumKind::Integer => format!("{}", x.round() as i64),
            NumKind::Real => noshape_units::format_number(x, 6),
        }
    };
    let tol = 1e-9 * spec.min.abs().max(spec.max.abs()).max(1e-9);
    // Integers are rounded by the evaluators, so 0.6 for a min of 1 is fine.
    let v = if spec.kind == NumKind::Integer { v.round() } else { v };
    if v < spec.min - tol {
        return Some(if spec.min > 0.0 && spec.min < 1e-4 && spec.kind != NumKind::Integer {
            "must be greater than 0".into()
        } else if spec.min == 0.0 {
            "must not be negative".into()
        } else {
            format!("must be at least {}", fmt(spec.min))
        });
    }
    if v > spec.max + tol {
        return Some(format!("must be at most {}", fmt(spec.max)));
    }
    None
}

struct ContextScope<'a>(&'a BTreeMap<String, Quantity>);

impl VariableScope for ContextScope<'_> {
    fn get(&self, name: &str) -> Option<Quantity> {
        self.0.get(name).copied()
    }
}

/// A feature of the Part Studio as the regenerator evaluates it (parameters
/// after configuration overrides). Evaluators that re-run other features
/// (feature patterns with "Reapply features", mirror) read them from
/// [`EvalCtx::features`].
#[derive(Clone, Debug)]
pub struct EffectiveFeature {
    pub id: FeatureId,
    pub type_id: String,
    pub name: String,
    pub params: ParamValues,
    pub suppressed: bool,
}

/// The Part Studio's features as a feature sees them (feature patterns re-run others): reading them makes the
/// evaluation one that is never reused (incremental regeneration doesn't know which it read).
#[derive(Clone, Copy, Default)]
pub struct Features<'a> {
    list: &'a [EffectiveFeature],
    reads: Option<&'a BodyReads>,
}

impl<'a> Features<'a> {
    pub fn new(list: &'a [EffectiveFeature], reads: Option<&'a BodyReads>) -> Self {
        Features { list, reads }
    }
    /// The list, for as long as the Part Studio lives (a read, as `Deref`).
    pub fn list(self) -> &'a [EffectiveFeature] {
        if let Some(r) = self.reads {
            r.read_all();
        }
        self.list
    }
}

impl<'a> std::ops::Deref for Features<'a> {
    type Target = [EffectiveFeature];
    fn deref(&self) -> &[EffectiveFeature] {
        if let Some(r) = self.reads {
            r.read_all();
        }
        self.list
    }
}

pub struct EvalCtx<'a> {
    pub feature: FeatureId,
    pub name: &'a str,
    pub params: &'a ParamValues,
    pub units: &'a UnitPrefs,
    pub ctx: Context,
    /// Extra variables (configuration variables, variable studios).
    pub outer_vars: &'a BTreeMap<String, Quantity>,
    /// Warnings to surface without failing.
    pub warnings: Vec<String>,
    /// Information messages (Onshape's `reportFeatureInfo`: a blue bubble,
    /// the feature still succeeds).
    pub infos: Vec<String>,
    /// Read-only parameter values the feature computed (see
    /// [`crate::display::FeatureState::computed`]).
    pub computed: Vec<(String, crate::display::ComputedValue)>,
    /// See [`crate::display::FeatureState::manipulators`].
    pub manipulators: Vec<(String, crate::display::FeatureManipulator)>,
    /// See [`crate::display::FeatureState::touched`].
    pub touched: Vec<BodyId>,
    /// See [`crate::display::FeatureState::error_mesh`]: shapes shown when
    /// the feature fails.
    pub error_shapes: Vec<Shape>,
    /// See [`crate::display::FeatureState::touched_behind`].
    pub touched_behind: Vec<BodyId>,
    /// See [`crate::display::FeatureState::debug`].
    pub debug: crate::display::DebugGeometry,
    /// `TOGGLE_POINTS` manipulator points (pattern Skip instances).
    pub toggle_points: Vec<crate::display::TogglePoint>,
    /// Sketch being edited (for display of constraint states).
    pub editing_sketch: Option<FeatureId>,
    /// Other Part Studios of the document (for Derived).
    pub derived: Option<std::sync::Arc<dyn super::derived::DerivedProvider>>,
    /// Every feature of the Part Studio in list order (empty outside a
    /// Part Studio regeneration).
    pub features: Features<'a>,
    /// Pattern instance transform while a feature is re-applied by a feature
    /// pattern / mirror. Geometry returned by the `resolve_*` helpers
    /// (points, planes, axes, directions, mate connectors and face profiles)
    /// is mapped by it; sketches are transformed in the context itself (see
    /// `features::pattern`), and topology lookups (`resolve_faces`,
    /// `resolve_edges`, `resolve_bodies`) are unaffected.
    pub xform: Option<DMat4>,
    /// Feature Studio scripts of the document (custom features).
    pub custom: Option<Arc<dyn crate::regen::custom_features::CustomFeatureProvider>>,
    /// Type id of the custom feature being evaluated (`custom:<tab>:<name>`).
    pub custom_type: Option<String>,
    /// The run's [`super::RegenCancel`]: long loops (pattern instances)
    /// stop early through [`EvalCtx::cancelled`].
    pub cancel: Option<super::RegenCancel>,
    /// Evaluated for a pattern's instance or seed tool, not as a feature of
    /// its own: no dialog-only work (the parts a tool touches: an exact
    /// distance to every part, per instance; user bug 10-24, a second fin
    /// pattern under a plate already carrying 400 fins).
    pub scratch: bool,
}

/// Map a plane frame by an affine (rigid or mirror) transform. Mirrors give a
/// left-handed frame (`x × y = -normal`), which keeps `to_world` and the
/// extrude direction consistent; [`kernel_plane`] handles it.
pub fn transform_frame(f: &PlaneFrame, m: &DMat4) -> PlaneFrame {
    PlaneFrame {
        origin: m.transform_point3(f.origin),
        x: m.transform_vector3(f.x).normalize_or_zero(),
        y: m.transform_vector3(f.y).normalize_or_zero(),
        normal: m.transform_vector3(f.normal).normalize_or_zero(),
    }
}

/// References whose geometry comes from a sketch (already transformed in the
/// context during a pattern re-application).
fn is_sketch_ref(r: &Reference) -> bool {
    match r {
        Reference::SketchEntity { .. } | Reference::SketchRegion { .. } | Reference::Sketch(_) => true,
        Reference::Implicit { on, .. } => is_sketch_ref(on),
        _ => false,
    }
}

/// Copy of `params` without the sketch regions that don't resolve in `ctx`
/// (their sketch is gone or comes later, or it no longer has the region)
/// and the parts of this studio's features (`own`) that don't exist there,
/// and the faces, edges and vertices of this studio's features that no body
/// has there, and how many were left out; `None` when everything resolves.
/// The solids among `bodies` that touch any of `tools` (their boxes overlap and a tool vertex is in or on the body, or
/// they are no farther apart than 1e-9). `exact: false` (the dialog's preselection probe): a part of many faces is
/// only tested by the tool's points, not the whole distance (cf-spur-gear's 220-face gear: 1.7 s a tool).
pub fn touching(bodies: &[Arc<Body>], tools: &[Shape], exact: bool) -> Vec<usize> {
    let tb: Vec<Option<k::BoundingBox>> = tools.iter().map(|t| t.bounding_box(false).ok()).collect();
    bodies
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == BodyKind::Solid)
        .filter(|(_, b)| {
            // The exact distance only for tools whose boxes reach the body,
            // and only when none of the tool's vertices is in or on it (a
            // boss standing on the part: the classification is cheaper
            // than the distance, REGEN-SPEED).
            let Ok(bb) = b.shape.bounding_box(false) else { return false };
            // (A point inside each of the tool's faces too: a boss on a gear's face touches it there, and the exact
            // distance to the gear's hundreds of B-spline faces took 1.7 s, cf-spur-gear.)
            let on = |p: DVec3| matches!(b.shape.classify_point(p, 1e-9), Ok(k::PointClass::Inside | k::PointClass::OnBoundary));
            let touches = |t: &Shape| {
                t.vertices().iter().take(8).filter_map(|v| v.vertex_point().ok()).any(on)
                    || t.faces().iter().take(16).filter_map(super::features::helpers_a::point_on_face).any(on)
                    || ((exact || b.shape.count(ShapeType::Face) <= 64) && k::distance(&b.shape, t).map(|d| d.distance < 1e-9).unwrap_or(false))
            };
            tools.iter().zip(&tb).any(|(t, tbox)| tbox.as_ref().is_none_or(|x| overlaps(&bb, x)) && touches(t))
        })
        .map(|(i, _)| i)
        .collect()
}

pub fn drop_missing_regions(params: &ParamValues, ctx: &Context, own: &dyn Fn(FeatureId) -> bool) -> (Option<ParamValues>, usize) {
    /// Whether every face `n` is named after comes from this studio's features
    /// (sketch and wire-body curves are left alone).
    fn own_topo(n: &TopoName, own: &dyn Fn(FeatureId) -> bool) -> bool {
        match n {
            TopoName::Face { feature, .. } => own(*feature),
            TopoName::Split { parent, .. } => own_topo(parent, own),
            TopoName::Edge { faces, .. } => faces.iter().all(|f| own_topo(f, own)),
            TopoName::Vertex { faces, .. } => !faces.is_empty() && faces.iter().all(|f| own_topo(f, own)),
            TopoName::Boundary { face, .. } => own_topo(face, own),
            TopoName::Wire { .. } => false,
        }
    }
    let found = |kind: LookupKind, n: &TopoName, any: &dyn Fn(&Body) -> bool| {
        ctx.bodies.note_lookup(kind, n);
        let bodies = ctx.bodies.untracked();
        bodies.iter().any(|b| b.has_exact(n)) || bodies.iter().any(|b| any(b))
    };
    let resolves = |r: &Reference| -> bool {
        match r {
            // A face, edge or vertex of a feature that comes later in the list, or that a feature before this one
            // consumed (live "(7) Dragging feature rows": a Chamfer dragged above the Extrude its edge comes from
            // fails with "Select edges or faces to chamfer." and "1 missing selection"; L20 Fillet R7 likewise; a
            // sketch on a face that is gone: "Select a sketch plane." and "1 missing selection").
            // Exact names first: the fuzzy lookups scan a body's entities.
            // (Noted as name lookups: an evaluation whose selection went missing is reused only while it still is.)
            Reference::Face(n) if own_topo(n, own) => found(LookupKind::Face, n, &|b| !b.find_faces(n).is_empty()),
            Reference::Edge(n) if own_topo(n, own) => found(LookupKind::Edge, n, &|b| !b.find_edges(n).is_empty()),
            Reference::Vertex(n) if own_topo(n, own) => found(LookupKind::Vertex, n, &|b| !b.find_vertices(n).is_empty()),
            _ => region_or_part_resolves(r, ctx, own),
        }
    };
    fn region_or_part_resolves(r: &Reference, ctx: &Context, own: &dyn Fn(FeatureId) -> bool) -> bool {
        match r {
            Reference::SketchRegion { sketch, key } => ctx
                .sketch(*sketch)
                .is_some_and(|s| decode_region_key(key).is_some_and(|id| s.split.iter().any(|r| r.id == id) || noshape_sketch::regions::find_region_in(&s.regions, &id).is_some())),
            // A part that doesn't exist at this point of the list (live "(7) Dragging feature rows": a feature
            // moved above the one that made its part fails on its own empty selection, "1 missing selection").
            // Only this studio's own parts: a derived feature lists another studio's.
            Reference::Body(id) => !own(id.feature) || ctx.bodies.position_id(id).is_some(),
            _ => true,
        }
    }
    fn prune(v: &ParamValues, resolves: &dyn Fn(&Reference) -> bool, missing: &mut usize) -> Option<ParamValues> {
        let mut out: Option<ParamValues> = None;
        for (k, p) in v.0.iter() {
            let new = match p {
                ParamValue::Query(q) if q.iter().any(|r| !resolves(r)) => {
                    *missing += q.iter().filter(|r| !resolves(r)).count();
                    Some(ParamValue::Query(q.iter().filter(|r| resolves(r)).cloned().collect()))
                }
                ParamValue::Array(items) => {
                    let pruned: Vec<Option<ParamValues>> = items.iter().map(|it| prune(it, resolves, missing)).collect();
                    pruned.iter().any(|x| x.is_some()).then(|| ParamValue::Array(items.iter().zip(pruned).map(|(it, x)| x.unwrap_or_else(|| it.clone())).collect()))
                }
                _ => None,
            };
            if let Some(new) = new {
                out.get_or_insert_with(|| v.clone()).0.insert(k.clone(), new);
            }
        }
        out
    }
    let mut missing = 0;
    let out = prune(params, &resolves, &mut missing);
    (out, missing)
}

/// Copy of `params` with every [`Reference::QueryVariable`] (also inside
/// arrays) replaced by the variable's references from `ctx`; `None` when
/// the parameters use no query variable. Unknown variables expand to
/// nothing (the selection reads as missing).
pub fn expand_query_variables(params: &ParamValues, ctx: &Context) -> Option<ParamValues> {
    fn uses(v: &ParamValues) -> bool {
        v.0.values().any(|p| match p {
            ParamValue::Query(q) => q.iter().any(|r| matches!(r, Reference::QueryVariable(_))),
            ParamValue::Array(items) => items.iter().any(uses),
            _ => false,
        })
    }
    fn expand(v: &ParamValues, ctx: &Context) -> ParamValues {
        let mut out = v.clone();
        for p in out.0.values_mut() {
            match p {
                ParamValue::Query(q) if q.iter().any(|r| matches!(r, Reference::QueryVariable(_))) => {
                    let mut e: Vec<Reference> = Vec::with_capacity(q.len());
                    for r in q.iter() {
                        match r {
                            Reference::QueryVariable(name) => {
                                for x in ctx.query_variables.get(name).map(|v| v.as_slice()).unwrap_or(&[]) {
                                    if !e.contains(x) {
                                        e.push(x.clone());
                                    }
                                }
                            }
                            other => {
                                if !e.contains(other) {
                                    e.push(other.clone());
                                }
                            }
                        }
                    }
                    *q = e;
                }
                ParamValue::Array(items) => {
                    for it in items.iter_mut() {
                        if uses(it) {
                            *it = expand(it, ctx);
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }
    uses(params).then(|| expand(params, ctx))
}

impl<'a> EvalCtx<'a> {
    /// A context for evaluating one feature (no feature list, no pattern
    /// transform, not editing a sketch).
    pub fn new(feature: FeatureId, name: &'a str, params: &'a ParamValues, units: &'a UnitPrefs, ctx: Context, outer_vars: &'a BTreeMap<String, Quantity>) -> Self {
        EvalCtx { feature, name, params, units, ctx, outer_vars, warnings: Vec::new(), infos: Vec::new(), computed: Vec::new(), manipulators: Vec::new(), touched: Vec::new(), error_shapes: Vec::new(), touched_behind: Vec::new(), debug: Default::default(), toggle_points: Vec::new(), editing_sketch: None, derived: None, features: Features::default(), xform: None, custom: None, custom_type: None, cancel: None, scratch: false }
    }

    /// The run was aborted (a newer edit made it stale): stop and return
    /// any error; the result is discarded.
    pub fn cancelled(&self) -> bool {
        self.cancel.as_ref().is_some_and(super::RegenCancel::aborted)
    }

    // ------------------------------------------------------------ pattern transform

    fn xf_point(&self, p: DVec3) -> DVec3 {
        match &self.xform {
            Some(m) => m.transform_point3(p),
            None => p,
        }
    }
    fn xf_vector(&self, v: DVec3) -> DVec3 {
        match &self.xform {
            Some(m) => m.transform_vector3(v).normalize_or_zero(),
            None => v,
        }
    }

    /// References of a query variable defined upstream (empty if unknown).
    pub fn query_variable(&self, name: &str) -> &[Reference] {
        self.ctx.query_variables.get(name).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// First reference behind a query variable (for single-entity resolvers).
    fn qv_first(&self, r: &Reference) -> Option<Reference> {
        match r {
            Reference::QueryVariable(n) => self.query_variable(n).first().cloned(),
            _ => None,
        }
    }

    // ------------------------------------------------------------ params

    pub fn bool(&self, id: &str) -> bool {
        self.params.bool(id)
    }
    pub fn enum_(&self, id: &str) -> &str {
        self.params.enum_(id)
    }
    pub fn is(&self, id: &str, v: &str) -> bool {
        self.params.is(id, v)
    }
    pub fn query(&self, id: &str) -> &'a [Reference] {
        self.params.query(id)
    }

    pub fn eval_num_in(&self, vals: &ParamValues, id: &str, kind: NumKind) -> FResult<f64> {
        let e = match vals.get(id) {
            Some(ParamValue::Expr(e)) => e,
            _ => return err(format!("Missing value for {id}")),
        };
        let scope_inner = ContextScope(&self.ctx.variables);
        let scope_outer = ContextScope(self.outer_vars);
        let scope = LayeredScope { inner: &scope_inner, outer: &scope_outer };
        let expr = e.parse().map_err(|x| FeatureError::new(format!("{}: {x}", e.text)))?;
        let q = noshape_units::evaluate_expr_as(&expr, kind.dim(), self.units, &scope).map_err(|x| match x {
            // Onshape's message (live qa5 pass 8: a Variable moved below the
            // feature using it).
            noshape_units::EvalError::UndefinedVariable(_) => FeatureError::new("Variable not found, is suppressed, or has not been set."),
            x => FeatureError::new(format!("{}: {x}", e.text)),
        })?;
        Ok(q.value)
    }

    pub fn length(&self, id: &str) -> FResult<f64> {
        self.eval_num_in(self.params, id, NumKind::Length)
    }
    pub fn angle(&self, id: &str) -> FResult<f64> {
        self.eval_num_in(self.params, id, NumKind::Angle)
    }
    pub fn real(&self, id: &str) -> FResult<f64> {
        self.eval_num_in(self.params, id, NumKind::Real)
    }
    pub fn integer(&self, id: &str) -> FResult<i64> {
        Ok(self.eval_num_in(self.params, id, NumKind::Integer)?.round() as i64)
    }

    /// Check every visible numeric parameter against its schema bounds
    /// (Onshape rejects out-of-range values such as a pattern count of 0 or
    /// 10⁶, or a zero fillet radius, before the feature runs). Values that
    /// fail to evaluate are left to the evaluator's own error.
    pub fn check_bounds(&self, specs: &[crate::params::ParamSpec]) -> FResult<()> {
        self.check_bounds_in(specs, self.params)
    }

    fn check_bounds_in(&self, specs: &[crate::params::ParamSpec], vals: &ParamValues) -> FResult<()> {
        use crate::params::ParamKind;
        for s in specs {
            if !s.is_visible(vals) {
                continue;
            }
            match &s.kind {
                ParamKind::Num(n) => {
                    let Ok(v) = self.eval_num_in(vals, s.id, n.kind) else { continue };
                    // Live (L32, L45, L55): Onshape's text for a value out of
                    // its range, with the field marked.
                    if bounds_violation(v, n, self.units).is_some() {
                        return err_at(&[s.id], "A parameter value is not valid.");
                    }
                }
                ParamKind::Array { item, .. } => {
                    for it in vals.array(s.id) {
                        self.check_bounds_in(item, it)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Evaluate an arbitrary expression text, its dimension as written.
    pub fn eval_any(&self, text: &str) -> FResult<Quantity> {
        let scope_inner = ContextScope(&self.ctx.variables);
        let scope_outer = ContextScope(self.outer_vars);
        let scope = LayeredScope { inner: &scope_inner, outer: &scope_outer };
        noshape_units::evaluate(text, &scope).map_err(|x| FeatureError::new(format!("{text}: {x}")))
    }

    /// Evaluate an arbitrary expression text with a dimension.
    pub fn eval_text(&self, text: &str, dim: Dim) -> FResult<Quantity> {
        let scope_inner = ContextScope(&self.ctx.variables);
        let scope_outer = ContextScope(self.outer_vars);
        let scope = LayeredScope { inner: &scope_inner, outer: &scope_outer };
        noshape_units::evaluate_as(text, dim, self.units, &scope).map_err(|x| FeatureError::new(format!("{text}: {x}")))
    }

    // ------------------------------------------------------------ resolution

    /// Sub-shape lookups: (body index, sub-shape index).
    pub fn resolve_faces(&self, refs: &[Reference]) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for r in refs {
            match r {
                Reference::Face(n) => out.extend(self.ctx.bodies.find(LookupKind::Face, n)),
                Reference::QueryVariable(name) => out.extend(self.resolve_faces(self.query_variable(name))),
                _ => {}
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    pub fn resolve_edges(&self, refs: &[Reference]) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for r in refs {
            match r {
                Reference::Edge(n) => out.extend(self.ctx.bodies.find(LookupKind::Edge, n)),
                // A face selection means all of its edges (Onshape fillet/chamfer behavior).
                Reference::Face(n) => {
                    for (bi, fi) in self.ctx.bodies.find(LookupKind::Face, n) {
                        let b = &self.ctx.bodies[bi];
                        {
                            if let Ok(map) = b.shape.ancestor_map(ShapeType::Edge, ShapeType::Face) {
                                for (ei, faces) in map.to_vecs().iter().enumerate() {
                                    if faces.contains(&(fi as u32)) {
                                        out.push((bi, ei));
                                    }
                                }
                            }
                        }
                    }
                }
                Reference::QueryVariable(name) => out.extend(self.resolve_edges(self.query_variable(name))),
                _ => {}
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Position of a point-like reference (pattern transform applied).
    pub fn resolve_vertex_point(&self, r: &Reference) -> Option<DVec3> {
        if let Some(q) = self.qv_first(r) {
            return self.resolve_vertex_point(&q);
        }
        let p = self.raw_vertex_point(r)?;
        Some(if is_sketch_ref(r) { p } else { self.xf_point(p) })
    }

    /// Like [`Self::resolve_vertex_point`] without the pattern transform.
    pub fn raw_vertex_point(&self, r: &Reference) -> Option<DVec3> {
        match r {
            Reference::Origin => Some(DVec3::ZERO),
            Reference::Vertex(n) => {
                let (bi, vi) = *self.ctx.bodies.find(LookupKind::Vertex, n).first()?;
                self.ctx.bodies[bi].shape.sub_shape(ShapeType::Vertex, vi).ok()?.vertex_point().ok()
            }
            Reference::SketchEntity { sketch, entity } => {
                let s = self.ctx.sketch(*sketch)?;
                let p = s.sketch.point(noshape_sketch::EntityId(*entity))?;
                Some(s.frame.to_world(p))
            }
            Reference::MateConnector { .. } | Reference::Implicit { .. } => self.raw_connector(r).map(|c| c.origin),
            Reference::QueryVariable(_) => self.raw_vertex_point(&self.qv_first(r)?),
            _ => None,
        }
    }

    pub fn resolve_bodies(&self, refs: &[Reference]) -> Vec<usize> {
        let mut out = Vec::new();
        for r in refs {
            match r {
                Reference::Body(id) => {
                    if let Some(i) = self.ctx.bodies.position_id(id) {
                        out.push(i);
                    }
                }
                // Faces/edges select their owning body.
                Reference::Face(_) | Reference::Edge(_) => {
                    for (bi, _) in self.resolve_faces(std::slice::from_ref(r)).into_iter().chain(self.resolve_edges(std::slice::from_ref(r))) {
                        out.push(bi);
                    }
                }
                Reference::QueryVariable(name) => out.extend(self.resolve_bodies(self.query_variable(name))),
                _ => {}
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    pub fn face_shape(&self, bi: usize, fi: usize) -> FResult<Shape> {
        Ok(self.ctx.bodies[bi].shape.sub_shape(ShapeType::Face, fi)?)
    }
    pub fn edge_shape(&self, bi: usize, ei: usize) -> FResult<Shape> {
        Ok(self.ctx.bodies[bi].shape.sub_shape(ShapeType::Edge, ei)?)
    }

    /// Resolve a plane-like reference to a frame (pattern transform applied).
    pub fn resolve_plane(&self, r: &Reference) -> Option<PlaneFrame> {
        if let Some(q) = self.qv_first(r) {
            return self.resolve_plane(&q);
        }
        let f = self.raw_plane(r)?;
        Some(match (&self.xform, is_sketch_ref(r)) {
            (Some(m), false) => transform_frame(&f, m),
            _ => f,
        })
    }

    /// A sketch's frame on a plane-like reference: std newSketch's on a construction plane (the world origin projected
    /// onto it, axes as a face's: live cf-fill-pattern's sketches on Line-Angle planes), the plane's own elsewhere;
    /// pattern transform applied after.
    pub fn resolve_sketch_plane(&self, r: &Reference) -> Option<PlaneFrame> {
        let base = self.qv_first(r).unwrap_or_else(|| r.clone());
        if self.ctx.legacy_plane_sketch_frames || !matches!(base, Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. }) {
            return self.resolve_plane(r);
        }
        let f = self.raw_plane(&base)?;
        let n = f.normal;
        let x = PlaneFrame::face_x(n);
        let canonical = PlaneFrame { origin: n * f.origin.dot(n), x, y: n.cross(x), normal: n };
        Some(match &self.xform {
            Some(m) => transform_frame(&canonical, m),
            None => canonical,
        })
    }

    /// Like [`Self::resolve_plane`] without the pattern transform.
    pub fn raw_plane(&self, r: &Reference) -> Option<PlaneFrame> {
        match r {
            Reference::Plane(DatumPlane::Top) => Some(PlaneFrame::TOP),
            Reference::Plane(DatumPlane::Front) => Some(PlaneFrame::FRONT),
            Reference::Plane(DatumPlane::Right) => Some(PlaneFrame::RIGHT),
            Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. } => self.ctx.planes.iter().find(|p| &p.reference == r).map(|p| p.frame),
            Reference::Face(_) => {
                let (bi, fi) = *self.resolve_faces(std::slice::from_ref(r)).first()?;
                let face = self.face_shape(bi, fi).ok()?;
                let pl = face.face_plane().ok()??;
                // Sketch origin on a face: world origin projected onto the face plane.
                let n = pl.unit_normal();
                let x = if self.ctx.legacy_face_frames { pl.unit_x() } else { PlaneFrame::face_x(n) };
                let origin = n * pl.origin.dot(n);
                Some(PlaneFrame { origin, x, y: n.cross(x), normal: n })
            }
            Reference::SketchRegion { sketch, .. } | Reference::Sketch(sketch) => self.ctx.sketch(*sketch).map(|s| s.frame),
            Reference::MateConnector { .. } | Reference::Implicit { .. } => {
                let c = self.raw_connector(r)?;
                Some(PlaneFrame { origin: c.origin, x: c.x, y: c.z.cross(c.x), normal: c.z })
            }
            Reference::QueryVariable(_) => self.raw_plane(&self.qv_first(r)?),
            _ => None,
        }
    }

    /// Mate connector (explicit or implicit), pattern transform applied.
    pub fn resolve_connector(&self, r: &Reference) -> Option<super::context::Connector> {
        if let Some(q) = self.qv_first(r) {
            return self.resolve_connector(&q);
        }
        let mut c = self.raw_connector(r)?;
        if !is_sketch_ref(r)
            && let Some(m) = &self.xform
        {
            c.origin = m.transform_point3(c.origin);
            c.x = m.transform_vector3(c.x).normalize_or_zero();
            c.z = m.transform_vector3(c.z).normalize_or_zero();
        }
        Some(c)
    }

    /// Like [`Self::resolve_connector`] without the pattern transform.
    pub fn raw_connector(&self, r: &Reference) -> Option<super::context::Connector> {
        match r {
            Reference::MateConnector { .. } => self.ctx.connectors.iter().find(|c| &c.reference == r).cloned(),
            Reference::Implicit { on, inference } => super::implicit::implicit_connector(self, on, *inference),
            Reference::QueryVariable(_) => self.raw_connector(&self.qv_first(r)?),
            _ => None,
        }
    }

    /// Resolve an axis: (origin, unit direction), pattern transform applied.
    pub fn resolve_axis(&self, r: &Reference) -> Option<(DVec3, DVec3)> {
        if let Some(q) = self.qv_first(r) {
            return self.resolve_axis(&q);
        }
        let (o, d) = self.raw_axis(r)?;
        Some(if is_sketch_ref(r) { (o, d) } else { (self.xf_point(o), self.xf_vector(d)) })
    }

    /// Like [`Self::resolve_axis`] without the pattern transform.
    pub fn raw_axis(&self, r: &Reference) -> Option<(DVec3, DVec3)> {
        match r {
            Reference::Edge(_) => {
                let (bi, ei) = *self.resolve_edges(std::slice::from_ref(r)).first()?;
                let e = self.edge_shape(bi, ei).ok()?;
                match e.edge_curve().ok()? {
                    // The edge's own direction (its curve's), as Parasolid
                    // gives it, not the sense of the face using it first
                    // (live oracle_p1: an extruded block's top front edge
                    // runs +x like its sketch line; patterns follow it).
                    k::Curve::Line { origin, direction } => {
                        let d = direction.normalize();
                        Some((origin, if e.orientation() == k::Orientation::Reversed { -d } else { d }))
                    }
                    k::Curve::Circle { center, axis, .. } => Some((center, axis.normalize())),
                    _ => {
                        let (a, b) = e.endpoints().ok()?;
                        ((b - a).length() > 1e-12).then(|| (a, (b - a).normalize()))
                    }
                }
            }
            Reference::Face(_) => {
                let (bi, fi) = *self.resolve_faces(std::slice::from_ref(r)).first()?;
                match self.face_shape(bi, fi).ok()?.face_surface().ok()? {
                    k::Surface::Cylinder { origin, axis, .. } => Some((origin, axis.normalize())),
                    k::Surface::Cone { origin, axis, .. } => Some((origin, axis.normalize())),
                    k::Surface::Revolution { axis_origin, axis_dir } => Some((axis_origin, axis_dir.normalize())),
                    // Revolved circles report as tori or spheres (the kernel recognises them).
                    k::Surface::Torus { center, axis, .. } | k::Surface::Sphere { center, axis, .. } => Some((center, axis.normalize())),
                    k::Surface::Plane(p) => Some((p.origin, p.unit_normal())),
                    _ => None,
                }
            }
            Reference::SketchEntity { sketch, entity } => {
                let s = self.ctx.sketch(*sketch)?;
                let id = noshape_sketch::EntityId(*entity);
                match s.sketch.curve(id)? {
                    noshape_sketch::Curve2::Line { a, b } => {
                        let wa = s.frame.to_world(a);
                        let wb = s.frame.to_world(b);
                        Some((wa, (wb - wa).normalize()))
                    }
                    noshape_sketch::Curve2::Circle { center, .. } | noshape_sketch::Curve2::Arc { center, .. } => Some((s.frame.to_world(center), s.frame.normal)),
                    _ => None,
                }
            }
            Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. } => {
                let f = self.raw_plane(r)?;
                Some((f.origin, f.normal))
            }
            Reference::MateConnector { .. } | Reference::Implicit { .. } => {
                let c = self.raw_connector(r)?;
                Some((c.origin, c.z))
            }
            Reference::QueryVariable(_) => self.raw_axis(&self.qv_first(r)?),
            _ => None,
        }
    }

    pub fn resolve_direction(&self, r: &Reference) -> Option<DVec3> {
        if let Some(q) = self.qv_first(r) {
            return self.resolve_direction(&q);
        }
        if let Some(f) = self.resolve_plane(r)
            && matches!(r, Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. } | Reference::Face(_))
        {
            return Some(f.normal);
        }
        self.resolve_axis(r).map(|(_, d)| d)
    }

    /// Sketch region by encoded key.
    pub fn resolve_region(&self, sketch: FeatureId, key: &[u32]) -> Option<(Arc<SketchResult>, noshape_sketch::regions::Region)> {
        let s = self.ctx.sketch(sketch)?.clone();
        let id = decode_region_key(key)?;
        // The sketch's own regions: rebuilding them per reference made an
        // extrude of 400 circles take minutes.
        // A region the imprinted face edges split, picked before imprinting: as it was.
        let r = match s.split.iter().find(|r| r.id == id) {
            Some(r) if !s.regions.iter().any(|x| x.id == id) => r.clone(),
            _ => noshape_sketch::regions::find_region_in(&s.regions, &id)?,
        };
        Some((s, r))
    }

    // ------------------------------------------------------------ profiles

    /// The sketch regions among `refs` (`None` elsewhere), regions of one sketch that share a line or a loop united at the
    /// first of them (`None` at the others): live extrudes them as one face.
    fn picked_regions(&self, refs: &[Reference]) -> FResult<Vec<Option<RegionOf>>> {
        let mut out = Vec::with_capacity(refs.len());
        let mut sketches: Vec<FeatureId> = Vec::new();
        for r in refs {
            let Reference::SketchRegion { sketch, key } = r else {
                out.push(None);
                continue;
            };
            let Some(found) = self.resolve_region(*sketch, key) else {
                return err("A selected sketch region no longer exists");
            };
            out.push(Some(found));
            if !sketches.contains(sketch) {
                sketches.push(*sketch);
            }
        }
        if noshape_kernel::legacy_topology() {
            return Ok(out);
        }
        for sk in sketches {
            let idx: Vec<usize> = (0..refs.len()).filter(|&i| matches!(&refs[i], Reference::SketchRegion { sketch, .. } if *sketch == sk)).collect();
            let picked: Vec<_> = idx.iter().filter_map(|&i| out[i].clone()).collect();
            if picked.len() < 2 {
                continue;
            }
            let regions: Vec<_> = picked.iter().map(|(_, r)| r.clone()).collect();
            let united = noshape_sketch::regions::union_touching(&regions);
            for ((&i, (s, _)), u) in idx.iter().zip(picked).zip(united) {
                out[i] = u.map(|r| (s, r));
            }
        }
        Ok(out)
    }

    /// Build profile faces from sketch regions and planar faces. Face
    /// profiles follow the pattern transform (sketch regions are already
    /// transformed through their sketch's frame).
    pub fn profiles(&self, refs: &[Reference]) -> FResult<Vec<Profile>> {
        // Many sketch regions (a plate's 400 holes): their faces made in
        // parallel. Each region's face gets its own edges (region_profile
        // makes them from the segments), so they share nothing.
        let picked = self.picked_regions(refs)?;
        if refs.len() >= 16 && refs.iter().all(|r| matches!(r, Reference::SketchRegion { .. })) {
            let mut regions = Vec::with_capacity(refs.len());
            for (r, p) in refs.iter().zip(picked) {
                let (Reference::SketchRegion { sketch, key }, Some((s, region))) = (r, p) else { continue };
                regions.push((s, region, Source::Region { sketch: *sketch, key: key.clone() }));
            }
            use rayon::prelude::*;
            let legacy = noshape_kernel::legacy_topology();
            let made: Vec<FResult<Profile>> = regions.into_par_iter().map(|(s, region, key)| noshape_kernel::with_legacy_topology(legacy, || region_profile(&s, &region, key))).collect();
            return made.into_iter().collect();
        }
        let mut out = Vec::new();
        for (r, p) in refs.iter().zip(picked) {
            match r {
                Reference::SketchRegion { sketch, key } => {
                    let Some((s, region)) = p else { continue };
                    out.push(region_profile(&s, &region, Source::Region { sketch: *sketch, key: key.clone() })?);
                }
                Reference::Face(n) => {
                    for (bi, fi) in self.resolve_faces(std::slice::from_ref(r)) {
                        let b = &self.ctx.bodies[bi];
                        let face = b.shape.sub_shape(ShapeType::Face, fi)?;
                        let Some(pl) = face.face_plane()? else { return err("Selected face is not planar") };
                        // Name profile edges by the body's edge names.
                        let edges = face.edges();
                        let names: Vec<Name> = edges.iter().map(|e| b.shape.index_of(e, ShapeType::Edge).and_then(|i| b.edges.get(i).cloned()).unwrap_or_else(|| Arc::new(TopoName::face(self.feature, Role::New(0))))).collect();
                        let mut frame = PlaneFrame { origin: pl.origin, x: pl.unit_x(), y: pl.unit_normal().cross(pl.unit_x()), normal: pl.unit_normal() };
                        let mut shape = face.clone();
                        if let Some(m) = &self.xform {
                            // Same sub-shape order after a transform: edge names stay aligned.
                            shape = k::transform(&shape, &k::Transform::Matrix(*m), true)?.shape;
                            frame = transform_frame(&frame, m);
                        }
                        let sources = names.iter().map(|n| Source::Topo(n.clone())).collect();
                        out.push(Profile { shape, frame, edge_sources: sources, key: Source::Topo(Arc::new((*n).clone())), from_face: true });
                    }
                }
                Reference::QueryVariable(name) => out.extend(self.profiles(self.query_variable(name))?),
                // A whole sketch (Onshape's whole-sketch region query, a feature-list pick: "Faces of Sketch 1"): its
                // regions as they are now, not the face's imprinted pieces (qSketchRegion's).
                Reference::Sketch(sketch) => {
                    let Some(s) = self.ctx.sketch(*sketch).cloned() else {
                        return err("A selected sketch no longer exists");
                    };
                    let regions: Vec<Reference> = s.regions.iter().enumerate().filter(|(i, _)| !s.imprinted.get(*i).copied().unwrap_or(false)).map(|(_, g)| Reference::SketchRegion { sketch: *sketch, key: encode_region_key(&g.id) }).collect();
                    out.extend(self.profiles(&regions)?);
                }
                _ => {}
            }
        }
        Ok(out)
    }

    // ------------------------------------------------------------ naming

    /// Name the faces of an operation result.
    pub fn name_result(&self, args: &[&ShapeNames], hist: &k::History, out: &Shape, roles: &BTreeMap<u32, Role>, generated: Option<&dyn Fn(&Name) -> Role>) -> Vec<Name> {
        let records: Vec<HistRecord> = hist
            .records
            .iter()
            .filter_map(|r| {
                let kind = |t: ShapeType| match t {
                    ShapeType::Face => Some(Kind::Face),
                    ShapeType::Edge => Some(Kind::Edge),
                    ShapeType::Vertex => Some(Kind::Vertex),
                    _ => None,
                };
                let relation = match r.relation {
                    k::Relation::Modified => Relation::Modified,
                    k::Relation::Kept => Relation::Same,
                    k::Relation::Generated => Relation::Generated,
                    k::Relation::First | k::Relation::Last | k::Relation::Deleted => return None,
                };
                Some(HistRecord { arg: r.arg, in_kind: kind(r.input_kind)?, in_index: r.input_index, relation, out_kind: kind(r.output_kind)?, out_index: r.output_index })
            })
            .collect();
        let count = out.count(ShapeType::Face);
        name_faces(&FaceNamingInput { feature: self.feature, args, records: &records, out_count: count, out_face_key: &|i| face_key(out, i), roles, generated_role: generated })
    }

    /// Bounding diagonal of everything (for "through all").
    pub fn model_extent(&self) -> f64 {
        self.ctx.bounds().map(|(a, b)| (b - a).length() + a.length().max(b.length())).unwrap_or(0.0).max(1.0) * 2.0
    }

    // ------------------------------------------------------------ booleans

    /// Apply a feature's result-body operation (New / Add / Remove /
    /// Intersect) with tool solids already named.
    pub fn apply_operation(&mut self, tools: Vec<NamedShape>, op: &str, scope: Option<Vec<usize>>, kind: BodyKind) -> FResult<()> {
        if tools.is_empty() {
            return err("Nothing was created");
        }
        // The parts the tool touches, for the dialog's boolean heuristics
        // (booleanHeuristics.fs `autoSelectionForBooleanStep`).
        if kind == BodyKind::Solid && tools.len() <= 8 && !self.scratch {
            let t = self.scope_of(&tools, false);
            self.touched = t.iter().map(|&i| self.ctx.bodies[i].id.clone()).collect();
        }
        match op {
            "NEW" => {
                // Tools that touch are merged into one part (like Onshape).
                let merged = if tools.len() > 1 { self.fuse_named(tools)? } else { tools };
                for (i, t) in merged.into_iter().enumerate() {
                    self.add_new_bodies(t, kind, i as u32)?;
                }
                Ok(())
            }
            "ADD" | "REMOVE" | "INTERSECT" => {
                // Onshape (booleanStepScopePredicate): the merge scope is
                // required unless "Merge with all" is on (live qa5 BOOL4:
                // "No merge scope selected.").
                let targets = match scope {
                    Some(s) if !s.is_empty() => s,
                    _ if kind == BodyKind::Solid => return err_at(&["booleanScope"], "No merge scope selected."),
                    _ => self.auto_scope(&tools),
                };
                if targets.is_empty() {
                    if op == "ADD" {
                        // Surfaces with nothing to merge with: new surfaces.
                        for (i, t) in tools.into_iter().enumerate() {
                            self.add_new_bodies(t, kind, i as u32)?;
                        }
                        return Ok(());
                    }
                    return err("Select parts in the merge scope");
                }
                let bop = match op {
                    "ADD" => k::BooleanOp::Fuse,
                    "REMOVE" => k::BooleanOp::Cut,
                    _ => k::BooleanOp::Common,
                };
                // Many tools (pattern instances): the many-tools options.
                let opts = if tools.len() >= 8 { Self::many_tools_options() } else { k::BooleanOptions::default() };
                // A part the boolean leaves as it was: same topology counts
                // and box, then the same volume (measured only then: comparing
                // volumes for every boolean measured each body twice per
                // feature, REGEN-SPEED).
                let before: Vec<Arc<Body>> = if kind == BodyKind::Solid && op != "INTERSECT" { targets.iter().map(|&i| self.ctx.bodies[i].clone()).collect() } else { Vec::new() };
                self.boolean_into_with(&targets, tools, bop, &opts)?;
                // Every part in the merge scope as it was: Onshape's infos
                // (live oracle_p1 E9/E11/E12: a tool inside the part or apart
                // from it, the latter kept as its own part; E13: a cut that
                // misses).
                // Faces, box and volume: a tool that only touches a part can
                // split its edges where they meet (a cylinder tangent to the
                // tool's sides) without changing it; live reports that as a
                // miss too.
                let same = |a: &Shape, b: &Shape| -> bool {
                    if a.count(ShapeType::Face) != b.count(ShapeType::Face) {
                        return false;
                    }
                    let (Ok(ba), Ok(bb)) = (a.bounding_box(false), b.bounding_box(false)) else { return false };
                    let tol = 1e-6 * ba.diagonal().max(1e-12);
                    if (ba.min - bb.min).abs().max_element() > tol || (ba.max - bb.max).abs().max_element() > tol {
                        return false;
                    }
                    match (a.volume(), b.volume()) {
                        (Ok(va), Ok(vb)) => (va - vb).abs() <= 1e-9 * va.abs().max(1e-12),
                        _ => false,
                    }
                };
                let mut unchanged = !before.is_empty();
                for old in &before {
                    match self.ctx.bodies.iter().position(|b| b.id == old.id) {
                        // The part as it was, without the touching tool's
                        // imprint.
                        Some(i) if same(&old.shape, &self.ctx.bodies[i].shape) => self.ctx.bodies[i] = old.clone(),
                        _ => unchanged = false,
                    }
                }
                if unchanged {
                    self.infos.push(if op == "ADD" { NO_GEOMETRY_CHANGE } else { TOOLS_MISS_TARGETS }.into());
                }
                Ok(())
            }
            other => err(format!("Unknown operation {other}")),
        }
    }

    /// Bodies whose bounding boxes overlap any tool (auto merge scope). Noted as a probe: an evaluation is reused
    /// only while the same bodies touch its tools.
    pub fn auto_scope(&self, tools: &[NamedShape]) -> Vec<usize> {
        self.scope_of(tools, true)
    }

    /// [`Self::auto_scope`], `exact: false` for the dialog's probe (see [`touching`]).
    fn scope_of(&self, tools: &[NamedShape], exact: bool) -> Vec<usize> {
        let shapes: Vec<Shape> = tools.iter().map(|t| t.shape.clone()).collect();
        let bodies = self.ctx.bodies.untracked();
        let found = touching(bodies, &shapes, exact);
        self.ctx.bodies.note_scope(shapes, found.iter().map(|&i| bodies[i].id.clone()).collect(), exact);
        found
    }

    fn fuse_named(&self, tools: Vec<NamedShape>) -> FResult<Vec<NamedShape>> {
        let shapes: Vec<Shape> = tools.iter().map(|t| t.shape.clone()).collect();
        let res = k::boolean(k::BooleanOp::Fuse, &shapes[..1], &shapes[1..], &k::BooleanOptions::default())?;
        let names: Vec<&ShapeNames> = tools.iter().map(|t| &t.names).collect();
        let face_names = self.name_result(&names, &res.history, &res.shape, &BTreeMap::new(), None);
        // Like Parasolid (and boolean_into), faces the union leaves on one
        // surface are one face (qa5 SF2: two adjacent regions extruded, 6
        // faces, no seam).
        let unified = k::unify_same_domain(&res.shape)?;
        if unify_merged(&res.shape, &unified.shape) {
            let before = unify_input_names(face_names);
            let names = self.name_result(&[&before], &unified.history, &unified.shape, &BTreeMap::new(), None);
            return Ok(split_solids(&unified.shape, &names, Some(self.feature)));
        }
        Ok(split_solids(&res.shape, &face_names, Some(self.feature)))
    }

    pub(crate) fn add_new_bodies(&mut self, t: NamedShape, kind: BodyKind, base_index: u32) -> FResult<()> {
        let pieces = if kind == BodyKind::Solid && t.shape.count(ShapeType::Solid) > 1 { split_solids(&t.shape, &t.names.faces, Some(self.feature)) } else { vec![t] };
        for (j, p) in pieces.into_iter().enumerate() {
            let name = self.ctx.alloc_part_name(kind);
            let color = self.ctx.alloc_color();
            let id = BodyId { feature: self.feature, index: base_index * 64 + j as u32 };
            let body = Body::named(id, name, kind, p.shape, p.names, color, self.feature);
            self.ctx.bodies.push(Arc::new(body));
        }
        Ok(())
    }

    /// Boolean tools into target bodies; results replace the targets.
    pub fn boolean_into(&mut self, targets: &[usize], tools: Vec<NamedShape>, op: k::BooleanOp) -> FResult<()> {
        self.boolean_into_with(targets, tools, op, &k::BooleanOptions::default())
    }

    /// Options for booleans with many tools of one kind (pattern instances):
    /// glue where the instances only touch the target (checked by volume),
    /// oriented boxes, in parallel.
    pub fn many_tools_options() -> k::BooleanOptions {
        k::BooleanOptions { glue: k::Glue::ShiftChecked, obb: true, parallel: true, ..Default::default() }
    }

    /// [`Self::boolean_into`] with explicit boolean options.
    pub fn boolean_into_with(&mut self, targets: &[usize], tools: Vec<NamedShape>, op: k::BooleanOp, opts: &k::BooleanOptions) -> FResult<()> {
        // Onshape adds into several parts as one union: the tool and every
        // part in the merge scope become one part (qa5 BOOL3), not a copy of
        // the tool fused into each part.
        if op == k::BooleanOp::Fuse && targets.len() > 1 {
            return self.fuse_targets(targets, tools, opts);
        }
        let tool_shapes: Vec<Shape> = tools.iter().map(|t| t.shape.clone()).collect();
        let mut new_bodies: Vec<(usize, Vec<Arc<Body>>)> = Vec::new();
        for &ti in targets {
            let target = self.ctx.bodies[ti].clone();
            // A cut or common only needs the tools whose boxes reach this
            // target (a fuse keeps every tool: the ones apart become parts).
            let tb = target.shape.bounding_box(false).ok().filter(|_| op != k::BooleanOp::Fuse);
            let near: Vec<usize> = (0..tools.len())
                .filter(|&i| match (&tb, tool_shapes[i].bounding_box(false)) {
                    (Some(a), Ok(b)) => overlaps(a, &b),
                    _ => true,
                })
                .collect();
            if near.is_empty() {
                continue;
            }
            let near_shapes: Vec<Shape> = near.iter().map(|&i| tool_shapes[i].clone()).collect();
            let perf = super::display::perf_enabled();
            let t0 = std::time::Instant::now();
            // A newer edit stops the kernel boolean too (pending state).
            let run = |o: &k::BooleanOptions| {
                let run = || k::boolean(op, std::slice::from_ref(&target.shape), &near_shapes, o);
                match &self.cancel {
                    Some(c) => c.scope(run),
                    None => run(),
                }
            };
            // A cut by many tools (400 holes) unifies in the kernel: then it
            // can cut slab by slab in parallel (boolean.cpp `cut_slabs`),
            // whose joined slabs need the unification this function does
            // below anyway (big plate PERF1).
            let opts = &if op == k::BooleanOp::Cut && near_shapes.len() >= 64 { k::BooleanOptions { unify: true, ..*opts } } else { *opts };
            let mut res = run(opts)?;
            if k::thread_cancelled() {
                return Err(FeatureError::new("cancelled"));
            }
            // OCCT sometimes returns an empty cut, with no error, for a valid
            // one (a hole running into a fillet, qa4 F12): never let a part
            // vanish that way. Retry with a small fuzzy tolerance; if it still
            // comes back empty, the feature fails instead.
            if op == k::BooleanOp::Cut && target.kind == BodyKind::Solid && res.shape.count(ShapeType::Solid) == 0 && !tools_engulf(&target.shape, &near_shapes) {
                let size = target.shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0);
                let mut fixed = false;
                for f in [1e-8, 1e-7, 1e-6] {
                    let r = run(&k::BooleanOptions { fuzzy: size * f, ..*opts })?;
                    if r.shape.count(ShapeType::Solid) > 0 {
                        res = r;
                        fixed = true;
                        break;
                    }
                }
                if !fixed {
                    return Err(FeatureError::new(format!("Removing material failed on {}", target.name)));
                }
            }
            let t_bool = t0.elapsed();
            // Aborted: no naming / unify of a result nobody will see.
            if self.cancelled() {
                return err("Cancelled");
            }
            // A fuse that merged nothing (tools apart from the part, or only
            // touching it along a line or at a point): the part stays as it
            // was, the tools become parts of their own (live request 1594: a
            // pattern's instances touching along a line, "Boolean resulted in
            // no geometry change", 3 faces each; qa TOUCH-PATTERN-ADD: the
            // touching line had split the part's face).
            let solids = |s: &Shape| s.count(ShapeType::Solid);
            if op == k::BooleanOp::Fuse && target.kind == BodyKind::Solid && solids(&target.shape) == 1 && near.iter().all(|&i| solids(&tool_shapes[i]) == 1) && solids(&res.shape) == 1 + near.len() {
                let mut bodies = vec![target.clone()];
                for (j, &i) in near.iter().enumerate() {
                    let name = self.ctx.alloc_part_name(target.kind);
                    let id = BodyId { feature: self.feature, index: 10_000 + (ti as u32) * 64 + 1 + j as u32 };
                    bodies.push(Arc::new(Body::named(id, name, target.kind, tools[i].shape.clone(), tools[i].names.clone(), target.color, self.feature)));
                }
                new_bodies.push((ti, bodies));
                continue;
            }
            let mut args: Vec<ShapeNames> = vec![target.shape_names()];
            args.extend(near.iter().map(|&i| tools[i].names.clone()));
            let arg_refs: Vec<&ShapeNames> = args.iter().collect();
            let face_names = self.name_result(&arg_refs, &res.history, &res.shape, &BTreeMap::new(), None);
            let t_named0 = t0.elapsed();
            // Like Parasolid, merge faces left coplanar (or on one cylinder...) by
            // the boolean: a block added flush with a face leaves one face.
            let unified = k::unify_same_domain(&res.shape)?;
            let t_unified = t0.elapsed();
            let (res, face_names) = if unify_merged(&res.shape, &unified.shape) {
                let before = unify_input_names(face_names);
                let names = self.name_result(&[&before], &unified.history, &unified.shape, &BTreeMap::new(), None);
                (unified, names)
            } else {
                (res, face_names)
            };
            if perf {
                log::info!(target: "perf", "boolean_into: naming {:.1} ms, unify {:.1} ms, naming the unified {:.1} ms", (t_named0 - t_bool).as_secs_f64() * 1e3, (t_unified - t_named0).as_secs_f64() * 1e3, (t0.elapsed() - t_unified).as_secs_f64() * 1e3);
            }
            // Parasolid refuses a cut leaving faces that meet along an edge
            // from both sides (live Bo3: a cylinder tangent to the block's
            // face, by Boolean or Extrude Remove); the parts stay as they were.
            if op != k::BooleanOp::Fuse && target.kind == BodyKind::Solid && non_manifold(&res.shape) && !non_manifold(&target.shape) {
                return Err(FeatureError::new("Boolean operation would result in non-manifold body.".to_string()));
            }
            let t_named = t0.elapsed();
            let pieces = split_solids(&res.shape, &face_names, Some(self.feature));
            if perf {
                log::info!(target: "perf", "boolean_into {:?} × {} tools: kernel {:.1} ms, naming + unify {:.1} ms, split {:.1} ms", op, near.len(), t_bool.as_secs_f64() * 1e3, (t_named - t_bool).as_secs_f64() * 1e3, (t0.elapsed() - t_named).as_secs_f64() * 1e3);
            }
            let t_split = t0.elapsed();
            let mut bodies = Vec::new();
            for (j, p) in pieces.into_iter().enumerate() {
                if p.shape.count(ShapeType::Face) == 0 {
                    continue;
                }
                let (id, name, color) = if j == 0 {
                    (target.id.clone(), target.name.clone(), target.color)
                } else {
                    let name = self.ctx.alloc_part_name(target.kind);
                    (BodyId { feature: self.feature, index: 10_000 + (ti as u32) * 64 + j as u32 }, name, target.color)
                };
                bodies.push(Arc::new(Body::named(id, name, target.kind, p.shape, p.names, color, self.feature)));
            }
            if perf {
                log::info!(target: "perf", "boolean_into: bodies (edge / vertex names) {:.1} ms", (t0.elapsed() - t_split).as_secs_f64() * 1e3);
            }
            new_bodies.push((ti, bodies));
        }
        // Replace targets (descending index order keeps indices valid).
        new_bodies.sort_by_key(|b| std::cmp::Reverse(b.0));
        for (ti, bodies) in new_bodies {
            self.ctx.bodies.remove(ti);
            for (k2, b) in bodies.into_iter().enumerate() {
                self.ctx.bodies.insert(ti + k2, b);
            }
        }
        Ok(())
    }

    /// One union of the bodies `targets` and `tools`. Each resulting piece
    /// keeps the id, name and colour of the first target (in `targets`
    /// order) whose faces it holds; a target merged into an earlier one's
    /// piece goes, and a piece holding no target is a new part. Disjoint
    /// targets stay separate parts with their own identity (live: a Boolean
    /// union of parts apart leaves both, qa5 BOOL1).
    pub fn fuse_targets(&mut self, targets: &[usize], tools: Vec<NamedShape>, opts: &k::BooleanOptions) -> FResult<()> {
        let mut targets: Vec<usize> = targets.to_vec();
        targets.dedup();
        let bodies: Vec<Arc<Body>> = targets.iter().map(|&ti| self.ctx.bodies[ti].clone()).collect();
        let mut shapes: Vec<Shape> = bodies.iter().map(|b| b.shape.clone()).collect();
        shapes.extend(tools.iter().map(|t| t.shape.clone()));
        if shapes.len() < 2 {
            return Ok(());
        }
        let run = || k::boolean(k::BooleanOp::Fuse, &shapes[..1], &shapes[1..], opts);
        let res = match &self.cancel {
            Some(c) => c.scope(run),
            None => run(),
        }?;
        let mut args: Vec<ShapeNames> = bodies.iter().map(|b| b.shape_names()).collect();
        args.extend(tools.iter().map(|t| t.names.clone()));
        let arg_refs: Vec<&ShapeNames> = args.iter().collect();
        let face_names = self.name_result(&arg_refs, &res.history, &res.shape, &BTreeMap::new(), None);
        let unified = k::unify_same_domain(&res.shape)?;
        let (shape, face_names) = if unify_merged(&res.shape, &unified.shape) {
            let before = unify_input_names(face_names);
            let names = self.name_result(&[&before], &unified.history, &unified.shape, &BTreeMap::new(), None);
            (unified.shape, names)
        } else {
            (res.shape, face_names)
        };
        let pieces = split_solids(&shape, &face_names, Some(self.feature));
        // Which target each piece holds first (by its faces' original names).
        let roots: Vec<std::collections::HashSet<&TopoName>> = bodies.iter().map(|b| b.faces.iter().map(|n| n.root()).collect()).collect();
        let mut owned: Vec<(usize, NamedShape)> = Vec::new();
        let mut free: Vec<NamedShape> = Vec::new();
        for p in pieces.into_iter().filter(|p| p.shape.count(ShapeType::Face) > 0) {
            let owner = (0..bodies.len()).find(|&t| !owned.iter().any(|(o, _)| *o == t) && p.names.faces.iter().any(|n| roots[t].contains(n.root())));
            match owner {
                Some(t) => owned.push((t, p)),
                None => free.push(p),
            }
        }
        // Each kept part stays in its place in the list; merged ones go; new
        // pieces follow the first kept part.
        let mut replaced: BTreeMap<usize, Arc<Body>> = BTreeMap::new();
        for (t, p) in owned {
            let b = &bodies[t];
            replaced.insert(targets[t], Arc::new(Body::named(b.id.clone(), b.name.clone(), b.kind, p.shape, p.names, b.color, self.feature)));
        }
        let (kind, color) = (bodies[0].kind, bodies[0].color);
        let mut extra: Vec<Arc<Body>> = Vec::new();
        for (j, p) in free.into_iter().enumerate() {
            let name = self.ctx.alloc_part_name(kind);
            extra.push(Arc::new(Body::named(BodyId { feature: self.feature, index: 30_000 + j as u32 }, name, kind, p.shape, p.names, color, self.feature)));
        }
        let first_kept = replaced.keys().next().copied();
        let old = std::mem::take(&mut self.ctx.bodies);
        for (i, b) in old.into_iter().enumerate() {
            if let Some(nb) = replaced.remove(&i) {
                self.ctx.bodies.push(nb);
                if Some(i) == first_kept {
                    self.ctx.bodies.append(&mut extra);
                }
            } else if !targets.contains(&i) {
                self.ctx.bodies.push(b);
            }
        }
        self.ctx.bodies.append(&mut extra);
        Ok(())
    }

    /// Replace a body's shape after a local operation (fillet, shell...).
    pub fn replace_body(&mut self, bi: usize, shape: Shape, faces: Vec<Name>) {
        let old = self.ctx.bodies[bi].clone();
        let pieces = split_solids(&shape, &faces, Some(self.feature));
        let mut iter = pieces.into_iter();
        if let Some(first) = iter.next() {
            self.ctx.bodies[bi] = Arc::new(Body::named(old.id.clone(), old.name.clone(), old.kind, first.shape, first.names, old.color, self.feature));
        }
        for (j, p) in iter.enumerate() {
            let name = self.ctx.alloc_part_name(old.kind);
            let id = BodyId { feature: self.feature, index: 20_000 + j as u32 };
            self.ctx.bodies.insert(bi + 1 + j, Arc::new(Body::named(id, name, old.kind, p.shape, p.names, old.color, self.feature)));
        }
    }
}

fn overlaps(a: &k::BoundingBox, b: &k::BoundingBox) -> bool {
    let e = 1e-7;
    a.min.x <= b.max.x + e && b.min.x <= a.max.x + e && a.min.y <= b.max.y + e && b.min.y <= a.max.y + e && a.min.z <= b.max.z + e && b.min.z <= a.max.z + e
}

/// A shape plus names of its faces (edges/vertices derive from them).
#[derive(Clone)]
pub struct NamedShape {
    pub shape: Shape,
    pub names: ShapeNames,
}

impl NamedShape {
    pub fn new(shape: Shape, faces: Vec<Name>) -> Self {
        let names = names_for(&shape, faces);
        Self { shape, names }
    }
}

/// A unify's input names for naming its result: the faces' only. The
/// unify's history takes faces to faces (merge_planar, UnifySameDomain), so
/// the edge and vertex names, which cost two ancestor maps of the whole part
/// (the 1600-face fin plate: ~30 ms), would go unread.
fn unify_input_names(faces: Vec<Name>) -> ShapeNames {
    ShapeNames { faces, edges: Vec::new(), vertices: Vec::new() }
}

/// Full names (faces + derived edges/vertices) for a shape.
pub fn names_for(shape: &Shape, faces: Vec<Name>) -> ShapeNames {
    let ef = shape.edge_faces().unwrap_or_default();
    let vf = shape.vertex_faces().unwrap_or_default();
    let edges = crate::naming::name_edges(&faces, &ef, &|i| super::context::edge_key(shape, i));
    let vertices = crate::naming::name_vertices(&faces, &vf, &|i| super::context::vertex_key(shape, i));
    ShapeNames { faces, edges, vertices }
}

/// Split a (compound) result into solids (or shells/faces for sheets), carrying face names.
/// `apart`: the feature whose result is divided into separate bodies; a face the pieces share (a split's cut face,
/// one face in both solids) is named apart per piece (`TopoName::Split` under it, by piece) so that a reference
/// picked on one piece resolves on that piece only, as Parasolid's ids (user P0 11-02: a Chamfer of one Split
/// piece's face chamfered all three). A reference made before (the shared name) still finds every piece's face
/// through `descends_from`. None: the pieces don't become bodies (the largest is kept), the names as they are.
pub fn split_solids(shape: &Shape, face_names: &[Name], apart: Option<FeatureId>) -> Vec<NamedShape> {
    let solids = shape.solids();
    let parts: Vec<Shape> = if !solids.is_empty() {
        solids
    } else if shape.shape_type() == ShapeType::Compound {
        let mut shells = shape.shells();
        if shells.is_empty() {
            vec![shape.clone()]
        } else {
            // Faces in no shell (a sheet that touched nothing) are pieces too.
            let shelled: Vec<Shape> = shells.iter().flat_map(|s| s.faces()).collect();
            let loose: Vec<Shape> = shape.faces().into_iter().filter(|f| !shelled.iter().any(|g| g.is_same(f))).collect();
            shells.extend(loose);
            shells
        }
    } else {
        vec![shape.clone()]
    };
    if parts.len() == 1 && parts[0].is_same(shape) {
        return vec![NamedShape::new(shape.clone(), face_names.to_vec())];
    }
    let mut pieces: Vec<NamedShape> = parts
        .into_iter()
        .map(|p| {
            let names: Vec<Name> = p
                .faces()
                .iter()
                .map(|f| shape.index_of(f, ShapeType::Face).and_then(|i| face_names.get(i).cloned()).unwrap_or_else(|| Arc::new(TopoName::face(FeatureId(0), Role::New(0)))))
                .collect();
            NamedShape::new(p, names)
        })
        .collect();
    if let Some(by) = apart {
        name_pieces_apart(&mut pieces, by);
    }
    pieces
}

/// Faces named alike on several pieces, named apart per piece (see [`split_solids`]).
pub fn name_pieces_apart(pieces: &mut [NamedShape], by: FeatureId) {
    let mut owners: std::collections::HashMap<Name, Vec<usize>> = std::collections::HashMap::new();
    for (pi, p) in pieces.iter().enumerate() {
        for n in &p.names.faces {
            let o = owners.entry(n.clone()).or_default();
            if o.last() != Some(&pi) {
                o.push(pi);
            }
        }
    }
    let shared: std::collections::HashSet<Name> = owners.into_iter().filter(|(_, o)| o.len() > 1).map(|(n, _)| n).collect();
    if shared.is_empty() {
        return;
    }
    for (pi, p) in pieces.iter_mut().enumerate() {
        let faces: Vec<Name> = p.names.faces.iter().map(|n| if shared.contains(n) { Arc::new(TopoName::Split { parent: n.clone(), by, ordinal: pi as u16 }) } else { n.clone() }).collect();
        *p = NamedShape::new(p.shape.clone(), faces);
    }
}


/// A profile face for extrude/revolve with names for its edges.
pub struct Profile {
    pub shape: Shape,
    pub frame: PlaneFrame,
    /// Source of each edge of `shape` (indexed like `shape.edges()`).
    pub edge_sources: Vec<Source>,
    /// Identity of the profile (for cap names).
    pub key: Source,
    pub from_face: bool,
}

/// Kernel plane whose local 2D coordinates match `f.to_world`. For a
/// mirrored (left-handed) frame the kernel normal is `x × y`, i.e. opposite
/// to `f.normal`, so 2D sketch geometry still lands where `to_world` puts it.
pub fn kernel_plane(f: &PlaneFrame) -> k::Plane {
    let n = f.x.cross(f.y);
    let n = if n.length_squared() > 0.25 && n.dot(f.normal) < 0.0 { n.normalize() } else { f.normal };
    k::Plane::new(f.origin, n, f.x)
}

/// Convert a solver profile segment to a kernel segment.
pub fn to_segment(s: &noshape_sketch::regions::ProfileSegment) -> k::Segment2d {
    use noshape_sketch::regions::ProfileSegment as P;
    match s {
        P::Line { a, b } => k::Segment2d::Line { a: *a, b: *b },
        P::Arc { center, radius, start_angle, end_angle, .. } => {
            if (end_angle - start_angle).abs() >= std::f64::consts::TAU - 1e-9 {
                k::Segment2d::Circle { center: *center, radius: *radius }
            } else {
                let mid = (start_angle + end_angle) * 0.5;
                let at = |t: f64| *center + DVec2::new(t.cos(), t.sin()) * *radius;
                k::Segment2d::Arc3 { a: at(*start_angle), mid: at(mid), b: at(*end_angle) }
            }
        }
        P::EllipseArc { center, major_axis, major_radius, minor_radius, start_param, end_param, ccw } => {
            let rotation = major_axis.y.atan2(major_axis.x);
            if (end_param - start_param).abs() >= std::f64::consts::TAU - 1e-9 {
                k::Segment2d::Ellipse { center: *center, major_radius: *major_radius, minor_radius: *minor_radius, rotation }
            } else {
                let (a0, a1) = if *ccw { (*start_param, *end_param) } else { (*end_param, *start_param) };
                k::Segment2d::EllipseArc { center: *center, major_radius: *major_radius, minor_radius: *minor_radius, rotation, start_angle: a0, end_angle: a1 }
            }
        }
        P::Spline { control_points, knots, degree, weights } => {
            let (uk, mults) = unique_knots(knots);
            k::Segment2d::BSpline { poles: control_points.clone(), weights: weights.clone(), knots: uk, mults, degree: *degree as u32, periodic: false }
        }
    }
}

fn unique_knots(knots: &[f64]) -> (Vec<f64>, Vec<u32>) {
    let mut uk: Vec<f64> = Vec::new();
    let mut m: Vec<u32> = Vec::new();
    for &kv in knots {
        if let Some(last) = uk.last()
            && (kv - last).abs() < 1e-12
        {
            *m.last_mut().unwrap() += 1;
            continue;
        }
        uk.push(kv);
        m.push(1);
    }
    (uk, m)
}

/// A sketch region with its sketch.
type RegionOf = (Arc<SketchResult>, noshape_sketch::regions::Region);

/// Kernel face for a sketch region, with each edge's sketch source.
pub fn region_profile(s: &SketchResult, region: &noshape_sketch::regions::Region, key: Source) -> FResult<Profile> {
    let plane = kernel_plane(&s.frame);
    // Merge consecutive arcs of the same circle entity (a full circle split at
    // a seam becomes one edge, so a hole is one cylindrical face).
    let merge = |lp: &noshape_sketch::regions::ProfileLoop| -> Vec<(noshape_sketch::regions::ProfileSegment, u32)> {
        use noshape_sketch::regions::ProfileSegment as P;
        let mut out: Vec<(P, u32)> = Vec::new();
        // The entity each `out` segment ends with (a merged line's last).
        let mut last: Vec<u32> = Vec::new();
        let legacy = noshape_kernel::legacy_topology();
        // Where two lines meet, no other curve of the sketch touches.
        let alone = |p: DVec2, a: u32, b: u32| {
            s.sketch.entities().filter(|e| !e.construction && e.kind() != noshape_sketch::EntityKind::Point && e.id.0 != a && e.id.0 != b).all(|e| s.sketch.curve(e.id).is_none_or(|c| c.distance(p) > 1e-9))
        };
        let joins = |a0: DVec2, b0: DVec2, a: DVec2, b: DVec2, e0: u32, e: u32| {
            let (d0, d) = ((b0 - a0).normalize_or_zero(), (b - a).normalize_or_zero());
            (b0 - a).length() < 1e-12 && d0.perp_dot(d).abs() < 1e-9 && d0.dot(d) > 0.0 && (e0 == e || alone(a, e0, e))
        };
        for (seg, src) in lp.segments.iter().zip(lp.sources.iter()) {
            if let (Some((P::Arc { center: c0, radius: r0, start_angle: s0, end_angle: e0, ccw: w0 }, e_prev)), P::Arc { center, radius, end_angle, ccw, .. }) = (out.last_mut(), seg)
                && *e_prev == src.entity.0
                && (*c0 - *center).length() < 1e-12
                && (*r0 - *radius).abs() < 1e-12
                && *w0 == *ccw
            {
                let sweep_prev = *e0 - *s0;
                let mut add = *end_angle - (*e0 - 0.0);
                let _ = &mut add;
                let seg_sweep = match seg { P::Arc { start_angle, end_angle, .. } => end_angle - start_angle, _ => 0.0 };
                *e0 = *s0 + sweep_prev + seg_sweep;
                continue;
            }
            // Likewise one line's pieces (split where other curves lie over it or end on it), and collinear lines
            // meeting end to end where nothing else of the sketch does: one edge, so one side face (live:
            // bracket120's bottom face, learning-center-frank's base of two mirrored lines; not in a studio saved
            // before, legacy_topology).
            if let (Some((P::Line { a: a0, b: b0 }, _)), P::Line { a, b }, Some(e_last)) = (out.last_mut(), seg, last.last_mut())
                && !legacy
                && joins(*a0, *b0, *a, *b, *e_last, src.entity.0)
            {
                *b0 = *b;
                *e_last = src.entity.0;
                continue;
            }
            out.push((seg.clone(), src.entity.0));
            last.push(src.entity.0);
        }
        // (The loop may start inside that line.)
        if out.len() > 2
            && !legacy
            && let (Some((P::Line { a: la, b: lb }, _)), Some((P::Line { a: fa, b: fb }, fe)), Some(le)) = (out.last(), out.first(), last.last())
            && joins(*la, *lb, *fa, *fb, *le, *fe)
        {
            let a = *la;
            out.pop();
            if let Some((P::Line { a: fa, .. }, _)) = out.first_mut() {
                *fa = a;
            }
        }
        // A loop made only of one entity's arcs may also wrap around at the start.
        if out.len() == 2
            && out[0].1 == out[1].1
            && let (P::Arc { start_angle: a0, end_angle: a1, center, radius, ccw }, P::Arc { start_angle: b0, end_angle: b1, .. }) = (&out[0].0, &out[1].0)
            && ((a1 - a0) + (b1 - b0)).abs() >= std::f64::consts::TAU - 1e-9
        {
            let e = out[0].1;
            let full = P::Arc { center: *center, radius: *radius, start_angle: *a0, end_angle: *a0 + (a1 - a0) + (b1 - b0), ccw: *ccw };
            out = vec![(full, e)];
        }
        // Likewise a whole ellipse (two halves of one entity): one edge, so
        // its extrusion is one face (live psb NE: an elliptic boss's side is
        // one face; OCCT can't merge two extrusion halves afterwards).
        if out.len() == 2
            && out[0].1 == out[1].1
            && let (P::EllipseArc { center, major_axis, major_radius, minor_radius, start_param: a0, end_param: a1, ccw }, P::EllipseArc { start_param: b0, end_param: b1, .. }) = (&out[0].0, &out[1].0)
            && ((a1 - a0).abs() + (b1 - b0).abs()) >= std::f64::consts::TAU - 1e-9
        {
            let e = out[0].1;
            let full = P::EllipseArc { center: *center, major_axis: *major_axis, major_radius: *major_radius, minor_radius: *minor_radius, start_param: *a0, end_param: *a0 + std::f64::consts::TAU, ccw: *ccw };
            out = vec![(full, e)];
        }
        // Likewise a closed spline (a periodic fit spline, split at its
        // seam): one edge, so its extrusion is one face (live's lobed recess).
        if out.len() == 2
            && out[0].1 == out[1].1
            && matches!((&out[0].0, &out[1].0), (P::Spline { .. }, P::Spline { .. }))
            && let Some(noshape_sketch::Curve2::Nurbs(n)) = s.sketch.curve(noshape_sketch::EntityId(out[0].1))
            && noshape_sketch::Curve2::Nurbs(n.clone()).is_closed()
        {
            let forward = lp.sources.first().is_none_or(|x| x.forward);
            let n = if forward { n } else { n.reversed() };
            let e = out[0].1;
            out = vec![(P::Spline { control_points: n.ctrl, knots: n.knots, degree: n.degree, weights: n.weights }, e)];
        }
        out
    };
    // A line the loop runs along and back (a slit: a line joining a ring's two circles, live learning-center-michael):
    // the loop cut there, its pieces the outer loop and holes (a dead end's pair dropped).
    let mut outer_m = merge(&region.outer);
    let mut holes_m: Vec<_> = region.holes.iter().map(merge).collect();
    let pieces = split_slits(outer_m.clone());
    if pieces.len() > 1 || pieces.first().is_some_and(|p| p.len() != outer_m.len()) {
        let area = |lp: &[(noshape_sketch::regions::ProfileSegment, u32)]| -> f64 {
            let pts: Vec<glam::DVec2> = lp.iter().flat_map(|(sg, _)| sg.tessellate(1e-6)).collect();
            (0..pts.len()).map(|i| pts[i].perp_dot(pts[(i + 1) % pts.len()])).sum::<f64>() * 0.5
        };
        let mut pieces: Vec<_> = pieces.into_iter().filter(|p| !p.is_empty()).collect();
        pieces.sort_by(|a, b| area(b).abs().total_cmp(&area(a).abs()));
        let mut it = pieces.into_iter();
        if let Some(first) = it.next() {
            outer_m = first;
            holes_m.extend(it);
        }
    }
    // Each line and arc edge runs its sketch curve's way, as drawn (live: an extruded rectangle's edges keep the
    // drawing order, a pattern along one goes its way), not the loop's.
    let edge = |(sg, e): &(noshape_sketch::regions::ProfileSegment, u32)| {
        use noshape_sketch::regions::ProfileSegment as P;
        let x = to_segment(sg);
        if s.legacy_edge_directions {
            return x;
        }
        match (x, sg, s.sketch.curve(noshape_sketch::EntityId(*e))) {
            (k::Segment2d::Line { a, b }, _, Some(noshape_sketch::Curve2::Line { a: ca, b: cb })) if (b - a).dot(cb - ca) < 0.0 => k::Segment2d::Line { a: b, b: a },
            (k::Segment2d::Arc3 { a, mid, b }, P::Arc { ccw: false, .. }, _) => k::Segment2d::Arc3 { a: b, mid, b: a },
            (x, ..) => x,
        }
    };
    let outer: Vec<k::Segment2d> = outer_m.iter().map(edge).collect();
    let holes: Vec<Vec<k::Segment2d>> = holes_m.iter().map(|h| h.iter().map(edge).collect()).collect();
    let tracked = k::make_face_2d_tracked(&plane, &outer, &holes)?;
    let mut sources_in: Vec<Source> = Vec::new();
    let mut count: BTreeMap<u32, u16> = BTreeMap::new();
    for lp in std::iter::once(&outer_m).chain(holes_m.iter()) {
        for (_, e) in lp {
            let c = count.entry(*e).or_insert(0);
            sources_in.push(Source::Sketch { sketch: s.feature, entity: *e, segment: *c });
            *c += 1;
        }
    }
    let n_edges = tracked.shape.count(ShapeType::Edge);
    let mut edge_sources = vec![Source::Index(0); n_edges];
    for (i, &e) in tracked.segment_edges.iter().enumerate() {
        if let (Some(slot), Some(src)) = (edge_sources.get_mut(e as usize), sources_in.get(i)) {
            *slot = src.clone();
        }
    }
    for (i, s2) in edge_sources.iter_mut().enumerate() {
        if matches!(s2, Source::Index(_)) {
            *s2 = Source::Index(i as u32);
        }
    }
    Ok(Profile { shape: tracked.shape, frame: s.frame, edge_sources, key, from_face: false })
}

/// A loop cut at each line it runs along and back along (a slit): the pieces between, each a loop.
fn split_slits(lp: Vec<(noshape_sketch::regions::ProfileSegment, u32)>) -> Vec<Vec<(noshape_sketch::regions::ProfileSegment, u32)>> {
    let n = lp.len();
    let back = |a: &noshape_sketch::regions::ProfileSegment, b: &noshape_sketch::regions::ProfileSegment| matches!(a, noshape_sketch::regions::ProfileSegment::Line { .. }) && (a.start() - b.end()).length() < 1e-12 && (a.end() - b.start()).length() < 1e-12;
    for i in 0..n {
        for j in i + 1..n {
            if lp[i].1 == lp[j].1 && back(&lp[i].0, &lp[j].0) {
                let a: Vec<_> = lp[i + 1..j].to_vec();
                let b: Vec<_> = lp[j + 1..].iter().chain(lp[..i].iter()).cloned().collect();
                return split_slits(a).into_iter().chain(split_slits(b)).filter(|p| !p.is_empty()).collect();
            }
        }
    }
    vec![lp]
}

/// Encode a sketch solver region id into a reference key.
pub fn encode_region_key(id: &noshape_sketch::regions::RegionId) -> Vec<u32> {
    let mut v = vec![id.ordinal];
    v.extend(id.boundary.iter().map(|s| (s.entity.0 << 1) | s.forward as u32));
    v
}

pub fn decode_region_key(key: &[u32]) -> Option<noshape_sketch::regions::RegionId> {
    let (ord, rest) = key.split_first()?;
    Some(noshape_sketch::regions::RegionId {
        ordinal: *ord,
        boundary: rest.iter().map(|v| noshape_sketch::regions::SegmentSource { entity: noshape_sketch::EntityId(v >> 1), forward: v & 1 == 1 }).collect(),
    })
}

/// Names for a swept profile result: caps by role, sides from profile edges.
pub fn sweep_names(ev: &EvalCtx, profile: &Profile, res: &k::OpResult, start_cap: bool, end_cap: bool) -> Vec<Name> {
    let mut roles = BTreeMap::new();
    if start_cap {
        for f in res.history.first_faces() {
            roles.insert(f, Role::CapStart(Some(profile.key.clone())));
        }
    }
    if end_cap {
        for f in res.history.last_faces() {
            roles.insert(f, Role::CapEnd(Some(profile.key.clone())));
        }
    }
    // Edge -> side face roles.
    for r in &res.history.records {
        if r.arg == 0 && r.input_kind == ShapeType::Edge && r.relation == k::Relation::Generated && r.output_kind == ShapeType::Face
            && let Some(src) = profile.edge_sources.get(r.input_index as usize) {
                roles.entry(r.output_index).or_insert_with(|| Role::Side(src.clone()));
            }
    }
    let empty = ShapeNames::default();
    ev.name_result(&[&empty], &res.history, &res.shape, &roles, None)
}

/// Whether a unify merged anything: faces, or edges only (a hole's loop in pieces across a seam made whole; not in a
/// studio saved before that, [`noshape_kernel::legacy_topology`]).
pub fn unify_merged(before: &Shape, after: &Shape) -> bool {
    after.count(ShapeType::Face) < before.count(ShapeType::Face) || (!noshape_kernel::legacy_topology() && after.count(ShapeType::Edge) < before.count(ShapeType::Edge))
}

/// Whether the tools can account for a cut that leaves nothing of `target`
/// (together at least its volume).
fn tools_engulf(target: &Shape, tools: &[Shape]) -> bool {
    let Ok(v) = target.volume() else { return true };
    let t: f64 = tools.iter().filter_map(|t| t.volume().ok()).map(f64::abs).sum();
    t >= v.abs() * (1.0 - 1e-6)
}

/// An edge of `shape` bounding more than two faces.
fn non_manifold(shape: &Shape) -> bool {
    shape.edge_faces().is_ok_and(|ef| {
        ef.iter().any(|f| {
            let mut f = f.clone();
            f.sort_unstable();
            f.dedup();
            f.len() > 2
        })
    })
}
