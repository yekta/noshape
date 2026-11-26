//! Free-form faces' sections by marching: where two surfaces, each over its face's parameter box, meet. Seeds where
//! the boxes' cells overlap (both boxes split until small, a Newton step from each overlapping pair's middles onto
//! both surfaces), each traced both ways along the cross product of the two normals, a step's end brought back onto
//! both surfaces in the plane across its tangent; to where either box ends, or round to the start. Each branch is
//! then a cubic through its stations (their tangents: C1), with its curves on both surfaces likewise.
//! (Not used by the Boolean yet: `brep_replay::free_sections_vs_occt` checks it against OCCT's sections.)

use crate::freeform::FreeSurface;
use crate::nurbs::NurbsCurve;
use glam::{DVec2, DVec3};

/// A surface over a parameter box (u0, u1, v0, v1): a free-form face's, its edges' curves on it span; and those
/// curves ((u, v, 0) and their ranges).
#[derive(Clone, Copy)]
pub struct Domain<'a> {
    pub s: &'a FreeSurface,
    pub uv: [f64; 4],
    pub edges: &'a [FaceEdge],
}

/// One of a face's edges: its curve on the face ((u, v, 0)) over `t0`..`t1`, and its tolerance.
#[derive(Clone, Debug)]
pub struct FaceEdge {
    pub c: NurbsCurve,
    pub t0: f64,
    pub t1: f64,
    pub tol: f64,
}

/// A free-form face as [`face_domain`] has it: what a [`Domain`] borrows.
pub struct Face {
    pub s: FreeSurface,
    pub uv: [f64; 4],
    pub edges: Vec<FaceEdge>,
}

impl Face {
    pub fn domain(&self) -> Domain<'_> {
        Domain { s: &self.s, uv: self.uv, edges: &self.edges }
    }
}

impl Domain<'_> {
    fn size(&self) -> f64 {
        let (lo, hi) = self.box_of((self.uv[0], self.uv[1]), (self.uv[2], self.uv[3]));
        (hi - lo).length()
    }

    /// A box holding the surface over `u` × `v`: its points on a 5 × 5 grid, grown by twice the most a quadratic
    /// through them may miss (the second derivatives at the middle); within the control points' hull where that is
    /// smaller.
    fn box_of(&self, u: (f64, f64), v: (f64, f64)) -> (DVec3, DVec3) {
        let us: Vec<f64> = (0..5).map(|k| u.0 + (u.1 - u.0) * k as f64 / 4.0).collect();
        let vs: Vec<f64> = (0..5).map(|k| v.0 + (v.1 - v.0) * k as f64 / 4.0).collect();
        let pts = self.s.grid(&us, &vs);
        let (mut lo, mut hi) = pts.iter().fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(l, h), &q| (l.min(q), h.max(q)));
        let [_, _, _, suu, suv, svv] = self.s.derivs(0.5 * (u.0 + u.1), 0.5 * (v.0 + v.1));
        let (du, dv) = ((u.1 - u.0) / 4.0, (v.1 - v.0) / 4.0);
        let sag = 2.0 * (du * du * suu.length() + 2.0 * du * dv * suv.length() + dv * dv * svv.length()) / 8.0;
        lo -= DVec3::splat(sag);
        hi += DVec3::splat(sag);
        if let Some((l, h)) = self.s.hull_box(u, v) {
            lo = lo.max(l);
            hi = hi.min(h);
        }
        (lo, hi)
    }
}

/// Face `face` of `body` as a free-form surface, the parameter box its edges' curves on it span (grown by a
/// hundredth, within the surface's own range), as the Boolean has it, and those curves; None for a plane, cylinder or
/// cone, or a face with no curves on it.
pub fn face_domain(body: &crate::Body, face: usize) -> Option<Face> {
    use crate::Surface;
    let s = match body.faces.get(face)?.surface {
        Surface::Nurbs { index } => FreeSurface::Nurbs(body.nurbs_surfaces[index as usize].clone()),
        Surface::Torus { origin, xdir, ydir, zdir, major, minor } => FreeSurface::Torus { o: origin, x: xdir, y: ydir, z: zdir, major, minor },
        Surface::Sphere { origin, xdir, ydir, zdir, radius } => FreeSurface::Sphere { o: origin, x: xdir, y: ydir, z: zdir, r: radius },
        Surface::Extrusion { curve, dir } => FreeSurface::Extrusion { c: body.nurbs_curves[curve as usize].clone(), d: dir },
        Surface::Revolution { curve, origin, axis } => FreeSurface::Revolution { c: body.nurbs_curves[curve as usize].clone(), o: origin, a: axis },
        _ => return None,
    };
    let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    let mut edges = Vec::new();
    for p in body.pcurves.iter().filter(|p| p.face as usize == face) {
        let c = &body.nurbs_curves[p.curve as usize];
        let tol = body.edge_tol.get(p.edge as usize).copied().unwrap_or(1e-7);
        edges.push(FaceEdge { c: c.clone(), t0: p.t0.min(p.t1), t1: p.t0.max(p.t1), tol });
        for k in 0..=32 {
            let q = c.at(p.t0 + (p.t1 - p.t0) * k as f64 / 32.0).truncate();
            lo = lo.min(q);
            hi = hi.max(q);
        }
    }
    let grow = (hi - lo) * 0.01 + DVec2::splat(1e-9);
    let (mut lo, mut hi) = (lo - grow, hi + grow);
    for (k, r) in s.range().into_iter().enumerate() {
        if let Some((a, b)) = r {
            lo[k] = lo[k].max(a);
            hi[k] = hi[k].min(b);
        }
    }
    (lo.x <= hi.x && lo.y <= hi.y && lo.is_finite() && hi.is_finite()).then_some(Face { s, uv: [lo.x, hi.x, lo.y, hi.y], edges })
}

/// A point of a section: where it is and its parameters on either surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Station {
    pub p: DVec3,
    pub a: DVec2,
    pub b: DVec2,
}

/// One section's stations in order (a closed one's last the first again).
#[derive(Clone, Debug)]
pub struct Branch {
    pub stations: Vec<Station>,
    pub closed: bool,
}

