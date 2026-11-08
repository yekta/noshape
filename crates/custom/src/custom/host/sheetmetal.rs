//! `@updateSheetMetalGeometry` (std sheetMetalUtils.fs): the solid sheet
//! metal part rebuilt from its definition sheet.
//!
//! Live (dom/fs-smprobe{,2}-{1,2,3}.txt): the definition is a sheet body of
//! planar WALL faces meeting at sharp edges; a bend is the shared edge, with
//! a JOINT attribute (`jointType BEND`, inner `radius`, `angle`); the body
//! carries the MODEL attribute (`frontThickness` along the face normal,
//! `backThickness` against it). The part is every wall thickened, pulled
//! back from each bend by `R tan(θ/2)` (R: the definition surface's bend
//! radius, the inner radius plus the thickness on the concave side), and
//! the bends filled with the wall's thickness revolved about the bend
//! axis. Every part face, edge and vertex gets the association attribute of
//! the definition entity it comes from: walls → the cap faces, free edges →
//! the side faces and cap edges, bend edges → the bend cylinders and their
//! tangent lines, vertices → corner edges, bend end faces and vertices. The
//! definition sheet is hidden from then on ([`CtxState::hidden`]).

use super::query::{Ent, QueryCx, evaluate_refs};
use super::{CtxState, FsId, context_arg, convert::*, run_op, std_tag};
use crate::display::BodyKind;
use crate::reference::{BodyId, FeatureId, Reference};
use crate::regen::eval::{FResult, FeatureError};
use crate::regen::features::helpers_a::push_body;
use glam::DVec3;
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::Value;
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub const NATIVES: &[(&str, NativeFn)] = &[
    ("updateSheetMetalGeometry", update_sheet_metal_geometry),
    ("evSheetMetalFlatTransformation", ev_sheet_metal_flat_transformation),
    ("evSheetMetalBendUp", ev_sheet_metal_bend_up),
    ("evCornerType", ev_corner_type),
    ("evSheetMetalHoleToolBodies", ev_sheet_metal_hole_tool_bodies),
    ("evSheetMetalFormToolBodies", ev_sheet_metal_form_tool_bodies),
];

const TOL: f64 = 1e-7;

fn fe(m: impl Into<String>) -> FeatureError {
    FeatureError::new(m)
}

/// The SM attribute (`SMAttribute`) of an entity with the given object type.
pub(super) fn sm_attribute(st: &CtxState, r: &Reference, object_type: &str) -> Option<Value> {
    st.attributes_of(r).into_iter().find(|v| v.get("attribute").get("objectType").as_str().is_some_and(|t| &*t == object_type)).map(|v| v.get("attribute").clone())
}

/// `{ value : <length> }` fields of SM attributes, in meters.
fn length_field(attr: &Value, key: &str) -> f64 {
    length_of(attr.get(key).get("value")).unwrap_or(0.0)
}

/// The association attributes of an entity (copied to the part entities).
pub(super) fn associations(st: &CtxState, r: &Reference) -> Vec<Value> {
    let tag = std_tag("sheetMetalAttribute.fs", "SMAssociationAttribute");
    st.attributes_of(r).into_iter().filter(|v| v.get("attribute").tag() == tag).map(|v| v.get("attribute").clone()).collect()
}

/// A definition sheet to build: its entities and what std attached to them.
struct Definition {
    body: BodyId,
    front: f64,
    back: f64,
    /// Bend radius by definition edge index.
    bends: BTreeMap<usize, f64>,
    /// Rolled bends (a Hem's arc: a curved definition face with a BEND
    /// joint): inner radius by definition face index.
    rolled: BTreeMap<usize, f64>,
    /// Rip edges (definition edge indices) and the gap walls keep from them.
    rips: Vec<usize>,
    /// Rip styles other than EDGE (BUTT / BUTT2) by rip edge.
    rip_styles: BTreeMap<usize, String>,
    /// Each joint's attribute id (its pieces share it after a cut splits it):
    /// by edge, and by rolled face.
    joint_ids: JointIds,
    clearance: f64,
    /// Bend relief at the ends of a partial bend: style, depth scale (× bend
    /// radius) and width scale (× thickness), from the model attribute.
    relief: BendRelief,
    corner: CornerRelief,
    /// Corner features' settings by definition vertex (else `corner`).
    corners: BTreeMap<usize, CornerRelief>,
    /// Bend relief features' settings by the bend end's definition vertex
    /// (else `relief`).
    reliefs: BTreeMap<usize, BendRelief>,
    k_factor: f64,
    /// Active (not finished by Finish sheet metal model), default bend radius.
    active: bool,
    default_radius: f64,
    /// The parts built from it before (rebuilt in place).
    previous: Vec<BodyId>,
    /// Form tools registered on walls (std registerSheetMetalFormedTools).
    forms: Vec<FormTools>,
    /// The walls' WALL attribute ids by definition face ("<feature>.<n>",
    /// numbered in the order std made them: a Flange's pick order).
    wall_ids: BTreeMap<usize, String>,
    /// Corner breaks by definition vertex (std's attribute-based ones: the CORNER attribute's cornerBreaks).
    breaks: BTreeMap<usize, CornerBreak>,
}

/// A corner break at a free wall corner: a fillet of this radius or a chamfer of this distance.
#[derive(Clone, Copy, Debug)]
enum CornerBreak {
    Fillet(f64),
    Chamfer(f64),
}

/// The corner breaks on a definition body's vertices (`cornerBreaks` of their CORNER attributes: std
/// sheetMetalCornerBreakAttributeBased, the community corner overcut's fillet).
fn corner_breaks(st: &CtxState, q: &QueryCx, b: usize) -> BTreeMap<usize, CornerBreak> {
    let mut out = BTreeMap::new();
    for v in 0..st.ctx.bodies[b].vertices.len() {
        let Some(corner) = sm_attribute(st, &q.to_ref(&Ent::Vertex(b, v)), "CORNER") else { continue };
        let Some(first) = corner.get("cornerBreaks").as_array().and_then(|a| a.first().cloned()) else { continue };
        let brk = first.get("value");
        let Some(range) = length_of(brk.get("range")) else { continue };
        match enum_field(brk, "cornerBreakStyle").as_deref() {
            Some("FILLET") => out.insert(v, CornerBreak::Fillet(range)),
            Some("CHAMFER") => out.insert(v, CornerBreak::Chamfer(range)),
            _ => None,
        };
    }
    out
}

/// The part's corner breaks: at each broken definition vertex, the edge through the thickness filleted or chamfered;
/// the faces that makes come from the vertex.
fn apply_corner_breaks(part: &mut Shape, pieces: &mut Vec<Piece>, def: &Definition, topo: &Topo) {
    let t = def.front + def.back;
    let key = |f: &Shape| f.bounding_box(false).map(|b| (b.center(), b.diagonal())).ok();
    for (&v, brk) in &def.breaks {
        let Some(&p) = topo.vertices.get(v) else { continue };
        let through = |e: &Shape| {
            e.edge_info().is_ok_and(|i| {
                let d = (i.end - i.start).normalize_or_zero();
                let r = p - i.start;
                matches!(i.curve, k::Curve::Line { .. }) && (i.length - t).abs() < 1e-6 && (r - d * r.dot(d)).length() < 1e-6
            })
        };
        let edges: Vec<Shape> = part.edges().into_iter().filter(through).collect();
        let [edge] = edges.as_slice() else { continue };
        let before: Vec<_> = part.faces().iter().map(key).collect();
        let made = match brk {
            CornerBreak::Fillet(r) => k::fillet(part, std::slice::from_ref(edge), *r),
            CornerBreak::Chamfer(d) => k::chamfer(part, std::slice::from_ref(edge), k::ChamferSpec::Equal(*d), &[]),
        };
        let Ok(made) = made else { continue };
        *part = made.shape;
        for f in part.faces() {
            let kf = key(&f);
            if !before.iter().any(|b| matches!((b, kf), (Some(b), Some(kf)) if b.0.distance(kf.0) < 1e-9 && (b.1 - kf.1).abs() < 1e-9)) {
                pieces.push(Piece { face: f, from: Def::Vertex(v) });
            }
        }
    }
}

/// A definition body's bends (edge index → radius), rips (edge indices),
/// rolled bends (face index → radius) and butt rip styles (edge → style).
/// Bends (edge → radius), rips, rolled bends (face → radius), rip styles,
/// and each joint's attribute id (edges, then rolled faces).
type Joints = (BTreeMap<usize, f64>, Vec<usize>, BTreeMap<usize, f64>, BTreeMap<usize, String>, JointIds);
type JointIds = (BTreeMap<usize, Arc<str>>, BTreeMap<usize, Arc<str>>);

fn joints(st: &CtxState, q: &QueryCx, b: usize) -> Joints {
    let mut bends = BTreeMap::new();
    let mut rips = Vec::new();
    let mut styles = BTreeMap::new();
    let mut rolled = BTreeMap::new();
    let mut ids: JointIds = (BTreeMap::new(), BTreeMap::new());
    let id_of = |j: &Value| j.get("attributeId").as_str().map(|s| Arc::<str>::from(&*s));
    for f in 0..st.ctx.bodies[b].faces.len() {
        if let Some(j) = sm_attribute(st, &q.to_ref(&Ent::Face(b, f)), "JOINT")
            && j.get("jointType").get("value").as_str().as_deref() == Some("BEND")
        {
            rolled.insert(f, length_field(&j, "radius"));
            if let Some(id) = id_of(&j) {
                ids.1.insert(f, id);
            }
        }
    }
    for e in 0..st.ctx.bodies[b].edges.len() {
        let Some(j) = sm_attribute(st, &q.to_ref(&Ent::Edge(b, e)), "JOINT") else { continue };
        if let Some(id) = id_of(&j) {
            ids.0.insert(e, id);
        }
        match j.get("jointType").get("value").as_str().as_deref() {
            Some("BEND") => {
                bends.insert(e, length_field(&j, "radius"));
            }
            Some("RIP") => {
                rips.push(e);
                if let Some(style) = enum_field(&j, "jointStyle").filter(|s| s != "EDGE") {
                    styles.insert(e, style);
                }
            }
            _ => {}
        }
    }
    (bends, rips, rolled, styles, ids)
}

/// An enum field of an SM attribute (stored bare: `defaultBendReliefStyle : OBROUND`).
fn enum_field(attr: &Value, key: &str) -> Option<String> {
    let v = attr.get(key);
    v.as_str().or_else(|| v.get("value").as_str()).map(|s| s.to_string())
}

/// The model's bend relief settings (live dom/fs-smprobe4-5{b,c,d}.txt):
/// a slot `width × thickness` wide beside the bend, straight for
/// `depth × bend radius` from the edge, ended by a half circle (OBROUND) or
/// squared off half a width further (RECTANGLE); TEAR cuts nothing.
#[derive(Clone, Debug)]
struct BendRelief {
    style: String,
    depth: f64,
    width: f64,
    /// SIZED_RECTANGLE / SIZED_OBROUND (a Bend relief feature): the straight
    /// depth itself, not a scale.
    sized_depth: Option<f64>,
}

/// The model's corner relief where two bends meet (live
/// dom/fs-smprobe4-6{d,e,f,g}.txt): in the flat, a square or circle on the
/// bends' bend-allowance square BA1 × BA2. RECTANGLE: sides `scale` × BA;
/// SIZED_RECTANGLE: sides `width`; ROUND: radius `scale` × the square's
/// half diagonal; SIZED_ROUND: radius `diameter` / 2 (smaller than the
/// half diagonal: not applied, "A corner relief is too small").
#[derive(Clone, Debug)]
struct CornerRelief {
    style: String,
    /// The model's style where three bends meet (std getDefaultThreeCornerStyle:
    /// Closed falls back to Simple there); `style` is the two-bend corners'.
    three: String,
    scale: f64,
    width: f64,
    diameter: f64,
}

impl CornerRelief {
    /// A Corner feature's attribute (std sheetMetalCorner.fs), its unset
    /// sizes the model's.
    fn of_corner(attr: &Value, model: &CornerRelief) -> CornerRelief {
        let len = |k: &str, d: f64| attr.get(k).get("value").as_number().or_else(|| length_of(attr.get(k).get("value"))).unwrap_or(d);
        let style = enum_field(attr, "cornerStyle").unwrap_or_else(|| model.style.clone());
        CornerRelief {
            three: style.clone(),
            style,
            scale: attr.get("cornerReliefScale").get("value").as_number().unwrap_or(model.scale),
            width: len("squareReliefWidth", model.width),
            diameter: len("roundReliefDiameter", model.diameter),
        }
    }

    fn of(model: &Value) -> CornerRelief {
        let three = enum_field(model, "defaultThreeCornerStyle").unwrap_or_else(|| "SIMPLE".to_string());
        CornerRelief {
            // Two-bend corners (std sheetMetalStart getDefaultTwoCornerStyle:
            // a model's Closed corners; harness sm6h).
            style: enum_field(model, "defaultTwoCornerStyle").unwrap_or_else(|| three.clone()),
            three,
            scale: model.get("defaultCornerReliefScale").get("value").as_number().unwrap_or(1.5),
            width: length_field(model, "defaultSquareReliefWidth"),
            diameter: length_field(model, "defaultRoundReliefDiameter"),
        }
    }
}

impl BendRelief {
    fn of(model: &Value) -> BendRelief {
        let num = |k: &str, d: f64| model.get(k).get("value").as_number().unwrap_or(d);
        BendRelief {
            style: enum_field(model, "defaultBendReliefStyle").unwrap_or_else(|| "OBROUND".to_string()),
            depth: num("defaultBendReliefDepthScale", 2.0),
            width: num("defaultBendReliefScale", 1.0625),
            sized_depth: None,
        }
    }

    /// A Bend relief feature's corner attribute at a bend end (std
    /// sheetMetalBendRelief.fs createNewCornerAttribute), its unset sizes the
    /// model's; `None` when the attribute is a corner's (not a relief style).
    fn of_vertex(attr: &Value, model: &BendRelief) -> Option<BendRelief> {
        let style = enum_field(attr, "cornerStyle")?;
        if !matches!(style.as_str(), "OBROUND" | "RECTANGLE" | "TEAR" | "SIZED_OBROUND" | "SIZED_RECTANGLE") {
            return None;
        }
        let num = |k: &str, d: f64| attr.get(k).get("value").as_number().unwrap_or(d);
        let sized = style.starts_with("SIZED_");
        Some(BendRelief {
            depth: num("bendReliefDepthScale", model.depth),
            width: num("bendReliefScale", model.width),
            sized_depth: if sized { length_of(attr.get("bendReliefDepth").get("value")).or(Some(0.0)) } else { None },
            style,
        })
    }
}

/// Corner features' settings on a definition body's vertices.
fn relief_overrides(st: &CtxState, q: &QueryCx, b: usize, model: &BendRelief) -> BTreeMap<usize, BendRelief> {
    (0..st.ctx.bodies[b].vertices.len()).filter_map(|v| BendRelief::of_vertex(&sm_attribute(st, &q.to_ref(&Ent::Vertex(b, v)), "CORNER")?, model).map(|r| (v, r))).collect()
}

fn corner_overrides(st: &CtxState, q: &QueryCx, b: usize, model: &CornerRelief) -> BTreeMap<usize, CornerRelief> {
    (0..st.ctx.bodies[b].vertices.len()).filter_map(|v| sm_attribute(st, &q.to_ref(&Ent::Vertex(b, v)), "CORNER").map(|a| (v, CornerRelief::of_corner(&a, model)))).collect()
}

/// What one part entity comes from: a definition face, edge or vertex.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Def {
    Face(usize),
    Edge(usize),
    Vertex(usize),
}

/// A construction piece's face and the definition entity it comes from.
/// A form registered on a wall (definition face `face`): the parts to add
/// and to remove (hidden bodies kept with the model).
struct FormTools {
    face: usize,
    add: Vec<BodyId>,
    remove: Vec<BodyId>,
    /// A cutting tool (a hole), not a form: cut out of the flat too, no form outline.
    cut: bool,
    /// Its flat sketches (the form studio Tag's flatFormSketch, placed with the tools).
    sketches: Vec<FeatureId>,
}

/// The body ids a wall attribute's `formedToolBodyIds` name, by role, and every
/// transient id in them with its entity.
fn form_tools(st: &CtxState, wall: &Value, face: usize, ids: &mut Vec<(Arc<str>, Reference)>) -> Vec<FormTools> {
    let mut out = Vec::new();
    for item in wall.get("formedToolBodyIds").as_array().iter().flat_map(|a| a.iter()) {
        let mut body = |v: &Value| -> Option<BodyId> {
            let s = v.as_str()?;
            let r = st.transient_ref(&s)?.clone();
            ids.push((s, r.clone()));
            if let Reference::Body(b) = r { Some(b) } else { None }
        };
        let add = body(item.get("positiveBodyId")).into_iter().collect();
        let remove = body(item.get("negativeBodyId")).into_iter().collect();
        body(item.get("placementBodyId"));
        let mut sketches = Vec::new();
        for sk in item.get("sketchBodyIds").as_array().iter().flat_map(|a| a.iter()) {
            body(sk);
            if let Some(Reference::Sketch(f)) = sk.as_str().and_then(|s| st.transient_ref(&s).cloned()) {
                sketches.push(f);
            }
        }
        out.push(FormTools { face, add, remove, cut: false, sketches });
    }
    // Tools std registered to cut the wall (registerSheetMetalBooleanTools:
    // a Hole through it, counterbores too).
    for item in wall.get("cuttingToolBodyIds").as_array().iter().flat_map(|a| a.iter()) {
        let Some(s) = item.as_str() else { continue };
        let Some(r) = st.transient_ref(&s).cloned() else { continue };
        ids.push((s, r.clone()));
        if let Reference::Body(b) = r {
            out.push(FormTools { face, add: Vec::new(), remove: vec![b], cut: true, sketches: Vec::new() });
        }
    }
    out
}

/// `v` with the strings `map` renames replaced (a wall attribute's tool ids).
fn rename_strings(v: &Value, map: &HashMap<Arc<str>, Arc<str>>) -> Value {
    match v {
        Value::Str(s, t) => match map.get(s) {
            Some(n) => Value::Str(n.clone(), *t),
            None => v.clone(),
        },
        Value::Array(a, t) => Value::Array(Arc::new(a.iter().map(|x| rename_strings(x, map)).collect()), *t),
        Value::Map(m, t) => Value::Map(Arc::new(m.iter().map(|(k, x)| (k.clone(), rename_strings(x, map))).collect()), *t),
        _ => v.clone(),
    }
}

struct Piece {
    face: Shape,
    from: Def,
}

/// One wall's outer loop as straight edges: points and the definition
/// edge index of each side (`side[k]` runs from `pts[k]` to `pts[k + 1]`).
struct Loop {
    pts: Vec<DVec3>,
    side: Vec<usize>,
    /// A chord of a curved side (an arc a cut left across a flange): the
    /// wall itself is then built from its face ([`pulled_face`]).
    curved: Vec<bool>,
}

/// A face's outer wire: the one spanning the most.
fn outer_wire(face: &Shape) -> Option<Shape> {
    let size = |w: &Shape| w.bounding_box(false).map_or(0.0, |b| b.diagonal());
    face.wires().into_iter().max_by(|a, b| size(a).total_cmp(&size(b)))
}

/// A wall's holes (a cut std made in the definition sheet: an Extrude or
/// Hole through an active sheet metal part): each inner wire as a face on
/// the definition plane, and its edges (definition edge indices).
fn wall_holes(face: &Shape, topo: &Topo) -> FResult<Vec<(Shape, Vec<usize>)>> {
    let Some(outer) = outer_wire(face) else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for w in face.wires().into_iter().filter(|w| !w.is_same(&outer)) {
        let edges = w.edges().iter().filter_map(|e| topo.edge_of(e)).collect();
        out.push((k::make_face_from_wire(&w, true).map_err(|e| fe(e.to_string()))?, edges));
    }
    Ok(out)
}

/// Definition topology: faces, edges (with their points and faces) and vertices.
struct Topo {
    faces: Vec<Shape>,
    edges: Vec<Shape>,
    edge_faces: Vec<Vec<u32>>,
    edge_ends: Vec<(DVec3, DVec3)>,
    vertices: Vec<DVec3>,
    /// Edge indices by [`Shape::hash_code`] (a sheet of a thousand holes: no search of every edge per edge).
    edge_index: HashMap<u64, Vec<usize>>,
}

impl Topo {
    fn of(shape: &Shape) -> FResult<Topo> {
        let faces = shape.faces();
        let edges = shape.edges();
        let edge_faces = shape.edge_faces().map_err(|e| fe(e.to_string()))?;
        let edge_ends = edges.iter().map(|e| e.endpoints().unwrap_or_default()).collect();
        let vertices = shape.vertices().iter().filter_map(|v| v.vertex_point().ok()).collect();
        let mut edge_index: HashMap<u64, Vec<usize>> = HashMap::new();
        for (x, e) in edges.iter().enumerate() {
            edge_index.entry(e.hash_code()).or_default().push(x);
        }
        Ok(Topo { faces, edges, edge_faces, edge_ends, vertices, edge_index })
    }

    /// The definition edge that is `e` (OCCT IsSame).
    fn edge_of(&self, e: &Shape) -> Option<usize> {
        self.edge_index.get(&e.hash_code())?.iter().copied().find(|x| self.edges[*x].is_same(e))
    }

    fn nearest_vertex(&self, p: DVec3) -> usize {
        (0..self.vertices.len()).min_by(|a, b| self.vertices[*a].distance(p).total_cmp(&self.vertices[*b].distance(p))).unwrap_or(0)
    }

    /// The definition vertex shared by two edges, if any.
    fn shared_vertex(&self, a: usize, b: usize) -> Option<usize> {
        let (a0, a1) = self.edge_ends[a];
        let (b0, b1) = self.edge_ends[b];
        [a0, a1].into_iter().find(|p| p.distance(b0) < TOL || p.distance(b1) < TOL).map(|p| self.nearest_vertex(p))
    }
}

