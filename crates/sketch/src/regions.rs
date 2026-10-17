//! Region (profile) detection and wires.
//!
//! All non-construction curves are intersected pairwise and split at the
//! intersections; the resulting planar arrangement is stored as a half-edge
//! graph whose bounded faces are the regions. Faces of nested connected
//! components become holes of the smallest face that contains them.

use crate::entities::EntityKind;
use crate::geom::{BBox, Curve2};
use crate::ids::EntityId;
use crate::intersect::intersect;
use crate::nurbs::Nurbs;
use crate::sketch::Sketch;
use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

/// One oriented segment of a profile loop or wire.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProfileSegment {
    Line { a: DVec2, b: DVec2 },
    /// Circular arc from `start_angle` to `end_angle`; `end_angle - start_angle`
    /// is positive when `ccw` and negative otherwise.
    Arc { center: DVec2, radius: f64, start_angle: f64, end_angle: f64, ccw: bool },
    /// Elliptical arc in ellipse parameter space (same convention as `Arc`).
    EllipseArc {
        center: DVec2,
        major_axis: DVec2,
        major_radius: f64,
        minor_radius: f64,
        start_param: f64,
        end_param: f64,
        ccw: bool,
    },
    /// NURBS segment oriented along the loop.
    Spline { control_points: Vec<DVec2>, knots: Vec<f64>, degree: usize, weights: Option<Vec<f64>> },
}

const GL8: [(f64, f64); 8] = [
    (-0.960_289_856_497_536_3, 0.101_228_536_290_376_26),
    (-0.796_666_477_413_626_7, 0.222_381_034_453_374_47),
    (-0.525_532_409_916_329, 0.313_706_645_877_887_3),
    (-0.183_434_642_495_649_8, 0.362_683_783_378_362_0),
    (0.183_434_642_495_649_8, 0.362_683_783_378_362_0),
    (0.525_532_409_916_329, 0.313_706_645_877_887_3),
    (0.796_666_477_413_626_7, 0.222_381_034_453_374_47),
    (0.960_289_856_497_536_3, 0.101_228_536_290_376_26),
];

impl ProfileSegment {
    fn nurbs(&self) -> Option<Nurbs> {
        match self {
            ProfileSegment::Spline { control_points, knots, degree, weights } => {
                Some(Nurbs { degree: *degree, knots: knots.clone(), ctrl: control_points.clone(), weights: weights.clone() })
            }
            _ => None,
        }
    }

    /// Point at normalized position `s in [0,1]` along the segment direction.
    pub fn point_at(&self, s: f64) -> DVec2 {
        self.ders_at(s).0
    }

    /// Point and derivative w.r.t. the native parameter at normalized `s`;
    /// also returns d(param)/ds.
    fn ders_at(&self, s: f64) -> (DVec2, DVec2, f64) {
        match self {
            ProfileSegment::Line { a, b } => (*a + (*b - *a) * s, *b - *a, 1.0),
            ProfileSegment::Arc { center, radius, start_angle, end_angle, .. } => {
                let t = start_angle + (end_angle - start_angle) * s;
                let (sn, cs) = t.sin_cos();
                (*center + DVec2::new(cs, sn) * *radius, DVec2::new(-sn, cs) * *radius, end_angle - start_angle)
            }
            ProfileSegment::EllipseArc { center, major_axis, major_radius, minor_radius, start_param, end_param, .. } => {
                let t = start_param + (end_param - start_param) * s;
                let m = *major_axis * *major_radius;
                let n = major_axis.perp() * *minor_radius;
                let (sn, cs) = t.sin_cos();
                (*center + m * cs + n * sn, -m * sn + n * cs, end_param - start_param)
            }
            ProfileSegment::Spline { .. } => {
                let nb = self.nurbs().unwrap();
                let (a, b) = nb.domain();
                let d = nb.eval_ders(a + (b - a) * s);
                (d[0], d[1], b - a)
            }
        }
    }

    pub fn start(&self) -> DVec2 {
        self.point_at(0.0)
    }
    pub fn end(&self) -> DVec2 {
        self.point_at(1.0)
    }