/// A section as a curve and its curves on both surfaces (same parameter), and how far the curve may be from either
/// surface.
#[derive(Clone, Debug)]
pub struct Section {
    pub curve: NurbsCurve,
    pub on: [NurbsCurve; 2],
    pub t0: f64,
    pub t1: f64,
    pub closed: bool,
    pub accuracy: f64,
}

/// Why no sections were made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fail {
    /// The two surfaces touch (their normals parallel where they meet): OCCT's (lead 11-15).
    Tangent,
    /// The faces lie on one surface where they meet (the Boolean's same-domain faces: OCCT's).
    Coincident,
    /// A section runs along an edge of each face (edges on edges: OCCT's, whose sections there follow its edges').
    OnEdges,
    /// Too many overlapping cells: the surfaces lie close along a stretch (or coincide).
    Crowded,
    /// A step couldn't be brought back onto both surfaces however short.
    Lost,
    /// A branch's cubic didn't come within [`FIT`] of both surfaces in 12 rounds of splitting.
    Unfit,
}

/// Pieces shorter than this, of the boxes' size, aren't split further (beside a surface's degenerate spot its
/// parameters' tangent grows without bound).
const SHORT: f64 = 1e-5;
/// How close a point must come to both surfaces, of the two boxes' size.
const EPS: f64 = 1e-12;
/// Cell pairs the seed search looks at, at most (256 cells a face: 65536 pairs at the finest, each reached in a few
/// splits).
const VISITS: usize = 1 << 20;
/// Below this sine between the normals the surfaces are taken to touch.
const TANGENT_SIN: f64 = 1e-4;
/// A section along which the normals come within this sine of parallel anywhere: the faces nearly touch there, and
/// OCCT's sections (by its marching) come and go (lead 11-15: tangent contact is OCCT's).
const TOUCH_SIN: f64 = 1e-2;
/// How near the second surface the first must stay around a point where they touch to lie on it (meters: OCCT's
/// confusion).
const COINCIDE: f64 = 1e-7;
/// Within this of a shared edge a section is the edge's, not a section (meters: OCCT's `off_shared_edges`).
const ALONG: f64 = 1e-5;

/// One face's edge in space: its curve on the face's surface, sampled.
struct Edge3<'a> {
    d: Domain<'a>,
    c: &'a NurbsCurve,
    tol: f64,
    ts: Vec<f64>,
    pts: Vec<DVec3>,
    lo: DVec3,
    hi: DVec3,
}

impl<'a> Edge3<'a> {
    fn new(d: Domain<'a>, e: &'a FaceEdge) -> Self {
        let (c, t0, t1) = (&e.c, e.t0, e.t1);
        let ts: Vec<f64> = (0..=32).map(|k| t0 + (t1 - t0) * k as f64 / 32.0).collect();
        let pts: Vec<DVec3> = ts.iter().map(|&t| d.s.at(c.at(t).x, c.at(t).y)).collect();
        let (mut lo, mut hi) = pts.iter().fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(l, h), &q| (l.min(q), h.max(q)));
        // (Grown by the most a sample's chord sags: a sixth of the longest chord.)
        let sag = pts.windows(2).map(|w| (w[1] - w[0]).length()).fold(0.0, f64::max) / 6.0;
        lo -= DVec3::splat(sag);
        hi += DVec3::splat(sag);
        Edge3 { d, c, tol: e.tol, ts, pts, lo, hi }
    }

    fn at(&self, t: f64) -> DVec3 {
        let q = self.c.at(t);
        self.d.s.at(q.x, q.y)
    }

    /// How far `p` is from the edge: from the nearest sample, a golden-section search between its neighbours.
    fn dist(&self, p: DVec3) -> f64 {
        let i = (0..self.pts.len()).min_by(|&i, &j| (self.pts[i] - p).length_squared().total_cmp(&(self.pts[j] - p).length_squared())).unwrap_or(0);
        let (mut a, mut b) = (self.ts[i.saturating_sub(1)], self.ts[(i + 1).min(self.ts.len() - 1)]);
        let g = 0.5 * (5f64.sqrt() - 1.0);
        let f = |t: f64| (self.at(t) - p).length();
        let (mut x, mut y) = (b - g * (b - a), a + g * (b - a));
        let (mut fx, mut fy) = (f(x), f(y));
        for _ in 0..60 {
            if fx < fy {
                b = y;
                (y, fy) = (x, fx);
                x = b - g * (b - a);
                fx = f(x);
            } else {
                a = x;
                (x, fx) = (y, fy);
                y = a + g * (b - a);
                fy = f(y);
            }
        }
        fx.min(fy).min((self.pts[i] - p).length())
    }

    fn near_box(&self, p: DVec3, r: f64) -> bool {
        (p - self.lo).cmpge(DVec3::splat(-r)).all() && (self.hi - p).cmpge(DVec3::splat(-r)).all()
    }
}

/// At most this turn of the tangent a step (radians).
const TURN: f64 = 0.12;
/// How far a fitted section may be from either surface, of the boxes' size.
const FIT: f64 = 1e-9;

fn solve4(m: [[f64; 4]; 4], r: [f64; 4]) -> Option<[f64; 4]> {
    let mut a = [[0.0; 5]; 4];
    for i in 0..4 {
        a[i][..4].copy_from_slice(&m[i]);
        a[i][4] = r[i];
    }
    for c in 0..4 {
        let p = (c..4).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[p][c].abs() < 1e-300 {
            return None;
        }
        a.swap(c, p);
        for i in 0..4 {
            if i != c {
                let (f, row) = (a[i][c] / a[c][c], a[c]);
                for (x, y) in a[i][c..].iter_mut().zip(&row[c..]) {
                    *x -= f * y;
                }
            }
        }
    }
    Some([a[0][4] / a[0][0], a[1][4] / a[1][1], a[2][4] / a[2][2], a[3][4] / a[3][3]])
}