/// A face's outer loop (straight edges only), chained end to end.
fn wall_loop(face: &Shape, topo: &Topo) -> FResult<Loop> {
    let wire = outer_wire(face).ok_or_else(|| fe("A sheet metal wall has no boundary"))?;
    let mut segs: Vec<(DVec3, DVec3, usize, bool)> = Vec::new();
    for e in wire.edges() {
        let info = e.edge_info().map_err(|x| fe(x.to_string()))?;
        let idx = topo.edge_of(&e).ok_or_else(|| fe("Sheet metal wall edge not found"))?;
        if matches!(info.curve, k::Curve::Line { .. }) {
            segs.push((info.start, info.end, idx, false));
        } else {
            // Two chords through its middle.
            let mid = e.point_at(info.first_param + (info.last_param - info.first_param) * 0.5).map_err(|x| fe(x.to_string()))?;
            segs.push((info.start, mid, idx, true));
            segs.push((mid, info.end, idx, true));
        }
    }
    if segs.len() < 3 {
        return Err(fe("A sheet metal wall needs at least three edges"));
    }
    // Chain the sides: each starts where the previous one ended.
    let (first_start, mut at, first_idx, first_curved) = segs.swap_remove(0);
    let mut pts = vec![first_start];
    let mut side = vec![first_idx];
    let mut curved = vec![first_curved];
    while !segs.is_empty() {
        let Some(k) = segs.iter().position(|(a, b, _, _)| a.distance(at) < 1e-7 || b.distance(at) < 1e-7) else {
            return Err(fe("A sheet metal wall boundary is not closed"));
        };
        let (a, b, idx, c) = segs.swap_remove(k);
        let next = if a.distance(at) < 1e-7 { b } else { a };
        pts.push(at);
        side.push(idx);
        curved.push(c);
        at = next;
    }
    // Counter-clockwise about the face's outward normal, as the face's own
    // loop runs (the edges come out each its curve's way, so the chain can
    // start backwards).
    let (_, n) = face_normal(face)?;
    let m = pts.len();
    let turn: DVec3 = (0..m).map(|k| pts[k].cross(pts[(k + 1) % m])).sum();
    if turn.dot(n) < 0.0 {
        pts[1..].reverse();
        side.reverse();
        curved.reverse();
    }
    Ok(Loop { pts, side, curved })
}

/// The outward (oriented) normal of a planar face.
fn face_normal(face: &Shape) -> FResult<(DVec3, DVec3)> {
    let [u0, u1, v0, v1] = face.uv_bounds().map_err(|e| fe(e.to_string()))?;
    let (u, v) = ((u0 + u1) * 0.5, (v0 + v1) * 0.5);
    let p = face.point_at_uv(u, v).map_err(|e| fe(e.to_string()))?;
    let n = face.normal_at_uv(u, v).map_err(|e| fe(e.to_string()))?.normalize();
    Ok((p, n))
}

/// The bend between two walls along a definition edge.
struct Bend {
    /// Edge start and unit direction, length.
    start: DVec3,
    dir: DVec3,
    len: f64,
    /// Bend angle (0 for flat).
    angle: f64,
    /// Pull-back of each wall from the edge (definition surface).
    setback: f64,
    axis_point: DVec3,
    /// Its radius at the definition surface.
    def_radius: f64,
}

/// A planar wall's in-plane direction away from its edge along `dir` at `at` (`n` its normal, `c` its middle): the side
/// a small step stays on the wall (an edge of a hole has the middle on its other side: a louver's flange on its slot),
/// else towards the middle.
fn wall_side(face: &Shape, n: DVec3, dir: DVec3, at: DVec3, c: DVec3) -> DVec3 {
    let a = n.cross(dir).normalize();
    let stays = |v: DVec3| face.contains_point(at + v * 1e-5, 1e-6).ok();
    match (stays(a), stays(-a)) {
        (Some(false), Some(true)) => -a,
        (Some(true), Some(false)) => a,
        _ if (c - at).dot(a) < 0.0 => -a,
        _ => a,
    }
}

/// Geometry of the bend at edge `e` between walls `fa` (seen from `a`) and `fb`.
fn bend_geometry(topo: &Topo, e: usize, fa: usize, fb: usize, radius: f64, front: f64, back: f64) -> FResult<Bend> {
    let (s, t) = topo.edge_ends[e];
    let len = s.distance(t);
    let dir = (t - s) / len.max(1e-300);
    let into = |f: usize| -> FResult<(DVec3, DVec3)> {
        let (p, n_mid) = face_normal(&topo.faces[f])?;
        // A curved (rolled) wall's normal where it meets the bend, not at its middle (live 2031: the angle the
        // bend attribute holds beside a rolled wall).
        let face = &topo.faces[f];
        let joint = (s + t) * 0.5;
        let n = face.normal_at_point(joint).map(|n| n.normalize()).unwrap_or(n_mid);
        let mut a = n.cross(dir).normalize();
        if face.face_plane().ok().flatten().is_some() {
            a = wall_side(face, n, dir, joint, p);
        } else {
            // A curved wall's middle may lie behind the joint (rolled far round): inward is the side a small step
            // stays on the face.
            // (The face classifier first: a projection of a point off a rolled wall takes ~2.5 ms.)
            let stays = |v: DVec3| face.contains_point(joint + v * 1e-5, 1e-6).ok();
            let off = |v: DVec3| face.project_point(joint + v * 1e-5).map_or(f64::INFINITY, |pr| pr.point.distance(joint + v * 1e-5));
            match (stays(a), stays(-a)) {
                (Some(false), Some(true)) => a = -a,
                (Some(true), Some(false)) => {}
                _ if off(a) > off(-a) => a = -a,
                _ => {}
            }
        }
        Ok((a, n))
    };
    let (a1, n1) = into(fa)?;
    let (a2, _) = into(fb)?;
    let angle = std::f64::consts::PI - a1.dot(a2).clamp(-1.0, 1.0).acos();
    // The concave side of wall a: towards wall b.
    let concave_front = n1.dot(a2) > 0.0;
    let m1 = if concave_front { n1 } else { -n1 };
    let def_radius = radius + if concave_front { front } else { back };
    let setback = def_radius * (angle * 0.5).tan();
    let axis_point = s + a1 * setback + m1 * def_radius;
    Ok(Bend { start: s, dir, len, angle, setback, axis_point, def_radius })
}

/// A wall with curved sides (a cut's arc across a flange) as its definition
/// face less a strip along each pulled side. A strip runs on past an end
/// whose neighbour is a free side across it (the face's outline stops it),
/// not past a joint or a free side running on in line (as [`pulled_sides`]).
fn pulled_face(face: &Shape, l: &Loop, n: DVec3, offset: &[f64], free: impl Fn(usize) -> bool) -> FResult<Shape> {
    let wire = outer_wire(face).ok_or_else(|| fe("A sheet metal wall has no boundary"))?;
    let base = on_definition(face, &wire, DVec3::ZERO)?;
    let big = face.bounding_box(false).map_err(|e| fe(e.to_string()))?.diagonal() * 2.0;
    let m = l.pts.len();
    let mut strips = Vec::new();
    for (k, &off) in offset.iter().enumerate().take(m) {
        if off <= TOL || l.curved[k] {
            continue;
        }
        let (a, b) = (l.pts[k], l.pts[(k + 1) % m]);
        let len = a.distance(b);
        let d = (b - a) / len;
        let u = inward(l, n, k);
        let in_line = |j: usize| (l.pts[(j + 1) % m] - l.pts[j]).normalize_or_zero().cross(d).length() <= 1e-6;
        // A free side across shorter than the pull, to a free side running on in line, doesn't stop the strip: it
        // ends at the bend's end (c2-sm-louver's Flange 3: a 0.275 mm step to the tab, the tab kept whole).
        let runs_on = |j: usize, beyond: usize| {
            free(j) && (l.curved[j] || (!in_line(j) && !(l.pts[(j + 1) % m].distance(l.pts[j]) < off && free(beyond) && !l.curved[beyond] && in_line(beyond))))
        };
        let (s0, s1) = (if runs_on((k + m - 1) % m, (k + m - 2) % m) { -big } else { 0.0 }, if runs_on((k + 1) % m, (k + 2) % m) { len + big } else { len });
        let quad = [a + d * s0 - u * big, a + d * s1 - u * big, a + d * s1 + u * off, a + d * s0 + u * off];
        let w = k::make_polyline(&quad, true).map_err(|e| fe(e.to_string()))?;
        strips.push(k::make_face_from_wire(&w, true).map_err(|e| fe(e.to_string()))?);
    }
    if strips.is_empty() {
        return Ok(base);
    }
    Ok(k::boolean(k::BooleanOp::Cut, &[base], &strips, &k::BooleanOptions::default()).map_err(|e| fe(format!("Sheet metal wall failed: {e}")))?.shape)
}

/// A face bounded by `wire` on definition face `face`'s plane moved by
/// `shift`, with that plane's axes: a sketch on the wall's faces gets the
/// definition's frame (live request 1619: a sketch on a sheet metal plate's
/// top has x = +X, as on an extruded plate's; a plane fitted to the outline
/// turned it). Any other face: the outline's own.
fn on_definition(face: &Shape, wire: &Shape, shift: DVec3) -> FResult<Shape> {
    match face.face_surface() {
        Ok(k::Surface::Plane(pl)) => k::make_face(&k::Plane::new(pl.origin + shift, pl.normal, pl.x_dir), wire, &[]),
        _ => k::make_face_from_wire(wire, true),
    }
    .map_err(|e| fe(e.to_string()))
}

/// Where pulled side `k`'s line runs inside the pulled face `pf`: the piece
/// over the side's own run, from its start's end on (as [`pulled_sides`]).
fn run_inside(pf: &Shape, l: &Loop, n: DVec3, k: usize, offset: f64) -> Option<(DVec3, DVec3)> {
    let m = l.pts.len();
    let (a, b) = (l.pts[k], l.pts[(k + 1) % m]);
    let d = (b - a).normalize_or_zero();
    let u = inward(l, n, k);
    let o = a + u * offset;
    let plane = k::Plane::new(o, u, d);
    let len = a.distance(b);
    k::section_wires(pf, &plane)
        .ok()?
        .iter()
        .flat_map(|w| w.edges())
        .filter_map(|e| {
            let i = e.edge_info().ok()?;
            let (t0, t1) = ((i.start - o).dot(d), (i.end - o).dot(d));
            let (t0, t1) = (t0.min(t1), t0.max(t1));
            let overlap = t1.min(len) - t0.max(0.0);
            (overlap > TOL).then_some((overlap, o + d * t0, o + d * t1))
        })
        .max_by(|x, y| x.0.total_cmp(&y.0))
        .map(|(_, p, q)| (p, q))
}

/// Pull the loop's sides in by `offset[k]` (0 for free edges), in the wall
/// plane: each side's pulled segment. Collinear sides pulled by different
/// amounts (a partial bend beside a free edge) end in a step. A side the
/// pulls of its neighbours pass over (it comes out reversed: a short free
/// side left at a corner by an earlier flange, pulled with the bend beside
/// it, next to a bigger bend's pull) is gone: its neighbours meet (a point
/// segment).
fn pulled_sides(l: &Loop, n: DVec3, offset: &[f64]) -> Vec<(DVec3, DVec3)> {
    let m = l.pts.len();
    let line = |k: usize| {
        let d = (l.pts[(k + 1) % m] - l.pts[k]).normalize();
        (l.pts[k] + inward(l, n, k) * offset[k], d)
    };
    let mut active = vec![true; m];
    loop {
        let on: Vec<usize> = (0..m).filter(|k| active[*k]).collect();
        if on.len() < 3 {
            break;
        }
        // Each active side's start (on the previous active side's line) and end.
        let corner = |a: usize, b: usize, v: DVec3| -> (DVec3, DVec3) {
            let ((p0, d0), (p1, d1)) = (line(a), line(b));
            let c = d0.cross(d1);
            if c.length_squared() < 1e-20 {
                return (p0 + d0 * (v - p0).dot(d0), p1 + d1 * (v - p1).dot(d1));
            }
            let x = p0 + d0 * ((p1 - p0).cross(d1).dot(c) / c.length_squared());
            (x, x)
        };
        let mut seg = vec![(DVec3::ZERO, DVec3::ZERO); m];
        for (i, &k) in on.iter().enumerate() {
            let prev = on[(i + on.len() - 1) % on.len()];
            let next = on[(i + 1) % on.len()];
            seg[k] = (corner(prev, k, l.pts[k]).1, corner(k, next, l.pts[next]).0);
        }
        let reversed = on.iter().copied().find(|&k| (seg[k].1 - seg[k].0).dot(line(k).1) < -1e-9);
        match reversed {
            Some(k) => active[k] = false,
            None => {
                // Gone sides: a point where their neighbours meet.
                for k in 0..m {
                    if !active[k] {
                        let next = (1..m).map(|j| (k + j) % m).find(|j| active[*j]).unwrap_or(k);
                        seg[k] = (seg[next].0, seg[next].0);
                    }
                }
                return seg;
            }
        }
    }
    // Degenerate (fewer than three sides left): as pulled, unclipped.
    (0..m).map(|k| (line(k).0, line(k).0 + line(k).1 * (l.pts[(k + 1) % m] - l.pts[k]).length())).collect()
}

/// The direction into the wall from side `k` of its loop.
fn inward(l: &Loop, n: DVec3, k: usize) -> DVec3 {
    let m = l.pts.len();
    let area: DVec3 = (0..m).map(|i| l.pts[i].cross(l.pts[(i + 1) % m])).sum();
    let d = (l.pts[(k + 1) % m] - l.pts[k]).normalize();
    if area.dot(n) > 0.0 { n.cross(d) } else { d.cross(n) }
}

/// The outline through the pulled sides (a step adds a corner).
fn outline_points(sides: &[(DVec3, DVec3)]) -> Vec<DVec3> {
    let mut out: Vec<DVec3> = Vec::new();
    for side in sides {
        for p in [side.0, side.1] {
            if out.last().is_none_or(|q: &DVec3| q.distance(p) > TOL) {
                out.push(p);
            }
        }
    }
    if out.len() > 1 && out[0].distance(out[out.len() - 1]) <= TOL {
        out.pop();
    }
    out
}

/// Bend relief cutters of one wall: at each end of a bend side that runs on
/// along a free side, a slot into the wall on the free side (see [`BendRelief`]).
fn relief_tools(def: &Definition, topo: &Topo, fi: usize, l: &Loop, n: DVec3, thickness: f64) -> FResult<Vec<(Shape, usize)>> {
    let mut out = Vec::new();
    let m = l.pts.len();
    for k in 0..m {
        let Some(&radius) = def.bends.get(&l.side[k]) else { continue };
        let dir = (l.pts[(k + 1) % m] - l.pts[k]).normalize();
        let into = inward(l, n, k);
        for (j, v, away) in [((k + m - 1) % m, l.pts[k], -dir), ((k + 1) % m, l.pts[(k + 1) % m], dir)] {
            let e = l.side[j];
            let free = !def.bends.contains_key(&e) && !def.rips.contains(&e);
            let along = (l.pts[(j + 1) % m] - l.pts[j]).normalize();
            if !free {
                continue;
            }
            // The free edge the bend runs on along: next to it, or past a
            // short connector when the bend edge was moved in (INNER flange
            // alignment; live sm5: the slot starts on the plate's edge line).
            let bend_end = v;
            let (v, moved_in) = if along.cross(dir).length() <= 1e-9 {
                (v, 0.0)
            } else {
                let (next, base) = if j == (k + m - 1) % m { ((j + m - 1) % m, l.pts[j]) } else { ((j + 1) % m, l.pts[(j + 1) % m]) };
                let e2 = l.side[next];
                let along2 = (l.pts[(next + 1) % m] - l.pts[next]).normalize();
                // Only a connector as short as the move (not a wall's side
                // to a tab running parallel to the bend: qa5 Tab on G34).
                let connector = (l.pts[(j + 1) % m] - l.pts[j]).length();
                // (No longer than the bend radius: c2-sm-louver's Flange 3 step of 0.275 mm is relieved, Flange 4's 2.286 mm
                // one to the narrower tab isn't, R 1.628.)
                if def.bends.contains_key(&e2) || def.rips.contains(&e2) || along2.cross(dir).length() > 1e-9 || connector > radius + TOL {
                    continue;
                }
                // How far the bend's edge lies in from the free side (a bend edge moved in by Outer alignment: live
                // request 1599 (a), y 40 → 32 for an edge moved from 40 to 38, depth 6; negative where the free side
                // steps in to a tab, c2-sm-louver's Flange 3).
                (base, -(base - l.pts[k]).dot(into))
            };
            // Another joint starting at the bend's end, or within its bend
            // zone (the plate's edge to a Hem's arc, which starts where the
            // bend's pull-back ends), makes it a corner, not a partial bend:
            // no bend relief (live qa5 Hem on G34: the front flange keeps its
            // full end, 9378.1835 mm³).
            if joint_near(topo, v, l.side[k], radius + thickness) {
                continue;
            }
            // The bend end's own relief (a Bend relief feature), else the model's.
            let vi = topo.nearest_vertex(bend_end);
            let relief = def.reliefs.get(&vi).unwrap_or(&def.relief);
            if relief.style == "TEAR" {
                continue;
            }
            let w = relief.width * thickness;
            // A scaled relief's round is centred depth × R × tan(angle / 2) in from the bend's edge (live 2254: d × R
            // at 90°, on the tangent line at depth 1 for c2-sm-louver's 51.5° Flange 3; past 90° not seen, d × R kept);
            // a sized one's depth runs from the free side.
            let straight = match relief.sized_depth {
                Some(d) => d + moved_in.max(0.0),
                None => {
                    let half = bend_angle(def, topo, fi, l.side[k], radius)?.min(std::f64::consts::FRAC_PI_2) * 0.5;
                    moved_in + relief.depth * radius * half.tan()
                }
            };
            let v = v + away * bend_side_back(def, topo, fi, l.side[k], bend_end, away, radius)?;
            let depth = if relief.style.ends_with("RECTANGLE") { straight + w * 0.5 } else { straight };
            // Through the whole thickness and a little more.
            let base = v - n * (def.back + thickness);
            if depth > TOL {
                let quad = [base, base + away * w, base + away * w + into * depth, base + into * depth];
                let wire = k::make_polyline(&quad, true).map_err(|e| fe(e.to_string()))?;
                let face = k::make_face_from_wire(&wire, true).map_err(|e| fe(e.to_string()))?;
                out.push((k::extrude(&face, n * thickness * 3.0).map_err(|e| fe(e.to_string()))?.shape, vi));
            }
            if relief.style.ends_with("OBROUND") {
                let frame = k::Plane::new(base + away * (w * 0.5) + into * depth, n, away);
                out.push((k::make_cylinder(&frame, w * 0.5, thickness * 3.0, std::f64::consts::TAU).map_err(|e| fe(e.to_string()))?, vi));
            }
        }
    }
    Ok(out)
}

/// Bend `bend`'s angle beside wall `fi` (90° when it has no other wall).
fn bend_angle(def: &Definition, topo: &Topo, fi: usize, bend: usize, radius: f64) -> FResult<f64> {
    let Some(&other) = topo.edge_faces[bend].iter().find(|&&f| f as usize != fi) else { return Ok(std::f64::consts::FRAC_PI_2) };
    Ok(bend_geometry(topo, bend, fi, other as usize, radius, def.front, def.back)?.angle)
}

/// How far the relief at bend `bend`'s end `v` on wall `fi` starts back along the bend (`away`: out of the bend):
/// where the other wall's free side, leaving `v` slanted back over the bend, crosses the bend's setback line, so the
/// slot clears the bend's side (live 2053: Shaped Flange's tapered flange, its relief from y 48.14, not the bend's
/// end at 48.5). 0 when that side runs square or outwards.
fn bend_side_back(def: &Definition, topo: &Topo, fi: usize, bend: usize, v: DVec3, away: DVec3, radius: f64) -> FResult<f64> {
    let Some(&other) = topo.edge_faces[bend].iter().find(|&&f| f as usize != fi) else { return Ok(0.0) };
    let other = other as usize;
    let side = (0..topo.edges.len()).find_map(|x| {
        if x == bend || topo.edge_faces[x].as_slice() != [other as u32] {
            return None;
        }
        let (a, b) = topo.edge_ends[x];
        if a.distance(v) < TOL {
            Some(b)
        } else if b.distance(v) < TOL {
            Some(a)
        } else {
            None
        }
    });
    let Some(end) = side else { return Ok(0.0) };
    let s = (end - v).normalize_or_zero();
    let along = s.dot(away);
    let across = s - away * along;
    if along >= -1e-9 || across.length() < 1e-9 {
        return Ok(0.0);
    }
    let g = bend_geometry(topo, bend, fi, other, radius, def.front, def.back)?;
    Ok(g.setback / across.length() * along)
}

/// Whether a joint other than bend edge `bend` starts at `v` or within
/// `zone` of it (a corner where the bend ends: no bend relief, and a free
/// side running on from the bend is pulled back with it).
fn joint_near(topo: &Topo, v: DVec3, bend: usize, zone: f64) -> bool {
    (0..topo.edges.len()).any(|x| {
        x != bend && topo.edge_faces[x].len() == 2 && {
            let (a, b) = topo.edge_ends[x];
            a.distance(v) < zone + TOL || b.distance(v) < zone + TOL
        }
    })
}

/// A wall's pulled outline: its loop, normal and pulled side segments.
type Outline = Option<(Loop, DVec3, Vec<(DVec3, DVec3)>)>;

/// Bend clips by corner reliefs: (bend edge, vertex, distance from it).
type Clips = Vec<(usize, DVec3, f64)>;
/// Relief cutters by wall: (tool, definition vertex its faces come from).
type Cuts = BTreeMap<usize, Vec<(Shape, usize)>>;
/// The bends being built: (edge, wall a, wall b, geometry).
type BendList = [(usize, usize, usize, Bend)];

/// A corner cut across a bend, drawn in the flat and wrapped onto the bend:
/// what lies between `range(w)` (distances from the corner vertex `v` along
/// the bend) at flat distance `w` across the bend allowance `ba`, measured
/// from the side of wall `plate`.
struct WrappedCut {
    edge: usize,
    v: DVec3,
    plate: usize,
    ba: f64,
    shape: CutShape,
}

