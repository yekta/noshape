//! Fillets with Onshape's cross sections where OCCT has none: conic (rho),
//! asymmetric (two distances: an elliptic section), partial fillets (flat caps
//! at the bounds) and fillets that stop at the selected edges (no tangent
//! propagation; the section runs straight on into the tangent neighbour).
//!
//! Per edge, the region between the edge and the section curve is swept
//! along it (a prism for lines, a revolution for arcs) and cut away (convex
//! edges) or added (concave edges). Supported edges: lines between planes,
//! arcs between faces whose section through the arc's axis is a line (planes
//! across the axis, coaxial cylinders and cones).

use crate::naming::Name;
use crate::reference::{Role, TopoName};
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape};
use glam::DVec3;
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Cross-section curve between the two tangent lines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Section {
    /// Circular arc (equal distances) or the elliptic arc of an asymmetric fillet.
    Circular,
    /// Conic through the tangent points with shoulder `rho` (0.5 = parabola).
    Conic(f64),
    /// Straight line between the tangent points (a chamfer).
    Chamfer,
    /// Curvature continuous with the faces (Onshape "Curvature", magnitude m):
    /// see [`curvature_profile`].
    Curvature(f64),
}

#[derive(Clone, Debug)]
pub struct BlendSpec {
    pub section: Section,
    /// Radius on the first and second face of each edge (asymmetric: different).
    pub r1: f64,
    pub r2: f64,
    /// Width measurement: the chord between the tangent lines (equal distances).
    pub width: Option<f64>,
    /// Partial fillet bounds (None = the whole selection).
    pub partial: Option<Partial>,
    /// Run on past ends that continue tangentially into unselected edges.
    pub run_on: bool,
    /// Allow edge overflow off: a ball wider than a face rests on that face's
    /// far edge (the cliff) instead (live oracle_p1 F7).
    pub cliff: bool,
    /// Face blend: the radii are offsets of side 1 and side 2 (see [`Sides`]).
    pub sides: Option<Sides>,
}

/// Face blend sizes (live psb 10-26 b, L brackets at 60°, 90° and 135°): the
/// section's centre lies at `r1` from side 1 and `r2` from side 2 and it
/// touches each side at the foot of the perpendicular from that centre, so
/// a setback is (d_other + d_own·cos θ) / sin θ, θ the angle between the
/// sides. A width is the chord between the touch points, with the offsets in
/// the ratio side 1 : side 2 = `ratio`. A setback at or behind the corner
/// fails like live ("Provided dimension is too small.").
#[derive(Clone, Debug)]
pub struct Sides {
    /// Faces of side 1 (indices in the body).
    pub side1: Vec<usize>,
    pub ratio: f64,
}

/// A partial fillet bound along the selected chain (position 0 = the chain's
/// start, see [`chain`]).
#[derive(Clone, Copy, Debug)]
pub enum Bound {
    /// Fraction of the chain's length.
    Fraction(f64),
    /// Distance along the chain.
    Offset(f64),
    /// Distance back from the chain's end (a second bound's offset).
    OffsetFromEnd(f64),
    /// The chain point nearest this point (a vertex, or a face's nearest point).
    Point(DVec3),
}

#[derive(Clone, Copy, Debug)]
pub struct Partial {
    pub first: Bound,
    pub second: Option<Bound>,
    /// One bound: true covers first → end (Onshape's default arrow), false start → first.
    pub opposite: bool,
}

fn fail(why: impl std::fmt::Display) -> FeatureError {
    log::debug!("section blend: {why}");
    FeatureError::new("Failed to fillet selections.")
}

/// One selected edge, oriented along the chain.
struct ChainEdge {
    index: usize,
    shape: Shape,
    /// Chain-oriented start and end points and length.
    start: DVec3,
    end: DVec3,
    length: f64,
    /// Whether the chain runs along the edge's own parameter direction.
    forward: bool,
}