/// The least-squares (du, dv) with su du + sv dv = w.
fn along(su: DVec3, sv: DVec3, w: DVec3) -> DVec2 {
    let (a, b, d) = (su.dot(su), su.dot(sv), sv.dot(sv));
    let det = a * d - b * b;
    if det.abs() < 1e-300 {
        return DVec2::ZERO;
    }
    let (x, y) = (su.dot(w), sv.dot(w));
    DVec2::new((d * x - b * y) / det, (a * y - b * x) / det)
}

/// The unit normal of `d`'s surface at `q`; where it has none (a pole, a B-spline's collapsed side: its derivatives
/// parallel), the normal a millionth of the box toward the box's middle.
fn normal(d: &Domain, q: DVec2) -> DVec3 {
    let [_, su, sv, ..] = d.s.derivs(q.x, q.y);
    let n = su.cross(sv);
    if n.length() > 1e-12 * su.length() * sv.length() {
        return n.normalize();
    }
    let mid = DVec2::new(0.5 * (d.uv[0] + d.uv[1]), 0.5 * (d.uv[2] + d.uv[3]));
    let w = DVec2::new(d.uv[1] - d.uv[0], d.uv[3] - d.uv[2]);
    let q = q + (mid - q).signum() * w * 1e-6;
    let [_, su, sv, ..] = d.s.derivs(q.x, q.y);
    su.cross(sv).normalize_or_zero()
}

struct Marcher<'a> {
    da: Domain<'a>,
    db: Domain<'a>,
    eps: f64,
    size: f64,
    edges: [Vec<Edge3<'a>>; 2],
    shared: Vec<usize>,
}

/// The first face's edges that lie on an edge of the second (at 7 points along it within both edges' tolerances and
/// OCCT's confusion, as OCCT's `off_shared_edges`).
fn shared_edges<'a>(a: &[Edge3<'a>], b: &[Edge3<'a>]) -> Vec<usize> {
    let on = |e: &Edge3, f: &Edge3| {
        let reach = e.tol + f.tol + 1e-7;
        (1..8).all(|k| {
            let p = e.at(e.ts[0] + (e.ts[e.ts.len() - 1] - e.ts[0]) * k as f64 / 8.0);
            f.near_box(p, reach) && f.dist(p) <= reach
        })
    };
    (0..a.len()).filter(|&i| b.iter().any(|f| on(&a[i], f))).collect()
}

impl<'a> Marcher<'a> {
    fn new(a: Domain<'a>, b: Domain<'a>) -> Self {
        let size = a.size().min(b.size()).max(1e-300);
        let edges: [Vec<Edge3>; 2] = [a.edges.iter().map(|e| Edge3::new(a, e)).collect(), b.edges.iter().map(|e| Edge3::new(b, e)).collect()];
        let shared = shared_edges(&edges[0], &edges[1]);
        Marcher { da: a, db: b, eps: EPS * size, size, edges, shared }
    }

    fn branches(&self) -> Result<Vec<Branch>, Fail> {
        let mut out: Vec<Branch> = Vec::new();
        for s in self.seeds()? {
            if out.iter().any(|br| on_branch(br, s.p, self.eps)) {
                continue;
            }
            // (Where the faces touch along an edge they share: the edge's.)
            if self.tangent(&s).is_none() && self.on_shared(s.p) {
                continue;
            }
            out.push(self.trace(s)?);
        }
        Ok(out)
    }
}

/// The sections of `a` and `b` within their boxes: branches of stations.
pub fn branches(a: Domain, b: Domain) -> Result<Vec<Branch>, Fail> {
    Marcher::new(a, b).branches()
}

