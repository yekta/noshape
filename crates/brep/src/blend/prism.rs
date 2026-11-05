//! Rounds of straight edges running between two faces ruled along them (planes along the edge, cylinders about
//! axes along it, profiles extruded along it), each edge ending on planar caps square to it at vertices of three
//! edges (stef-radijator's fins round a hub, between the fin's end and the gaps' cylinders; a gear's tooth flanks at
//! its root cylinder): each round is the section's 2D fillet swept between the caps, a cylinder touching both faces
//! along a line; the caps' corners at its ends become its arcs.

use super::{curve_point, no, BlendEdge, EdgeUses, Spec};
use crate::nurbs::NurbsCurve;
use crate::{Body, Curve, Decline, Edge, Face, Kind, Origin, Output, Record, Relation, Surface, Use};
use glam::{DVec2, DVec3};

/// A face along direction `d` as seen in the plane square to `d` through `o`: a line or a circle, its offset
/// towards the outward normal by `s` (2D, in the plane's basis `u`, `w`).
enum Trace {
    Line { p: DVec2, n: DVec2 },
    Circle { c: DVec2, r: f64 },
    /// An extruded profile's section: its curve's points offset by `s` along the face's outward normal.
    Profile { x: Extruded, s: f64 },
}

/// A face extruded along the edge: its profile curve (in a plane square to the extrusion), the extrusion's unit
/// direction, and the sign turning the surface's normal (curve tangent x direction) outward.
#[derive(Clone, Copy)]
struct Extruded {
    curve: u32,
    dir: DVec3,
    sign: f64,
}

impl Extruded {
    fn of(b: &Body, f: &Face, d: DVec3) -> Option<Self> {
        let Surface::Extrusion { curve, dir } = f.surface else { return None };
        let dir = dir.try_normalize()?;
        if dir.cross(d).length() > 1e-9 {
            return None;
        }
        // (Its profile in one plane square to the extrusion: a point's foot on it is the nearest point.)
        let c = b.nurbs_curves.get(curve as usize)?;
        let h0 = c.poles.first()?.dot(dir);
        let size = c.poles.iter().fold(0.0f64, |m, p| m.max((*p - c.poles[0]).length())).max(1e-12);
        if c.poles.iter().any(|p| (p.dot(dir) - h0).abs() > 1e-12 * size.max(1.0)) {
            return None;
        }
        Some(Self { curve, dir, sign: if f.reversed { -1.0 } else { 1.0 } })
    }
    fn curve<'a>(&self, b: &'a Body) -> &'a NurbsCurve {
        &b.nurbs_curves[self.curve as usize]
    }
    /// The profile's point at `t` moved along the extrusion to the plane through `o` square to it.
    fn at(&self, b: &Body, t: f64, o: DVec3) -> DVec3 {
        let q = self.curve(b).at(t);
        q + self.dir * (o - q).dot(self.dir)
    }
    /// The face's outward normal at `t`.
    fn normal(&self, b: &Body, t: f64) -> DVec3 {
        (self.curve(b).derivs(t)[1].cross(self.dir) * self.sign).normalize_or_zero()
    }
    /// The parameter of the profile point nearest `p` (moved into the profile's plane).
    fn param(&self, b: &Body, p: DVec3) -> f64 {
        let c = self.curve(b);
        let q0 = c.at(c.range().0);
        c.param_of(p + self.dir * (q0 - p).dot(self.dir)).0
    }
}

/// A face ruled along `d` (a plane along it, a cylinder about an axis along it) cut square to `d` at `o`, its
/// outward side offset by `s`; None for any other face.
fn trace(b: &Body, f: &Face, d: DVec3, o: DVec3, u: DVec3, w: DVec3, s: f64) -> Option<Trace> {
    let flat = |p: DVec3| DVec2::new((p - o).dot(u), (p - o).dot(w));
    let sign = if f.reversed { -1.0 } else { 1.0 };
    if let Some(x) = Extruded::of(b, f, d) {
        return Some(Trace::Profile { x, s });
    }
    match f.surface {
        Surface::Plane { origin, normal } if normal.dot(d).abs() < 1e-9 => {
            let n = sign * DVec2::new(normal.dot(u), normal.dot(w)).normalize();
            Some(Trace::Line { p: flat(origin) + n * s, n })
        }
        Surface::Cylinder { origin, axis, radius, .. } if axis.cross(d).length() < 1e-9 => {
            // (Outward away from the axis unless reversed: the offset grows or shrinks the circle.)
            let r = radius + sign * s;
            (r > 0.0).then_some(Trace::Circle { c: flat(origin), r })
        }
        _ => None,
    }
}

/// Where two traces meet, nearest `near`.
fn meet(a: &Trace, b: &Trace, near: DVec2) -> Option<DVec2> {
    let line_circle = |p: DVec2, n: DVec2, c: DVec2, r: f64| -> Vec<DVec2> {
        let t = DVec2::new(-n.y, n.x);
        let h = (c - p).dot(n);
        let k = r * r - h * h;
        if k < 0.0 {
            return Vec::new();
        }
        let base = p + t * (c - p).dot(t);
        vec![base + t * k.sqrt(), base - t * k.sqrt()]
    };
    let roots = match (a, b) {
        (Trace::Line { p: p1, n: n1 }, Trace::Line { p: p2, n: n2 }) => {
            let det = n1.x * n2.y - n1.y * n2.x;
            if det.abs() < 1e-12 {
                return None;
            }
            let (c1, c2) = (n1.dot(*p1), n2.dot(*p2));
            vec![DVec2::new((c1 * n2.y - c2 * n1.y) / det, (n1.x * c2 - n2.x * c1) / det)]
        }
        (Trace::Line { p, n }, Trace::Circle { c, r }) | (Trace::Circle { c, r }, Trace::Line { p, n }) => line_circle(*p, *n, *c, *r),
        (Trace::Circle { c: c1, r: r1 }, Trace::Circle { c: c2, r: r2 }) => {
            let dc = *c2 - *c1;
            let l = dc.length();
            if l < 1e-15 {
                return None;
            }
            let x = (l * l + r1 * r1 - r2 * r2) / (2.0 * l);
            let h2 = r1 * r1 - x * x;
            if h2 < 0.0 {
                return None;
            }
            let e = dc / l;
            let m = *c1 + e * x;
            let t = DVec2::new(-e.y, e.x) * h2.sqrt();
            vec![m + t, m - t]
        }
        // (A profile's: meet_profile.)
        _ => return None,
    };
    roots.into_iter().min_by(|x, y| (*x - near).length().total_cmp(&(*y - near).length()))
}