/// Order the edges as one chain from the end with the smallest (x, y, z)
/// (Onshape's position 0 on a block's edge is its end nearest the origin).
/// Edges that don't form one chain keep their order, each its own piece.
fn chain(shape: &Shape, edges: &[usize]) -> FResult<Vec<ChainEdge>> {
    let mut items = Vec::new();
    for &i in edges {
        let e = shape.sub_shape(ShapeType::Edge, i)?;
        let info = e.edge_info()?;
        items.push((i, e, info.start, info.end, info.length));
    }
    let key = |p: DVec3| (p.x, p.y, p.z);
    let lt = |a: DVec3, b: DVec3| key(a).partial_cmp(&key(b)) == Some(std::cmp::Ordering::Less);
    let tol = 1e-9;
    let near = |a: DVec3, b: DVec3| (a - b).length() < tol;
    // Free ends: points used by exactly one edge.
    let ends: Vec<DVec3> = items.iter().flat_map(|it| [it.2, it.3]).collect();
    let count = |p: DVec3| ends.iter().filter(|q| near(**q, p)).count();
    let mut start = items.iter().flat_map(|it| [it.2, it.3]).filter(|p| count(*p) == 1).fold(None, |m: Option<DVec3>, p| match m {
        Some(q) if !lt(p, q) => Some(q),
        _ => Some(p),
    });
    let mut out = Vec::new();
    let mut left = items;
    while !left.is_empty() {
        let at = start.unwrap_or(left[0].2);
        let pos = left.iter().position(|it| near(it.2, at) || near(it.3, at)).unwrap_or(0);
        let (index, shape, a, b, length) = left.remove(pos);
        let forward = near(a, at) || !near(b, at);
        let (s, e) = if forward { (a, b) } else { (b, a) };
        start = Some(e);
        out.push(ChainEdge { index, shape, start: s, end: e, length, forward });
    }
    Ok(out)
}

/// Std fillet.fs's partial fillet checks (getPath and the bounds), with
/// its messages: one open chain (or a closed one with two bounds), offsets
/// within the chain, two distinct bounds.
pub fn check_partial(shape: &Shape, edges: &[usize], p: &Partial) -> FResult<()> {
    let tol = 1e-9;
    let mut ends = Vec::new();
    for &i in edges {
        let info = shape.sub_shape(ShapeType::Edge, i)?.edge_info()?;
        ends.extend([info.start, info.end]);
    }
    if ends.iter().any(|a| ends.iter().filter(|b| (**b - *a).length() < tol).count() > 2) {
        return Err(FeatureError::new("Edges do not form a continuous path."));
    }
    let chain = chain(shape, edges)?;
    if chain.windows(2).any(|w| (w[0].end - w[1].start).length() > tol) {
        return Err(FeatureError::new("Only one edge or one chain of tangent edges can be partially fillet."));
    }
    let closed = chain.first().zip(chain.last()).is_some_and(|(a, b)| (a.start - b.end).length() < tol);
    if closed && p.second.is_none() {
        return Err(FeatureError::new("Closed entity requires a second boundary point."));
    }
    let total: f64 = chain.iter().map(|c| c.length).sum();
    let parameter = |b: Bound, second: bool| -> FResult<Option<f64>> {
        let t = match b {
            Bound::Fraction(f) => f,
            Bound::Offset(d) => d / total,
            Bound::OffsetFromEnd(d) => (total - d) / total,
            Bound::Point(_) => return Ok(None),
        };
        if matches!(b, Bound::Offset(_) | Bound::OffsetFromEnd(_)) && !(0.0..=1.0).contains(&t) {
            let field = if second { "endPartialOffset" } else { "startPartialOffset" };
            return Err(FeatureError { faulty: vec![field], ..FeatureError::new("Enter a value less than the length of the selected edge(s).") });
        }
        Ok(Some(t))
    };
    let a = parameter(p.first, false)?;
    if let Some(sb) = p.second
        && let (Some(a), Some(b)) = (a, parameter(sb, true)?)
        && ((a - b) * total).abs() < 1e-8
    {
        return Err(FeatureError::new("Boundary points cannot be equal."));
    }
    Ok(())
}

/// Directions at a point of an edge: into each face (perpendicular to the
/// edge) and toward each face's material, from the solid itself.
struct Frame {
    u: [DVec3; 2],
    m: [DVec3; 2],
    /// How far each face reaches from the edge along `u` (infinite: unknown).
    extent: [f64; 2],
}

fn frame_at(body: &Shape, faces: [&Shape; 2], p: DVec3, tangent: DVec3, scale: f64) -> FResult<Frame> {
    let d = scale * 1e-4;
    let mut u = [DVec3::ZERO; 2];
    let mut m = [DVec3::ZERO; 2];
    for i in 0..2 {
        let n = faces[i].normal_at_point(p)?.normalize();
        let dir = n.cross(tangent).normalize();
        let mut found = false;
        // Along the face: the side the face lies on (at a concave edge both
        // sides of the edge have material below them).
        let on_face = |su: f64| -> FResult<bool> {
            let q = k::make_vertex(p + dir * (su * d))?;
            Ok(k::distance(&q, faces[i])?.distance < d * 1e-3)
        };
        let sides: Vec<f64> = [1.0, -1.0].into_iter().filter(|su| on_face(*su).unwrap_or(false)).collect();
        let sides = if sides.is_empty() { vec![1.0, -1.0] } else { sides };
        'search: for su in sides {
            for sn in [-1.0, 1.0] {
                let q = p + dir * (su * d) + n * (sn * d);
                if body.classify_point(q, d * 1e-3)? == k::PointClass::Inside {
                    u[i] = dir * su;
                    m[i] = n * sn;
                    found = true;
                    break 'search;
                }
            }
        }
        if !found {
            return Err(fail("could not orient a face at the edge"));
        }
    }
    Ok(Frame { u, m, extent: [f64::INFINITY; 2] })
}