/// Whether `p` lies on branch `br`'s polyline (within a twentieth of the segment it is beside, its chord's sag).
fn on_branch(br: &Branch, p: DVec3, eps: f64) -> bool {
    br.stations.windows(2).any(|w| {
        let (a, b) = (w[0].p, w[1].p);
        let ab = b - a;
        let l2 = ab.length_squared();
        let t = if l2 > 0.0 { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
        (a + ab * t - p).length() <= 0.05 * l2.sqrt() + 10.0 * eps
    }) || br.stations.first().is_some_and(|s| (s.p - p).length() <= 10.0 * eps)
}

impl Marcher<'_> {
    /// How far `p` is from the faces' shared edges.
    fn off(&self, p: DVec3) -> f64 {
        self.shared.iter().map(|&i| &self.edges[0][i]).filter(|e| e.near_box(p, ALONG)).map(|e| e.dist(p)).fold(f64::INFINITY, f64::min)
    }

    /// Whether `sec` runs along an edge of each face over its length: all of 31 points inside it within [`ALONG`] of
    /// both (where OCCT keeps or drops such a section by whether its own marching finds it).
    fn along_both(&self, sec: &Section) -> bool {
        let near = |es: &[Edge3], p: DVec3| es.iter().any(|e| e.near_box(p, ALONG) && e.dist(p) <= ALONG);
        (1..32).map(|k| sec.curve.at(sec.t0 + (sec.t1 - sec.t0) * k as f64 / 32.0)).all(|p| near(&self.edges[0], p) && near(&self.edges[1], p))
    }

    /// The least sine between the two normals at 31 points inside `sec`.
    fn least_sine(&self, sec: &Section) -> f64 {
        (1..32)
            .map(|k| sec.t0 + (sec.t1 - sec.t0) * k as f64 / 32.0)
            .map(|t| {
                let (qa, qb) = (sec.on[0].at(t), sec.on[1].at(t));
                normal(&self.da, DVec2::new(qa.x, qa.y)).cross(normal(&self.db, DVec2::new(qb.x, qb.y))).length()
            })
            .fold(f64::INFINITY, f64::min)
    }

    /// Whether `p` lies along an edge both faces share (where a walk runs along it or the faces touch there: the
    /// edge's, not a section).
    fn on_shared(&self, p: DVec3) -> bool {
        self.off(p) <= ALONG
    }

    /// Points on both surfaces, one at least on each section: where the boxes' cells overlap (each pair split, the
    /// larger cell in two along its longer side, down to a sixteenth of each box), a Newton step onto both from the
    /// cells' middles.
    fn seeds(&self) -> Result<Vec<Station>, Fail> {
        type Cell = ((f64, f64), (f64, f64), DVec3, DVec3);
        let cell = |d: &Domain, u: (f64, f64), v: (f64, f64)| -> Cell {
            let (lo, hi) = d.box_of(u, v);
            (u, v, lo, hi)
        };
        let meets = |x: &Cell, y: &Cell| {
            let g = 1e3 * self.eps;
            (x.2 - DVec3::splat(g)).cmple(y.3).all() && (y.2 - DVec3::splat(g)).cmple(x.3).all()
        };
        // (A sixteenth of the face's range, within a few ulps of the parameters: halving leaves widths off by those,
        // and splitting the other way on such a rounding never ends; 05997f93, 08547623.)
        let fine = |lo: f64, hi: f64, span: f64| hi - lo <= span / 16.0 + 16.0 * f64::EPSILON * lo.abs().max(hi.abs()).max(span);
        let small_u = |d: &Domain, c: &Cell| fine(c.0.0, c.0.1, d.uv[1] - d.uv[0]);
        let small_v = |d: &Domain, c: &Cell| fine(c.1.0, c.1.1, d.uv[3] - d.uv[2]);
        let small = |d: &Domain, c: &Cell| small_u(d, c) && small_v(d, c);
        let split = |d: &Domain, c: &Cell| -> [Cell; 2] {
            let (su, sv) = (d.s.at(c.0.1, 0.5 * (c.1.0 + c.1.1)) - d.s.at(c.0.0, 0.5 * (c.1.0 + c.1.1)), d.s.at(0.5 * (c.0.0 + c.0.1), c.1.1) - d.s.at(0.5 * (c.0.0 + c.0.1), c.1.0));
            let by_u = if small_u(d, c) { false } else if small_v(d, c) { true } else { su.length() >= sv.length() };
            if by_u {
                let m = 0.5 * (c.0.0 + c.0.1);
                [cell(d, (c.0.0, m), c.1), cell(d, (m, c.0.1), c.1)]
            } else {
                let m = 0.5 * (c.1.0 + c.1.1);
                [cell(d, c.0, (c.1.0, m)), cell(d, c.0, (m, c.1.1))]
            }
        };
        let (da, db) = (&self.da, &self.db);
        let mut stack = vec![(cell(da, (da.uv[0], da.uv[1]), (da.uv[2], da.uv[3])), cell(db, (db.uv[0], db.uv[1]), (db.uv[2], db.uv[3])))];
        let mut leaves: Vec<(Cell, Cell)> = Vec::new();
        let mut visited = 0usize;
        while let Some((x, y)) = stack.pop() {
            visited += 1;
            if visited > VISITS {
                return Err(Fail::Crowded);
            }
            if !meets(&x, &y) {
                continue;
            }
            let (sx, sy) = (small(da, &x), small(db, &y));
            if sx && sy {
                leaves.push((x, y));
                if leaves.len() > 4096 {
                    return Err(Fail::Crowded);
                }
                continue;
            }
            let x_larger = !sx && (sy || (x.3 - x.2).length() >= (y.3 - y.2).length());
            if x_larger {
                for c in split(da, &x) {
                    stack.push((c, y));
                }
            } else {
                for c in split(db, &y) {
                    stack.push((x, c));
                }
            }
        }
        let mut out: Vec<Station> = Vec::new();
        for (x, y) in leaves {
            let mid = |c: &Cell| DVec2::new(0.5 * (c.0.0 + c.0.1), 0.5 * (c.1.0 + c.1.1));
            let Some(s) = self.onto_both(mid(&x), mid(&y)) else { continue };
            if out.iter().any(|q| (q.p - s.p).length() <= 10.0 * self.eps) {
                continue;
            }
            out.push(s);
        }
        // (Those on a box's side first: an open section traced from its end.)
        let rim = |s: &Station| {
            let near = |d: &Domain, q: DVec2| {
                let (w, h) = (d.uv[1] - d.uv[0], d.uv[3] - d.uv[2]);
                (q.x - d.uv[0]).abs() <= 1e-9 * w || (d.uv[1] - q.x).abs() <= 1e-9 * w || (q.y - d.uv[2]).abs() <= 1e-9 * h || (d.uv[3] - q.y).abs() <= 1e-9 * h
            };
            near(da, s.a) || near(db, s.b)
        };
        out.sort_by_key(|s| !rim(s));
        Ok(out)
    }

    /// A point on both surfaces near (a on the first, b on the second): Newton's least-change steps on Sa(a) = Sb(b),
    /// kept within the boxes (a parameter at its box's side the step would take past it held there: else a section
    /// along the side only creeps toward it, a halving a step). None where it doesn't converge.
    fn onto_both(&self, a: DVec2, b: DVec2) -> Option<Station> {
        let lo = [self.da.uv[0], self.da.uv[2], self.db.uv[0], self.db.uv[2]];
        let hi = [self.da.uv[1], self.da.uv[3], self.db.uv[1], self.db.uv[3]];
        let mut x = [a.x, a.y, b.x, b.y];
        for _ in 0..40 {
            let [pa, au, av, ..] = self.da.s.derivs(x[0], x[1]);
            let [pb, bu, bv, ..] = self.db.s.derivs(x[2], x[3]);
            let r = pa - pb;
            if r.length() <= self.eps {
                return Some(Station { p: 0.5 * (pa + pb), a: DVec2::new(x[0], x[1]), b: DVec2::new(x[2], x[3]) });
            }
            // (The least-change step -(JᵀJ + λ)⁻¹ Jᵀ r over the free parameters, J = [au av -bu -bv]; λ a 1e-10 of
            // its trace, the step all but the least-norm one.)
            let cols = [au, av, -bu, -bv];
            let trace: f64 = cols.iter().map(|c| c.length_squared()).sum();
            let mut free = [true; 4];
            let mut step = [0.0; 4];
            for _ in 0..4 {
                let mut m = [[0.0; 4]; 4];
                let mut rhs = [0.0; 4];
                for i in 0..4 {
                    if !free[i] {
                        m[i][i] = 1.0;
                        continue;
                    }
                    for k in 0..4 {
                        m[i][k] = if free[k] { cols[i].dot(cols[k]) } else { 0.0 };
                    }
                    m[i][i] += 1e-10 * trace;
                    rhs[i] = -cols[i].dot(r);
                }
                step = solve4(m, rhs)?;
                let mut held = false;
                for k in 0..4 {
                    if free[k] && ((x[k] <= lo[k] && step[k] < 0.0) || (x[k] >= hi[k] && step[k] > 0.0)) {
                        free[k] = false;
                        held = true;
                    }
                }
                if !held {
                    break;
                }
            }
            for k in 0..4 {
                x[k] = (x[k] + if free[k] { step[k] } else { 0.0 }).clamp(lo[k], hi[k]);
            }
        }
        None
    }

    /// Why a walk can't go on where the normals at `s` are parallel: the surfaces coincide around it (points of the
    /// first a sixty-fourth of its box off `s` each way all on the second) or only touch.
    fn touching(&self, s: &Station) -> Fail {
        let (w, h) = ((self.da.uv[1] - self.da.uv[0]) / 64.0, (self.da.uv[3] - self.da.uv[2]) / 64.0);
        // (On the second surface past its box too: its box grown by its size, within its range.)
        let mut wide = self.db.uv;
        for (k, r) in self.db.s.range().into_iter().enumerate() {
            let d = wide[2 * k + 1] - wide[2 * k];
            let (lo, hi) = r.unwrap_or((f64::NEG_INFINITY, f64::INFINITY));
            wide[2 * k] = (wide[2 * k] - d).max(lo);
            wide[2 * k + 1] = (wide[2 * k + 1] + d).min(hi);
        }
        let on = [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)].iter().all(|&(i, j)| {
            let q = DVec2::new((s.a.x + i * w).clamp(self.da.uv[0], self.da.uv[1]), (s.a.y + j * h).clamp(self.da.uv[2], self.da.uv[3]));
            self.db.s.param_of(self.da.s.at(q.x, q.y), s.b, wide).1 <= COINCIDE
        });
        if on { Fail::Coincident } else { Fail::Tangent }
    }

    /// The unit tangent at `s` (the normals' cross product), or None where they are parallel.
    fn tangent(&self, s: &Station) -> Option<DVec3> {
        let t = normal(&self.da, s.a).cross(normal(&self.db, s.b));
        (t.length() >= TANGENT_SIN).then(|| t.normalize())
    }

    /// From `s`, the point on both surfaces where the plane across `t` at `h` along it cuts their section; or, given
    /// `bound` (which of ua, va, ub, vb and its value), where the section crosses that parameter line instead.
    fn step(&self, s: &Station, t: DVec3, h: f64, bound: Option<(usize, f64)>, guess: [f64; 4]) -> Option<Station> {
        let mut x = guess;
        for _ in 0..12 {
            let [pa, au, av, ..] = self.da.s.derivs(x[0], x[1]);
            let [pb, bu, bv, ..] = self.db.s.derivs(x[2], x[3]);
            let r = pa - pb;
            let last = match bound {
                Some((k, v)) => x[k] - v,
                None => (pa - s.p).dot(t) - h,
            };
            if r.length() <= self.eps && last.abs() <= self.eps.max(1e-15 * h.abs()) {
                return Some(Station { p: 0.5 * (pa + pb), a: DVec2::new(x[0], x[1]), b: DVec2::new(x[2], x[3]) });
            }
            let row = match bound {
                Some((k, _)) => {
                    let mut e = [0.0; 4];
                    e[k] = 1.0;
                    e
                }
                None => [au.dot(t), av.dot(t), 0.0, 0.0],
            };
            let m = [[au.x, av.x, -bu.x, -bv.x], [au.y, av.y, -bu.y, -bv.y], [au.z, av.z, -bu.z, -bv.z], row];
            let d = solve4(m, [-r.x, -r.y, -r.z, -last])?;
            for k in 0..4 {
                x[k] += d[k];
            }
        }
        None
    }

    /// The parameters a step `h` along `t` from `s` would reach, to first order.
    fn guess(&self, s: &Station, t: DVec3, h: f64) -> [f64; 4] {
        let [_, au, av, ..] = self.da.s.derivs(s.a.x, s.a.y);
        let [_, bu, bv, ..] = self.db.s.derivs(s.b.x, s.b.y);
        let (da, db) = (along(au, av, t * h), along(bu, bv, t * h));
        [s.a.x + da.x, s.a.y + da.y, s.b.x + db.x, s.b.y + db.y]
    }

    /// Which parameter of the four left its box first between `s` and `x` (the share of the way there) and the
    /// side's value.
    fn exit(&self, s: &Station, x: [f64; 4]) -> Option<((usize, f64), f64)> {
        let from = [s.a.x, s.a.y, s.b.x, s.b.y];
        let bounds = [self.da.uv[0], self.da.uv[1], self.da.uv[2], self.da.uv[3], self.db.uv[0], self.db.uv[1], self.db.uv[2], self.db.uv[3]];
        let mut best: Option<(f64, usize, f64)> = None;
        for k in 0..4 {
            let (lo, hi) = (bounds[2 * k], bounds[2 * k + 1]);
            // (Within 1e-7 of its side: on it (a section along a face's own boundary, rounding either way).)
            let slack = 1e-7 * (hi - lo);
            let side = if x[k] < lo - slack { lo } else if x[k] > hi + slack { hi } else { continue };
            let f = if x[k] != from[k] { (side - from[k]) / (x[k] - from[k]) } else { 0.0 };
            if best.is_none_or(|b| f < b.0) {
                best = Some((f, k, side));
            }
        }
        best.map(|(f, k, v)| ((k, v), f.clamp(0.0, 1.0)))
    }

    /// From seed `s` both ways: a branch to where it leaves either box (each way), or round to `s`.
    fn trace(&self, s: Station) -> Result<Branch, Fail> {
        let t0 = self.tangent(&s).ok_or_else(|| self.touching(&s))?;
        let (fwd, closed) = self.walk(s, t0, None)?;
        if closed {
            return Ok(Branch { stations: fwd, closed: true });
        }
        let (back, _) = self.walk(s, -t0, Some(&fwd))?;
        let mut stations: Vec<Station> = back.into_iter().rev().collect();
        stations.pop();
        stations.extend(fwd);
        Ok(Branch { stations, closed: false })
    }

    /// Steps from `s` along `t0` (the stations from `s`, and whether it came round to `s`). Each step at most
    /// [`TURN`] of turn and a sixteenth of either box; longer by half where it turned less than a third of that,
    /// halved where it turned more or failed. `other` (the other way's stations): ends where it meets them (a
    /// closed section whose seed's first way found it open: never; a guard).
    fn walk(&self, s: Station, t0: DVec3, other: Option<&[Station]>) -> Result<(Vec<Station>, bool), Fail> {
        let mut out = vec![s];
        let mut t = t0;
        let hmax = self.size / 16.0;
        let mut h = hmax / 4.0;
        let hmin = 1e-9 * self.size;
        let mut walked = 0.0;
        while out.len() < 20000 {
            let cur = *out.last().unwrap();
            let g = self.guess(&cur, t, h);
            let Some(n) = self.step(&cur, t, h, None, g) else {
                if h <= hmin {
                    return Err(Fail::Lost);
                }
                h *= 0.5;
                continue;
            };
            let x = [n.a.x, n.a.y, n.b.x, n.b.y];
            if let Some((bound, f)) = self.exit(&cur, x) {
                // (Past a box's side within this step: to where it crosses that side, from that share of the way.)
                let from = [cur.a.x, cur.a.y, cur.b.x, cur.b.y];
                let g = std::array::from_fn(|k| from[k] + (x[k] - from[k]) * f);
                let slack = |d: &Domain, q: DVec2| {
                    let w = DVec2::new(d.uv[1] - d.uv[0], d.uv[3] - d.uv[2]) * 1e-9;
                    q.cmpge(DVec2::new(d.uv[0], d.uv[2]) - w).all() && q.cmple(DVec2::new(d.uv[1], d.uv[3]) + w).all()
                };
                if let Some(e) = self.step(&cur, t, h, Some(bound), g)
                    && (e.p - cur.p).dot(t) > -10.0 * self.eps
                    && (e.p - cur.p).length() <= 1.5 * h
                    && slack(&self.da, e.a)
                    && slack(&self.db, e.b)
                {
                    if (e.p - cur.p).length() > 10.0 * self.eps {
                        out.push(e);
                    }
                    return Ok((out, false));
                }
                if h > hmin {
                    h *= 0.5;
                    continue;
                }
                return Ok((out, false));
            }
            // (Run into where they touch: on both faces' edges, the section ends there.)
            let Some(mut tn) = self.tangent(&n) else {
                if self.on_shared(n.p) {
                    out.push(n);
                    return Ok((out, false));
                }
                return Err(self.touching(&n));
            };
            if tn.dot(t) < 0.0 {
                tn = -tn;
            }
            let turn = tn.dot(t).clamp(-1.0, 1.0).acos();
            let uv_far = (n.a - cur.a).abs().cmpgt(DVec2::new(self.da.uv[1] - self.da.uv[0], self.da.uv[3] - self.da.uv[2]) / 16.0).any()
                || (n.b - cur.b).abs().cmpgt(DVec2::new(self.db.uv[1] - self.db.uv[0], self.db.uv[3] - self.db.uv[2]) / 16.0).any();
            if (turn > TURN || uv_far) && h > hmin {
                h *= 0.5;
                continue;
            }
            // (Back at the start, past a turn's worth of walking: closed.)
            let back = s.p - cur.p;
            if out.len() > 2 && walked > 2.0 * h && back.length() <= h * 1.01 && back.dot(t) > 0.0 && other.is_none() {
                out.push(s);
                return Ok((out, true));
            }
            if let Some(o) = other
                && o.len() > 1
                && out.len() > 2
                && o[1..].iter().any(|q| (q.p - n.p).length() <= 0.5 * h)
            {
                return Ok((out, false));
            }
            walked += (n.p - cur.p).length();
            out.push(n);
            t = tn;
            if turn < TURN / 3.0 {
                h = (h * 1.5).min(hmax);
            }
        }
        Err(Fail::Lost)
    }
}