    /// Same segment traversed in the opposite direction.
    pub fn reversed(&self) -> ProfileSegment {
        match self {
            ProfileSegment::Line { a, b } => ProfileSegment::Line { a: *b, b: *a },
            ProfileSegment::Arc { center, radius, start_angle, end_angle, ccw } => {
                ProfileSegment::Arc { center: *center, radius: *radius, start_angle: *end_angle, end_angle: *start_angle, ccw: !ccw }
            }
            ProfileSegment::EllipseArc { center, major_axis, major_radius, minor_radius, start_param, end_param, ccw } => {
                ProfileSegment::EllipseArc {
                    center: *center,
                    major_axis: *major_axis,
                    major_radius: *major_radius,
                    minor_radius: *minor_radius,
                    start_param: *end_param,
                    end_param: *start_param,
                    ccw: !ccw,
                }
            }
            ProfileSegment::Spline { .. } => {
                let r = self.nurbs().unwrap().reversed();
                ProfileSegment::Spline { control_points: r.ctrl, knots: r.knots, degree: r.degree, weights: r.weights }
            }
        }
    }

    /// Polyline with chordal deviation `<= tol` (includes both ends).
    pub fn tessellate(&self, tol: f64) -> Vec<DVec2> {
        match self {
            ProfileSegment::Line { a, b } => vec![*a, *b],
            ProfileSegment::Spline { .. } => {
                let pts = Curve2::Nurbs(self.nurbs().unwrap()).tessellate(tol);
                pts
            }
            ProfileSegment::Arc { radius, start_angle, end_angle, .. } => {
                let n = segs(*radius, (end_angle - start_angle).abs(), tol);
                (0..=n).map(|i| self.point_at(i as f64 / n as f64)).collect()
            }
            ProfileSegment::EllipseArc { major_radius, minor_radius, start_param, end_param, .. } => {
                let n = segs(major_radius.max(*minor_radius), (end_param - start_param).abs(), tol).max(8);
                (0..=n).map(|i| self.point_at(i as f64 / n as f64)).collect()
            }
        }
    }

    /// Contribution `1/2 * integral (x dy - y dx)` to the enclosed signed area.
    pub fn area_term(&self) -> f64 {
        match self {
            ProfileSegment::Line { a, b } => 0.5 * a.perp_dot(*b),
            ProfileSegment::Arc { center, radius, start_angle, end_angle, .. } => {
                let (t0, t1) = (*start_angle, *end_angle);
                0.5 * (radius * radius * (t1 - t0) + radius * (center.x * (t1.sin() - t0.sin()) - center.y * (t1.cos() - t0.cos())))
            }
            _ => {
                let pieces = match self {
                    ProfileSegment::Spline { knots, .. } => (knots.len() * 2).clamp(8, 400),
                    _ => 16,
                };
                let mut acc = 0.0;
                for k in 0..pieces {
                    let (s0, s1) = (k as f64 / pieces as f64, (k + 1) as f64 / pieces as f64);
                    for (x, w) in GL8 {
                        let s = 0.5 * (s0 + s1) + 0.5 * (s1 - s0) * x;
                        let (p, d, dp) = self.ders_at(s);
                        acc += w * 0.5 * (s1 - s0) * 0.5 * p.perp_dot(d) * dp;
                    }
                }
                acc
            }
        }
    }

    /// Unoriented curve geometry of the segment.
    pub fn to_curve(&self) -> Curve2 {
        match self {
            ProfileSegment::Line { a, b } => Curve2::Line { a: *a, b: *b },
            ProfileSegment::Arc { center, radius, start_angle, end_angle, .. } => Curve2::Arc {
                center: *center,
                radius: *radius,
                start_angle: start_angle.min(*end_angle),
                end_angle: start_angle.max(*end_angle),
            },
            ProfileSegment::EllipseArc { center, major_axis, major_radius, minor_radius, start_param, end_param, .. } => Curve2::EllipticalArc {
                center: *center,
                major: *major_axis * *major_radius,
                minor: *minor_radius,
                start_param: start_param.min(*end_param),
                end_param: start_param.max(*end_param),
            },
            ProfileSegment::Spline { .. } => Curve2::Nurbs(self.nurbs().unwrap()),
        }
    }
}