enum CutShape {
    /// A round relief: the circle centred `along` from the vertex, mid-allowance.
    Round { along: f64, radius: f64 },
    /// A closed corner (live G34a): the bend of `angle` runs to the
    /// square's diagonal, from `plate_end` at the plate's side to
    /// `flange_end` at the flange's, by the sine of the angle bent so far
    /// (live's bend cylinder areas: 186.1572 / 372.3144 mm²).
    Closed { plate_end: f64, flange_end: f64, angle: f64 },
    /// A closed corner whose rip is a butt joint (live 1670 / 1676): in the
    /// flat, one cubic Bézier from `plate_end` on the plate's side (45°,
    /// handle half the chord) to `flange_end` on the flange's (along the
    /// wall's end, handle a tenth of the chord).
    Butt { plate_end: f64, flange_end: f64 },
}

/// The flat Bézier of [`CutShape::Butt`] in (along the bend from the
/// corner, across it from the plate) for a bend allowance `ba`.
fn butt_bezier(plate_end: f64, flange_end: f64, ba: f64) -> [glam::DVec2; 4] {
    use glam::DVec2;
    let (p0, p3) = (DVec2::new(plate_end, 0.0), DVec2::new(flange_end, ba));
    let chord = (p3 - p0).length();
    [p0, p0 + DVec2::new(-1.0, 1.0).normalize() * 0.5 * chord, p3 - DVec2::Y * 0.1 * chord, p3]
}

/// Where a [`butt_bezier`] runs `w` across the bend (along the bend).
fn butt_along(c: &[glam::DVec2; 4], w: f64) -> f64 {
    let at = |t: f64| {
        let u = 1.0 - t;
        c[0] * (u * u * u) + c[1] * (3.0 * u * u * t) + c[2] * (3.0 * u * t * t) + c[3] * (t * t * t)
    };
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if at(mid).y < w {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    at(0.5 * (lo + hi)).x
}

impl WrappedCut {
    fn range(&self, w: f64, reach: f64) -> (f64, f64) {
        match self.shape {
            CutShape::Round { along, radius } => {
                let half = (radius * radius - (w - self.ba * 0.5).powi(2)).max(0.0).sqrt();
                (along - half, along + half)
            }
            CutShape::Closed { plate_end, flange_end, angle } => {
                let t = (w / self.ba).clamp(0.0, 1.0);
                let f = if angle <= std::f64::consts::FRAC_PI_2 + 1e-9 { (t * angle).sin() / angle.sin() } else { t };
                (-reach, plate_end + (flange_end - plate_end) * f)
            }
            CutShape::Butt { plate_end, flange_end } => (-reach - plate_end.abs() - flange_end.abs(), butt_along(&butt_bezier(plate_end, flange_end, self.ba), w.clamp(0.0, self.ba))),
        }
    }
}

/// Rectangular corner reliefs (see [`CornerRelief`]): where two bends of a
/// wall meet at a vertex, each bend ends `setback + ext` from the vertex
/// (`ext`: how far the relief reaches past the bend-allowance square), and
/// the wall and both flanges lose the relief's rectangle. Returns the bend
/// clips (edge, vertex, distance) and the cutters by wall.
fn corner_reliefs(def: &Definition, topo: &Topo, outlines: &[Outline], bends: &BendList, setbacks: &BTreeMap<usize, f64>) -> FResult<(Clips, Clips, Cuts, Vec<WrappedCut>)> {
    let mut clips = Vec::new();
    let mut extends = Vec::new();
    let mut cuts = Cuts::new();
    let mut wraps = Vec::new();
    let thickness = def.front + def.back;
    let bend_of = |e: usize| bends.iter().find(|x| x.0 == e);
    // Bend allowance θ (r + K T).
    let ba = |e: usize| -> Option<f64> { Some(bend_of(e)?.3.angle * (def.bends.get(&e)? + def.k_factor * thickness)) };
    // How far a rectangular relief reaches past the bend-allowance square of bend `e`.
    let ext = |c: &CornerRelief, e: usize| -> Option<f64> {
        let ba = ba(e)?;
        let side = if c.style == "RECTANGLE" { c.scale * ba } else { c.width };
        Some((side - ba) * 0.5)
    };
    let prism = |cuts: &mut Cuts, wall: usize, quad: &[DVec3], vi: usize| -> FResult<()> {
        let n = face_normal(&topo.faces[wall])?.1;
        let shift = -n * (def.back + thickness);
        let pts: Vec<DVec3> = quad.iter().map(|p| *p + shift).collect();
        let wire = k::make_polyline(&pts, true).map_err(|e| fe(e.to_string()))?;
        let face = k::make_face_from_wire(&wire, true).map_err(|e| fe(e.to_string()))?;
        let tool = k::extrude(&face, n * thickness * 3.0).map_err(|e| fe(e.to_string()))?.shape;
        cuts.entry(wall).or_default().push((tool, vi));
        Ok(())
    };
    for (p, outline) in outlines.iter().enumerate() {
        let Some((l, _, _)) = outline else { continue };
        let m = l.pts.len();
        for k in 0..m {
            let (e1, e2) = (l.side[(k + m - 1) % m], l.side[k]);
            let v = l.pts[k];
            let vi = topo.nearest_vertex(v);
            // The Corner feature's setting at this vertex, else the model's
            // (its three-bend style where three bends meet).
            let three = def.bends.keys().filter(|&&e| e < topo.edge_ends.len() && [topo.edge_ends[e].0, topo.edge_ends[e].1].iter().any(|p| p.distance(topo.vertices[vi]) < TOL)).count() >= 3;
            let model = CornerRelief { style: if three { def.corner.three.clone() } else { def.corner.style.clone() }, ..def.corner.clone() };
            let corner = def.corners.get(&vi).unwrap_or(&model);
            let style = corner.style.as_str();
            let (Some(&s1), Some(&s2), Some(x1), Some(x2), Some(b1), Some(b2)) = (setbacks.get(&e1), setbacks.get(&e2), ext(corner, e1), ext(corner, e2), ba(e1), ba(e2)) else { continue };
            let (d1, d2) = ((l.pts[(k + m - 1) % m] - v).normalize(), (l.pts[(k + 1) % m] - v).normalize());
            let into = |e: usize, d: DVec3| -> FResult<Option<(usize, DVec3, DVec3)>> {
                let Some(&(_, fa, fb, _)) = bend_of(e) else { return Ok(None) };
                let f = if fa == p { fb } else { fa };
                let (c, nf) = face_normal(&topo.faces[f])?;
                let mut into = nf.cross(d).normalize();
                if (c - v).dot(into) < 0.0 {
                    into = -into;
                }
                Ok(Some((f, into, nf)))
            };
            if style == "CLOSED" {
                // Each bend runs into the corner square up to its diagonal,
                // the two kept `clearance` apart across it; the wall loses
                // the sliver at its corner (live G34a: 0.02 × 2 mm face).
                let gap = def.clearance * std::f64::consts::FRAC_1_SQRT_2;
                let angle = |e: usize| bend_of(e).map_or(std::f64::consts::FRAC_PI_2, |b| b.3.angle);
                // A butt joint on the corner's rip (live 1670 / 1676): the
                // through wall's bend runs to its wall's end, both bends end
                // in flat Béziers from a plate point that keeps them a
                // clearance apart. Two 90° bends of one radius only.
                let corner_rip = def.rip_styles.keys().copied().find(|&r| r < topo.edge_ends.len() && [topo.edge_ends[r].0, topo.edge_ends[r].1].iter().any(|q| q.distance(topo.vertices[vi]) < TOL));
                let flange_of = |e: usize| bend_of(e).map(|&(_, fa, fb, _)| if fa == p { fb } else { fa });
                let right = |e: usize| (angle(e) - std::f64::consts::FRAC_PI_2).abs() < 1e-9;
                if let (Some(rip), Some(f1), Some(f2), Some(&r1), Some(&r2)) = (corner_rip, flange_of(e1), flange_of(e2), def.bends.get(&e1), def.bends.get(&e2))
                    && right(e1)
                    && right(e2)
                    && (r1 - r2).abs() < TOL
                    && (b1 - b2).abs() < TOL
                {
                    let thickness = def.front + def.back;
                    let (fe1, fe2) = (rip_offset(def, topo, rip, f1), rip_offset(def, topo, rip, f2));
                    // The corner's frame: bend e1 measured from e2's setback.
                    let (o1, o2) = (s2 - r1, s1 - r1);
                    // Live 1680: P = R + (K T + gap) / √2 (G34 b: 2.650538).
                    let p_model = r1 + (def.k_factor * thickness + def.clearance) * std::f64::consts::FRAC_1_SQRT_2;
                    extends.push((e1, v, fe1));
                    extends.push((e2, v, fe2));
                    wraps.push(WrappedCut { edge: e1, v, plate: p, ba: b1, shape: CutShape::Butt { plate_end: p_model + o1, flange_end: fe1 } });
                    wraps.push(WrappedCut { edge: e2, v, plate: p, ba: b2, shape: CutShape::Butt { plate_end: p_model + o2, flange_end: fe2 } });
                    let reach = p_model + o1 + s1;
                    prism(&mut cuts, p, &[v - (d1 + d2) * reach, v + d1 * (2.0 * reach) - d2 * reach, v - d1 * reach + d2 * (2.0 * reach)], vi)?;
                    continue;
                }
                // The flanges end where their walls do at the corner's (edge joint) rip.
                let edge_rip = def.rips.iter().copied().find(|&r| r < topo.edge_ends.len() && [topo.edge_ends[r].0, topo.edge_ends[r].1].iter().any(|q| q.distance(topo.vertices[vi]) < TOL));
                let flange_end = |e: usize| match (edge_rip, flange_of(e)) {
                    (Some(r), Some(f)) => rip_offset(def, topo, r, f),
                    _ => def.clearance,
                };
                let (fe1, fe2) = (flange_end(e1), flange_end(e2));
                extends.push((e1, v, fe1));
                extends.push((e2, v, fe2));
                wraps.push(WrappedCut { edge: e1, v, plate: p, ba: b1, shape: CutShape::Closed { plate_end: s2 + gap, flange_end: fe1, angle: angle(e1) } });
                wraps.push(WrappedCut { edge: e2, v, plate: p, ba: b2, shape: CutShape::Closed { plate_end: s1 + gap, flange_end: fe2, angle: angle(e2) } });
                let reach = s1 + s2 + gap;
                prism(&mut cuts, p, &[v - (d1 + d2) * reach, v + d1 * (2.0 * reach) - d2 * reach, v - d1 * reach + d2 * (2.0 * reach)], vi)?;
                continue;
            }
            let round = matches!(style, "ROUND" | "SIZED_ROUND");
            if !round && !matches!(style, "RECTANGLE" | "SIZED_RECTANGLE") {
                continue;
            }
            if round {
                // The circle on the bend-allowance square's centre.
                let half_diagonal = (b1 * b1 + b2 * b2).sqrt() * 0.5;
                let radius = if style == "ROUND" { corner.scale * half_diagonal } else { corner.diameter * 0.5 };
                if radius < half_diagonal {
                    continue;
                }
                let (c1, c2) = (s2 - b2 * 0.5, s1 - b1 * 0.5);
                let cylinder = |cuts: &mut Cuts, wall: usize, centre: DVec3, n: DVec3| -> FResult<()> {
                    let frame = k::Plane::new(centre - n * (def.back + thickness), n, d1.cross(n).normalize_or(d2));
                    let tool = k::make_cylinder(&frame, radius, thickness * 3.0, std::f64::consts::TAU).map_err(|e| fe(e.to_string()))?;
                    cuts.entry(wall).or_default().push((tool, vi));
                    Ok(())
                };
                cylinder(&mut cuts, p, v + d1 * c1 + d2 * c2, face_normal(&topo.faces[p])?.1)?;
                if let Some((f, i1, nf)) = into(e1, d1)? {
                    cylinder(&mut cuts, f, v + d1 * c1 + i1 * c2, nf)?;
                }
                if let Some((f, i2, nf)) = into(e2, d2)? {
                    cylinder(&mut cuts, f, v + d2 * c2 + i2 * c1, nf)?;
                }
                wraps.push(WrappedCut { edge: e1, v, plate: p, ba: b1, shape: CutShape::Round { along: c1, radius } });
                wraps.push(WrappedCut { edge: e2, v, plate: p, ba: b2, shape: CutShape::Round { along: c2, radius } });
                continue;
            }
            if x1 < 0.0 || x2 < 0.0 {
                continue;
            }
            let (a, b) = (s2 + x2, s1 + x1);
            clips.push((e1, v, a));
            clips.push((e2, v, b));
            prism(&mut cuts, p, &[v, v + d1 * a, v + d1 * a + d2 * b, v + d2 * b], vi)?;
            // Each flange: along its bend as far as the wall's cut, into it past the bend.
            for (e, d, along, depth) in [(e1, d1, a, b), (e2, d2, b, a)] {
                if let Some((f, into, _)) = into(e, d)? {
                    prism(&mut cuts, f, &[v, v + d * along, v + d * along + into * depth, v + into * depth], vi)?;
                }
            }
        }
    }
    Ok((clips, extends, cuts, wraps))
}

/// The solid a closed corner cuts off a bend of up to 90° (live G34a): what
/// lies before its end face, the radial lines at axial `x(θ) = plate_end +
/// (flange_end − plate_end) sin θ / sin angle` (θ turned from the plate's
/// wall, distances from the corner vertex) through the thickness. At each
/// radius the end is an ellipse arc, affine in (cos θ, sin θ): one rational
/// B-spline face (degree 1 across, the circle's quadratic around), pushed
/// back past the vertex. `span`: the strip's offsets along `n1`; `from_plate`:
/// θ runs from the plate's wall (wall a is the plate).
#[allow(clippy::too_many_arguments)]
fn closed_cutter(b: &Bend, sign: f64, base: DVec3, n1: DVec3, span: [f64; 2], at_start: bool, from_plate: bool, ends: [f64; 2], angle: f64) -> FResult<Shape> {
    let [plate_end, flange_end] = ends;
    let k_ = (flange_end - plate_end) / angle.sin();
    // x(θ) = A + B cos θ + C sin θ along the bend from its start.
    let (mut ca, mut cb, mut cc) = if from_plate { (plate_end, 0.0, k_) } else { (plate_end, k_ * angle.sin(), -k_ * angle.cos()) };
    if !at_start {
        (ca, cb, cc) = (b.len - ca, -cb, -cc);
    }
    let perp = |p: DVec3| {
        let r = p - b.axis_point;
        r - b.dir * r.dot(b.dir)
    };
    let e0 = perp(base).normalize();
    let e1 = b.dir.cross(e0) * sign;
    let (ra, rb) = (perp(base + n1 * span[0]).length(), perp(base + n1 * span[1]).length());
    let (lo, hi) = (ra.min(rb), ra.max(rb));
    let margin = (0.25 * (hi - lo)).min(0.5 * lo);
    let (t_a, t_b) = (-0.05 * angle, 1.05 * angle);
    let half = (t_b - t_a) * 0.5;
    let w = half.cos();
    let mid = (t_a + t_b) * 0.5;
    let cs = [(t_a.cos(), t_a.sin()), (mid.cos() / w, mid.sin() / w), (t_b.cos(), t_b.sin())];
    let mut poles = Vec::new();
    let mut weights = Vec::new();
    for r in [lo - margin, hi + margin] {
        for (j, (c, s)) in cs.iter().enumerate() {
            poles.push(b.axis_point + b.dir * (ca + cb * c + cc * s) + (e0 * c + e1 * s) * r);
            weights.push(if j == 1 { w } else { 1.0 });
        }
    }
    let surface = k::BSplineSurfaceData { poles, nu: 2, nv: 3, weights: Some(weights), u_knots: vec![0.0, 0.0, 1.0, 1.0], v_knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], u_degree: 1, v_degree: 2, u_periodic: false, v_periodic: false };
    let face = k::make_bspline_surface(&surface, &[], false).map_err(|e| fe(format!("Sheet metal corner relief failed: {e}")))?;
    let back = plate_end.max(flange_end) + 2.0 * (hi - lo) + 1e-3;
    let push = if at_start { -b.dir * back } else { b.dir * back };
    Ok(k::extrude(&face, push).map_err(|e| fe(format!("Sheet metal corner relief failed: {e}")))?.shape)
}

/// [`closed_cutter`] for a [`CutShape::Butt`] end (live 1676): the flat
/// Bézier wrapped onto the bend, its rulings radial and exact, sampled every
/// 0.75° around (the chord sags 1e-4 mm), pushed back past the vertex.
#[allow(clippy::too_many_arguments)]
fn butt_cutter(b: &Bend, sign: f64, base: DVec3, n1: DVec3, span: [f64; 2], at_start: bool, from_plate: bool, ba: f64, ends: [f64; 2], angle: f64) -> FResult<Shape> {
    let [plate_end, flange_end] = ends;
    let curve = butt_bezier(plate_end, flange_end, ba);
    // Along the bend at angle θ from wall a; past the bend's sides on along
    // the curve's end tangents (a smooth surface: a kink there left the
    // plate a 0.011 mm sliver).
    let slope = |a: glam::DVec2, b: glam::DVec2| (b.x - a.x) / (b.y - a.y);
    let x_at = |th: f64| {
        let th_plate = if from_plate { th } else { angle - th };
        let w = th_plate / angle * ba;
        let x = if w < 0.0 {
            curve[0].x + slope(curve[0], curve[1]) * w
        } else if w > ba {
            curve[3].x + slope(curve[2], curve[3]) * (w - ba)
        } else {
            butt_along(&curve, w)
        };
        if at_start { x } else { b.len - x }
    };
    let perp = |p: DVec3| {
        let r = p - b.axis_point;
        r - b.dir * r.dot(b.dir)
    };
    let e0 = perp(base).normalize();
    let e1 = b.dir.cross(e0) * sign;
    let (ra, rb) = (perp(base + n1 * span[0]).length(), perp(base + n1 * span[1]).length());
    let (lo, hi) = (ra.min(rb), ra.max(rb));
    let margin = (0.25 * (hi - lo)).min(0.5 * lo);
    let n = 120;
    let mut poles = Vec::new();
    for r in [lo - margin, hi + margin] {
        for j in 0..=n {
            let th = angle * (-0.05 + 1.1 * j as f64 / n as f64);
            poles.push(b.axis_point + b.dir * x_at(th) + (e0 * th.cos() + e1 * th.sin()) * r);
        }
    }
    // Cubic around the bend (the samples as its poles: smooth for the
    // booleans; at 0.75° apart they stray well under a micron).
    let mut v_knots = vec![0.0; 4];
    v_knots.extend((1..n - 2).map(|j| j as f64 / (n - 2) as f64));
    v_knots.extend([1.0; 4]);
    let surface = k::BSplineSurfaceData { poles, nu: 2, nv: n + 1, weights: None, u_knots: vec![0.0, 0.0, 1.0, 1.0], v_knots, u_degree: 1, v_degree: 3, u_periodic: false, v_periodic: false };
    let face = k::make_bspline_surface(&surface, &[], false).map_err(|e| fe(format!("Sheet metal corner relief failed: {e}")))?;
    let back = 2.0 * (plate_end.abs() + flange_end.abs()) + 4.0 * (hi - lo) + 1e-3;
    let push = if at_start { -b.dir * back } else { b.dir * back };
    Ok(k::extrude(&face, push).map_err(|e| fe(format!("Sheet metal corner relief failed: {e}")))?.shape)
}

/// A built sheet metal part and what its flat pattern is made of.
struct Built {
    part: Shape,
    pieces: Vec<Piece>,
    topo: Topo,
    /// Each wall's face on its definition plane (reliefs cut), by definition face.
    wall_flats: Vec<(usize, Shape)>,
    strips: Vec<Strip>,
}

/// A bend's developed strip: the tangent lines on its two walls (from the
/// bend's start to its end), unfolded with each wall.
#[derive(Clone)]
struct Strip {
    edge: usize,
    wall_a: usize,
    wall_b: usize,
    a: [DVec3; 2],
    b: [DVec3; 2],
    angle: f64,
    radius: f64,
    /// Wall a's definition normal, and the direction into wall b from the bend.
    normal_a: DVec3,
    toward_b: DVec3,
    /// The bend's axis (a point and the direction it turns wall b about),
    /// the direction into wall a, from the axis to wall a's tangent line,
    /// and its extent along the axis from `origin`.
    origin: DVec3,
    axis: DVec3,
    dir_a: DVec3,
    radial_a: DVec3,
    extent: (f64, f64),
    /// A rolled bend's definition face (else the bend is at `edge`).
    rolled: Option<usize>,
    /// Corner cuts across the strip's ends (closed corners): per end at
    /// the start (true) or the end, the cut's curve as (fraction along a→a'
    /// from `a[0]`, fraction across from wall a), wall a's side first.
    end_cuts: Vec<(bool, Vec<(f64, f64)>)>,
}