/// Each branch as a [`Section`]: cubic pieces between its stations from their tangents (the curve's and, from them,
/// both surfaces' parameters'), by chord length; a piece whose middle is farther than [`FIT`] from either surface
/// split at a station between.
pub fn sections(a: Domain, b: Domain) -> Result<Vec<Section>, Fail> {
    let m = Marcher::new(a, b);
    let mut out = Vec::new();
    for br in m.branches()? {
        let mut st = br.stations.clone();
        if st.len() < 2 {
            continue;
        }
        let mut round = 0;
        let sec = loop {
            let (sec, bad) = fit(&m, &st, br.closed)?;
            if bad.is_empty() {
                break sec;
            }
            round += 1;
            if round == 12 {
                return Err(Fail::Unfit);
            }
            // (Each bad piece: a station at its middle.)
            for &i in bad.iter().rev() {
                let (p, q) = (st[i], st[i + 1]);
                let t = (q.p - p.p).normalize_or_zero();
                let h = 0.5 * (q.p - p.p).length();
                let g = [0.5 * (p.a.x + q.a.x), 0.5 * (p.a.y + q.a.y), 0.5 * (p.b.x + q.b.x), 0.5 * (p.b.y + q.b.y)];
                let mid = m.step(&p, t, h, None, g).ok_or(Fail::Lost)?;
                st.insert(i + 1, mid);
            }
        };
        for sec in m.off_shared(sec) {
            if m.along_both(&sec) {
                return Err(Fail::OnEdges);
            }
            if m.least_sine(&sec) < TOUCH_SIN {
                return Err(Fail::Tangent);
            }
            out.push(sec);
        }
    }
    Ok(out)
}