fn segs(r: f64, sweep: f64, tol: f64) -> usize {
    let r = r.max(1e-300);
    let x = (1.0 - (tol / r).min(1.0)).clamp(-1.0, 1.0);
    let step = (2.0 * x.acos()).max(1e-4);
    ((sweep / step).ceil() as usize).clamp(1, 4096)
}

/// Which entity a segment comes from and whether it runs in the entity's
/// natural direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SegmentSource {
    pub entity: EntityId,
    pub forward: bool,
}

/// A closed loop of segments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileLoop {
    pub segments: Vec<ProfileSegment>,
    pub sources: Vec<SegmentSource>,
}

impl ProfileLoop {
    /// Signed area (positive for CCW loops).
    pub fn signed_area(&self) -> f64 {
        self.segments.iter().map(|s| s.area_term()).sum()
    }

    pub fn tessellate(&self, tol: f64) -> Vec<DVec2> {
        let mut out = Vec::new();
        for s in &self.segments {
            let pts = s.tessellate(tol);
            let skip = if out.is_empty() { 0 } else { 1 };
            out.extend(pts.into_iter().skip(skip));
        }
        if out.len() > 1 && (out[0] - *out.last().unwrap()).length() < 1e-15 {
            out.pop();
        }
        out
    }

    pub fn bbox(&self) -> BBox {
        let mut b = BBox::EMPTY;
        for s in &self.segments {
            b = b.union(&s.to_curve().bbox());
        }
        b
    }

    /// Winding-number containment (points on the boundary are ambiguous).
    pub fn contains(&self, p: DVec2) -> bool {
        let tol = self.bbox().size().max_element().max(1e-12) * 1e-5;
        winding(&self.tessellate(tol), p) != 0
    }
}

fn winding(poly: &[DVec2], p: DVec2) -> i32 {
    let n = poly.len();
    let mut w = 0;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        if a.y <= p.y {
            if b.y > p.y && (b - a).perp_dot(p - a) > 0.0 {
                w += 1;
            }
        } else if b.y <= p.y && (b - a).perp_dot(p - a) < 0.0 {
            w -= 1;
        }
    }
    w
}

/// Stable region key: the oriented set of bounding entities plus an ordinal
/// that disambiguates regions with identical boundary sets (ordered by
/// position, left-to-right then bottom-to-top).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RegionId {
    pub boundary: Vec<SegmentSource>,
    pub ordinal: u32,
}

impl RegionId {
    /// Distinct bounding entity ids.
    pub fn entities(&self) -> Vec<EntityId> {
        let mut v: Vec<EntityId> = self.boundary.iter().map(|s| s.entity).collect();
        v.dedup();
        v
    }
}

/// A closed area bounded by sketch curves: CCW outer loop + CW holes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub id: RegionId,
    pub outer: ProfileLoop,
    pub holes: Vec<ProfileLoop>,
}

