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
use crate::reference::{BodyId, Reference};
use crate::regen::eval::{FResult, FeatureError};
use crate::regen::features::helpers_a::push_body;
use glam::DVec3;
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::Value;
use noshape_kernel as k;
use noshape_kernel::Shape;
use std::collections::BTreeMap;
use std::sync::Arc;

pub const NATIVES: &[(&str, NativeFn)] = &[("updateSheetMetalGeometry", update_sheet_metal_geometry), ("evSheetMetalFlatTransformation", ev_sheet_metal_flat_transformation)];

const TOL: f64 = 1e-7;

fn fe(m: impl Into<String>) -> FeatureError {
    FeatureError::new(m)
}

/// The SM attribute (`SMAttribute`) of an entity with the given object type.
fn sm_attribute(st: &CtxState, r: &Reference, object_type: &str) -> Option<Value> {
    st.attributes.iter().find(|(x, v)| x == r && v.get("attribute").get("objectType").as_str().is_some_and(|t| &*t == object_type)).map(|(_, v)| v.get("attribute").clone())
}

/// `{ value : <length> }` fields of SM attributes, in meters.
fn length_field(attr: &Value, key: &str) -> f64 {
    length_of(attr.get(key).get("value")).unwrap_or(0.0)
}

/// The association attributes of an entity (copied to the part entities).
fn associations(st: &CtxState, r: &Reference) -> Vec<Value> {
    let tag = std_tag("sheetMetalAttribute.fs", "SMAssociationAttribute");
    st.attributes.iter().filter(|(x, v)| x == r && v.get("attribute").tag() == tag).map(|(_, v)| v.get("attribute").clone()).collect()
}

/// A definition sheet to build: its entities and what std attached to them.
struct Definition {
    body: BodyId,
    front: f64,
    back: f64,
    /// Bend radius by definition edge index.
    bends: BTreeMap<usize, f64>,
    /// Rip edges (definition edge indices) and the gap walls keep from them.
    rips: Vec<usize>,
    clearance: f64,
    /// Bend relief at the ends of a partial bend: style, depth scale (× bend
    /// radius) and width scale (× thickness), from the model attribute.
    relief: BendRelief,
    corner: CornerRelief,
    /// Corner features' settings by definition vertex (else `corner`).
    corners: BTreeMap<usize, CornerRelief>,
    k_factor: f64,
    /// The part built from it before (rebuilt in place).
    previous: Option<BodyId>,
}

/// A definition body's bends (edge index → radius) and rips (edge indices).
fn joints(st: &CtxState, q: &QueryCx, b: usize) -> (BTreeMap<usize, f64>, Vec<usize>) {
    let mut bends = BTreeMap::new();
    let mut rips = Vec::new();
    for e in 0..st.ctx.bodies[b].edges.len() {
        let Some(j) = sm_attribute(st, &q.to_ref(&Ent::Edge(b, e)), "JOINT") else { continue };
        match j.get("jointType").get("value").as_str().as_deref() {
            Some("BEND") => {
                bends.insert(e, length_field(&j, "radius"));
            }
            Some("RIP") => rips.push(e),
            _ => {}
        }
    }
    (bends, rips)
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
    scale: f64,
    width: f64,
    diameter: f64,
}

impl CornerRelief {
    /// A Corner feature's attribute (std sheetMetalCorner.fs), its unset
    /// sizes the model's.
    fn of_corner(attr: &Value, model: &CornerRelief) -> CornerRelief {
        let len = |k: &str, d: f64| attr.get(k).get("value").as_number().or_else(|| length_of(attr.get(k).get("value"))).unwrap_or(d);
        CornerRelief {
            style: enum_field(attr, "cornerStyle").unwrap_or_else(|| model.style.clone()),
            scale: attr.get("cornerReliefScale").get("value").as_number().unwrap_or(model.scale),
            width: len("squareReliefWidth", model.width),
            diameter: len("roundReliefDiameter", model.diameter),
        }
    }