/// Setbacks along the two faces of a frame (face 0, face 1); `first_is_side1`
/// says which face is side 1 of a face blend.
fn setbacks(f: &Frame, spec: &BlendSpec, first_is_side1: bool) -> FResult<(f64, f64)> {
    let half = f.m[0].dot(f.m[1]).clamp(-1.0, 1.0).acos() / 2.0;
    let Some(sides) = &spec.sides else {
        return Ok(match spec.width {
            Some(w) => {
                let s = w / (2.0 * half.cos());
                (s, s)
            }
            None => (spec.r1 * half.tan(), spec.r2 * half.tan()),
        });
    };
    let c = f.u[0].dot(f.u[1]).clamp(-1.0, 1.0);
    let sn = (1.0 - c * c).sqrt();
    if sn < 1e-9 {
        return Err(fail("the faces at the edge are tangent"));
    }
    let (mut d0, mut d1) = if spec.width.is_some() { (sides.ratio, 1.0) } else { (spec.r1, spec.r2) };
    if !first_is_side1 {
        std::mem::swap(&mut d0, &mut d1);
    }
    let (mut t0, mut t1) = ((d1 + d0 * c) / sn, (d0 + d1 * c) / sn);
    if let Some(w) = spec.width {
        let k = w / (f.u[0] * t0 - f.u[1] * t1).length().max(1e-300);
        (t0, t1) = (t0 * k, t1 * k);
    }
    let size = t0.abs().max(t1.abs()).max(1e-300);
    if t0 <= size * 1e-9 || t1 <= size * 1e-9 {
        return Err(FeatureError::new("Provided dimension is too small."));
    }
    Ok((t0, t1))
}

/// The section face at `p` (2D region between the corner, the tangent points
/// and the curve) and its three edges (corner → T1, curve, T2 → corner).
fn section_face(p: DVec3, f: &Frame, spec: &BlendSpec, first_is_side1: bool) -> FResult<(Shape, [Shape; 3], bool)> {
    let convex = f.u[0].dot(f.m[1]) > 0.0;
    let cos_phi = f.m[0].dot(f.m[1]).clamp(-1.0, 1.0);
    let phi = cos_phi.acos();
    if phi < 1e-6 || phi > std::f64::consts::PI - 1e-6 {
        return Err(fail("the faces at the edge are tangent"));
    }
    let half = phi / 2.0;
    let (s1, s2) = setbacks(f, spec, first_is_side1)?;
    let (t1, t2) = (p + f.u[0] * s1, p + f.u[1] * s2);
    // A ball wider than a face: tangent to the other face, through the
    // cliff (that face's far edge).
    if spec.cliff && matches!(spec.section, Section::Circular) && (s1 - s2).abs() <= 1e-12 * s1.max(s2) && (s1 > f.extent[0] || s2 > f.extent[1]) {
        let i = if s1 > f.extent[0] { 0 } else { 1 };
        let j = 1 - i;
        if [s1, s2][j] > f.extent[j] {
            return Err(fail("the fillet is wider than both faces"));
        }
        let r = s1 / half.tan();
        let cliff = p + f.u[i] * f.extent[i];
        let w = if convex { f.m[j] } else { -f.m[j] } * r;
        let v = w - f.u[i] * f.extent[i];
        let b = f.u[j].dot(v);
        let disc = b * b - v.length_squared() + r * r;
        let s = if disc >= 0.0 { [-b + disc.sqrt(), -b - disc.sqrt()].into_iter().filter(|s| *s > 0.0 && *s <= [s1, s2][j] * (1.0 + 1e-9)).fold(f64::NAN, f64::max) } else { f64::NAN };
        if !s.is_finite() {
            return Err(fail("no cliff fillet fits"));
        }
        let t = p + f.u[j] * s;
        let c = t + w;
        let arc = k::make_arc_3pts(t, c + ((t - c) + (cliff - c)).normalize() * r, cliff)?;
        let edges = if j == 0 { [k::make_line(p, t)?, arc, k::make_line(cliff, p)?] } else { [k::make_line(p, cliff)?, arc, k::make_line(t, p)?] };
        let wire = k::make_wire(&edges)?;
        return Ok((k::make_face_from_wire(&wire, true)?, edges, convex));
    }
    let curve = match spec.section {
        Section::Circular if (s1 - s2).abs() <= 1e-12 * s1.max(s2) => {
            let r = s1 / half.tan();
            let c = t1 + if convex { f.m[0] } else { -f.m[0] } * r;
            k::make_arc_3pts(t1, c + (p - c).normalize() * r, t2)?
        }
        Section::Circular => rational(t1, p, t2, half.cos())?,
        Section::Conic(rho) => rational(t1, p, t2, rho / (1.0 - rho))?,
        Section::Chamfer => k::make_line(t1, t2)?,
        Section::Curvature(m) => curvature_profile(p, t1, t2, m)?,
    };
    let edges = [k::make_line(p, t1)?, curve, k::make_line(t2, p)?];
    let wire = k::make_wire(&edges)?;
    let face = k::make_face_from_wire(&wire, true)?;
    Ok((face, edges, convex))
}