impl Region {
    /// Net area (outer minus holes).
    pub fn area(&self) -> f64 {
        self.outer.signed_area() + self.holes.iter().map(|h| h.signed_area()).sum::<f64>()
    }
    pub fn contains(&self, p: DVec2) -> bool {
        self.outer.contains(p) && !self.holes.iter().any(|h| h.contains(p))
    }
    pub fn bbox(&self) -> BBox {
        self.outer.bbox()
    }
    pub fn entities(&self) -> Vec<EntityId> {
        self.id.entities()
    }
    /// A point strictly inside the region (useful for picking / labels).
    pub fn interior_point(&self) -> DVec2 {
        let bb = self.bbox();
        let tol = bb.size().max_element().max(1e-12) * 1e-5;
        let outer = self.outer.tessellate(tol);
        let holes: Vec<Vec<DVec2>> = self.holes.iter().map(|h| h.tessellate(tol)).collect();
        let inside = |p: DVec2| winding(&outer, p) != 0 && !holes.iter().any(|h| winding(h, p) != 0);
        // scanline through several heights: midpoint of the widest inside span
        let mut best = (f64::NEG_INFINITY, bb.center());
        for k in 1..16 {
            let y = bb.min.y + bb.size().y * k as f64 / 16.0;
            let mut xs: Vec<f64> = Vec::new();
            for poly in std::iter::once(&outer).chain(holes.iter()) {
                let n = poly.len();
                for i in 0..n {
                    let (a, b) = (poly[i], poly[(i + 1) % n]);
                    if (a.y <= y) != (b.y <= y) {
                        xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
                    }
                }
            }
            xs.sort_by(f64::total_cmp);
            for w in xs.windows(2) {
                let p = DVec2::new(0.5 * (w[0] + w[1]), y);
                if w[1] - w[0] > best.0 && inside(p) {
                    best = (w[1] - w[0], p);
                }
            }
        }
        best.1
    }
}

/// A connected chain of curves (open or closed).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wire {
    pub segments: Vec<ProfileSegment>,
    pub sources: Vec<SegmentSource>,
    pub closed: bool,
}

fn segment_of(c: &Curve2, t0: f64, t1: f64, forward: bool) -> ProfileSegment {
    let s = match c {
        Curve2::Line { .. } => ProfileSegment::Line { a: c.point(t0), b: c.point(t1) },
        Curve2::Circle { center, radius } | Curve2::Arc { center, radius, .. } => {
            ProfileSegment::Arc { center: *center, radius: *radius, start_angle: t0, end_angle: t1, ccw: true }
        }
        Curve2::Ellipse { center, major, minor } | Curve2::EllipticalArc { center, major, minor, .. } => ProfileSegment::EllipseArc {
            center: *center,
            major_axis: major.normalize_or(DVec2::X),
            major_radius: major.length(),
            minor_radius: *minor,
            start_param: t0,
            end_param: t1,
            ccw: true,
        },
        Curve2::Nurbs(n) => {
            let sub = n.sub_curve(t0, t1);
            ProfileSegment::Spline { control_points: sub.ctrl, knots: sub.knots, degree: sub.degree, weights: sub.weights }
        }
    };
    if forward { s } else { s.reversed() }
}

struct Edge {
    curve: usize,
    t0: f64,
    t1: f64,
    v0: usize,
    v1: usize,
}

fn tolerance(curves: &[(EntityId, Curve2)]) -> f64 {
    let mut bb = BBox::EMPTY;
    for (_, c) in curves {
        bb = bb.union(&c.bbox());
    }
    1e-9 * bb.size().max_element().max(1.0)
}

fn sketch_curves(sk: &Sketch) -> Vec<(EntityId, Curve2)> {
    let mut out = Vec::new();
    for e in sk.entities() {
        if e.construction || e.kind() == EntityKind::Point {
            continue;
        }
        let Some(c) = sk.curve(e.id) else { continue };
        let degenerate = match &c {
            Curve2::Line { a, b } => (*b - *a).length() < 1e-12,
            Curve2::Circle { radius, .. } | Curve2::Arc { radius, .. } => *radius < 1e-12,
            Curve2::Ellipse { major, minor, .. } | Curve2::EllipticalArc { major, minor, .. } => major.length() < 1e-12 || *minor < 1e-12,
            Curve2::Nurbs(n) => !n.is_valid(),
        };
        if !degenerate {
            out.push((e.id, c));
        }
    }
    out
}

