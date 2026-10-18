//! Geometry behind Onshape's analysis tools: curvature combs on edges and
//! along face isocurves (Curve/surface analysis, Shift+C) and dihedral
//! angle combs along edges (Dihedral analysis, Shift+D). The app draws the
//! combs; scaling to the screen is left to it.

use crate::reference::Reference;
use crate::regen::context::{Body, Context};
use glam::DVec3;
use noshape_kernel::ShapeType;

/// A comb: teeth from points on a curve; the tips form its envelope.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Comb {
    pub base: Vec<DVec3>,
    /// Tooth vectors (direction and length before screen scaling).
    pub teeth: Vec<DVec3>,
}

impl Comb {
    pub fn max_len(&self) -> f64 {
        self.teeth.iter().map(|t| t.length()).fold(0.0, f64::max)
    }
}

/// Curvature comb of a polyline sampled densely along a curve: at each
/// interior sample, curvature 1/R from the circle through its neighbours;
/// the tooth points away from the center of curvature (Onshape draws combs
/// on the convex side). Returns the comb and the minimum radius (with its
/// point), `None` radius when the curve is straight.
pub fn polyline_comb(points: &[DVec3]) -> (Comb, Option<(f64, DVec3)>) {
    let mut comb = Comb::default();
    let mut min: Option<(f64, DVec3)> = None;
    if points.len() < 3 {
        return (comb, None);
    }
    for w in points.windows(3) {
        let (a, b, c) = (w[0], w[1], w[2]);
        let (ab, cb) = (a - b, c - b);
        let cross = ab.cross(cb);
        let denom = 2.0 * cross.length_squared();
        let tooth = if denom <= 1e-30 {
            DVec3::ZERO
        } else {
            // Circumcenter of a, b, c relative to b.
            let center = (ab.length_squared() * cb - cb.length_squared() * ab).cross(cross) / denom;
            let r = center.length();
            if r > 0.0 && min.is_none_or(|(m, _)| r < m) {
                min = Some((r, b));
            }
            -center.normalize_or_zero() / r.max(1e-30)
        };
        comb.base.push(b);
        comb.teeth.push(tooth);
    }
    (comb, min)
}

fn body_of<'a>(ctx: &'a Context, r: &Reference) -> Option<(&'a Body, ShapeType, usize)> {
    ctx.bodies.iter().find_map(|b| match r {
        Reference::Face(n) => b.find_faces(n).first().map(|i| (&**b, ShapeType::Face, *i)),
        Reference::Edge(n) => b.find_edges(n).first().map(|i| (&**b, ShapeType::Edge, *i)),
        _ => None,
    })
}

/// Points evenly spaced in parameter along a part edge.
fn edge_points(b: &Body, e: usize, samples: usize) -> Option<Vec<DVec3>> {
    let s = b.shape.sub_shape(ShapeType::Edge, e).ok()?;
    let info = s.edge_info().ok()?;
    let n = samples.max(3);
    (0..=n).map(|i| s.point_at(info.first_param + (info.last_param - info.first_param) * i as f64 / n as f64).ok()).collect()
}

/// Curvature comb of a part edge and its minimum radius.
pub fn edge_curvature(ctx: &Context, edge: &Reference, samples: usize) -> Option<(Comb, Option<(f64, DVec3)>)> {
    let (b, kind, e) = body_of(ctx, edge)?;
    if kind != ShapeType::Edge {
        return None;
    }
    Some(polyline_comb(&edge_points(b, e, samples)?))
}

/// Isocurve combs of a face: `u_curves` curves of constant u (running in
/// v, Onshape's "V curves", blue) and `v_curves` of constant v ("U curves",
/// red). Returns (u-direction combs, v-direction combs).
pub fn face_curvature(ctx: &Context, face: &Reference, u_curves: usize, v_curves: usize, samples: usize) -> Option<(Vec<Comb>, Vec<Comb>)> {
    let (b, kind, f) = body_of(ctx, face)?;
    if kind != ShapeType::Face {
        return None;
    }
    let s = b.shape.sub_shape(ShapeType::Face, f).ok()?;
    let [u0, u1, v0, v1] = s.uv_bounds().ok()?;
    let n = samples.max(3);
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    // Only points inside the trimmed face (isocurves of the underlying surface).
    let on_face = |p: DVec3| s.project_point(p).map(|q| q.point.distance(p) < 1e-7_f64.max(p.length() * 1e-7)).unwrap_or(false);
    let curve = |pts: Vec<DVec3>| -> Comb {
        let pts: Vec<DVec3> = pts.into_iter().filter(|p| on_face(*p)).collect();
        polyline_comb(&pts).0
    };
    let along_u: Vec<Comb> = (0..v_curves.max(1))
        .map(|j| {
            let v = lerp(v0, v1, (j as f64 + 0.5) / v_curves.max(1) as f64);
            curve((0..=n).filter_map(|i| s.point_at_uv(lerp(u0, u1, i as f64 / n as f64), v).ok()).collect())
        })
        .collect();
    let along_v: Vec<Comb> = (0..u_curves.max(1))
        .map(|j| {
            let u = lerp(u0, u1, (j as f64 + 0.5) / u_curves.max(1) as f64);
            curve((0..=n).filter_map(|i| s.point_at_uv(u, lerp(v0, v1, i as f64 / n as f64)).ok()).collect())
        })
        .collect();
    Some((along_u, along_v))
}