/// Onshape's curvature-continuous blend section (live G11, exact): a quintic
/// B-spline, knots 0⁶ ½² 1⁶, from tangent point `a` to `b` around `corner`.
/// In units of the setbacks along the two faces, the control points are
/// (1,0) (1−0.3m,0) (1−0.6m,0) (u,v) (v,u) (0,1−0.6m) (0,1−0.3m) (0,1): the
/// first three on a face make it G2 to the face, u = ⅔(1−m), and v puts the
/// midpoint at (1−m)/2 from the corner on each face; when that would leave
/// the face, v stays at 0.001 and u keeps the midpoint (m = 0.8).
fn curvature_profile(corner: DVec3, a: DVec3, b: DVec3, m: f64) -> FResult<Shape> {
    if !(0.0..1.0).contains(&m) {
        return Err(fail("curvature magnitude out of range"));
    }
    let (da, db) = (a - corner, b - corner);
    let at = |x: f64, y: f64| corner + da * x + db * y;
    // Weights of the control points at t = ½: 1/8, 3/8, 3/8, 1/8 (points 2..5).
    let mid = (1.0 - m) / 2.0;
    let mut p2 = 1.0 - 0.6 * m;
    let mut u = 2.0 / 3.0 * (1.0 - m);
    let mut v = (mid - 0.125 * p2 - 0.375 * u) / 0.375;
    if v < 0.001 {
        v = 0.001;
        u = (mid - 0.125 * p2 - 0.375 * v) / 0.375;
    }
    // Above 0.8 Parasolid moves the third points too (live "kern (10-24
    // 00:30) curvature section": exact poles at 0.8 / 0.9 / 0.95 / 0.99, in
    // setbacks (p2, v) below; the second points stay at 1 − 0.3m and the
    // midpoint at (1−m)/2). The rule behind them isn't known: p2 and v are
    // interpolated between those, u keeps the midpoint; above 0.99 the inner
    // points close in on the corner with 1 − m.
    const HIGH: [(f64, f64, f64); 4] = [(0.8, 0.52, 0.001), (0.9, 0.3670668738876986, 0.0013922239490643808), (0.95, 0.1615265245644751, 0.0005127210380370142), (0.99, 0.01073877481929723, 0.0031029341051257)];
    if m > HIGH[0].0 {
        (p2, v) = match HIGH.windows(2).find(|w| m <= w[1].0) {
            Some(w) => {
                let f = (m - w[0].0) / (w[1].0 - w[0].0);
                (w[0].1 + (w[1].1 - w[0].1) * f, w[0].2 + (w[1].2 - w[0].2) * f)
            }
            None => {
                let k = (1.0 - m) / (1.0 - HIGH[3].0);
                (HIGH[3].1 * k, HIGH[3].2 * k)
            }
        };
        u = (mid - 0.125 * p2) / 0.375 - v;
    }
    let pts = [at(1.0, 0.0), at(1.0 - 0.3 * m, 0.0), at(p2, 0.0), at(u, v), at(v, u), at(0.0, p2), at(0.0, 1.0 - 0.3 * m), at(0.0, 1.0)];
    Ok(k::make_bspline(&pts, None, &[0.0, 0.5, 1.0], &[6, 2, 6], 5, false)?)
}

fn rational(a: DVec3, corner: DVec3, b: DVec3, w: f64) -> FResult<Shape> {
    if w.is_nan() || w <= 0.0 {
        return Err(fail("degenerate conic"));
    }
    Ok(k::make_bspline(&[a, corner, b], Some(&[1.0, w, 1.0]), &[0.0, 1.0], &[3, 3], 2, false)?)
}