/// Merges nearby points; returns the cluster index of every point and the cluster positions.
fn merge_points(pts: &[DVec2], tol: f64) -> (Vec<usize>, Vec<DVec2>) {
    let n = pts.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|a, b| pts[*a].x.total_cmp(&pts[*b].x));
    let mut uf: Vec<usize> = (0..n).collect();
    fn f(uf: &mut [usize], mut i: usize) -> usize {
        while uf[i] != i {
            uf[i] = uf[uf[i]];
            i = uf[i];
        }
        i
    }
    for a in 0..n {
        let i = idx[a];
        for &j in idx.iter().skip(a + 1) {
            if pts[j].x - pts[i].x > tol {
                break;
            }
            if (pts[j] - pts[i]).length() <= tol {
                let (ri, rj) = (f(&mut uf, i), f(&mut uf, j));
                if ri != rj {
                    uf[ri.max(rj)] = ri.min(rj);
                }
            }
        }
    }
    let mut cluster = vec![usize::MAX; n];
    let mut pos = Vec::new();
    for i in 0..n {
        let r = f(&mut uf, i);
        if cluster[r] == usize::MAX {
            cluster[r] = pos.len();
            pos.push(pts[r]);
        }
        cluster[i] = cluster[r];
    }
    (cluster, pos)
}

fn build_edges(curves: &[(EntityId, Curve2)], tol: f64) -> (Vec<Edge>, Vec<DVec2>) {
    let n = curves.len();
    let bbs: Vec<BBox> = curves.iter().map(|(_, c)| c.bbox().expanded(tol)).collect();
    let mut splits: Vec<Vec<f64>> = vec![Vec::new(); n];
    for i in 0..n {
        for j in i + 1..n {
            if !bbs[i].intersects(&bbs[j]) {
                continue;
            }
            for x in intersect(&curves[i].1, &curves[j].1, tol) {
                splits[i].push(x.t1);
                splits[j].push(x.t2);
            }
        }
    }
    // raw edges
    let mut raw: Vec<(usize, f64, f64)> = Vec::new();
    for (i, (_, c)) in curves.iter().enumerate() {
        let (a, b) = c.domain();
        let periodic = c.is_periodic();
        let mut ps: Vec<f64> = splits[i]
            .iter()
            .map(|&t| if periodic { a + (t - a).rem_euclid(b - a) } else { t.clamp(a, b) })
            .collect();
        if !periodic {
            ps.push(a);
            ps.push(b);
        }
        ps.sort_by(f64::total_cmp);
        let mut uniq: Vec<f64> = Vec::new();
        for t in ps {
            if let Some(&last) = uniq.last() {
                if (c.point(t) - c.point(last)).length() <= tol && (t - last).abs() < 0.5 * (b - a) {
                    continue;
                }
            }
            uniq.push(t);
        }
        if periodic {
            if uniq.len() > 1 && (c.point(uniq[0]) - c.point(*uniq.last().unwrap())).length() <= tol {
                uniq.pop();
            }
            match uniq.len() {
                0 => uniq = vec![a, a + PI],
                1 => uniq.push(uniq[0] + PI),
                _ => {}
            }
            let period = b - a;
            for k in 0..uniq.len() {
                let t0 = uniq[k];
                let t1 = if k + 1 < uniq.len() { uniq[k + 1] } else { uniq[0] + period };
                raw.push((i, t0, t1));
            }
        } else {
            for w in uniq.windows(2) {
                raw.push((i, w[0], w[1]));
            }
        }
    }
    // closed non-periodic edges (start == end) are split in half
    let mut raw2 = Vec::new();
    for (i, t0, t1) in raw {
        let c = &curves[i].1;
        if (c.point(t0) - c.point(t1)).length() <= tol {
            let tm = 0.5 * (t0 + t1);
            if (c.point(tm) - c.point(t0)).length() <= 4.0 * tol {
                continue; // degenerate
            }
            raw2.push((i, t0, tm));
            raw2.push((i, tm, t1));
        } else {
            raw2.push((i, t0, t1));
        }
    }
    let mut pts = Vec::with_capacity(raw2.len() * 2);
    for &(i, t0, t1) in &raw2 {
        pts.push(curves[i].1.point(t0));
        pts.push(curves[i].1.point(t1));
    }
    let (cl, pos) = merge_points(&pts, tol * 4.0);
    let mut edges: Vec<Edge> = Vec::new();
    for (k, &(i, t0, t1)) in raw2.iter().enumerate() {
        let (v0, v1) = (cl[2 * k], cl[2 * k + 1]);
        if v0 == v1 {
            continue;
        }
        let mid = curves[i].1.point(0.5 * (t0 + t1));
        let dup = edges.iter().any(|e| {
            ((e.v0 == v0 && e.v1 == v1) || (e.v0 == v1 && e.v1 == v0))
                && (curves[e.curve].1.point(0.5 * (e.t0 + e.t1)) - mid).length() <= tol * 100.0
        });
        if !dup {
            edges.push(Edge { curve: i, t0, t1, v0, v1 });
        }
    }
    (edges, pos)
}