/// A rolled bend (definition face `fi`, a cylinder with a BEND joint such as
/// a Hem's arc) as a strip from the wall on one straight edge to the wall on
/// the other: wall a is the lower-numbered one. A rolled Hem's arc ends free
/// (wall b `usize::MAX`).
fn rolled_strip(topo: &Topo, fi: usize, radius: f64) -> Option<Strip> {
    let face = &topo.faces[fi];
    let Ok(k::Surface::Cylinder { origin, axis, .. }) = face.face_surface() else { return None };
    let axis = axis.normalize();
    let mut sides = Vec::new();
    for e in 0..topo.edges.len() {
        let fs = &topo.edge_faces[e];
        let (s, t) = topo.edge_ends[e];
        if !fs.contains(&(fi as u32)) || s.distance(t) < TOL || (t - s).normalize().cross(axis).length() > 1e-7 {
            continue;
        }
        let w = fs.iter().map(|f| *f as usize).find(|f| *f != fi).unwrap_or(usize::MAX);
        let (s, t) = if (t - s).dot(axis) < 0.0 { (t, s) } else { (s, t) };
        if sides.iter().all(|x: &(usize, usize, DVec3, DVec3)| x.1 != w) {
            sides.push((e, w, s, t));
        }
    }
    sides.sort_by_key(|x| x.1);
    if sides.first().is_none_or(|x| x.1 == usize::MAX) {
        return None;
    }
    let [(ea, wa, sa, ta), (_, wb, sb, tb)] = sides[..] else { return None };
    let [u0, u1, ..] = face.uv_bounds().ok()?;
    let angle = (u1 - u0).abs();
    let foot = |p: DVec3| origin + axis * (p - origin).dot(axis);
    let o = foot(sa);
    let (ra, rb) = ((sa - o).normalize(), (sb - foot(sb)).normalize());
    let turn = |v: DVec3, ang: f64| glam::DMat3::from_axis_angle(axis, ang) * v;
    let sign = if turn(ra, angle).distance(rb) <= turn(ra, -angle).distance(rb) { 1.0 } else { -1.0 };
    let (pa, na) = face_normal(&topo.faces[wa]).ok()?;
    let mut a1 = na.cross(axis).normalize();
    if (pa - sa).dot(a1) < 0.0 {
        a1 = -a1;
    }
    let len = sa.distance(ta);
    let (s0, s1) = (0.0_f64, len * sign);
    Some(Strip {
        edge: ea,
        wall_a: wa,
        wall_b: wb,
        a: [sa, ta],
        b: [sb, tb],
        angle,
        radius,
        normal_a: na,
        // The side the strip curls to (wall b's side of wall a).
        toward_b: -ra,
        origin: o,
        axis: axis * sign,
        dir_a: a1,
        radial_a: ra,
        extent: (s0.min(s1), s0.max(s1)),
        rolled: Some(fi),
        end_cuts: Vec::new(),
    })
}

/// The part solid of one definition sheet and the pieces its faces come from.
fn build_part(def: &Definition, shape: &Shape) -> FResult<Built> {
    let topo = Topo::of(shape)?;
    let (front, back) = (def.front, def.back);
    if front + back <= TOL {
        return Err(fe("The sheet metal thickness must be positive"));
    }
    let mut solids = Vec::new();
    let mut pieces = Vec::new();
    let mut wall_flats = Vec::new();
    let mut strips = Vec::new();
    // Bend pull-backs first (they trim the walls).
    let mut setbacks: BTreeMap<usize, f64> = BTreeMap::new();
    let mut bends = Vec::new();
    for (&e, &radius) in &def.bends {
        let faces = &topo.edge_faces[e];
        if faces.len() != 2 {
            continue;
        }
        let (fa, fb) = (faces[0] as usize, faces[1] as usize);
        let b = bend_geometry(&topo, e, fa, fb, radius, front, back)?;
        if b.angle.abs() < 1e-9 {
            continue;
        }
        setbacks.insert(e, b.setback);
        bends.push((e, fa, fb, b));
    }
    // Bends beside a curved wall come from the definition edge's fillet; their flat walls stop where it meets them.
    let (fillet_bends, trimmed_walls, fillet_setbacks) = curved_wall_bends(&topo, &bends);
    setbacks.extend(fillet_setbacks);
    // Each wall's outline in its definition plane: pulled back from its
    // bends, and from its rips by the minimal clearance (live
    // dom/fs-smprobe3-{4,6}.txt: both walls stop 0.02 mm short of a rip).
    // Curved faces are rolled bends (a closed Hem's half cylinder, live
    // dom/fs-smprobe3-7.txt): thickened as they are, the walls meeting
    // them tangentially.
    let mut outlines = Vec::new();
    // Walls with curved sides and no joints (a Corner break's arc): built
    // as they are.
    let mut plain = std::collections::BTreeSet::new();
    // Walls with curved sides: their pulled face (definition plane).
    let mut exact: BTreeMap<usize, Shape> = BTreeMap::new();
    for (fi, face) in topo.faces.iter().enumerate() {
        if face.face_plane().map_err(|e| fe(e.to_string()))?.is_none() {
            outlines.push(None);
            continue;
        }
        let l = match wall_loop(face, &topo) {
            Ok(l) => l,
            Err(e) => {
                let jointed = (0..topo.edges.len()).any(|x| topo.edge_faces[x].contains(&(fi as u32)) && (def.bends.contains_key(&x) || def.rips.contains(&x)));
                if jointed {
                    return Err(e);
                }
                plain.insert(fi);
                outlines.push(None);
                continue;
            }
        };
        let (_, n) = face_normal(face)?;
        let mut offset: Vec<f64> = l.side.iter().map(|e| setbacks.get(e).copied().unwrap_or_else(|| if def.rips.contains(e) { rip_offset(def, &topo, *e, fi) } else { 0.0 })).collect();
        // A free side running on from a bend into a corner (the front
        // flange past its bend, beside a Hem): pulled back with the bend, no
        // step (live qa5 Hem on G34, 9378.1835 mm³); or lying in the
        // pull-back of the bend one or two sides on round its other end, so
        // nothing of it is left (live 2055: the sloped wall's 2 mm free ends
        // between Flange 1's bend and the 135° bend).
        let m = l.pts.len();
        let setback_of = offset.clone();
        let eaten = |j: usize, k: usize| {
            let (p, q) = (l.pts[j], l.pts[(j + 1) % m]);
            let ahead = if (j + 1) % m == k { [(j + m - 1) % m, (j + m - 2) % m] } else { [(j + 1) % m, (j + 2) % m] };
            ahead.into_iter().filter(|&s| s != k && def.bends.contains_key(&l.side[s]) && setback_of[s] > TOL && !l.curved[s]).any(|s| {
                let u = inward(&l, n, s);
                [p, q].iter().all(|x| (-TOL..setback_of[s] + TOL).contains(&(*x - l.pts[s]).dot(u)))
            })
        };
        for k in 0..m {
            let Some(&radius) = def.bends.get(&l.side[k]) else { continue };
            let dir = (l.pts[(k + 1) % m] - l.pts[k]).normalize();
            for (j, v) in [((k + m - 1) % m, l.pts[k]), ((k + 1) % m, l.pts[(k + 1) % m])] {
                let e = l.side[j];
                let free = !def.bends.contains_key(&e) && !def.rips.contains(&e);
                let along = (l.pts[(j + 1) % m] - l.pts[j]).normalize();
                if free && along.cross(dir).length() <= 1e-9 && (joint_near(&topo, v, l.side[k], radius + front + back) || eaten(j, k)) {
                    offset[j] = offset[k];
                }
            }
        }
        // A free side running on in line from a rip (the corner's wall
        // beside a Hem's end): pulled back with it, no step (live qa4 S1:
        // the x = 100 wall's 0.02 mm beside the hem, request 1617).
        for k in 0..m {
            if !def.rips.contains(&l.side[k]) || offset[k] <= TOL {
                continue;
            }
            let dir = (l.pts[(k + 1) % m] - l.pts[k]).normalize_or_zero();
            for j in [(k + m - 1) % m, (k + 1) % m] {
                let e = l.side[j];
                let along = (l.pts[(j + 1) % m] - l.pts[j]).normalize_or_zero();
                if !def.bends.contains_key(&e) && !def.rips.contains(&e) && !l.curved[j] && along.cross(dir).length() <= 1e-9 && offset[j] <= TOL {
                    offset[j] = offset[k];
                }
            }
        }
        let mut sides = pulled_sides(&l, n, &offset);
        // A side the pulls turn round or close up (a free side between a bend's end and another bend that pulls the
        // wall back past it: cf-sheet-metal-model-plus's sloped wall after Flange 1, live's corner at x −11): the wall
        // from its face less the strips, as a curved one.
        let turned = (0..m).any(|k| (sides[k].1 - sides[k].0).dot(l.pts[(k + 1) % m] - l.pts[k]) <= TOL * TOL);
        if turned || l.curved.iter().any(|c| *c) {
            if l.curved.iter().zip(&l.side).any(|(c, e)| *c && (def.bends.contains_key(e) || def.rips.contains(e))) {
                return Err(fe("Curved sheet metal joints are not supported yet"));
            }
            let free = |j: usize| !def.bends.contains_key(&l.side[j]) && !def.rips.contains(&l.side[j]);
            let pf = pulled_face(face, &l, n, &offset, free)?;
            for k in 0..m {
                if offset[k] > TOL
                    && !l.curved[k]
                    && let Some(run) = run_inside(&pf, &l, n, k, offset[k])
                {
                    sides[k] = run;
                }
            }
            exact.insert(fi, pf);
        }
        outlines.push(Some((l, n, sides)));
    }
    // Where a wall's pulled-back side along edge `e` runs, as distances along `dir` from `start`.
    let extent = |f: usize, e: usize, start: DVec3, dir: DVec3| -> Option<(f64, f64)> {
        let (l, _, sides) = outlines[f].as_ref()?;
        let k = l.side.iter().position(|x| *x == e)?;
        let (t0, t1) = ((sides[k].0 - start).dot(dir), (sides[k].1 - start).dot(dir));
        Some((t0.min(t1), t0.max(t1)))
    };
    // Wall `f`'s side next to edge `e` at its end near `v`, when it is a free
    // side or a rip across the bend's end (not a bend, not running on along it).
    let free_end = |f: usize, e: usize, v: DVec3, dir: DVec3| -> Option<DVec3> {
        let (l, _, _) = outlines[f].as_ref()?;
        let k = l.side.iter().position(|x| *x == e)?;
        let m = l.pts.len();
        let j = if l.pts[k].distance(v) < l.pts[(k + 1) % m].distance(v) { (k + m - 1) % m } else { (k + 1) % m };
        let along = (l.pts[(j + 1) % m] - l.pts[j]).normalize_or_zero();
        (!def.bends.contains_key(&l.side[j]) && !l.curved[j] && along.cross(dir).length() > 1e-6).then_some(along)
    };
    // Wall `f`'s side next to edge `e` at its end near `v` is a short connector (no longer than `reach`) to a free side
    // running on along the bend: the bend edge moved in (INNER flange alignment), its end relieved (as
    // [`relief_tools`]), not a slanted end.
    let moved_in_end = |f: usize, e: usize, v: DVec3, dir: DVec3, reach: f64| -> bool {
        let Some((l, _, _)) = outlines[f].as_ref() else { return false };
        let Some(k) = l.side.iter().position(|x| *x == e) else { return false };
        let m = l.pts.len();
        let at_start = l.pts[k].distance(v) < l.pts[(k + 1) % m].distance(v);
        let (j, next) = if at_start { ((k + m - 1) % m, (k + m - 2) % m) } else { ((k + 1) % m, (k + 2) % m) };
        let connector = (l.pts[(j + 1) % m] - l.pts[j]).length();
        let along = (l.pts[(next + 1) % m] - l.pts[next]).normalize_or_zero();
        connector <= reach + TOL && along.cross(dir).length() <= 1e-9 && !def.bends.contains_key(&l.side[next]) && !def.rips.contains(&l.side[next])
    };
    let (clips, extends, mut cuts, wraps) = corner_reliefs(def, &topo, &outlines, &bends, &setbacks)?;
    for (e, fa, fb, b) in bends {
        // The bend runs where both walls do (a simple corner ends it where
        // the neighbouring bend starts: live sm4, sm6), less a corner relief.
        let (mut t0, mut t1) = (0.0, b.len);
        let (ea, eb) = (extent(fa, e, b.start, b.dir), extent(fb, e, b.start, b.dir));
        for (a0, a1) in [ea, eb].into_iter().flatten() {
            t0 = f64::max(t0, a0);
            t1 = f64::min(t1, a1);
        }
        // An end where both walls stop on free sides at different places
        // (a loft's bend meeting the profile at a slant): the bend runs to
        // each wall's end, its end one ruled face, rulings radial, its
        // definition-side arc on the plane of the two free sides (live
        // request 1612: 7.348509 mm², centroid within 1e-3).
        let mut skew: [Option<(f64, f64, DVec3, DVec3)>; 2] = [None, None];
        if let (Some(ea), Some(eb)) = (ea, eb) {
            let none_at = |at: DVec3| !clips.iter().chain(extends.iter()).any(|(ce, v, _)| *ce == e && v.distance(at) < TOL);
            let end = b.start + b.dir * b.len;
            for (i, v, (ta, tb)) in [(0, b.start, (ea.0, eb.0)), (1, end, (ea.1, eb.1))] {
                if (ta - tb).abs() <= TOL || !none_at(v) {
                    continue;
                }
                // (A moved-in bend edge's end: c2-sm-louver's Flange 3 at its tab, live's bend ends with the flange.)
                let reach = def.bends.get(&e).copied().unwrap_or(0.0);
                if moved_in_end(fa, e, v, b.dir, reach) || moved_in_end(fb, e, v, b.dir, reach) {
                    continue;
                }
                let (Some(da), Some(db)) = (free_end(fa, e, v, b.dir), free_end(fb, e, v, b.dir)) else { continue };
                skew[i] = Some((ta, tb, v, da.cross(db).normalize_or_zero()));
                if i == 0 {
                    t0 = ta.min(tb);
                } else {
                    t1 = ta.max(tb);
                }
            }
        }
        for (ce, v, d) in &clips {
            if *ce == e {
                if v.distance(b.start) < TOL {
                    t0 = t0.max(*d);
                } else {
                    t1 = t1.min(b.len - d);
                }
            }
        }
        // A closed corner: the bend runs on into the corner (its cut follows).
        for (ce, v, d) in &extends {
            if *ce == e {
                if v.distance(b.start) < TOL {
                    t0 = *d;
                } else {
                    t1 = b.len - d;
                }
            }
        }
        if t1 - t0 <= TOL {
            continue;
        }
        // Wall a's thickness at the pull-back line, revolved onto wall b.
        let (pa, n1) = face_normal(&topo.faces[fa])?;
        let mid = b.start + b.dir * (b.len * 0.5);
        let a1 = wall_side(&topo.faces[fa], n1, b.dir, mid, pa);
        let p1 = b.start + a1 * b.setback + b.dir * t0;
        let len = t1 - t0;
        // A closed corner's end is its cut alone: the solid starts a
        // thickness before it (the cut's end face meets the bend's start
        // plane tangentially at the flange side, live G34a).
        let closed_at = |start: bool| wraps.iter().any(|rc| rc.edge == e && (matches!(rc.shape, CutShape::Closed { angle, .. } if angle <= std::f64::consts::FRAC_PI_2 + 1e-9) || matches!(rc.shape, CutShape::Butt { .. })) && (rc.v.distance(b.start) < TOL) == start);
        // A slanted end: on past both walls' ends (its arc can bulge past
        // them), cut back below.
        let margin = |i: usize| skew[i].map_or(0.0, |(ta, tb, _, _)| b.angle.abs() * ((b.axis_point - b.start).length() + front + back) + 2.0 * (front + back) + (ta - tb).abs());
        let (m0, m1) = (if closed_at(true) { front + back } else { margin(0) }, if closed_at(false) { front + back } else { margin(1) });
        let (q1, qlen) = (p1 - b.dir * m0, len + m0 + m1);
        let strip = [q1 - n1 * back, q1 + n1 * front, q1 + n1 * front + b.dir * qlen, q1 - n1 * back + b.dir * qlen];
        let wire = k::make_polyline(&strip, true).map_err(|e| fe(e.to_string()))?;
        let face = k::make_face_from_wire(&wire, true).map_err(|e| fe(e.to_string()))?;
        // Turn towards wall b.
        let (pb, nb) = face_normal(&topo.faces[fb])?;
        let a2 = wall_side(&topo.faces[fb], nb, b.dir, mid, pb);
        let p2 = b.start + a2 * b.setback + b.dir * t0;
        let rotate = |ang: f64| {
            let r = p1 - b.axis_point;
            let along = b.dir * r.dot(b.dir);
            let perp = r - along;
            b.axis_point + along + perp * ang.cos() + b.dir.cross(perp) * ang.sin()
        };
        let sign = if rotate(b.angle).distance(p2) <= rotate(-b.angle).distance(p2) { 1.0 } else { -1.0 };
        let radial = {
            let r = b.start + a1 * b.setback - b.axis_point;
            (r - b.dir * r.dot(b.dir)).normalize_or_zero()
        };
        // Along the axis (`dir * sign`) from the axis point at the edge's start.
        let (s0, s1) = (t0 * sign, t1 * sign);
        // A slanted end: each wall's side of the strip runs to that wall's end.
        let (ta, tb) = ([skew[0].map_or(t0, |x| x.0), skew[1].map_or(t1, |x| x.0)], [skew[0].map_or(t0, |x| x.1), skew[1].map_or(t1, |x| x.1)]);
        // Closed corners' cuts in the flat (live 1676: the bend ends, not the
        // bend zone's rectangle): each curve sampled across the strip.
        let mut end_cuts = Vec::new();
        for rc in wraps.iter().filter(|rc| rc.edge == e && t1 - t0 > TOL) {
            let at_start = rc.v.distance(b.start) < TOL;
            let x_of = |w_plate: f64| match rc.shape {
                CutShape::Closed { plate_end, flange_end, angle } if angle <= std::f64::consts::FRAC_PI_2 + 1e-9 => Some(plate_end + (flange_end - plate_end) * (w_plate / rc.ba * angle).sin() / angle.sin()),
                CutShape::Butt { plate_end, flange_end } => Some(butt_along(&butt_bezier(plate_end, flange_end, rc.ba), w_plate)),
                _ => None,
            };
            let pts: Option<Vec<(f64, f64)>> = (0..=48)
                .map(|i| {
                    let w_a = i as f64 / 48.0;
                    let x = x_of(rc.ba * if fa == rc.plate { w_a } else { 1.0 - w_a })?;
                    let t = if at_start { x } else { b.len - x };
                    Some(((t - t0) / (t1 - t0), w_a))
                })
                .collect();
            if let Some(pts) = pts {
                end_cuts.push((at_start, pts));
            }
        }
        strips.push(Strip {
            edge: e,
            wall_a: fa,
            wall_b: fb,
            a: ta.map(|t| p1 + b.dir * (t - t0)),
            b: tb.map(|t| p2 + b.dir * (t - t0)),
            angle: b.angle,
            radius: def.bends.get(&e).copied().unwrap_or(0.0),
            normal_a: n1,
            toward_b: a2,
            origin: b.axis_point,
            axis: b.dir * sign,
            dir_a: a1,
            radial_a: radial,
            extent: (s0.min(s1), s0.max(s1)),
            rolled: None,
            end_cuts,
        });
        let mut bend = match fillet_bends.get(&e) {
            Some(f) => {
                let bend = k::thicken(f, front, back).map_err(|e| fe(format!("Sheet metal bend failed: {e}")))?.shape;
                // A curved wall's bend also stops where its flat wall's pulled side does (live 2055: Flange 1's bend
                // pulls the sloped wall back to x −11, and the curved bend stops there).
                if t0 > TOL || t1 < b.len - TOL { span_cut(&bend, b.start + b.dir * t0, b.dir, t1 - t0)? } else { bend }
            }
            None => k::revolve(&face, b.axis_point, b.dir * sign, b.angle).map_err(|e| fe(format!("Sheet metal bend failed: {e}")))?.shape,
        };
        let (v0, v1) = (topo.nearest_vertex(b.start), topo.nearest_vertex(b.start + b.dir * b.len));
        let neutral = def.bends.get(&e).copied().unwrap_or(0.0) + def.k_factor * (front + back);
        let turn = |p: DVec3, ang: f64| {
            let r = p - b.axis_point;
            let along = b.dir * r.dot(b.dir);
            let perp = r - along;
            b.axis_point + along + perp * ang.cos() + b.dir.cross(perp) * ang.sin()
        };
        for (i, end) in skew.iter().enumerate() {
            let Some((ta, tb, v, plane)) = *end else { continue };
            // Sections every so far across the flat strip (wall a at w = 0,
            // wall b at w = ba), the definition side's point at t(w) on the
            // sides' plane through the vertex, eased linearly onto the walls'
            // ends (a rip's side is pulled off the vertex; live 1612's bend D
            // top: 3.0233 mm²) (or a straight line between the walls' ends,
            // no plane); the cut reaches past the bend's square end.
            let (ba, steps) = (b.angle.abs() * neutral, 34);
            let reach = 2.0 * (b.angle.abs() * ((b.axis_point - b.start).length() + front + back) + 2.0 * (front + back) + (ta - tb).abs());
            let base = b.start + a1 * b.setback;
            let on_plane = |w: f64| (plane.length() > 0.5 && b.dir.dot(plane).abs() > 1e-6).then(|| (v - turn(base, sign * w / neutral)).dot(plane) / b.dir.dot(plane));
            let ease = on_plane(0.0).zip(on_plane(ba)).map(|(p0, p1)| (ta - p0, tb - p1));
            let at = |k: usize| {
                let w = ba * (k as f64 - 1.0) / (steps - 2) as f64;
                let t = match (on_plane(w), ease) {
                    (Some(t), Some((e0, e1))) => t + e0 + (e1 - e0) * w / ba,
                    _ => ta + (tb - ta) * w / ba,
                };
                (w, t)
            };
            let out_of = |t: f64| if i == 0 { t - reach } else { t + reach };
            let t0 = at(0).1;
            let cutter = if (0..=steps).all(|k| (at(k).1 - t0).abs() < 1e-9) {
                // The end square to the bend (every section at one place along it): the slab past it, planar (the
                // loft of those sections, a B-spline, made c2-sm-louver's Extrude 3 / 4 ~15 % slower).
                let size = bend.bounding_box(false).map(|bb| bb.diagonal()).unwrap_or(1.0) + reach;
                let face = k::make_plane_face(&k::Plane::from_normal(base + b.dir * t0, b.dir), -size, size, -size, size).map_err(|e| fe(e.to_string()))?;
                k::extrude(&face, b.dir * (out_of(t0) - t0)).map_err(|e| fe(format!("Sheet metal bend failed: {e}")))?.shape
            } else {
                let mut sections = Vec::new();
                for k in 0..=steps {
                    let (w, t_in) = at(k);
                    let t_out = out_of(t_in);
                    let (s_lo, s_hi) = (-back - (front + back), front + (front + back));
                    let quad: Vec<DVec3> = [(t_in, s_lo), (t_out, s_lo), (t_out, s_hi), (t_in, s_hi)].iter().map(|(t, s)| turn(base + b.dir * *t + n1 * *s, sign * w / neutral)).collect();
                    sections.push(k::make_polyline(&quad, true).map_err(|e| fe(e.to_string()))?);
                }
                k::loft(&sections, &k::LoftOptions { solid: true, ruled: false, closed: false, ..Default::default() }).map_err(|e| fe(format!("Sheet metal bend failed: {e}")))?.shape
            };
            bend = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&bend), &[cutter], &k::BooleanOptions::default()).map_err(|e| fe(format!("Sheet metal bend failed: {e}")))?.shape;
        }
        // Round corner reliefs: the flat circle wrapped onto the bend (angle
        // = flat distance / (r + K T)), cut out through the thickness.
        for rc in wraps.iter().filter(|rc| rc.edge == e) {
            let at_start = rc.v.distance(b.start) < TOL;
            let base = b.start + a1 * b.setback;
            if let CutShape::Butt { plate_end, flange_end } = rc.shape {
                let cutter = butt_cutter(&b, sign, base, n1, [-back, front], at_start, fa == rc.plate, rc.ba, [plate_end, flange_end], b.angle)?;
                bend = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&bend), &[cutter], &k::BooleanOptions::default()).map_err(|e| fe(format!("Sheet metal corner relief failed: {e}")))?.shape;
                continue;
            }
            if let CutShape::Closed { plate_end, flange_end, angle } = rc.shape
                && angle <= std::f64::consts::FRAC_PI_2 + 1e-9
            {
                let cutter = closed_cutter(&b, sign, base, n1, [-back, front], at_start, fa == rc.plate, [plate_end, flange_end], angle)?;
                bend = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&bend), &[cutter], &k::BooleanOptions::default()).map_err(|e| fe(format!("Sheet metal corner relief failed: {e}")))?.shape;
                continue;
            }
            let steps = 32;
            let mut sections = Vec::new();
            for i in 0..=steps {
                let w = rc.ba * (-0.02 + 1.04 * i as f64 / steps as f64);
                // `w` runs from wall a; the cut is drawn from the plate's side.
                let (lo, hi) = rc.range(if fa == rc.plate { w } else { rc.ba - w }, front + back);
                let (t_lo, t_hi) = if at_start { (lo, hi) } else { (b.len - hi, b.len - lo) };
                let (s_lo, s_hi) = (-back - (front + back), front + (front + back));
                let quad: Vec<DVec3> = [(t_lo, s_lo), (t_hi, s_lo), (t_hi, s_hi), (t_lo, s_hi)].iter().map(|(t, s)| turn(base + b.dir * *t + n1 * *s, sign * w / neutral)).collect();
                sections.push(k::make_polyline(&quad, true).map_err(|e| fe(e.to_string()))?);
            }
            let cutter = k::loft(&sections, &k::LoftOptions { solid: true, ruled: false, closed: false, ..Default::default() }).map_err(|e| fe(format!("Sheet metal corner relief failed: {e}")))?.shape;
            bend = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&bend), &[cutter], &k::BooleanOptions::default()).map_err(|e| fe(format!("Sheet metal corner relief failed: {e}")))?.shape;
        }
        for f in bend.faces() {
            let from = match f.face_surface() {
                Ok(k::Surface::Plane(pl)) if pl.unit_normal().dot(b.dir).abs() > 1.0 - 1e-9 => {
                    // The bend's end faces: the definition vertex at that end.
                    let c = face_normal(&f).map(|x| x.0).unwrap_or(b.start);
                    if (c - b.start).dot(b.dir) < b.len * 0.5 { Def::Vertex(v0) } else { Def::Vertex(v1) }
                }
                Ok(k::Surface::Cylinder { .. }) => Def::Edge(e),
                // A round corner relief's cut: the corner vertex.
                _ => {
                    let c = inner_point(&f).unwrap_or(b.start);
                    if (c - b.start).dot(b.dir) < b.len * 0.5 { Def::Vertex(v0) } else { Def::Vertex(v1) }
                }
            };
            pieces.push(Piece { face: f, from });
        }
        solids.push(bend);
    }
    // Walls: pulled back from their bends and thickened.
    let mut wall_solid: BTreeMap<usize, usize> = BTreeMap::new();
    for (fi, outline) in outlines.iter().enumerate() {
        let Some((l, n, pulled)) = outline else {
            if plain.contains(&fi) {
                wall_solid.insert(fi, solids.len());
                plain_wall(&topo, fi, front, back, &mut solids, &mut pieces)?;
                wall_flats.push((fi, topo.faces[fi].clone()));
                continue;
            }
            let face = trimmed_walls.get(&fi).unwrap_or(&topo.faces[fi]);
            rolled_bend(&topo, fi, face, front, back, &mut solids, &mut pieces)?;
            if let Some(st) = def.rolled.get(&fi).and_then(|r| rolled_strip(&topo, fi, *r)) {
                strips.push(st);
            }
            continue;
        };
        let n = *n;
        let base = match exact.get(&fi) {
            Some(pf) => pf.translated(-n * back).map_err(|e| fe(e.to_string()))?,
            None => {
                let pts: Vec<DVec3> = outline_points(pulled).into_iter().map(|p| p - n * back).collect();
                let wire = k::make_polyline(&pts, true).map_err(|e| fe(e.to_string()))?;
                on_definition(&topo.faces[fi], &wire, -n * back)?
            }
        };
        let mut slab = k::extrude(&base, n * (front + back)).map_err(|e| fe(format!("Sheet metal wall failed: {e}")))?.shape;
        // A curved side the definition carries in pieces (a cut's arc split
        // at its top): one face, as live (qa4 S1's notch, request 1617).
        if exact.contains_key(&fi) {
            slab = k::unify_same_domain(&slab).map_err(|e| fe(format!("Sheet metal wall failed: {e}")))?.shape;
        }
        let mut reliefs = relief_tools(def, &topo, fi, l, n, front + back)?;
        reliefs.extend(cuts.remove(&fi).unwrap_or_default());
        // The wall in the flat: its outline on the definition plane, reliefs cut.
        let mut flat = match exact.get(&fi) {
            Some(pf) => pf.clone(),
            None => k::make_face_from_wire(&k::make_polyline(&outline_points(pulled), true).map_err(|e| fe(e.to_string()))?, true).map_err(|e| fe(e.to_string()))?,
        };
        // Its holes, through the thickness (and out of the flat), all in one cut (a Fill pattern makes thousands).
        let mut holes = wall_holes(&topo.faces[fi], &topo)?;
        // A bend on a hole's side (a louver's flange on its slot): the wall pulled back from it by the setback, as from
        // an outer side (live c2-sm-corner-break: else the bend overlaps the wall and the flap comes off as its own part).
        for (hole, edges) in &mut holes {
            if !edges.iter().any(|e| setbacks.get(e).is_some_and(|s| *s > TOL)) {
                continue;
            }
            let Ok(hl) = wall_loop(hole, &topo) else { continue };
            let (_, hn) = face_normal(hole)?;
            let mut pull: Vec<f64> = hl.side.iter().map(|e| -setbacks.get(e).copied().unwrap_or(0.0)).collect();
            // A free side running on in line from a bend's side (the clearance pieces at a flange's ends) goes with it,
            // no step (live: the slot's ends run on to the pulled-back line).
            let m = hl.pts.len();
            let dir = |k: usize| (hl.pts[(k + 1) % m] - hl.pts[k]).normalize_or_zero();
            for _ in 0..m {
                for k in 0..m {
                    if pull[k] != 0.0 {
                        continue;
                    }
                    for j in [(k + m - 1) % m, (k + 1) % m] {
                        if pull[j] != 0.0 && dir(k).cross(dir(j)).length() < 1e-9 {
                            pull[k] = pull[j];
                        }
                    }
                }
            }
            // A slot with rounded corners (a Corner break's arcs, live c2-sm-corner-break): kept exact, the pulled
            // sides added as strips beyond it.
            if hl.curved.iter().any(|c| *c) {
                let mut with = vec![hole.clone()];
                for (j, &off) in pull.iter().enumerate() {
                    if off >= -TOL || hl.curved[j] {
                        continue;
                    }
                    let (a, b) = (hl.pts[j], hl.pts[(j + 1) % m]);
                    let u = inward(&hl, hn, j);
                    let quad = [a - u * off, b - u * off, b + u * off, a + u * off];
                    if let Ok(f) = k::make_polyline(&quad, true).and_then(|w| k::make_face(&k::Plane::new(a, hn, (b - a).normalize()), &w, &[])) {
                        with.push(f);
                    }
                }
                let fused = k::boolean(k::BooleanOp::Fuse, &with[..1], &with[1..], &k::BooleanOptions { unify: true, ..Default::default() });
                if let Ok(r) = fused
                    && let [f] = r.shape.faces().as_slice()
                {
                    *hole = f.clone();
                    continue;
                }
            }
            let pts = outline_points(&pulled_sides(&hl, hn, &pull));
            if let Ok(f) = k::make_polyline(&pts, true).and_then(|w| k::make_face_from_wire(&w, true)) {
                *hole = f;
            }
        }
        if !holes.is_empty() {
            let mut tools = Vec::with_capacity(holes.len());
            for (hole, edges) in &holes {
                let base = hole.translated(-n * (back + front + back)).map_err(|e| fe(e.to_string()))?;
                let tool = k::extrude(&base, n * (front + back) * 3.0).map_err(|e| fe(e.to_string()))?.shape;
                // Its sides come from the hole's definition edges.
                for face in tool.faces() {
                    let c = inner_point(&face).unwrap_or_default();
                    let near = |e: &usize| k::make_vertex(c).ok().and_then(|v| k::distance(&v, &topo.edges[*e]).ok()).map_or(f64::INFINITY, |d| d.distance);
                    let from = edges.iter().copied().min_by(|a, b| near(a).total_cmp(&near(b))).map_or(Def::Face(fi), Def::Edge);
                    pieces.push(Piece { face, from });
                }
                tools.push(tool);
            }
            let faces: Vec<Shape> = holes.into_iter().map(|h| h.0).collect();
            if let Ok(r) = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&flat), &faces, &k::BooleanOptions::default()) {
                flat = r.shape;
            }
            slab = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&slab), &tools, &k::BooleanOptions::default()).map_err(|e| fe(format!("Sheet metal wall hole failed: {e}")))?.shape;
        }
        if !reliefs.is_empty() {
            let tools: Vec<Shape> = reliefs.iter().map(|r| r.0.clone()).collect();
            if let Ok(r) = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&flat), &tools, &k::BooleanOptions::default()) {
                flat = r.shape;
            }
            slab = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&slab), &tools, &k::BooleanOptions::default()).map_err(|e| fe(format!("Sheet metal bend relief failed: {e}")))?.shape;
            // The slot's faces come from the bend's end vertex.
            for (tool, v) in &reliefs {
                pieces.extend(tool.faces().into_iter().map(|face| Piece { face, from: Def::Vertex(*v) }));
            }
        }
        for f in slab.faces() {
            let from = match f.face_surface() {
                Ok(k::Surface::Plane(pl)) if pl.unit_normal().cross(n).length() < 1e-9 => Def::Face(fi),
                _ => {
                    // A side face: the loop side it was swept from (the nearest side itself, not its line: a short
                    // step's line runs across the wall, past every end face on its plane: c2-sm-louver's Flange 4).
                    let c = face_normal(&f).map(|x| x.0).unwrap_or_default();
                    let k = (0..l.pts.len())
                        .min_by(|a, b| {
                            let d = |k: usize| {
                                let (p, q) = (pulled[k].0 - n * back, pulled[k].1 - n * back);
                                let len = (q - p).length();
                                let dq = (q - p) / len.max(1e-300);
                                let r = c - p - n * (c - p).dot(n);
                                let t = r.dot(dq).clamp(0.0, len);
                                (r - dq * t).length()
                            };
                            d(*a).total_cmp(&d(*b))
                        })
                        .unwrap_or(0);
                    Def::Edge(l.side[k])
                }
            };
            pieces.push(Piece { face: f, from });
        }
        wall_flats.push((fi, flat));
        wall_solid.insert(fi, solids.len());
        solids.push(slab);
    }
    // The walls and bends only touch (on the faces they share): glued (kept only when the volumes add up, else
    // fused as usual).
    let glued = k::BooleanOptions { glue: k::Glue::ShiftChecked, ..Default::default() };
    let fuse = |solids: &[Shape]| -> FResult<Shape> {
        match solids {
            [one] => Ok(one.clone()),
            _ => Ok(k::fuse_all(solids, &glued).map_err(|e| fe(format!("Sheet metal part failed: {e}")))?.shape),
        }
    };
    // Coplanar walls (split by a rip) are one wall: unified apart from the rest, so the ends of the walls and bends
    // beside them stay in pieces (c2-sm-louver's Louver, live's 22 faces).
    let mut root: Vec<usize> = (0..solids.len()).collect();
    fn find(root: &mut [usize], i: usize) -> usize {
        if root[i] != i {
            root[i] = find(root, root[i]);
        }
        root[i]
    }
    for e in (0..topo.edges.len()).filter(|&e| coplanar_rip(&topo, e)) {
        let [a, b] = topo.edge_faces[e][..] else { continue };
        if let (Some(&sa), Some(&sb)) = (wall_solid.get(&(a as usize)), wall_solid.get(&(b as usize))) {
            let (ra, rb) = (find(&mut root, sa), find(&mut root, sb));
            root[ra] = rb;
        }
    }
    let mut groups: BTreeMap<usize, Vec<Shape>> = BTreeMap::new();
    for (i, solid) in solids.into_iter().enumerate() {
        groups.entry(find(&mut root, i)).or_default().push(solid);
    }
    let mut parts = Vec::with_capacity(groups.len());
    for group in groups.into_values() {
        parts.push(match group.as_slice() {
            [one] => one.clone(),
            _ => k::unify_same_domain(&fuse(&group)?).map_err(|e| fe(format!("Sheet metal part failed: {e}")))?.shape,
        });
    }
    let fused = fuse(&parts)?;
    Ok(Built { part: fused, pieces, topo, wall_flats, strips })
}