/// The cubic through `st` (and its pieces farther than [`FIT`] from a surface, by index).
fn fit(m: &Marcher, st: &[Station], closed: bool) -> Result<(Section, Vec<usize>), Fail> {
    let n = st.len();
    let mut s = vec![0.0f64; n];
    for i in 1..n {
        s[i] = s[i - 1] + (st[i].p - st[i - 1].p).length();
    }
    // (Tangents along the way, the walk's direction.)
    let mut tans: Vec<(DVec3, DVec2, DVec2)> = Vec::with_capacity(n);
    for i in 0..n {
        let chord = if i + 1 < n { st[i + 1].p - st[i].p } else { st[i].p - st[i - 1].p };
        // (A section's end where the faces touch, on their shared edge: along its chord.)
        let mut t = match m.tangent(&st[i]) {
            Some(t) => t,
            None if (i == 0 || i == n - 1) && m.on_shared(st[i].p) => chord.normalize_or_zero(),
            None => return Err(Fail::Tangent),
        };
        if t.dot(chord) < 0.0 {
            t = -t;
        }
        let [_, au, av, ..] = m.da.s.derivs(st[i].a.x, st[i].a.y);
        let [_, bu, bv, ..] = m.db.s.derivs(st[i].b.x, st[i].b.y);
        tans.push((t, along(au, av, t), along(bu, bv, t)));
    }
    if closed {
        tans[n - 1] = tans[0];
    }
    let mut knots = vec![s[0]; 4];
    let (mut p3, mut pa, mut pb) = (vec![st[0].p], vec![st[0].a.extend(0.0)], vec![st[0].b.extend(0.0)]);
    let mut bad = Vec::new();
    let mut acc = 0.0f64;
    for i in 0..n - 1 {
        let l = s[i + 1] - s[i];
        let (t0, ta0, tb0) = tans[i];
        let (t1, ta1, tb1) = tans[i + 1];
        p3.extend([st[i].p + t0 * (l / 3.0), st[i + 1].p - t1 * (l / 3.0), st[i + 1].p]);
        pa.extend([(st[i].a + ta0 * (l / 3.0)).extend(0.0), (st[i + 1].a - ta1 * (l / 3.0)).extend(0.0), st[i + 1].a.extend(0.0)]);
        pb.extend([(st[i].b + tb0 * (l / 3.0)).extend(0.0), (st[i + 1].b - tb1 * (l / 3.0)).extend(0.0), st[i + 1].b.extend(0.0)]);
        knots.extend(std::iter::repeat_n(s[i + 1], if i + 1 == n - 1 { 4 } else { 3 }));
        // (Its middle against both surfaces: the cubic's and its curves' points there.)
        let bez = |a: DVec3, b: DVec3, c: DVec3, d: DVec3| 0.125 * (a + 3.0 * b + 3.0 * c + d);
        let k = p3.len() - 4;
        let q = bez(p3[k], p3[k + 1], p3[k + 2], p3[k + 3]);
        let qa = bez(pa[k], pa[k + 1], pa[k + 2], pa[k + 3]).truncate();
        let qb = bez(pb[k], pb[k + 1], pb[k + 2], pb[k + 3]).truncate();
        let off = (m.da.s.at(qa.x, qa.y) - q).length().max((m.db.s.at(qb.x, qb.y) - q).length());
        acc = acc.max(off);
        if off > FIT * m.size && l > SHORT * m.size {
            bad.push(i);
        }
    }
    let curve = |poles: Vec<DVec3>| NurbsCurve { degree: 3, knots: knots.clone(), poles, weights: Vec::new(), spans: Default::default() };
    let sec = Section { curve: curve(p3), on: [curve(pa), curve(pb)], t0: s[0], t1: s[n - 1], closed, accuracy: acc };
    Ok((sec, bad))
}