struct Arrangement {
    curves: Vec<(EntityId, Curve2)>,
    edges: Vec<Edge>,
    verts: Vec<DVec2>,
}

impl Arrangement {
    fn new(sk: &Sketch) -> Arrangement {
        let curves = sketch_curves(sk);
        let tol = tolerance(&curves);
        let (edges, verts) = build_edges(&curves, tol);
        Arrangement { curves, edges, verts }
    }

    /// Half-edge h = 2*e (forward) or 2*e+1 (reversed): origin, target.
    fn he_verts(&self, h: usize) -> (usize, usize) {
        let e = &self.edges[h / 2];
        if h % 2 == 0 { (e.v0, e.v1) } else { (e.v1, e.v0) }
    }

    fn he_segment(&self, h: usize) -> (ProfileSegment, SegmentSource) {
        let e = &self.edges[h / 2];
        let (id, c) = &self.curves[e.curve];
        let fwd = h % 2 == 0;
        (segment_of(c, e.t0, e.t1, fwd), SegmentSource { entity: *id, forward: fwd })
    }

    /// Outgoing direction angle and signed curvature of a half-edge.
    fn he_dir(&self, h: usize) -> (f64, f64) {
        let e = &self.edges[h / 2];
        let c = &self.curves[e.curve].1;
        let (t, sign) = if h % 2 == 0 { (e.t0, 1.0) } else { (e.t1, -1.0) };
        let d = c.ders(t);
        let mut dir = d[1] * sign;
        if dir.length() < 1e-12 * c.size().max(1e-12) {
            let tt = if h % 2 == 0 { e.t0 + 1e-6 * (e.t1 - e.t0) } else { e.t1 - 1e-6 * (e.t1 - e.t0) };
            dir = c.point(tt) - d[0];
        }
        (dir.y.atan2(dir.x), c.curvature(t) * sign)
    }