/// A wall without joints built as its definition face is (curved sides
/// too): thickened, caps from the face, sides from the nearest edge.
fn plain_wall(topo: &Topo, fi: usize, front: f64, back: f64, solids: &mut Vec<Shape>, pieces: &mut Vec<Piece>) -> FResult<()> {
    let face = &topo.faces[fi];
    let (_, n) = face_normal(face)?;
    let base = k::transform(face, &k::Transform::Matrix(glam::DMat4::from_translation(-n * back)), true).map_err(|e| fe(e.to_string()))?.shape;
    let slab = k::extrude(&base, n * (front + back)).map_err(|e| fe(format!("Sheet metal wall failed: {e}")))?.shape;
    let own: Vec<usize> = (0..topo.edges.len()).filter(|e| topo.edge_faces[*e].contains(&(fi as u32))).collect();
    for f in slab.faces() {
        let cap = matches!(f.face_surface(), Ok(k::Surface::Plane(pl)) if pl.unit_normal().cross(n).length() < 1e-9);
        let from = if cap {
            Def::Face(fi)
        } else {
            let c = inner_point(&f).unwrap_or_default();
            let near = |e: &usize| k::make_vertex(c).ok().and_then(|v| k::distance(&v, &topo.edges[*e]).ok()).map_or(f64::INFINITY, |d| d.distance);
            own.iter().copied().min_by(|a, b| near(a).total_cmp(&near(b))).map_or(Def::Face(fi), Def::Edge)
        };
        pieces.push(Piece { face: f, from });
    }
    solids.push(slab);
    Ok(())
}

/// Bends beside a curved (rolled) wall: the definition edge's fillet at the definition radius (both walls are
/// swept along the bend, so it is a cylinder meeting each tangentially, as live 2031's), the wall trimmed by it.
/// Each curved wall's bends are filleted together on the wall and its neighbours across them. A wall's planar
/// neighbours are pulled back by the setback as usual, which is where the fillet leaves them.
/// Bends' fillet faces by definition edge, curved walls as their fillets left them, flat walls' setbacks by edge.
type CurvedWallBends = (BTreeMap<usize, Shape>, BTreeMap<usize, Shape>, BTreeMap<usize, f64>);

fn curved_wall_bends(topo: &Topo, bends: &[(usize, usize, usize, Bend)]) -> CurvedWallBends {
    let (mut by_edge, mut walls, mut setbacks) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let curved = |f: usize| topo.faces[f].face_plane().ok().flatten().is_none();
    for c in (0..topo.faces.len()).filter(|&f| curved(f)) {
        let mine: Vec<&(usize, usize, usize, Bend)> = bends.iter().filter(|(_, fa, fb, _)| *fa == c || *fb == c).collect();
        if mine.is_empty() {
            continue;
        }
        let on = |p: DVec3, x: &Shape| k::make_vertex(p).ok().and_then(|v| k::distance(&v, x).ok()).is_some_and(|d| d.distance < 1e-7);
        // The curved wall and its flat neighbours filleted along the bends; where that fails (a neighbour running on
        // past the curved wall's end: cf-sheet-metal-model-plus's sloped wall after Flange 1), the same with each
        // neighbour cut to its bend edge's span (only the fillets and the curved wall are taken).
        let fillet = |trim: bool| -> Option<(Vec<(usize, f64)>, k::OpResult)> {
            let mut faces = vec![topo.faces[c].clone()];
            for (e, fa, fb, _) in &mine {
                let flat = if *fa == c { *fb } else { *fa };
                faces.push(if trim { span_of(topo, *e, flat)? } else { topo.faces[flat].clone() });
            }
            let shell = k::sew(&faces, 1e-7, false).ok()?.shape;
            let edges = shell.edges();
            let picked: Vec<(usize, f64)> = mine
                .iter()
                .map(|(e, _, _, b)| {
                    let (s, t) = topo.edge_ends[*e];
                    edges.iter().position(|x| on((s + t) * 0.5, x)).map(|i| (i, b.def_radius))
                })
                .collect::<Option<_>>()?;
            let shapes: Vec<Shape> = picked.iter().map(|(i, _)| edges[*i].clone()).collect();
            let radii: Vec<f64> = picked.iter().map(|(_, r)| *r).collect();
            Some((picked, k::fillet_radii(&shell, &shapes, &radii).ok()?))
        };
        let Some((picked, res)) = fillet(false).or_else(|| fillet(true)) else { continue };
        let generated = |i: usize| -> Option<Shape> {
            res.history.generated(0, ShapeType::Edge, i as u32).into_iter().find(|(t, _)| *t == ShapeType::Face).and_then(|(_, f)| res.shape.sub_shape(ShapeType::Face, f as usize).ok())
        };
        let fillets: Option<Vec<Shape>> = picked.iter().map(|(i, _)| generated(*i)).collect();
        let Some(fillets) = fillets else { continue };
        // Each the exact cylinder through its two tangent lines (the kernel approximates a fillet against a
        // B-spline wall; Parasolid's, live 2031's, is a cylinder).
        let fillets: Vec<Shape> = mine.iter().zip(fillets).map(|((e, _, _, b), f)| {
            let (s, t) = topo.edge_ends[*e];
            exact_bend_face(&f, (t - s).normalize_or_zero(), b.def_radius).unwrap_or(f)
        }).collect();
        // The curved wall as the fillets left it: the result's face lying on the wall's middle.
        let (mid, _) = match face_normal(&topo.faces[c]) {
            Ok(x) => x,
            Err(_) => continue,
        };
        let Some(wall) = res.shape.faces().into_iter().find(|f| on(mid, f) && !fillets.iter().any(|x| x == f)) else { continue };
        for ((e, fa, fb, _), f) in mine.iter().zip(fillets) {
            // The flat wall's setback: from the joint to the fillet's edge on that wall.
            let flat = if *fa == c { *fb } else { *fa };
            let (s, t) = topo.edge_ends[*e];
            let joint = (s + t) * 0.5;
            if let Ok(Some(pl)) = topo.faces[flat].face_plane() {
                let on_wall = f.edges().into_iter().filter_map(|x| x.endpoints().ok()).map(|(a, b)| (a + b) * 0.5).find(|m| (*m - pl.origin).dot(pl.normal).abs() < 1e-7 && m.distance(joint) > 1e-9);
                if let Some(m) = on_wall {
                    let dir = (t - s).normalize_or_zero();
                    let d = m - joint;
                    setbacks.insert(*e, (d - dir * d.dot(dir)).length());
                }
            }
            by_edge.insert(*e, f);
        }
        walls.insert(c, wall);
    }
    (by_edge, walls, setbacks)
}

/// Flat wall `flat` cut to the span of its bend edge `e` (between the planes square to the edge at its ends).
fn span_of(topo: &Topo, e: usize, flat: usize) -> Option<Shape> {
    let (s, t) = topo.edge_ends[e];
    let len = s.distance(t);
    let cut = span_cut(&topo.faces[flat], s, (t - s) / len.max(1e-300), len).ok()?;
    match cut.faces().as_slice() {
        [one] => Some(one.clone()),
        _ => None,
    }
}

/// Shape `s` between the planes square to `dir` at `from` and `len` further on.
fn span_cut(s: &Shape, from: DVec3, dir: DVec3, len: f64) -> FResult<Shape> {
    let big = s.bounding_box(false).map_err(|e| fe(e.to_string()))?.diagonal() * 2.0;
    let end = k::make_plane_face(&k::Plane::new(from, dir, dir.any_orthonormal_vector()), -big, big, -big, big).map_err(|e| fe(e.to_string()))?;
    let slab = k::extrude(&end, dir * len).map_err(|e| fe(e.to_string()))?.shape;
    Ok(k::boolean(k::BooleanOp::Common, std::slice::from_ref(s), &[slab], &k::BooleanOptions::default()).map_err(|e| fe(format!("Sheet metal bend failed: {e}")))?.shape)
}