impl Marcher<'_> {
    /// `sec` without its stretches along the faces' shared edges (within [`ALONG`]), as OCCT's `off_shared_edges`:
    /// from 129 samples, each stretch off them from where it leaves (bisected, then the line off it extrapolated back
    /// to its nearest the edge, the section's point nearest that).
    fn off_shared(&self, sec: Section) -> Vec<Section> {
        let (f, l) = (sec.t0, sec.t1);
        if self.shared.is_empty() {
            return vec![sec];
        }
        let n = 128;
        let at = |k: usize| f + (l - f) * k as f64 / n as f64;
        let along: Vec<bool> = (0..=n).map(|k| self.on_shared(sec.curve.at(at(k)))).collect();
        let count = along.iter().filter(|&&x| x).count();
        if count == 0 {
            return vec![sec];
        }
        if count == n + 1 {
            return Vec::new();
        }
        let c = &sec.curve;
        let leaves = |inside: f64, outside: f64| -> f64 {
            let (mut a, mut b) = (inside, outside);
            for _ in 0..50 {
                let m = 0.5 * (a + b);
                if self.on_shared(c.at(m)) {
                    a = m;
                } else {
                    b = m;
                }
            }
            let step = (outside - inside) / 8.0;
            let (p, q) = (c.at(b), c.at(b + step));
            let u = (p - q).normalize_or_zero();
            if u == DVec3::ZERO {
                return b;
            }
            let s_best = (0..=200).map(|j| 4.0 * ALONG * j as f64 / 200.0).min_by(|&x, &y| self.off(p + u * x).total_cmp(&self.off(p + u * y))).unwrap_or(0.0);
            let target = p + u * s_best;
            // (The section's point nearest that, between `inside` and `b`: a golden-section search.)
            let (mut lo, mut hi) = (inside.min(b), inside.max(b));
            let g = 0.5 * (5f64.sqrt() - 1.0);
            let d = |t: f64| (c.at(t) - target).length();
            for _ in 0..80 {
                let (x, y) = (hi - g * (hi - lo), lo + g * (hi - lo));
                if d(x) < d(y) {
                    hi = y;
                } else {
                    lo = x;
                }
            }
            0.5 * (lo + hi)
        };
        let mut out = Vec::new();
        let mut i0 = 0;
        while i0 <= n {
            if along[i0] {
                i0 += 1;
                continue;
            }
            let mut i1 = i0;
            while i1 < n && !along[i1 + 1] {
                i1 += 1;
            }
            let t0 = if i0 > 0 { leaves(at(i0 - 1), at(i0)) } else { f };
            let t1 = if i1 < n { leaves(at(i1 + 1), at(i1)) } else { l };
            i0 = i1 + 1;
            if t1 - t0 <= 1e-9 {
                continue;
            }
            out.push(Section { t0, t1, closed: false, ..sec.clone() });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(a: DVec3, b: DVec3) -> NurbsCurve {
        NurbsCurve { degree: 1, knots: vec![0.0, 0.0, 1.0, 1.0], poles: vec![a, b], weights: Vec::new(), spans: Default::default() }
    }

    /// A sphere cut by a plane (an extruded line) through it off its middle: one closed circle, every point of it on
    /// both, its radius √(r² - d²).
    #[test]
    fn a_sphere_cut_by_an_extruded_line_is_one_circle() {
        let sphere = FreeSurface::Sphere { o: DVec3::ZERO, x: DVec3::X, y: DVec3::Y, z: DVec3::Z, r: 1.0 };
        let plane = FreeSurface::Extrusion { c: line(DVec3::new(-2.0, -2.0, 0.4), DVec3::new(2.0, -2.0, 0.4)), d: DVec3::Y };
        let a = Domain { s: &sphere, uv: [0.0, std::f64::consts::TAU, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2], edges: &[] };
        let b = Domain { s: &plane, uv: [0.0, 1.0, 0.0, 4.0], edges: &[] };
        let secs = sections(a, b).unwrap();
        let r = (1.0f64 - 0.16).sqrt();
        // (The sphere's box ends at its seam: the circle in one piece from seam to seam.)
        assert_eq!(secs.len(), 1, "{secs:?}");
        let c = &secs[0];
        for k in 0..=200 {
            let t = c.t0 + (c.t1 - c.t0) * k as f64 / 200.0;
            let p = c.curve.at(t);
            assert!((p.z - 0.4).abs() < 1e-8 && (p.truncate().length() - r).abs() < 1e-8, "{p}");
            let qa = c.on[0].at(t).truncate();
            assert!((sphere.at(qa.x, qa.y) - p).length() < 1e-8);
        }
        // (By chord length: the stations' polygon, a little shorter than the circle.)
        assert!((c.t1 - c.t0) / (std::f64::consts::TAU * r) > 0.9999 && c.t1 - c.t0 < std::f64::consts::TAU * r, "{}", c.t1 - c.t0);
    }

    /// The sphere and plane above with the plane's parameters far from 0 and its range not a power of two (shfall
    /// 08547623's): a sixteenth of it reached by halving is off by an ulp or two, which once split the cells the other
    /// way without end.
    #[test]
    fn a_sphere_cut_by_a_plane_with_offset_parameters_is_one_circle() {
        let (u0, u1, v1) = (26.775536035811133, 29.488453261014147, 0.0322315837148884);
        let sphere = FreeSurface::Sphere { o: DVec3::ZERO, x: DVec3::X, y: DVec3::Y, z: DVec3::Z, r: 1.0 };
        let mut c = line(DVec3::new(-2.0, -2.0, 0.4), DVec3::new(2.0, -2.0, 0.4));
        c.knots = vec![u0, u0, u1, u1];
        let plane = FreeSurface::Extrusion { c, d: DVec3::Y * (4.0 / v1) };
        let a = Domain { s: &sphere, uv: [0.0, std::f64::consts::TAU, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2], edges: &[] };
        let b = Domain { s: &plane, uv: [u0, u1, 0.0, v1], edges: &[] };
        let secs = sections(a, b).unwrap();
        let r = (1.0f64 - 0.16).sqrt();
        assert_eq!(secs.len(), 1, "{secs:?}");
        let c = &secs[0];
        for k in 0..=200 {
            let p = c.curve.at(c.t0 + (c.t1 - c.t0) * k as f64 / 200.0);
            assert!((p.z - 0.4).abs() < 1e-8 && (p.truncate().length() - r).abs() < 1e-8, "{p}");
        }
    }

    /// Two crossing extruded arcs' surfaces (cylinders as B-splines, axes at right angles, radii 1 and 0.6): two closed
    /// loops where the thinner one passes through the wider.
    #[test]
    fn two_crossing_extruded_circles_meet_in_two_loops() {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let circle = |r: f64, f: &dyn Fn(f64, f64) -> DVec3| NurbsCurve {
            degree: 2,
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0],
            poles: [(1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (-1.0, 1.0), (-1.0, 0.0), (-1.0, -1.0), (0.0, -1.0), (1.0, -1.0), (1.0, 0.0)].iter().map(|&(x, y)| f(r * x, r * y)).collect(),
            weights: vec![1.0, w, 1.0, w, 1.0, w, 1.0, w, 1.0],
            spans: Default::default(),
        };
        let wide = FreeSurface::Extrusion { c: circle(1.0, &|x, y| DVec3::new(x, y, -2.0)), d: DVec3::Z };
        let thin = FreeSurface::Extrusion { c: circle(0.6, &|x, y| DVec3::new(-2.0, x, y)), d: DVec3::X };
        let a = Domain { s: &wide, uv: [0.0, 4.0, 0.0, 4.0], edges: &[] };
        let b = Domain { s: &thin, uv: [0.0, 4.0, 0.0, 4.0], edges: &[] };
        let secs = sections(a, b).unwrap();
        let ends: usize = secs.iter().map(|c| if c.closed { 0 } else { 2 }).sum();
        // (Each loop crosses the thin one's seam (u 0 at y 0.6... its box's side): in pieces, every piece on both.)
        assert!(!secs.is_empty() && ends.is_multiple_of(2));
        let mut length = 0.0;
        for c in &secs {
            length += c.t1 - c.t0;
            for k in 0..=50 {
                let p = c.curve.at(c.t0 + (c.t1 - c.t0) * k as f64 / 50.0);
                assert!((p.truncate().length() - 1.0).abs() < 1e-7 && ((p.y * p.y + p.z * p.z).sqrt() - 0.6).abs() < 1e-7, "{p}");
            }
        }
        // (Each loop about 2π·0.6 long and a little more: between 3.77 and 4.5.)
        assert!(length > 2.0 * 3.77 && length < 2.0 * 4.5, "{length}");
    }
}