/// Where the offset profile `x` (by `s`) meets the other trace `other`, in the section plane through `o` (basis `u`,
/// `w`, the frame): the root nearest `near` along the profile and its parameter. Sampled over the profile's range, each change of
/// sign refined by bisection.
fn meet_profile(b: &Body, x: &Extruded, s: f64, other: &Trace, (o, u, w): (DVec3, DVec3, DVec3), near: DVec2) -> Option<(DVec2, f64)> {
    let flat = |p: DVec3| DVec2::new((p - o).dot(u), (p - o).dot(w));
    let at = |t: f64| -> DVec2 {
        let n = x.normal(b, t);
        flat(x.at(b, t, o)) + DVec2::new(n.dot(u), n.dot(w)) * s
    };
    let g = |t: f64| -> f64 {
        let q = at(t);
        match other {
            Trace::Line { p, n } => (q - *p).dot(*n),
            Trace::Circle { c, r } => (q - *c).length() - r,
            Trace::Profile { .. } => f64::NAN,
        }
    };
    let (t0, t1) = x.curve(b).range();
    let n = 400;
    let mut best: Option<(DVec2, f64)> = None;
    let mut prev = (t0, g(t0));
    for k in 1..=n {
        let t = t0 + (t1 - t0) * k as f64 / n as f64;
        let gt = g(t);
        if !gt.is_finite() || !prev.1.is_finite() {
            return None;
        }
        if (prev.1 <= 0.0) != (gt <= 0.0) {
            let (mut a, mut ga, mut z) = (prev.0, prev.1, t);
            for _ in 0..80 {
                let m = 0.5 * (a + z);
                let gm = g(m);
                if (ga <= 0.0) == (gm <= 0.0) {
                    a = m;
                    ga = gm;
                } else {
                    z = m;
                }
            }
            let tm = 0.5 * (a + z);
            let q = at(tm);
            if best.is_none_or(|(bq, _)| (q - near).length() < (bq - near).length()) {
                best = Some((q, tm));
            }
        }
        prev = (t, gt);
    }
    best
}

/// The foot on face `f`'s trace of 3D point `c` in the section plane (the round's tangent point there).
fn foot(f: &Face, c: DVec3, d: DVec3) -> Option<DVec3> {
    match f.surface {
        Surface::Plane { origin, normal } => Some(c - normal * (c - origin).dot(normal)),
        Surface::Cylinder { origin, axis, radius, .. } => {
            let a = axis.normalize();
            let on = origin + a * (c - origin).dot(a);
            let r = c - on;
            (r.length() > 1e-15 && a.cross(d).length() < 1e-9).then(|| on + r.normalize() * radius)
        }
        _ => None,
    }
}

/// A box round face `f`: its edges' (arcs along their span), a free-form surface's poles.
fn face_box(b: &Body, f: &Face) -> (DVec3, DVec3) {
    let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
    let mut add = |p: DVec3| {
        lo = lo.min(p);
        hi = hi.max(p);
    };
    for u in f.loops.iter().flatten() {
        let e = &b.edges[u.edge as usize];
        match e.curve {
            Curve::Line | Curve::Degenerate => {
                add(b.vertices[e.v[0] as usize]);
                add(b.vertices[e.v[1] as usize]);
            }
            Curve::Circle { .. } => {
                for k in 0..=32 {
                    if let Some(p) = curve_point(b, e, k as f64 / 32.0) {
                        add(p);
                    }
                }
            }
            Curve::Nurbs { index, .. } => b.nurbs_curves[index as usize].poles.iter().for_each(|&p| add(p)),
            _ => return (DVec3::splat(f64::NEG_INFINITY), DVec3::splat(f64::INFINITY)),
        }
    }
    match f.surface {
        Surface::Nurbs { index } => b.nurbs_surfaces[index as usize].poles.iter().for_each(|&p| add(p)),
        // (A sphere's or torus's patch: a blend's, within its arcs' boxes.)
        // (An extrusion between its edges: their boxes hold it.)
        Surface::Plane { .. } | Surface::Cylinder { .. } | Surface::Cone { .. } | Surface::Sphere { .. } | Surface::Torus { .. } | Surface::Extrusion { .. } => {}
        _ => return (DVec3::splat(f64::NEG_INFINITY), DVec3::splat(f64::INFINITY)),
    }
    (lo, hi)
}

/// One round's end: the cap, the vertex, the edges along the round's two faces there, the ball's centre and the
/// two tangent points; or (no cap) a corner of earlier rounds.
struct End {
    cap: u32,
    v: u32,
    g: [u32; 2],
    c: DVec3,
    t: [DVec3; 2],
    /// A curved end edge's parameter at its tangent point (its new end).
    tpar: [Option<f64>; 2],
    /// The tangent point at its end edge's far end (a round as wide as the face it goes with).
    at_far: [bool; 2],
    /// A round past a face it rolls over (`t` where it crosses the face beyond).
    roll: [Option<Roll>; 2],
    corner: Option<Corner>,
}

/// A round wider than one of its faces (B, a rectangle): it crosses the face C beyond B's far edge `bc` (its far end
/// `far` here) where its section meets C's cap edge `h`; B, its cap edges and `bc` go.
#[derive(Clone, Copy)]
struct Roll {
    bc: u32,
    face: u32,
    h: u32,
    far: u32,
}

/// A round's section in a cap: a circle about `c` of radius `r` from `from` to `to`, or a chamfer's line from `from`
/// towards `to`.
#[derive(Clone, Copy)]
struct Section {
    chamfer: bool,
    c: DVec3,
    r: f64,
    from: DVec3,
    to: DVec3,
}

/// Where a round's section crosses the line from `q` along `dir`: within (`margin`, `len` − `margin`) of `q`, on the
/// section past `from` (`n` the cap's normal); the nearest such.
fn section_meets_line(sec: &Section, (q, dir, len): (DVec3, DVec3, f64), margin: f64, n: DVec3) -> Option<DVec3> {
    let Section { chamfer, c, r, from, to } = *sec;
    let ss: Vec<f64> = if chamfer {
        let dl = to - from;
        let den = dl.cross(dir).dot(n);
        if den.abs() < 1e-12 * dl.length() {
            return None;
        }
        vec![(q - from).cross(dl).dot(n) / den]
    } else {
        let w = q - c;
        let bq = dir.dot(w);
        let disc = bq * bq - (w.length_squared() - r * r);
        if disc < 0.0 {
            return None;
        }
        let sq = disc.sqrt();
        vec![-bq - sq, -bq + sq]
    };
    let on = |p: DVec3| {
        if chamfer {
            let dl = to - from;
            let f = (p - from).dot(dl) / dl.length_squared();
            return f > 0.0;
        }
        let (u0, u1, w) = (from - c, to - c, p - c);
        let turn = u0.cross(u1).dot(n);
        u0.cross(w).dot(n) * turn > 0.0 && w.cross(u1).dot(n) * turn > 0.0
    };
    ss.into_iter().filter(|&s| s > margin && s < len - margin).map(|s| (s, q + dir * s)).filter(|&(_, p)| on(p)).min_by(|a, b| a.0.total_cmp(&b.0)).map(|x| x.1)
}

/// An end at two earlier rounds (tori, the round's radius) beside its faces: their meeting edge `m` (to vertex
/// `p`, the sphere's pole) and the tori beside the end edges.
struct Corner {
    m: u32,
    tori: [u32; 2],
    p: u32,
}

/// Two rounds that use up the face between them: plan `a`'s side `ka` and plan `b`'s side `kb` are that face; one
/// cylinder (`one`, a full round) or two meeting along a ridge (wider than the face).
struct Full {
    a: usize,
    ka: usize,
    b: usize,
    kb: usize,
    face: u32,
    one: bool,
}

/// Where two convex chamfers' lines in a cap cross, within each between its ends (`ka`, `kb` their sides on the face
/// between them; `n` the cap's normal).
fn chamfers_cross(ea: &End, ka: usize, eb: &End, kb: usize, n: DVec3) -> Option<DVec3> {
    let (pa, da) = (ea.t[1 - ka], ea.t[ka] - ea.t[1 - ka]);
    let (pb, db) = (eb.t[1 - kb], eb.t[kb] - eb.t[1 - kb]);
    let den = da.cross(db).dot(n);
    if den.abs() < 1e-12 * da.length() * db.length() {
        return None;
    }
    let sa = (pb - pa).cross(db).dot(n) / den;
    let sb = (pb - pa).cross(da).dot(n) / den;
    (sa > 0.0 && sa < 1.0 && sb > 0.0 && sb < 1.0).then_some(pa + da * sa)
}