/// Names of the tool faces: the swept curve is the fillet face (`Blend(edge)`);
/// caps and the faces on the supports are generated by the edge.
fn tool_names(feature: crate::reference::FeatureId, tool: &k::OpResult, profile: &Shape, curve: &Shape, edge: &Name) -> Vec<Name> {
    let n = tool.shape.count(ShapeType::Face);
    let blend = Arc::new(TopoName::face(feature, Role::Blend(edge.clone())));
    let generated = Arc::new(TopoName::face(feature, Role::Generated(edge.clone())));
    let curve_faces: Vec<u32> = profile
        .index_of(curve, ShapeType::Edge)
        .map(|i| tool.history.generated(0, ShapeType::Edge, i as u32).into_iter().filter(|g| g.0 == ShapeType::Face).map(|g| g.1).collect())
        .unwrap_or_default();
    (0..n as u32).map(|i| if curve_faces.contains(&i) { blend.clone() } else { generated.clone() }).collect()
}

/// Whether an edge end continues tangentially into another edge of the body.
fn tangent_continues(body: &Shape, at: DVec3, dir: DVec3, skip: usize) -> bool {
    body.edges().iter().enumerate().any(|(j, e)| {
        if j == skip {
            return false;
        }
        let Ok(info) = e.edge_info() else { return false };
        // The curve's parameter at a point (start / end follow the edge's
        // orientation, first / last parameter the curve's).
        let param = |q: DVec3| if e.point_at(info.first_param).is_ok_and(|x| (x - q).length() < 1e-9) { info.first_param } else { info.last_param };
        let t = if (info.start - at).length() < 1e-9 {
            e.tangent_at(param(info.start)).ok()
        } else if (info.end - at).length() < 1e-9 {
            e.tangent_at(param(info.end)).ok().map(|v| -v)
        } else {
            None
        };
        t.is_some_and(|t| t.normalize().dot(dir) > 0.9998)
    })
}