    fn of(model: &Value) -> CornerRelief {
        CornerRelief {
            style: enum_field(model, "defaultThreeCornerStyle").unwrap_or_else(|| "SIMPLE".to_string()),
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
        }
    }
}

/// Corner features' settings on a definition body's vertices.
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
struct Piece {
    face: Shape,
    from: Def,
}

/// One wall's outer loop as straight edges: points and the definition
/// edge index of each side (`side[k]` runs from `pts[k]` to `pts[k + 1]`).
struct Loop {
    pts: Vec<DVec3>,
    side: Vec<usize>,
}

/// Definition topology: faces, edges (with their points and faces) and vertices.
struct Topo {
    faces: Vec<Shape>,
    edges: Vec<Shape>,
    edge_faces: Vec<Vec<u32>>,
    edge_ends: Vec<(DVec3, DVec3)>,
    vertices: Vec<DVec3>,
}

impl Topo {
    fn of(shape: &Shape) -> FResult<Topo> {
        let faces = shape.faces();
        let edges = shape.edges();
        let edge_faces = shape.edge_faces().map_err(|e| fe(e.to_string()))?;
        let edge_ends = edges.iter().map(|e| e.endpoints().unwrap_or_default()).collect();
        let vertices = shape.vertices().iter().filter_map(|v| v.vertex_point().ok()).collect();
        Ok(Topo { faces, edges, edge_faces, edge_ends, vertices })
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
    let wire = face.wires().into_iter().next().ok_or_else(|| fe("A sheet metal wall has no boundary"))?;
    let mut segs: Vec<(DVec3, DVec3, usize)> = Vec::new();
    for e in wire.edges() {
        let info = e.edge_info().map_err(|x| fe(x.to_string()))?;
        if !matches!(info.curve, k::Curve::Line { .. }) {
            return Err(fe("Curved sheet metal wall edges are not supported yet"));
        }
        let idx = topo.edges.iter().position(|x| x.is_same(&e)).ok_or_else(|| fe("Sheet metal wall edge not found"))?;
        segs.push((info.start, info.end, idx));
    }
    if segs.len() < 3 {
        return Err(fe("A sheet metal wall needs at least three edges"));
    }
    // Chain the sides: each starts where the previous one ended.
    let (first_start, mut at, first_idx) = segs.swap_remove(0);
    let mut pts = vec![first_start];
    let mut side = vec![first_idx];
    while !segs.is_empty() {
        let Some(k) = segs.iter().position(|(a, b, _)| a.distance(at) < 1e-7 || b.distance(at) < 1e-7) else {
            return Err(fe("A sheet metal wall boundary is not closed"));
        };
        let (a, b, idx) = segs.swap_remove(k);
        let next = if a.distance(at) < 1e-7 { b } else { a };
        pts.push(at);
        side.push(idx);
        at = next;
    }
    Ok(Loop { pts, side })
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
}

/// Geometry of the bend at edge `e` between walls `fa` (seen from `a`) and `fb`.
fn bend_geometry(topo: &Topo, e: usize, fa: usize, fb: usize, radius: f64, front: f64, back: f64) -> FResult<Bend> {
    let (s, t) = topo.edge_ends[e];
    let len = s.distance(t);
    let dir = (t - s) / len.max(1e-300);
    let into = |f: usize| -> FResult<(DVec3, DVec3)> {
        let (p, n) = face_normal(&topo.faces[f])?;
        let mut a = n.cross(dir).normalize();
        if (p - s).dot(a) < 0.0 {
            a = -a;
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
    Ok(Bend { start: s, dir, len, angle, setback, axis_point })
}

/// Pull the loop's sides in by `offset[k]` (0 for free edges), in the wall
/// plane: each side's pulled segment. Collinear sides pulled by different
/// amounts (a partial bend beside a free edge) end in a step.
fn pulled_sides(l: &Loop, n: DVec3, offset: &[f64]) -> Vec<(DVec3, DVec3)> {
    let m = l.pts.len();
    let line = |k: usize| {
        let d = (l.pts[(k + 1) % m] - l.pts[k]).normalize();
        (l.pts[k] + inward(l, n, k) * offset[k], d)
    };
    // Each corner's point on the side before it and on the side after it.
    let corners: Vec<(DVec3, DVec3)> = (0..m)
        .map(|k| {
            let ((p0, d0), (p1, d1)) = (line((k + m - 1) % m), line(k));
            let c = d0.cross(d1);
            if c.length_squared() < 1e-20 {
                let v = l.pts[k];
                return (p0 + d0 * (v - p0).dot(d0), p1 + d1 * (v - p1).dot(d1));
            }
            let x = p0 + d0 * ((p1 - p0).cross(d1).dot(c) / c.length_squared());
            (x, x)
        })
        .collect();
    (0..m).map(|k| (corners[k].1, corners[(k + 1) % m].0)).collect()
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
    let m = sides.len();
    let mut out = Vec::new();
    for k in 0..m {
        out.push(sides[k].0);
        if sides[k].1.distance(sides[(k + 1) % m].0) > TOL {
            out.push(sides[k].1);
        }
    }
    out
}

/// Bend relief cutters of one wall: at each end of a bend side that runs on
/// along a free side, a slot into the wall on the free side (see [`BendRelief`]).
fn relief_tools(def: &Definition, topo: &Topo, l: &Loop, n: DVec3, thickness: f64) -> FResult<Vec<(Shape, usize)>> {
    let mut out = Vec::new();
    if def.relief.style == "TEAR" {
        return Ok(out);
    }
    let m = l.pts.len();
    let w = def.relief.width * thickness;
    for k in 0..m {
        let Some(&radius) = def.bends.get(&l.side[k]) else { continue };
        let dir = (l.pts[(k + 1) % m] - l.pts[k]).normalize();
        let into = inward(l, n, k);
        for (j, v, away) in [((k + m - 1) % m, l.pts[k], -dir), ((k + 1) % m, l.pts[(k + 1) % m], dir)] {
            let e = l.side[j];
            let free = !def.bends.contains_key(&e) && !def.rips.contains(&e);
            let along = (l.pts[(j + 1) % m] - l.pts[j]).normalize();
            if !free || along.cross(dir).length() > 1e-9 {
                continue;
            }
            let straight = def.relief.depth * radius;
            let depth = if def.relief.style == "RECTANGLE" { straight + w * 0.5 } else { straight };
            // Through the whole thickness and a little more.
            let base = v - n * (def.back + thickness);
            let quad = [base, base + away * w, base + away * w + into * depth, base + into * depth];
            let wire = k::make_polyline(&quad, true).map_err(|e| fe(e.to_string()))?;
            let face = k::make_face_from_wire(&wire, true).map_err(|e| fe(e.to_string()))?;
            let vi = topo.nearest_vertex(v);
            out.push((k::extrude(&face, n * thickness * 3.0).map_err(|e| fe(e.to_string()))?.shape, vi));
            if def.relief.style == "OBROUND" {
                let frame = k::Plane::new(base + away * (w * 0.5) + into * depth, n, away);
                out.push((k::make_cylinder(&frame, w * 0.5, thickness * 3.0, std::f64::consts::TAU).map_err(|e| fe(e.to_string()))?, vi));
            }
        }
    }
    Ok(out)
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
    /// A closed corner (live G34a): the bend runs to the square's diagonal,
    /// from `plate_end` at the plate's side to `flange_end` at the flange's.
    Closed { plate_end: f64, flange_end: f64 },
}

impl WrappedCut {
    fn range(&self, w: f64, reach: f64) -> (f64, f64) {
        match self.shape {
            CutShape::Round { along, radius } => {
                let half = (radius * radius - (w - self.ba * 0.5).powi(2)).max(0.0).sqrt();
                (along - half, along + half)
            }
            CutShape::Closed { plate_end, flange_end } => (-reach, plate_end + (flange_end - plate_end) * (w / self.ba)),
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
            // The Corner feature's setting at this vertex, else the model's.
            let corner = def.corners.get(&vi).unwrap_or(&def.corner);
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
                extends.push((e1, v, def.clearance));
                extends.push((e2, v, def.clearance));
                wraps.push(WrappedCut { edge: e1, v, plate: p, ba: b1, shape: CutShape::Closed { plate_end: s2 + gap, flange_end: def.clearance } });
                wraps.push(WrappedCut { edge: e2, v, plate: p, ba: b2, shape: CutShape::Closed { plate_end: s1 + gap, flange_end: def.clearance } });
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

/// The part solid of one definition sheet and the pieces its faces come from.
fn build_part(def: &Definition, shape: &Shape) -> FResult<(Shape, Vec<Piece>, Topo)> {
    let topo = Topo::of(shape)?;
    let (front, back) = (def.front, def.back);
    if front + back <= TOL {
        return Err(fe("The sheet metal thickness must be positive"));
    }
    let mut solids = Vec::new();
    let mut pieces = Vec::new();
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
    // Each wall's outline in its definition plane: pulled back from its
    // bends, and from its rips by the minimal clearance (live
    // dom/fs-smprobe3-{4,6}.txt: both walls stop 0.02 mm short of a rip).
    // Curved faces are rolled bends (a closed Hem's half cylinder, live
    // dom/fs-smprobe3-7.txt): thickened as they are, the walls meeting
    // them tangentially.
    let mut outlines = Vec::new();
    for face in &topo.faces {
        if face.face_plane().map_err(|e| fe(e.to_string()))?.is_none() {
            outlines.push(None);
            continue;
        }
        let l = wall_loop(face, &topo)?;
        let (_, n) = face_normal(face)?;
        let offset: Vec<f64> = l.side.iter().map(|e| setbacks.get(e).copied().unwrap_or(if def.rips.contains(e) { def.clearance } else { 0.0 })).collect();
        let sides = pulled_sides(&l, n, &offset);
        outlines.push(Some((l, n, sides)));
    }
    // Where a wall's pulled-back side along edge `e` runs, as distances along `dir` from `start`.
    let extent = |f: usize, e: usize, start: DVec3, dir: DVec3| -> Option<(f64, f64)> {
        let (l, _, sides) = outlines[f].as_ref()?;
        let k = l.side.iter().position(|x| *x == e)?;
        let (t0, t1) = ((sides[k].0 - start).dot(dir), (sides[k].1 - start).dot(dir));
        Some((t0.min(t1), t0.max(t1)))
    };
    let (clips, extends, mut cuts, wraps) = corner_reliefs(def, &topo, &outlines, &bends, &setbacks)?;
    for (e, fa, fb, b) in bends {
        // The bend runs where both walls do (a simple corner ends it where
        // the neighbouring bend starts: live sm4, sm6), less a corner relief.
        let (mut t0, mut t1) = (0.0, b.len);
        for f in [fa, fb] {
            if let Some((a0, a1)) = extent(f, e, b.start, b.dir) {
                t0 = f64::max(t0, a0);
                t1 = f64::min(t1, a1);
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
        let mut a1 = n1.cross(b.dir).normalize();
        if (pa - b.start).dot(a1) < 0.0 {
            a1 = -a1;
        }
        let p1 = b.start + a1 * b.setback + b.dir * t0;
        let len = t1 - t0;
        let strip = [p1 - n1 * back, p1 + n1 * front, p1 + n1 * front + b.dir * len, p1 - n1 * back + b.dir * len];
        let wire = k::make_polyline(&strip, true).map_err(|e| fe(e.to_string()))?;
        let face = k::make_face_from_wire(&wire, true).map_err(|e| fe(e.to_string()))?;
        // Turn towards wall b.
        let (pb, nb) = face_normal(&topo.faces[fb])?;
        let mut a2 = nb.cross(b.dir).normalize();
        if (pb - b.start).dot(a2) < 0.0 {
            a2 = -a2;
        }
        let p2 = b.start + a2 * b.setback + b.dir * t0;
        let rotate = |ang: f64| {
            let r = p1 - b.axis_point;
            let along = b.dir * r.dot(b.dir);
            let perp = r - along;
            b.axis_point + along + perp * ang.cos() + b.dir.cross(perp) * ang.sin()
        };
        let sign = if rotate(b.angle).distance(p2) <= rotate(-b.angle).distance(p2) { 1.0 } else { -1.0 };
        let mut bend = k::revolve(&face, b.axis_point, b.dir * sign, b.angle).map_err(|e| fe(format!("Sheet metal bend failed: {e}")))?.shape;
        let (v0, v1) = (topo.nearest_vertex(b.start), topo.nearest_vertex(b.start + b.dir * b.len));
        // Round corner reliefs: the flat circle wrapped onto the bend (angle
        // = flat distance / (r + K T)), cut out through the thickness.
        let neutral = def.bends.get(&e).copied().unwrap_or(0.0) + def.k_factor * (front + back);
        for rc in wraps.iter().filter(|rc| rc.edge == e) {
            let turn = |p: DVec3, ang: f64| {
                let r = p - b.axis_point;
                let along = b.dir * r.dot(b.dir);
                let perp = r - along;
                b.axis_point + along + perp * ang.cos() + b.dir.cross(perp) * ang.sin()
            };
            let at_start = rc.v.distance(b.start) < TOL;
            let base = b.start + a1 * b.setback;
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
    for (fi, outline) in outlines.iter().enumerate() {
        let Some((l, n, pulled)) = outline else {
            rolled_bend(&topo, fi, front, back, &mut solids, &mut pieces)?;
            continue;
        };
        let n = *n;
        let pts: Vec<DVec3> = outline_points(pulled).into_iter().map(|p| p - n * back).collect();
        let wire = k::make_polyline(&pts, true).map_err(|e| fe(e.to_string()))?;
        let base = k::make_face_from_wire(&wire, true).map_err(|e| fe(e.to_string()))?;
        let mut slab = k::extrude(&base, n * (front + back)).map_err(|e| fe(format!("Sheet metal wall failed: {e}")))?.shape;
        let mut reliefs = relief_tools(def, &topo, l, n, front + back)?;
        reliefs.extend(cuts.remove(&fi).unwrap_or_default());
        if !reliefs.is_empty() {
            let tools: Vec<Shape> = reliefs.iter().map(|r| r.0.clone()).collect();
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
                    // A side face: the loop side it was swept from.
                    let c = face_normal(&f).map(|x| x.0).unwrap_or_default();
                    let k = (0..l.pts.len())
                        .min_by(|a, b| {
                            let d = |k: usize| {
                                let (p, q) = (pulled[k].0 - n * back, pulled[k].1 - n * back);
                                let dq = (q - p).normalize();
                                let r = c - p - n * (c - p).dot(n);
                                (r - dq * r.dot(dq)).length()
                            };
                            d(*a).total_cmp(&d(*b))
                        })
                        .unwrap_or(0);
                    Def::Edge(l.side[k])
                }
            };
            pieces.push(Piece { face: f, from });
        }
        solids.push(slab);
    }
    let fused = if solids.len() == 1 {
        solids.remove(0)
    } else {
        k::fuse_all(&solids, &k::BooleanOptions::default()).map_err(|e| fe(format!("Sheet metal part failed: {e}")))?.shape
    };
    Ok((fused, pieces, topo))
}

/// A rolled bend: definition face `fi` thickened (`front` along its normal).
/// Its offsets come from the face; its end caps from the face's edges there.
fn rolled_bend(topo: &Topo, fi: usize, front: f64, back: f64, solids: &mut Vec<Shape>, pieces: &mut Vec<Piece>) -> FResult<()> {
    let face = &topo.faces[fi];
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
fn inner_point(f: &Shape) -> Option<DVec3> {
    let [u0, u1, v0, v1] = f.uv_bounds().ok()?;
    for (a, b) in [(0.5, 0.5), (0.3, 0.3), (0.7, 0.7), (0.3, 0.7), (0.7, 0.3), (0.5, 0.2), (0.2, 0.5), (0.5, 0.8), (0.8, 0.5)] {
        let p = f.point_at_uv(u0 + (u1 - u0) * a, v0 + (v1 - v0) * b).ok()?;
        if f.project_point(p).is_ok_and(|pr| pr.point.distance(p) < 1e-9) {
            return Some(p);
        }
    }
    None
}

/// What each face, edge and vertex of the part comes from.
fn part_sources(part: &Shape, pieces: &[Piece], topo: &Topo) -> (Vec<Def>, Vec<Def>, Vec<Def>) {
    let faces: Vec<Def> = part
        .faces()
        .iter()
        .map(|f| {
            let Some(p) = inner_point(f) else { return Def::Face(0) };
            pieces.iter().find(|x| x.face.project_point(p).is_ok_and(|pr| pr.point.distance(p) < 1e-7)).map_or(Def::Face(0), |x| x.from)
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
            let (bends, rips) = joints(&st, &q, b);
            // The part made from it before: the solid carrying its association.
            let own = associations(&st, &body_ref);
            let previous = st.ctx.bodies.iter().enumerate().find(|(pb, p)| p.kind == BodyKind::Solid && !own.is_empty() && associations(&st, &q.to_ref(&Ent::Body(*pb))).iter().any(|x| own.contains(x))).map(|(_, p)| p.id.clone());
            defs.push(Definition { body: body.id.clone(), front: length_field(&model, "frontThickness"), back: length_field(&model, "backThickness"), bends, rips, clearance: length_field(&model, "minimalClearance"), relief: BendRelief::of(&model), corner: CornerRelief::of(&model), corners: corner_overrides(&st, &q, b, &CornerRelief::of(&model)), k_factor: model.get("k-factor").get("value").as_number().unwrap_or(0.45), previous });
            let faces: Vec<Reference> = (0..body.faces.len()).map(|f| q.to_ref(&Ent::Face(b, f))).collect();
            let edges: Vec<Reference> = (0..body.edges.len()).map(|e| q.to_ref(&Ent::Edge(b, e))).collect();
            let vertices: Vec<Reference> = (0..body.vertices.len()).map(|v| q.to_ref(&Ent::Vertex(b, v))).collect();
            def_refs.push((body_ref, faces, edges, vertices));
        }
        (defs, def_refs)
    };
    if defs.is_empty() {
        return Ok(Value::Undefined);
    }
    let op: FsId = id.iter().cloned().chain(std::iter::once(Arc::<str>::from("smGeometry"))).collect();
    let built = run_op(i, &cx, &op, "SHEET_METAL_REBUILD_ERROR", |ev| {
        let mut out = Vec::new();
        for (n, def) in defs.iter().enumerate() {
            let Some(bi) = ev.ctx.bodies.iter().position(|b| b.id == def.body) else { continue };
            let shape = ev.ctx.bodies[bi].shape.clone();
            let (part, pieces, topo) = build_part(def, &shape)?;
            let sources = part_sources(&part, &pieces, &topo);
            // The definition body's own topology order (references are by index).
            let def_topo = (ev.ctx.bodies[bi].faces.len(), ev.ctx.bodies[bi].edges.len());
            if def_topo != (topo.faces.len(), topo.edges.len()) {
                return Err(fe("The sheet metal definition changed while rebuilding"));
            }
            let id = match &def.previous {
                Some(old) => {
                    ev.ctx.bodies.retain(|b| &b.id != old);
                    old.clone()
                }
                None => BodyId { feature: ev.feature, index: n as u32 },
            };
            let names = super::ops::fresh_names(ev, &part);
            push_body(ev, id.clone(), BodyKind::Solid, part, names);
            out.push((id, sources));
        }
        Ok(out)
    })?;
    // Associations and hiding.
    let mut st = cx.state.lock();
    let (marker, feature) = (super::marker_of(&op), st.feature);
    for ((part_id, (faces, edges, vertices)), (body_ref, dfaces, dedges, dverts)) in built.into_iter().zip(def_refs) {
        // New parts were renamed to the feature after the operation.
        let part_id = super::wrap_body_id(&part_id, marker, feature);
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
        for (to, from) in links {
            for attr in associations(&st, &from) {
                st.push_attribute(to.clone(), Value::map_from([("attribute", attr)]));
            }
        }
        if let Reference::Body(def_body) = &body_ref {
            st.hidden.insert(def_body.clone());
        }
    }
    Ok(Value::Undefined)
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
struct Unfolded {
    walls: BTreeMap<usize, glam::DMat4>,
    bends: BTreeMap<usize, glam::DMat4>,
}

fn rotation_about(point: DVec3, axis: DVec3, angle: f64) -> glam::DMat4 {
    glam::DMat4::from_translation(point) * glam::DMat4::from_axis_angle(axis.normalize(), angle) * glam::DMat4::from_translation(-point)
}

fn unfold(def: &Definition, k_factor: f64, topo: &Topo) -> FResult<Unfolded> {
    let thickness = def.front + def.back;
    let mut walls = BTreeMap::new();
    let mut bends = BTreeMap::new();
    if topo.faces.is_empty() {
        return Ok(Unfolded { walls, bends });
    }
    // The first wall: its definition normal to -z, its plane to z = 0.
    let (p0, n0) = face_normal(&topo.faces[0])?;
    let target = DVec3::NEG_Z;
    let rot = if n0.dot(target) > 1.0 - 1e-12 {
        glam::DMat4::IDENTITY
    } else if n0.dot(target) < -1.0 + 1e-12 {
        glam::DMat4::from_axis_angle(DVec3::X, std::f64::consts::PI)
    } else {
        glam::DMat4::from_axis_angle(n0.cross(target).normalize(), n0.dot(target).clamp(-1.0, 1.0).acos())
    };
    let z = rot.transform_point3(p0).z;
    walls.insert(0, glam::DMat4::from_translation(DVec3::new(0.0, 0.0, -z)) * rot);
    let mut queue = vec![0usize];
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
                let a = n.cross(b.dir).normalize();
                Ok(if (p - b.start).dot(a) < 0.0 { -a } else { a })
            };
            let (a1, a2) = (into(w1)?, into(w2)?);
            let developed = b.angle * (radius + k_factor * thickness);
            // Turning a2 onto -a1 about the bend axis.
            let sign = if glam::DMat4::from_axis_angle(b.dir, b.angle).transform_vector3(a2).dot(-a1) > 0.0 { 1.0 } else { -1.0 };
            let step = |f: f64| glam::DMat4::from_translation(-a1 * developed * f) * rotation_about(b.axis_point, b.dir, sign * b.angle * f);
            bends.entry(e).or_insert(base * step(0.5));
            if let std::collections::btree_map::Entry::Vacant(v) = walls.entry(w2) {
                v.insert(base * step(1.0));
                queue.push(w2);
            }
        }
    }
    Ok(Unfolded { walls, bends })
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
                st.attributes.iter().filter(|(x, v)| x != r && *v.get("attribute") == assoc).find_map(|(x, _)| {
                    let e = q.from_ref(x).into_iter().next()?;
                    let Some(Ent::Body(b)) = q.owner(&e) else { return None };
                    sm_attribute(&st, &q.to_ref(&Ent::Body(b)), "MODEL").map(|model| (e, b, model))
                })
            })
        });
        def_entity.map(|(e, b, model)| {
            let body = &st.ctx.bodies[b];
            let (bends, rips) = joints(&st, &q, b);
            let k_factor = model.get("k-factor").get("value").as_number().unwrap_or(0.45);
            let def = Definition { body: body.id.clone(), front: length_field(&model, "frontThickness"), back: length_field(&model, "backThickness"), bends, rips, clearance: length_field(&model, "minimalClearance"), relief: BendRelief::of(&model), corner: CornerRelief::of(&model), corners: corner_overrides(&st, &q, b, &CornerRelief::of(&model)), k_factor: model.get("k-factor").get("value").as_number().unwrap_or(0.45), previous: None };
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
