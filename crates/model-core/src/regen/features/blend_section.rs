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
use glam::{DVec2, DVec3};
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
    /// A chamfer OCCT can't build, as sections: `r1` on these faces, `r2` on the others or, with an angle, the chamfer
    /// at that angle to the `r1` face (its other distance from the angle between the faces at each section).
    pub reference: Option<ChamferRef>,
}

/// See [`BlendSpec::reference`].
#[derive(Clone, Debug)]
pub struct ChamferRef {
    pub faces: Vec<usize>,
    pub angle: Option<f64>,
    /// The section's constant offsets from the reference face and from the other (live 2441 / 2462: a chamfer is the
    /// chord between the feet of a point at fixed offsets from both faces, sized so its distances (and angle) are the
    /// asked ones at the middle of the chain's longest edge): set by [`section_blend`].
    pub offsets: Option<(f64, f64)>,
}

/// The offsets from the reference face and the other whose feet lie `d` and `e` from the edge, faces at cos `c`.
fn feet_offsets(d: f64, e: f64, c: f64) -> FResult<(f64, f64)> {
    let s = (1.0 - c * c).sqrt();
    if s < 1e-9 {
        return Err(fail("the faces at the edge are tangent"));
    }
    Ok(((e - d * c) / s, (d - e * c) / s))
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
    pub hold: Vec<HoldLine>,
}

/// A tangent hold line's edge with its points at even parameters, sampled once and read at every station.
#[derive(Clone, Debug)]
pub struct HoldLine {
    edge: Shape,
    t: (f64, f64),
    pts: Vec<DVec3>,
}

impl HoldLine {
    const N: usize = 32;