/// Where two convex rounds' circles in a cap cross (`n` the cap's normal): the crossing nearer the face between them,
/// on each round's arc between its tangent points (`ka`, `kb` their sides on that face).
fn crossing(ea: &End, ra: f64, ka: usize, eb: &End, rb: f64, kb: usize, n: DVec3) -> Option<DVec3> {
    let v = eb.c - ea.c;
    let l = v.length();
    if l < 1e-12 || l > ra + rb || l < (ra - rb).abs() {
        return None;
    }
    let along = (ra * ra - rb * rb + l * l) / (2.0 * l);
    let h = (ra * ra - along * along).max(0.0).sqrt();
    let base = ea.c + v / l * along;
    let side = n.cross(v / l) * h;
    let near = (ea.t[ka] + eb.t[kb]) * 0.5;
    let p = if (base + side - near).length() <= (base - side - near).length() { base + side } else { base - side };
    let on_arc = |e: &End, k: usize| {
        let (u0, u1, w) = (e.t[1 - k] - e.c, e.t[k] - e.c, p - e.c);
        let turn = u0.cross(u1).dot(n);
        u0.cross(w).dot(n) * turn > 0.0 && w.cross(u1).dot(n) * turn > 0.0
    };
    (on_arc(ea, ka) && on_arc(eb, kb)).then_some(p)
}

/// One round: its edge, radius, two faces, convex (+1) or concave (−1), its two ends (the edge's order).
struct Plan {
    edge: u32,
    /// The radius, or an equal chamfer's distance (`chamfer`: the round a plane, its sections lines).
    r: f64,
    chamfer: bool,
    fs: [u32; 2],
    sigma: f64,
    ends: [End; 2],
}

