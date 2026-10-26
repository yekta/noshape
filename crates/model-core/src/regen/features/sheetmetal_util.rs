//! Shared helpers for the sheet metal evaluators: model data access, local
//! frames, oriented tool solids, named booleans and bend measurement.

use crate::naming::{Name, ShapeNames};
use crate::reference::{FeatureId, Role, Source, TopoName};
use crate::regen::context::Body;
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape, err};
use crate::sheetmetal::{BendReliefStyle, SheetMetalData};
use glam::{DVec2, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn fe(m: impl Into<String>) -> FeatureError {
    FeatureError::new(m)
}

/// Sub-name "piece `k` of `parent` made by `by`" used to give the faces a
/// sheet metal feature creates from one selection distinct, stable roles.
pub fn piece(parent: &Name, by: FeatureId, k: u16) -> Name {
    Arc::new(TopoName::Split { parent: parent.clone(), by, ordinal: k })
}

/// Source for profile segment `k` of the geometry built from `parent`.
pub fn piece_source(parent: &Name, by: FeatureId, k: u16) -> Source {
    Source::Topo(piece(parent, by, k))
}

/// Face name of profile segment `k` swept by `feature` (see `sweep_names`).
pub fn side_name(feature: FeatureId, parent: &Name, k: u16) -> Name {
    Arc::new(TopoName::face(feature, Role::Side(piece_source(parent, feature, k))))
}

/// Read the model parameters of a Sheet metal model feature.
pub fn model_from_params(ev: &EvalCtx, t: f64) -> FResult<SheetMetalData> {
    let mut d = SheetMetalData::new(ev.feature, t, ev.length("radius")?, ev.real("kFactor").unwrap_or(0.45));
    d.k_factor_rolled = ev.real("kFactorRolled").unwrap_or(0.5);
    d.minimal_clearance = ev.length("minimalClearance").unwrap_or(2e-5).max(1e-6);
    d.bend_calculation = ev.enum_("bendCalculationType").to_string();
    if d.bend_calculation.is_empty() {
        d.bend_calculation = "K_FACTOR".into();
    }
    d.bend_relief = BendReliefStyle::from_id(ev.enum_("defaultBendReliefStyle"));
    d.bend_relief_depth_scale = ev.real("defaultBendReliefDepthScale").unwrap_or(2.0);
    d.bend_relief_width_scale = ev.real("defaultBendReliefScale").unwrap_or(1.0625);
    d.corner_relief = ev.enum_("defaultCornerStyle").to_string();
    d.flip_direction_up = ev.bool("flipDirectionUp");
    if d.radius <= 0.0 {
        return err("Bend radius must be positive");
    }
    Ok(d)
}

/// Sheet metal model of body `bi` if it is active.
pub fn active_data(ev: &EvalCtx, bi: usize) -> FResult<Arc<SheetMetalData>> {
    let b = &ev.ctx.bodies[bi];
    match ev.ctx.sheet_metal.get(&b.id) {
        Some(d) if d.active => Ok(d.clone()),
        Some(_) => err(format!("{} is no longer an active sheet metal model (Finish sheet metal model was applied)", b.name)),
        None => err(format!("{} is not a sheet metal part; select geometry of an active sheet metal model", b.name)),
    }
}

pub fn put_data(ev: &mut EvalCtx, bi: usize, data: SheetMetalData) {
    let id = ev.ctx.bodies[bi].id.clone();
    ev.ctx.sheet_metal.insert(id, Arc::new(data));
}

/// Box with edges along the orthonormal axes `ex`, `ey`, `ex × ey`, spanning
/// the given coordinate ranges relative to `origin`.
pub fn oriented_box(origin: DVec3, ex: DVec3, ey: DVec3, x: [f64; 2], y: [f64; 2], z: [f64; 2]) -> FResult<Shape> {
    let ez = ex.cross(ey);
    let (x0, x1) = (x[0].min(x[1]), x[0].max(x[1]));
    let (y0, y1) = (y[0].min(y[1]), y[0].max(y[1]));
    let (z0, z1) = (z[0].min(z[1]), z[0].max(z[1]));
    if x1 - x0 < 1e-12 || y1 - y0 < 1e-12 || z1 - z0 < 1e-12 {
        return err("Degenerate tool geometry");
    }
    let o = origin + ex * x0 + ey * y0 + ez * z0;
    Ok(k::make_box(&k::Plane::new(o, ez, ex), x1 - x0, y1 - y0, z1 - z0)?)
}

/// Planar face from 2D segments in the frame (`origin`, `ex`, `ey`).
pub fn local_face(origin: DVec3, ex: DVec3, ey: DVec3, segs: &[k::Segment2d]) -> FResult<k::TrackedProfile> {
    let plane = k::Plane::new(origin, ex.cross(ey), ex);
    Ok(k::make_face_2d_tracked(&plane, segs, &[])?)
}

/// Names for tool faces: every face gets `name(i)` (ordered by index).
pub fn tool_named(shape: Shape, name: impl Fn(u32) -> Name) -> NamedShape {
    let n = shape.count(ShapeType::Face) as u32;
    let faces = (0..n).map(name).collect();
    NamedShape::new(shape, faces)
}

/// Boolean of a body with tools, naming result faces from both. Returns the
/// result shape and its face names.
pub fn bool_named(ev: &EvalCtx, op: k::BooleanOp, body: &NamedShape, tools: &[NamedShape]) -> FResult<(Shape, Vec<Name>)> {
    let shapes: Vec<Shape> = tools.iter().map(|t| t.shape.clone()).collect();
    let res = k::boolean(op, std::slice::from_ref(&body.shape), &shapes, &k::BooleanOptions::default())?;
    let mut args: Vec<&ShapeNames> = vec![&body.names];
    args.extend(tools.iter().map(|t| &t.names));
    let names = ev.name_result(&args, &res.history, &res.shape, &BTreeMap::new(), None);
    Ok((res.shape, names))
}

pub fn body_named(b: &Body) -> NamedShape {
    NamedShape { shape: b.shape.clone(), names: b.shape_names() }
}

/// Number of solids (a compound of one solid counts as one).
pub fn solid_count(s: &Shape) -> usize {
    s.count(ShapeType::Solid)
}

/// Endpoints of a linear edge.
pub fn line_of(e: &Shape) -> Option<(DVec3, DVec3)> {
    match e.edge_curve().ok()? {
        k::Curve::Line { .. } => e.endpoints().ok(),
        _ => None,
    }
}

/// Outward plane of a planar face: (point, unit normal).
pub fn plane_of(f: &Shape) -> Option<(DVec3, DVec3)> {
    let p = f.face_plane().ok()??;
    Some((f.centroid().unwrap_or(p.origin), p.unit_normal()))
}

/// Faces of a body matching any of the names.
pub fn faces_named(body: &Body, names: &[Name]) -> Vec<usize> {
    let mut v: Vec<usize> = names.iter().flat_map(|n| body.find_faces(n)).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Current names of the faces matching any of the names (follows splits).
pub fn current_names(body: &Body, names: &[Name]) -> Vec<Name> {
    faces_named(body, names).into_iter().map(|i| body.faces[i].clone()).collect()
}

/// Whether the edge between two planar faces (outward normals `n1`, `n2`)
/// is convex: a point slightly off the edge on the outer side of face 1 and
/// the inner side of face 2 is outside the material only at convex edges.
pub fn is_convex(shape: &Shape, mid: DVec3, n1: DVec3, n2: DVec3, probe: f64) -> bool {
    let d = n1 - n2;
    if d.length() < 1e-9 {
        return true;
    }
    !matches!(shape.classify_point(mid + d.normalize() * probe, probe * 1e-3), Ok(k::PointClass::Inside))
}

/// Geometry of a bend measured from its axis and the adjacent walls.
pub struct MeasuredBend {
    pub axis: DVec3,
    pub angle: f64,
    pub dir_a: DVec3,
    pub radial_a: DVec3,
    pub ref_inner: bool,
    pub s0: f64,
    pub s1: f64,
}

/// Measure a bend: axis line (`o`, `axis`), inner radius `r`, thickness `t`,
/// parent reference face (point, outward normal, centroid), child face
/// (normal, centroid) and the axial extent `[s0, s1]` from `o`.
#[allow(clippy::too_many_arguments)]
pub fn measure_bend(o: DVec3, axis: DVec3, r: f64, t: f64, pa: DVec3, na: DVec3, ca: DVec3, nb: DVec3, cb: DVec3, s: (f64, f64)) -> MeasuredBend {
    let mut a = axis.normalize_or_zero();
    let dist = (pa - o).dot(na);
    let radial_a = if dist >= 0.0 { na } else { -na };
    let rho = dist.abs();
    let ref_inner = (rho - r).abs() <= (rho - (r + t)).abs();
    let mut dir_a = a.cross(na).normalize_or_zero();
    if (ca - o).dot(dir_a) < 0.0 {
        dir_a = -dir_a;
    }
    let mut dir_b = a.cross(nb).normalize_or_zero();
    if (cb - o).dot(dir_b) < 0.0 {
        dir_b = -dir_b;
    }
    let from = -dir_a;
    let mut angle = from.cross(dir_b).dot(a).atan2(from.dot(dir_b));
    let (mut s0, mut s1) = s;
    if angle < 0.0 {
        a = -a;
        angle = -angle;
        (s0, s1) = (-s1, -s0);
    }
    MeasuredBend { axis: a, angle, dir_a, radial_a, ref_inner, s0, s1 }
}

/// Extent of a shape's vertices along an axis.
pub fn extent_along(s: &Shape, o: DVec3, axis: DVec3) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for v in s.vertices() {
        if let Ok(p) = v.vertex_point() {
            let d = (p - o).dot(axis);
            lo = lo.min(d);
            hi = hi.max(d);
        }
    }
    if lo > hi { (0.0, 0.0) } else { (lo, hi) }
}

/// Arc as a three-point segment (center, radius, start angle, signed sweep).
pub fn arc3(c: DVec2, r: f64, a0: f64, sweep: f64) -> k::Segment2d {
    let at = |t: f64| c + DVec2::from_angle(t) * r;
    k::Segment2d::Arc3 { a: at(a0), mid: at(a0 + sweep * 0.5), b: at(a0 + sweep) }
}

/// Relief tool profile in a local (u, s) frame: a slot spanning `s` in
/// `[s0, s0 + w]` (w may be negative) and `u` from `u_deep` up to `u_top`,
/// rounded at the deep end for obround reliefs.
pub fn relief_segments(style: BendReliefStyle, u_deep: f64, u_top: f64, s0: f64, w: f64) -> Vec<k::Segment2d> {
    let (sa, sb) = (s0.min(s0 + w), s0.max(s0 + w));
    let width = sb - sa;
    let line = |a: DVec2, b: DVec2| k::Segment2d::Line { a, b };
    let p = |u: f64, s: f64| DVec2::new(u, s);
    if style == BendReliefStyle::Obround && u_top - u_deep > width * 0.5 + 1e-9 {
        let uc = u_deep + width * 0.5;
        let c = p(uc, (sa + sb) * 0.5);
        vec![
            line(p(uc, sa), p(u_top, sa)),
            line(p(u_top, sa), p(u_top, sb)),
            line(p(u_top, sb), p(uc, sb)),
            arc3(c, width * 0.5, std::f64::consts::FRAC_PI_2, std::f64::consts::PI),
        ]
    } else {
        vec![line(p(u_deep, sa), p(u_top, sa)), line(p(u_top, sa), p(u_top, sb)), line(p(u_top, sb), p(u_deep, sb)), line(p(u_deep, sb), p(u_deep, sa))]
    }
}

/// Resolve the single (body, edge) of an edge reference, or the long edge
/// of a side face reference (preferring the edge on the reference side).
pub fn resolve_flange_edge(ev: &EvalCtx, r: &crate::reference::Reference) -> FResult<Vec<(usize, usize)>> {
    use crate::reference::Reference;
    match r {
        Reference::Edge(_) => Ok(ev.resolve_edges(std::slice::from_ref(r))),
        Reference::Face(_) => {
            let mut out = Vec::new();
            for (bi, fi) in ev.resolve_faces(std::slice::from_ref(r)) {
                let body = &ev.ctx.bodies[bi];
                let data = ev.ctx.sheet_metal.get(&body.id);
                let ef = body.shape.edge_faces()?;
                let mut best: Option<(usize, f64, bool)> = None;
                for (ei, faces) in ef.iter().enumerate() {
                    if !faces.contains(&(fi as u32)) || faces.len() != 2 {
                        continue;
                    }
                    let other = if faces[0] as usize == fi { faces[1] } else { faces[0] } as usize;
                    let on = data.and_then(|d| d.wall_of_face(&body.faces[other]));
                    let Some((_, on_ref)) = on else { continue };
                    let e = body.shape.sub_shape(ShapeType::Edge, ei)?;
                    let Some((a, b)) = line_of(&e) else { continue };
                    let len = (b - a).length();
                    let better = match best {
                        None => true,
                        Some((_, l, r0)) => (on_ref && !r0) || (on_ref == r0 && len > l + 1e-12),
                    };
                    if better {
                        best = Some((ei, len, on_ref));
                    }
                }
                match best {
                    Some((ei, _, _)) => out.push((bi, ei)),
                    None => return err("Selected side face is not on the free edge of a sheet metal wall"),
                }
            }
            Ok(out)
        }
        _ => Ok(Vec::new()),
    }
}