    pub fn new(edge: Shape) -> FResult<Self> {
        let info = edge.edge_info()?;
        let t = (info.first_param, info.last_param);
        let pts = (0..=Self::N).map(|i| edge.point_at(t.0 + (t.1 - t.0) * i as f64 / Self::N as f64)).collect::<Result<Vec<_>, _>>()?;
        Ok(HoldLine { edge, t, pts })
    }
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

/// Distance along `chain` (of length `total`) of a partial fillet's bound.
fn bound_along(chain: &[ChainEdge], total: f64, bd: Bound) -> FResult<f64> {
    Ok(match bd {
        Bound::Fraction(f) => f.clamp(0.0, 1.0) * total,
        Bound::Offset(d) => d.clamp(0.0, total),
        Bound::OffsetFromEnd(d) => (total - d).clamp(0.0, total),
        Bound::Point(q) => {
            let (mut best, mut acc, mut where_) = (f64::INFINITY, 0.0, 0.0);
            for c in chain {
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
            'chain: for c in chain {
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
}

/// A partial fillet's range along `edges`' chain (from its end nearest the origin corner): from, to and its length.
pub(super) fn partial_range(shape: &Shape, edges: &[usize], p: &Partial) -> FResult<(f64, f64, f64)> {
    let chain = chain(shape, edges)?;
    let total: f64 = chain.iter().map(|c| c.length).sum();
    let a = bound_along(&chain, total, p.first)?;
    Ok(match p.second {
        Some(sb) => {
            let b = bound_along(&chain, total, sb)?;
            (a.min(b), a.max(b), total)
        }
        None if p.opposite => (a, total, total),
        None => (0.0, a, total),
    })
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
/// An edge of a run (see [`chain_runs`]): its index, start, end, length, and whether it runs along its own parameter.
pub(super) type RunEdge = (usize, DVec3, DVec3, f64, bool);

/// `edges` as runs of connected edges, each from its own free end, in the run's order.
pub(super) fn chain_runs(shape: &Shape, edges: &[usize]) -> FResult<Vec<Vec<RunEdge>>> {
    let mut runs: Vec<Vec<RunEdge>> = Vec::new();
    for c in chain_ordered(shape, edges, true)? {
        let joined = runs.last().and_then(|r| r.last()).is_some_and(|l| (l.2 - c.start).length() < 1e-9);
        if !joined {
            runs.push(Vec::new());
        }
        runs.last_mut().expect("a run").push((c.index, c.start, c.end, c.length, c.forward));
    }
    Ok(runs)
}

fn chain(shape: &Shape, edges: &[usize]) -> FResult<Vec<ChainEdge>> {
    chain_ordered(shape, edges, false)
}

/// [`chain`]; with `own_ends` each of several chains starts from its own free end (the rolling balls' junctions), else
/// a later chain runs as its first edge does (the section route's run-ons read their ends so: qa's
/// rust-zero-solid-f197's Fillet 1, two straight edges).
fn chain_ordered(shape: &Shape, edges: &[usize], own_ends: bool) -> FResult<Vec<ChainEdge>> {
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
    let first_free = |left: &[(usize, Shape, DVec3, DVec3, f64)]| {
        left.iter().flat_map(|it| [it.2, it.3]).filter(|p| count(*p) == 1).fold(None, |m: Option<DVec3>, p| match m {
            Some(q) if !lt(p, q) => Some(q),
            _ => Some(p),
        })
    };
    let mut start = first_free(&items);
    let mut out = Vec::new();
    let mut left = items;
    while !left.is_empty() {
        let at = match start.filter(|s| !own_ends || left.iter().any(|it| near(it.2, *s) || near(it.3, *s))) {
            Some(s) => s,
            None => first_free(&left).unwrap_or(left[0].2),
        };
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
    if let Some(r) = spec.reference.as_ref().filter(|_| spec.section == Section::Chamfer) {
        if let Some((o_ref, o_other)) = r.offsets {
            let c = f.u[0].dot(f.u[1]).clamp(-1.0, 1.0);
            let sn = (1.0 - c * c).sqrt();
            if sn < 1e-9 {
                return Err(fail("the faces at the edge are tangent"));
            }
            let (on_ref, on_other) = ((o_other + o_ref * c) / sn, (o_ref + o_other * c) / sn);
            return Ok(if first_is_side1 { (on_ref, on_other) } else { (on_other, on_ref) });
        }
        let other = match r.angle {
            // The triangle edge - r1 point - chamfer's far end: its angle at the edge is the faces' angle.
            Some(a) => {
                let between = f.u[0].dot(f.u[1]).clamp(-1.0, 1.0).acos();
                if a + between >= std::f64::consts::PI - 1e-9 {
                    return Err(fail("the chamfer's angle doesn't meet the other face"));
                }
                spec.r1 * a.sin() / (a + between).sin()
            }
            None => spec.r2,
        };
        return Ok(if first_is_side1 { (spec.r1, other) } else { (other, spec.r1) });
    }
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
    let (wire, edges, convex) = section_wire(p, f, spec, first_is_side1)?;
    Ok((k::make_face_from_wire(&wire, true)?, edges, convex))
}

/// [`section_face`]'s outline only (a loft's sections: ThruSections reads their wires).
fn section_wire(p: DVec3, f: &Frame, spec: &BlendSpec, first_is_side1: bool) -> FResult<(Shape, [Shape; 3], bool)> {
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
        return Ok((k::make_wire(&edges)?, edges, convex));
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
    // (A chamfer as sections: its corner out in the air, so the tool's sides needn't lie on free-form faces.)
    let p = if spec.reference.is_some() && convex { p - (f.m[0] + f.m[1]).normalize() * s1.max(s2) } else { p };
    let edges = [k::make_line(p, t1)?, curve, k::make_line(t2, p)?];
    Ok((k::make_wire(&edges)?, edges, convex))
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
    // (Boxes first: a distance to each of the knife's faces took 100 ms of its Fillet 2.)
    let boxes: Vec<k::BoundingBox> = run_on.iter().filter_map(|p| p.bounding_box(false).ok()).collect();
    let near_box = |f: &Shape| f.bounding_box(false).map_or(true, |b| boxes.iter().any(|r| (b.min - DVec3::splat(tol)).cmple(r.max).all() && r.min.cmple(b.max + DVec3::splat(tol)).all()));
    // (The faces it meets in parallel: a distance to a free-form face takes ms.)
    use rayon::prelude::*;
    let body_faces = shape.faces();
    let legacy = k::kernel_legacy();
    let met: Vec<bool> = body_faces
        .par_iter()
        .enumerate()
        .map(|(i, f)| !supports.contains(&i) && near_box(f) && k::with_kernel_legacy(legacy.clone(), || run_on.iter().any(|p| k::distance(f, p).is_ok_and(|d| d.distance <= tol))))
        .collect();
    for (i, f) in body_faces.iter().enumerate() {
        if !met[i] {
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
    sections(ev, bi, edges, spec, false)
}

/// A round of radius `r` whose sections on curved faces are exact rolling balls, one face per edge, running on past
/// the chains' free ends until off the part (live c2-hunting-knife's Fillet 3: the tip's rolling-ball faces where
/// OCCT's pinch, the corners' single faces trimmed by the side planes where OCCT's split in two).
pub fn rolling_ball_blend(ev: &mut EvalCtx, bi: usize, edges: &[usize], r: f64) -> FResult<()> {
    let spec = BlendSpec { section: Section::Circular, r1: r, r2: r, width: None, partial: None, run_on: false, cliff: false, sides: None, overflow: false, reference: None };
    sections(ev, bi, edges, &spec, true)
}

fn sections(ev: &mut EvalCtx, bi: usize, edges: &[usize], spec: &BlendSpec, ball: bool) -> FResult<()> {
    use super::sheetmetal_util::{body_named, bool_named};
    let body = ev.ctx.bodies[bi].clone();
    let shape = &body.shape;
    let faces = shape.faces();
    let ef = shape.edge_faces()?;
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let chain = chain_ordered(shape, edges, ball)?;
    let total: f64 = chain.iter().map(|c| c.length).sum();
    // Distance along the chain of a bound.
    let along = |bd: Bound| bound_along(&chain, total, bd);
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
    // A chamfer as sections: each edge's frame oriented once, at its middle (classifying points at every section took
    // half its time); the sections only follow the faces' normals.
    // (A held blend's too: ~450 stations to find its pieces, c2-variable-fillet-faceblend's Face blend 1.)
    let held_any = spec.sides.as_ref().is_some_and(|s| !s.hold.is_empty());
    let edge_frames: Vec<Option<Frame>> = match spec.reference.is_some() || held_any {
        true => chain
            .iter()
            .map(|c| {
                let info = c.shape.edge_info().ok()?;
                let m = 0.5 * (info.first_param + info.last_param);
                let fs = ef.get(c.index).filter(|f| f.len() == 2)?;
                frame_at(shape, [&faces[fs[0] as usize], &faces[fs[1] as usize]], c.shape.point_at(m).ok()?, c.shape.tangent_at(m).ok()?.normalize(), scale).ok()
            })
            .collect(),
        false => Vec::new(),
    };
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
                let side1_first = match &spec.reference {
                    Some(r) => r.faces.contains(&(fs[0] as usize)),
                    None => spec.sides.as_ref().is_none_or(|sd| sd.side1.contains(&(fs[0] as usize))),
                };
                let tangent = c.shape.tangent_at(t)?.normalize();
                let oriented = match edge_frames.get(i).and_then(|f| f.as_ref()) {
                    Some(rf) => {
                        let mut fr = Frame { u: rf.u, m: rf.m, extent: rf.extent };
                        for (k, face) in ff.iter().enumerate() {
                            let n = if face.is_planar() { rf.m[k] } else { surface_normal(face, p)? };
                            let dir = n.cross(tangent).normalize();
                            fr.u[k] = if dir.dot(rf.u[k]) < 0.0 { -dir } else { dir };
                            fr.m[k] = if n.dot(rf.m[k]) < 0.0 { -n } else { n };
                        }
                        Some(fr)
                    }
                    None => None,
                };
                let fr = match oriented.map_or_else(|| frame_at(shape, ff, p, tangent, scale), Ok) {
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
                // A chamfer as sections measures along the faces' sections, not their tangents (a hollow grind): each
                // face's direction is the chord to its point at the first distance.
                let mut fr = fr;
                if spec.reference.is_some() {
                    for (i, face) in ff.iter().enumerate().filter(|(_, f)| !f.is_planar()) {
                        let mut a = p + fr.u[i] * spec.r1;
                        for _ in 0..2 {
                            let v = face.project_point(a)?.point - p;
                            let v = v - tangent * v.dot(tangent);
                            a = p + v.normalize() * spec.r1;
                        }
                        fr.u[i] = (a - p).normalize();
                    }
                }
                return Ok((p, fr, side1_first, tangent));
            }
            acc += c.length;
        }
        Err(fail("empty chain"))
    };
    // A chamfer as sections: its offsets from the asked distances at one place, the same all along.
    let sized;
    let spec = match spec.reference.as_ref().filter(|r| r.offsets.is_none() && spec.section == Section::Chamfer) {
        Some(r) => {
            // (The middle of the chain's longest edge: c2-hunting-knife's Chamfer 1, live 2462.)
            let (mut acc, mut mid, mut longest) = (0.0f64, (lo + hi) / 2.0, 0.0f64);
            for c in &chain {
                if c.length > longest {
                    (mid, longest) = ((acc + c.length / 2.0).clamp(lo, hi), c.length);
                }
                acc += c.length;
            }
            // (Only where the faces' angle varies along the chain: elsewhere the offsets are OCCT's distances, and a
            // chamfer OCCT can't build stays failed: c2-rings-loft's pattern of Chamfer 2.)
            let mut spread = (f64::INFINITY, f64::NEG_INFINITY);
            for k in 0..=8 {
                let (_, fr, _, _) = station(lo + (hi - lo) * k as f64 / 8.0)?;
                let a = fr.u[0].dot(fr.u[1]).clamp(-1.0, 1.0).acos();
                spread = (spread.0.min(a), spread.1.max(a));
            }
            if spread.1 - spread.0 < 1e-3 {
                return Err(fail("the faces' angle is the same all along"));
            }
            let (_, fr, ref_first, _) = station(mid)?;
            let (d, e) = setbacks(&fr, spec, ref_first)?;
            let (d, e) = if ref_first { (d, e) } else { (e, d) };
            let offsets = feet_offsets(d, e, fr.u[0].dot(fr.u[1]).clamp(-1.0, 1.0))?;
            sized = BlendSpec { reference: Some(ChamferRef { offsets: Some(offsets), ..r.clone() }), ..spec.clone() };
            &sized
        }
        None => spec,
    };
    let ranges = hold_ranges(spec, lo, hi, scale, &station)?;
    if std::env::var_os("NOSHAPE_BLEND_DEBUG").is_some() {
        eprintln!("section blend: ranges {ranges:?} of {total}");
    }
    let partial = spec.partial.is_some() || ranges.len() > 1;
    // Other curves (live NE: an elliptic boss's foot): sections along the
    // whole chain, lofted into one tool (live's blend face is one spline).
    let general = chain.iter().any(|c| c.shape.edge_info().is_ok_and(|i| !matches!(i.curve, k::Curve::Line { .. } | k::Curve::Circle { .. })));
    let feature = ev.feature;
    let (mut cuts, mut adds) = (Vec::new(), Vec::new());
    // Each cut's chain edge (its rim, for the direct build).
    let mut cut_edges: Vec<usize> = Vec::new();
    let mut refaced: Vec<(usize, Shape)> = Vec::new();
    let mut conics = Vec::new();
    if ball {
        let (tools, contacts) = ball_tools(shape, &chain, &ef, &faces, spec.r1, scale)?;
        conics = contacts;
        for (ci, tool) in tools.into_iter().enumerate() {
            let fs = &ef[chain[ci].index];
            let supports = [&faces[fs[0] as usize], &faces[fs[1] as usize]];
            let ename = body.edges[chain[ci].index].clone();
            let blend = Arc::new(TopoName::face(feature, Role::Blend(ename.clone())));
            let generated = Arc::new(TopoName::face(feature, Role::Generated(ename)));
            let names = tool
                .faces()
                .iter()
                .map(|f| match super::helpers_a::point_on_face(f) {
                    Some(q) if supports.iter().any(|s| s.contains_point(q, scale * 1e-6).unwrap_or(false)) => generated.clone(),
                    Some(_) if f.is_planar() => generated.clone(),
                    _ => blend.clone(),
                })
                .collect();
            cuts.push(NamedShape::new(tool, names));
            cut_edges.push(chain[ci].index);
        }
    }
    for &(lo, hi, held) in ranges.iter().filter(|_| !ball) {
    if general || held {
        const N: usize = 64;
        let whole = (chain[0].start - chain[chain.len() - 1].end).length() < 1e-9 && lo <= 1e-12 && hi >= total - 1e-12;
        // A chamfer as sections: one loft (one face) per edge, as live's; others one along the chain.
        let mut pieces = vec![(lo, hi, 0)];
        if held && spec.reference.is_none() {
            // A held blend: one loft (one face) per chain edge and hold line edge, as live's (c2-variable-fillet-
            // faceblend: 8 faces round a pocket's rim); its radius bends where either changes, a loft through such a
            // bend needs ~4x the sections.
            pieces = held_pieces(spec, &chain, lo, hi, scale, &station)?;
            if std::env::var_os("NOSHAPE_BLEND_DEBUG").is_some() {
                eprintln!("section blend: held pieces {pieces:?}");
            }
        } else if spec.reference.is_some() && !whole && chain.len() > 1 {
            pieces.clear();
            let mut acc = 0.0f64;
            for (ci, c) in chain.iter().enumerate() {
                let (a, b) = (acc.max(lo), (acc + c.length).min(hi));
                acc += c.length;
                if b > a {
                    pieces.push((a, b, ci));
                }
            }
        }
        let closed = whole && pieces.len() == 1;
        let last_piece = pieces.len() - 1;
        // A round's sections as wires (its bare loft reads only those); a chamfer's as faces (its loft's history).
        let section = if spec.reference.is_some() { section_face } else { section_wire };
        let mut jobs = Vec::new();
        for (pi, &(plo, phi, ci)) in pieces.iter().enumerate() {
            // (A chamfer as sections: a ruled face, fewer sections; its cut took most of its time.)
            let total_n = if spec.reference.is_some() { 24 } else { N };
            let total_n = if held && pieces.len() > 1 { total_n.max(24 * pieces.len()) } else { total_n };
            let n = if pieces.len() > 1 { ((total_n as f64 * (phi - plo) / (hi - lo)).ceil() as usize).max(6) } else { total_n };
            // (Stations a hair inside a piece: one at its end vertex belongs to its own edge.)
            let nudge = if pieces.len() > 1 { 1e-9 * total } else { 0.0 };
            let mut secs = Vec::new();
            let mut starts = Vec::new();
            let mut convex = false;
            let count = if closed { n } else { n + 1 };
            for k in 0..count {
                // A closed loft starts a quarter of the way round: its own seam
                // on the edge's (the periodic faces' seam) made OCCT's boolean
                // with an elliptic boss's whole side come back empty.
                let f = if closed { (k as f64 / n as f64 + 0.25).fract() } else { k as f64 / n as f64 };
                let (p, fr, side1_first, tangent) = station((plo + (phi - plo) * f).clamp(plo + nudge, phi - nudge))?;
                let here = match held {
                    true => held_spec(spec, p, tangent, &fr, scale)?,
                    false => spec.clone(),
                };
                let (face, e3, cv) = section(p, &fr, &here, side1_first)?;
                convex = cv;
                // (Its corner: the edge's point, or out in the air for a chamfer as sections.)
                let corner = e3[0].edge_info()?.start;
                // A chamfer as sections runs straight on past a free end of its chain until it is off the part.
                let run_on = if spec.reference.is_some() && !closed && convex && ((k == 0 && pi == 0) || (k == n && pi == last_piece)) {
                    let out = if k == 0 { -tangent } else { tangent };
                    let e = 2.0 * spec.r1.max(spec.r2);
                    let probe = p + out * e + (fr.u[0] + fr.u[1]) * (0.1 * e);
                    (!tangent_continues(shape, p, out, usize::MAX) && shape.classify_point(probe, scale * 1e-9)? == k::PointClass::Outside).then_some(out * e)
                } else {
                    None
                };
                let past = match run_on {
                    Some(v) => Some((section(p + v, &fr, &here, side1_first)?.0, corner + v)),
                    None => None,
                };
                if k == 0 && let Some((f, q)) = &past {
                    secs.push(f.clone());
                    starts.push(*q);
                }
                secs.push(face);
                starts.push(corner);
                if k != 0 && let Some((f, q)) = past {
                    secs.push(f);
                    starts.push(q);
                }
            }
            jobs.push((pi, ci, secs, starts, convex));
        }
        // The lofts in parallel (c2-variable-fillet-faceblend's eight took 425 ms one after another); a round's bare (a
        // chamfer's sides as the Loft feature makes them: c2-hunting-knife's Chamfer 1 needs them).
        use rayon::prelude::*;
        let legacy = k::kernel_legacy();
        let chamfer = spec.reference.is_some();
        let lofts = jobs
            .par_iter()
            // Every section starts at its corner: a curvature section's B-spline wires twisted otherwise.
            .map(|(_, _, secs, starts, _)| {
                k::with_kernel_legacy(legacy.clone(), || match chamfer {
                    true => k::loft(secs, &k::LoftOptions { solid: true, closed, start_points: Some(starts.clone()), ..Default::default() }),
                    false => k::loft_tool(secs, closed, starts),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        for ((pi, ci, secs, _, convex), tool) in jobs.into_iter().zip(lofts) {
            if std::env::var_os("NOSHAPE_BLEND_DEBUG").is_some() {
                eprintln!("section blend: loft of {} sections (closed {closed}, chain {} edges): valid {} volume {:?} faces {}", secs.len(), chain.len(), tool.shape.is_valid(), tool.shape.volume(), tool.shape.count(ShapeType::Face));
            }
            // The face touching neither support is the blend face (a held blend's later pieces on one edge: its splits).
            let ename = body.edges[chain[ci].index].clone();
            let on_edge = pieces[..pi].iter().filter(|q| q.2 == ci).count();
            let blend = match on_edge {
                0 => Arc::new(TopoName::face(feature, Role::Blend(ename.clone()))),
                k => Arc::new(TopoName::face(feature, Role::Blend(Arc::new(TopoName::Split { parent: ename.clone(), by: feature, ordinal: k as u16 })))),
            };
            let generated = Arc::new(TopoName::face(feature, Role::Generated(ename)));
            let supports: Vec<&Shape> = chain.iter().flat_map(|c| ef.get(c.index).cloned().unwrap_or_default()).map(|f| &faces[f as usize]).collect();
            // (Only the end sections can be a cap.)
            let ends = [secs.first(), secs.last()]
                .into_iter()
                .flatten()
                .map(|s| if s.shape_type() == ShapeType::Face { Ok(s.clone()) } else { k::make_face_from_wire(s, true) })
                .collect::<Result<Vec<_>, _>>()?;
            // (A point on each face found once: per section it was ~1,000 projections in c2-variable-fillet-faceblend.)
            let names: Vec<Name> = tool
                .shape
                .faces()
                .iter()
                .map(|f| {
                    let Some(q) = super::helpers_a::point_on_face(f) else { return blend.clone() };
                    let on = |s: &Shape| s.contains_point(q, scale * 1e-6).unwrap_or(false);
                    if (f.is_planar() && ends.iter().any(on)) || supports.iter().any(|s| on(s)) { generated.clone() } else { blend.clone() }
                })
                .collect();
            let named = NamedShape::new(tool.shape, names);
            if convex {
                cuts.push(named);
                cut_edges.push(chain[ci].index);
            } else {
                adds.push(named);
            }
        }
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
        let side1_first = match &spec.reference {
            Some(r) => r.faces.contains(&(fs[0] as usize)),
            None => spec.sides.as_ref().is_none_or(|sd| sd.side1.contains(&(fs[0] as usize))),
        };
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
        if convex {
            cuts.push(named);
            cut_edges.push(chain[ci].index);
        } else {
            adds.push(named);
        }
    }
    }
    // Held blends (hold lines) built directly where they can be: the loft's sides only approximate a curved wall, and a
    // cut by them leaves slivers (c2-variable-fillet-faceblend's rounded ends).
    if (ball || ranges.iter().any(|r| r.2)) && adds.is_empty() && !cuts.is_empty() {
        let tools: Vec<Shape> = cuts.iter().map(|t| t.shape.clone()).collect();
        let rims = cut_edges.iter().map(|&e| shape.sub_shape(ShapeType::Edge, e)).collect::<Result<Vec<_>, _>>()?;
        match k::blend_direct(shape, &tools, &rims) {
            // (Checked valid by the build.)
            Ok(res) if res.shape.count(ShapeType::Solid) == 1 => {
                let names = body.shape_names();
                let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&|n: &Name| Role::Blend(n.clone())));
                let (shape, face_names) = if ball { conic_contacts(ev, shape, &chain.iter().map(|c| c.shape.clone()).collect::<Vec<_>>(), &conics, res.shape, face_names, spec.r1)? } else { (res.shape, face_names) };
                ev.replace_body(bi, shape, face_names);
                return Ok(());
            }
            other => {
                if std::env::var_os("NOSHAPE_BLEND_DEBUG").is_some() {
                    eprintln!("section blend: direct build declined: {:?}", other.map(|r| (r.shape.is_valid(), r.shape.count(ShapeType::Solid))).map_err(|e| e.to_string()));
                }
                // (A rolling ball's sides only approximate its curved faces: a boolean by them leaves slivers.)
                if ball {
                    return Err(fail("the rolling balls' direct build declined"));
                }
            }
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
    // A chamfer's sliver end (an edge under 0.02 mm it made): one vertex, as live's (c2-hunting-knife's Chamfer 1: the
    // bottom strip 0.011 mm wide where it meets the slope, live 2462's JZQ; 0.037 and 0.044 mm edges stay).
    if spec.reference.is_some() {
        const SLIVER: f64 = 2e-5;
        let mid = |x: &Shape| x.edge_info().ok().filter(|i| i.length < SLIVER).and_then(|i| x.point_at(0.5 * (i.first_param + i.last_param)).ok());
        let before: Vec<DVec3> = body.shape.edges().iter().filter_map(mid).collect();
        let made: Vec<DVec3> = shape.edges().iter().filter_map(mid).filter(|p| before.iter().all(|q| (*p - *q).length() > SLIVER)).collect();
        if !made.is_empty() {
            let collapsed = k::collapse_short_edges(&shape, &made, SLIVER)?;
            names = ev.name_result(&[&crate::regen::eval::names_for(&shape, names)], &collapsed.history, &collapsed.shape, &BTreeMap::new(), None);
            shape = collapsed.shape;
        }
    }
    if !shape.is_valid() || shape.count(ShapeType::Solid) != 1 {
        return Err(fail("the result is not one valid solid"));
    }
    ev.replace_body(bi, shape, names);
    Ok(())
}

/// A face a rolling ball touches; a plane or cylinder extended past the face's boundary (a ball running on past a
/// chain's free end).
#[derive(Clone)]
enum Support {
    Plane { o: DVec3, n: DVec3 },
    Cylinder { o: DVec3, a: DVec3, r: f64, s: f64 },
    Face(Shape, f64),
}

impl Support {
    /// `inward`: the normal into the material at `p` on the face.
    fn of(face: &Shape, p: DVec3, inward: DVec3) -> FResult<Support> {
        Ok(match face.face_surface()? {
            k::Surface::Plane(pl) => {
                let n = pl.unit_normal();
                Support::Plane { o: pl.origin, n: if n.dot(inward) < 0.0 { -n } else { n } }
            }
            k::Surface::Cylinder { origin, axis, radius, .. } => {
                let a = axis.normalize();
                let v = p - origin;
                let s = if (v - a * v.dot(a)).dot(inward) < 0.0 { -1.0 } else { 1.0 };
                Support::Cylinder { o: origin, a, r: radius, s }
            }
            _ => Support::Face(face.clone(), if face.normal_at_point(p)?.dot(inward) < 0.0 { -1.0 } else { 1.0 }),
        })
    }

    /// The point nearest `q` and the unit normal into the material there.
    fn foot(&self, q: DVec3) -> FResult<(DVec3, DVec3)> {
        Ok(match self {
            Support::Plane { o, n } => (q - *n * (q - *o).dot(*n), *n),
            Support::Cylinder { o, a, r, s } => {
                let v = q - *o;
                let along = *a * v.dot(*a);
                let radial = (v - along).normalize_or_zero();
                if radial == DVec3::ZERO {
                    return Err(fail("a ball centred on a cylinder's axis"));
                }
                (*o + along + radial * *r, radial * *s)
            }
            Support::Face(f, s) => {
                let pr = f.project_point(q)?;
                (pr.point, f.normal_at_uv(pr.u, pr.v)?.normalize() * *s)
            }
        })
    }
}

/// The ball of radius `r` inside the material touching both supports, its centre in the plane through `origin` across
/// `normal`: its centre and contacts.
fn rolling_ball(origin: DVec3, normal: DVec3, sup: &[Support; 2], r: f64, guess: DVec3) -> FResult<(DVec3, [DVec3; 2])> {
    let t = normal.normalize();
    let e1 = t.any_orthonormal_vector();
    let e2 = t.cross(e1);
    let mut c = guess - t * (guess - origin).dot(t);
    for _ in 0..60 {
        // (c - foot) · n = r on both, c = origin + a e1 + b e2 (each foot's tangent plane).
        let row = |(f, n): (DVec3, DVec3)| (n.dot(e1), n.dot(e2), r + (f - origin).dot(n));
        let ((a0, b0, r0), (a1, b1, r1)) = (row(sup[0].foot(c)?), row(sup[1].foot(c)?));
        let det = a0 * b1 - a1 * b0;
        if det.abs() < 1e-12 {
            return Err(fail("the faces at the edge are tangent"));
        }
        let next = origin + e1 * ((r0 * b1 - r1 * b0) / det) + e2 * ((a0 * r1 - a1 * r0) / det);
        let moved = (next - c).length();
        c = next;
        if moved < r * 1e-12 {
            return Ok((c, [sup[0].foot(c)?.0, sup[1].foot(c)?.0]));
        }
    }
    Err(fail("no rolling ball fits"))
}

/// A ball's section: the region between its arc (contact 0 to 1) and the edge, its sides on the supports (a line on a
/// plane, an arc through three points on a curved face) from the contacts to the edge's point in the section's plane
/// (the direct build's strips on the part's faces), as its wire (a loft's section; a face cost 0.8 ms). With its first
/// vertex (for the loft).
fn ball_section(c: DVec3, [t0, t1]: [DVec3; 2], sup: &[Support; 2], r: f64) -> FResult<(Shape, DVec3)> {
    let (v0, v1) = ((t0 - c).normalize(), (t1 - c).normalize());
    let half = v0.dot(v1).clamp(-1.0, 1.0).acos() / 2.0;
    if half < 1e-6 || half > std::f64::consts::FRAC_PI_2 - 1e-6 {
        return Err(fail("the faces at the edge are tangent"));
    }
    let n = v0.cross(v1).normalize();
    let in_plane = |x: DVec3| x - n * (x - c).dot(n);
    // The edge's point: on both supports, in the plane (Newton from the tangent lines' corner).
    let mut p = c + (v0 + v1).normalize() * (r / half.cos());
    for _ in 0..40 {
        let ((f0, n0), (f1, n1)) = (sup[0].foot(p)?, sup[1].foot(p)?);
        let (d0, d1) = ((p - f0).dot(n0), (p - f1).dot(n1));
        let (g0, g1) = (in_plane(c + n0) - c, in_plane(c + n1) - c);
        // Solve g0·x = -d0, g1·x = -d1 for x in the plane (spanned by g0, g1).
        let (a, b, e) = (g0.dot(g0), g0.dot(g1), g1.dot(g1));
        let det = a * e - b * b;
        if det.abs() < 1e-24 {
            return Err(fail("the faces at the edge are tangent"));
        }
        let (k0, k1) = ((-d0 * e + d1 * b) / det, (-d1 * a + d0 * b) / det);
        let step = g0 * k0 + g1 * k1;
        p += step;
        if step.length() < r * 1e-13 {
            break;
        }
    }
    let side = |s: &Support, from: DVec3, to: DVec3| -> FResult<Shape> {
        if matches!(s, Support::Plane { .. }) {
            return Ok(k::make_line(from, to)?);
        }
        let mut m = (from + to) / 2.0;
        for _ in 0..20 {
            m = in_plane(s.foot(m)?.0);
        }
        // (A cylinder cut along its axis: straight.)
        if (m - (from + to) / 2.0).length() < (to - from).length() * 1e-5 {
            return Ok(k::make_line(from, to)?);
        }
        Ok(k::make_arc_3pts(from, m, to)?)
    };
    let arc = k::make_arc_3pts(t0, c + (v0 + v1).normalize() * r, t1)?;
    Ok((k::make_wire(&[side(&sup[0], p, t0)?, arc, side(&sup[1], t1, p)?])?, p))
}

/// A shared ball's section with its contacts in the order of `own` (an edge's own contacts there).
fn ball_section_as(shared: &(DVec3, [DVec3; 2]), own: [DVec3; 2], sup: &[Support; 2], r: f64) -> FResult<(Shape, DVec3)> {
    let (c, [a, b]) = *shared;
    let swap = (a - own[1]).length() + (b - own[0]).length() < (a - own[0]).length() + (b - own[1]).length();
    ball_section(c, if swap { [b, a] } else { [a, b] }, sup, r)
}

/// One chain edge's rolling ball: its supports, the normals into the material at its middle, and whether it is a line
/// between planes (its round a cylinder: one section extruded).
struct BallEdge {
    faces: [usize; 2],
    sup: [Support; 2],
    inward: DVec3,
    straight: bool,
}

/// Whether an arc about `axis` through `center` runs between a plane square to the axis and a cylinder about it.
fn coaxial_supports(sup: &[Support; 2], center: DVec3, axis: DVec3, scale: f64) -> bool {
    let square = |s: &Support| matches!(s, Support::Plane { n, .. } if n.dot(axis).abs() > 1.0 - 1e-9);
    let about = |s: &Support| match s {
        Support::Cylinder { o, a, .. } => a.dot(axis).abs() > 1.0 - 1e-9 && (center - *o - *a * (center - *o).dot(*a)).length() < scale * 1e-9,
        _ => false,
    };
    (square(&sup[0]) && about(&sup[1])) || (about(&sup[0]) && square(&sup[1]))
}

fn ball_edge(c: &ChainEdge, faces: [usize; 2], ff: [&Shape; 2], scale: f64) -> FResult<BallEdge> {
    let info = c.shape.edge_info()?;
    // Each face's own normal reversed into the material (no classifier: c2-hunting-knife's Fillet 3 classified 70
    // points ~30 ms), and the side of the edge it lies on.
    let t = info.first_param + (info.last_param - info.first_param) * 0.5;
    let (p, tangent) = (c.shape.point_at(t)?, c.shape.tangent_at(t)?.normalize());
    let step = (info.length * 1e-3).max(scale * 1e-7);
    let mut m = [DVec3::ZERO; 2];
    let mut u = [DVec3::ZERO; 2];
    for k in 0..2 {
        let Some((uk, mk)) = into_face(ff[k], p, tangent, step)? else {
            return Err(fail("could not orient a face at the edge"));
        };
        (u[k], m[k]) = (uk, mk);
    }
    if u[0].dot(m[1]) <= 0.0 {
        return Err(fail("a concave edge's rolling ball"));
    }
    let sup = [Support::of(ff[0], p, m[0])?, Support::of(ff[1], p, m[1])?];
    let straight = matches!(info.curve, k::Curve::Line { .. }) && sup.iter().all(|s| matches!(s, Support::Plane { .. }));
    Ok(BallEdge { faces, sup, inward: (m[0] + m[1]).normalize(), straight })
}

/// The direction into face `f` from a point `p` of its edge across `tangent`, and the face's normal into the material;
/// None where neither side is on the face.
fn into_face(f: &Shape, p: DVec3, tangent: DVec3, step: f64) -> FResult<Option<(DVec3, DVec3)>> {
    let m = -f.normal_at_point(p)?.normalize();
    let dir = m.cross(tangent).normalize();
    let on = |v: DVec3| f.project_point(p + v * step).is_ok_and(|q| q.distance < step * 1e-2);
    Ok(match (on(dir), on(-dir)) {
        (true, false) => Some((dir, m)),
        (false, true) => Some((-dir, m)),
        _ => None,
    })
}

/// The ball where two chain edges meet whose faces other than a common one are tangent along an edge there (a line's
/// plane running on into a round): the ball touching the common face and that edge, the boundary between the two
/// edges' rounds (live's circle KjpB between c2-hunting-knife's spine round and corner face). None otherwise.
fn tangency_ball(shape: &Shape, ef: &[Vec<u32>], [a, b]: [&BallEdge; 2], q: DVec3, r: f64, scale: f64) -> FResult<Option<(DVec3, [DVec3; 2])>> {
    let common: Vec<usize> = a.faces.iter().copied().filter(|f| b.faces.contains(f)).collect();
    let [s] = common[..] else { return Ok(None) };
    let (ks, kp) = if a.faces[0] == s { (0, 1) } else { (1, 0) };
    let p = a.faces[kp];
    let Some(qf) = b.faces.iter().copied().find(|f| *f != s) else { return Ok(None) };
    // Their common edge through the vertex.
    let found = ef.iter().enumerate().find_map(|(e, fs)| {
        let pair = fs.len() == 2 && ((fs[0] as usize == p && fs[1] as usize == qf) || (fs[0] as usize == qf && fs[1] as usize == p));
        let edge = shape.sub_shape(ShapeType::Edge, e).ok().filter(|_| pair)?;
        let info = edge.edge_info().ok()?;
        let t = if (info.start - q).length() < scale * 1e-9 || (info.end - q).length() < scale * 1e-9 {
            if (edge.point_at(info.first_param).ok()? - q).length() < scale * 1e-9 { info.first_param } else { info.last_param }
        } else {
            return None;
        };
        Some((edge, t, (info.last_param - info.first_param).abs()))
    });
    let Some((edge, mut t, span)) = found else { return Ok(None) };
    let ball_at = |t: f64| -> FResult<(f64, DVec3, DVec3)> {
        let g = edge.point_at(t)?;
        let c = g + a.sup[kp].foot(g)?.1 * r;
        let (f, n) = a.sup[ks].foot(c)?;
        Ok(((c - f).dot(n) - r, c, g))
    };
    let h = span * 1e-6;
    for _ in 0..30 {
        let (f, c, g) = ball_at(t)?;
        if f.abs() < r * 1e-12 {
            let touch = a.sup[ks].foot(c)?.0;
            return Ok(Some((c, if ks == 0 { [touch, g] } else { [g, touch] })));
        }
        let slope = (ball_at(t + h)?.0 - f) / h;
        if slope.abs() < 1e-300 {
            return Ok(None);
        }
        t -= f / slope;
    }
    Ok(None)
}

/// The cutting tools of a convex chain's rolling-ball round, one per edge: a line between planes one section extruded,
/// other edges a loft of exact ball sections (more where the edge turns). Two edges meeting share one section there
/// (the straight edge's), so their tools meet exactly; at a free end the ball rolls on along its supports until it is
/// off the part.
fn ball_tools(shape: &Shape, chain: &[ChainEdge], ef: &[Vec<u32>], faces: &[Shape], r: f64, scale: f64) -> FResult<(Vec<Shape>, Vec<k::Conic>)> {
    let near = |a: DVec3, b: DVec3| (a - b).length() < scale * 1e-9;
    let edges = chain
        .iter()
        .map(|c| {
            let fs = ef.get(c.index).filter(|f| f.len() == 2).ok_or_else(|| fail("the edge doesn't have two faces"))?;
            ball_edge(c, [fs[0] as usize, fs[1] as usize], [&faces[fs[0] as usize], &faces[fs[1] as usize]], scale)
        })
        .collect::<FResult<Vec<_>>>()?;
    // The ball at a chain vertex shared by two edges; None at a free end.
    let junction = |q: DVec3| -> FResult<Option<(DVec3, [DVec3; 2])>> {
        let at: Vec<(usize, bool)> = chain.iter().enumerate().filter_map(|(i, c)| if near(c.start, q) { Some((i, true)) } else if near(c.end, q) { Some((i, false)) } else { None }).collect();
        if at.len() != 2 {
            return Ok(None);
        }
        let (i, is_start) = at.iter().copied().find(|(i, _)| edges[*i].straight).unwrap_or(at[0]);
        let j = if at[0].0 == i { at[1].0 } else { at[0].0 };
        if let Some(b) = tangency_ball(shape, ef, [&edges[i], &edges[j]], q, r, scale)? {
            return Ok(Some(b));
        }
        Ok(Some(rolling_ball(q, tangent_dir(&chain[i], is_start)?, &edges[i].sup, r, q + edges[i].inward * r)?))
    };
    // A section off the part: its corners and their middle outside.
    let pbox = shape.bounding_box(false)?;
    let off = |wire: &Shape| -> FResult<bool> {
        let out = |p: DVec3| !(pbox.min.cmple(p).all() && p.cmple(pbox.max).all()) || shape.classify_point(p, scale * 1e-9).is_ok_and(|k| k == k::PointClass::Outside);
        let corners: Vec<DVec3> = wire.vertices().iter().filter_map(|v| v.vertex_point().ok()).collect();
        Ok(!corners.is_empty() && corners.iter().all(|p| out(*p)) && out(corners.iter().sum::<DVec3>() / corners.len() as f64))
    };
    // Each edge's tool on its own (in parallel: c2-hunting-knife's six lofts took 200 ms one after another).
    let build = |c: &ChainEdge, e: &BallEdge| -> FResult<Shape> {
        let ends = [junction(c.start)?, junction(c.end)?];
        let outs = [-tangent_dir(c, true)?, tangent_dir(c, false)?];
        let rolls_on = |end: usize| ends[end].is_none() && !tangent_continues(shape, if end == 0 { c.start } else { c.end }, outs[end], c.index);
        if e.straight {
            let d = (c.end - c.start).normalize();
            let ball = |q: DVec3| rolling_ball(q, d, &e.sup, r, q + e.inward * r);
            let section = |q: DVec3| -> FResult<(Shape, DVec3)> {
                let (c, t) = ball(q)?;
                ball_section(c, t, &e.sup, r)
            };
            // Run on past a free end until the section is off the part.
            let mut ext = [0.0f64; 2];
            for end in 0..2 {
                if !rolls_on(end) {
                    continue;
                }
                let q = if end == 0 { c.start } else { c.end };
                for k in 1..=12 {
                    ext[end] = 0.5 * r * k as f64;
                    if off(&section(q + outs[end] * ext[end])?.0)? {
                        break;
                    }
                }
            }
            // From the ball at its start to the one at its end (a junction's ball off the vertex's plane).
            let s0 = match &ends[0] {
                Some(j) => (j.0 - c.start).dot(d),
                None => -ext[0],
            };
            let s1 = match &ends[1] {
                Some(j) => (j.0 - c.start).dot(d),
                None => (c.end - c.start).dot(d) + ext[1],
            };
            let (first, _) = match &ends[0] {
                Some(j) => ball_section_as(j, ball(c.start + d * s0)?.1, &e.sup, r)?,
                None => section(c.start + d * s0)?,
            };
            return Ok(k::extrude(&k::make_face_from_wire(&first, true)?, d * (s1 - s0))?.shape);
        }
        let info = c.shape.edge_info()?;
        // An arc between a plane square to its axis and a cylinder about it: the balls' centres run round that axis, the
        // round a torus (live's; a loft only approximates it): the section revolved.
        if let k::Curve::Circle { center, axis, .. } = info.curve
            && coaxial_supports(&e.sup, center, axis.normalize(), scale)
        {
            let t0 = tangent_dir(c, true)?;
            let a = {
                let a = axis.normalize();
                if a.dot((c.start - center).cross(t0)) < 0.0 { -a } else { a }
            };
            let closed = near(c.start, c.end);
            // The point and travel direction `phi` round the axis from the edge's start.
            let at = |phi: f64| {
                let q = rotate(c.start, center, a, phi);
                (q, a.cross(q - center).normalize())
            };
            let ball = |phi: f64| {
                let (q, t) = at(phi);
                rolling_ball(q, t, &e.sup, r, q + e.inward * r)
            };
            let angle_of = |v: DVec3| {
                let v0 = c.start - center;
                let (x, y) = (v0 - a * v0.dot(a), v - center - a * (v - center).dot(a));
                a.dot(x.cross(y)).atan2(x.dot(y))
            };
            let span = if closed { std::f64::consts::TAU } else { angle_of(c.end).rem_euclid(std::f64::consts::TAU) };
            // Run on past a free end until the section is off the part.
            let (centre0, _) = ball(0.0)?;
            let reach = ((centre0 - center) - a * (centre0 - center).dot(a)).length().max(r);
            let mut ext = [0.0f64; 2];
            for (end, e_end) in ext.iter_mut().enumerate() {
                if closed || !rolls_on(end) {
                    continue;
                }
                for k in 1..=12 {
                    *e_end = 0.5 * r * k as f64 / reach;
                    let phi = if end == 0 { -*e_end } else { span + *e_end };
                    let (cb, tb) = ball(phi)?;
                    if off(&ball_section(cb, tb, &e.sup, r)?.0)? {
                        break;
                    }
                }
            }
            // From the ball at its start to the one at its end (a junction's ball off the vertex's plane).
            let phi0 = match &ends[0] {
                Some(j) => angle_of(j.0),
                None => -ext[0],
            };
            let phi1 = match &ends[1] {
                Some(j) => {
                    let p = angle_of(j.0);
                    if p < phi0 + 1e-9 { p + std::f64::consts::TAU } else { p }
                }
                None => span + ext[1],
            };
            let (first, _) = match &ends[0] {
                Some(j) => ball_section_as(j, ball(phi0)?.1, &e.sup, r)?,
                None => {
                    let (cb, tb) = ball(phi0)?;
                    ball_section(cb, tb, &e.sup, r)?
                }
            };
            let tool = k::revolve(&k::make_face_from_wire(&first, true)?, center, a, if closed { std::f64::consts::TAU } else { phi1 - phi0 })?.shape;
            if std::env::var_os("NOSHAPE_BLEND_DEBUG").is_some() {
                eprintln!("ball torus: {:.4} rad from {:.4}: valid {} volume {:?}", phi1 - phi0, phi0, tool.is_valid(), tool.volume());
            }
            return Ok(tool);
        }
        let at = |f: f64| -> FResult<(DVec3, DVec3)> {
            let t = info.first_param + (info.last_param - info.first_param) * if c.forward { f } else { 1.0 - f };
            Ok((c.shape.point_at(t)?, c.shape.tangent_at(t)?.normalize()))
        };
        let mut turn = 0.0;
        let mut prev = at(0.0)?.1;
        for i in 1..=32 {
            let t = at(i as f64 / 32.0)?.1;
            turn += prev.dot(t).clamp(-1.0, 1.0).acos();
            prev = t;
        }
        let closed = near(c.start, c.end);
        // (A section per 0.1 rad of turn: the knife's R3.5 tip within 1e-6 mm of its balls.)
        let n = ((turn / 0.1).ceil() as usize).clamp(8, 240);
        let (mut secs, mut starts, mut centres) = (Vec::new(), Vec::new(), Vec::new());
        let mut guess = at(0.0)?.0 + e.inward * r;
        for k in 0..if closed { n } else { n + 1 } {
            let f = if closed { (k as f64 / n as f64 + 0.25).fract() } else { k as f64 / n as f64 };
            let (p, t) = at(f)?;
            let (centre, touch) = rolling_ball(p, t, &e.sup, r, guess)?;
            guess = centre;
            centres.push(centre);
            let shared = if closed { None } else if k == 0 { ends[0].as_ref() } else if k == n { ends[1].as_ref() } else { None };
            let (face, start) = match shared {
                Some(j) => ball_section_as(j, touch, &e.sup, r)?,
                None => ball_section(centre, touch, &e.sup, r)?,
            };
            secs.push(face);
            starts.push(start);
        }
        // Past a free end the ball rolls on along the supports' surfaces, each station a step on from the last centre.
        let extends = e.sup.iter().all(|s| !matches!(s, Support::Face(..)));
        for (end, out) in outs.into_iter().enumerate() {
            if closed || !extends || !rolls_on(end) {
                continue;
            }
            let (mut last, mut dir) = (if end == 0 { centres[0] } else { centres[centres.len() - 1] }, out);
            let (mut more, mut more_starts) = (Vec::new(), Vec::new());
            for _ in 0..12 {
                let q = last + dir * (0.5 * r);
                let (centre, touch) = rolling_ball(q, dir, &e.sup, r, q)?;
                let (face, start) = ball_section(centre, touch, &e.sup, r)?;
                dir = (centre - last).normalize();
                last = centre;
                let done = off(&face)?;
                more.push(face);
                more_starts.push(start);
                if done {
                    break;
                }
            }
            if end == 0 {
                more.reverse();
                more_starts.reverse();
                secs.splice(0..0, more);
                starts.splice(0..0, more_starts);
            } else {
                secs.extend(more);
                starts.extend(more_starts);
            }
        }
        let tool = k::loft_tool(&secs, closed, &starts)?;
        if std::env::var_os("NOSHAPE_BLEND_DEBUG").is_some() {
            eprintln!("ball loft: {} sections ({n} on the edge, turning {turn:.3}): valid {} volume {:?}", secs.len(), tool.shape.is_valid(), tool.shape.volume());
        }
        Ok(tool.shape)
    };
    use rayon::prelude::*;
    let legacy = k::kernel_legacy();
    let tools = chain.par_iter().zip(edges.par_iter()).map(|(c, e)| k::with_kernel_legacy(legacy.clone(), || build(c, e))).collect::<FResult<Vec<_>>>()?;
    let conics = edges.iter().flat_map(|e| plane_cylinder_contacts(&e.sup, r)).collect();
    Ok((tools, conics))
}

/// The contact curves of a ball of radius `r` rolling between a plane and a cylinder: the plane offset by `r` meets the
/// cylinder offset by `r` along the ball's centres, an ellipse; its contact with the plane is that ellipse moved onto
/// the plane, its contact with the cylinder (radially in) the cylinder's section by a plane through the same axis
/// point, tilted R'/R as much (live c2-hunting-knife's Jjt: 3.015587 x 3). None where the plane runs along the axis.
fn plane_cylinder_contacts(sup: &[Support; 2], r: f64) -> Vec<k::Conic> {
    let (plane, cyl) = match sup {
        [p @ Support::Plane { .. }, c @ Support::Cylinder { .. }] | [c @ Support::Cylinder { .. }, p @ Support::Plane { .. }] => (p, c),
        _ => return Vec::new(),
    };
    let (Support::Plane { o: po, n }, Support::Cylinder { o: co, a, r: rc, s }) = (plane, cyl) else { return Vec::new() };
    let na = n.dot(*a);
    if na.abs() < 1e-9 {
        return Vec::new();
    }
    let spine = rc + s * r;
    let o = *co + *a * ((*po + *n * r - *co).dot(*n) / na);
    let ellipse = |centre: DVec3, normal: DVec3, minor: f64| {
        let normal = normal.normalize();
        let along = *a - normal * a.dot(normal);
        let major_dir = if along.length() < 1e-12 { normal.any_orthonormal_vector() } else { along.normalize() };
        k::Conic { centre, normal, major_dir, major: minor / a.dot(normal).abs(), minor }
    };
    let tilted = (*n - *a * na) * (spine / rc) + *a * na;
    vec![ellipse(o - *n * r, *n, spine), ellipse(o, tilted, *rc)]
}

/// A rolling ball's new edges on the result rebuilt as the conics they are (`conics`, its contacts with planes and
/// cylinders: live's contact edges of c2-hunting-knife's Fillet 3), within 0.004 r.
fn conic_contacts(ev: &EvalCtx, before: &Shape, chain: &[Shape], conics: &[k::Conic], shape: Shape, names: Vec<Name>, r: f64) -> FResult<(Shape, Vec<Name>)> {
    let old = before.edges();
    let near_chain = |p: DVec3| k::make_vertex(p).is_ok_and(|v| chain.iter().any(|c| k::distance(&v, c).is_ok_and(|d| d.distance < 4.0 * r)));
    let mids: Vec<DVec3> = shape
        .edges()
        .iter()
        .filter(|e| !old.iter().any(|o| o.is_same(e)))
        .filter_map(|e| {
            let i = e.edge_info().ok()?;
            e.point_at(0.5 * (i.first_param + i.last_param)).ok()
        })
        .filter(|p| near_chain(*p))
        .collect();
    if mids.is_empty() || conics.is_empty() {
        return Ok((shape, names));
    }
    let res = k::canonical_edges(&shape, &mids, conics, 0.004 * r)?;
    let names = ev.name_result(&[&crate::regen::eval::names_for(&shape, names)], &res.history, &res.shape, &BTreeMap::new(), None);
    Ok((res.shape, names))
}

/// The radius a hold line sets at a station: the circle touching the face the hold line is on at its crossing of the
/// section plane (through `p` across `tangent`) and the other face. None where no hold line crosses the section.
fn hold_radius(hold: &[HoldLine], p: DVec3, tangent: DVec3, f: &Frame, scale: f64) -> FResult<Option<f64>> {
    Ok(hold_of(hold, p, tangent, f, scale)?.map(|(r, _)| r))
}

/// [`hold_radius`] and the hold line's edge (its index in `hold`) that sets it.
fn hold_of(hold: &[HoldLine], p: DVec3, tangent: DVec3, f: &Frame, scale: f64) -> FResult<Option<(f64, usize)>> {
    let mut best: Option<(f64, usize)> = None;
    for (ei, h) in hold.iter().enumerate() {
        let (e, (t0, t1)) = (&h.edge, h.t);
        let g = |t: f64| -> FResult<f64> { Ok((e.point_at(t)? - p).dot(tangent)) };
        let n = HoldLine::N;
        let gs: Vec<f64> = h.pts.iter().map(|q| (*q - p).dot(tangent)).collect();
        // Where the section's plane crosses it, and an end on the plane (a hold line ending where the chain's edges
        // meet: the sign test misses an end at 0 to rounding).
        let mut at: Vec<f64> = [(t0, gs[0]), (t1, gs[n])].into_iter().filter(|(_, v)| v.abs() <= scale * 1e-9).map(|(t, _)| t).collect();
        let mut prev = (t0, gs[0]);
        for (i, &gi) in gs.iter().enumerate().skip(1) {
            let cur = (t0 + (t1 - t0) * i as f64 / n as f64, gi);
            if (prev.1 <= 0.0) != (cur.1 <= 0.0) || cur.1 == 0.0 {
                // (Illinois: regula falsi halving the end that stays, ~6 evaluations to a point on the plane.)
                let (mut a, mut b, mut kept) = (prev, cur, 0i8);
                let mut root = if prev.1.abs() < cur.1.abs() { prev.0 } else { cur.0 };
                for _ in 0..60 {
                    if b.1 == a.1 {
                        break;
                    }
                    let m = (a.0 * b.1 - b.0 * a.1) / (b.1 - a.1);
                    if !(m > a.0.min(b.0) && m < a.0.max(b.0)) || (b.0 - a.0).abs() <= (t1 - t0).abs() * 1e-15 {
                        break;
                    }
                    let gm = g(m)?;
                    root = m;
                    if (gm <= 0.0) == (a.1 <= 0.0) {
                        a = (m, gm);
                        if kept == 1 {
                            b.1 *= 0.5;
                        }
                        kept = 1;
                    } else {
                        b = (m, gm);
                        if kept == -1 {
                            a.1 *= 0.5;
                        }
                        kept = -1;
                    }
                    if gm.abs() <= scale * 1e-15 {
                        break;
                    }
                }
                at.push(root);
            }
            prev = cur;
        }
        for t in at {
            let v = e.point_at(t)? - p;
            for u in f.u {
                let s = v.dot(u);
                if s > 0.0 && (v - u * s).length() < scale * 1e-6 && best.is_none_or(|b| s < b.0) {
                    best = Some((s, ei));
                }
            }
        }
    }
    // Equal offsets d from both sides touch each at (d + d·cos θ) / sin θ from the edge (see [`Sides`]).
    let c = f.u[0].dot(f.u[1]).clamp(-1.0, 1.0);
    Ok(best.map(|(s, ei)| (s * (1.0 - c * c).sqrt() / (1.0 + c), ei)))
}

/// A held station's spec: the radius the hold line sets, capped by the entered one.
fn held_spec(spec: &BlendSpec, p: DVec3, tangent: DVec3, f: &Frame, scale: f64) -> FResult<BlendSpec> {
    let hold = spec.sides.as_ref().map(|s| s.hold.as_slice()).unwrap_or(&[]);
    let r = hold_radius(hold, p, tangent, f, scale)?.map_or(spec.r1, |h| h.min(spec.r1));
    Ok(BlendSpec { r1: r, r2: r, ..spec.clone() })
}

type Station<'a> = dyn Fn(f64) -> FResult<(DVec3, Frame, bool, DVec3)> + 'a;

/// A face's surface normal at `p` on it, either way round: a cylinder's straight from its axis (projecting onto the face
/// took ~0.3 ms, 800 times in c2-variable-fillet-faceblend's held Face blend 1), others projected.
fn surface_normal(face: &Shape, p: DVec3) -> FResult<DVec3> {
    if let Ok(k::Surface::Cylinder { origin, axis, .. }) = face.face_surface() {
        let v = p - origin;
        if let Some(n) = (v - axis * v.dot(axis)).try_normalize() {
            return Ok(n);
        }
    }
    Ok(face.normal_at_point(p)?.normalize())
}

/// A held range [lo, hi] of the chain in pieces (from, to, chain edge): split at the chain's vertices and where the
/// hold line's edge that sets the radius changes.
fn held_pieces(spec: &BlendSpec, chain: &[ChainEdge], lo: f64, hi: f64, scale: f64, station: &Station) -> FResult<Vec<(f64, f64, usize)>> {
    let hold = spec.sides.as_ref().map(|s| s.hold.as_slice()).unwrap_or(&[]);
    let which = |s: f64| -> FResult<Option<usize>> {
        let (p, fr, _, tangent) = station(s)?;
        Ok(hold_of(hold, p, tangent, &fr, scale)?.map(|h| h.1))
    };
    let span = hi - lo;
    let mut at: Vec<f64> = Vec::new();
    let mut acc = 0.0;
    for c in chain {
        acc += c.length;
        if acc > lo + 1e-9 * span && acc < hi - 1e-9 * span {
            at.push(acc);
        }
    }
    const M: usize = 64;
    let mut prev = (lo, which(lo)?);
    for i in 1..=M {
        let s = lo + span * i as f64 / M as f64;
        let cur = (s, which(s)?);
        if cur.1 != prev.1 {
            let (mut a, mut b) = (prev.0, cur.0);
            for _ in 0..30 {
                let m = 0.5 * (a + b);
                if which(m)? == prev.1 {
                    a = m;
                } else {
                    b = m;
                }
            }
            at.push(0.5 * (a + b));
        }
        prev = cur;
    }
    at.sort_by(f64::total_cmp);
    at.dedup_by(|b, a| (*b - *a).abs() < 1e-6 * span);
    let mut out = Vec::new();
    let mut from = lo;
    for to in at.into_iter().chain([hi]) {
        if to - from > 1e-9 * span {
            let mid = 0.5 * (from + to);
            let (mut acc, mut ci) = (0.0, chain.len() - 1);
            for (i, c) in chain.iter().enumerate() {
                if mid <= acc + c.length {
                    ci = i;
                    break;
                }
                acc += c.length;
            }
            out.push((from, to, ci));
        }
        from = to;
    }
    Ok(out)
}

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

    /// The angle of a whole rim's middle, where Parasolid's ellipse is halfway round from its start (its major axis's
    /// end opposite the cylinder's axis seen on the plane), the end toward that axis.
    fn whole_middle(&self) -> f64 {
        let (x, y) = self.frame();
        match (self.np - self.axis * self.axis.dot(self.np)).try_normalize() {
            Some(n) => {
                let v = -n * self.axis.dot(self.np).signum();
                v.dot(y).atan2(v.dot(x))
            }
            None => 0.0,
        }
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

/// A rim's blend: a round of a radius, a chamfer of equal distances, or an Offset chamfer of a distance and an angle or
/// two distances.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RimBlend {
    Round,
    Chamfer,
    /// An Offset chamfer measured on the cylinder (`on_wall`) or the plane, the other side an angle or a distance:
    /// constant offsets from both faces sized at the rim's middle (live c2-hunting-knife's Chamfer 2).
    Offset { on_wall: bool, other: OffsetOther },
}

/// An Offset chamfer's other side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OffsetOther {
    Angle(f64),
    Distance(f64),
}

/// The rims a blend's edges make ([`rims_of`]), found once for [`rims_overflow`] and [`ring_blend`].
pub struct Rims(Vec<(Rim, usize)>);

/// Whether every rim is all of its plane and cylinder's intersection (no arc of it).
pub fn rims_whole(rims: &Rims) -> bool {
    rims.0.iter().all(|(rim, _)| rim.arc.is_empty())
}

/// Whether an Offset chamfer's first distance runs down every rim's wall: live 2675 puts it on the cylinder, the face or
/// the edges picked alike (c2-hunting-knife's hole rims, c2-fan-blade-sweep's pin tops); `flipped(i)` (a rim, by its
/// first edge's index: a direction override on its edges, or Opposite direction) puts it on the plane. None where the
/// rims disagree.
pub fn rims_measure_on_wall(rims: &Rims, flipped: impl Fn(usize) -> bool) -> Option<bool> {
    let mut on: Option<bool> = None;
    for (_, i) in &rims.0 {
        let wall = !flipped(*i);
        if on.is_some_and(|o| o != wall) {
            return None;
        }
        on = Some(wall);
    }
    on
}

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
            // (Off the part's box is off the part, without a classification: each takes ~0.7 ms on rounds' faces,
            // ~1.4 on c2-rings-loft's ring after its Fillet 1.)
            let bb = shape.bounding_box(false)?;
            let pad = DVec3::splat(scale * 1e-6);
            const N: usize = 32;
            for k in 0..N {
                let a = k as f64 * TAU / N as f64;
                if rim.on_arc(a) {
                    continue;
                }
                let (q, radial) = rim.at(x, y, a);
                if q.cmpge(bb.min - pad).all() && q.cmple(bb.max + pad).all() && !off_part(shape, q, (rim.np + radial * 0.3).normalize(), scale)? {
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
                // r along each face, square to the rim (an Offset chamfer's sizes near enough for this check).
                RimBlend::Chamfer | RimBlend::Offset { .. } => {
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

/// Whether `q` is off the solid: the first face a ray from it along `dir` crosses is one into the material (or none);
/// classified where the ray's crossings aren't clear (one at its start, on a face's edge or tangent).
fn off_part(shape: &Shape, q: DVec3, dir: DVec3, scale: f64) -> FResult<bool> {
    if let Ok(hits) = shape.line_crossings(q, dir, 2.0 * scale)
        && hits.iter().all(|&(t, tr, on)| !on && tr != 0 && t > scale * 1e-7)
    {
        return Ok(hits.iter().min_by(|a, b| a.0.total_cmp(&b.0)).is_none_or(|h| h.1 == 1));
    }
    Ok(shape.classify_point(q, scale * 1e-9)? == k::PointClass::Outside)
}

/// Whether any rim's plane is tilted to its cylinder's axis (the faces' angle varies along it: an equal-distance
/// chamfer's setback then follows the angle, live 2441).
pub fn rims_tilted(rims: &Rims) -> bool {
    rims.0.iter().any(|(rim, _)| rim.axis.dot(rim.np).abs() < 1.0 - 1e-9)
}

/// Fillets and chamfers of convex closed edges where a plane meets a cylinder (or arcs of that whose rest is off the
/// body) whose blends run off the plane or into each other ([`rims_overflow`]), or where OCCT's chain fails: each rim
/// blended on its own all round (a round on a wedge of its plane and cylinder, OCCT's blend of a lone rim; a chamfer
/// as [`rim_chamfer_tool`]), the region between the rim and its blend cut away, so each blend ends where it meets
/// another face or blend, as live's
/// (c2-rings-loft's Fillet 1: a ring's two R2 rims meet at its narrow bottom and run out onto its outside; its Chamfer
/// 1: a notch's arc chamfered through the strip of plane beside it, the chamfer running on past the arc's ends until
/// it leaves the part).
/// Equal chamfers of straight convex edges between two planes, run past their edges' ends where OCCT's can't: each the
/// part cut by its chamfer's prism along the edge (the edge and its two offset lines on the faces) until it leaves the
/// part, as live's (c2-rings-loft's Chamfer 3: 10 mm on alternate 15 mm edges of a hexagonal prism, its top a
/// triangle). `Ok(false)` (nothing done) unless every edge is such, and each prism takes material only at its edge.
pub fn planar_chamfer_cut(ev: &mut EvalCtx, bi: usize, edges: &[usize], d: f64) -> FResult<bool> {
    use super::sheetmetal_util::{body_named, bool_named};
    let body = ev.ctx.bodies[bi].clone();
    let shape = &body.shape;
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let faces = shape.faces();
    let ef = shape.edge_faces()?;
    let mut tools = Vec::new();
    for &e in edges {
        let edge = shape.sub_shape(ShapeType::Edge, e)?;
        let info = edge.edge_info()?;
        let k::Curve::Line { .. } = info.curve else { return Ok(false) };
        let Some(fs) = ef.get(e).filter(|f| f.len() == 2) else { return Ok(false) };
        let ff = [&faces[fs[0] as usize], &faces[fs[1] as usize]];
        if !ff.iter().all(|f| f.is_planar()) {
            return Ok(false);
        }
        let (a, b) = (info.start, info.end);
        let t = (b - a).normalize();
        let fr = frame_at(shape, ff, a + (b - a) * 0.5, t, scale)?;
        // Convex: the faces' material on the chamfer's side of the edge.
        if fr.u[0].dot(fr.m[1]) <= 1e-9 || fr.u[1].dot(fr.m[0]) <= 1e-9 {
            return Ok(false);
        }
        let l = 2.0 * scale;
        let p0 = a - t * l;
        let (pa, pb) = (p0 + fr.u[0] * d, p0 + fr.u[1] * d);
        // Past the faces (out of the material) on the two sides along them: only the chamfer's plane cuts.
        let pts = [pa, pa - fr.m[0] * d, p0 - (fr.m[0] + fr.m[1]) * d, pb - fr.m[1] * d, pb];
        let lines: Vec<Shape> = (0..pts.len()).map(|i| k::make_line(pts[i], pts[(i + 1) % pts.len()])).collect::<Result<_, _>>()?;
        let section = k::make_face_from_wire(&k::make_wire(&lines)?, true)?;
        let tool = k::extrude(&section, t * ((b - a).length() + 2.0 * l))?.shape;
        // Only where its edge is: what it takes off the part, each piece at the edge.
        let taken = k::common(shape, &tool)?.shape;
        if taken.solids().iter().any(|piece| k::distance(piece, &edge).map_or(true, |dd| dd.distance > scale * 1e-7)) {
            return Ok(false);
        }
        let ename = body.edges[e].clone();
        let blend = Arc::new(TopoName::face(ev.feature, Role::Blend(ename.clone())));
        let generated = Arc::new(TopoName::face(ev.feature, Role::Generated(ename)));
        // Its chamfer face: the one through both offset lines.
        let names: Vec<Name> = tool
            .faces()
            .iter()
            .map(|f| if f.contains_point(pa + t * l, scale * 1e-9).unwrap_or(false) && f.contains_point(pb + t * l, scale * 1e-9).unwrap_or(false) { blend.clone() } else { generated.clone() })
            .collect();
        tools.push(NamedShape::new(tool, names));
    }
    if tools.is_empty() {
        return Ok(false);
    }
    let (cut, names) = bool_named(ev, k::BooleanOp::Cut, &body_named(&body), &tools)?;
    let cut = NamedShape::new(cut, names);
    let un = k::unify_same_domain(&cut.shape)?;
    if !un.shape.is_valid() || un.shape.count(ShapeType::Solid) != 1 {
        return Ok(false);
    }
    let names = ev.name_result(&[&cut.names], &un.history, &un.shape, &BTreeMap::new(), None);
    ev.replace_body(bi, un.shape, names);
    Ok(true)
}

pub fn ring_blend(ev: &mut EvalCtx, bi: usize, rims: &Rims, edges: &[usize], sizes: &[f64], kind: RimBlend) -> FResult<()> {
    use super::sheetmetal_util::{body_named, bool_named};
    let body = ev.ctx.bodies[bi].clone();
    let shape = &body.shape;
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let feature = ev.feature;
    // Each rim's tool on its own (in parallel: OCCT's round of its wedge and two Booleans each).
    let tool_of = |(rim, i): &(Rim, usize)| -> FResult<NamedShape> {
        let i = *i;
        let r = sizes.get(i).or(sizes.first()).copied().ok_or_else(|| fail("no size"))?;
        let e = edges[i];
        let ename = body.edges[e].clone();
        let blend = Arc::new(TopoName::face(feature, Role::Blend(ename.clone())));
        let generated = Arc::new(TopoName::face(feature, Role::Generated(ename.clone())));
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
        let tool = match kind {
            RimBlend::Round => k::cut(&wedge, &k::fillet(&wedge, std::slice::from_ref(&edge), r)?.shape)?.shape,
            // A chamfer: the chord of one rolling ball all round (live 2384 / 2441); on a rim square to its plane that is
            // OCCT's chamfer (the setback the same all round), faster.
            RimBlend::Chamfer if rim.axis.dot(rim.np).abs() >= 1.0 - 1e-9 => k::cut(&wedge, &k::chamfer(&wedge, std::slice::from_ref(&edge), k::ChamferSpec::Equal(r), &[])?.shape)?.shape,
            RimBlend::Chamfer | RimBlend::Offset { .. } => rim_chamfer_tool(rim, &shape.sub_shape(ShapeType::Edge, e)?, r, kind, &wedge, seam_where_whole(rim, r)?, scale)?,
        };
        if !tool.is_valid() || tool.count(ShapeType::Solid) != 1 {
            return Err(fail("the rim's blend left no one solid to cut"));
        }
        // Its blend: the face on neither the plane nor the cylinder.
        let names: Vec<Name> = tool
            .faces()
            .iter()
            .map(|f| if matches!(f.face_surface(), Ok(k::Surface::Plane(_) | k::Surface::Cylinder { .. })) { generated.clone() } else { blend.clone() })
            .collect();
        Ok(NamedShape::new(tool, names))
    };
    use rayon::prelude::*;
    let legacy = k::kernel_legacy();
    let cuts = rims.0.par_iter().map(|x| k::with_kernel_legacy(legacy.clone(), || tool_of(x))).collect::<FResult<Vec<_>>>()?;
    // A round or chamfer built directly where it can be (the tools' touch curves put on the part's faces, no cut:
    // c2-rings-loft's Fillet 1 375 -> ~95 ms, c2-hunting-knife's Chamfer 2 ~73 -> ~36 ms), else the cut.
    let tools: Vec<Shape> = cuts.iter().map(|c| c.shape.clone()).collect();
    let rim_edges = rims.0.iter().map(|(_, i)| shape.sub_shape(ShapeType::Edge, edges[*i])).collect::<Result<Vec<_>, _>>()?;
    if let Ok(res) = k::blend_direct(shape, &tools, &rim_edges)
        && res.shape.count(ShapeType::Solid) == 1
    {
        let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), Some(&super::detail::blend_role));
        let un = k::unify_same_domain(&res.shape)?;
        if un.shape.is_valid() && un.shape.count(ShapeType::Solid) == 1 {
            let names = ev.name_result(&[&crate::regen::eval::names_for(&res.shape, names)], &un.history, &un.shape, &BTreeMap::new(), None);
            ev.replace_body(bi, un.shape, names);
            return Ok(());
        }
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

/// Rounds of concave arcs where a plane meets a cylinder square to it, whose ends run on tangentially into convex edges
/// left out (the tangent chain stops where the convexity changes): as live's (asm-isaac-ball's Fillet 2, a tab's
/// underside rounded into the cylinder below it, the tab's corner rounds' bottom edges convex), the round runs along the
/// arc and the faces round the plane face run on along its normal to trim it (the corner rounds extended down to it).
/// Built as the region between the round and the corner, its section revolved about the cylinder's axis, within the
/// plane face swept along its normal, added to the body. `Ok(false)` (nothing done) unless every edge is such an arc and
/// the plane face's other neighbours run along its normal (planes along it, cylinders about it), so the sweep's sides are
/// them run on.
pub fn stopped_concave_arc_round(ev: &mut EvalCtx, bi: usize, edges: &[usize], r: f64) -> FResult<bool> {
    use super::sheetmetal_util::{body_named, bool_named};
    let body = ev.ctx.bodies[bi].clone();
    let shape = &body.shape;
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let tol = scale * 1e-9;
    let faces = shape.faces();
    let ef = shape.edge_faces()?;
    let feature = ev.feature;
    let mut adds = Vec::new();
    for &e in edges {
        let Some(fs) = ef.get(e).filter(|f| f.len() == 2) else { return Ok(false) };
        let (pi, ci) = match (faces[fs[0] as usize].face_surface()?, faces[fs[1] as usize].face_surface()?) {
            (k::Surface::Plane(_), k::Surface::Cylinder { .. }) => (fs[0] as usize, fs[1] as usize),
            (k::Surface::Cylinder { .. }, k::Surface::Plane(_)) => (fs[1] as usize, fs[0] as usize),
            _ => return Ok(false),
        };
        let (plane, wall) = (&faces[pi], &faces[ci]);
        let k::Surface::Cylinder { origin, axis, radius: rc, .. } = wall.face_surface()? else { return Ok(false) };
        let a = axis.normalize();
        let edge = shape.sub_shape(ShapeType::Edge, e)?;
        let info = edge.edge_info()?;
        if info.closed || !matches!(info.curve, k::Curve::Circle { .. }) {
            return Ok(false);
        }
        let mid = edge.point_at(0.5 * (info.first_param + info.last_param))?;
        // The plane's outward normal along the axis.
        let np = plane.normal_at_point(mid)?.normalize();
        if np.dot(a).abs() < 1.0 - 1e-9 {
            return Ok(false);
        }
        // The cylinder's outward normal there: away from the axis (a boss) or towards it (a hole).
        let foot = origin + a * (mid - origin).dot(a);
        let radial = (mid - foot).normalize();
        let s = if wall.normal_at_point(mid)?.dot(radial) > 0.0 { 1.0 } else { -1.0 };
        // Concave: just off the edge on both faces' outer sides is outside the body.
        let probe = mid + radial * (s * r * 0.25) + np * (r * 0.25);
        if !matches!(shape.classify_point(probe, tol * 1e3), Ok(k::PointClass::Outside)) {
            return Ok(false);
        }
        // The plane face's other neighbours run along its normal.
        for oe in plane.edges() {
            let Some(oi) = shape.index_of(&oe, ShapeType::Edge) else { return Ok(false) };
            if edges.contains(&oi) {
                continue;
            }
            let Some(of) = ef.get(oi).and_then(|f| f.iter().map(|&x| x as usize).find(|&x| x != pi)) else { return Ok(false) };
            let along = match faces[of].face_surface()? {
                k::Surface::Plane(_) => faces[of].normal_at_point(oe.point_at(0.5 * (oe.edge_info()?.first_param + oe.edge_info()?.last_param))?)?.normalize().dot(np).abs() < 1e-9,
                k::Surface::Cylinder { axis: ax, .. } => ax.normalize().cross(np).length() < 1e-9,
                _ => false,
            };
            if !along {
                return Ok(false);
            }
        }
        // The section in the half-plane through the axis opposite the edge's middle (the revolution's seam away from the
        // round: one face): from the corner along the plane by r, the round back to the wall, the wall back to the
        // corner; revolved all round.
        let corner = foot - radial * rc + a * (mid - foot).dot(a);
        let corner = corner + np * (mid - corner).dot(np);
        let q = |rho: f64, h: f64| corner - radial * (s * rho) + np * h;
        let d = std::f64::consts::FRAC_1_SQRT_2;
        let wire = k::make_wire(&[k::make_line(q(0.0, 0.0), q(r, 0.0))?, k::make_arc_3pts(q(r, 0.0), q(r - r * d, r - r * d), q(0.0, r))?, k::make_line(q(0.0, r), q(0.0, 0.0))?])?;
        let ring = k::revolve(&k::make_face_from_wire(&wire, true)?, origin, a, std::f64::consts::TAU)?.shape;
        // (Swept back from the far side, its sides cylinders and planes as the faces they run on (axes the same way),
        // not swept surfaces: they unify with those faces.)
        let far = k::transform(plane, &k::Transform::Translate(np * (2.0 * r)), true)?.shape;
        let sweep = k::extrude(&far, -np * (2.0 * r))?.shape.with_elementary_surfaces()?;
        let tool = k::common(&ring, &sweep)?.shape;
        if !tool.is_valid() || tool.count(ShapeType::Solid) != 1 || tool.volume().unwrap_or(0.0) <= 0.0 {
            return Err(fail("the stopped arc's round left no one solid to add"));
        }
        let ename = body.edges[e].clone();
        let blend = Arc::new(TopoName::face(feature, Role::Blend(ename.clone())));
        let generated = Arc::new(TopoName::face(feature, Role::Generated(ename)));
        let names: Vec<Name> = tool.faces().iter().map(|f| if matches!(f.face_surface(), Ok(k::Surface::Torus { .. })) { blend.clone() } else { generated.clone() }).collect();
        adds.push(NamedShape::new(tool, names));
    }
    let (fused, names) = bool_named(ev, k::BooleanOp::Fuse, &body_named(&body), &adds)?;
    let fused = NamedShape::new(fused, names);
    let un = k::unify_same_domain(&fused.shape)?;
    if !un.shape.is_valid() || un.shape.count(ShapeType::Solid) != 1 {
        return Err(fail("the result is not one valid solid"));
    }
    let names = ev.name_result(&[&fused.names], &un.history, &un.shape, &BTreeMap::new(), None);
    // The corner rounds' straight tangent edges OCCT's fillet left B-splines, beside the lines their run on adds: as
    // lines they join them (one edge each, as live).
    if let Ok(straight) = k::straighten_edges(&un.shape, scale * 1e-7)
        && !straight.is_same(&un.shape)
        && straight.count(ShapeType::Face) == un.shape.count(ShapeType::Face)
        && let Ok(again) = k::unify_same_domain(&straight)
        && again.shape.is_valid()
        && again.shape.count(ShapeType::Solid) == 1
    {
        // (The straightened shape's faces in the unified one's order.)
        let before = crate::regen::eval::names_for(&straight, names.clone());
        let names = ev.name_result(&[&before], &again.history, &again.shape, &BTreeMap::new(), None);
        ev.replace_body(bi, again.shape, names);
        return Ok(true);
    }
    ev.replace_body(bi, un.shape, names);
    Ok(true)
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
    let (mut cut_edges, mut add_edges) = (Vec::new(), Vec::new());
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
        if convex {
            cuts.push(named);
            cut_edges.push(edge);
        } else {
            adds.push(named);
            add_edges.push(edge);
        }
    }
    // Built directly where they can be (the tools' touch circles put on the part's faces, no Boolean: cf-snap-hook's
    // Fillet 5, the bosses' rounds into the floor, its Boolean 50 of 64 ms).
    {
        let tools: Vec<Shape> = cuts.iter().chain(&adds).map(|t| t.shape.clone()).collect();
        let rims: Vec<Shape> = cut_edges.iter().chain(&add_edges).cloned().collect();
        if let Ok(res) = k::blend_direct(shape, &tools, &rims)
            && res.shape.count(ShapeType::Solid) == 1
        {
            let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), Some(&super::detail::blend_role));
            let un = k::unify_same_domain(&res.shape)?;
            if un.shape.is_valid() && un.shape.count(ShapeType::Solid) == 1 {
                let names = ev.name_result(&[&crate::regen::eval::names_for(&res.shape, names)], &un.history, &un.shape, &BTreeMap::new(), None);
                ev.replace_body(bi, un.shape, names);
                return Ok(true);
            }
        }
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

/// A round of a closed edge where a cylinder (a tube) meets another face (a wall), overflow off, whose ball would
/// touch the tube past its end: there the ball rolls over the tube's end rim instead, touching the wall and the rim
/// edge (live c2-impeller-loft's Fillet 1: R10 where its stern tube meets the housing, face JWO between the round and
/// the rim). Built as OCCT's round on the part with the tube run on past its end, less what lies between the two balls
/// that touch the tube exactly at the rim, and there a loft of the rim balls' sections. `Ok(false)` (nothing done)
/// unless the edge is such a loop and some ball has to roll over the rim.
pub fn cliff_end_round(ev: &mut EvalCtx, bi: usize, edges: &[usize], r: f64) -> FResult<bool> {
    let &[e] = edges else { return Ok(false) };
    let body = ev.ctx.bodies[bi].clone();
    let shape = &body.shape;
    let faces = shape.faces();
    let ef = shape.edge_faces()?;
    let edge = shape.sub_shape(ShapeType::Edge, e)?;
    if !edge.edge_info()?.closed {
        return Ok(false);
    }
    let Some(&[f0, f1]) = ef.get(e).map(|f| f.as_slice()) else { return Ok(false) };
    let (tube_i, wall_i) = match (faces[f0 as usize].face_surface()?, faces[f1 as usize].face_surface()?) {
        (k::Surface::Cylinder { .. }, k::Surface::Cylinder { .. }) => return Ok(false),
        (k::Surface::Cylinder { .. }, _) => (f0 as usize, f1 as usize),
        (_, k::Surface::Cylinder { .. }) => (f1 as usize, f0 as usize),
        _ => return Ok(false),
    };
    let k::Surface::Cylinder { axis, radius: rt, .. } = faces[tube_i].face_surface()? else { return Ok(false) };
    let axis = axis.normalize();
    let wall = faces[wall_i].clone();
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    // The tube's end rim: a whole circle of it between it and a plane across the axis.
    let rim = ef.iter().enumerate().find_map(|(i, fs)| {
        if i == e || !fs.contains(&(tube_i as u32)) || fs.len() != 2 {
            return None;
        }
        let cap = fs.iter().copied().find(|&f| f as usize != tube_i)? as usize;
        let k::Surface::Plane(pl) = faces[cap].face_surface().ok()? else { return None };
        let info = shape.sub_shape(ShapeType::Edge, i).ok()?.edge_info().ok()?;
        let k::Curve::Circle { center, .. } = info.curve else { return None };
        (pl.normal.normalize().dot(axis).abs() > 1.0 - 1e-9 && info.closed).then_some((center, cap))
    });
    let Some((rim_center, cap_i)) = rim else { return Ok(false) };
    // Out of the tube's end: the cap's outward normal.
    let out = if faces[cap_i].normal_at_point(rim_center)?.dot(axis) >= 0.0 { axis } else { -axis };
    let x_dir = axis.any_orthonormal_vector();
    let y_dir = axis.cross(x_dir);
    // The wall's nearest point and normal: Newton steps on its parameters from the last answer (the balls move a
    // little at a time), OCCT's projection where they don't settle (~1.7 ms on the impeller's 111-pole wall).
    let bounds = wall.uv_bounds()?;
    let hint = std::cell::Cell::new(None::<DVec2>);
    // (The wall read once as NURBS where it is a B-spline: its normal's side is the face's, checked at the first foot.)
    let nurbs = wall.face_nurbs()?;
    let side = std::cell::Cell::new(0.0f64);
    // [S, Su, Sv, Suu, Suv, Svv, the face's normal].
    let eval = |uv: DVec2| -> FResult<[DVec3; 7]> {
        match (&nurbs, side.get()) {
            (Some(ns), sg) if sg != 0.0 => {
                let [s0, su, sv, suu, suv, svv] = ns.derivs(uv.x, uv.y);
                Ok([s0, su, sv, suu, suv, svv, su.cross(sv) * sg])
            }
            _ => {
                let [su, sv, suu, suv, svv, n] = wall.d2_at_uvs(&[uv])?[0];
                Ok([wall.point_at_uv(uv.x, uv.y)?, su, sv, suu, suv, svv, n])
            }
        }
    };
    let foot_on = |p: DVec3, check: bool| -> FResult<(DVec3, DVec3)> {
        if let Some(mut uv) = hint.get() {
            for _ in 0..16 {
                let [s0, su, sv, suu, suv, svv, n] = eval(uv)?;
                let d = p - s0;
                let (a, b, c) = (su.dot(su) - d.dot(suu), su.dot(sv) - d.dot(suv), sv.dot(sv) - d.dot(svv));
                let det = a * c - b * b;
                if det.abs() < 1e-300 {
                    break;
                }
                let (r1, r2) = (su.dot(d), sv.dot(d));
                let step = DVec2::new((r1 * c - r2 * b) / det, (a * r2 - b * r1) / det);
                uv = DVec2::new((uv.x + step.x).clamp(bounds[0], bounds[1]), (uv.y + step.y).clamp(bounds[2], bounds[3]));
                if (su * step.x + sv * step.y).length() < scale * 1e-9 {
                    let s1 = eval(uv)?[0];
                    let n = n.normalize();
                    // (A foot inside the face's parameters, the offset along its normal.)
                    let along = (p - s1).cross(n).length() <= scale * 1e-9;
                    if along && (!check || wall.contains_uv(uv.x, uv.y, scale * 1e-7).unwrap_or(false)) {
                        hint.set(Some(uv));
                        return Ok((s1, n));
                    }
                    break;
                }
            }
        }
        let pr = wall.project_point(p)?;
        if side.get() == 0.0 && nurbs.is_some() {
            let [_, su, sv, ..] = nurbs.as_ref().map(|ns| ns.derivs(pr.u, pr.v)).unwrap_or([DVec3::ZERO; 6]);
            let n = wall.normal_at_uv(pr.u, pr.v)?;
            side.set(if su.cross(sv).dot(n) >= 0.0 { 1.0 } else { -1.0 });
        }
        hint.set(Some(DVec2::new(pr.u, pr.v)));
        Ok((pr.point, wall.normal_at_uv(pr.u, pr.v)?.normalize()))
    };
    // (On the face checked only for the touches: the solvers' steps may look a little past its edge.)
    let foot = |p: DVec3| foot_on(p, false);
    let touch = |p: DVec3| -> FResult<DVec3> { Ok(foot_on(p, true)?.0) };
    // Signed distance from the wall's surface: positive in the air.
    let sd = |p: DVec3| -> FResult<f64> {
        let (q, n) = foot(p)?;
        Ok((p - q).dot(n))
    };
    // The ball touching the rim at angle `a` and the wall: its tilt past the rim plane (0: it touches the tube at the
    // rim), centre, touch points on the rim and the wall. None where no such ball reaches the wall. (Secant steps: each
    // step projects onto the wall, ~ms on a dense spline.)
    let ball = |a: f64| -> FResult<Option<(f64, DVec3, DVec3, DVec3)>> {
        let u = x_dir * a.cos() + y_dir * a.sin();
        let kp = rim_center + u * rt;
        let f = |psi: f64| -> FResult<f64> { Ok(sd(kp + (u * psi.cos() + out * psi.sin()) * r)? - r) };
        let (mut x0, mut x1) = (0.0f64, 0.5f64);
        let (mut f0, mut f1) = (f(x0)?, f(x1)?);
        for _ in 0..30 {
            if (f1 - f0).abs() < 1e-300 {
                break;
            }
            let x2 = (x1 - f1 * (x1 - x0) / (f1 - f0)).clamp(-1.2, 1.5);
            (x0, f0) = (x1, f1);
            x1 = x2;
            f1 = f(x1)?;
            if f1.abs() < scale * 1e-10 {
                break;
            }
        }
        if !f1.is_finite() || f1.abs() > scale * 1e-7 {
            return Ok(None);
        }
        let c = kp + (u * x1.cos() + out * x1.sin()) * r;
        Ok(Some((x1, c, kp, touch(c)?)))
    };
    // The rim angles whose ball tilts past the rim plane: one run of them, its ends where the tilt is 0.
    const N: usize = 36;
    let step = std::f64::consts::TAU / N as f64;
    let tilts: Vec<Option<f64>> = (0..N).map(|k| ball(k as f64 * step).map(|b| b.map(|b| b.0))).collect::<FResult<_>>()?;
    let past = |k: usize| tilts[k % N].is_some_and(|t| t > 0.0);
    let Some(start) = (0..N).find(|&k| past(k) && !past(k + N - 1)) else { return Ok(false) };
    let len = (0..N).take_while(|&j| past(start + j)).count();
    if len >= N - 1 || (0..N).filter(|&k| past(k)).count() != len {
        return Ok(false);
    }
    // The tilt's zero between an angle not past (`lo`) and one past (`hi`): secant steps on the tilt.
    let edge_at = |lo: f64, hi: f64| -> FResult<f64> {
        let tilt = |a: f64| -> FResult<Option<f64>> { Ok(ball(a)?.map(|b| b.0)) };
        let (Some(mut t0), Some(mut t1)) = (tilt(lo)?, tilt(hi)?) else { return Ok(0.5 * (lo + hi)) };
        let (mut x0, mut x1) = (lo, hi);
        for _ in 0..20 {
            if (t1 - t0).abs() < 1e-300 {
                break;
            }
            let x2 = (x1 - t1 * (x1 - x0) / (t1 - t0)).clamp(lo.min(hi), lo.max(hi));
            (x0, t0) = (x1, t1);
            x1 = x2;
            let Some(t) = tilt(x1)? else { break };
            t1 = t;
            if t1.abs() < 1e-10 {
                break;
            }
        }
        Ok(x1)
    };
    let a0 = edge_at((start as f64 - 1.0) * step, start as f64 * step)?;
    let a1 = edge_at((start + len) as f64 * step, (start + len - 1) as f64 * step)?;
    // The balls all round by their angle about the tube's axis: on the tube (centre rt + r from the axis, the wall r
    // away) except over the rim's band [a0, a1], where they roll over the rim (`ball`).
    let dir = |a: f64| x_dir * a.cos() + y_dir * a.sin();
    let tube_ball = |a: f64, guess: f64| -> FResult<Option<(DVec3, DVec3, DVec3)>> {
        let u = dir(a);
        let f = |s: f64| -> FResult<f64> { Ok(sd(rim_center + out * s + u * (rt + r))? - r) };
        let (mut x0, mut x1) = (guess, guess - 0.1 * r);
        let (mut f0, mut f1) = (f(x0)?, f(x1)?);
        for _ in 0..40 {
            if (f1 - f0).abs() < 1e-300 {
                break;
            }
            let x2 = x1 - f1 * (x1 - x0) / (f1 - f0);
            (x0, f0) = (x1, f1);
            x1 = x2;
            f1 = f(x1)?;
            if f1.abs() < scale * 1e-11 {
                break;
            }
        }
        if !f1.is_finite() || f1.abs() > scale * 1e-8 {
            return Ok(None);
        }
        let c = rim_center + out * x1 + u * (rt + r);
        Ok(Some((c, rim_center + out * x1 + u * rt, touch(c)?)))
    };
    // The balls' arcs from the tube (or rim) to the wall: over the rim (live's JWO) from a0 to a1, on the tube (JWK)
    // from a1 round to a0, each run of arcs lofted into its face.
    let arc = |(c, kp, tw): (DVec3, DVec3, DVec3)| -> FResult<Shape> {
        Ok(k::make_wire(&[k::make_arc_3pts(kp, c + (0.5 * (kp + tw) - c).normalize() * r, tw)?])?)
    };
    // Sections ~3° apart, and one at the tube's seam (where its face and the rim are split: the faces' edges must pass
    // through that point to sew).
    let seam = faces[tube_i].edges().into_iter().find_map(|x| {
        let i = x.edge_info().ok()?;
        let k::Curve::Line { .. } = i.curve else { return None };
        let v = i.start - rim_center;
        Some(v.dot(y_dir).atan2(v.dot(x_dir)))
    });
    let angles = |from: f64, to: f64| -> Vec<f64> {
        let n = ((to - from) / 3f64.to_radians()).ceil().max(2.0) as usize;
        let mut v: Vec<f64> = (0..=n).map(|j| from + (to - from) * j as f64 / n as f64).collect();
        if let Some(s) = seam {
            let s = from + (s - from).rem_euclid(std::f64::consts::TAU);
            let step = (to - from) / n as f64;
            if s > from + 0.05 * step && s < to - 0.05 * step && !v.iter().any(|a| (a - s).abs() < 1e-9) {
                v.retain(|a| (a - s).abs() > 0.3 * step || *a == from || *a == to);
                v.push(s);
                v.sort_by(f64::total_cmp);
            }
        }
        v
    };
    let (mut over, mut on) = (Vec::new(), Vec::new());
    for a in angles(a0, a1) {
        let Some((_, c, kp, tw)) = ball(a)? else { return Err(fail("a rim ball doesn't reach the wall")) };
        over.push((a, c, kp, tw));
    }
    let last = over.len() - 1;
    on.push(over[last]);
    let mut guess = (over[last].2 - rim_center).dot(out);
    let around = angles(a1, a0 + std::f64::consts::TAU);
    for &a in &around[1..around.len() - 1] {
        let Some((c, kp, tw)) = tube_ball(a, guess)? else { return Err(fail("a ball on the tube doesn't reach the wall")) };
        guess = (kp - rim_center).dot(out);
        on.push((a, c, kp, tw));
    }
    on.push(over[0]);
    let face_of = |run: &[(f64, DVec3, DVec3, DVec3)]| -> FResult<Shape> {
        let arcs: Vec<Shape> = run.iter().map(|&(_, c, kp, tw)| arc((c, kp, tw))).collect::<FResult<_>>()?;
        Ok(k::loft(&arcs, &k::LoftOptions { solid: false, ..Default::default() })?.shape)
    };
    let (jwo, jwk) = (face_of(&over)?, face_of(&on)?);
    // Their edges along the touches (not the end arcs): the wall's (through the wall touches) and the tube's.
    let along = |f: &Shape, run: &[(f64, DVec3, DVec3, DVec3)], pick: fn(&(f64, DVec3, DVec3, DVec3)) -> DVec3| -> FResult<Shape> {
        let mid = pick(&run[run.len() / 2]);
        let v = k::make_vertex(mid)?;
        f.edges().into_iter().min_by(|x, y| k::distance(x, &v).map_or(f64::INFINITY, |d| d.distance).total_cmp(&k::distance(y, &v).map_or(f64::INFINITY, |d| d.distance))).ok_or_else(|| fail("a blend face without edges"))
    };
    let wall_curves = [along(&jwo, &over, |b| b.3)?, along(&jwk, &on, |b| b.3)?];
    let tube_curve = along(&jwk, &on, |b| b.2)?;
    // The wall, the tube and the end's cap split by the faces' touch edges as they are (they lie on them within the
    // loft's tolerance, and the blend faces sew to the same curves; the three together, so the rim they share splits
    // alike; only these: the whole part's split checked the curves against every face, 0.6 s on the impeller). The
    // wall's and the tube's pieces along the edge (between it and the touches) are under the round.
    let part_names = body.shape_names();
    let trio = Shape::compound(&[wall.clone(), faces[tube_i].clone(), faces[cap_i].clone()]);
    let trio_names = crate::regen::eval::names_for(&trio, vec![part_names.faces[wall_i].clone(), part_names.faces[tube_i].clone(), part_names.faces[cap_i].clone()]);
    let curves = [wall_curves[0].clone(), wall_curves[1].clone(), tube_curve];
    let on_edge = |f: &Shape| {
        f.edges().iter().any(|x| {
            let Ok(i) = x.edge_info() else { return false };
            x.point_at(0.5 * (i.first_param + i.last_param)).is_ok_and(|q| k::make_vertex(q).and_then(|v| k::distance(&v, &edge)).is_ok_and(|d| d.distance < scale * 1e-7))
        })
    };
    let blend = Arc::new(TopoName::face(ev.feature, Role::Blend(body.edges[e].clone())));
    let blends = Shape::compound(&[jwo, jwk]);
    let blend_names = crate::regen::eval::names_for(&blends, vec![blend.clone(), blend]);
    // Split in 2D along the curves on their faces (the general split took ~270 ms of 9 edge-face intersections), else
    // where that doesn't sew, by the general split.
    for general in [false, true] {
        let split = match general {
            false => match k::split_on(&trio, &curves, scale * 1e-6) {
                Ok(s) => s,
                Err(_) => continue,
            },
            true => k::imprint(&trio, &curves)?,
        };
        let split_names = ev.name_result(&[&trio_names], &split.history, &split.shape, &BTreeMap::new(), None);
        let (mut kept, mut kept_names) = (Vec::new(), Vec::new());
        for (f, n) in split.shape.faces().into_iter().zip(split_names) {
            if !on_edge(&f) {
                kept.push(f);
                kept_names.push(n);
            }
        }
        for (i, f) in faces.iter().enumerate() {
            if i != wall_i && i != tube_i && i != cap_i {
                kept.push(f.clone());
                kept_names.push(part_names.faces[i].clone());
            }
        }
        if std::env::var_os("NOSHAPE_BLEND_DEBUG").is_some() {
            eprintln!("cliff end: angles {:.2}..{:.2} deg; split {} faces (general {general}), kept {} of the part's {}", a0.to_degrees(), a1.to_degrees(), split.shape.faces().len(), kept.len(), faces.len());
        }
        let kept_shape = Shape::compound(&kept);
        let kept_names = crate::regen::eval::names_for(&kept_shape, kept_names);
        let sewn = match k::sew(&[kept_shape, blends.clone()], scale * 1e-6, true) {
            Ok(s) => s,
            Err(_) if !general => continue,
            Err(e) => return Err(e.into()),
        };
        let sewn_names = ev.name_result(&[&kept_names, &blend_names], &sewn.history, &sewn.shape, &BTreeMap::new(), None);
        let un = k::unify_same_domain(&sewn.shape)?;
        if std::env::var_os("NOSHAPE_BLEND_DEBUG").is_some() {
            eprintln!("cliff end: sewn valid {} faces {} volume {:?} (was {:?}); unified valid {} faces {}", sewn.shape.is_valid(), sewn.shape.faces().len(), sewn.shape.volume().map(|v| v * 1e9), shape.volume().map(|v| v * 1e9), un.shape.is_valid(), un.shape.faces().len());
        }
        if !un.shape.is_valid() || un.shape.count(ShapeType::Solid) != 1 {
            if !general {
                continue;
            }
            return Err(fail("the round over the rim is not one valid solid"));
        }
        let named = NamedShape::new(sewn.shape, sewn_names);
        let names = ev.name_result(&[&named.names], &un.history, &un.shape, &BTreeMap::new(), None);
        ev.replace_body(bi, un.shape, names);
        return Ok(true);
    }
    Err(fail("the round over the rim didn't sew"))
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
/// The region a rim's chamfer `d` takes off `wedge` (the rim's plane and cylinder about it), all round the rim: live's
/// chamfer is the chord of one ball rolling between the plane and the cylinder (2384, 2441), its radius `d tan(φ / 2)`
/// at the edge's middle (φ the faces' angle there), so the setback is `d` there and follows the angle elsewhere. The
/// wedge split by the closed sheet through the ball's touch points (each chord a little longer both ways), the piece
/// at the rim kept: its faces on the plane and the cylinder are theirs. The sheet starts at `seam` (a radial off the
/// part).
fn rim_chamfer_tool(rim: &Rim, edge: &Shape, d: f64, kind: RimBlend, wedge: &Shape, seam: DVec3, scale: f64) -> FResult<Shape> {
    let (x, y) = rim.frame();
    let angle = |v: DVec3| v.dot(y).atan2(v.dot(x));
    // The edge's middle: an arc's; a whole rim's, where Parasolid's ellipse is halfway round from its start. An equal
    // chamfer (live 2441, our axis): the major axis's end toward the cylinder's axis seen on the plane; an Offset one
    // (live c2-hunting-knife's Chamfer 2, Parasolid's axis = ours: its hole's rims start at x 27.5 and 32.5): the end
    // away from it. (Which is Parasolid's rule for both is open.)
    let a_mid = if rim.arc.is_empty() {
        rim.whole_middle() + if matches!(kind, RimBlend::Offset { .. }) { std::f64::consts::PI } else { 0.0 }
    } else {
        let info = edge.edge_info()?;
        rim.angle_of(edge.point_at(0.5 * (info.first_param + info.last_param))?)
    };
    // The point `ow` off the cylinder and `op` under the plane at angle `a`, and its feet on the plane and the cylinder.
    let ball = |a: f64, ow: f64, op: f64| -> (DVec3, DVec3, DVec3) {
        let (p, radial) = rim.at(x, y, a);
        let base = rim.origin + radial * (rim.rc + rim.out * ow);
        let c = base + rim.axis * (((p - base).dot(rim.np) - op) / rim.axis.dot(rim.np));
        (p, c + rim.np * op, c - radial * (rim.out * ow))
    };
    let (_, radial) = rim.at(x, y, a_mid);
    let phi = std::f64::consts::PI - rim.np.dot(-radial * rim.out).clamp(-1.0, 1.0).acos();
    // An equal chamfer: one rolling ball, its setback `d` at the middle. An Offset one: the offsets whose feet lie at
    // the asked distances there (Distance 1 on its face, the other from the angle or its distance).
    let (ow, op) = match kind {
        RimBlend::Offset { on_wall, other } => {
            let e = match other {
                OffsetOther::Angle(a) if a + phi < std::f64::consts::PI - 1e-9 => d * a.sin() / (a + phi).sin(),
                OffsetOther::Angle(_) => return Err(fail("the chamfer's angle doesn't meet the other face")),
                OffsetOther::Distance(e) => e,
            };
            let (o_ref, o_other) = feet_offsets(d, e, phi.cos())?;
            if on_wall { (o_ref, o_other) } else { (o_other, o_ref) }
        }
        _ => {
            let r = d * (phi / 2.0).tan();
            (r, r)
        }
    };
    let r = ow.max(op);
    if !r.is_finite() || ow <= 0.0 || op <= 0.0 {
        return Err(fail("the rim's faces are tangent"));
    }
    const N: usize = 64;
    let mut chords = Vec::new();
    for k in 0..N {
        // (From the wedge's seam, off the part: the sheet's own seam would split the chamfer face.)
        let (_, t1, t2) = ball(angle(seam) + k as f64 * std::f64::consts::TAU / N as f64, ow, op);
        let past = (t2 - t1) * 0.25;
        chords.push(k::make_wire(&[k::make_line(t1 - past, t2 + past)?])?);
    }
    let sheet = k::loft(&chords, &k::LoftOptions { solid: false, closed: true, ..Default::default() })?.shape;
    let (p, t1, t2) = ball(a_mid, ow, op);
    let inside = (p + t1 + t2) / 3.0;
    let pieces = k::split(std::slice::from_ref(wedge), std::slice::from_ref(&sheet), &k::BooleanOptions::default())?.shape.solids();
    // The piece keeping the wedge's rim edge (closed, through the rim's point): no classification against the sheet's
    // B-spline (~1 ms each, a dozen pieces a rim).
    let at = k::make_vertex(p)?;
    let keeps_rim = |piece: &Shape| piece.edges().iter().any(|e| e.edge_info().is_ok_and(|i| i.closed) && k::distance(e, &at).is_ok_and(|d| d.distance < scale * 1e-7));
    let mut kept = pieces.iter().filter(|piece| keeps_rim(piece));
    if let (Some(one), None) = (kept.next(), kept.next()) {
        return Ok(one.clone());
    }
    pieces
        .into_iter()
        .find(|piece| piece.classify_point(inside, scale * 1e-9).is_ok_and(|c| c == k::PointClass::Inside))
        .ok_or_else(|| fail("the chamfer's sheet didn't cut the rim off the wedge"))
}

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
    variable_round_in(ev, bi, laws, None)
}

/// The cubic Bézier `b` (values over t in [0, 1]) over [t0, t1] alone (de Casteljau).
fn bezier_part(b: [f64; 4], t0: f64, t1: f64) -> [f64; 4] {
    let split = |b: [f64; 4], t: f64| -> ([f64; 4], [f64; 4]) {
        let l = |a: f64, c: f64| a + (c - a) * t;
        let (p01, p12, p23) = (l(b[0], b[1]), l(b[1], b[2]), l(b[2], b[3]));
        let (p012, p123) = (l(p01, p12), l(p12, p23));
        let p = l(p012, p123);
        ([b[0], p01, p012, p], [p, p123, p23, b[3]])
    };
    let (_, right) = split(b, t0);
    if t1 >= 1.0 - 1e-15 || t0 >= 1.0 - 1e-15 {
        return right;
    }
    split(right, (t1 - t0) / (1.0 - t0)).0
}

/// [`variable_round`] of one edge's span alone where `range` is given (a partial variable fillet, live 3028: the whole
/// edge's law within the span, flat ends): from and to, as distances along each edge from its start.
pub fn variable_round_in(ev: &mut EvalCtx, bi: usize, laws: &[(usize, RadiusLaw)], range: Option<(f64, f64)>) -> FResult<()> {
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
        let (lo, hi) = range.unwrap_or((0.0, len));
        let (e0, e1) = (if lo <= 1e-12 * len { ext(a, -d, r_start)? } else { 0.0 }, if hi >= len * (1.0 - 1e-12) { ext(b, d, r_end)? } else { 0.0 });
        // The law's spans as cubic Béziers in s (the slopes per fraction: per length / len), the ends' run-ons constant.
        let mut spans: Vec<(f64, f64, [f64; 4])> = Vec::new();
        if e0 > 0.0 {
            spans.push((-e0, 0.0, [r_start; 4]));
        }
        for (k, w) in points.windows(2).enumerate() {
            let (s0, s1) = (w[0].0 * len, w[1].0 * len);
            let h = s1 - s0;
            let (m0, m1) = (slopes[k] / len, slopes[k + 1] / len);
            let b = [w[0].1, w[0].1 + m0 * h / 3.0, w[1].1 - m1 * h / 3.0, w[1].1];
            // (Only what lies in the range: the span cut at its ends.)
            let (c0, c1) = (s0.max(lo), s1.min(hi));
            if c1 - c0 <= 1e-12 * len {
                continue;
            }
            spans.push((c0, c1, if c0 > s0 || c1 < s1 { bezier_part(b, (c0 - s0) / h, (c1 - s0) / h) } else { b }));
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

#[cfg(test)]
mod tests {
    use super::{Support, plane_cylinder_contacts, rolling_ball};
    use glam::DVec3;

    /// Live c2-hunting-knife's Fillet 3 (mm): the 0.5 ball between the bottom plane and the R3 cove, and its contacts'
    /// ellipses: Kj1B on the plane 3.502620 x 3.5, KjtB on the cove 3.003056 x 3 (and the top's Jj1 3.513369, Jjt
    /// 3.015587).
    #[test]
    fn a_rolling_ball_between_a_plane_and_a_cylinder_touches_both_along_live_ellipses() {
        let axis = DVec3::new(4.768546, -4.899886, 0.0);
        let cove = Support::Cylinder { o: axis, a: DVec3::Z, r: 3.0, s: 1.0 };
        for (point, outward, majors) in [
            (DVec3::new(2.828121, -2.0, -10.390551), DVec3::new(0.038671, 0.0, -0.999252), (3.502620, 3.003056)),
            (DVec3::new(2.852363, -2.0, 5.250451), DVec3::new(0.087156, 0.0, 0.996195), (3.513369, 3.015587)),
        ] {
            let n = -outward.normalize();
            let plane = Support::Plane { o: point, n };
            let sup = [plane, cove.clone()];
            // Across the edge (the plane's line on the cove) halfway along it.
            let round = axis + DVec3::new(-0.4, 2.973, 0.0).normalize() * 3.0;
            let on_edge = round.with_z(point.z - n.x * (round.x - point.x) / n.z);
            let tangent = DVec3::Z.cross(on_edge - axis).normalize();
            let (c, [t0, t1]) = rolling_ball(on_edge, tangent, &sup, 0.5, on_edge + (n + (on_edge - axis).with_z(0.0).normalize()) * 0.3).unwrap();
            assert!(((c - point).dot(n) - 0.5).abs() < 1e-9, "the ball's centre is off the plane by {}", (c - point).dot(n));
            assert!(((c - axis).with_z(0.0).length() - 3.5).abs() < 1e-9);
            let conics = plane_cylinder_contacts(&sup, 0.5);
            assert_eq!(conics.len(), 2);
            assert!((conics[0].major - majors.0).abs() < 2e-6 && (conics[0].minor - 3.5).abs() < 1e-9, "{:?}", conics[0]);
            assert!((conics[1].major - majors.1).abs() < 2e-6 && (conics[1].minor - 3.0).abs() < 1e-9, "{:?}", conics[1]);
            for (t, e) in [(t0, &conics[0]), (t1, &conics[1])] {
                let v = t - e.centre;
                let (x, y) = (v.dot(e.major_dir), v.dot(e.normal.cross(e.major_dir)));
                assert!(v.dot(e.normal).abs() < 1e-9 && ((x / e.major).powi(2) + (y / e.minor).powi(2) - 1.0).abs() < 1e-9, "a contact off its ellipse: {t:?}");
            }
        }
    }
}