/// Dihedral comb of an edge between two faces: at each sample the tooth
/// length is the angle (radians) between the faces' normals there, pointing
/// out of the material; returns the comb and the min / max angle in degrees.
pub fn edge_dihedral(ctx: &Context, edge: &Reference, samples: usize) -> Option<(Comb, f64, f64)> {
    let (b, kind, e) = body_of(ctx, edge)?;
    if kind != ShapeType::Edge {
        return None;
    }
    let faces = b.shape.edge_faces().ok()?.get(e)?.clone();
    let (fa, fb) = match faces.as_slice() {
        [a, b2] => (*a as usize, *b2 as usize),
        _ => return None,
    };
    let sa = b.shape.sub_shape(ShapeType::Face, fa).ok()?;
    let sb = b.shape.sub_shape(ShapeType::Face, fb).ok()?;
    let mut comb = Comb::default();
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for p in edge_points(b, e, samples)? {
        let (Ok(na), Ok(nb)) = (sa.normal_at_point(p), sb.normal_at_point(p)) else { continue };
        let (na, nb) = (na.normalize_or_zero(), nb.normalize_or_zero());
        let angle = na.dot(nb).clamp(-1.0, 1.0).acos();
        lo = lo.min(angle.to_degrees());
        hi = hi.max(angle.to_degrees());
        comb.base.push(p);
        comb.teeth.push((na + nb).normalize_or_zero() * angle);
    }
    (!comb.base.is_empty()).then_some((comb, lo, hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_comb_has_constant_curvature() {
        let r = 2.0;
        let pts: Vec<DVec3> = (0..=32).map(|i| {
            let a = i as f64 / 32.0 * std::f64::consts::PI;
            DVec3::new(r * a.cos(), r * a.sin(), 0.0)
        }).collect();
        let (comb, min) = polyline_comb(&pts);
        assert_eq!(comb.base.len(), 31);
        for (b, t) in comb.base.iter().zip(&comb.teeth) {
            assert!((t.length() - 1.0 / r).abs() < 1e-3, "{}", t.length());
            // Away from the center (the origin).
            assert!(t.dot(*b) > 0.0);
        }
        assert!((min.unwrap().0 - r).abs() < 1e-3);
        // A straight line: no curvature, no radius.
        let line: Vec<DVec3> = (0..5).map(|i| DVec3::new(i as f64, 0.0, 0.0)).collect();
        let (c, m) = polyline_comb(&line);
        assert!(c.max_len() == 0.0 && m.is_none());
    }

    #[test]
    fn plate_edges_dihedral_and_curvature() {
        let ps = crate::demo::plate_studio();
        let ctx = crate::regen::Regenerator::new().regenerate(&ps, &Default::default(), &noshape_units::UnitPrefs::default(), &crate::regen::RegenOptions::default()).context;
        let b = &ctx.bodies[0];
        let r = |e: usize| Reference::Edge((*b.edges[e]).clone());
        // Some edge meets at 90° (a plate corner edge).
        let square = (0..b.edges.len()).filter_map(|e| edge_dihedral(&ctx, &r(e), 8)).any(|(_, lo, hi)| (lo - 90.0).abs() < 1.0 && (hi - 90.0).abs() < 1.0);
        assert!(square);
        // A circular edge has a comb with a finite minimum radius.
        let curved = (0..b.edges.len()).filter_map(|e| edge_curvature(&ctx, &r(e), 16)).any(|(c, m)| c.max_len() > 0.0 && m.is_some());
        assert!(curved);
    }
}
