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
    /// Allow edge overflow on a concave line: past a free end the round runs on and the faces it meets there,
    /// extended, trim it (live c2-hunting-knife's Fillet 2: the round crosses the grind face to the blade's side).
    pub overflow: bool,
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
    /// Tangent hold lines (live 2233): where one is nearer the edge than the radius, the section is the circle touching
    /// its face at the hold line (measured in the section plane) and the other face; elsewhere the plain round.
    pub hold: Vec<Shape>,
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
    /// Where the chain first crosses this plane (origin, normal): a datum plane or a planar face; the chain's nearest
    /// point to it if it doesn't.
    Plane(DVec3, DVec3),
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
    if std::env::var_os("NOSHAPE_BLEND_DEBUG").is_some() {
        eprintln!("section blend: {why}");
    }
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
            Bound::Point(_) | Bound::Plane(..) => return Ok(None),
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
        // (Undecided, both a step from a face's corner or both on it: the side its centroid is on. A concave corner's
        // section went into the material otherwise: c2-extension-spring's Curvature fillet.)
        let sides = match sides.as_slice() {
            [_] => sides,
            _ => match faces[i].centroid() {
                Ok(c) if (c - p).dot(dir).abs() > d => vec![(c - p).dot(dir).signum()],
                _ => vec![1.0, -1.0],
            },
        };
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
            // A chamfer's distances lie along the faces.
            None if spec.section == Section::Chamfer => (spec.r1, spec.r2),
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

/// A run-on round's pieces to fuse, and the faces to rebuild on their extensions first (index, extended face).
type RunOn = (Vec<NamedShape>, Vec<(usize, Shape)>);