/// Blend `edges` of body `bi` with `spec` (see the module docs).
pub fn section_blend(ev: &mut EvalCtx, bi: usize, edges: &[usize], spec: &BlendSpec) -> FResult<()> {
    use super::sheetmetal_util::{body_named, bool_named};
    let body = ev.ctx.bodies[bi].clone();
    let shape = &body.shape;
    let faces = shape.faces();
    let ef = shape.edge_faces()?;
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let chain = chain(shape, edges)?;
    let total: f64 = chain.iter().map(|c| c.length).sum();
    // Distance along the chain of a bound.
    let along = |bd: Bound| -> FResult<f64> {
        Ok(match bd {
            Bound::Fraction(f) => f.clamp(0.0, 1.0) * total,
            Bound::Offset(d) => d.clamp(0.0, total),
            Bound::OffsetFromEnd(d) => (total - d).clamp(0.0, total),
            Bound::Point(q) => {
                let (mut best, mut acc, mut where_) = (f64::INFINITY, 0.0, 0.0);
                for c in &chain {
                    for i in 0..=64 {
                        let t = i as f64 / 64.0;
                        let info = c.shape.edge_info()?;
                        let prm = info.first_param + (info.last_param - info.first_param) * if c.forward { t } else { 1.0 - t };
                        let dist = (c.shape.point_at(prm)? - q).length();
                        if dist < best {
                            best = dist;
                            where_ = acc + t * c.length;
                        }
                    }
                    acc += c.length;
                }
                where_
            }
        })
    };
    let (lo, hi) = match spec.partial {
        None => (0.0, total),
        Some(p) => {
            let a = along(p.first)?;
            match p.second {
                Some(sb) => {
                    let b = along(sb)?;
                    (a.min(b), a.max(b))
                }
                None if p.opposite => (a, total),
                None => (0.0, a),
            }
        }
    };
    if hi - lo <= 1e-12 * total {
        return Err(fail("empty partial range"));
    }
    let partial = spec.partial.is_some();
    let feature = ev.feature;
    let (mut cuts, mut adds) = (Vec::new(), Vec::new());
    let mut at = 0.0;
    for (ci, c) in chain.iter().enumerate() {
        let (a, b) = ((lo - at).max(0.0), (hi - at).min(c.length));
        at += c.length;
        if b <= a {
            continue;
        }
        let fs = ef.get(c.index).cloned().unwrap_or_default();
        if fs.len() != 2 {
            return Err(fail("the edge doesn't have two faces"));
        }
        let ff = [&faces[fs[0] as usize], &faces[fs[1] as usize]];
        let side1_first = spec.sides.as_ref().is_none_or(|sd| sd.side1.contains(&(fs[0] as usize)));
        let info = c.shape.edge_info()?;
        let ename = body.edges[c.index].clone();
        let r_ext = 2.0 * spec.r1.max(spec.r2).max(spec.width.unwrap_or(0.0));
        // How far to run on past a bound: only at the chain's ends, not at partial bounds.
        let ext_at = |is_start: bool| -> FResult<f64> {
            let (end_of_chain, at_bound) = if is_start { (ci == 0, partial && a > 1e-12) } else { (ci + 1 == chain.len(), partial && b < c.length - 1e-12) };
            // A closed edge has no free end.
            if !end_of_chain || at_bound || (c.start - c.end).length() < 1e-9 {
                return Ok(0.0);
            }
            let (p, out) = if is_start { (c.start, -tangent_dir(c, true)?) } else { (c.end, tangent_dir(c, false)?) };
            if tangent_continues(shape, p, out, c.index) {
                return Ok(if spec.run_on { r_ext } else { 0.0 });
            }
            // Past a free end there's nothing to cut: run on so the tool ends outside.
            let mid_prm = (info.first_param + info.last_param) / 2.0;
            let mid = c.shape.point_at(mid_prm)?;
            let fr = frame_at(shape, ff, mid, c.shape.tangent_at(mid_prm)?.normalize(), scale)?;
            // A concave edge's section is added: running on past the part's
            // end would stick out of it.
            if fr.u[0].dot(fr.m[1]) <= 0.0 {
                return Ok(0.0);
            }
            let probe = p + out * (r_ext * 0.5) + (fr.u[0] + fr.u[1]) * (r_ext * 0.05);
            Ok(if shape.classify_point(probe, scale * 1e-9)? == k::PointClass::Outside { r_ext } else { 0.0 })
        };
        let (e0, e1) = (ext_at(true)?, ext_at(false)?);
        let (tool, names, convex) = match &info.curve {
            k::Curve::Line { .. } => {
                let d = (c.end - c.start).normalize();
                let mut fr = frame_at(shape, ff, c.start + d * ((a + b) / 2.0), d, scale)?;
                // The ball must touch both faces within them (otherwise Onshape
                // fails too), or rest on a face's far edge with overflow off.
                let sb = setbacks(&fr, spec, side1_first)?;
                for (k, (fi, setback)) in ff.iter().zip([sb.0, sb.1]).enumerate() {
                    let line_dist = |q: DVec3| (q - c.start - d * (q - c.start).dot(d)).length();
                    let width = fi.vertices().iter().filter_map(|v| v.vertex_point().ok()).map(line_dist).fold(0.0, f64::max);
                    if spec.cliff {
                        fr.extent[k] = width;
                    } else if width < setback * (1.0 - 1e-9) {
                        return Err(fail("the fillet is wider than a face"));
                    }
                }
                // Running on into a tangent neighbour: the section carries on until it
                // leaves the part (a chamfer's prism, G7), not a fixed distance.
                let reach = |end: DVec3, out: DVec3, ext: f64| -> FResult<f64> {
                    if ext <= 0.0 || !spec.run_on {
                        return Ok(ext);
                    }
                    let bb = shape.bounding_box(false)?;
                    let limit = (bb.max - bb.min).length() + ext;
                    let mut e = ext;
                    while e < limit {
                        let (f, _, _) = section_face(end + out * e, &fr, spec, side1_first)?;
                        if k::distance(&f, shape)?.distance > scale * 1e-9 && shape.classify_point(f.centroid()?, scale * 1e-9)? == k::PointClass::Outside {
                            return Ok(e);
                        }
                        e *= 1.5;
                    }
                    Ok(limit)
                };
                let (e0, e1) = (reach(c.start + d * a, -d, e0)?, reach(c.start + d * b, d, e1)?);
                let p0 = c.start + d * (a - e0);
                let (face, edges3, convex) = section_face(p0, &fr, spec, side1_first)?;
                let tool = k::extrude(&face, d * (b - a + e0 + e1))?;
                let names = tool_names(feature, &tool, &face, &edges3[1], &ename);
                (tool.shape, names, convex)
            }
            k::Curve::Circle { center, axis, radius, .. } => {
                // Angles along the chain direction.
                let span = (info.last_param - info.first_param).abs();
                let (f0, f1) = (a / c.length, b / c.length);
                let (g0, g1) = (e0 / radius, e1 / radius);
                let mut ax = axis.normalize();
                if !c.forward {
                    ax = -ax;
                }
                let p0 = rotate(c.start, *center, ax, f0 * span - g0);
                let fr = frame_at(shape, ff, rotate(c.start, *center, ax, (f0 + f1) / 2.0 * span), ax.cross(rotate(c.start, *center, ax, (f0 + f1) / 2.0 * span) - *center).normalize(), scale)?;
                // Frame at p0: rotate the mid frame back.
                let back = (f0 - (f0 + f1) / 2.0) * span - g0;
                let rot = |v: DVec3| glam::DQuat::from_axis_angle(ax, back) * v;
                let fr0 = Frame { u: [rot(fr.u[0]), rot(fr.u[1])], m: [rot(fr.m[0]), rot(fr.m[1])], extent: fr.extent };
                let (face, edges3, convex) = section_face(p0, &fr0, spec, side1_first)?;
                let mut angle = (f1 - f0) * span + g0 + g1;
                // A closed circle (live K3: a conic on a Ø20 boss's top edge) is
                // blended all round; an arc never runs on past a full turn.
                let closed = span >= std::f64::consts::TAU - 1e-9;
                if angle >= std::f64::consts::TAU - 1e-9 {
                    if !(closed && e0 == 0.0 && e1 == 0.0) {
                        return Err(fail("the blend would run all round"));
                    }
                    angle = std::f64::consts::TAU;
                }
                let tool = k::revolve(&face, *center, ax, angle)?;
                let names = tool_names(feature, &tool, &face, &edges3[1], &ename);
                (tool.shape, names, convex)
            }
            _ => return Err(fail("only lines and arcs")),
        };
        let named = NamedShape::new(tool, names);
        if convex { cuts.push(named) } else { adds.push(named) }
    }
    let mut named = body_named(&body);
    for (op, tools) in [(k::BooleanOp::Cut, cuts), (k::BooleanOp::Fuse, adds)] {
        if tools.is_empty() {
            continue;
        }
        let (s, n) = bool_named(ev, op, &named, &tools)?;
        named = NamedShape::new(s, n);
    }
    let un = k::unify_same_domain(&named.shape)?;
    let names = ev.name_result(&[&named.names], &un.history, &un.shape, &BTreeMap::new(), None);
    if !un.shape.is_valid() || un.shape.count(ShapeType::Solid) != 1 {
        return Err(fail("the result is not one valid solid"));
    }
    ev.replace_body(bi, un.shape, names);
    Ok(())
}