/// The rounds of `edges` (fillets and equal chamfers), every one such an edge; a decline otherwise.
pub(super) fn blend(body: &Body, edges: &[BlendEdge], uses: &EdgeUses, ve: &[Vec<u32>], margin: f64, pairs: bool) -> Result<Output, Decline> {
    let mut blended = vec![false; body.edges.len()];
    for e in edges {
        blended[e.edge as usize] = true;
    }
    // (Curved edges given: earlier rounds' meeting curves, each taken by a corner.)
    let mut absorbed = vec![false; body.edges.len()];
    let mut taken_m = vec![false; body.edges.len()];
    for e in edges {
        absorbed[e.edge as usize] = body.edges[e.edge as usize].curve != Curve::Line;
    }
    let mut taken_v = vec![false; body.vertices.len()];
    let mut plans: Vec<Plan> = Vec::new();
    for be in edges.iter().filter(|e| !absorbed[e.edge as usize]) {
        let (r, chamfer) = match be.spec {
            Spec::Fillet { radius } => (radius, false),
            Spec::Chamfer { d1, d2 } if (d1 - d2).abs() <= margin => (d1, true),
            _ => return no("a prism round not a fillet or an equal chamfer"),
        };
        let e = &body.edges[be.edge as usize];
        if e.curve != Curve::Line {
            return no("a prism round's edge not a line");
        }
        let (p0, p1) = (body.vertices[e.v[0] as usize], body.vertices[e.v[1] as usize]);
        let d = (p1 - p0).normalize();
        let fs = [uses[be.edge as usize][0].0, uses[be.edge as usize][1].0];
        // Convex where one face runs away from the other's outward side.
        let mut ends: Vec<End> = Vec::new();
        let mut sigma = 0.0;
        for &v in &e.v {
            if std::mem::replace(&mut taken_v[v as usize], true) {
                return no("two prism rounds at a vertex");
            }
            let p = body.vertices[v as usize];
            let mut others: Vec<u32> = ve[v as usize].iter().copied().filter(|&o| o != be.edge).collect();
            let ms: Vec<u32> = others.iter().copied().filter(|&o| absorbed[o as usize]).collect();
            others.retain(|&o| !absorbed[o as usize]);
            let [o1, o2] = others[..] else { return no("a prism round's end not at three edges") };
            if blended[o1 as usize] || blended[o2 as usize] || ms.len() > 1 {
                return no("a prism round's end at another blended edge");
            }
            let on = |o: u32, f: u32| uses[o as usize].iter().any(|x| x.0 == f);
            let g = if on(o1, fs[0]) && on(o2, fs[1]) { [o1, o2] } else if on(o2, fs[0]) && on(o1, fs[1]) { [o2, o1] } else { return no("a prism round's end edges not along its faces") };
            let beside = |k: usize| uses[g[k] as usize].iter().map(|x| x.0).find(|&f| f != fs[k]);
            let (cap, corner) = match ms[..] {
                [] => {
                    let caps: Vec<u32> = uses[g[0] as usize].iter().map(|x| x.0).filter(|&f| f != fs[0] && on(g[1], f)).collect();
                    let [cap] = caps[..] else { return no("a prism round's end without one cap") };
                    let Surface::Plane { normal: nc, .. } = body.faces[cap as usize].surface else { return no("a prism round's cap not planar") };
                    if nc.cross(d).length() > 1e-9 {
                        return no("a prism round's cap not square to it");
                    }
                    (cap, None)
                }
                [m] => {
                    let (Some(t0), Some(t1)) = (beside(0), beside(1)) else { return no("a prism round's corner without its rounds") };
                    let torus_r = |f: u32| matches!(body.faces[f as usize].surface, Surface::Torus { minor, .. } if (minor - r).abs() <= margin);
                    if !torus_r(t0) || !torus_r(t1) || !on(m, t0) || !on(m, t1) || std::mem::replace(&mut taken_m[m as usize], true) {
                        return no("a prism round's corner not between two rounds of its radius");
                    }
                    let me = &body.edges[m as usize];
                    let p = if me.v[0] == v { me.v[1] } else { me.v[0] };
                    (u32::MAX, Some(Corner { m, tori: [t0, t1], p }))
                }
                _ => unreachable!(),
            };
            // The way along each end edge from the vertex (into its face).
            let away = |o: u32| -> Option<DVec3> {
                let oe = &body.edges[o as usize];
                let far = if oe.v[0] == v { oe.v[1] } else { oe.v[0] };
                match oe.curve {
                    Curve::Line => Some((body.vertices[far as usize] - p).normalize()),
                    Curve::Circle { center, normal, .. } => {
                        let t = normal.cross(p - center).normalize();
                        Some(if oe.v[0] == v { t } else { -t })
                    }
                    // (A profile at the cap: its tangent at the vertex, the way to its other end.)
                    Curve::Nurbs { index, t0, t1 } => {
                        let at_v = if oe.v[0] == v { t0 } else { t1 };
                        let tg = body.nurbs_curves[index as usize].derivs(at_v)[1] * (t1 - t0).signum();
                        tg.try_normalize().map(|t| if oe.v[0] == v { t } else { -t })
                    }
                    _ => None,
                }
            };
            let (Some(along_a), Some(along_b)) = (away(g[0]), away(g[1])) else { return no("a prism round's end edge not a line or an arc") };
            if sigma == 0.0 {
                // Into A across the edge: its outward normal across the edge's way as A's loop runs it (the face is on
                // its left looking against the normal; at a face's reflex corner its end edge runs the other way).
                // (A face's outward normal at the vertex: a profile's as it is, already outward.)
                let outward = |f: &Face| -> Option<DVec3> {
                    if let Some(x) = Extruded::of(body, f, d) {
                        return Some(x.normal(body, x.param(body, p)));
                    }
                    let n = match f.surface {
                        Surface::Plane { normal, .. } => normal,
                        Surface::Cylinder { origin, axis, .. } => {
                            let a = axis.normalize();
                            (p - origin - a * (p - origin).dot(a)).normalize()
                        }
                        _ => return None,
                    };
                    Some(n * if f.reversed { -1.0 } else { 1.0 })
                };
                let fa = &body.faces[fs[0] as usize];
                let Some(na) = outward(fa) else { return no("a prism round's face not ruled along it") };
                let Some(rev) = uses[be.edge as usize].iter().find(|x| x.0 == fs[0]).map(|x| x.3) else { return no("a prism round's edge not in its face") };
                let qa = na.cross(if rev { -d } else { d });
                let Some(nb) = outward(&body.faces[fs[1] as usize]) else { return no("a prism round's face not ruled along it") };
                sigma = if qa.dot(nb) < 0.0 { 1.0 } else { -1.0 };
            }
            // (An equal chamfer between planes: its distance along each face's end edge, from the vertex.)
            let (c, t0, t1) = if chamfer {
                let planar = |f: u32| matches!(body.faces[f as usize].surface, Surface::Plane { .. });
                let line = |o: u32| body.edges[o as usize].curve == Curve::Line;
                if corner.is_some() || !planar(fs[0]) || !planar(fs[1]) || !line(g[0]) || !line(g[1]) {
                    return no("a prism chamfer not between planes ending on caps");
                }
                (p, p + along_a * r, p + along_b * r)
            } else {
                let u = d.any_orthonormal_vector();
                let w = d.cross(u);
                let (Some(ta), Some(tb)) = (trace(body, &body.faces[fs[0] as usize], d, p, u, w, -sigma * r), trace(body, &body.faces[fs[1] as usize], d, p, u, w, -sigma * r)) else {
                    return no("a prism round's face not ruled along it");
                };
                // (A profile's: the root along it, its parameter the tangent point's.)
                let mut on_profile = [None, None];
                let c2 = match (&ta, &tb) {
                    (Trace::Profile { .. }, Trace::Profile { .. }) => return no("a prism round between two profiles"),
                    (Trace::Profile { x, s }, other) | (other, Trace::Profile { x, s }) => {
                        let Some((q, t)) = meet_profile(body, x, *s, other, (p, u, w), DVec2::ZERO) else { return no("a prism round's faces' offsets don't meet") };
                        on_profile[if matches!(ta, Trace::Profile { .. }) { 0 } else { 1 }] = Some((*x, t));
                        q
                    }
                    _ => {
                        let Some(q) = meet(&ta, &tb, DVec2::ZERO) else { return no("a prism round's faces' offsets don't meet") };
                        q
                    }
                };
                if c2.length() > 10.0 * r {
                    return no("a prism round's centre far from its edge");
                }
                let c = p + u * c2.x + w * c2.y;
                let foot_k = |k: usize| match on_profile[k] {
                    Some((x, t)) => Some(x.at(body, t, c)),
                    None => foot(&body.faces[fs[k] as usize], c, d),
                };
                let (Some(t0), Some(t1)) = (foot_k(0), foot_k(1)) else { return no("a prism round's tangent points") };
                (c, t0, t1)
            };
            let mut tpar = [None, None];
            let mut at_far = [false; 2];
            let mut roll = [None, None];
            let mut ts = [t0, t1];
            // Each tangent point on its end edge, short of its far end (no overflow; at it only where the face between
            // goes with another round's, checked with the pairs).
            for (k, t) in [t0, t1].into_iter().enumerate() {
                let along = [along_a, along_b][k];
                let oe = &body.edges[g[k] as usize];
                let on_edge = match oe.curve {
                    Curve::Line => (t - p).normalize_or_zero().dot(along) > 1.0 - 1e-9,
                    // (Along the arc's way from the vertex: its chord's turn from the tangent within a half turn.)
                    Curve::Circle { .. } => (t - p).normalize_or_zero().dot(along) > 0.0,
                    // (A profile: the point on it, inside its range from the vertex's end.)
                    Curve::Nurbs { index, t0: e0, t1: e1 } => {
                        let (tp, dist) = body.nurbs_curves[index as usize].param_of(t);
                        let (tv, tf) = if oe.v[0] == v { (e0, e1) } else { (e1, e0) };
                        let frac = (tp - tv) / (tf - tv);
                        tpar[k] = Some(tp);
                        dist <= 10.0 * margin && frac > 0.0 && frac < 1.0
                    }
                    _ => false,
                };
                if !on_edge {
                    return no("a prism round's tangent point off its end edge");
                }
                let far = body.vertices[if oe.v[0] == v { oe.v[1] } else { oe.v[0] } as usize];
                let reach = (t - p).length();
                let room = match oe.curve {
                    Curve::Line => (far - p).length(),
                    Curve::Circle { center, radius, .. } => {
                        let (a, b) = ((p - center).normalize(), (far - center).normalize());
                        radius * a.dot(b).clamp(-1.0, 1.0).acos()
                    }
                    // (A profile: room enough when the point is short of its far end by more than the margin.)
                    Curve::Nurbs { .. } => reach + (far - t).length(),
                    _ => 0.0,
                };
                // (Past it: as wide as a face that goes with another round's, or rolling over the face beyond.)
                let fv = if oe.v[0] == v { oe.v[1] } else { oe.v[0] };
                let bc = ve[fv as usize].iter().copied().find(|&o| o != g[k] && uses[o as usize].iter().any(|x| x.0 == fs[k]));
                if pairs && matches!(oe.curve, Curve::Line) && reach >= margin && reach + margin >= room && corner.is_none() && sigma > 0.0 && bc.is_some_and(|o| !blended[o as usize]) {
                    let bc = bc.unwrap_or(u32::MAX);
                    let beyond = uses[bc as usize].iter().map(|x| x.0).find(|&f| f != fs[k]);
                    let h = ve[fv as usize].iter().copied().find(|&o| o != g[k] && o != bc && uses[o as usize].iter().any(|x| x.0 == cap));
                    let (Some(face), Some(h)) = (beyond, h) else { return no("a prism round past its end edges") };
                    let he = &body.edges[h as usize];
                    if !matches!(body.faces[face as usize].surface, Surface::Plane { .. }) || he.curve != Curve::Line || body.edges[bc as usize].curve != Curve::Line {
                        return no("a prism round past its end edges onto a face not planar");
                    }
                    let hf = body.vertices[if he.v[0] == fv { he.v[1] } else { he.v[0] } as usize];
                    let len = (hf - far).length();
                    let Some(x) = section_meets_line(&Section { chamfer, c, r, from: ts[1 - k], to: t }, (far, (hf - far) / len, len), margin, d) else {
                        return no("a prism round past its end edges");
                    };
                    ts[k] = x;
                    roll[k] = Some(Roll { bc, face, h, far: fv });
                } else if matches!(oe.curve, Curve::Line) && reach >= margin && (reach - room).abs() < margin {
                    at_far[k] = true;
                } else if reach < margin || reach + margin >= room {
                    return no("a prism round past its end edges");
                }
            }
            if let Some(cn) = &corner
                && ((body.vertices[cn.p as usize] - c).length() - r).abs() > 100.0 * margin
            {
                return no("a prism round's corner not at its rounds' meeting point");
            }
            ends.push(End { cap, v, g, c, t: ts, tpar, at_far, roll, corner });
        }
        let [a, b] = <[End; 2]>::try_from(ends).map_err(|_| Decline("a prism round without two ends"))?;
        if (b.c - a.c).cross(d).length() > margin {
            return no("a prism round's ends not on one axis");
        }
        // (Rolling over at both ends or neither, B the rectangle between the edge, its cap edges and the far edge.)
        for (k, &f) in fs.iter().enumerate() {
            match (a.roll[k], b.roll[k]) {
                (None, None) => {}
                (Some(x), Some(y)) if x.bc == y.bc => {
                    let lp = &body.faces[f as usize].loops;
                    if lp.len() != 1 || lp[0].len() != 4 || !lp[0].iter().all(|u| [be.edge, a.g[k], b.g[k], x.bc].contains(&u.edge)) {
                        return no("a prism round past a face not a rectangle");
                    }
                }
                _ => return no("a prism round past its end edges at one end"),
            }
        }
        // Nothing else within the round: the box of what it takes or adds (between the edge and its arcs, a corner's
        // sphere too) meets no face but its own two, its caps and its corners' rounds (slow1151's Fillet 2: an R12 round
        // through a 0.5 mm wall cut the wall's other side away).
        let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
        let mut own = vec![fs[0], fs[1]];
        own.extend(a.roll.iter().flatten().map(|x| x.face));
        for end in [&a, &b] {
            let mid = if chamfer { (end.t[0] + end.t[1]) * 0.5 } else { end.c + ((end.t[0] + end.t[1]) * 0.5 - end.c).normalize_or_zero() * r };
            let mut pts = vec![body.vertices[end.v as usize], end.t[0], end.t[1], mid];
            match &end.corner {
                Some(cn) => {
                    pts.push(body.vertices[cn.p as usize]);
                    own.extend(cn.tori);
                }
                None => own.push(end.cap),
            }
            for p in pts {
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        let shrink = DVec3::splat(10.0 * margin);
        let region = (lo + shrink, hi - shrink);
        for (fi, face) in body.faces.iter().enumerate() {
            if own.contains(&(fi as u32)) {
                continue;
            }
            let fb = face_box(body, face);
            if (region.0.cmplt(fb.1)).all() && (fb.0.cmplt(region.1)).all() {
                return no("another face within a prism round");
            }
        }
        plans.push(Plan { edge: be.edge, r, chamfer, fs, sigma, ends: [a, b] });
    }
    if (0..body.edges.len()).any(|i| absorbed[i] && !taken_m[i]) {
        return no("a curved edge not a prism round's corner");
    }
    let rolled: Vec<u32> = plans.iter().flat_map(|p| p.ends[0].roll.iter().flatten().map(|x| x.face)).collect();
    if rolled.iter().enumerate().any(|(i, f)| rolled[..i].contains(f) || plans.iter().any(|p| p.fs.contains(f))) {
        return no("a prism round rolling over onto another round's face");
    }
    // Two rounds on one face leave some of it between them, or use it up (the face the rectangle between their edges
    // and the caps' edges, it goes): exactly (a full round: their tangent points there one at each cap; one face,
    // live's), or wider than it (convex: two cylinders meeting along a ridge where their circles cross, live's).
    let mut fulls: Vec<Full> = Vec::new();
    let mut ridges: Vec<(usize, usize, usize, DVec3)> = Vec::new();
    for (i, a) in plans.iter().enumerate() {
        for (jb, bp) in plans.iter().enumerate().skip(i + 1) {
            for (ka, &f) in a.fs.iter().enumerate() {
                let Some(kb) = bp.fs.iter().position(|&g| g == f) else { continue };
                let crosses = |ea: &End, eb: &End, n: DVec3| match (a.chamfer, bp.chamfer) {
                    (false, false) => crossing(ea, a.r, ka, eb, bp.r, kb, n),
                    (true, true) => chamfers_cross(ea, ka, eb, kb, n),
                    _ => None,
                };
                let mut met = 0;
                let mut caps = 0;
                let mut crossed = Vec::new();
                let ae = body.edges[a.edge as usize];
                let n = (body.vertices[ae.v[1] as usize] - body.vertices[ae.v[0] as usize]).normalize();
                for (ia, ea) in a.ends.iter().enumerate() {
                    let Some(ib) = bp.ends.iter().position(|e| e.cap == ea.cap && e.cap != u32::MAX) else { continue };
                    let eb = &bp.ends[ib];
                    caps += 1;
                    let (pa, pb) = (body.vertices[ea.v as usize], body.vertices[eb.v as usize]);
                    if (ea.t[ka] - eb.t[kb]).length() <= 10.0 * margin && ea.g[ka] == eb.g[kb] {
                        met += 1;
                    } else if (ea.t[ka] - pa).length() + (eb.t[kb] - pb).length() >= (pa - pb).length() - 10.0 * margin {
                        let p = if a.sigma > 0.0 && bp.sigma > 0.0 && ea.g[ka] == eb.g[kb] { crosses(ea, eb, n) } else { None };
                        let Some(p) = p else { return no("two prism rounds meet across a face") };
                        crossed.push((ia, ib, p));
                    }
                }
                if met == 0 && crossed.is_empty() {
                    continue;
                }
                if !pairs {
                    return no("two prism rounds meet across a face");
                }
                // (Exactly the face's rectangle: its one loop the two edges and the two cap edges.)
                let face = &body.faces[f as usize];
                let rect = (met == 2 || crossed.len() == 2)
                    && caps == 2
                    && face.loops.len() == 1
                    && face.loops[0].len() == 4
                    && face.loops[0].iter().all(|u| u.edge == a.edge || u.edge == bp.edge || a.ends.iter().any(|e| e.g[ka] == u.edge));
                if !rect {
                    return no("two prism rounds meet across a face");
                }
                // (Met exactly: one cylinder where they are rounds of one radius, else a ridge there.)
                if met == 2 && (a.chamfer != bp.chamfer || (a.chamfer && a.sigma < 0.0)) {
                    return no("two prism rounds meet across a face");
                }
                fulls.push(Full { a: i, ka, b: jb, kb, face: f, one: met == 2 && !a.chamfer && (a.r - bp.r).abs() <= margin });
                for (ia, ib, p) in crossed {
                    ridges.push((i, ia, ka, p));
                    ridges.push((jb, ib, kb, p));
                }
            }
        }
    }
    for (pi, j, k, p) in ridges {
        plans[pi].ends[j].t[k] = p;
        plans[pi].ends[j].at_far[k] = false;
    }
    if plans.iter().any(|p| p.ends.iter().any(|e| e.at_far.contains(&true))) {
        return no("a prism round past its end edges");
    }
    build(body, plans, &fulls)
}

fn build(body: &Body, plans: Vec<Plan>, fulls: &[Full]) -> Result<Output, Decline> {
    let mut b = body.clone();
    let (nv, ne, nf) = (body.vertices.len(), body.edges.len(), body.faces.len());
    let mut vertex_origin: Vec<Origin> = (0..nv as u32).map(|i| Origin::Same { arg: 0, index: i }).collect();
    let mut edge_origin: Vec<Origin> = (0..ne as u32).map(|i| Origin::Same { arg: 0, index: i }).collect();
    let mut face_origin: Vec<Origin> = (0..nf as u32).map(|i| Origin::Same { arg: 0, index: i }).collect();
    let mut history = vec![Record { kind: Kind::Solid, arg: 0, index: 0, relation: Relation::Modified, out_kind: Kind::Solid, out: 0 }];
    let mut dead_v = vec![false; nv];
    let mut dead_e = vec![false; ne];
    let mut touched: Vec<u32> = Vec::new();
    let mut new_faces: Vec<(u32, Face)> = Vec::new();
    let mut corner_faces: Vec<(u32, Face)> = Vec::new();
    let add_v = |b: &mut Body, p: DVec3, vo: &mut Vec<Origin>| {
        b.vertices.push(p);
        vo.push(Origin::New);
        b.vertices.len() as u32 - 1
    };
    // A full round's face (by plan and side), its shared tangent vertex at each cap and its one line.
    let full_face = |pi: usize, k: usize| fulls.iter().find(|x| (x.a == pi && x.ka == k) || (x.b == pi && x.kb == k)).map(|x| x.face);
    let mut shared_v: std::collections::BTreeMap<(u32, u32), u32> = Default::default();
    let mut shared_line: std::collections::HashMap<u32, (u32, u32)> = Default::default();
    let mut dead_f = vec![false; nf];
    for x in fulls {
        dead_f[x.face as usize] = true;
        history.push(Record { kind: Kind::Face, arg: 0, index: x.face, relation: Relation::Deleted, out_kind: Kind::Face, out: u32::MAX });
    }
    for (pi, Plan { edge, r, chamfer, fs, sigma, ends }) in plans.into_iter().enumerate() {
        let e = body.edges[edge as usize];
        let d = (body.vertices[e.v[1] as usize] - body.vertices[e.v[0] as usize]).normalize();
        // Per end: the tangent points' vertices, the end edges cut back to them, the cap's arc between.
        let mut tv = [[0u32; 2]; 2];
        let mut arcs = [0u32; 2];
        let mut spheres = Vec::new();
        for (j, end) in ends.iter().enumerate() {
            dead_v[end.v as usize] = true;
            for (k, t) in tv[j].iter_mut().enumerate() {
                // (Rolling over B: the crossing on C's cap edge, which is cut back to it; B's cap edge goes.)
                if let Some(rl) = end.roll[k] {
                    *t = add_v(&mut b, end.t[k], &mut vertex_origin);
                    let slot = if b.edges[rl.h as usize].v[0] == rl.far { 0 } else { 1 };
                    b.edges[rl.h as usize].v[slot] = *t;
                    edge_origin[rl.h as usize] = Origin::Piece { arg: 0, index: rl.h };
                    history.push(Record { kind: Kind::Edge, arg: 0, index: rl.h, relation: Relation::Modified, out_kind: Kind::Edge, out: rl.h });
                    let g = end.g[k];
                    dead_e[g as usize] = true;
                    history.push(Record { kind: Kind::Edge, arg: 0, index: g, relation: Relation::Deleted, out_kind: Kind::Edge, out: u32::MAX });
                    history.push(Record { kind: Kind::Edge, arg: 0, index: rl.h, relation: Relation::Generated, out_kind: Kind::Vertex, out: *t });
                    for lp in &mut b.faces[end.cap as usize].loops {
                        lp.retain(|u| u.edge != g);
                    }
                    dead_v[rl.far as usize] = true;
                    continue;
                }
                // (A full round's face: one tangent vertex there for both rounds; the cap's edge along it goes.)
                if let Some(f) = full_face(pi, k) {
                    *t = match shared_v.get(&(f, end.cap)) {
                        Some(&v) => v,
                        None => {
                            let v = add_v(&mut b, end.t[k], &mut vertex_origin);
                            shared_v.insert((f, end.cap), v);
                            v
                        }
                    };
                    let g = end.g[k];
                    if !std::mem::replace(&mut dead_e[g as usize], true) {
                        history.push(Record { kind: Kind::Edge, arg: 0, index: g, relation: Relation::Deleted, out_kind: Kind::Edge, out: u32::MAX });
                    }
                    for lp in &mut b.faces[end.cap as usize].loops {
                        lp.retain(|u| u.edge != g);
                    }
                    continue;
                }
                *t = add_v(&mut b, end.t[k], &mut vertex_origin);
                let g = end.g[k] as usize;
                let slot = if b.edges[g].v[0] == end.v { 0 } else { 1 };
                b.edges[g].v[slot] = *t;
                // (A profile cut back to the tangent point: its range ends there.)
                if let (Curve::Nurbs { t0, t1, .. }, Some(tp)) = (&mut b.edges[g].curve, end.tpar[k]) {
                    *(if slot == 0 { t0 } else { t1 }) = tp;
                }
                edge_origin[g] = Origin::Piece { arg: 0, index: g as u32 };
                history.push(Record { kind: Kind::Edge, arg: 0, index: g as u32, relation: Relation::Modified, out_kind: Kind::Edge, out: g as u32 });
            }
            if let Some(cn) = &end.corner {
                // The round's end arc (from the first tangent point to the second), each torus's meridian through
                // its tangent point to the pole in place of the meeting edge, the pole a degenerate edge.
                let toward = body.vertices[end.v as usize] - end.c;
                let pa = end.t[0];
                let normal = if d.dot((pa - end.c).cross(toward)) > 0.0 { d } else { -d };
                b.edges.push(Edge { v: [tv[j][0], tv[j][1]], curve: Curve::Circle { center: end.c, normal, xdir: (pa - end.c).normalize(), radius: r } });
                edge_origin.push(Origin::New);
                arcs[j] = b.edges.len() as u32 - 1;
                let pp = body.vertices[cn.p as usize];
                let mut meridians = [0u32; 2];
                for k in 0..2 {
                    let tk = end.t[k];
                    let n = (tk - end.c).cross(pp - end.c).normalize();
                    b.edges.push(Edge { v: [tv[j][k], cn.p], curve: Curve::Circle { center: end.c, normal: n, xdir: (tk - end.c).normalize(), radius: r } });
                    edge_origin.push(Origin::New);
                    meridians[k] = b.edges.len() as u32 - 1;
                    // The torus ran the meeting edge to or from the pole: the meridian the same way.
                    let me = body.edges[cn.m as usize];
                    for lp in &mut b.faces[cn.tori[k] as usize].loops {
                        for x in lp.iter_mut() {
                            if x.edge == cn.m {
                                let to_p = (if x.reversed { me.v[0] } else { me.v[1] }) == cn.p;
                                *x = Use { edge: meridians[k], reversed: !to_p };
                            }
                        }
                    }
                    touched.push(cn.tori[k]);
                }
                dead_e[cn.m as usize] = true;
                history.push(Record { kind: Kind::Edge, arg: 0, index: cn.m, relation: Relation::Deleted, out_kind: Kind::Edge, out: u32::MAX });
                b.edges.push(Edge { v: [cn.p, cn.p], curve: Curve::Degenerate });
                edge_origin.push(Origin::New);
                spheres.push((j, end.v, end.c, pp, pa, arcs[j], meridians, b.edges.len() as u32 - 1, cn.tori));
                continue;
            }
            // The cap runs into the vertex along one end edge and out along the other: the arc between them its way.
            let cap = &body.faces[end.cap as usize];
            let into_v = |o: u32| {
                cap.loops.iter().flatten().find(|x| x.edge == o).map(|x| {
                    let oe = &body.edges[o as usize];
                    (if x.reversed { oe.v[0] } else { oe.v[1] }) == end.v
                })
            };
            let Some(in0) = into_v(end.g[0]) else { return no("a prism round's cap without its end edge") };
            let (from, to) = if in0 { (0, 1) } else { (1, 0) };
            let (pa, pb) = (end.t[from], end.t[to]);
            // The short arc: counter-clockwise from its start about the normal that turns it towards the vertex.
            let mid = (pa + pb) * 0.5 - end.c;
            let toward = if mid.length() > 1e-15 { mid } else { body.vertices[end.v as usize] - end.c };
            let normal = if d.dot((pa - end.c).cross(toward)) > 0.0 { d } else { -d };
            let xdir = (pa - end.c).normalize();
            let curve = if chamfer { Curve::Line } else { Curve::Circle { center: end.c, normal, xdir, radius: r } };
            b.edges.push(Edge { v: [tv[j][from], tv[j][to]], curve });
            edge_origin.push(Origin::New);
            arcs[j] = b.edges.len() as u32 - 1;
            let keys: Vec<u32> = end.g.iter().copied().chain(end.roll.iter().flatten().map(|x| x.h)).collect();
            let cap_loop = b.faces[end.cap as usize].loops.iter().position(|lp| lp.iter().any(|x| keys.contains(&x.edge))).unwrap_or(0);
            b.faces[end.cap as usize].loops[cap_loop].push(Use { edge: arcs[j], reversed: false });
            touched.push(end.cap);
        }
        // The lines where the round touches its faces, the old edge's way (its ends in its order; its faces use
        // them as they used it).
        let mut lines = [0u32; 2];
        let mut flipped = [false; 2];
        for k in 0..2 {
            // (Rolling over B: the line where it crosses C in place of B's far edge there, B gone.)
            if let Some(rl) = ends[0].roll[k] {
                b.edges.push(Edge { v: [tv[0][k], tv[1][k]], curve: Curve::Line });
                edge_origin.push(Origin::New);
                lines[k] = b.edges.len() as u32 - 1;
                let same = body.edges[rl.bc as usize].v[0] == rl.far;
                for lp in &mut b.faces[rl.face as usize].loops {
                    for x in lp.iter_mut() {
                        if x.edge == rl.bc {
                            *x = Use { edge: lines[k], reversed: x.reversed != !same };
                        }
                    }
                }
                dead_e[rl.bc as usize] = true;
                history.push(Record { kind: Kind::Edge, arg: 0, index: rl.bc, relation: Relation::Deleted, out_kind: Kind::Edge, out: u32::MAX });
                // (B goes; the line where the round crosses C is C's, as OCCT's, its ends its cap edges'.)
                if !std::mem::replace(&mut dead_f[fs[k] as usize], true) {
                    history.push(Record { kind: Kind::Face, arg: 0, index: fs[k], relation: Relation::Deleted, out_kind: Kind::Face, out: u32::MAX });
                }
                history.push(Record { kind: Kind::Face, arg: 0, index: rl.face, relation: Relation::Generated, out_kind: Kind::Edge, out: lines[k] });
                touched.push(rl.face);
                continue;
            }
            // (A full round's face: one line for both rounds, the second running it the other way if need be.)
            if let Some(f) = full_face(pi, k) {
                match shared_line.get(&f) {
                    Some(&(id, from)) => {
                        lines[k] = id;
                        flipped[k] = tv[0][k] != from;
                    }
                    None => {
                        b.edges.push(Edge { v: [tv[0][k], tv[1][k]], curve: Curve::Line });
                        edge_origin.push(Origin::New);
                        lines[k] = b.edges.len() as u32 - 1;
                        shared_line.insert(f, (lines[k], tv[0][k]));
                    }
                }
                continue;
            }
            b.edges.push(Edge { v: [tv[0][k], tv[1][k]], curve: Curve::Line });
            edge_origin.push(Origin::New);
            lines[k] = b.edges.len() as u32 - 1;
            let f = fs[k];
            for lp in &mut b.faces[f as usize].loops {
                for x in lp.iter_mut() {
                    if x.edge == edge {
                        x.edge = lines[k];
                    }
                }
            }
            touched.push(f);
        }
        dead_e[edge as usize] = true;
        // The round: each of its four edges the other way from the face beside it.
        let use_in = |b: &Body, f: u32, o: u32| b.faces[f as usize].loops.iter().flatten().find(|x| x.edge == o).map(|x| x.reversed);
        let mut lp = Vec::new();
        for (k, (f, o)) in [(fs[0], lines[0]), (fs[1], lines[1])].into_iter().enumerate() {
            // (A full round's face is gone: the way it ran the old edge, the shared line's way against this round's.)
            let rev = if let Some(rl) = ends[0].roll[k] {
                use_in(&b, rl.face, o)
            } else if full_face(pi, k).is_some() { body.faces[f as usize].loops.iter().flatten().find(|x| x.edge == edge).map(|x| x.reversed != flipped[k]) } else { use_in(&b, f, o) };
            let Some(rev) = rev else { return no("a prism round's edge not in its face") };
            lp.push(Use { edge: o, reversed: !rev });
        }
        // (Its first line run from the first end to the second: on along the second end's arc from its first tangent
        // point, back along the first's to it.)
        let forward = !lp[0].reversed;
        for j in 0..2 {
            let reversed = if ends[j].corner.is_some() {
                (j == 0) == forward
            } else {
                let Some(rev) = use_in(&b, ends[j].cap, arcs[j]) else { return no("a prism round's edge not in its face") };
                !rev
            };
            lp.push(Use { edge: arcs[j], reversed });
        }
        // Each corner's sphere: its arcs the other way from the round's and the tori's, the pole.
        for (j, v, c, pp, pa, arc, meridians, pole, tori) in spheres {
            let mut sl = vec![Use { edge: arc, reversed: !lp[2 + j].reversed }];
            for k in 0..2 {
                let Some(rev) = use_in(&b, tori[k], meridians[k]) else { return no("a prism round's corner meridian not in its torus") };
                sl.push(Use { edge: meridians[k], reversed: !rev });
            }
            sl.push(Use { edge: pole, reversed: false });
            let zdir = (c - pp).normalize();
            let xdir = (pa - c - zdir * (pa - c).dot(zdir)).normalize();
            corner_faces.push((v, Face { surface: Surface::Sphere { origin: c, xdir, ydir: zdir.cross(xdir), zdir, radius: r }, reversed: sigma < 0.0, loops: vec![sl] }));
        }
        let c = ends[0].c;
        if chamfer {
            // (Its normal towards the corner it cuts off, away from the one it fills.)
            let n = (ends[0].t[1] - ends[0].t[0]).cross(d).normalize();
            let normal = if n.dot(c - ends[0].t[0]) * sigma > 0.0 { n } else { -n };
            new_faces.push((edge, Face { surface: Surface::Plane { origin: ends[0].t[0], normal }, reversed: false, loops: vec![lp] }));
            continue;
        }
        let xdir = (ends[0].t[0] - c).normalize();
        let axis = if d.cross(xdir).dot(ends[0].t[1] - c) >= 0.0 { d } else { -d };
        new_faces.push((edge, Face { surface: Surface::Cylinder { origin: c, axis, xdir, radius: r }, reversed: sigma < 0.0, loops: vec![lp] }));
    }
    // A full round is one face, as live's: the two rounds' faces (one cylinder) joined across their line. (Each cap's
    // two arcs stay, as after OCCT's merge_full_rounds; the shown topology joins them.)
    let mut merged_into: Vec<Option<usize>> = vec![None; new_faces.len()];
    dead_e.resize(b.edges.len(), false);
    // (A ridge: generated by both rounds' edges.)
    for x in fulls.iter().filter(|x| !x.one) {
        let Some(&(line, _)) = shared_line.get(&x.face) else { return no("two rounds' ridge without its line") };
        for e in [new_faces[x.a].0, new_faces[x.b].0] {
            history.push(Record { kind: Kind::Edge, arg: 0, index: e, relation: Relation::Generated, out_kind: Kind::Edge, out: line });
        }
    }
    for x in fulls.iter().filter(|x| x.one) {
        let Some(&(line, _)) = shared_line.get(&x.face) else { return no("a full round without its line") };
        let lp: Vec<Use> = new_faces[x.a].1.loops[0].iter().chain(&new_faces[x.b].1.loops[0]).filter(|u| u.edge != line).copied().collect();
        dead_e[line as usize] = true;
        // (Its seam at one end of the half round: the arc into a meeting point run from its start.)
        let Some((_, &m)) = shared_v.range((x.face, 0)..=(x.face, u32::MAX)).next() else { return no("a full round without its arcs") };
        let Some(u) = lp.iter().find(|u| b.edges[u.edge as usize].v[if u.reversed { 0 } else { 1 }] == m) else { return no("a full round's arcs not through their meeting point") };
        let Curve::Circle { center, normal, .. } = b.edges[u.edge as usize].curve else { return no("a full round's arc not a circle") };
        let start = b.edges[u.edge as usize].v[if u.reversed { 1 } else { 0 }];
        let axis = if u.reversed { -normal } else { normal };
        let xdir = (b.vertices[start as usize] - center).normalize();
        let Surface::Cylinder { radius, .. } = new_faces[x.a].1.surface else { return no("a full round not a cylinder") };
        new_faces[x.a].1.surface = Surface::Cylinder { origin: center, axis, xdir, radius };
        new_faces[x.a].1.loops = vec![lp];
        merged_into[x.b] = Some(x.a);
    }
    touched.sort_unstable();
    touched.dedup();
    touched.retain(|&f| !dead_f[f as usize]);
    for f in touched {
        face_origin[f as usize] = Origin::Piece { arg: 0, index: f };
        history.push(Record { kind: Kind::Face, arg: 0, index: f, relation: Relation::Modified, out_kind: Kind::Face, out: f });
    }
    let mut round_face = vec![0u32; new_faces.len()];
    for (i, (_, face)) in new_faces.iter().enumerate() {
        if merged_into[i].is_some() {
            continue;
        }
        b.faces.push(face.clone());
        face_origin.push(Origin::New);
        round_face[i] = b.faces.len() as u32 - 1;
        b.solids[0].shells[0].push(round_face[i]);
    }
    for (i, &(e, _)) in new_faces.iter().enumerate() {
        let fi = round_face[merged_into[i].unwrap_or(i)];
        history.push(Record { kind: Kind::Edge, arg: 0, index: e, relation: Relation::Generated, out_kind: Kind::Face, out: fi });
        history.push(Record { kind: Kind::Edge, arg: 0, index: e, relation: Relation::Deleted, out_kind: Kind::Edge, out: u32::MAX });
    }
    for (v, face) in corner_faces {
        b.faces.push(face);
        face_origin.push(Origin::New);
        let fi = b.faces.len() as u32 - 1;
        b.solids[0].shells[0].push(fi);
        history.push(Record { kind: Kind::Vertex, arg: 0, index: v, relation: Relation::Generated, out_kind: Kind::Face, out: fi });
    }
    for (v, &dv) in dead_v.iter().enumerate() {
        if dv {
            history.push(Record { kind: Kind::Vertex, arg: 0, index: v as u32, relation: Relation::Deleted, out_kind: Kind::Vertex, out: u32::MAX });
        }
    }
    // Drop a full round's faces (renumbering the rest, the shell's list and the history's).
    if dead_f.iter().any(|&d| d) {
        let mut fmap = vec![u32::MAX; b.faces.len()];
        let (mut fs_out, mut fo) = (Vec::new(), Vec::new());
        for (i, f) in b.faces.iter().enumerate() {
            if i < nf && dead_f[i] {
                continue;
            }
            fmap[i] = fs_out.len() as u32;
            fs_out.push(f.clone());
            fo.push(face_origin[i]);
        }
        b.faces = fs_out;
        face_origin = fo;
        for sh in &mut b.solids[0].shells {
            sh.retain(|&f| fmap[f as usize] != u32::MAX);
            for f in sh.iter_mut() {
                *f = fmap[*f as usize];
            }
        }
        for h in &mut history {
            if h.relation != Relation::Deleted && h.out_kind == Kind::Face {
                h.out = fmap[h.out as usize];
            }
        }
    }
    // Drop the dead vertices and edges (renumbering the rest).
    let mut vmap = vec![u32::MAX; b.vertices.len()];
    let (mut vs, mut vo) = (Vec::new(), Vec::new());
    for (i, p) in b.vertices.iter().enumerate() {
        if i < nv && dead_v[i] {
            continue;
        }
        vmap[i] = vs.len() as u32;
        vs.push(*p);
        vo.push(vertex_origin[i]);
    }
    let mut emap = vec![u32::MAX; b.edges.len()];
    let (mut es, mut eo) = (Vec::new(), Vec::new());
    for (i, e) in b.edges.iter().enumerate() {
        if dead_e.get(i) == Some(&true) {
            continue;
        }
        emap[i] = es.len() as u32;
        es.push(Edge { v: [vmap[e.v[0] as usize], vmap[e.v[1] as usize]], curve: e.curve });
        eo.push(edge_origin[i]);
    }
    if es.iter().any(|e| e.v.contains(&u32::MAX)) {
        return no("an edge left at a vertex a prism round takes");
    }
    for f in &mut b.faces {
        for lp in &mut f.loops {
            for u in lp.iter_mut() {
                u.edge = emap[u.edge as usize];
                if u.edge == u32::MAX {
                    return no("a face left with an edge a prism round takes");
                }
            }
        }
    }
    for h in &mut history {
        if h.relation != Relation::Deleted && h.out_kind == Kind::Edge {
            h.out = emap[h.out as usize];
        }
        if h.relation != Relation::Deleted && h.out_kind == Kind::Vertex {
            h.out = vmap[h.out as usize];
        }
    }
    if !vs.iter().all(|p| p.is_finite()) {
        return no("a prism round whose geometry came out wrong");
    }
    b.vertices = vs;
    b.edges = es;
    let face_u0 = vec![0.0; b.faces.len()];
    let edge_section = vec![u32::MAX; b.edges.len()];
    history.sort_unstable();
    history.dedup();
    Ok(Output { body: b, vertex_origin: vo, edge_origin: eo, face_origin, solid_origin: vec![Origin::Piece { arg: 0, index: 0 }], face_u0, edge_section, edge_apex: Vec::new(), face_internal: Vec::new(), history })
}
