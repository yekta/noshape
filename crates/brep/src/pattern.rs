//! Arguments that are rigid copies of each other (a pattern's instances, a mirror's images, whatever made them):
//! found from their geometry alone. Two bodies are copies when one rigid motion (a rotation, translation or
//! reflection) takes each vertex, curve, surface and curve on a surface of one onto the other's of the same index,
//! within a tolerance; nothing else is assumed (copies made by moving a shape keep their parts' order).

use crate::{Body, Curve, Surface};
use glam::{DMat3, DVec3};

/// A rigid motion: `x ↦ m x + t` (`m` orthonormal; its determinant -1 for a reflection).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rigid {
    pub m: DMat3,
    pub t: DVec3,
}

impl Rigid {
    pub const IDENTITY: Rigid = Rigid { m: DMat3::IDENTITY, t: DVec3::ZERO };

    pub fn point(&self, p: DVec3) -> DVec3 {
        self.m * p + self.t
    }

    pub fn dir(&self, v: DVec3) -> DVec3 {
        self.m * v
    }

    /// `self` after `o`: `x ↦ self(o(x))`.
    pub fn after(&self, o: &Rigid) -> Rigid {
        Rigid { m: self.m * o.m, t: self.m * o.t + self.t }
    }

    pub fn inverse(&self) -> Rigid {
        let mt = self.m.transpose();
        Rigid { m: mt, t: -(mt * self.t) }
    }

    /// Whether `o` is the same motion, within `tol` over a region of size `size` about the origin.
    pub fn near(&self, o: &Rigid, tol: f64, size: f64) -> bool {
        let dm = (0..3).map(|i| (self.m.col(i) - o.m.col(i)).abs().max_element()).fold(0.0, f64::max);
        dm * size <= tol && (self.t - o.t).abs().max_element() <= tol
    }

    /// A rotation's axis (a point on it, its direction) and angle in (0, π], or None for the identity, a
    /// translation or a reflection.
    pub fn rotation(&self) -> Option<(DVec3, DVec3, f64)> {
        if self.m.determinant() < 0.0 {
            return None;
        }
        let c = ((self.m.x_axis.x + self.m.y_axis.y + self.m.z_axis.z - 1.0) * 0.5).clamp(-1.0, 1.0);
        let angle = c.acos();
        if angle < 1e-9 {
            return None;
        }
        let w = DVec3::new(self.m.y_axis.z - self.m.z_axis.y, self.m.z_axis.x - self.m.x_axis.z, self.m.x_axis.y - self.m.y_axis.x);
        let axis = if w.length() > 1e-9 {
            w.normalize()
        } else {
            // (A half turn: the axis is the direction m keeps.)
            let s = self.m + DMat3::IDENTITY;
            [s.x_axis, s.y_axis, s.z_axis].into_iter().max_by(|a, b| a.length().total_cmp(&b.length()))?.normalize()
        };
        // The point on the axis: (I - m) p = t has a solution along the plane across the axis.
        let (u, v) = axis.any_orthonormal_pair();
        let a = DMat3::from_cols(u - self.m * u, v - self.m * v, axis);
        if a.determinant().abs() < 1e-12 {
            return None;
        }
        let x = a.inverse() * self.t;
        Some((u * x.x + v * x.y, axis, angle))
    }
}