/// Fillet face `f` along `dir` as the exact cylinder of `radius` through its two straight tangent edges, facing
/// the same way.
fn exact_bend_face(f: &Shape, dir: DVec3, radius: f64) -> Option<Shape> {
    let lines: Vec<(DVec3, DVec3)> = f.edges().iter().filter_map(|e| e.endpoints().ok()).filter(|(a, b)| (*b - *a).length() > 1e-9 && (*b - *a).normalize().cross(dir).length() < 1e-6).collect();
    let [(a0, a1), (b0, _)] = lines.as_slice() else { return None };
    let flat = |p: DVec3| p - dir * (p - *a0).dot(dir);
    let (a, b) = (*a0, flat(*b0));
    let half = a.distance(b) * 0.5;
    if half <= 1e-12 || half > radius {
        return None;
    }
    let m = (a + b) * 0.5;
    let perp = dir.cross(b - a).normalize();
    let h = (radius * radius - half * half).max(0.0).sqrt();
    let inside = flat(inner_point(f)?);
    let c = [m + perp * h, m - perp * h].into_iter().min_by(|x, y| (inside.distance(*x) - radius).abs().total_cmp(&(inside.distance(*y) - radius).abs()))?;
    let (ra, rb) = ((a - c).normalize(), (b - c).normalize());
    let angle = ra.dot(rb).clamp(-1.0, 1.0).acos();
    let axis = if ra.cross(rb).dot(dir) >= 0.0 { dir } else { -dir };
    let mut face = k::revolve(&k::make_line(*a0, *a1).ok()?, c, axis, angle).ok()?.shape.faces().into_iter().next()?;
    // Facing as the fillet did (the definition sheet's side).
    let probe = c + (ra + rb).normalize() * radius;
    if let (Ok(n0), Ok(n1)) = (f.normal_at_point(probe), face.normal_at_point(probe))
        && n0.dot(n1) < 0.0
    {
        face = face.reversed().ok()?;
    }
    Some(face)
}

/// A rolled bend: definition face `fi` thickened (`front` along its normal).
/// Its offsets come from the face; its end caps from the face's edges there.
fn rolled_bend(topo: &Topo, fi: usize, face: &Shape, front: f64, back: f64, solids: &mut Vec<Shape>, pieces: &mut Vec<Piece>) -> FResult<()> {
    let res = k::thicken(face, front, back).map_err(|e| fe(format!("Sheet metal bend failed: {e}")))?.shape;
    let own: Vec<usize> = (0..topo.edges.len()).filter(|e| topo.edge_faces[*e].contains(&(fi as u32))).collect();
    for f in res.faces() {
        let from = match f.face_surface() {
            Ok(k::Surface::Plane(_)) => {
                let c = inner_point(&f).unwrap_or_default();
                let near = |e: &usize| k::make_vertex(c).ok().and_then(|v| k::distance(&v, &topo.edges[*e]).ok()).map_or(f64::INFINITY, |d| d.distance);
                own.iter().copied().min_by(|a, b| near(a).total_cmp(&near(b))).map_or(Def::Face(fi), Def::Edge)
            }
            _ => Def::Face(fi),
        };
        pieces.push(Piece { face: f, from });
    }
    solids.push(res);
    Ok(())
}

/// A point inside a face.
pub(super) fn inner_point(f: &Shape) -> Option<DVec3> {
    let [u0, u1, v0, v1] = f.uv_bounds().ok()?;
    for (a, b) in [(0.5, 0.5), (0.3, 0.3), (0.7, 0.7), (0.3, 0.7), (0.7, 0.3), (0.5, 0.2), (0.2, 0.5), (0.5, 0.8), (0.8, 0.5)] {
        let p = f.point_at_uv(u0 + (u1 - u0) * a, v0 + (v1 - v0) * b).ok()?;
        if f.contains_point(p, 1e-9).unwrap_or(false) {
            return Some(p);
        }
    }
    None
}

/// The part's faces lying on a face of the part this rebuild replaces take
/// that face's name (several pieces of one: its splits by `feature`), so what
/// a feature leaves alone keeps its references and stays pickable in the
/// feature's preview (Onshape keeps the ids through the definition body).
fn keep_names(part: &Shape, names: &mut [crate::naming::Name], before: &Shape, before_names: &[crate::naming::Name], feature: crate::reference::FeatureId) {
    use crate::reference::TopoName;
    let (new, old) = (part.faces(), before.faces());
    let probe = |f: &Shape| inner_point(f).and_then(|p| Some((p, f.normal_at_point(p).ok()?)));
    // On the face, facing the same way, and not on its boundary (a bend's
    // end face touches the side face it replaces at one edge).
    let on = |f: &Shape, (p, n): (DVec3, DVec3)| {
        f.contains_point(p, 1e-7).unwrap_or(false)
            && f.normal_at_point(p).is_ok_and(|m| m.dot(n) > 1.0 - 1e-6)
            && k::make_vertex(p).is_ok_and(|v| f.edges().iter().all(|e| k::distance(&v, e).is_ok_and(|d| d.distance > 1e-6)))
    };
    let (new_probes, old_probes): (Vec<_>, Vec<_>) = (new.iter().map(probe).collect(), old.iter().map(probe).collect());
    // A point outside a face's box is not on it (most pairs skip the
    // projection: a hem's rebuild spent 2 s here, part_sources' rule).
    let boxed = |fs: &[Shape]| -> Vec<Option<k::BoundingBox>> { fs.iter().map(|f| f.bounding_box(false).ok()).collect() };
    let (new_boxes, old_boxes) = (boxed(&new), boxed(&old));
    // Nor off a planar face's plane (a plate with a Fill pattern's thousand holes: every probe is in its box).
    let (new_planes, old_planes) = (face_planes(&new), face_planes(&old));
    let in_box = |b: &Option<k::BoundingBox>, pl: &Option<(DVec3, DVec3)>, x: &Option<(DVec3, DVec3)>| {
        x.is_some_and(|(p, _)| b.as_ref().is_none_or(|b| (b.min - DVec3::splat(1e-6)).cmple(p).all() && p.cmple(b.max + DVec3::splat(1e-6)).all()) && pl.is_none_or(|(o, n)| (p - o).dot(n).abs() < 1e-6))
    };
    let mut of: Vec<Vec<usize>> = vec![Vec::new(); old.len()];
    for (i, f) in new.iter().enumerate() {
        let hit = (0..old.len()).find(|&j| (in_box(&old_boxes[j], &old_planes[j], &new_probes[i]) && new_probes[i].is_some_and(|x| on(&old[j], x))) || (in_box(&new_boxes[i], &new_planes[i], &old_probes[j]) && old_probes[j].is_some_and(|x| on(f, x))));
        if let Some(j) = hit.filter(|&j| j < before_names.len()) {
            of[j].push(i);
        }
    }
    for (j, pieces) in of.iter().enumerate() {
        match pieces[..] {
            [] => {}
            [i] => names[i] = before_names[j].clone(),
            _ => {
                for (o, &i) in pieces.iter().enumerate() {
                    names[i] = Arc::new(TopoName::Split { parent: before_names[j].clone(), by: feature, ordinal: o as u16 });
                }
            }
        }
    }
}

/// Each planar face's plane (a point, its unit normal); `None` for the others.
fn face_planes(faces: &[Shape]) -> Vec<Option<(DVec3, DVec3)>> {
    faces.iter().map(|f| f.face_plane().ok().flatten().map(|p| (p.origin, p.normal.normalize()))).collect()
}

/// What each face, edge and vertex of the part comes from.
fn part_sources(part: &Shape, pieces: &[Piece], topo: &Topo) -> (Vec<Def>, Vec<Def>, Vec<Def>) {
    // Each piece's box (a point outside it is not on it: most pieces are
    // skipped without projecting onto them).
    let boxes: Vec<Option<k::BoundingBox>> = pieces.iter().map(|x| x.face.bounding_box(false).ok()).collect();
    let planes = face_planes(&pieces.iter().map(|x| x.face.clone()).collect::<Vec<_>>());
    let in_box = |i: usize, p: DVec3| boxes[i].is_none_or(|b| (b.min - DVec3::splat(1e-6)).cmple(p).all() && p.cmple(b.max + DVec3::splat(1e-6)).all()) && planes[i].is_none_or(|(o, n)| (p - o).dot(n).abs() < 1e-6);
    let faces: Vec<Def> = part
        .faces()
        .iter()
        .map(|f| {
            let Some(p) = inner_point(f) else { return Def::Face(0) };
            // A piece through the point that faces the same way (a unified wall's
            // middle can lie on the edge of a side piece).
            let n = f.normal_at_point(p).ok();
            let along = |x: &Piece| match (n, x.face.normal_at_point(p)) {
                (Some(a), Ok(b)) => a.cross(b).length() < 1e-6,
                _ => true,
            };
            pieces.iter().enumerate().find(|(i, x)| in_box(*i, p) && x.face.contains_point(p, 1e-7).unwrap_or(false) && along(x)).map_or(Def::Face(0), |(_, x)| x.from)
        })
        .collect();
    let ef = part.edge_faces().unwrap_or_default();
    let edges: Vec<Def> = part
        .edges()
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let fs: Vec<Def> = ef.get(i).map(|v| v.iter().filter_map(|f| faces.get(*f as usize).copied()).collect()).unwrap_or_default();
            let mid = e.endpoints().map(|(a, b)| (a + b) * 0.5).unwrap_or_default();
            match fs.as_slice() {
                [a, b] if a == b => *a,
                [Def::Vertex(v), _] | [_, Def::Vertex(v)] => Def::Vertex(*v),
                [Def::Edge(x), Def::Edge(y)] => Def::Vertex(topo.shared_vertex(*x, *y).unwrap_or_else(|| topo.nearest_vertex(mid))),
                [Def::Edge(x), Def::Face(_)] | [Def::Face(_), Def::Edge(x)] => Def::Edge(*x),
                [a, ..] => *a,
                [] => Def::Vertex(topo.nearest_vertex(mid)),
            }
        })
        .collect();
    let ve = part.vertex_edges().unwrap_or_default();
    let vertices: Vec<Def> = part
        .vertices()
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let from_edges = ve.get(i).and_then(|es| {
                es.iter().find_map(|e| match edges.get(*e as usize) {
                    Some(Def::Vertex(x)) => Some(Def::Vertex(*x)),
                    _ => None,
                })
            });
            from_edges.unwrap_or_else(|| Def::Vertex(topo.nearest_vertex(v.vertex_point().unwrap_or_default())))
        })
        .collect();
    (faces, edges, vertices)
}

fn update_sheet_metal_geometry(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let args = def_arg(i, &a, 2)?;
    let changed = evaluate_refs(i, &cx, args.get("entities"))?;
    // The definition sheets to rebuild and their attributes.
    let mut tool_ids: Vec<(Arc<str>, Reference)> = Vec::new();
    let (defs, def_refs) = {
        let st = cx.state.lock();
        let q = QueryCx::new(&st);
        let changed_owners: Vec<BodyId> = changed.iter().flat_map(|r| q.from_ref(r)).filter_map(|e| q.owner(&e)).filter_map(|o| if let Ent::Body(b) = o { Some(st.ctx.bodies[b].id.clone()) } else { None }).collect();
        let mut defs = Vec::new();
        let mut def_refs = Vec::new();
        for (b, body) in st.ctx.bodies.iter().enumerate() {
            let body_ref = q.to_ref(&Ent::Body(b));
            let Some(model) = sm_attribute(&st, &body_ref, "MODEL") else { continue };
            if !changed_owners.is_empty() && !changed_owners.contains(&body.id) {
                continue;
            }
            let (bends, rips, rolled, rip_styles, joint_ids) = joints(&st, &q, b);
            // The part made from it before: the solid carrying its association.
            let own = associations(&st, &body_ref);
            let previous: Vec<BodyId> = st.ctx.bodies.iter().enumerate().filter(|(pb, p)| p.kind == BodyKind::Solid && !own.is_empty() && associations(&st, &q.to_ref(&Ent::Body(*pb))).iter().any(|x| own.contains(x))).map(|(_, p)| p.id.clone()).collect();
            let mut forms = Vec::new();
            let mut wall_ids = BTreeMap::new();
            for f in 0..body.faces.len() {
                if let Some(wall) = sm_attribute(&st, &q.to_ref(&Ent::Face(b, f)), "WALL") {
                    forms.extend(form_tools(&st, &wall, f, &mut tool_ids));
                    if let Some(id) = wall.get("attributeId").as_str() {
                        wall_ids.insert(f, id.to_string());
                    }
                }
            }
            defs.push(Definition { forms, body: body.id.clone(), front: length_field(&model, "frontThickness"), back: length_field(&model, "backThickness"), bends, rolled, rips, rip_styles, joint_ids, clearance: length_field(&model, "minimalClearance"), relief: BendRelief::of(&model), corner: CornerRelief::of(&model), corners: corner_overrides(&st, &q, b, &CornerRelief::of(&model)), reliefs: relief_overrides(&st, &q, b, &BendRelief::of(&model)), k_factor: model.get("k-factor").get("value").as_number().unwrap_or(0.45), active: model.get("active").as_bool().unwrap_or(true), default_radius: length_field(&model, "defaultBendRadius"), previous, wall_ids, breaks: corner_breaks(&st, &q, b) });
            let faces: Vec<Reference> = (0..body.faces.len()).map(|f| q.to_ref(&Ent::Face(b, f))).collect();
            let edges: Vec<Reference> = (0..body.edges.len()).map(|e| q.to_ref(&Ent::Edge(b, e))).collect();
            let vertices: Vec<Reference> = (0..body.vertices.len()).map(|v| q.to_ref(&Ent::Vertex(b, v))).collect();
            // Its bend centerline wires from before: the wires associated with its edges or faces.
            let def_assoc: Vec<Value> = faces.iter().chain(&edges).flat_map(|r| associations(&st, r)).collect();
            let wires: Vec<BodyId> = st
                .ctx
                .bodies
                .iter()
                .enumerate()
                .filter(|(wb, w)| w.kind == BodyKind::Wire && !def_assoc.is_empty() && associations(&st, &q.to_ref(&Ent::Body(*wb))).iter().any(|x| def_assoc.contains(x)))
                .map(|(_, w)| w.id.clone())
                .collect();
            def_refs.push((body_ref, faces, edges, vertices, wires));
        }
        (defs, def_refs)
    };
    if defs.is_empty() {
        return Ok(Value::Undefined);
    }
    let op: FsId = id.iter().cloned().chain(std::iter::once(Arc::<str>::from("smGeometry"))).collect();
    let built = run_op(i, &cx, &op, "SHEET_METAL_REBUILD_ERROR", |ev| {
        let mut out = Vec::new();
        let mut fresh = 0u32;
        for (n, def) in defs.iter().enumerate() {
            let Some(bi) = ev.ctx.bodies.iter().position(|b| b.id == def.body) else { continue };
            let shape = ev.ctx.bodies[bi].shape.clone();
            let Built { mut part, mut pieces, topo, wall_flats, strips } = build_part(def, &shape)?;
            let births = births(&mut ev.ctx.sm_births, def, &topo);
            // Forms: the parts to remove cut, then the parts to add united;
            // their faces belong to the wall they sit on.
            let mut form_shapes: Vec<FormShapes> = Vec::new();
            let mut cut_shapes: Vec<(usize, Vec<Shape>)> = Vec::new();
            for ft in &def.forms {
                let shapes = |ids: &[BodyId]| -> Vec<Shape> { ids.iter().filter_map(|id| ev.ctx.bodies.iter().find(|b| &b.id == id)).map(|b| b.shape.clone()).collect() };
                let (adds, removes) = (shapes(&ft.add), shapes(&ft.remove));
                if !removes.is_empty() {
                    part = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&part), &removes, &k::BooleanOptions::default()).map_err(|e| fe(format!("The form failed: {e}")))?.shape;
                }
                if !adds.is_empty() {
                    part = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(&part), &adds, &k::BooleanOptions::default()).map_err(|e| fe(format!("The form failed: {e}")))?.shape;
                }
                let n_removes = removes.len();
                let tools: Vec<Shape> = removes.into_iter().chain(adds).collect();
                pieces.extend(tools.iter().flat_map(|t| t.faces()).map(|face| Piece { face, from: Def::Face(ft.face) }));
                if ft.cut {
                    cut_shapes.push((ft.face, tools));
                } else {
                    let sketches: Vec<_> = ft.sketches.iter().filter_map(|f| ev.ctx.sketches.iter().find(|s| s.feature == *f)).collect();
                    let sketch = FormSketch { curves: sketches.iter().flat_map(|s| sketch_curves(s)).collect(), origin: sketches.first().map(|s| s.frame.origin) };
                    form_shapes.push((ft.face, tools, sketch, n_removes));
                }
            }
            apply_corner_breaks(&mut part, &mut pieces, def, &topo);
            // The definition body's own topology order (references are by index).
            let def_topo = (ev.ctx.bodies[bi].faces.len(), ev.ctx.bodies[bi].edges.len());
            if def_topo != (topo.faces.len(), topo.edges.len()) {
                return Err(fe("The sheet metal definition changed while rebuilding"));
            }
            // The parts this rebuild replaces (by `previous`' order).
            let before: Vec<Option<Arc<crate::regen::context::Body>>> = def.previous.iter().map(|id| ev.ctx.bodies.iter().find(|b| &b.id == id).cloned()).collect();
            let old_wires = def_refs.get(n).map(|r| &r.4).cloned().unwrap_or_default();
            ev.ctx.bodies.retain(|b| !def.previous.contains(&b.id) && !old_wires.contains(&b.id));
            // Walls joined only by rips are separate parts (live G34d), first
            // the one holding the lowest definition face.
            let solids = part.solids();
            let solids = if solids.len() > 1 { solids } else { vec![part] };
            let mut parts: Vec<PartOf> = solids
                .into_iter()
                .map(|solid| {
                    let sources = part_sources(&solid, &pieces, &topo);
                    let mut members: Vec<usize> = sources.0.iter().filter_map(|d| if let Def::Face(f) = d { Some(*f) } else { None }).collect();
                    members.sort_unstable();
                    members.dedup();
                    PartOf { solid, names: Vec::new(), sources, members }
                })
                .collect();
            parts.sort_by_key(|p| p.members.first().copied().unwrap_or(usize::MAX));
            let mut others = BTreeMap::new();
            let mut taken: std::collections::HashSet<crate::naming::Name> = ev.ctx.bodies.iter().flat_map(|b| b.faces.iter().cloned()).collect();
            for (k, p) in parts.iter_mut().enumerate() {
                p.names = super::ops::fresh_names(ev, &p.solid);
                // Fresh faces are named by their order in the solid, so the
                // part of another model (or a second part of this one) built
                // in the same feature named its faces as the first did: their
                // edges shared names, and every attribute std put on one
                // part's entity was on the other's too (two association
                // attributes per edge; SM-MAKE-JOINT-ONE-EDGE). Names taken
                // already get this part's own (a lone part keeps its names).
                if p.names.iter().any(|x| taken.contains(x)) {
                    let instance = def.body.index << 8 | k as u32;
                    p.names = p.names.iter().map(|x| Arc::new(crate::reference::TopoName::face(ev.feature, crate::reference::Role::Copy { instance, of: x.clone() }))).collect();
                }
                if let Some(Some(b)) = before.get(k) {
                    keep_names(&p.solid, &mut p.names, &b.shape, &b.faces, ev.feature);
                }
                taken.extend(p.names.iter().cloned());
                let part_faces = p.solid.faces();
                for &fi in &p.members {
                    if let Some(n) = wall_faces(&topo, fi, &part_faces, &p.named(Def::Face(fi))).0.first() {
                        others.insert(fi, n.clone());
                    }
                }
            }
            for (k, p) in parts.into_iter().enumerate() {
                let walls: Vec<(usize, Shape)> = wall_flats.iter().filter(|w| p.members.contains(&w.0)).cloned().collect();
                let own: Vec<Strip> = strips.iter().filter(|s| p.members.contains(&s.wall_a)).cloned().collect();
                let own_forms: Vec<FormShapes> = form_shapes.iter().filter(|f| p.members.contains(&f.0)).cloned().collect();
                let own_cuts: Vec<(usize, Vec<Shape>)> = cut_shapes.iter().filter(|f| p.members.contains(&f.0)).cloned().collect();
                let flat = flat_of(def, &topo, &walls, &own, &own_forms, &own_cuts);
                // A centerline wire per bend, in the flat (live 2015: "Bend line 1", hidden), for std (pcbEnd).
                let mut wires = Vec::new();
                for line in flat.iter().flat_map(|f| f.bend_lines.iter().map(move |l| (f, l))) {
                    let (f, l) = line;
                    let Some(strip) = own.get(l.bend) else { continue };
                    let Ok(shape) = k::make_line(f.frame.to_world(l.a), f.frame.to_world(l.b)) else { continue };
                    fresh += 1;
                    let wid = BodyId { feature: ev.feature, index: fresh - 1 };
                    push_body(ev, wid.clone(), BodyKind::Wire, shape, Vec::new());
                    wires.push((wid, strip.rolled.map_or(Def::Edge(strip.edge), Def::Face), l.up));
                }
                let data = sheet_data(def, &topo, &own, &p, flat.as_ref(), &others, &births);
                let id = def.previous.get(k).cloned().unwrap_or_else(|| {
                    fresh += 1;
                    BodyId { feature: ev.feature, index: fresh - 1 }
                });
                // A rebuilt part keeps its name and colour (a fresh colour
                // came out orange after std's miter tools took the next ones).
                match before.get(k) {
                    Some(Some(b)) => ev.ctx.bodies.push(Arc::new(crate::regen::context::Body::new(id.clone(), b.name.clone(), BodyKind::Solid, p.solid, p.names, b.color, ev.feature))),
                    _ => push_body(ev, id.clone(), BodyKind::Solid, p.solid, p.names),
                }
                out.push((n, id, p.sources, flat, data, wires));
            }
        }
        Ok(out)
    })?;
    // Associations and hiding.
    let mut st = cx.state.lock();
    let (marker, feature) = (super::marker_of(&op), st.feature);
    // Form tools go with the model (hidden; std deletes the ones left
    // public), named by ids that last for the features after this one.
    let mut renamed: HashMap<Arc<str>, Arc<str>> = HashMap::new();
    for (old, r) in &tool_ids {
        let new = st.persist_transient(r);
        if *old != new {
            renamed.insert(old.clone(), new);
        }
        if let Reference::Body(b) = r {
            st.hidden.insert(b.clone());
        }
    }
    if !renamed.is_empty() {
        for (_, v) in st.attributes_mut().iter_mut() {
            if v.get("attribute").get("formedToolBodyIds").as_array().is_some() || v.get("attribute").get("cuttingToolBodyIds").as_array().is_some() {
                *v = rename_strings(v, &renamed);
            }
        }
    }
    for (n, part_id, (faces, edges, vertices), flat, data, wires) in built {
        let Some((body_ref, dfaces, dedges, dverts, _)) = def_refs.get(n) else { continue };
        // New parts were renamed to the feature after the operation.
        let part_id = super::wrap_body_id(&part_id, marker, st.op_bits(marker), feature);
        // Its face names as the operation's bodies were renamed after it.
        let mut data = data;
        let wrap = |v: &mut Vec<crate::naming::Name>| v.iter_mut().for_each(|n| *n = super::wrap_name(n, marker, feature));
        for w in &mut data.walls {
            wrap(&mut w.faces);
            wrap(&mut w.back);
        }
        for b in &mut data.bends {
            wrap(&mut b.inner_faces);
            wrap(&mut b.outer_faces);
        }
        for r in &mut data.rips {
            wrap(&mut r.faces);
            if let Some(o) = &mut r.other {
                *o = super::wrap_name(o, marker, feature);
            }
        }
        st.ctx.sheet_metal.insert(part_id.clone(), Arc::new(data));
        match flat {
            Some(mut f) => {
                f.built_for = st.ctx.bodies.iter().find(|b| b.id == part_id).map(|b| crate::sheetmetal::flat::body_key(&b.shape));
                st.ctx.sheet_metal_flats.insert(part_id.clone(), Arc::new(f));
            }
            None => {
                st.ctx.sheet_metal_flats.remove(&part_id);
            }
        }
        let Some(pb) = st.ctx.bodies.iter().position(|b| b.id == part_id) else { continue };
        let part = st.ctx.bodies[pb].clone();
        let def_ref = |d: &Def| match *d {
            Def::Face(x) => dfaces.get(x).cloned(),
            Def::Edge(x) => dedges.get(x).cloned(),
            Def::Vertex(x) => dverts.get(x).cloned(),
        };
        let mut links: Vec<(Reference, Reference)> = vec![(Reference::Body(part_id.clone()), body_ref.clone())];
        links.extend(faces.iter().enumerate().filter_map(|(f, d)| Some((Reference::Face((*part.faces[f]).clone()), def_ref(d)?))));
        links.extend(edges.iter().enumerate().filter_map(|(e, d)| Some((Reference::Edge((*part.edges.get(e)?.clone()).clone()), def_ref(d)?))));
        links.extend(vertices.iter().enumerate().filter_map(|(v, d)| Some((Reference::Vertex((*part.vertices.get(v)?.clone()).clone()), def_ref(d)?))));
        // All read before any is added (each addition drops the attributes' index).
        let assoc: Vec<Vec<Value>> = links.iter().map(|(_, from)| associations(&st, from)).collect();
        for ((to, _), attrs) in links.into_iter().zip(assoc) {
            for attr in attrs {
                st.push_attribute(to.clone(), Value::map_from([("attribute", attr)]));
            }
        }
        if let Reference::Body(def_body) = body_ref {
            st.hidden.insert(def_body.clone());
            // The bend centerlines: hidden, with their bend's definition entity's association.
            for (wid, d, _) in &wires {
                let wid = super::wrap_body_id(wid, marker, st.op_bits(marker), feature);
                if let Some(from) = def_ref(d) {
                    for attr in associations(&st, &from) {
                        st.push_attribute(Reference::Body(wid.clone()), Value::map_from([("attribute", attr)]));
                    }
                }
                st.hidden.insert(wid);
            }
        }
    }
    Ok(Value::Undefined)
}

