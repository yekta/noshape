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

pub const NATIVES: &[(&str, NativeFn)] = &[("updateSheetMetalGeometry", update_sheet_metal_geometry)];

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
    /// The part built from it before (rebuilt in place).
    previous: Option<BodyId>,
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

/// Pull the loop's sides in by `offset[k]` (0 for free edges), in the wall plane.
fn pulled_loop(l: &Loop, n: DVec3, offset: &[f64]) -> Vec<DVec3> {
    let m = l.pts.len();
    // Interior side of each side: the loop's orientation about n.
    let area: DVec3 = (0..m).map(|k| l.pts[k].cross(l.pts[(k + 1) % m])).sum();
    let ccw = area.dot(n) > 0.0;
    let line = |k: usize| {
        let (p, q) = (l.pts[k], l.pts[(k + 1) % m]);
        let d = (q - p).normalize();
        let inward = if ccw { n.cross(d) } else { d.cross(n) };
        (p + inward * offset[k], d)
    };
    (0..m)
        .map(|k| {
            let (p0, d0) = line((k + m - 1) % m);
            let (p1, d1) = line(k);
            // Intersection of the two sides' lines (in the plane).
            let w = p1 - p0;
            let c = d0.cross(d1);
            if c.length_squared() < 1e-20 {
                return p1;
            }
            let t = w.cross(d1).dot(c) / c.length_squared();
            p0 + d0 * t
        })
        .collect()
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
    // Bends first (their pull-backs trim the walls).
    let mut setbacks: BTreeMap<usize, f64> = BTreeMap::new();
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
        // Wall a's thickness at the pull-back line, revolved onto wall b.
        let (_, n1) = face_normal(&topo.faces[fa])?;
        let (pa, _) = face_normal(&topo.faces[fa])?;
        let mut a1 = n1.cross(b.dir).normalize();
        if (pa - b.start).dot(a1) < 0.0 {
            a1 = -a1;
        }
        let p1 = b.start + a1 * b.setback;
        let strip = [p1 - n1 * back, p1 + n1 * front, p1 + n1 * front + b.dir * b.len, p1 - n1 * back + b.dir * b.len];
        let wire = k::make_polyline(&strip, true).map_err(|e| fe(e.to_string()))?;
        let face = k::make_face_from_wire(&wire, true).map_err(|e| fe(e.to_string()))?;
        // Turn towards wall b.
        let (pb, nb) = face_normal(&topo.faces[fb])?;
        let mut a2 = nb.cross(b.dir).normalize();
        if (pb - b.start).dot(a2) < 0.0 {
            a2 = -a2;
        }
        let p2 = b.start + a2 * b.setback;
        let rotate = |ang: f64| {
            let r = p1 - b.axis_point;
            let along = b.dir * r.dot(b.dir);
            let perp = r - along;
            b.axis_point + along + perp * ang.cos() + b.dir.cross(perp) * ang.sin()
        };
        let sign = if rotate(b.angle).distance(p2) <= rotate(-b.angle).distance(p2) { 1.0 } else { -1.0 };
        let res = k::revolve(&face, b.axis_point, b.dir * sign, b.angle).map_err(|e| fe(format!("Sheet metal bend failed: {e}")))?;
        let (v0, v1) = (topo.nearest_vertex(b.start), topo.nearest_vertex(b.start + b.dir * b.len));
        for f in res.shape.faces() {
            let from = match f.face_surface() {
                Ok(k::Surface::Plane(pl)) if pl.unit_normal().dot(b.dir).abs() > 1.0 - 1e-9 => {
                    // The bend's end faces: the definition vertex at that end.
                    let c = face_normal(&f).map(|x| x.0).unwrap_or(b.start);
                    if (c - b.start).dot(b.dir) < b.len * 0.5 { Def::Vertex(v0) } else { Def::Vertex(v1) }
                }
                _ => Def::Edge(e),
            };
            pieces.push(Piece { face: f, from });
        }
        solids.push(res.shape);
    }
    // Walls: pulled back from their bends and thickened.
    for (fi, face) in topo.faces.iter().enumerate() {
        let l = wall_loop(face, &topo)?;
        let (_, n) = face_normal(face)?;
        let offset: Vec<f64> = l.side.iter().map(|e| setbacks.get(e).copied().unwrap_or(0.0)).collect();
        let pts: Vec<DVec3> = pulled_loop(&l, n, &offset).into_iter().map(|p| p - n * back).collect();
        let wire = k::make_polyline(&pts, true).map_err(|e| fe(e.to_string()))?;
        let base = k::make_face_from_wire(&wire, true).map_err(|e| fe(e.to_string()))?;
        let slab = k::extrude(&base, n * (front + back)).map_err(|e| fe(format!("Sheet metal wall failed: {e}")))?.shape;
        for f in slab.faces() {
            let from = match f.face_surface() {
                Ok(k::Surface::Plane(pl)) if pl.unit_normal().cross(n).length() < 1e-9 => Def::Face(fi),
                _ => {
                    // A side face: the loop side it was swept from.
                    let c = face_normal(&f).map(|x| x.0).unwrap_or_default();
                    let k = (0..l.pts.len())
                        .min_by(|a, b| {
                            let d = |k: usize| {
                                let (p, q) = (pts[k], pts[(k + 1) % pts.len()]);
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
            let mut bends = BTreeMap::new();
            for e in 0..body.edges.len() {
                if let Some(j) = sm_attribute(&st, &q.to_ref(&Ent::Edge(b, e)), "JOINT")
                    && j.get("jointType").get("value").as_str().is_some_and(|t| &*t == "BEND")
                {
                    bends.insert(e, length_field(&j, "radius"));
                }
            }
            // The part made from it before: the solid carrying its association.
            let own = associations(&st, &body_ref);
            let previous = st.ctx.bodies.iter().enumerate().find(|(pb, p)| p.kind == BodyKind::Solid && !own.is_empty() && associations(&st, &q.to_ref(&Ent::Body(*pb))).iter().any(|x| own.contains(x))).map(|(_, p)| p.id.clone());
            defs.push(Definition { body: body.id.clone(), front: length_field(&model, "frontThickness"), back: length_field(&model, "backThickness"), bends, previous });
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
                st.attributes.push((to.clone(), Value::map_from([("attribute", attr)])));
            }
        }
        if let Reference::Body(def_body) = &body_ref {
            st.hidden.insert(def_body.clone());
        }
    }
    Ok(Value::Undefined)
}