    fn regions(&self) -> Vec<Region> {
        let ne = self.edges.len();
        let nv = self.verts.len();
        // prune dangling edges
        let mut alive = vec![true; ne];
        let mut deg = vec![0usize; nv];
        for e in &self.edges {
            deg[e.v0] += 1;
            deg[e.v1] += 1;
        }
        loop {
            let mut changed = false;
            for (k, e) in self.edges.iter().enumerate() {
                if alive[k] && (deg[e.v0] == 1 || deg[e.v1] == 1) {
                    alive[k] = false;
                    deg[e.v0] -= 1;
                    deg[e.v1] -= 1;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        // sorted outgoing half-edges per vertex
        let mut out: Vec<Vec<(f64, f64, usize)>> = vec![Vec::new(); nv];
        for (k, a) in alive.iter().enumerate() {
            if !*a {
                continue;
            }
            for h in [2 * k, 2 * k + 1] {
                let (ang, curv) = self.he_dir(h);
                let mut a = ang.rem_euclid(TAU);
                if a >= TAU - 1e-9 {
                    a -= TAU;
                }
                out[self.he_verts(h).0].push((a, curv, h));
            }
        }
        let mut pos = vec![usize::MAX; 2 * ne];
        for list in out.iter_mut() {
            list.sort_by(|x, y| {
                if (x.0 - y.0).abs() <= 1e-9 { x.1.total_cmp(&y.1) } else { x.0.total_cmp(&y.0) }
            });
            for (i, &(_, _, h)) in list.iter().enumerate() {
                pos[h] = i;
            }
        }
        let next = |h: usize| -> usize {
            let twin = h ^ 1;
            let v = self.he_verts(h).1;
            let list = &out[v];
            let k = pos[twin];
            list[(k + list.len() - 1) % list.len()].2
        };
        // components (union-find on vertices over alive edges)
        let mut comp: Vec<usize> = (0..nv).collect();
        fn f(c: &mut [usize], mut i: usize) -> usize {
            while c[i] != i {
                c[i] = c[c[i]];
                i = c[i];
            }
            i
        }
        for (k, e) in self.edges.iter().enumerate() {
            if alive[k] {
                let (a, b) = (f(&mut comp, e.v0), f(&mut comp, e.v1));
                if a != b {
                    comp[a.max(b)] = a.min(b);
                }
            }
        }
        // trace faces
        let mut seen = vec![false; 2 * ne];
        struct Cycle {
            hes: Vec<usize>,
            area: f64,
            comp: usize,
            lp: ProfileLoop,
        }
        let mut cycles: Vec<Cycle> = Vec::new();
        for start in 0..2 * ne {
            if !alive[start / 2] || seen[start] {
                continue;
            }
            let mut hes = Vec::new();
            let mut h = start;
            let mut guard = 0;
            loop {
                seen[h] = true;
                hes.push(h);
                h = next(h);
                guard += 1;
                if h == start || guard > 4 * ne + 4 {
                    break;
                }
            }
            let mut lp = ProfileLoop { segments: Vec::new(), sources: Vec::new() };
            for &h in &hes {
                let (s, src) = self.he_segment(h);
                lp.segments.push(s);
                lp.sources.push(src);
            }
            let area = lp.signed_area();
            let c = f(&mut comp, self.he_verts(hes[0]).0);
            cycles.push(Cycle { hes, area, comp: c, lp });
        }
        let faces: Vec<usize> = (0..cycles.len()).filter(|&i| cycles[i].area > 0.0).collect();
        let outers: Vec<usize> = (0..cycles.len()).filter(|&i| cycles[i].area <= 0.0).collect();
        let mut holes: Vec<Vec<usize>> = vec![Vec::new(); cycles.len()];
        let bbox_tol = 1e-5;
        let polys: Vec<Option<Vec<DVec2>>> = cycles
            .iter()
            .map(|c| {
                if c.area > 0.0 {
                    let t = c.lp.bbox().size().max_element().max(1e-12) * bbox_tol;
                    Some(c.lp.tessellate(t))
                } else {
                    None
                }
            })
            .collect();
        for &o in &outers {
            if cycles[o].area.abs() <= 0.0 {
                continue;
            }
            let sample = self.verts[self.he_verts(cycles[o].hes[0]).0];
            let mut best: Option<usize> = None;
            for &fi in &faces {
                if cycles[fi].comp == cycles[o].comp {
                    continue;
                }
                if best.is_some_and(|b| cycles[b].area <= cycles[fi].area) {
                    continue;
                }
                if winding(polys[fi].as_ref().unwrap(), sample) != 0 {
                    best = Some(fi);
                }
            }
            if let Some(b) = best {
                holes[b].push(o);
            }
        }
        let mut regions: Vec<(Region, DVec2)> = Vec::new();
        for &fi in &faces {
            let outer = cycles[fi].lp.clone();
            let hl: Vec<ProfileLoop> = holes[fi].iter().map(|&o| cycles[o].lp.clone()).collect();
            let mut boundary: Vec<SegmentSource> = outer.sources.clone();
            for h in &hl {
                boundary.extend(h.sources.iter().copied());
            }
            boundary.sort();
            boundary.dedup();
            let key = outer.bbox().center();
            regions.push((Region { id: RegionId { boundary, ordinal: 0 }, outer, holes: hl }, key));
        }
        // ordinals for identical boundary sets
        regions.sort_by(|a, b| a.0.id.boundary.cmp(&b.0.id.boundary).then(a.1.x.total_cmp(&b.1.x)).then(a.1.y.total_cmp(&b.1.y)));
        for i in 1..regions.len() {
            if regions[i].0.id.boundary == regions[i - 1].0.id.boundary {
                regions[i].0.id.ordinal = regions[i - 1].0.id.ordinal + 1;
            }
        }
        regions.into_iter().map(|r| r.0).collect()
    }
}

fn wires(sk: &Sketch) -> Vec<Wire> {
    let curves = sketch_curves(sk);
    let tol = tolerance(&curves);
    let mut out = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    for (i, (id, c)) in curves.iter().enumerate() {
        if c.is_periodic() || c.is_closed() {
            let (a, b) = c.domain();
            out.push(Wire { segments: vec![segment_of(c, a, b, true)], sources: vec![SegmentSource { entity: *id, forward: true }], closed: true });
        } else {
            open.push(i);
        }
    }
    let mut pts = Vec::new();
    for &i in &open {
        pts.push(curves[i].1.start_point());
        pts.push(curves[i].1.end_point());
    }
    let (cl, pos) = merge_points(&pts, tol * 4.0);
    let mut inc: Vec<Vec<(usize, bool)>> = vec![Vec::new(); pos.len()];
    for (k, _) in open.iter().enumerate() {
        inc[cl[2 * k]].push((k, true));
        inc[cl[2 * k + 1]].push((k, false));
    }
    let mut used = vec![false; open.len()];
    let walk = |start_v: usize, first: (usize, bool), used: &mut Vec<bool>| -> Wire {
        let mut segs = Vec::new();
        let mut srcs = Vec::new();
        let (mut k, mut at_start) = first;
        let v = loop {
            used[k] = true;
            let (id, c) = &curves[open[k]];
            let (a, b) = c.domain();
            segs.push(segment_of(c, a, b, at_start));
            srcs.push(SegmentSource { entity: *id, forward: at_start });
            let v = if at_start { cl[2 * k + 1] } else { cl[2 * k] };
            if inc[v].len() != 2 {
                break v;
            }
            let Some(&(k2, s2)) = inc[v].iter().find(|(kk, _)| !used[*kk]) else { break v };
            k = k2;
            at_start = s2;
        };
        let closed = v == start_v && segs.len() > 1;
        Wire { segments: segs, sources: srcs, closed }
    };
    for v in 0..pos.len() {
        if inc[v].len() == 2 {
            continue;
        }
        for idx in 0..inc[v].len() {
            let (k, s) = inc[v][idx];
            if !used[k] {
                out.push(walk(v, (k, s), &mut used));
            }
        }
    }
    for k in 0..open.len() {
        if !used[k] {
            let v = cl[2 * k];
            out.push(walk(v, (k, true), &mut used));
        }
    }
    out
}

impl Sketch {
    /// All bounded regions formed by non-construction curves (including
    /// areas created by overlapping / intersecting curves), sorted by id.
    pub fn regions(&self) -> Vec<Region> {
        Arrangement::new(self).regions()
    }

    /// Connected chains of non-construction curves (split where more than two
    /// curves meet). Closed chains have `closed == true`.
    pub fn wires(&self) -> Vec<Wire> {
        wires(self)
    }

    /// Resolves a region id after edits: exact match first, otherwise the
    /// region whose boundary shares the most oriented entities.
    pub fn find_region(&self, id: &RegionId) -> Option<Region> {
        let regions = self.regions();
        if let Some(r) = regions.iter().find(|r| &r.id == id) {
            return Some(r.clone());
        }
        let mut best: Option<(f64, usize)> = None;
        for (i, r) in regions.iter().enumerate() {
            let common = r.id.boundary.iter().filter(|s| id.boundary.contains(s)).count() as f64;
            let union = (r.id.boundary.len() + id.boundary.len()) as f64 - common;
            let score = if union > 0.0 { common / union } else { 0.0 };
            if score > 0.0 && best.is_none_or(|b| score > b.0) {
                best = Some((score, i));
            }
        }
        best.map(|b| regions[b.1].clone())
    }

    /// The smallest region containing point `p` (region picking).
    pub fn region_at(&self, p: DVec2) -> Option<Region> {
        self.regions()
            .into_iter()
            .filter(|r| r.contains(p))
            .min_by(|a, b| a.area().total_cmp(&b.area()))
    }
}