/// evSheetMetalBendUp: whether a bend centerline wire's bend goes up (toward the viewer of the flat, the definition
/// face's normal side; live 2015): the flat pattern's bend line the wire lies on.
fn ev_sheet_metal_bend_up(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let arg = a.get(1).cloned().unwrap_or_default();
    let wires = evaluate_refs(i, &cx, arg.get("wireBody"))?;
    let st = cx.state.lock();
    let ends = |b: &BodyId| st.ctx.bodies.iter().find(|x| &x.id == b).and_then(|x| x.shape.edges().first().and_then(|e| e.endpoints().ok()));
    let up = wires.iter().find_map(|r| {
        let Reference::Body(b) = r else { return None };
        let (p, q) = ends(b)?;
        st.ctx.sheet_metal_flats.values().flat_map(|f| f.bend_lines.iter().map(move |l| (f.frame.to_world(l.a), f.frame.to_world(l.b), l.up))).find(|(a, b, _)| {
            (a.distance(p) < 1e-9 && b.distance(q) < 1e-9) || (a.distance(q) < 1e-9 && b.distance(p) < 1e-9)
        }).map(|x| x.2)
    });
    drop(st);
    match up {
        Some(up) => Ok(Value::bool(up)),
        None => Err(super::raise_error(i, "INVALID_INPUT", None)),
    }
}

// ---------------------------------------------------------------- flat pattern

/// The flat pattern's placement of each wall (by definition face) and bend
/// (by definition edge). Live (dom/fs-smprobe-{1,2,3}.txt,
/// evSheetMetalFlatTransformation): the first wall turns by the smallest
/// rotation taking its definition normal to -z and moves along z until its
/// definition plane is z = 0 (sm1: (0, 0, -2 mm); sm3: 90° about x). A wall
/// across a bend unfolds about the bend axis by the bend angle, then moves
/// back by the developed length BA = θ (r + K T) along its neighbour's
/// direction (sm2's flange: y' = 1.0155 mm - z); a bend face gets half of it
/// (sm2: translation (0, 0.921957, -1.828427) mm).
/// The part as the app's sheet metal data (table, panel; its flat pattern is
/// the std-built one): walls by definition face (the faces seen in the flat
/// first), bends in strip order with up as in the flat, rips.
/// `others`: the reference-side face of every wall, for rips to a wall in
/// another part.
/// When each definition edge and rolled face appeared (see
/// [`crate::regen::context::Context::sm_births`]): new ones registered in
/// definition order. Returns the births by edge index and by rolled face.
fn births(seen: &mut Vec<Vec<i64>>, def: &Definition, topo: &Topo) -> (Vec<u64>, BTreeMap<usize, u64>) {
    let q = |v: f64| (v * 1e7).round() as i64;
    let pair = |a: DVec3, b: DVec3| {
        let (a, b) = ([q(a.x), q(a.y), q(a.z)], [q(b.x), q(b.y), q(b.z)]);
        if a <= b { [a, b] } else { [b, a] }
    };
    let boxed = |f: usize| topo.faces.get(f).and_then(|f| f.bounding_box(false).ok()).map_or([0; 6], |b| {
        let [a, b] = [b.min, b.max].map(|p| [q(p.x), q(p.y), q(p.z)]);
        [a[0], a[1], a[2], b[0], b[1], b[2]]
    });
    // `a` holds `b` (1 unit slack).
    let holds = |a: &[i64], b: &[i64]| (0..3).all(|k| a[k] <= b[k] + 1 && a[k + 3] >= b[k + 3] - 1);
    // An entry: [kind, its end points or box, then its faces' boxes]. An
    // edge is the one seen before when its ends and face count are the same
    // and each face only grew (live qa5: a Tab merged into the plate kept
    // the plate's bends, the Hem's trim of the plate renewed them; a
    // Flange's new wall renews the edge it stands on).
    // A face a cut split (one of several coplanar pieces inside the old
    // face's box) is kept too (live qa4 S1: a cut across the plate kept its
    // bend as Bend A); one face that only shrank is not.
    let boxes: Vec<[i64; 6]> = (0..topo.faces.len()).map(boxed).collect();
    let planes: Vec<Option<(DVec3, DVec3)>> = topo.faces.iter().map(|f| f.face_plane().ok().flatten().and_then(|_| face_normal(f).ok())).collect();
    let split = |f: usize, o: &[i64]| {
        let Some((p, n)) = planes[f] else { return false };
        holds(o, &boxes[f]) && (0..topo.faces.len()).any(|g| g != f && holds(o, &boxes[g]) && planes[g].is_some_and(|(pg, ng)| ng.dot(n) > 1.0 - 1e-9 && (pg - p).dot(n).abs() < TOL))
    };
    let kept = |f: u32, o: &[i64]| holds(&boxes[f as usize], o) || split(f as usize, o);
    let same = |x: &Vec<i64>, head: &[i64], faces: &[u32]| {
        if x.len() != head.len() + 6 * faces.len() || x[..head.len()] != *head {
            return false;
        }
        let old: Vec<&[i64]> = x[head.len()..].chunks(6).collect();
        match faces.len() {
            0 => true,
            2 => (kept(faces[0], old[0]) && kept(faces[1], old[1])) || (kept(faces[0], old[1]) && kept(faces[1], old[0])),
            _ => faces.iter().zip(&old).all(|(f, o)| kept(*f, o)),
        }
    };
    let heads: Vec<Vec<i64>> = (0..topo.edge_ends.len())
        .map(|e| {
            let [a, b] = pair(topo.edge_ends[e].0, topo.edge_ends[e].1);
            [0, a[0], a[1], a[2], b[0], b[1], b[2]].to_vec()
        })
        .collect();
    let keys: Vec<(Vec<i64>, Vec<[i64; 6]>)> = heads.iter().enumerate().map(|(e, h)| (h.clone(), topo.edge_faces[e].iter().map(|f| boxes[*f as usize]).collect())).collect();
    let found: Vec<Option<usize>> = (0..keys.len()).map(|e| seen.iter().position(|x| same(x, &heads[e], &topo.edge_faces[e]))).collect();
    // New ones: those renewed in place (same ends, a face rebuilt) first,
    // then moved and new ones, each in definition order (live qa5: the Hem
    // renewed x = 0 before y = 0, whose end it moved, then made its own).
    let in_place = |h: &[i64]| seen.iter().any(|x| x.len() >= h.len() && x[..h.len()] == *h);
    let mut order: Vec<usize> = (0..keys.len()).filter(|&e| found[e].is_none()).collect();
    order.sort_by_key(|&e| (!in_place(&keys[e].0), e));
    let entry = |(h, f): &(Vec<i64>, Vec<[i64; 6]>)| h.iter().copied().chain(f.iter().flatten().copied()).collect::<Vec<i64>>();
    let mut edges = vec![0u64; keys.len()];
    for (e, i) in found.iter().enumerate() {
        if let Some(i) = *i {
            seen[i] = entry(&keys[e]);
            edges[e] = i as u64;
        }
    }
    for e in order {
        seen.push(entry(&keys[e]));
        edges[e] = seen.len() as u64 - 1;
    }
    let faces = def
        .rolled
        .keys()
        .filter(|&&fi| fi < topo.faces.len())
        .map(|&fi| {
            let head: Vec<i64> = [1].into_iter().chain(boxed(fi)).collect();
            let i = seen.iter().position(|x| *x == head).unwrap_or_else(|| {
                seen.push(head);
                seen.len() - 1
            });
            (fi, i as u64)
        })
        .collect();
    (edges, faces)
}

#[allow(clippy::too_many_arguments)]
fn sheet_data(def: &Definition, topo: &Topo, strips: &[Strip], part: &PartOf, flat: Option<&crate::sheetmetal::FlatPattern>, others: &BTreeMap<usize, crate::naming::Name>, births: &(Vec<u64>, BTreeMap<usize, u64>)) -> crate::sheetmetal::SheetMetalData {
    use crate::sheetmetal::{Bend as SmBend, BendKind, BendReliefStyle, Rip, SheetMetalData, Wall};
    let thickness = def.front + def.back;
    let mut data = SheetMetalData::new(def.body.feature, thickness, def.default_radius, def.k_factor);
    data.active = def.active;
    data.minimal_clearance = def.clearance;
    data.bend_relief = BendReliefStyle::from_id(&def.relief.style);
    data.bend_relief_depth_scale = def.relief.depth;
    data.bend_relief_width_scale = def.relief.width;
    data.corner_relief = def.corner.style.clone();
    let part_faces = part.solid.faces();
    let named = |d: Def| part.named(d);
    // Walls by definition face of this part (a rolled bend's faces are the bend's).
    let index = |fi: usize| part.members.iter().position(|m| *m == fi);
    for &fi in &part.members {
        let (seen, back) = if def.rolled.contains_key(&fi) { (Vec::new(), Vec::new()) } else { wall_faces(topo, fi, &part_faces, &named(Def::Face(fi))) };
        data.walls.push(Wall { feature: def.body.feature, faces: seen, back });
    }
    for (k, st) in strips.iter().enumerate() {
        let (mut inner, mut outer) = (Vec::new(), Vec::new());
        for (i, name) in named(st.rolled.map_or(Def::Edge(st.edge), Def::Face)) {
            match part_faces.get(i).and_then(|f| f.face_surface().ok()) {
                Some(k::Surface::Cylinder { radius, .. }) if radius < st.radius + thickness * 0.5 => inner.push(name),
                Some(k::Surface::Cylinder { .. }) => outer.push(name),
                _ => {}
            }
        }
        let up = flat.and_then(|f| f.bend_lines.get(k)).is_some_and(|l| l.up);
        data.bends.push(SmBend {
            feature: def.body.feature,
            kind: if st.rolled.is_some() { BendKind::Hem } else { BendKind::Bend },
            parent: index(st.wall_a).unwrap_or(0),
            child: index(st.wall_b),
            origin: st.origin,
            axis: st.axis,
            angle: st.angle,
            radius: st.radius,
            k_factor: def.k_factor,
            dir_a: st.dir_a,
            radial_a: st.radial_a,
            s0: st.extent.0,
            s1: st.extent.1,
            ref_inner: up,
            inner_faces: inner,
            outer_faces: outer,
            flat: false,
            birth: match st.rolled {
                Some(fi) => births.1.get(&fi).copied().unwrap_or(u64::MAX),
                None => births.0.get(st.edge).copied().unwrap_or(u64::MAX),
            },
            joint: match st.rolled {
                Some(fi) => def.joint_ids.1.get(&fi).cloned(),
                None => def.joint_ids.0.get(&st.edge).cloned(),
            },
        });
    }
    for &e in def.rips.iter().filter(|&&e| !coplanar_rip(topo, e)) {
        let fs: Vec<usize> = topo.edge_faces.get(e).map(|v| v.iter().map(|x| *x as usize).collect()).unwrap_or_default();
        // Wall a in this part; wall b here or (`usize::MAX`) in another part.
        let Some(a) = fs.iter().copied().find(|f| index(*f).is_some()) else { continue };
        let b = fs.iter().copied().find(|f| *f != a);
        let b_here = b.and_then(index);
        data.rips.push(Rip {
            feature: def.body.feature,
            a: index(a).unwrap_or(0),
            b: b_here.unwrap_or(usize::MAX),
            style: def.rip_styles.get(&e).cloned().unwrap_or_else(|| "EDGE".into()),
            gap: def.clearance,
            other: if b_here.is_none() { b.and_then(|b| others.get(&b).cloned()) } else { None },
            faces: named(Def::Edge(e)).into_iter().map(|x| x.1).collect(),
            birth: births.0.get(e).copied().unwrap_or(u64::MAX),
            joint: def.joint_ids.0.get(&e).cloned(),
        });
    }
    data
}

/// How far wall `f` stops short of rip `e` (negative: runs past it). Edge
/// joint: both walls the minimal gap (live G34d). Butt joint (live G34b,
/// G34d with the Joint feature): the wall running through (Direction 1: the
/// first in definition order, Direction 2 the other) covers the other's
/// thickness; the other stops the gap short of it.
/// A rip between two coplanar pieces of one wall (live, behaviours.md
/// "fs (10-23 17:30)": a diagonal rip across a flat plate leaves the part as
/// it was: no gap, no joint in the table).
fn coplanar_rip(topo: &Topo, e: usize) -> bool {
    let [a, b] = topo.edge_faces[e][..] else { return false };
    match (face_normal(&topo.faces[a as usize]), face_normal(&topo.faces[b as usize])) {
        (Ok((pa, na)), Ok((pb, nb))) => na.dot(nb) > 1.0 - 1e-9 && (pb - pa).dot(na).abs() < TOL,
        _ => false,
    }
}

fn rip_offset(def: &Definition, topo: &Topo, e: usize, f: usize) -> f64 {
    if coplanar_rip(topo, e) {
        return 0.0;
    }
    let walls = &topo.edge_faces[e];
    let Some(other) = walls.iter().map(|x| *x as usize).find(|x| *x != f) else { return def.clearance };
    // Direction 1: the wall std made first runs through (live: the wall of
    // the first-picked flange edge, request 1599 (b)); walls of different
    // features by definition face.
    let made = |w: usize| def.wall_ids.get(&w).and_then(|id| id.rsplit_once('.')).and_then(|(feature, n)| Some((feature, n.parse::<u64>().ok()?)));
    let first = match (made(f), made(other)) {
        (Some((a, m)), Some((b, n))) if a == b => if m < n { f } else { other },
        _ => f.min(other),
    };
    let through = if def.rip_styles.get(&e).is_some_and(|s| s == "BUTT2") { first != f } else { first == f };
    let (s, t) = topo.edge_ends[e];
    let dir = (t - s).normalize_or_zero();
    // A wall's in-plane direction from its inside toward the rip.
    let toward = |w: usize| -> Option<DVec3> {
        let (p, n) = face_normal(&topo.faces[w]).ok()?;
        let u = n.cross(dir).normalize_or_zero();
        Some(if (p - s).dot(u) > 0.0 { -u } else { u })
    };
    // Where a wall's material lies along `u`, from its definition plane.
    let span = |w: usize, u: DVec3| -> Option<(f64, f64)> {
        let n = face_normal(&topo.faces[w]).ok()?.1;
        let (a, b) = (-def.back * n.dot(u), def.front * n.dot(u));
        Some((a.min(b), a.max(b)))
    };
    let (Some(uf), Some(uo)) = (toward(f), toward(other)) else { return def.clearance };
    // An edge joint: both walls stop short by the clearance over one less
    // the cosine between their directions in from the rip (toward it, alike) (live: 0.02 mm at
    // 90°, fs-smprobe3-{4,6}; 0.018071 mm at a loft's 96.1° seam, request 1612).
    // Where the other wall's material reaches in before the rip (its thickness on this wall's side: a converted box's
    // corner, material inward), the wall stops short of that material instead (live c2-sm-corner-break: both walls
    // 0.02 mm short of the other's inner face, the corner square open).
    if !def.rip_styles.contains_key(&e) {
        let reach = span(other, uf).map_or(0.0, |(lo, _)| -lo.min(0.0));
        return def.clearance / (1.0 - uf.dot(uo)).max(1e-9) + reach;
    }
    if through {
        span(other, uf).map_or(0.0, |(_, hi)| -hi.max(0.0))
    } else {
        span(other, uo).map_or(def.clearance, |(lo, _)| def.clearance - lo.min(0.0))
    }
}

/// One part built from a definition sheet: its solid, face names, what each
/// face comes from, and its definition faces (walls and rolled bends).
struct PartOf {
    solid: Shape,
    names: Vec<crate::naming::Name>,
    sources: (Vec<Def>, Vec<Def>, Vec<Def>),
    members: Vec<usize>,
}

impl PartOf {
    /// This part's faces made from definition entity `d` (index, name).
    fn named(&self, d: Def) -> Vec<(usize, crate::naming::Name)> {
        self.sources.0.iter().enumerate().filter(|(_, x)| **x == d).filter_map(|(i, _)| Some((i, self.names.get(i)?.clone()))).collect()
    }
}

/// A wall's faces: those seen from the reference side (opposite the
/// definition normal) and the back ones.
fn wall_faces(topo: &Topo, fi: usize, part_faces: &[Shape], named: &[(usize, crate::naming::Name)]) -> (Vec<crate::naming::Name>, Vec<crate::naming::Name>) {
    let n = face_normal(&topo.faces[fi]).map(|x| x.1).unwrap_or(DVec3::Z);
    let (mut seen, mut back) = (Vec::new(), Vec::new());
    for (i, name) in named {
        let fnormal = part_faces.get(*i).and_then(|f| face_normal(f).ok()).map(|x| x.1).unwrap_or(n);
        if fnormal.dot(n) < 0.0 { seen.push(name.clone()) } else { back.push(name.clone()) }
    }
    (seen, back)
}

/// The flat pattern of a built part (live's placement: the first wall's
/// definition plane at z = 0, seen from +z; dom/qa5-sm-flat.dxf): walls and
/// bend strips unfolded, bend lines on the strips' centres, up when the bend
/// rises toward the viewer.
/// A form on wall `.0`: its tools (the first `.3` cut, the rest added),
/// and its flat sketch.
type FormShapes = (usize, Vec<Shape>, FormSketch, usize);

/// A form's flat sketch in model space (live 1623): its curves (`true` a
/// tear: the sketch's own geometry; else an outline: its construction
/// geometry) and its origin, the form's point.
#[derive(Clone, Debug, Default)]
struct FormSketch {
    curves: Vec<(Vec<DVec3>, bool)>,
    origin: Option<DVec3>,
}

/// A sketch's curves as points in model space, each with whether it is a
/// tear (not construction).
fn sketch_curves(s: &crate::regen::context::SketchResult) -> Vec<(Vec<DVec3>, bool)> {
    let f = &s.frame;
    s.sketch
        .entities()
        .filter_map(|e| {
            let pts = s.sketch.curve(e.id)?.tessellate(1e-6);
            (pts.len() >= 2).then(|| (pts.into_iter().map(|p| f.origin + f.x * p.x + f.y * p.y).collect(), !e.construction))
        })
        .collect()
}

