//! Feature evaluation context: parameter access, reference resolution,
//! profile construction, history-based naming and boolean merging.

use super::context::{Body, Context, SketchResult, face_key};
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
}

impl FeatureError {
    pub fn new(m: impl Into<String>) -> Self {
        Self { message: m.into(), warning: false }
    }
}

impl From<k::KernelError> for FeatureError {
    fn from(e: k::KernelError) -> Self {
        FeatureError::new(format!("Geometry operation failed: {e}"))
    }
}

pub type FResult<T> = Result<T, FeatureError>;

pub fn err<T>(m: impl Into<String>) -> FResult<T> {
    Err(FeatureError::new(m))
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
    /// Sketch being edited (for display of constraint states).
    pub editing_sketch: Option<FeatureId>,
    /// Other Part Studios of the document (for Derived).
    pub derived: Option<std::sync::Arc<dyn super::derived::DerivedProvider>>,
    /// Every feature of the Part Studio in list order (empty outside a
    /// Part Studio regeneration).
    pub features: &'a [EffectiveFeature],
    /// Pattern instance transform while a feature is re-applied by a feature
    /// pattern / mirror. Geometry returned by the `resolve_*` helpers
    /// (points, planes, axes, directions, mate connectors and face profiles)
    /// is mapped by it; sketches are transformed in the context itself (see
    /// `features::pattern`), and topology lookups (`resolve_faces`,
    /// `resolve_edges`, `resolve_bodies`) are unaffected.
    pub xform: Option<DMat4>,
    /// Feature Studio scripts of the document (custom features).
    pub custom: Option<Arc<dyn crate::custom::CustomFeatureProvider>>,
    /// Type id of the custom feature being evaluated (`custom:<tab>:<name>`).
    pub custom_type: Option<String>,
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
        EvalCtx { feature, name, params, units, ctx, outer_vars, warnings: Vec::new(), editing_sketch: None, derived: None, features: &[], xform: None, custom: None, custom_type: None }
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
        let q = noshape_units::evaluate_expr_as(&expr, kind.dim(), self.units, &scope).map_err(|x| FeatureError::new(format!("{}: {x}", e.text)))?;
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
                Reference::Face(n) => {
                    for (bi, b) in self.ctx.bodies.iter().enumerate() {
                        for fi in b.find_faces(n) {
                            out.push((bi, fi));
                        }
                    }
                }
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
                Reference::Edge(n) => {
                    for (bi, b) in self.ctx.bodies.iter().enumerate() {
                        for ei in b.find_edges(n) {
                            out.push((bi, ei));
                        }
                    }
                }
                // A face selection means all of its edges (Onshape fillet/chamfer behavior).
                Reference::Face(n) => {
                    for (bi, b) in self.ctx.bodies.iter().enumerate() {
                        for fi in b.find_faces(n) {
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
                for b in &self.ctx.bodies {
                    if let Some(vi) = b.find_vertices(n).first() {
                        return b.shape.sub_shape(ShapeType::Vertex, *vi).ok()?.vertex_point().ok();
                    }
                }
                None
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
                    if let Some(i) = self.ctx.bodies.iter().position(|b| &b.id == id) {
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

    /// Like [`Self::resolve_plane`] without the pattern transform.
    pub fn raw_plane(&self, r: &Reference) -> Option<PlaneFrame> {
        match r {
            Reference::Plane(DatumPlane::Top) => Some(PlaneFrame::TOP),
            Reference::Plane(DatumPlane::Front) => Some(PlaneFrame::FRONT),
            Reference::Plane(DatumPlane::Right) => Some(PlaneFrame::RIGHT),
            Reference::FeaturePlane(_) => self.ctx.planes.iter().find(|p| &p.reference == r).map(|p| p.frame),
            Reference::Face(_) => {
                let (bi, fi) = *self.resolve_faces(std::slice::from_ref(r)).first()?;
                let face = self.face_shape(bi, fi).ok()?;
                let pl = face.face_plane().ok()??;
                // Sketch origin on a face: world origin projected onto the face plane.
                let n = pl.unit_normal();
                let x = pl.unit_x();
                let origin = pl.origin - n * (pl.origin.dot(n));
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
                    k::Curve::Line { origin, direction } => Some((origin, direction.normalize())),
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
            Reference::Plane(_) | Reference::FeaturePlane(_) => {
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
            && matches!(r, Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::Face(_))
        {
            return Some(f.normal);
        }
        self.resolve_axis(r).map(|(_, d)| d)
    }

    /// Sketch region by encoded key.
    pub fn resolve_region(&self, sketch: FeatureId, key: &[u32]) -> Option<(Arc<SketchResult>, noshape_sketch::regions::Region)> {
        let s = self.ctx.sketch(sketch)?.clone();
        let id = decode_region_key(key)?;
        let r = s.sketch.find_region(&id)?;
        Some((s, r))
    }

    // ------------------------------------------------------------ profiles

    /// Build profile faces from sketch regions and planar faces. Face
    /// profiles follow the pattern transform (sketch regions are already
    /// transformed through their sketch's frame).
    pub fn profiles(&self, refs: &[Reference]) -> FResult<Vec<Profile>> {
        let mut out = Vec::new();
        for r in refs {
            match r {
                Reference::SketchRegion { sketch, key } => {
                    let Some((s, region)) = self.resolve_region(*sketch, key) else {
                        return err("A selected sketch region no longer exists");
                    };
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
                let targets = match scope {
                    Some(s) if !s.is_empty() => s,
                    _ => self.auto_scope(&tools),
                };
                if targets.is_empty() {
                    if op == "ADD" {
                        // Nothing to merge with: behave like New (Onshape's auto heuristic).
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
                self.boolean_into(&targets, tools, bop)
            }
            other => err(format!("Unknown operation {other}")),
        }
    }

    /// Bodies whose bounding boxes overlap any tool (auto merge scope).
    pub fn auto_scope(&self, tools: &[NamedShape]) -> Vec<usize> {
        let tb: Vec<k::BoundingBox> = tools.iter().filter_map(|t| t.shape.bounding_box(false).ok()).collect();
        self.ctx
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind == BodyKind::Solid)
            .filter(|(_, b)| {
                let Ok(bb) = b.shape.bounding_box(false) else { return false };
                tb.iter().any(|t| overlaps(&bb, t))
            })
            .filter(|(_, b)| tools.iter().any(|t| k::distance(&b.shape, &t.shape).map(|d| d.distance < 1e-9).unwrap_or(false)))
            .map(|(i, _)| i)
            .collect()
    }

    fn fuse_named(&self, tools: Vec<NamedShape>) -> FResult<Vec<NamedShape>> {
        let shapes: Vec<Shape> = tools.iter().map(|t| t.shape.clone()).collect();
        let res = k::boolean(k::BooleanOp::Fuse, &shapes[..1], &shapes[1..], &k::BooleanOptions::default())?;
        let names: Vec<&ShapeNames> = tools.iter().map(|t| &t.names).collect();
        let face_names = self.name_result(&names, &res.history, &res.shape, &BTreeMap::new(), None);
        Ok(split_solids(&res.shape, &face_names))
    }

    fn add_new_bodies(&mut self, t: NamedShape, kind: BodyKind, base_index: u32) -> FResult<()> {
        let pieces = if kind == BodyKind::Solid && t.shape.count(ShapeType::Solid) > 1 { split_solids(&t.shape, &t.names.faces) } else { vec![t] };
        for (j, p) in pieces.into_iter().enumerate() {
            let name = self.ctx.alloc_part_name(kind);
            let color = self.ctx.alloc_color();
            let id = BodyId { feature: self.feature, index: base_index * 64 + j as u32 };
            let body = Body::new(id, name, kind, p.shape, p.names.faces, color, self.feature);
            self.ctx.bodies.push(Arc::new(body));
        }
        Ok(())
    }

    /// Boolean tools into target bodies; results replace the targets.
    pub fn boolean_into(&mut self, targets: &[usize], tools: Vec<NamedShape>, op: k::BooleanOp) -> FResult<()> {
        let tool_shapes: Vec<Shape> = tools.iter().map(|t| t.shape.clone()).collect();
        let mut new_bodies: Vec<(usize, Vec<Body>)> = Vec::new();
        for &ti in targets {
            let target = self.ctx.bodies[ti].clone();
            let res = k::boolean(op, std::slice::from_ref(&target.shape), &tool_shapes, &k::BooleanOptions::default())?;
            let mut args: Vec<ShapeNames> = vec![target.shape_names()];
            args.extend(tools.iter().map(|t| t.names.clone()));
            let arg_refs: Vec<&ShapeNames> = args.iter().collect();
            let face_names = self.name_result(&arg_refs, &res.history, &res.shape, &BTreeMap::new(), None);
            let pieces = split_solids(&res.shape, &face_names);
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
                bodies.push(Body::new(id, name, target.kind, p.shape, p.names.faces, color, self.feature));
            }
            new_bodies.push((ti, bodies));
        }
        // Replace targets (descending index order keeps indices valid).
        new_bodies.sort_by_key(|b| std::cmp::Reverse(b.0));
        for (ti, bodies) in new_bodies {
            self.ctx.bodies.remove(ti);
            for (k2, b) in bodies.into_iter().enumerate() {
                self.ctx.bodies.insert(ti + k2, Arc::new(b));
            }
        }
        Ok(())
    }

    /// Replace a body's shape after a local operation (fillet, shell...).
    pub fn replace_body(&mut self, bi: usize, shape: Shape, faces: Vec<Name>) {
        let old = self.ctx.bodies[bi].clone();
        let pieces = split_solids(&shape, &faces);
        let mut iter = pieces.into_iter();
        if let Some(first) = iter.next() {
            self.ctx.bodies[bi] = Arc::new(Body::new(old.id.clone(), old.name.clone(), old.kind, first.shape, first.names.faces, old.color, self.feature));
        }
        for (j, p) in iter.enumerate() {
            let name = self.ctx.alloc_part_name(old.kind);
            let id = BodyId { feature: self.feature, index: 20_000 + j as u32 };
            self.ctx.bodies.insert(bi + 1 + j, Arc::new(Body::new(id, name, old.kind, p.shape, p.names.faces, old.color, self.feature)));
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

/// Full names (faces + derived edges/vertices) for a shape.
pub fn names_for(shape: &Shape, faces: Vec<Name>) -> ShapeNames {
    let ef = shape.edge_faces().unwrap_or_default();
    let vf = shape.vertex_faces().unwrap_or_default();
    let edges = crate::naming::name_edges(&faces, &ef, &|i| super::context::edge_key(shape, i));
    let vertices = crate::naming::name_vertices(&faces, &vf, &|i| super::context::vertex_key(shape, i));
    ShapeNames { faces, edges, vertices }
}

/// Split a (compound) result into solids (or shells/faces for sheets), carrying face names.
pub fn split_solids(shape: &Shape, face_names: &[Name]) -> Vec<NamedShape> {
    let solids = shape.solids();
    let parts: Vec<Shape> = if !solids.is_empty() {
        solids
    } else if shape.shape_type() == ShapeType::Compound {
        let shells = shape.shells();
        if shells.is_empty() { vec![shape.clone()] } else { shells }
    } else {
        vec![shape.clone()]
    };
    if parts.len() == 1 && parts[0].is_same(shape) {
        return vec![NamedShape::new(shape.clone(), face_names.to_vec())];
    }
    parts
        .into_iter()
        .map(|p| {
            let names: Vec<Name> = p
                .faces()
                .iter()
                .map(|f| shape.index_of(f, ShapeType::Face).and_then(|i| face_names.get(i).cloned()).unwrap_or_else(|| Arc::new(TopoName::face(FeatureId(0), Role::New(0)))))
                .collect();
            NamedShape::new(p, names)
        })
        .collect()
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

/// Kernel face for a sketch region, with each edge's sketch source.
pub fn region_profile(s: &SketchResult, region: &noshape_sketch::regions::Region, key: Source) -> FResult<Profile> {
    let plane = kernel_plane(&s.frame);
    // Merge consecutive arcs of the same circle entity (a full circle split at
    // a seam becomes one edge, so a hole is one cylindrical face).
    let merge = |lp: &noshape_sketch::regions::ProfileLoop| -> Vec<(noshape_sketch::regions::ProfileSegment, u32)> {
        use noshape_sketch::regions::ProfileSegment as P;
        let mut out: Vec<(P, u32)> = Vec::new();
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
            out.push((seg.clone(), src.entity.0));
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
        out
    };
    let outer_m = merge(&region.outer);
    let holes_m: Vec<_> = region.holes.iter().map(merge).collect();
    let outer: Vec<k::Segment2d> = outer_m.iter().map(|(sg, _)| to_segment(sg)).collect();
    let holes: Vec<Vec<k::Segment2d>> = holes_m.iter().map(|h| h.iter().map(|(sg, _)| to_segment(sg)).collect()).collect();
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