fn rotate(p: DVec3, center: DVec3, axis: DVec3, angle: f64) -> DVec3 {
    center + glam::DQuat::from_axis_angle(axis, angle) * (p - center)
}

/// Unit tangent of a chain edge at its chain start (or end), along the chain.
fn tangent_dir(c: &ChainEdge, at_start: bool) -> FResult<DVec3> {
    let info = c.shape.edge_info()?;
    let param = if at_start == c.forward { info.first_param } else { info.last_param };
    let t = c.shape.tangent_at(param)?.normalize();
    Ok(if c.forward { t } else { -t })
}

/// A face blend between two planar faces of one body without a common edge
/// (live psb 10-26 b, GA/GC: an L bracket with an inside chamfer, and one
/// with a groove between base and wall). Like faceBlend.fs:
/// - The side each face blends on comes from wallsNormalAlignment: at the
///   faces' closest points, a face whose normal leans toward the other face
///   is "aligned", and its normal is reversed unless aligned XOR Flip.
/// - A side whose direction disagrees with the corner (outward on a concave
///   corner, inward on a convex one) fails: "Side 1 direction is invalid.",
///   then side 2. Live words the wrong side of a thin wall differently by
///   radius ("Provided dimension is too large." for r 1–4 in GC); this
///   always names the side.
/// - The section runs along the planes' intersection line over both faces'
///   common length, touching each face where the section's centre puts it;
///   a touch point short of its face is "Provided dimension is too small.",
///   past its far end "Provided dimension is too large.".
/// - The walls are trimmed to the blend: on a concave corner the space
///   between a wall's plane and the other face (GC's groove) is filled.
pub fn apart_blend(ev: &mut EvalCtx, bi: usize, f1: usize, f2: usize, spec: &BlendSpec, flips: [bool; 2]) -> FResult<()> {
    use super::sheetmetal_util::{body_named, bool_named};
    let unsupported = || FeatureError::new("Face blend between faces that don't share an edge is supported between planar faces only");
    let body = ev.ctx.bodies[bi].clone();
    let faces = body.shape.faces();
    let (fa, fb) = (&faces[f1], &faces[f2]);
    if fa.face_plane()?.is_none() || fb.face_plane()?.is_none() {
        return Err(unsupported());
    }
    let d = k::distance(fa, fb)?;
    let (pa, pb) = (d.point_a, d.point_b);
    let scale = body.shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    if d.distance <= scale * 1e-9 {
        return Err(unsupported());
    }
    // Outward normals and the blend side of each face.
    let o = [fa.normal_at_point(pa)?.normalize(), fb.normal_at_point(pb)?.normalize()];
    let v = (pb - pa).normalize();
    let aligned = [o[0].dot(v) >= -1e-5, o[1].dot(v) >= -1e-5];
    let n: [DVec3; 2] = std::array::from_fn(|i| {
        let flip_normal = if aligned[i] { flips[i] } else { !flips[i] };
        if flip_normal { -o[i] } else { o[i] }
    });
    let line = o[0].cross(o[1]);
    if line.length() < 1e-9 {
        return Err(unsupported());
    }
    let line = line.normalize();
    // The corner: the point of both planes nearest the closest points' middle.
    let mid = (pa + pb) * 0.5;
    let (c1, c2) = (o[0].dot(pa), o[1].dot(pb));
    let c3 = line.dot(mid);
    let m = glam::DMat3::from_cols(o[0], o[1], line).transpose();
    let corner = m.inverse() * DVec3::new(c1, c2, c3);
    // In each face's plane, across the line, toward the face.
    let u: [DVec3; 2] = std::array::from_fn(|i| {
        let w = o[i].cross(line).normalize();
        if ([pa, pb][i] - corner).dot(w) >= 0.0 { w } else { -w }
    });
    // Material directions; the corner is convex when face 1 lies on face 2's
    // material side.
    let mat = [-o[0], -o[1]];
    let convex = u[0].dot(mat[1]) > 0.0;
    for i in 0..2 {
        let want = if convex { mat[i] } else { -mat[i] };
        if n[i].dot(want) < 0.0 {
            return Err(FeatureError::new(format!("Side {} direction is invalid.", i + 1)));
        }
    }
    let fr = Frame { u, m: mat, extent: [f64::INFINITY; 2] };
    let (s1, s2) = setbacks(&fr, spec, true)?;
    // Each face's reach across and along the line, from points on its edges.
    let reach = |f: &Shape, w: DVec3| -> FResult<((f64, f64), (f64, f64))> {
        let (mut a, mut l) = ((f64::INFINITY, f64::NEG_INFINITY), (f64::INFINITY, f64::NEG_INFINITY));
        for e in f.edges() {
            let info = e.edge_info()?;
            for k in 0..=16 {
                let q = e.point_at(info.first_param + (info.last_param - info.first_param) * k as f64 / 16.0)? - corner;
                a = (a.0.min(q.dot(w)), a.1.max(q.dot(w)));
                l = (l.0.min(q.dot(line)), l.1.max(q.dot(line)));
            }
        }
        Ok((a, l))
    };
    let ((a0, l0), (a1, l1)) = (reach(fa, u[0])?, reach(fb, u[1])?);
    let tol = scale * 1e-9;
    for (s, (lo, hi)) in [(s1, a0), (s2, a1)] {
        if s < lo - tol {
            return Err(FeatureError::new("Provided dimension is too small."));
        }
        if s > hi + tol {
            return Err(FeatureError::new("Provided dimension is too large."));
        }
    }
    let (from, to) = (l0.0.max(l1.0), l0.1.min(l1.1));
    if to - from <= tol {
        return Err(fail("the faces don't overlap along the blend"));
    }
    let (face, edges3, _) = section_face(corner + line * from, &fr, spec, true)?;
    let tool = k::extrude(&face, line * (to - from))?;
    let names = tool_names(ev.feature, &tool, &face, &edges3[1], &body.faces[f1]);
    let mut tools = vec![NamedShape::new(tool.shape, names)];
    // The walls are trimmed to the blend: on a concave corner what lies
    // between a wall's plane and the other face, as far as that face reaches
    // past the corner (GC's groove under the base's plane), becomes material.
    let (h1, h2) = ((-a1.0).max(0.0), (-a0.0).max(0.0));
    if !convex && (h1 > tol || h2 > tol) {
        let at = |a: f64, b: f64| corner + line * from + u[0] * a + u[1] * b;
        for (a, b) in [((-h2, s1), (-h1, 0.0)), ((-h2, 0.0), (0.0, s2))] {
            if a.1 - a.0 <= tol || b.1 - b.0 <= tol {
                continue;
            }
            let pts = [at(a.0, b.0), at(a.1, b.0), at(a.1, b.1), at(a.0, b.1)];
            let wire = k::make_wire(&(0..4).map(|k| k::make_line(pts[k], pts[(k + 1) % 4])).collect::<Result<Vec<_>, _>>()?)?;
            let back = k::extrude(&k::make_face_from_wire(&wire, true)?, line * (to - from))?;
            let n = back.shape.count(ShapeType::Face);
            tools.push(NamedShape::new(back.shape, (0..n).map(|_| Arc::new(TopoName::face(ev.feature, Role::Generated(body.faces[f1].clone())))).collect()));
        }
    }
    let op = if convex { k::BooleanOp::Cut } else { k::BooleanOp::Fuse };
    let (shape, names) = bool_named(ev, op, &body_named(&body), &tools)?;
    let named = NamedShape::new(shape, names);
    let un = k::unify_same_domain(&named.shape)?;
    let names = ev.name_result(&[&named.names], &un.history, &un.shape, &BTreeMap::new(), None);
    if !un.shape.is_valid() || un.shape.count(ShapeType::Solid) != 1 {
        return Err(fail("the result is not one valid solid"));
    }
    ev.replace_body(bi, un.shape, names);
    Ok(())
}