fn flat_of(def: &Definition, topo: &Topo, wall_flats: &[(usize, Shape)], strips: &[Strip], forms: &[FormShapes], cuts: &[(usize, Vec<Shape>)]) -> Option<crate::sheetmetal::FlatPattern> {
    let unfolded = unfold(def, def.k_factor, topo).ok()?;
    let thickness = def.front + def.back;
    let frame = k::Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X);
    let mut pieces = Vec::new();
    let mut heads = Vec::new();
    for (fi, face) in wall_flats {
        let m = unfolded.walls.get(fi)?;
        let mut face = face.clone();
        // The wall's cutting tools (a counterbore Hole): the flat loses what
        // goes through the sheet (the tool's sections at both faces, where
        // they overlap); a bigger section on one face is the head, drawn as
        // a circle (live dx 10-27 flat pattern line styles).
        if let Ok(Some(plane)) = face.face_plane() {
            let (o, n, x) = (plane.origin, plane.normal.normalize(), plane.x_dir);
            let eps = 1e-3 * thickness;
            for tool in cuts.iter().filter(|c| c.0 == *fi).flat_map(|c| c.1.iter()) {
                let cap = |h: f64| k::section_caps(tool, &k::Plane::new(o + n * h, n, x)).ok().filter(|c| c.count(ShapeType::Face) > 0).and_then(|c| c.translated(-n * h).ok());
                let (Some(front), Some(back)) = (cap(def.front - eps), cap(-def.back + eps)) else { continue };
                let Ok(through) = k::boolean(k::BooleanOp::Common, std::slice::from_ref(&front), std::slice::from_ref(&back), &k::BooleanOptions::default()) else { continue };
                if let Ok(r) = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&face), std::slice::from_ref(&through.shape), &k::BooleanOptions::default()) {
                    face = r.shape;
                }
                let area = |s: &Shape| s.area().unwrap_or(0.0);
                let (af, ab) = (area(&front), area(&back));
                if (af - ab).abs() > 1e-3 * af.max(ab) {
                    let (head, side) = if af > ab { (front, n) } else { (back, -n) };
                    let Ok(bb) = head.bounding_box(false) else { continue };
                    let c = m.transform_point3(bb.center());
                    let size = bb.max - bb.min;
                    let radius = (size - n * size.dot(n)).abs().max_element() * 0.5;
                    heads.push(crate::sheetmetal::flat::FlatHead { center: frame.to_local(c), radius, up: m.transform_vector3(side).z > 0.0 });
                }
            }
        }
        pieces.push(k::transform(&face, &k::Transform::Matrix(*m), true).ok()?.shape);
    }
    // Forms: their footprint fills the flat, outlined with a center mark (as the native route).
    let mut flat_forms = Vec::new();
    for (fi, tools, sketched, n_removes) in forms {
        let (Some(m), Some((_, wall))) = (unfolded.walls.get(fi), wall_flats.iter().find(|w| w.0 == *fi)) else { continue };
        let Ok(Some(plane)) = wall.face_plane() else { continue };
        let (o, n, x) = (plane.origin, plane.normal.normalize(), plane.x_dir);
        let tools_in = tools;
        let tools = if tools.len() == 1 { tools[0].clone() } else { Shape::compound(tools) };
        let to_plane = |p: DVec3| p - n * (p - o).dot(n);
        let extent = |l: &Vec<DVec3>| {
            let (lo, hi) = l.iter().fold((DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
            (hi - lo).length()
        };
        let mut loops: Vec<Vec<DVec3>> = Vec::new();
        for h in [-def.back - 0.5 * thickness, -0.5 * def.back, 0.0, 0.5 * def.front, def.front + 0.5 * thickness] {
            let mut ls: Vec<Vec<DVec3>> = Vec::new();
            for w in k::section_wires(&tools, &k::Plane::new(o + n * h, n, x)).unwrap_or_default() {
                let pts = crate::sheetmetal::flat::wire_points(&w);
                if pts.len() >= 3 {
                    ls.push(pts.into_iter().map(to_plane).collect());
                }
            }
            ls.sort_by(|a, b| extent(b).total_cmp(&extent(a)));
            // The add and subtract tools often share an outline at the sheet: once.
            let same = |a: &Vec<DVec3>, b: &Vec<DVec3>| (extent(a) - extent(b)).abs() < 1e-9 && a.iter().copied().sum::<DVec3>().distance(b.iter().copied().sum::<DVec3>()) / (a.len().max(1) as f64) < 1e-7;
            let mut kept: Vec<Vec<DVec3>> = Vec::new();
            for l in ls {
                if !kept.iter().any(|k| same(k, &l)) {
                    kept.push(l);
                }
            }
            let ls = kept;
            if ls.first().map(extent).unwrap_or(0.0) > loops.first().map(extent).unwrap_or(0.0) + 1e-9 {
                loops = ls;
            }
        }
        let mid = 0.5 * (def.front - def.back);
        if let Ok(fill) = k::section_caps(&tools, &k::Plane::new(o + n * mid, n, x))
            && fill.count(ShapeType::Face) > 0
            && let Ok(fill) = fill.translated(-n * mid)
            && let Ok(fill) = k::transform(&fill, &k::Transform::Matrix(*m), true)
        {
            pieces.extend(fill.shape.faces());
        }
        // A form with a flat sketch shows only that (live 1623), centred on
        // the form's point; else its tools' widest sections.
        let center = match (sketched.origin.filter(|_| !sketched.curves.is_empty()), loops.first()) {
            (Some(o), _) => to_plane(o),
            (None, Some(first)) => first.iter().copied().sum::<DVec3>() / first.len() as f64,
            (None, None) => continue,
        };
        if !sketched.curves.is_empty() {
            loops.clear();
        }
        let curves: Vec<(Vec<DVec3>, bool)> = sketched.curves.iter().map(|(l, tear)| (l.iter().copied().map(to_plane).collect(), *tear)).collect();
        // Up: the tools stand out of the wall toward the flat's viewer (+z).
        // The side they reach furthest past the sheet ([-back, front] along
        // n): a form's cut tool runs through the sheet, its added material
        // stands out on one side, so that decides when there is any (a cut
        // reaching as far the other way left it to rounding).
        let added = &tools_in[(*n_removes).min(tools_in.len())..];
        let reach = if added.is_empty() { tools.clone() } else if added.len() == 1 { added[0].clone() } else { Shape::compound(added) };
        let side = reach
            .bounding_box(false)
            .map(|b| {
                let ds: Vec<f64> = (0..8).map(|i| DVec3::new([b.min.x, b.max.x][i & 1], [b.min.y, b.max.y][(i >> 1) & 1], [b.min.z, b.max.z][(i >> 2) & 1])).map(|c| (c - o).dot(n)).collect();
                let (lo, hi) = ds.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), d| (lo.min(*d), hi.max(*d)));
                (hi - def.front) - (-def.back - lo)
            })
            .unwrap_or(1.0);
        let up = m.transform_vector3(n * side.signum()).z > 0.0;
        flat_forms.push(crate::sheetmetal::flat::FlatForm {
            loops: loops.iter().map(|l| l.iter().map(|p| frame.to_local(m.transform_point3(*p))).collect()).collect(),
            curves: curves.iter().map(|(l, tear)| (l.iter().map(|p| frame.to_local(m.transform_point3(*p))).collect(), *tear)).collect(),
            center: frame.to_local(m.transform_point3(center)),
            up,
        });
    }
    let mut lines = Vec::new();
    for (n, s) in strips.iter().enumerate() {
        let Some(ma) = unfolded.walls.get(&s.wall_a) else { continue };
        let a = s.a.map(|p| ma.transform_point3(p));
        let b = match unfolded.walls.get(&s.wall_b) {
            Some(mb) => s.b.map(|p| mb.transform_point3(p)),
            // A free rolled end: the developed length on from wall a.
            None if s.rolled.is_some() => s.a.map(|p| ma.transform_point3(p - s.dir_a * s.angle * (s.radius + def.k_factor * thickness))),
            None => continue,
        };
        // The strip's outline, its ends along their corner cuts if any.
        let at = |(f, w): (f64, f64)| a[0] + (a[1] - a[0]) * f + (b[0] - a[0]) * w;
        let cut = |start: bool| s.end_cuts.iter().find(|c| c.0 == start).map(|c| c.1.iter().copied().map(at).collect::<Vec<DVec3>>());
        let mut outline: Vec<DVec3> = cut(true).unwrap_or_else(|| vec![a[0], b[0]]);
        outline.extend(cut(false).map(|mut pts| {
            pts.reverse();
            pts
        }).unwrap_or_else(|| vec![b[1], a[1]]));
        if let Ok(face) = k::make_polyline(&outline, true).and_then(|w| k::make_face_from_wire(&w, true)) {
            pieces.push(face);
        }
        // Wall b rises toward the viewer (+z) when it lies on the side wall a's
        // flat normal points away from.
        let flat_normal = ma.transform_vector3(s.normal_a);
        let up = (s.toward_b.dot(s.normal_a) < 0.0) == (flat_normal.z < 0.0);
        // The centre line, from where a corner cut crosses it (live 1676:
        // the front bend's line ends at x 0.171619, not the zone's end).
        let mid = |i: usize| {
            let start = i == 0;
            let p = s.end_cuts.iter().find(|c| c.0 == start).and_then(|c| c.1.get(c.1.len() / 2).copied()).map_or((a[i] + b[i]) * 0.5, at);
            glam::DVec2::new(p.x, p.y)
        };
        // Running as the bend's edge runs round wall a (wall a on its left
        // seen against its normal), as live's DXF (dom/qa5-sm-flat.dxf).
        let (i0, i1) = if s.normal_a.cross(s.a[1] - s.a[0]).dot(s.dir_a) >= 0.0 { (0, 1) } else { (1, 0) };
        lines.push(crate::sheetmetal::FlatBendLine {
            a: mid(i0),
            b: mid(i1),
            up,
            angle: s.angle,
            radius: s.radius,
            k_factor: def.k_factor,
            allowance: s.angle * (s.radius + def.k_factor * thickness),
            bend: n,
        });
    }
    let mut flat = crate::sheetmetal::flat::flat_from_pieces(frame, &pieces, lines, thickness).ok()?;
    flat.forms = flat_forms;
    flat.heads = heads;
    Some(flat)
}

struct Unfolded {
    walls: BTreeMap<usize, glam::DMat4>,
    bends: BTreeMap<usize, glam::DMat4>,
}

fn rotation_about(point: DVec3, axis: DVec3, angle: f64) -> glam::DMat4 {
    glam::DMat4::from_translation(point) * glam::DMat4::from_axis_angle(axis.normalize(), angle) * glam::DMat4::from_translation(-point)
}

fn unfold(def: &Definition, k_factor: f64, topo: &Topo) -> FResult<Unfolded> {
    let mut walls = BTreeMap::new();
    let mut bends = BTreeMap::new();
    if topo.faces.is_empty() {
        return Ok(Unfolded { walls, bends });
    }
    // Each part's first wall (walls only joined by rips are separate parts):
    // its definition normal to -z, its plane to z = 0.
    for root in 0..topo.faces.len() {
        if walls.contains_key(&root) || def.rolled.contains_key(&root) || !matches!(topo.faces[root].face_plane(), Ok(Some(_))) {
            continue;
        }
        place_from(def, k_factor, topo, root, &mut walls, &mut bends)?;
    }
    Ok(Unfolded { walls, bends })
}

/// Place wall `root` (normal to -z, plane z = 0) and every wall reached from
/// it across bends.
fn place_from(def: &Definition, k_factor: f64, topo: &Topo, root: usize, walls: &mut BTreeMap<usize, glam::DMat4>, bends: &mut BTreeMap<usize, glam::DMat4>) -> FResult<()> {
    let thickness = def.front + def.back;
    let (p0, n0) = face_normal(&topo.faces[root])?;
    let target = DVec3::NEG_Z;
    let rot = if n0.dot(target) > 1.0 - 1e-12 {
        glam::DMat4::IDENTITY
    } else if n0.dot(target) < -1.0 + 1e-12 {
        glam::DMat4::from_axis_angle(DVec3::X, std::f64::consts::PI)
    } else {
        glam::DMat4::from_axis_angle(n0.cross(target).normalize(), n0.dot(target).clamp(-1.0, 1.0).acos())
    };
    let z = rot.transform_point3(p0).z;
    walls.insert(root, glam::DMat4::from_translation(DVec3::new(0.0, 0.0, -z)) * rot);
    let mut queue = vec![root];
    while let Some(w1) = queue.pop() {
        let base = walls[&w1];
        for (&e, &radius) in &def.bends {
            let faces = &topo.edge_faces[e];
            if faces.len() != 2 || !faces.contains(&(w1 as u32)) {
                continue;
            }
            let w2 = faces.iter().map(|f| *f as usize).find(|f| *f != w1).unwrap_or(w1);
            let b = bend_geometry(topo, e, w1, w2, radius, def.front, def.back)?;
            // Directions from the edge into each wall.
            let into = |f: usize| -> FResult<DVec3> {
                let (p, n) = face_normal(&topo.faces[f])?;
                Ok(wall_side(&topo.faces[f], n, b.dir, b.start + b.dir * (b.len * 0.5), p))
            };
            let (a1, a2) = (into(w1)?, into(w2)?);
            let developed = b.angle * (radius + k_factor * thickness);
            // Turning a2 onto -a1 about the bend axis (the nearer way: under
            // 45° both turns leave it on -a1's side).
            let turned = |s: f64| glam::DMat4::from_axis_angle(b.dir, s * b.angle).transform_vector3(a2).dot(-a1);
            let sign = if turned(1.0) >= turned(-1.0) { 1.0 } else { -1.0 };
            let step = |f: f64| glam::DMat4::from_translation(-a1 * developed * f) * rotation_about(b.axis_point, b.dir, sign * b.angle * f);
            bends.entry(e).or_insert(base * step(0.5));
            if let std::collections::btree_map::Entry::Vacant(v) = walls.entry(w2) {
                v.insert(base * step(1.0));
                queue.push(w2);
            }
        }
        // Coplanar neighbours (a rip splitting a wall) lie where they are.
        for e in (0..topo.edges.len()).filter(|&e| coplanar_rip(topo, e)) {
            let faces = &topo.edge_faces[e];
            if !faces.contains(&(w1 as u32)) {
                continue;
            }
            let w2 = faces.iter().map(|f| *f as usize).find(|f| *f != w1).unwrap_or(w1);
            if let std::collections::btree_map::Entry::Vacant(v) = walls.entry(w2) {
                v.insert(base);
                queue.push(w2);
            }
        }
        // Across rolled bends: wall b turned about the arc's axis onto wall
        // a's tangent line, then moved out by the developed length.
        for (&fi, &radius) in &def.rolled {
            let Some(st) = rolled_strip(topo, fi, radius) else { continue };
            let (w2, line1, line2) = if st.wall_a == w1 { (st.wall_b, st.a, st.b) } else if st.wall_b == w1 { (st.wall_a, st.b, st.a) } else { continue };
            if w2 == usize::MAX || walls.contains_key(&w2) {
                continue;
            }
            let axis = st.axis.normalize();
            let into = |f: usize, at: DVec3| -> FResult<DVec3> {
                let (p, n) = face_normal(&topo.faces[f])?;
                let a = n.cross(axis).normalize();
                Ok(if (p - at).dot(a) < 0.0 { -a } else { a })
            };
            let (a1, a2) = (into(w1, line1[0])?, into(w2, line2[0])?);
            let developed = st.angle * (radius + k_factor * thickness);
            let turned = |s: f64| glam::DMat4::from_axis_angle(axis, s * st.angle).transform_vector3(a2).dot(-a1);
            let sign = if turned(1.0) >= turned(-1.0) { 1.0 } else { -1.0 };
            walls.insert(w2, base * glam::DMat4::from_translation(-a1 * developed) * rotation_about(st.origin, axis, sign * st.angle));
            queue.push(w2);
        }
    }
    Ok(())
}

/// `@evSheetMetalHoleToolBodies` (std's sheet metal face pattern): the tools of the sheet metal holes among the
/// faces (those with a hole attribute). Without any, none; hole tools aren't tracked yet.
fn ev_sheet_metal_hole_tool_bodies(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let arg = def_arg(i, &a, 1)?;
    if !evaluate_refs(i, &cx, arg.get("sheetMetalHoleFaces"))?.is_empty() {
        return Err(i.error("@evSheetMetalHoleToolBodies: the tools of sheet metal holes are not supported yet"));
    }
    let none = || Value::array(Vec::new());
    Ok(Value::map_from([("sheetMetalHoleToolBodies", none()), ("sheetMetalHoleToolWalls", none()), ("sheetMetalHoleToolTransforms", none())]))
}

/// `@evSheetMetalFormToolBodies` (std's sheet metal face pattern): the tools of the forms among the faces, by wall.
/// With no faces or no form on any wall, none (`{}`); form tools aren't returned yet.
fn ev_sheet_metal_form_tool_bodies(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let arg = def_arg(i, &a, 1)?;
    let faces = evaluate_refs(i, &cx, arg.get("sheetMetalFormFaces"))?;
    let formed = cx.state.lock().attributes().iter().any(|(_, v)| v.get("attribute").get("formedToolBodyIds").as_array().is_some_and(|f| !f.is_empty()));
    if !faces.is_empty() && formed {
        return Err(i.error("@evSheetMetalFormToolBodies: the tools of sheet metal forms are not supported yet"));
    }
    Ok(Value::map_from(Vec::new()))
}

fn ev_sheet_metal_flat_transformation(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let arg = def_arg(i, &a, 1)?;
    let faces = evaluate_refs(i, &cx, arg.get("face"))?;
    let found = {
        let st = cx.state.lock();
        let q = QueryCx::new(&st);
        // The definition entity the part face comes from (shared association).
        let def_entity = faces.first().and_then(|r| {
            associations(&st, r).into_iter().find_map(|assoc| {
                st.attributes().iter().filter(|(x, v)| x != r && *v.get("attribute") == assoc).find_map(|(x, _)| {
                    let e = q.from_ref(x).into_iter().next()?;
                    let Some(Ent::Body(b)) = q.owner(&e) else { return None };
                    sm_attribute(&st, &q.to_ref(&Ent::Body(b)), "MODEL").map(|model| (e, b, model))
                })
            })
        });
        def_entity.map(|(e, b, model)| {
            let body = &st.ctx.bodies[b];
            let (bends, rips, rolled, rip_styles, joint_ids) = joints(&st, &q, b);
            let k_factor = model.get("k-factor").get("value").as_number().unwrap_or(0.45);
            let def = Definition { body: body.id.clone(), front: length_field(&model, "frontThickness"), back: length_field(&model, "backThickness"), bends, rolled, rips, rip_styles, joint_ids, clearance: length_field(&model, "minimalClearance"), relief: BendRelief::of(&model), corner: CornerRelief::of(&model), corners: corner_overrides(&st, &q, b, &CornerRelief::of(&model)), reliefs: relief_overrides(&st, &q, b, &BendRelief::of(&model)), k_factor: model.get("k-factor").get("value").as_number().unwrap_or(0.45), active: model.get("active").as_bool().unwrap_or(true), default_radius: length_field(&model, "defaultBendRadius"), previous: Vec::new(), forms: Vec::new(), wall_ids: BTreeMap::new(), breaks: BTreeMap::new() };
            (e, body.shape.clone(), def, k_factor)
        })
    };
    let Some((entity, shape, def, k_factor)) = found else { return Err(i.error("evSheetMetalFlatTransformation: not a sheet metal wall or bend")) };
    let topo = Topo::of(&shape).map_err(|e| i.error(e.message))?;
    let unfolded = unfold(&def, k_factor, &topo).map_err(|e| i.error(e.message))?;
    let m = match entity {
        Ent::Face(_, f) => unfolded.walls.get(&f).copied(),
        Ent::Edge(_, e) => unfolded.bends.get(&e).copied().or_else(|| topo.edge_faces.get(e).and_then(|fs| fs.first()).and_then(|f| unfolded.walls.get(&(*f as usize)).copied())),
        _ => None,
    };
    let Some(m) = m else { return Err(i.error("evSheetMetalFlatTransformation: no flat transformation for this entity")) };
    let row = |r: usize| Value::array((0..3).map(|c| Value::num(m.col(c)[r])).collect());
    let t = m.col(3);
    Ok(Value::map_from([("linear", Value::array(vec![row(0), row(1), row(2)])), ("translation", Value::array(vec![Value::num(t.x), Value::num(t.y), Value::num(t.z)]))]))
}

/// `@evCornerType` (std evaluate.fs; Corner feature): what meets at a sheet
/// metal definition vertex, with the definition vertices lying there (a
/// corner's walls end in coincident vertices). No bend: NOT_A_CORNER; one
/// bend alone: BEND_END; bends and a rip between their walls: CLOSED_CORNER
/// (live G34a, a Closed corner on two flanges); otherwise OPEN_CORNER.
fn ev_corner_type(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let (kind, refs) = super::query::with_ents(i, &cx, def.get("vertex"), |i, q, ents| {
        let [Ent::Vertex(b, v)] = ents.as_slice() else { return Err(super::raise_error(i, "BAD_GEOMETRY", None)) };
        let shape = &q.st.ctx.bodies[*b].shape;
        let at = shape.sub_shape(k::ShapeType::Vertex, *v).and_then(|x| x.vertex_point()).unwrap_or_default();
        let verts: Vec<usize> = shape.vertices().iter().enumerate().filter(|(_, x)| x.vertex_point().is_ok_and(|p| p.distance(at) < TOL)).map(|(n, _)| n).collect();
        let ve = shape.vertex_edges().unwrap_or_default();
        let edges: std::collections::BTreeSet<u32> = verts.iter().flat_map(|n| ve.get(*n).cloned().unwrap_or_default()).collect();
        let (mut bends, mut rips) = (0, 0);
        for e in edges {
            let Some(j) = sm_attribute(q.st, &q.to_ref(&Ent::Edge(*b, e as usize)), "JOINT") else { continue };
            match j.get("jointType").get("value").as_str().as_deref() {
                Some("BEND") => bends += 1,
                Some("RIP") => rips += 1,
                _ => {}
            }
        }
        let kind = match (bends, rips) {
            (0, _) => "NOT_A_CORNER",
            (1, 0) => "BEND_END",
            (2.., 1..) => "CLOSED_CORNER",
            _ => "OPEN_CORNER",
        };
        Ok((kind, verts.iter().map(|n| q.to_ref(&Ent::Vertex(*b, *n))).collect::<Vec<Reference>>()))
    })?;
    // Transient ids (std's wrapper makes them queries).
    let mut st = cx.state.lock();
    let ids: Vec<Value> = refs.iter().map(|r| Value::str(st.transient(r))).collect();
    let corner = super::std_enum("smcornertype.gen.fs", "SMCornerType", kind).unwrap_or_else(|| Value::str(kind));
    Ok(Value::map_from([("cornerType", corner), ("primaryVertex", ids.first().cloned().unwrap_or_default()), ("allVertices", Value::array(ids))]))
}