/// The body's NURBS curves that are curves on its faces (in their parameters).
fn in_parameters(b: &Body) -> Vec<u32> {
    let mut v: Vec<u32> = b.pcurves.iter().map(|p| p.curve).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// The parts of a body the transform is solved from: its vertices, then its curves' and surfaces' defining points
/// (a body of one closed curved face has a single vertex).
fn points(b: &Body) -> Vec<DVec3> {
    let mut out = b.vertices.clone();
    for e in &b.edges {
        if let Curve::Circle { center, normal, xdir, radius } = e.curve {
            out.extend([center, center + normal * radius, center + xdir * radius]);
        }
    }
    // (Not the curves on faces: their points are parameters.)
    let on_faces = in_parameters(b);
    for (i, c) in b.nurbs_curves.iter().enumerate() {
        if !on_faces.contains(&(i as u32)) {
            out.extend(&c.poles);
        }
    }
    for s in &b.nurbs_surfaces {
        out.extend(&s.poles);
    }
    out
}

/// The motion taking points `p` onto `q` (corresponding), from four of them spread out (both handednesses where
/// they're coplanar), checked on all; None if none fits within `tol`.
fn solve(p: &[DVec3], q: &[DVec3], tol: f64) -> Option<Rigid> {
    if p.len() != q.len() || p.is_empty() {
        return None;
    }
    let far = |from: &dyn Fn(DVec3) -> f64| (0..p.len()).max_by(|&a, &b| from(p[a]).total_cmp(&from(p[b])));
    let i0 = 0;
    let i1 = far(&|x| (x - p[i0]).length())?;
    let d1 = p[i1] - p[i0];
    if d1.length() <= tol {
        // (One point: no rotation to tell.)
        return (p.iter().zip(q).all(|(a, b)| (*b - *a - (q[i0] - p[i0])).length() <= tol)).then_some(Rigid { m: DMat3::IDENTITY, t: q[i0] - p[i0] });
    }
    let i2 = far(&|x| (x - p[i0]).cross(d1).length())?;
    let frame = |a: DVec3, b: DVec3, c: DVec3| -> Option<DMat3> {
        let x = (b - a).normalize();
        let z = x.cross(c - a);
        if z.length() < 1e-12 {
            return None;
        }
        let z = z.normalize();
        Some(DMat3::from_cols(x, z.cross(x), z))
    };
    let fits = |r: &Rigid| p.iter().zip(q).all(|(a, b)| (r.point(*a) - *b).length() <= tol);
    let fp = frame(p[i0], p[i1], p[i2]);
    let fq = frame(q[i0], q[i1], q[i2]);
    let candidates: Vec<DMat3> = match (fp, fq) {
        (Some(fp), Some(fq)) => {
            // Rotation, and the reflection across the frame's plane (needed where all points lie in it).
            let flip = DMat3::from_diagonal(DVec3::new(1.0, 1.0, -1.0));
            vec![fq * fp.transpose(), fq * flip * fp.transpose()]
        }
        // (All on a line: any rotation about it fits; not told.)
        _ => Vec::new(),
    };
    candidates.into_iter().map(|m| Rigid { m, t: q[i0] - m * p[i0] }).find(fits)
}

/// The motion taking `a` onto `b` part by part (same indices), within `tol`; None where they aren't such copies.
pub fn copy_of(a: &Body, b: &Body, tol: f64) -> Option<Rigid> {
    if a.vertices.len() != b.vertices.len()
        || a.edges.len() != b.edges.len()
        || a.faces.len() != b.faces.len()
        || a.solids != b.solids
        || a.nurbs_curves.len() != b.nurbs_curves.len()
        || a.nurbs_surfaces.len() != b.nurbs_surfaces.len()
        || a.pcurves.len() != b.pcurves.len()
    {
        return None;
    }
    let r = solve(&points(a), &points(b), tol)?;
    // Topology the same.
    if a.edges.iter().zip(&b.edges).any(|(x, y)| x.v != y.v || std::mem::discriminant(&x.curve) != std::mem::discriminant(&y.curve)) {
        return None;
    }
    if a.faces.iter().zip(&b.faces).any(|(x, y)| x.loops != y.loops || x.reversed != y.reversed) {
        return None;
    }
    let pt = |x: DVec3, y: DVec3| (r.point(x) - y).length() <= tol;
    let dir = |x: DVec3, y: DVec3| (r.dir(x) - y).length() <= 1e-9;
    let num = |x: f64, y: f64| (x - y).abs() <= tol;
    for (x, y) in a.edges.iter().zip(&b.edges) {
        let ok = match (x.curve, y.curve) {
            (Curve::Line, Curve::Line) | (Curve::Degenerate, Curve::Degenerate) => true,
            (Curve::Circle { center: c1, normal: n1, xdir: x1, radius: r1 }, Curve::Circle { center: c2, normal: n2, xdir: x2, radius: r2 }) => {
                // (A reflection turns a circle's sense: its normal goes with the cross product.)
                let s = r.m.determinant().signum();
                pt(c1, c2) && (r.dir(n1) * s - n2).length() <= 1e-9 && dir(x1, x2) && num(r1, r2)
            }
            (Curve::Nurbs { index: i1, t0: a1, t1: b1 }, Curve::Nurbs { index: i2, t0: a2, t1: b2 }) => i1 == i2 && a1 == a2 && b1 == b2,
            _ => false,
        };
        if !ok {
            return None;
        }
    }
    // (Curves on faces the same, their points parameters; the others moved.)
    let on_faces = in_parameters(a);
    for (i, (x, y)) in a.nurbs_curves.iter().zip(&b.nurbs_curves).enumerate() {
        let moved = !on_faces.contains(&(i as u32));
        if x.degree != y.degree || x.knots != y.knots || x.weights != y.weights || x.poles.len() != y.poles.len() || x.poles.iter().zip(&y.poles).any(|(p, q)| if moved { !pt(*p, *q) } else { (*p - *q).length() > tol }) {
            return None;
        }
    }
    for (x, y) in a.nurbs_surfaces.iter().zip(&b.nurbs_surfaces) {
        if (x.udegree, x.vdegree, x.nu, x.nv) != (y.udegree, y.vdegree, y.nu, y.nv) || x.uknots != y.uknots || x.vknots != y.vknots || x.weights != y.weights || x.poles.iter().zip(&y.poles).any(|(p, q)| !pt(*p, *q)) {
            return None;
        }
    }
    if a.pcurves != b.pcurves {
        return None;
    }
    let s = r.m.determinant().signum();
    for (x, y) in a.faces.iter().zip(&b.faces) {
        let ok = match (x.surface, y.surface) {
            (Surface::Plane { origin: o1, normal: n1 }, Surface::Plane { origin: o2, normal: n2 }) => dir(n1, n2) && (r.point(o1) - o2).dot(n2).abs() <= tol,
            (Surface::Cylinder { origin: o1, axis: a1, xdir: x1, radius: r1 }, Surface::Cylinder { origin: o2, axis: a2, xdir: x2, radius: r2 }) => {
                // (Its parameters too: the seam where the other's is, the angles the same way.)
                pt(o1, o2) && (r.dir(a1) * s - a2).length() <= 1e-9 && dir(x1, x2) && num(r1, r2)
            }
            (Surface::Cone { origin: o1, axis: a1, xdir: x1, radius: r1, angle: g1 }, Surface::Cone { origin: o2, axis: a2, xdir: x2, radius: r2, angle: g2 }) => {
                pt(o1, o2) && (r.dir(a1) * s - a2).length() <= 1e-9 && dir(x1, x2) && num(r1, r2) && (g1 - g2).abs() <= 1e-12
            }
            (Surface::Nurbs { index: i1 }, Surface::Nurbs { index: i2 }) => i1 == i2,
            _ => false,
        };
        if !ok {
            return None;
        }
    }
    Some(r)
}

/// Tools in families of copies: each family's first tool and, for every member, the motion from the first onto it
/// (the first's the identity). A tool no other copies is a family of one.
pub fn families(tools: &[&Body], tol: f64) -> Vec<Vec<(usize, Rigid)>> {
    let mut out: Vec<Vec<(usize, Rigid)>> = Vec::new();
    'tools: for (i, t) in tools.iter().enumerate() {
        for fam in &mut out {
            if let Some(r) = copy_of(tools[fam[0].0], t, tol) {
                fam.push((i, r));
                continue 'tools;
            }
        }
        out.push(vec![(i, Rigid::IDENTITY)]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, Face, Solid, Use};

    /// A box from `lo` to `hi` moved by `r` (its parts in one order whatever the motion).
    fn moved_box(lo: DVec3, hi: DVec3, r: &Rigid) -> Body {
        let mut b = Body::default();
        for k in 0..8 {
            let p = DVec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z });
            b.vertices.push(r.point(p));
        }
        let pairs = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];
        for (a, c) in pairs {
            b.edges.push(Edge { v: [a, c], curve: Curve::Line });
        }
        let faces = [(DVec3::NEG_X, lo), (DVec3::X, hi), (DVec3::NEG_Y, lo), (DVec3::Y, hi), (DVec3::NEG_Z, lo), (DVec3::Z, hi)];
        for (k, (n, o)) in faces.into_iter().enumerate() {
            b.faces.push(Face { surface: Surface::Plane { origin: r.point(o), normal: r.dir(n) }, reversed: false, loops: vec![vec![Use { edge: k as u32, reversed: false }]] });
        }
        b.solids.push(Solid { shells: vec![(0..6).collect()] });
        b
    }

    fn rot_z(angle: f64, about: DVec3) -> Rigid {
        let m = DMat3::from_rotation_z(angle);
        Rigid { m, t: about - m * about }
    }

    #[test]
    fn rotated_copies_are_found_with_their_axis_and_angle() {
        let (lo, hi) = (DVec3::new(0.02, -0.001, 0.0), DVec3::new(0.03, 0.001, 0.01));
        let about = DVec3::new(0.001, -0.002, 0.0);
        let bodies: Vec<Body> = (0..12).map(|k| moved_box(lo, hi, &rot_z(std::f64::consts::TAU * k as f64 / 12.0, about))).collect();
        let refs: Vec<&Body> = bodies.iter().collect();
        let fams = families(&refs, 1e-9);
        assert_eq!(fams.len(), 1);
        let (o, axis, angle) = fams[0][1].1.rotation().expect("a rotation");
        assert!((angle - std::f64::consts::TAU / 12.0).abs() < 1e-12);
        assert!((axis - DVec3::Z).length() < 1e-12);
        assert!((o - about).cross(DVec3::Z).length() < 1e-9, "{o}");
    }

    #[test]
    fn translated_and_mirrored_copies_too_but_not_other_shapes() {
        let (lo, hi) = (DVec3::ZERO, DVec3::new(1.0, 2.0, 3.0));
        let a = moved_box(lo, hi, &Rigid::IDENTITY);
        let t = moved_box(lo, hi, &Rigid { m: DMat3::IDENTITY, t: DVec3::new(5.0, 0.0, 0.0) });
        assert_eq!(copy_of(&a, &t, 1e-9).map(|r| r.t), Some(DVec3::new(5.0, 0.0, 0.0)));
        // A mirror image across x = 0 (its parts in the same order; its faces' normals mirrored).
        let mirror = Rigid { m: DMat3::from_diagonal(DVec3::new(-1.0, 1.0, 1.0)), t: DVec3::ZERO };
        let m = moved_box(lo, hi, &mirror);
        assert!(copy_of(&a, &m, 1e-9).is_some_and(|r| r.m.determinant() < 0.0));
        // Another size: no copy; off by more than the tolerance: none either.
        assert!(copy_of(&a, &moved_box(lo, DVec3::new(1.0, 2.0, 3.1), &Rigid::IDENTITY), 1e-9).is_none());
        let mut off = t.clone();
        off.vertices[7].z += 1e-6;
        assert!(copy_of(&a, &off, 1e-9).is_none());
    }
}