/// A concave round's tool run on past its edge's free ends ([`BlendSpec::overflow`]): the pieces of it inside every
/// face its run-on pieces meet (supports aside), each face extended, named as the tool's faces they lie on; and the
/// free-form faces among those, each extended only where the run-on needs it (its index, the extended face): the body's
/// face is rebuilt on that before the fuse, so the round's piece on it merges with it (live extends the face the
/// round runs onto: c2-hunting-knife's grind face).
fn trimmed_run_on(shape: &Shape, supports: [usize; 2], (tool, names): (Shape, Vec<Name>), generated: &Name, run_on: &[Shape], len: f64, scale: f64) -> FResult<RunOn> {
    let tol = scale * 1e-7;
    let bb = shape.bounding_box(false)?;
    // The run-on's points over the body (beyond its box another face trims it).
    let mut points = Vec::new();
    for piece in run_on {
        for e in piece.edges() {
            let info = e.edge_info()?;
            for i in 0..=16 {
                let q = e.point_at(info.first_param + (info.last_param - info.first_param) * i as f64 / 16.0)?;
                if q.cmpge(bb.min - DVec3::splat(tol)).all() && q.cmple(bb.max + DVec3::splat(tol)).all() {
                    points.push(q);
                }
            }
        }
    }
    let mut walls = Vec::new();
    let mut refaced = Vec::new();
    for (i, f) in shape.faces().iter().enumerate() {
        if supports.contains(&i) || !run_on.iter().any(|p| k::distance(f, p).is_ok_and(|d| d.distance <= tol)) {
            continue;
        }
        if matches!(f.face_surface()?, k::Surface::BSpline { .. }) {
            let ext = k::extend_face_over(f, &points, 0.05 * len)?;
            refaced.push((i, ext.clone()));
            walls.push(ext);
        } else {
            walls.push(k::extend_face(f, 2.0 * len)?);
        }
    }
    if walls.is_empty() {
        return Ok((vec![NamedShape::new(tool, names)], refaced));
    }
    let pieces = k::split(std::slice::from_ref(&tool), &walls, &k::BooleanOptions::default())?.shape.solids();
    let inside = |q: DVec3| walls.iter().all(|w| w.project_point(q).is_ok_and(|pr| w.normal_at_point(q).is_ok_and(|n| (q - pr.point).dot(n) < 0.0)));
    let faces = tool.faces();
    let on = |f: &Shape| -> Name {
        let q = super::helpers_a::point_on_face(f);
        q.and_then(|q| faces.iter().zip(&names).find(|(t, _)| t.contains_point(q, scale * 1e-6).unwrap_or(false))).map_or_else(|| generated.clone(), |(_, n)| n.clone())
    };
    let mut kept = Vec::new();
    for piece in pieces {
        if !inside(piece.centroid()?) {
            continue;
        }
        let piece_names = piece.faces().iter().map(on).collect();
        kept.push(NamedShape::new(piece, piece_names));
    }
    if kept.is_empty() {
        return Err(fail("nothing of the round is inside the faces past its ends"));
    }
    Ok((kept, refaced))
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

/// The point and the chain-forward tangent at `fraction` of the chain of
/// `edges` (as a partial fillet's bounds measure it, along each edge's
/// parameter), and the chain's length: where a partial fillet's bound
/// manipulator ends.
pub fn partial_point(shape: &Shape, edges: &[usize], fraction: f64) -> Option<(DVec3, DVec3, f64)> {
    let chain = chain(shape, edges).ok()?;
    let total: f64 = chain.iter().map(|c| c.length).sum();
    let mut at = fraction.clamp(0.0, 1.0) * total;
    for (k, c) in chain.iter().enumerate() {
        if at <= c.length || k + 1 == chain.len() {
            let info = c.shape.edge_info().ok()?;
            let t = (at / c.length.max(1e-300)).clamp(0.0, 1.0);
            let prm = info.first_param + (info.last_param - info.first_param) * if c.forward { t } else { 1.0 - t };
            let d = c.shape.tangent_at(prm).ok()?.try_normalize()?;
            return Some((c.shape.point_at(prm).ok()?, if c.forward { d } else { -d }, total));
        }
        at -= c.length;
    }
    None
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
            Bound::Plane(o, n) => {
                // (The first sign change of the distance to the plane, else the nearest approach.)
                let (mut best, mut acc, mut where_, mut prev): (f64, f64, f64, Option<(f64, f64)>) = (f64::INFINITY, 0.0, 0.0, None);
                let mut crossing = None;
                'chain: for c in &chain {
                    let info = c.shape.edge_info()?;
                    for i in 0..=64 {
                        let t = i as f64 / 64.0;
                        let prm = info.first_param + (info.last_param - info.first_param) * if c.forward { t } else { 1.0 - t };
                        let d = (c.shape.point_at(prm)? - o).dot(n);
                        let at = acc + t * c.length;
                        if let Some((pd, pat)) = prev
                            && pd != 0.0
                            && (pd < 0.0) != (d < 0.0)
                        {
                            crossing = Some(pat + (at - pat) * pd / (pd - d));
                            break 'chain;
                        }
                        if d.abs() < best {
                            best = d.abs();
                            where_ = at;
                        }
                        prev = Some((d, at));
                    }
                    acc += c.length;
                }
                crossing.unwrap_or(where_)
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
    // Stations along the chain: the point, its frame, whether its first face is side 1, the chain's tangent.
    let station = |s: f64| -> FResult<(DVec3, Frame, bool, DVec3)> {
        let mut acc = 0.0;
        let last = chain.len() - 1;
        for (i, c) in chain.iter().enumerate() {
            if s <= acc + c.length || i == last {
                let info = c.shape.edge_info()?;
                let f = ((s - acc) / c.length).clamp(0.0, 1.0);
                let t = info.first_param + (info.last_param - info.first_param) * if c.forward { f } else { 1.0 - f };
                let p = c.shape.point_at(t)?;
                let fs = ef.get(c.index).cloned().unwrap_or_default();
                if fs.len() != 2 {
                    return Err(fail("the edge doesn't have two faces"));
                }
                let ff = [&faces[fs[0] as usize], &faces[fs[1] as usize]];
                let side1_first = spec.sides.as_ref().is_none_or(|sd| sd.side1.contains(&(fs[0] as usize)));
                let tangent = c.shape.tangent_at(t)?.normalize();
                let fr = match frame_at(shape, ff, p, tangent, scale) {
                    Ok(fr) => fr,
                    // At a free end (a part's corner) the probes fall on the end face: orient from just inside.
                    Err(_) => {
                        let fi = info.first_param + (info.last_param - info.first_param) * if c.forward { f.clamp(1e-3, 1.0 - 1e-3) } else { 1.0 - f.clamp(1e-3, 1.0 - 1e-3) };
                        let inner = frame_at(shape, ff, c.shape.point_at(fi)?, c.shape.tangent_at(fi)?.normalize(), scale)?;
                        let mut fr = Frame { u: inner.u, m: inner.m, extent: inner.extent };
                        for (i, face) in ff.iter().enumerate() {
                            let n = face.normal_at_point(p)?.normalize();
                            let dir = n.cross(tangent).normalize();
                            fr.u[i] = if dir.dot(inner.u[i]) < 0.0 { -dir } else { dir };
                            fr.m[i] = if n.dot(inner.m[i]) < 0.0 { -n } else { n };
                        }
                        fr
                    }
                };
                return Ok((p, fr, side1_first, tangent));
            }
            acc += c.length;
        }
        Err(fail("empty chain"))
    };
    let ranges = hold_ranges(spec, lo, hi, scale, &station)?;
    let partial = spec.partial.is_some() || ranges.len() > 1;
    // Other curves (live NE: an elliptic boss's foot): sections along the
    // whole chain, lofted into one tool (live's blend face is one spline).
    let general = chain.iter().any(|c| c.shape.edge_info().is_ok_and(|i| !matches!(i.curve, k::Curve::Line { .. } | k::Curve::Circle { .. })));
    let feature = ev.feature;
    let (mut cuts, mut adds) = (Vec::new(), Vec::new());
    let mut refaced: Vec<(usize, Shape)> = Vec::new();
    for &(lo, hi, held) in &ranges {
    if general || held {
        const N: usize = 64;
        let closed = (chain[0].start - chain[chain.len() - 1].end).length() < 1e-9 && lo <= 1e-12 && hi >= total - 1e-12;
        let mut secs = Vec::new();
        let mut starts = Vec::new();
        let mut convex = false;
        let count = if closed { N } else { N + 1 };
        for k in 0..count {
            // A closed loft starts a quarter of the way round: its own seam
            // on the edge's (the periodic faces' seam) made OCCT's boolean
            // with an elliptic boss's whole side come back empty.
            let f = if closed { (k as f64 / N as f64 + 0.25).fract() } else { k as f64 / N as f64 };
            let (p, fr, side1_first, tangent) = station(lo + (hi - lo) * f)?;
            let here = match held {
                true => held_spec(spec, p, tangent, &fr, scale)?,
                false => spec.clone(),
            };
            let (face, _, cv) = section_face(p, &fr, &here, side1_first)?;
            convex = cv;
            secs.push(face);
            starts.push(p);
        }
        // Every section starts at its edge point (its corner on the edge):
        // a curvature section's B-spline wires twisted otherwise.
        let tool = k::loft(&secs, &k::LoftOptions { solid: true, closed, start_points: Some(starts), ..Default::default() })?;
        if std::env::var_os("NOSHAPE_BLEND_DEBUG").is_some() {
            eprintln!("section blend: loft of {} sections (closed {closed}, chain {} edges): valid {} volume {:?} faces {}", secs.len(), chain.len(), tool.shape.is_valid(), tool.shape.volume(), tool.shape.count(ShapeType::Face));
        }
        // The face touching neither support is the blend face.
        let ename = body.edges[chain[0].index].clone();
        let blend = Arc::new(TopoName::face(feature, Role::Blend(ename.clone())));
        let generated = Arc::new(TopoName::face(feature, Role::Generated(ename)));
        let supports: Vec<&Shape> = chain.iter().flat_map(|c| ef.get(c.index).cloned().unwrap_or_default()).map(|f| &faces[f as usize]).collect();
        let on = |f: &Shape, s: &Shape| -> bool {
            let Some(q) = super::helpers_a::point_on_face(f) else { return false };
            s.contains_point(q, scale * 1e-6).unwrap_or(false)
        };
        let names: Vec<Name> = tool.shape.faces().iter().map(|f| if (f.is_planar() && secs.iter().any(|s| on(f, s))) || supports.iter().any(|s| on(f, s)) { generated.clone() } else { blend.clone() }).collect();
        let named = NamedShape::new(tool.shape, names);
        if convex { cuts.push(named) } else { adds.push(named) }
    }
    let mut at = 0.0;
    for (ci, c) in chain.iter().enumerate().filter(|_| !general && !held) {
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
            // end would stick out of it (with overflow, what does is trimmed).
            if fr.u[0].dot(fr.m[1]) <= 0.0 {
                return Ok(if spec.overflow && matches!(info.curve, k::Curve::Line { .. }) { r_ext } else { 0.0 });
            }
            let probe = p + out * (r_ext * 0.5) + (fr.u[0] + fr.u[1]) * (r_ext * 0.05);
            Ok(if shape.classify_point(probe, scale * 1e-9)? == k::PointClass::Outside { r_ext } else { 0.0 })
        };
        let (e0, e1) = (ext_at(true)?, ext_at(false)?);
        let (tool, names, convex) = match &info.curve {
            k::Curve::Line { .. } => {
                let d = (c.end - c.start).normalize();
                let mut fr = frame_at(shape, ff, c.start + d * ((a + b) / 2.0), d, scale)?;
                // With overflow off the ball rests on a face's far edge; on, a section wider than a face runs on past it
                // and the cut trims it where it meets the faces beyond (live c2-extension-spring's Fillet 2: a 1.5 mm
                // Curvature round across a 1 mm arm's end, two meeting in the middle of a 3 mm end).
                if spec.cliff {
                    for (k, fi) in ff.iter().enumerate() {
                        let line_dist = |q: DVec3| (q - c.start - d * (q - c.start).dot(d)).length();
                        fr.extent[k] = fi.vertices().iter().filter_map(|v| v.vertex_point().ok()).map(line_dist).fold(0.0, f64::max);
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
                // (Overflow's run-on is a concave round's; a convex one past its faces fails, as live's.)
                if spec.overflow && convex {
                    return Err(fail("a convex edge's round with overflow"));
                }
                let tool = k::extrude(&face, d * (b - a + e0 + e1))?;
                let names = tool_names(feature, &tool, &face, &edges3[1], &ename);
                if spec.overflow && !convex && e0 + e1 > 0.0 {
                    let run_on = [(c.start + d * (a - e0), e0), (c.start + d * b, e1)]
                        .into_iter()
                        .filter(|(_, e)| *e > 0.0)
                        .map(|(q, e)| k::extrude(&section_face(q, &fr, spec, side1_first)?.0, d * e).map(|t| t.shape).map_err(FeatureError::from))
                        .collect::<FResult<Vec<_>>>()?;
                    let generated = Arc::new(TopoName::face(feature, Role::Generated(ename.clone())));
                    let (kept, faces_on) = trimmed_run_on(shape, [fs[0] as usize, fs[1] as usize], (tool.shape, names), &generated, &run_on, r_ext, scale)?;
                    adds.extend(kept);
                    refaced.extend(faces_on);
                    continue;
                }
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
    }
    let mut named = body_named(&body);
    for (i, on) in &refaced {
        named.shape = k::reface_on(&named.shape, *i, on)?;
    }
    for (op, tools) in [(k::BooleanOp::Cut, cuts), (k::BooleanOp::Fuse, adds)] {
        if tools.is_empty() {
            continue;
        }
        let (s, n) = bool_named(ev, op, &named, &tools)?;
        named = NamedShape::new(s, n);
    }
    // A hold line is the user's split of its face: it stays (live 2233: the top's strip beyond an r 6 round).
    let held = spec.sides.as_ref().is_some_and(|s| !s.hold.is_empty());
    let un = if held { k::unify_edges(&named.shape)? } else { k::unify_same_domain(&named.shape)? };
    if std::env::var_os("NOSHAPE_BLEND_DEBUG").is_some() {
        eprintln!("section blend: boolean valid {} solids {}, unified valid {} solids {}", named.shape.is_valid(), named.shape.count(ShapeType::Solid), un.shape.is_valid(), un.shape.count(ShapeType::Solid));
    }
    let mut names = ev.name_result(&[&named.names], &un.history, &un.shape, &BTreeMap::new(), None);
    let mut shape = un.shape;
    // A face extended to meet the round: its old edge and the new piece running on from it one edge, as live's.
    if !refaced.is_empty() {
        let corners: Vec<DVec3> = refaced.iter().flat_map(|(i, _)| faces[*i].vertices()).filter_map(|v| v.vertex_point().ok()).collect();
        let joined = k::join_edges_at(&shape, &corners, scale * 1e-7)?;
        names = ev.name_result(&[&crate::regen::eval::names_for(&shape, names)], &joined.history, &joined.shape, &BTreeMap::new(), None);
        shape = joined.shape;
    }
    if !shape.is_valid() || shape.count(ShapeType::Solid) != 1 {
        return Err(fail("the result is not one valid solid"));
    }
    ev.replace_body(bi, shape, names);
    Ok(())
}

/// The radius a hold line sets at a station: the circle touching the face the hold line is on at its crossing of the
/// section plane (through `p` across `tangent`) and the other face. None where no hold line crosses the section.
fn hold_radius(hold: &[Shape], p: DVec3, tangent: DVec3, f: &Frame, scale: f64) -> FResult<Option<f64>> {
    let mut best: Option<f64> = None;
    for e in hold {
        let info = e.edge_info()?;
        let (t0, t1) = (info.first_param, info.last_param);
        let g = |t: f64| -> FResult<f64> { Ok((e.point_at(t)? - p).dot(tangent)) };
        const N: usize = 32;
        let mut prev = (t0, g(t0)?);
        for i in 1..=N {
            let t = t0 + (t1 - t0) * i as f64 / N as f64;
            let cur = (t, g(t)?);
            if (prev.1 <= 0.0) != (cur.1 <= 0.0) || cur.1 == 0.0 {
                let (mut a, mut b) = (prev, cur);
                for _ in 0..60 {
                    let m = 0.5 * (a.0 + b.0);
                    let gm = g(m)?;
                    if (gm <= 0.0) == (a.1 <= 0.0) {
                        a = (m, gm);
                    } else {
                        b = (m, gm);
                    }
                }
                let v = e.point_at(0.5 * (a.0 + b.0))? - p;
                for u in f.u {
                    let s = v.dot(u);
                    if s > 0.0 && (v - u * s).length() < scale * 1e-6 && best.is_none_or(|b| s < b) {
                        best = Some(s);
                    }
                }
            }
            prev = cur;
        }
    }
    // Equal offsets d from both sides touch each at (d + d·cos θ) / sin θ from the edge (see [`Sides`]).
    let c = f.u[0].dot(f.u[1]).clamp(-1.0, 1.0);
    Ok(best.map(|s| s * (1.0 - c * c).sqrt() / (1.0 + c)))
}

/// A held station's spec: the radius the hold line sets, capped by the entered one.
fn held_spec(spec: &BlendSpec, p: DVec3, tangent: DVec3, f: &Frame, scale: f64) -> FResult<BlendSpec> {
    let hold = spec.sides.as_ref().map(|s| s.hold.as_slice()).unwrap_or(&[]);
    let r = hold_radius(hold, p, tangent, f, scale)?.map_or(spec.r1, |h| h.min(spec.r1));
    Ok(BlendSpec { r1: r, r2: r, ..spec.clone() })
}

type Station<'a> = dyn Fn(f64) -> FResult<(DVec3, Frame, bool, DVec3)> + 'a;

/// The chain's range [lo, hi] in pieces, each held (a hold line nearer the edge than the radius: its section follows
/// the hold line) or not (the plain round). Live 2233: the entered radius caps the hold line's.
fn hold_ranges(spec: &BlendSpec, lo: f64, hi: f64, scale: f64, station: &Station) -> FResult<Vec<(f64, f64, bool)>> {
    let hold = spec.sides.as_ref().map(|s| s.hold.as_slice()).unwrap_or(&[]);
    if hold.is_empty() {
        return Ok(vec![(lo, hi, false)]);
    }
    if spec.section != Section::Circular || spec.width.is_some() || (spec.r1 - spec.r2).abs() > 1e-12 * spec.r1 {
        return Err(FeatureError::new("Tangent hold lines are supported with a symmetric circular radius only"));
    }
    let held = |s: f64| -> FResult<bool> {
        let (p, fr, _, tangent) = station(s)?;
        Ok(hold_radius(hold, p, tangent, &fr, scale)?.is_some_and(|r| r < spec.r1 * (1.0 - 1e-9)))
    };
    const N: usize = 64;
    let mut ranges: Vec<(f64, f64, bool)> = Vec::new();
    let mut start = lo;
    let mut prev = (lo, held(lo)?);
    for i in 1..=N {
        let s = lo + (hi - lo) * i as f64 / N as f64;
        let cur = (s, held(s)?);
        if cur.1 != prev.1 {
            // Where the hold line's distance passes the radius.
            let (mut a, mut b) = (prev.0, cur.0);
            for _ in 0..40 {
                let m = 0.5 * (a + b);
                if held(m)? == prev.1 {
                    a = m;
                } else {
                    b = m;
                }
            }
            let at = 0.5 * (a + b);
            ranges.push((start, at, prev.1));
            start = at;
        }
        prev = cur;
    }
    ranges.push((start, hi, prev.1));
    ranges.retain(|r| r.1 - r.0 > 1e-9 * (hi - lo));
    Ok(ranges)
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

/// The geometry of a face blend between two faces without a common edge,
/// from their tangent planes at their closest points, after live's checks
/// of the sides and touch points (see [`apart_blend`]).
struct Apart {
    corner: DVec3,
    line: DVec3,
    u: [DVec3; 2],
    fr: Frame,
    convex: bool,
    s: [f64; 2],
    reach: [(f64, f64); 2],
    along: [(f64, f64); 2],
    scale: f64,
}

fn sides_geometry(shape: &Shape, f1: usize, f2: usize, spec: &BlendSpec, flips: [bool; 2]) -> FResult<Apart> {
    let unsupported = || fail("faces whose closest points are tangent or touch");
    let faces = shape.faces();
    let (fa, fb) = (&faces[f1], &faces[f2]);
    let curved = !fa.is_planar() || !fb.is_planar();
    let d = k::distance(fa, fb)?;
    let (pa, pb) = (d.point_a, d.point_b);
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
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
    // A side is wrong when its blend side disagrees with the corner.
    let wrong: [bool; 2] = std::array::from_fn(|i| n[i].dot(if convex { mat[i] } else { -mat[i] }) < 0.0);
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
    // Where the blend with the right sides would touch each face.
    #[derive(PartialEq)]
    enum Touch {
        Short,
        On,
        Past,
    }
    let touch = |s: f64, (lo, hi): (f64, f64)| if s < lo - tol { Touch::Short } else if s > hi + tol { Touch::Past } else { Touch::On };
    let t = [touch(s1, a0), touch(s2, a1)];
    let too_small = || FeatureError::new("Provided dimension is too small.");
    let too_large = || FeatureError::new("Provided dimension is too large.");
    match (wrong[0], wrong[1]) {
        (false, false) => {
            for ti in &t {
                match ti {
                    Touch::Short => return Err(too_small()),
                    Touch::Past => return Err(too_large()),
                    Touch::On => {}
                }
            }
        }
        (true, true) => {
            if curved || t.iter().any(|x| *x != Touch::On) {
                return Err(too_large());
            }
            return Err(FeatureError::new("Both side 1 and side 2 directions are invalid."));
        }
        _ => {
            let w = if wrong[0] { 0 } else { 1 };
            // On curved faces live also fails at the face's edge (NP2 r1).
            let lo = [a0.0, a1.0][w];
            if t[w] == Touch::Short || (curved && [s1, s2][w] <= lo + scale * 1e-7) {
                return Err(fail("the wrong side's touch point is short of its face"));
            }
            if curved {
                return Err(too_large());
            }
            if t.iter().any(|x| *x != Touch::On) {
                return Err(too_large());
            }
            return Err(FeatureError::new(format!("Side {} direction is invalid.", w + 1)));
        }
    }
    Ok(Apart { corner, line, u, fr, convex, s: [s1, s2], reach: [a0, a1], along: [l0, l1], scale })
}

/// Live's checks of the sides and touch points for curved faces without a
/// common edge, on their tangent planes at the closest points.
pub fn apart_senses(shape: &Shape, f1: usize, f2: usize, spec: &BlendSpec, flips: [bool; 2]) -> FResult<()> {
    sides_geometry(shape, f1, f2, spec, flips).map(|_| ())
}

/// A face blend between two planar faces of one body without a common edge
/// (live psb 10-26 b, GA/GC: an L bracket with an inside chamfer, and one
/// with a groove between base and wall). Like faceBlend.fs:
/// - The side each face blends on comes from wallsNormalAlignment: at the
///   faces' closest points, a face whose normal leans toward the other face
///   is "aligned", and its normal is reversed unless aligned XOR Flip.
/// - A side whose direction disagrees with the corner (outward on a concave
///   corner, inward on a convex one) is wrong. Live's messages (GA–GD, 22
///   cases), from where the blend with the right sides would touch:
///   - one side wrong: its touch point short of its face "Failed to blend
///     faces.", any other touch point off its face "Provided dimension is
///     too large.", else "Side N direction is invalid.";
///   - both wrong: a touch point off its face "Provided dimension is too
///     large.", else "Both side 1 and side 2 directions are invalid.";
///   - with a curved side (NP2, a cylinder; 7 cases) the direction messages
///     become "Provided dimension is too large.", and a wrong side's touch
///     point at its face's edge already fails.
/// - The section runs along the planes' intersection line over both faces'
///   common length, touching each face where the section's centre puts it;
///   with the right sides a touch point short of its face is "Provided
///   dimension is too small.", past its far end "Provided dimension is too
///   large.".
/// - The walls are trimmed to the blend: on a concave corner the space
///   between a wall's plane and the other face (GC's groove) is filled.
pub fn apart_blend(ev: &mut EvalCtx, bi: usize, f1: usize, f2: usize, spec: &BlendSpec, flips: [bool; 2]) -> FResult<()> {
    use super::sheetmetal_util::{body_named, bool_named};
    let body = ev.ctx.bodies[bi].clone();
    let Apart { corner, line, u, fr, convex, s: [s1, s2], reach: [a0, a1], along: [l0, l1], scale } = sides_geometry(&body.shape, f1, f2, spec, flips)?;
    let tol = scale * 1e-9;
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

/// The faces between two faces without a common edge, for a face blend
/// that consumes them (live NP2: a chamfer between a plate and a boss; GC:
/// a groove): in the plane through the faces' closest points across the
/// gap, the shortest run of the body's section from one face to the other.
/// `None` when the section doesn't join them.
pub fn gap_faces(shape: &Shape, f1: usize, f2: usize) -> FResult<Option<Vec<usize>>> {
    let faces = shape.faces();
    let d = k::distance(&faces[f1], &faces[f2])?;
    let (pa, pb) = (d.point_a, d.point_b);
    let gap = pb - pa;
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let tol = scale * 1e-6;
    if gap.length() <= tol {
        return Ok(None);
    }
    let mut np = gap.cross(faces[f1].normal_at_point(pa)?);
    if np.length() < 1e-9 * gap.length() {
        np = gap.cross(faces[f2].normal_at_point(pb)?);
    }
    if np.length() < 1e-9 * gap.length() {
        return Ok(None);
    }
    let plane = k::Plane::from_normal(pa, np.normalize());
    // Section segments of the other faces: (face, from, to, length).
    let mut segs: Vec<(usize, DVec3, DVec3, f64)> = Vec::new();
    for (i, f) in faces.iter().enumerate() {
        if i == f1 || i == f2 {
            continue;
        }
        let Ok(sec) = k::section_plane(f, &plane) else { continue };
        for e in sec.edges() {
            let Ok((a, b)) = e.endpoints() else { continue };
            segs.push((i, a, b, e.length().unwrap_or((b - a).length())));
        }
    }
    // Dijkstra over the segments' ends from pa to pb.
    let mut nodes: Vec<DVec3> = vec![pa, pb];
    let mut node = |p: DVec3| -> usize {
        if let Some(i) = nodes.iter().position(|q| (*q - p).length() < tol) {
            return i;
        }
        nodes.push(p);
        nodes.len() - 1
    };
    let links: Vec<(usize, usize, usize, f64)> = segs.iter().map(|&(f, a, b, l)| (node(a), node(b), f, l)).collect();
    let n = nodes.len();
    let mut dist = vec![f64::INFINITY; n];
    let mut prev: Vec<Option<(usize, usize)>> = vec![None; n];
    let mut done = vec![false; n];
    dist[0] = 0.0;
    while let Some(u) = (0..n).filter(|i| !done[*i] && dist[*i].is_finite()).min_by(|x, y| dist[*x].total_cmp(&dist[*y])) {
        done[u] = true;
        if u == 1 {
            break;
        }
        for (li, &(a, b, _, l)) in links.iter().enumerate() {
            let v = if a == u { b } else if b == u { a } else { continue };
            if dist[u] + l < dist[v] {
                dist[v] = dist[u] + l;
                prev[v] = Some((u, li));
            }
        }
    }
    if !dist[1].is_finite() {
        return Ok(None);
    }
    let mut out = Vec::new();
    let mut at = 1;
    while let Some((u, li)) = prev[at] {
        let f = links[li].2;
        if !out.contains(&f) {
            out.push(f);
        }
        at = u;
    }
    Ok(Some(out))
}

/// A closed edge where a plane meets a cylinder (a hole's or a boss's rim on a slanted face).
struct Rim {
    plane: Shape,
    wall: Shape,
    origin: DVec3,
    axis: DVec3,
    x_dir: DVec3,
    rc: f64,
    /// A point of the rim, the plane's outward normal.
    p: DVec3,
    np: DVec3,
    /// Into the material from the plane.
    wp: DVec3,
    /// +1 when the material is outside the cylinder (a hole), -1 inside (a boss).
    out: f64,
    /// The angles about the axis (from [`Rim::frame`]'s x) the rim's edges span when they are an arc of the plane and
    /// cylinder's whole intersection, the rest of it outside the body (a notch's edge), as (start, sweep); empty when
    /// they are all of it.
    arc: Vec<(f64, f64)>,
}

impl Rim {
    /// The rim's point at angle `a` about the axis from `x` (`y` = axis × `x`).
    fn at(&self, x: DVec3, y: DVec3, a: f64) -> (DVec3, DVec3) {
        let radial = x * a.cos() + y * a.sin();
        let on_cyl = self.origin + radial * self.rc;
        (on_cyl + self.axis * ((self.p - on_cyl).dot(self.np) / self.axis.dot(self.np)), radial)
    }

    /// The point of the cylinder nearest `q`.
    fn onto_wall(&self, q: DVec3) -> DVec3 {
        let along = self.axis * (q - self.origin).dot(self.axis);
        self.origin + along + (q - self.origin - along).normalize() * self.rc
    }

    fn frame(&self) -> (DVec3, DVec3) {
        let x = (self.x_dir - self.axis * self.x_dir.dot(self.axis)).normalize();
        (x, self.axis.cross(x))
    }

    /// The angle about the axis of `q`.
    fn angle_of(&self, q: DVec3) -> f64 {
        let (x, y) = self.frame();
        (q - self.origin).dot(y).atan2((q - self.origin).dot(x))
    }

    /// Whether the rim's point at angle `a` is on the body's edges.
    fn on_arc(&self, a: f64) -> bool {
        const TAU: f64 = std::f64::consts::TAU;
        self.arc.is_empty() || self.arc.iter().any(|&(start, sweep)| (a - start).rem_euclid(TAU) <= sweep + 1e-9)
    }
}

/// A rim's blend: a round of a radius, or a chamfer of equal distances.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RimBlend {
    Round,
    Chamfer,
}

/// The rims a blend's edges make ([`rims_of`]), found once for [`rims_overflow`] and [`ring_blend`].
pub struct Rims(Vec<(Rim, usize)>);

/// These edges' rims, if they all are (see [`rims_of`]).
pub fn rims(shape: &Shape, edges: &[usize]) -> Option<Rims> {
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    rims_of(shape, edges, scale).ok().map(Rims)
}

/// The rims these edges make: each group of edges between one plane and one cylinder, all the edges between them
/// (pieces of a rim a seam split), as a rim and the first of its edges (an index in `edges`).
fn rims_of(shape: &Shape, edges: &[usize], scale: f64) -> FResult<Vec<(Rim, usize)>> {
    let faces = shape.faces();
    let ef = shape.edge_faces()?;
    let mut groups: Vec<([u32; 2], Vec<usize>)> = Vec::new();
    for (i, &e) in edges.iter().enumerate() {
        let fs = ef.get(e).filter(|f| f.len() == 2).ok_or_else(|| fail("the edge doesn't have two faces"))?;
        let pair = match (faces[fs[0] as usize].face_surface()?, faces[fs[1] as usize].face_surface()?) {
            (k::Surface::Plane(_), k::Surface::Cylinder { .. }) => [fs[0], fs[1]],
            (k::Surface::Cylinder { .. }, k::Surface::Plane(_)) => [fs[1], fs[0]],
            _ => return Err(fail("not a plane and a cylinder")),
        };
        match groups.iter_mut().find(|g| g.0 == pair) {
            Some(g) => g.1.push(i),
            None => groups.push((pair, vec![i])),
        }
    }
    groups
        .into_iter()
        .map(|(pair, members)| {
            let between = ef.iter().filter(|f| f.len() == 2 && (f[0] == pair[0] && f[1] == pair[1] || f[0] == pair[1] && f[1] == pair[0])).count();
            // All of them, round a closed loop (each vertex ending two of its pieces, or one closed piece twice).
            let mut ends: Vec<Shape> = Vec::new();
            for &m in &members {
                let vs = shape.sub_shape(ShapeType::Edge, edges[m])?.vertices();
                if vs.len() == 1 {
                    ends.push(vs[0].clone());
                }
                ends.extend(vs);
            }
            let closed = ends.iter().all(|v| ends.iter().filter(|w| w.is_same(v)).count() % 2 == 0);
            let mut rim = rim_of(shape, [&faces[pair[0] as usize], &faces[pair[1] as usize]], edges[members[0]], scale)?;
            if between == members.len() && closed {
                return Ok((rim, members[0]));
            }
            // An arc: the rest of the whole intersection must be off the body.
            const TAU: f64 = std::f64::consts::TAU;
            for &m in &members {
                let e = shape.sub_shape(ShapeType::Edge, edges[m])?;
                let info = e.edge_info()?;
                let [a0, a1, am] = [info.start, info.end, e.point_at(0.5 * (info.first_param + info.last_param))?].map(|q| rim.angle_of(q));
                let sweep = (a1 - a0).rem_euclid(TAU);
                rim.arc.push(if (am - a0).rem_euclid(TAU) < sweep { (a0, sweep) } else { (a1, TAU - sweep) });
            }
            let (x, y) = rim.frame();
            // (Off the part's box is off the part, without a classification: each takes ~0.7 ms on rounds' faces.)
            let bb = shape.bounding_box(false)?;
            let pad = DVec3::splat(scale * 1e-6);
            const N: usize = 32;
            for k in 0..N {
                let a = k as f64 * TAU / N as f64;
                if rim.on_arc(a) {
                    continue;
                }
                let q = rim.at(x, y, a).0;
                if q.cmpge(bb.min - pad).all() && q.cmple(bb.max + pad).all() && shape.classify_point(q, scale * 1e-9)? != k::PointClass::Outside {
                    return Err(fail("not a whole rim"));
                }
            }
            Ok((rim, members[0]))
        })
        .collect()
}

fn rim_of(shape: &Shape, [plane, wall]: [&Shape; 2], e: usize, scale: f64) -> FResult<Rim> {
    let edge = shape.sub_shape(ShapeType::Edge, e)?;
    let info = edge.edge_info()?;
    let k::Surface::Cylinder { origin, axis, x_dir, radius: rc } = wall.face_surface()? else { return Err(fail("not a cylinder")) };
    let axis = axis.normalize();
    // (Its middle: an arc's ends are corners.)
    let mid = 0.5 * (info.first_param + info.last_param);
    let p = edge.point_at(mid)?;
    let np = plane.normal_at_point(p)?.normalize();
    if axis.dot(np).abs() < 0.05 {
        return Err(fail("a cylinder along its plane"));
    }
    let fr = frame_at(shape, [plane, wall], p, edge.tangent_at(mid)?.normalize(), scale)?;
    // (A concave rim's round would be added the same way, from the air's wedge: not checked against live yet.)
    if fr.u[0].dot(fr.m[1]) <= 0.0 {
        return Err(fail("a concave rim"));
    }
    let radial = (p - origin - axis * (p - origin).dot(axis)).normalize();
    Ok(Rim { plane: plane.clone(), wall: wall.clone(), origin, axis, x_dir, rc, p, np, wp: fr.m[0], out: fr.m[1].dot(radial).signum(), arc: Vec::new() })
}

/// Whether blends of these rims (closed edges where a plane meets a cylinder, or arcs of that whose rest is off the
/// body) run off their plane or into each other somewhere: there live trims each blend by what it meets (OCCT's rolls
/// on, or fails), [`ring_blend`]'s way. The blend's touch points at 32 angles round each rim (on its arc) must lie on
/// the plane, and on a cylinder another rim shares, with room for that rim's blend too.
pub fn rims_overflow(rims: &Rims, shape: &Shape, sizes: &[f64], kind: RimBlend) -> bool {
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let rims = &rims.0;
    let size = |i: usize| sizes.get(i).or(sizes.first()).copied().unwrap_or(0.0);
    let r_max = rims.iter().map(|(_, i)| size(*i)).fold(0.0, f64::max);
    const N: usize = 32;
    for (i, (rim, first)) in rims.iter().enumerate() {
        let r = size(*first);
        let shared = rims.iter().enumerate().any(|(j, (o, _))| j != i && o.wall.is_same(&rim.wall));
        // (A hair short of the touch line: a blend that just consumes a face, its touch line on the face's far edge,
        // stays OCCT's, as live's.)
        let reach = (1.0 - 1e-6) * (1.0 + r_max / r);
        let (x, y) = rim.frame();
        for k in 0..N {
            let a = k as f64 * std::f64::consts::TAU / N as f64;
            let (p, radial) = rim.at(x, y, a);
            if !rim.on_arc(a) {
                continue;
            }
            // Outward normals there.
            let n_w = radial * -rim.out;
            let (tp, tw) = match kind {
                // The ball's centre r inside both.
                RimBlend::Round => {
                    let c = p + (rim.np + n_w) * (-r / (1.0 + rim.np.dot(n_w)));
                    (c + rim.np * r, c + n_w * r)
                }
                // r along each face, square to the rim.
                RimBlend::Chamfer => {
                    let t = (rim.at(x, y, a + 1e-4).0 - rim.at(x, y, a - 1e-4).0).normalize();
                    let along = |n: DVec3, into: DVec3| {
                        let u = t.cross(n).normalize();
                        if u.dot(into) < 0.0 { -u } else { u }
                    };
                    (p + along(rim.np, radial * rim.out) * r, p + along(n_w, rim.wp) * r)
                }
            };
            let on_plane = rim.plane.contains_point(p + (tp - p) * (1.0 - 1e-6), scale * 1e-9).unwrap_or(false);
            // (A lone rim's round meeting something cut in its wall is OCCT's, as live's: motor-mount-jack's Fillet 1.)
            let on_wall = !shared || rim.wall.contains_point(rim.onto_wall(p + (tw - p) * reach), scale * 1e-9).unwrap_or(false);
            if !on_plane || !on_wall {
                return true;
            }
        }
    }
    false
}

/// Fillets and chamfers of convex closed edges where a plane meets a cylinder (or arcs of that whose rest is off the
/// body) whose blends run off the plane or into each other ([`rims_overflow`]), or where OCCT's chain fails: each rim
/// blended on its own on a wedge of its plane and cylinder (OCCT's blend of a lone rim, the whole intersection), the
/// region between the rim and its blend cut away, so each blend ends where it meets another face or blend, as live's
/// (c2-rings-loft's Fillet 1: a ring's two R2 rims meet at its narrow bottom and run out onto its outside; its Chamfer
/// 1: a notch's arc chamfered through the strip of plane beside it, the chamfer running on past the arc's ends until
/// it leaves the part).
pub fn ring_blend(ev: &mut EvalCtx, bi: usize, rims: &Rims, edges: &[usize], sizes: &[f64], kind: RimBlend) -> FResult<()> {
    use super::sheetmetal_util::{body_named, bool_named};
    let body = ev.ctx.bodies[bi].clone();
    let shape = &body.shape;
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let feature = ev.feature;
    let mut cuts = Vec::new();
    for (rim, i) in &rims.0 {
        let i = *i;
        let r = sizes.get(i).or(sizes.first()).copied().ok_or_else(|| fail("no size"))?;
        let e = edges[i];
        let (axis, np, p) = (rim.axis, rim.np, rim.p);
        let cos = axis.dot(np);
        // The wedge: a slab on the plane around the rim, with the cylinder's inside or outside.
        let q = rim.origin + axis * ((p - rim.origin).dot(np) / cos);
        let half = rim.rc / cos.abs() + 4.0 * r;
        let depth = 4.0 * r + rim.rc;
        let down = if rim.wp.dot(np) < 0.0 { -np } else { np };
        let x = (axis - np * cos).try_normalize().unwrap_or_else(|| np.any_orthonormal_vector());
        let y = down.cross(x);
        let slab = k::make_box(&k::Plane::new(q - x * half - y * half, down, x), 2.0 * half, 2.0 * half, depth)?;
        let long = 2.0 * (half + depth);
        let cyl = k::make_cylinder(&k::Plane::new(rim.origin + axis * ((q - rim.origin).dot(axis) - long), axis, seam_where_whole(rim, r)?), rim.rc, 2.0 * long, std::f64::consts::TAU)?;
        let wedge = if rim.out < 0.0 { k::common(&slab, &cyl)? } else { k::cut(&slab, &cyl)? }.shape;
        let at = k::make_vertex(p)?;
        let edge = wedge
            .edges()
            .into_iter()
            .filter(|c| c.edge_info().is_ok_and(|i| i.closed))
            .map(|c| (k::distance(&c, &at).map_or(f64::INFINITY, |d| d.distance), c))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .filter(|(d, _)| *d < scale * 1e-6)
            .ok_or_else(|| fail("no rim on the wedge"))?
            .1;
        let rounded = match kind {
            RimBlend::Round => k::fillet(&wedge, std::slice::from_ref(&edge), r)?,
            RimBlend::Chamfer => k::chamfer(&wedge, std::slice::from_ref(&edge), k::ChamferSpec::Equal(r), &[])?,
        };
        let tool = k::cut(&wedge, &rounded.shape)?;
        if !tool.shape.is_valid() || tool.shape.count(ShapeType::Solid) != 1 {
            return Err(fail("the rim's blend left no one solid to cut"));
        }
        // Its blend: the face on neither the plane nor the cylinder.
        let ename = body.edges[e].clone();
        let blend = Arc::new(TopoName::face(feature, Role::Blend(ename.clone())));
        let generated = Arc::new(TopoName::face(feature, Role::Generated(ename)));
        let names: Vec<Name> = tool
            .shape
            .faces()
            .iter()
            .map(|f| if matches!(f.face_surface(), Ok(k::Surface::Plane(_) | k::Surface::Cylinder { .. })) { generated.clone() } else { blend.clone() })
            .collect();
        cuts.push(NamedShape::new(tool.shape, names));
    }
    let (cut, names) = bool_named(ev, k::BooleanOp::Cut, &body_named(&body), &cuts)?;
    let cut = NamedShape::new(cut, names);
    let un = k::unify_same_domain(&cut.shape)?;
    if !un.shape.is_valid() || un.shape.count(ShapeType::Solid) != 1 {
        return Err(fail("the result is not one valid solid"));
    }
    let names = ev.name_result(&[&cut.names], &un.history, &un.shape, &BTreeMap::new(), None);
    ev.replace_body(bi, un.shape, names);
    Ok(())
}

/// Rounds of arcs where a plane meets a cylinder whose ends run on tangentially into edges left out (a picked face's
/// edge, or tangent propagation off): as live's (cf-surface-pattern's Fillet 1, live 2227), the ball rolls between the
/// plane and the cylinder along the arc, then runs straight on past each end between the plane and the cylinder's
/// tangent plane there (a cylinder), until the body trims it. Built on a wedge: the slab under the plane with the
/// cylinder's arc and its two tangent planes, whose three top edges OCCT rounds; the wedge minus its round is cut away.
/// `Ok(false)` (nothing done) unless every edge is such an arc.
pub fn run_on_rim_blend(ev: &mut EvalCtx, bi: usize, edges: &[usize], r: f64) -> FResult<bool> {
    use super::sheetmetal_util::{body_named, bool_named};
    const TAU: f64 = std::f64::consts::TAU;
    let body = ev.ctx.bodies[bi].clone();
    let shape = &body.shape;
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let faces = shape.faces();
    let ef = shape.edge_faces()?;
    let feature = ev.feature;
    let mut cuts = Vec::new();
    for &e in edges {
        let Some(fs) = ef.get(e).filter(|f| f.len() == 2) else { return Ok(false) };
        let pair = match (faces[fs[0] as usize].face_surface()?, faces[fs[1] as usize].face_surface()?) {
            (k::Surface::Plane(_), k::Surface::Cylinder { .. }) => [fs[0], fs[1]],
            (k::Surface::Cylinder { .. }, k::Surface::Plane(_)) => [fs[1], fs[0]],
            _ => return Ok(false),
        };
        let Ok(rim) = rim_of(shape, [&faces[pair[0] as usize], &faces[pair[1] as usize]], e, scale) else { return Ok(false) };
        let edge = shape.sub_shape(ShapeType::Edge, e)?;
        let info = edge.edge_info()?;
        if info.closed {
            return Ok(false);
        }
        // The arc's angles about the axis, less than half way round (its end tangents then part).
        let [a0, a1, am] = [info.start, info.end, edge.point_at(0.5 * (info.first_param + info.last_param))?].map(|q| rim.angle_of(q));
        let sweep = (a1 - a0).rem_euclid(TAU);
        let (start, sweep) = if (am - a0).rem_euclid(TAU) < sweep { (a0, sweep) } else { (a1, TAU - sweep) };
        if !(1e-3..std::f64::consts::PI - 0.05).contains(&sweep) {
            return Ok(false);
        }
        // In the plane square to the axis through its origin: the arc, its ends' tangents away from it and a far
        // side closing them.
        let (x, y) = rim.frame();
        let radial = |a: f64| x * a.cos() + y * a.sin();
        let o = rim.origin;
        let (pa, pb, pm) = (o + radial(start) * rim.rc, o + radial(start + sweep) * rim.rc, o + radial(start + 0.5 * sweep) * rim.rc);
        let (ta, tb) = (-rim.axis.cross(radial(start)), rim.axis.cross(radial(start + sweep)));
        let len = 2.0 * scale.max(rim.rc) + 4.0 * r;
        let back = -radial(start + 0.5 * sweep) * len;
        let (fa, fb) = (pa + ta * len, pb + tb * len);
        let tall = 4.0 * (len + rim.rc + 4.0 * r) + (rim.p - o).dot(rim.axis).abs();
        let at = |q: DVec3| q - rim.axis * tall;
        let wire = k::make_wire(&[
            k::make_arc_3pts(at(pa), at(pm), at(pb))?,
            k::make_line(at(pb), at(fb))?,
            k::make_line(at(fb), at(fb + back))?,
            k::make_line(at(fb + back), at(fa + back))?,
            k::make_line(at(fa + back), at(fa))?,
            k::make_line(at(fa), at(pa))?,
        ])?;
        let prism = k::extrude(&k::make_face_from_wire(&wire, true)?, rim.axis * (2.0 * tall))?.shape;
        // The slab under the plane, its top on it.
        let np = rim.np;
        let cos = rim.axis.dot(np);
        let q = o + rim.axis * ((rim.p - o).dot(np) / cos);
        let half = (rim.rc + 2.0 * len) / cos.abs();
        let down = if rim.wp.dot(np) < 0.0 { -np } else { np };
        let sx = (rim.axis - np * cos).try_normalize().unwrap_or_else(|| np.any_orthonormal_vector());
        let sy = down.cross(sx);
        let slab = k::make_box(&k::Plane::new(q - sx * half - sy * half, down, sx), 2.0 * half, 2.0 * half, 4.0 * r)?;
        let wedge = if rim.out < 0.0 { k::common(&slab, &prism)? } else { k::cut(&slab, &prism)? }.shape;
        // Its top edges along the arc and the tangents (not the far side's).
        let flat = |v: DVec3| v - rim.axis * v.dot(rim.axis);
        let tol = scale * 1e-7;
        let chain: Vec<Shape> = wedge
            .edges()
            .into_iter()
            .filter(|c| {
                let Ok(i) = c.edge_info() else { return false };
                let Ok(m) = c.point_at(0.5 * (i.first_param + i.last_param)) else { return false };
                let on_top = [i.start, i.end, m].iter().all(|v| (*v - rim.p).dot(np).abs() < tol);
                on_top && flat(m - pa).length().min(flat(m - pb).length()) < 0.75 * len
            })
            .collect();
        if chain.len() != 3 {
            return Err(fail("the run-on wedge's top chain"));
        }
        let rounded = k::fillet(&wedge, &chain, r)?;
        let tool = k::cut(&wedge, &rounded.shape)?;
        if !tool.shape.is_valid() || tool.shape.count(ShapeType::Solid) != 1 {
            return Err(fail("the run-on round left no one solid to cut"));
        }
        // Its round: the faces on neither the planes nor the wall.
        let ename = body.edges[e].clone();
        let blend = Arc::new(TopoName::face(feature, Role::Blend(ename.clone())));
        let generated = Arc::new(TopoName::face(feature, Role::Generated(ename)));
        let names: Vec<Name> = tool
            .shape
            .faces()
            .iter()
            .map(|f| match f.face_surface() {
                Ok(k::Surface::Plane(_)) => generated.clone(),
                Ok(k::Surface::Cylinder { axis, radius, .. }) if (radius - rim.rc).abs() < tol && axis.normalize().cross(rim.axis).length() < 1e-9 => generated.clone(),
                _ => blend.clone(),
            })
            .collect();
        cuts.push(NamedShape::new(tool.shape, names));
    }
    let (cut, names) = bool_named(ev, k::BooleanOp::Cut, &body_named(&body), &cuts)?;
    let cut = NamedShape::new(cut, names);
    let un = k::unify_same_domain(&cut.shape)?;
    if !un.shape.is_valid() || un.shape.count(ShapeType::Solid) != 1 {
        return Err(fail("the result is not one valid solid"));
    }
    let names = ev.name_result(&[&cut.names], &un.history, &un.shape, &BTreeMap::new(), None);
    ev.replace_body(bi, un.shape, names);
    Ok(true)
}

/// A face's section through its axis, in (distance from the axis, height along it) about a circle edge's centre.
enum Meridian {
    Line { at: glam::DVec2, dir: glam::DVec2 },
    Circle { center: glam::DVec2, radius: f64 },
}

/// Rounds of whole circles between faces of revolution about the circle's axis, a torus or sphere among them (cf-snap-hook's
/// Fillet 5: R1 where the bosses' R5 rounds meet the case's floor): the round is a torus, the 2D round of the two faces'
/// sections revolved, the region between it and the edge added (concave) or cut away (convex). OCCT approximates it in
/// sixteen B-spline pieces an edge (live: one face). `Ok(false)` (nothing done) unless every edge is such a circle.
pub fn coaxial_round(ev: &mut EvalCtx, bi: usize, edges: &[usize], r: f64) -> FResult<bool> {
    use super::sheetmetal_util::body_named;
    use glam::DVec2;
    let body = ev.ctx.bodies[bi].clone();
    let shape = &body.shape;
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let tol = scale * 1e-9;
    let faces = shape.faces();
    let ef = shape.edge_faces()?;
    let feature = ev.feature;
    let (mut cuts, mut adds) = (Vec::new(), Vec::new());
    for &e in edges {
        let edge = shape.sub_shape(ShapeType::Edge, e)?;
        let info = edge.edge_info()?;
        let k::Curve::Circle { center: o, axis, .. } = info.curve else { return Ok(false) };
        let Some(fs) = ef.get(e).filter(|f| f.len() == 2 && info.closed) else { return Ok(false) };
        let a = axis.normalize();
        let p0 = info.start;
        let x = (p0 - o - a * (p0 - o).dot(a)).normalize();
        let to3 = |q: DVec2| o + x * q.x + a * q.y;
        let to2 = |q: DVec3| DVec2::new((q - o).dot(x), (q - o).dot(a));
        let on_axis = |c: DVec3, ax: DVec3| ax.normalize().cross(a).length() < 1e-9 && (c - o - a * (c - o).dot(a)).length() < tol * 1e3;
        let ff = [&faces[fs[0] as usize], &faces[fs[1] as usize]];
        let mut curved = false;
        let mut mer = Vec::new();
        for f in ff {
            let n = f.normal_at_point(p0)?.normalize();
            let line = || Meridian::Line { at: to2(p0), dir: DVec2::new(-n.dot(a), n.dot(x)) };
            mer.push(match f.face_surface()? {
                k::Surface::Plane(pl) if pl.normal.normalize().cross(a).length() < 1e-9 => line(),
                k::Surface::Cylinder { origin, axis, .. } | k::Surface::Cone { origin, axis, .. } if on_axis(origin, axis) => line(),
                k::Surface::Torus { center, axis, major_radius, minor_radius } if on_axis(center, axis) => {
                    curved = true;
                    Meridian::Circle { center: DVec2::new(major_radius, (center - o).dot(a)), radius: minor_radius }
                }
                k::Surface::Sphere { center, radius, .. } if on_axis(center, a) => {
                    curved = true;
                    Meridian::Circle { center: DVec2::new(0.0, (center - o).dot(a)), radius }
                }
                _ => return Ok(false),
            });
        }
        if !curved {
            return Ok(false);
        }
        let fr = frame_at(shape, ff, p0, edge.tangent_at(info.first_param)?.normalize(), scale)?;
        let convex = fr.u[0].dot(fr.m[1]) > 0.0;
        // The ball's centre: r from both sections, on the material's side of both (convex) or the air's.
        let side = |i: usize| to2(o + fr.m[i] * if convex { 1.0 } else { -1.0 });
        let offset = |i: usize| -> Meridian {
            let s = side(i);
            match mer[i] {
                Meridian::Line { at, dir } => {
                    let nrm = DVec2::new(-dir.y, dir.x).normalize();
                    Meridian::Line { at: at + nrm * (r * nrm.dot(s).signum()), dir }
                }
                Meridian::Circle { center, radius } => {
                    let away = (to2(p0) - center).dot(s) > 0.0;
                    Meridian::Circle { center, radius: if away { radius + r } else { radius - r } }
                }
            }
        };
        let near = to2(p0);
        let cands: Vec<DVec2> = match (offset(0), offset(1)) {
            (Meridian::Line { at: a0, dir: d0 }, Meridian::Line { at: a1, dir: d1 }) => {
                let den = d0.perp_dot(d1);
                if den.abs() < 1e-12 { vec![] } else { vec![a0 + d0 * ((a1 - a0).perp_dot(d1) / den)] }
            }
            (Meridian::Line { at, dir }, Meridian::Circle { center, radius }) | (Meridian::Circle { center, radius }, Meridian::Line { at, dir }) => {
                let d = dir.normalize();
                let t = (center - at).dot(d);
                let h2 = radius * radius - (at + d * t - center).length_squared();
                if h2 < 0.0 { vec![] } else { vec![at + d * (t - h2.sqrt()), at + d * (t + h2.sqrt())] }
            }
            (Meridian::Circle { center: c0, radius: r0 }, Meridian::Circle { center: c1, radius: r1 }) => {
                let dd = (c1 - c0).length();
                if dd < 1e-12 || dd > r0 + r1 || dd < (r0 - r1).abs() {
                    vec![]
                } else {
                    let l = (r0 * r0 - r1 * r1 + dd * dd) / (2.0 * dd);
                    let h = (r0 * r0 - l * l).max(0.0).sqrt();
                    let u = (c1 - c0) / dd;
                    let m = c0 + u * l;
                    vec![m + u.perp() * h, m - u.perp() * h]
                }
            }
        };
        let Some(c) = cands.into_iter().filter(|c| c.x > r).min_by(|p, q| (*p - near).length().total_cmp(&(*q - near).length())) else { return Ok(false) };
        if (c - near).length() > 4.0 * r {
            return Ok(false);
        }
        let foot = |m: &Meridian| match *m {
            Meridian::Line { at, dir } => {
                let d = dir.normalize();
                at + d * (c - at).dot(d)
            }
            Meridian::Circle { center, radius } => center + (c - center).normalize() * radius,
        };
        let t = [foot(&mer[0]), foot(&mer[1])];
        for i in 0..2 {
            if (to3(t[i]) - p0).dot(fr.u[i]) <= 0.0 || !ff[i].contains_point(to3(t[i]), scale * 1e-7)? {
                return Ok(false);
            }
        }
        // Along a section from the edge's point to its touch point.
        let along = |m: &Meridian, from: DVec2, to: DVec2| -> FResult<Shape> {
            Ok(match *m {
                Meridian::Line { .. } => k::make_line(to3(from), to3(to))?,
                Meridian::Circle { center, radius } => {
                    let mid = center + ((from - center).normalize() + (to - center).normalize()).normalize() * radius;
                    k::make_arc_3pts(to3(from), to3(mid), to3(to))?
                }
            })
        };
        let mid = c + (near - c).normalize() * r;
        let wire = k::make_wire(&[along(&mer[0], near, t[0])?, k::make_arc_3pts(to3(t[0]), to3(mid), to3(t[1]))?, along(&mer[1], t[1], near)?])?;
        let tool = k::revolve(&k::make_face_from_wire(&wire, true)?, o, a, std::f64::consts::TAU)?;
        if !tool.shape.is_valid() || tool.shape.count(ShapeType::Solid) != 1 {
            return Err(fail("the coaxial round's region didn't revolve"));
        }
        let ename = body.edges[e].clone();
        let blend = Arc::new(TopoName::face(feature, Role::Blend(ename.clone())));
        let generated = Arc::new(TopoName::face(feature, Role::Generated(ename)));
        let names: Vec<Name> = tool
            .shape
            .faces()
            .iter()
            .map(|f| match f.face_surface() {
                Ok(k::Surface::Torus { major_radius, minor_radius, .. }) if (minor_radius - r).abs() < tol * 1e3 && (major_radius - c.x).abs() < tol * 1e3 => blend.clone(),
                _ => generated.clone(),
            })
            .collect();
        let named = NamedShape::new(tool.shape, names);
        if convex { cuts.push(named) } else { adds.push(named) }
    }
    let mut named = body_named(&body);
    for (op, tools) in [(k::BooleanOp::Cut, cuts), (k::BooleanOp::Fuse, adds)] {
        if tools.is_empty() {
            continue;
        }
        // (At the working scale, an added region glued: OCCT's Boolean of tools lying on the tori they round took
        // 306 ms in metres, 38 so.)
        let glue = if op == k::BooleanOp::Fuse { k::Glue::Shift } else { k::Glue::Off };
        let opts = k::BooleanOptions { glue, scaled: true, ..Default::default() };
        let shapes: Vec<Shape> = tools.iter().map(|t| t.shape.clone()).collect();
        let res = k::boolean(op, std::slice::from_ref(&named.shape), &shapes, &opts)?;
        let mut args: Vec<&crate::naming::ShapeNames> = vec![&named.names];
        args.extend(tools.iter().map(|t| &t.names));
        let n = ev.name_result(&args, &res.history, &res.shape, &BTreeMap::new(), None);
        named = NamedShape::new(res.shape, n);
    }
    let un = k::unify_same_domain(&named.shape)?;
    if !un.shape.is_valid() || un.shape.count(ShapeType::Solid) != 1 {
        return Err(fail("the result is not one valid solid"));
    }
    let names = ev.name_result(&[&named.names], &un.history, &un.shape, &BTreeMap::new(), None);
    ev.replace_body(bi, un.shape, names);
    Ok(true)
}

/// Rounds of boss rims (closed edges where a plane meets a cylinder square to it, the material inside) whose wall is
/// shorter than the round needs, with Allow edge overflow off: as Onshape's, the ball rolls on the cap and over the
/// wall's far edge, the round replacing the wall (cf-snap-hook's Fillet 4: R5 on Ø12 bosses 2 mm tall, live 18
/// faces and 76.32 mm³ off each): a torus of the radius tangent to the cap at R - √(h(2r - h)) from the axis,
/// through the far circle; what lies between it and the corner cut away. `Ok(false)` (nothing done) unless every
/// edge is such a rim.
pub fn cliff_rim_blend(ev: &mut EvalCtx, bi: usize, edges: &[usize], sizes: &[f64]) -> FResult<bool> {
    use super::sheetmetal_util::{body_named, bool_named};
    let body = ev.ctx.bodies[bi].clone();
    let shape = &body.shape;
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let Ok(rims) = rims_of(shape, edges, scale) else { return Ok(false) };
    let feature = ev.feature;
    let mut cuts = Vec::new();
    for (rim, i) in rims {
        let r = sizes.get(i).or(sizes.first()).copied().unwrap_or(0.0);
        // A whole boss rim, its plane square to the axis.
        if !rim.arc.is_empty() || rim.out > 0.0 || rim.axis.dot(rim.np).abs() < 1.0 - 1e-9 || r <= 0.0 {
            return Ok(false);
        }
        let (x, y) = rim.frame();
        let (p0, radial) = rim.at(x, y, 0.0);
        let along = rim.axis * rim.axis.dot(rim.wp).signum();
        let tol = scale * 1e-7;
        // The wall's depth from the cap: its edges round the axis (not its seam) all at 0 or at one far depth h.
        let mut depths: Vec<f64> = Vec::new();
        for e in rim.wall.edges() {
            let Ok(info) = e.edge_info() else { return Ok(false) };
            if let (Ok(a), Ok(b)) = (e.point_at(info.first_param), e.point_at(info.last_param))
                && ((b - a).cross(along).length() < tol && (b - a).length() > tol)
            {
                continue;
            }
            for t in [info.first_param, 0.5 * (info.first_param + info.last_param), info.last_param] {
                let q = e.point_at(t)?;
                depths.push((q - p0).dot(along));
            }
        }
        let h = depths.iter().copied().fold(0.0, f64::max);
        if depths.iter().any(|d| d.abs() > tol && (d - h).abs() > tol) || h <= tol || h >= r * (1.0 - 1e-9) {
            return Ok(false);
        }
        let big = rim.rc;
        let c = big - (h * (2.0 * r - h)).sqrt();
        if c <= tol {
            return Ok(false);
        }
        // In the half-plane through the axis and p0: (s from the axis, t into the boss).
        let cap = p0 - radial * big;
        let at = |s: f64, t: f64| cap + radial * s + along * t;
        let o = (c, r);
        let a_c = (h - r).atan2(big - c);
        let mid = 0.5 * (a_c - std::f64::consts::FRAC_PI_2);
        let profile = k::make_wire(&[
            k::make_line(at(c, 0.0), at(big, 0.0))?,
            k::make_line(at(big, 0.0), at(big, h))?,
            k::make_arc_3pts(at(big, h), at(o.0 + r * mid.cos(), o.1 + r * mid.sin()), at(c, 0.0))?,
        ])?;
        let face = k::make_face_from_wire(&profile, true)?;
        let tool = k::revolve(&face, cap, rim.axis, std::f64::consts::TAU)?;
        if !tool.shape.is_valid() || tool.shape.count(ShapeType::Solid) != 1 {
            return Ok(false);
        }
        let ename = body.edges[edges[i]].clone();
        let blend = Arc::new(TopoName::face(feature, Role::Blend(ename.clone())));
        let generated = Arc::new(TopoName::face(feature, Role::Generated(ename)));
        let names: Vec<Name> = tool.shape.faces().iter().map(|f| if matches!(f.face_surface(), Ok(k::Surface::Plane(_) | k::Surface::Cylinder { .. })) { generated.clone() } else { blend.clone() }).collect();
        cuts.push(NamedShape::new(tool.shape, names));
    }
    let (cut, names) = bool_named(ev, k::BooleanOp::Cut, &body_named(&body), &cuts)?;
    let cut = NamedShape::new(cut, names);
    let un = k::unify_same_domain(&cut.shape)?;
    if !un.shape.is_valid() || un.shape.count(ShapeType::Solid) != 1 {
        return Err(fail("the result is not one valid solid"));
    }
    let names = ev.name_result(&[&cut.names], &un.history, &un.shape, &BTreeMap::new(), None);
    ev.replace_body(bi, un.shape, names);
    Ok(true)
}

/// Where a rim's round can have its seam (the wedge's cylinder's x direction): an angle about the axis at which
/// its touch lines on the plane and the cylinder stay (a point a little past each lies on its face), from the
/// cylinder's own seam round; the seam's ends then split only pieces of one curve (one edge, as live's, which has no
/// seam), never a curve where the round meets another face or round.
fn seam_where_whole(rim: &Rim, r: f64) -> FResult<DVec3> {
    let (x, y) = rim.frame();
    // An arc's blend: the middle of the longest way round off the arc, where the result keeps none of it (just past
    // the arc's ends the blend runs on until it leaves the part).
    if !rim.arc.is_empty() {
        const N: usize = 64;
        let angle = |k: usize| k as f64 * std::f64::consts::TAU / N as f64;
        let off: Vec<bool> = (0..N).map(|k| !rim.on_arc(angle(k))).collect();
        let (mut best, mut best_len) = (0, 0);
        for start in (0..N).filter(|&k| off[k] && !off[(k + N - 1) % N]) {
            let len = (0..N).take_while(|&j| off[(start + j) % N]).count();
            if len > best_len {
                (best, best_len) = (start, len);
            }
        }
        return Ok(rim.at(x, y, angle(best) + angle(best_len) / 2.0).1);
    }
    let along_w = rim.axis * rim.axis.dot(rim.wp).signum();
    const N: usize = 16;
    for k in 0..N {
        // 0, +1, -1, +2, ... steps from the cylinder's seam.
        let step = (k as f64 / 2.0).ceil() * if k % 2 == 1 { 1.0 } else { -1.0 };
        let (p, radial) = rim.at(x, y, step * std::f64::consts::TAU / N as f64);
        let into_p = (radial - rim.np * radial.dot(rim.np)).normalize() * rim.out;
        if rim.plane.contains_point(p + into_p * (1.2 * r), r * 1e-3)? && rim.wall.contains_point(rim.onto_wall(p + along_w * (2.2 * r)), r * 1e-3)? {
            return Ok(radial);
        }
    }
    Ok(x)
}

/// A variable round's radius law as cubic Hermite spans: its points (fraction from the edge's start, radius) and the
/// slope (per fraction) at each.
pub type RadiusLaw = (Vec<(f64, f64)>, Vec<f64>);

/// Whether a variable round of straight edge `e` between two planes is wider somewhere than a face beside it (its
/// setback past the face's far side): where OCCT's variable fillet fails (c2-tpd-fillet-variable: 40 mm on 25 mm walls,
/// ~70 ms to fail), so [`variable_round`] goes first. The law's largest radius is bounded by its Bézier values.
pub fn variable_round_overflows(shape: &Shape, e: usize, (points, slopes): &RadiusLaw) -> bool {
    let (Ok(edge), Ok(ef)) = (shape.sub_shape(ShapeType::Edge, e), shape.edge_faces()) else { return false };
    let Ok(info) = edge.edge_info() else { return false };
    let k::Curve::Line { .. } = info.curve else { return false };
    let Some(fs) = ef.get(e).filter(|f| f.len() == 2) else { return false };
    let faces = shape.faces();
    let mid = 0.5 * (info.start + info.end);
    let d = (info.end - info.start).normalize();
    let mut normals = Vec::new();
    let mut width = f64::INFINITY;
    for &f in fs {
        let face = &faces[f as usize];
        let (Ok(Some(_)), Ok(n)) = (face.face_plane(), face.normal_at_point(mid)) else { return false };
        normals.push(n.normalize());
        let off = |q: DVec3| (q - info.start - d * (q - info.start).dot(d)).length();
        width = width.min(face.vertices().iter().filter_map(|v| v.vertex_point().ok()).map(off).fold(0.0, f64::max));
    }
    let half = 0.5 * normals[0].dot(normals[1]).clamp(-1.0, 1.0).acos();
    let len = (info.end - info.start).length().max(1e-12);
    let mut r_max = points.iter().map(|p| p.1).fold(0.0, f64::max);
    for (w, m) in points.windows(2).zip(slopes.windows(2)) {
        let h = (w[1].0 - w[0].0) * len;
        r_max = r_max.max(w[0].1 + m[0] / len * h / 3.0).max(w[1].1 - m[1] / len * h / 3.0);
    }
    r_max * half.tan() > width * (1.0 + 1e-9)
}

/// A variable round of straight edges between planes where OCCT's fails (c2-tpd-fillet-variable's Fillet 1: radii to
/// 40 mm on 25 mm walls, the round running down onto the floor): per edge, its exact round (`k::round_tool`: the
/// circular section swept with the radius its law gives, `law`: the Hermite spans' points (fraction from the edge's
/// start, radius) and slopes per fraction), on past a free end (into a corner's neighbour's round, mitred there), and
/// every edge's region between the edge and its round cut away (added at a concave edge) at once, so a round ends where
/// it meets the faces beyond or another round.
pub fn variable_round(ev: &mut EvalCtx, bi: usize, laws: &[(usize, RadiusLaw)]) -> FResult<()> {
    use super::sheetmetal_util::{body_named, bool_named};
    let body = ev.ctx.bodies[bi].clone();
    let shape = &body.shape;
    let faces = shape.faces();
    let ef = shape.edge_faces()?;
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let feature = ev.feature;
    let (mut cuts, mut adds) = (Vec::new(), Vec::new());
    for (e, (points, slopes)) in laws {
        let (Some(&(_, r_start)), Some(&(_, r_end))) = (points.first(), points.last()) else { return Err(fail("an empty law")) };
        let edge = shape.sub_shape(ShapeType::Edge, *e)?;
        let info = edge.edge_info()?;
        if !matches!(info.curve, k::Curve::Line { .. }) {
            return Err(fail("a variable round of a curved edge"));
        }
        let fs = ef.get(*e).filter(|f| f.len() == 2).ok_or_else(|| fail("the edge doesn't have two faces"))?;
        let ff = [&faces[fs[0] as usize], &faces[fs[1] as usize]];
        if ff.iter().any(|f| f.face_plane().ok().flatten().is_none()) {
            return Err(fail("a variable round off planes"));
        }
        let (a, b) = (info.start, info.end);
        let len = (b - a).length();
        let d = (b - a) / len;
        let fr = frame_at(shape, ff, (a + b) * 0.5, d, scale)?;
        let convex = fr.u[0].dot(fr.m[1]) > 0.0;
        // On past an end with nothing of the part beyond it (a corner's: the neighbour's round meets this one there).
        let ext = |p: DVec3, out: DVec3, r: f64| -> FResult<f64> {
            if !convex {
                return Ok(0.0);
            }
            let probe = p + out * (r * 0.5) + (fr.u[0] + fr.u[1]) * (r * 0.05);
            Ok(if shape.classify_point(probe, scale * 1e-9)? == k::PointClass::Outside { 1.5 * r } else { 0.0 })
        };
        let (e0, e1) = (ext(a, -d, r_start)?, ext(b, d, r_end)?);
        // The law's spans as cubic Béziers in s (the slopes per fraction: per length / len), the ends' run-ons constant.
        let mut spans: Vec<(f64, f64, [f64; 4])> = Vec::new();
        if e0 > 0.0 {
            spans.push((-e0, 0.0, [r_start; 4]));
        }
        for (k, w) in points.windows(2).enumerate() {
            let (s0, s1) = (w[0].0 * len, w[1].0 * len);
            let h = s1 - s0;
            let (m0, m1) = (slopes[k] / len, slopes[k + 1] / len);
            spans.push((s0, s1, [w[0].1, w[0].1 + m0 * h / 3.0, w[1].1 - m1 * h / 3.0, w[1].1]));
        }
        if e1 > 0.0 {
            spans.push((len, len + e1, [r_end; 4]));
        }
        let half = fr.m[0].dot(fr.m[1]).clamp(-1.0, 1.0).acos() / 2.0;
        let tool = k::round_tool(a, d, fr.u[0], fr.u[1], half, &spans)?;
        if !tool.is_valid() || tool.count(ShapeType::Solid) != 1 {
            return Err(fail("a variable round's tool isn't one solid"));
        }
        // Its round: the face on neither support's plane.
        let ename = body.edges[*e].clone();
        let blend = Arc::new(TopoName::face(feature, Role::Blend(ename.clone())));
        let generated = Arc::new(TopoName::face(feature, Role::Generated(ename)));
        let names: Vec<Name> = tool.faces().iter().map(|f| if f.face_plane().ok().flatten().is_some() { generated.clone() } else { blend.clone() }).collect();
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
    if !un.shape.is_valid() || un.shape.count(ShapeType::Solid) != 1 {
        return Err(fail("the result is not one valid solid"));
    }
    let names = ev.name_result(&[&named.names], &un.history, &un.shape, &BTreeMap::new(), None);
    ev.replace_body(bi, un.shape, names);
    Ok(())
}

/// A chamfer whose sides Onshape measures other than OCCT where a face is curved across the edge. Tangent (APEX_RANGE):
/// `d` along each face's tangent line from where the two meet, or `d` along the reference face's and the chamfer at an
/// angle to it. Offset's distance and angle: the angle to the straight line from the chamfer's side to the edge (OCCT:
/// to the reference face's tangent there).
#[derive(Clone, Copy)]
pub enum ChamferSides {
    Tangent(f64, f64),
    TangentAngle(f64, f64),
    OffsetAngle(f64, f64),
}

/// A point on a face's section across an edge and its unit tangent, at a parameter from the edge.
type SectionCurve = Box<dyn Fn(f64) -> (glam::DVec2, glam::DVec2)>;

/// A face's section across an edge: a line, or a circle about a center.
enum Sec {
    Line,
    Circle(DVec3, f64),
}

/// The section of a face across edge curve `curve` at `p`, where it is the same all along the edge (a plane or a
/// parallel cylinder along a line; a face of revolution about an arc's axis).
fn constant_section(surface: &k::Surface, curve: &k::Curve, p: DVec3) -> Option<Sec> {
    let parallel = |a: DVec3, b: DVec3| a.normalize().cross(b.normalize()).length() < 1e-9;
    let on_axis = |q: DVec3, c: DVec3, axis: DVec3| (q - c).cross(axis.normalize()).length() < 1e-9;
    match (curve, surface) {
        (k::Curve::Line { .. }, k::Surface::Plane(_)) => Some(Sec::Line),
        (k::Curve::Line { direction, .. }, k::Surface::Cylinder { origin, axis, radius, .. }) if parallel(*direction, *axis) => {
            let a = axis.normalize();
            Some(Sec::Circle(*origin + a * (p - *origin).dot(a), *radius))
        }
        (k::Curve::Circle { axis, .. }, k::Surface::Plane(pl)) if parallel(*axis, pl.normal) => Some(Sec::Line),
        (k::Curve::Circle { center, axis, .. }, k::Surface::Cylinder { origin, axis: a, .. } | k::Surface::Cone { origin, axis: a, .. })
            if parallel(*axis, *a) && on_axis(*origin, *center, *axis) =>
        {
            Some(Sec::Line)
        }
        (k::Curve::Circle { center, axis, .. }, k::Surface::Sphere { center: c, radius, .. }) if on_axis(*c, *center, *axis) => Some(Sec::Circle(*c, *radius)),
        (k::Curve::Circle { center, axis, .. }, k::Surface::Torus { center: c, axis: a, major_radius, minor_radius }) if parallel(*axis, *a) && on_axis(*c, *center, *axis) => {
            let a = a.normalize();
            let radial = ((p - *c) - a * (p - *c).dot(a)).normalize_or_zero();
            Some(Sec::Circle(*c + radial * *major_radius, *minor_radius))
        }
        _ => None,
    }
}

/// A chamfer in OCCT's terms at edge `e` (an index in `shape`'s edges): the straight distances from the edge to where
/// it meets `faces[0]` (the reference face) and `faces[1]`; Some(None) where OCCT reads `chamfer` as Onshape does. None
/// where a face's section changes along the edge. Live (11-08, a flat on a round shaft) for all five.
pub fn chamfer_offsets(shape: &Shape, e: usize, faces: [&Shape; 2], chamfer: ChamferSides) -> FResult<Option<Option<[f64; 2]>>> {
    use glam::DVec2;
    let edge = shape.sub_shape(ShapeType::Edge, e)?;
    let info = edge.edge_info()?;
    let mid = 0.5 * (info.first_param + info.last_param);
    let (p, t) = (edge.point_at(mid)?, edge.tangent_at(mid)?.normalize());
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let mut secs = Vec::new();
    for f in faces {
        let Some(sec) = constant_section(&f.face_surface()?, &info.curve, p) else { return Ok(None) };
        secs.push(sec);
    }
    // (OCCT's distance and angle stops below 90°: an acute edge's chamfer can lean further.)
    let curved = match chamfer {
        ChamferSides::OffsetAngle(_, angle) | ChamferSides::TangentAngle(_, angle) if angle >= std::f64::consts::FRAC_PI_2 => true,
        ChamferSides::OffsetAngle(..) => matches!(secs[0], Sec::Circle(..)),
        _ => secs.iter().any(|s| matches!(s, Sec::Circle(..))),
    };
    if !curved {
        return Ok(Some(None));
    }
    let frame = frame_at(shape, faces, p, t, scale)?;
    let e1 = frame.u[0].normalize();
    let e2 = t.cross(e1);
    let flat = |v: DVec3| DVec2::new(v.dot(e1), v.dot(e2));
    // Each face's section from the edge (the origin) into the face: a point and its unit tangent at parameter φ
    // (a length along a line, an angle around a circle).
    let curves: Vec<SectionCurve> = secs
        .iter()
        .zip(frame.u)
        .map(|(sec, u)| -> SectionCurve {
            let b = flat(u).normalize();
            match *sec {
                Sec::Line => Box::new(move |s| (b * s, b)),
                Sec::Circle(c, r) => {
                    let c = flat(c - p);
                    let a = -c / r;
                    Box::new(move |phi: f64| (c + (a * phi.cos() + b * phi.sin()) * r, b * phi.cos() - a * phi.sin()))
                }
            }
        })
        .collect();
    let start = |sec: &Sec, d: f64| match sec {
        Sec::Line => d,
        Sec::Circle(_, r) => d / r,
    };
    let angle_between = |a: DVec2, b: DVec2| a.perp_dot(b).abs().atan2(a.dot(b));
    let residual = |x: [f64; 2]| -> [f64; 2] {
        let (p0, t0) = curves[0](x[0]);
        let (p1, t1) = curves[1](x[1]);
        match chamfer {
            ChamferSides::Tangent(d0, d1) => {
                let gap = (p0 - t0 * d0) - (p1 - t1 * d1);
                [gap.x, gap.y]
            }
            ChamferSides::TangentAngle(d, angle) => {
                let apex = p0 - t0 * d;
                [(apex - p1).perp_dot(t1), angle_between(p1 - p0, -t0) - angle]
            }
            ChamferSides::OffsetAngle(d, angle) => [p0.length() - d, angle_between(p1 - p0, -p0) - angle],
        }
    };
    let (d0, d1) = match chamfer {
        ChamferSides::Tangent(a, b) => (a, b),
        ChamferSides::TangentAngle(d, angle) | ChamferSides::OffsetAngle(d, angle) if angle < std::f64::consts::FRAC_PI_4 => (d, d * angle.tan()),
        ChamferSides::TangentAngle(d, _) | ChamferSides::OffsetAngle(d, _) => (d, 2.0 * d),
    };
    let mut x = [start(&secs[0], d0), start(&secs[1], d1)];
    let tol = 1e-12 * scale;
    for _ in 0..60 {
        let f = residual(x);
        if f[0].hypot(f[1]) < tol {
            break;
        }
        let mut jac = [[0.0; 2]; 2];
        for j in 0..2 {
            let h = 1e-7 * x[j].abs().max(1e-3 * d0.max(1e-9));
            let mut y = x;
            y[j] += h;
            let g = residual(y);
            jac[0][j] = (g[0] - f[0]) / h;
            jac[1][j] = (g[1] - f[1]) / h;
        }
        let det = jac[0][0] * jac[1][1] - jac[0][1] * jac[1][0];
        if det.is_nan() || det.abs() <= 1e-300 {
            return Ok(None);
        }
        x[0] -= (f[0] * jac[1][1] - f[1] * jac[0][1]) / det;
        x[1] -= (jac[0][0] * f[1] - jac[1][0] * f[0]) / det;
    }
    let f = residual(x);
    let inside = |sec: &Sec, v: f64| v.is_finite() && v > 0.0 && (matches!(sec, Sec::Line) || v < std::f64::consts::PI);
    let miss = f[0].hypot(f[1]);
    if miss.is_nan() || miss >= tol * 1e3 || !inside(&secs[0], x[0]) || !inside(&secs[1], x[1]) {
        return Ok(None);
    }
    Ok(Some(Some([curves[0](x[0]).0.length(), curves[1](x[1]).0.length()])))
}
