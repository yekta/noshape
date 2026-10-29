//! Fuse, cut and common of solids bounded by planes and cylinders, with OCCT's result and history (see the
//! crate docs).
//!
//! 1. Every vertex goes into one pool; points within `tol` of each other are one vertex.
//! 2. Each pair of faces of two solids from different arguments: where their surfaces meet (a line, a
//!    circle) is clipped to both faces, and the pieces inside both are added to both; an edge lying on the
//!    other face's surface adds its pieces inside that face.
//! 3. Every edge and added curve is split at the pool's vertices on it.
//! 4. Each face is split, in its parameter plane, into the regions its pieces bound; a region inside the face
//!    is a piece of it.
//! 5. Each piece is in, out of, or on (coincident with a face of) each other solid; the op keeps some.
//! 6. The kept pieces make closed shells, one solid each.
//!
//! Cylinders are taken where every cylinder of the inputs is parallel to one axis and every plane is square
//! or parallel to it: then surfaces meet in lines along the axis or circles around it.

use crate::arrange::{deeper, regions};
use crate::curve2::{distance as distance2, inside as inside2, C2};
use crate::freeform::FreeSurface;
use crate::nurbs::NurbsCurve;
use crate::{Body, Curve, Decline, Edge, Face, Kind, Origin, Output, Record, Relation, SectionSource, Solid, Surface, Use, GREY};
use std::sync::Arc;
use glam::{DVec2, DVec3};
use crate::fxhash::{HashMap, HashSet};
use std::f64::consts::{PI, TAU};

mod free;
mod line;

/// The Boolean operation: `args[0]` is the object, the other arguments the tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Fuse,
    Cut,
    Common,
}

type R<T> = Result<T, Decline>;

/// A circle: centre, unit normal, and the frame its angles are measured in (`y = n × x`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Circ {
    c: DVec3,
    n: DVec3,
    x: DVec3,
    y: DVec3,
    r: f64,
}

impl Circ {
    fn at(&self, a: f64) -> DVec3 {
        self.c + (self.x * a.cos() + self.y * a.sin()) * self.r
    }

    fn angle(&self, p: DVec3) -> f64 {
        let w = p - self.c;
        w.dot(self.y).atan2(w.dot(self.x))
    }
}

/// An ellipse: centre, unit normal, and its axes: `x` the major one (radius `a`), `y = n × x` the minor one
/// (radius `b`); parameter t at `c + a cos t x + b sin t y` (OCCT's). `hyp`: the hyperbola's branch
/// `c + a cosh t x + b sinh t y` instead (OCCT's; not closed).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ell {
    c: DVec3,
    n: DVec3,
    x: DVec3,
    y: DVec3,
    a: f64,
    b: f64,
    hyp: bool,
}

impl Ell {
    fn at(&self, t: f64) -> DVec3 {
        if self.hyp {
            return self.c + self.x * (self.a * t.cosh()) + self.y * (self.b * t.sinh());
        }
        self.c + self.x * (self.a * t.cos()) + self.y * (self.b * t.sin())
    }

    fn deriv(&self, t: f64) -> DVec3 {
        if self.hyp {
            return self.x * (self.a * t.sinh()) + self.y * (self.b * t.cosh());
        }
        self.y * (self.b * t.cos()) - self.x * (self.a * t.sin())
    }

    /// The parameter of the point of it nearest `p` (from its angle, then Newton's method).
    fn param(&self, p: DVec3) -> f64 {
        let w = p - self.c;
        let mut t = if self.hyp { (w.dot(self.y) / self.b).asinh() } else { (w.dot(self.y) / self.b).atan2(w.dot(self.x) / self.a) };
        for _ in 0..8 {
            let (d1, q) = (self.deriv(t), self.at(t) - p);
            let d2 = if self.hyp { self.at(t) - self.c } else { self.c - self.at(t) };
            let g = q.dot(d1);
            let gp = d1.length_squared() + q.dot(d2);
            if gp.abs() < 1e-300 {
                break;
            }
            let step = g / gp;
            t -= step;
            if step.abs() < 1e-15 {
                break;
            }
        }
        t
    }

    /// A box around its piece from `t0` through `sweep` (an ellipse: the whole of it).
    fn piece_box(&self, t0: f64, sweep: f64) -> (DVec3, DVec3) {
        if !self.hyp {
            return (self.c - DVec3::splat(self.a), self.c + DVec3::splat(self.a));
        }
        // Samples, grown by h² / 8 times its largest second derivative there (|at(t) - c|).
        const N: usize = 32;
        let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
        for k in 0..=N {
            let p = self.at(t0 + sweep * k as f64 / N as f64);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        let big = t0.abs().max((t0 + sweep).abs());
        let bulge = DVec3::splat((self.a * big.cosh() + self.b * big.sinh()) * (sweep / N as f64).powi(2) / 8.0);
        (lo - bulge, hi + bulge)
    }

    /// Its length from `t0` to `t1` (Gauss–Legendre on 16 pieces).
    fn length(&self, t0: f64, t1: f64) -> f64 {
        const N: usize = 16;
        const G: [(f64, f64); 4] = [
            (0.069_431_844_202_973_71, 0.173_927_422_568_726_9),
            (0.330_009_478_207_571_9, 0.326_072_577_431_273_1),
            (0.669_990_521_792_428_1, 0.326_072_577_431_273_1),
            (0.930_568_155_797_026_3, 0.173_927_422_568_726_9),
        ];
        let h = (t1 - t0) / N as f64;
        (0..N).map(|k| G.iter().map(|&(x, w)| w * self.deriv(t0 + h * (k as f64 + x)).length()).sum::<f64>() * h).sum()
    }
}

/// A curve between two pool vertices: a segment, an arc from angle `a0` through `sweep` (> 0) of its
/// circle, or a piece of an ellipse from parameter `t0` through `sweep` (> 0), the section of `faces`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Geo {
    Line,
    Arc { k: Circ, a0: f64, sweep: f64 },
    Ell { e: Ell, t0: f64, sweep: f64, faces: [usize; 2] },
    /// No curve: an edge at a cone's apex.
    Apex,
    /// `Ctx::ncurves[id]` from parameter `t0` to `t1` (an input's B-spline edge, or a piece of OCCT's section of
    /// two faces, one of them free-form).
    Nurbs { id: u32, t0: f64, t1: f64 },
}

#[derive(Clone, Copy, Debug)]
struct Seg {
    v: [u32; 2],
    g: Geo,
}

enum Surf {
    /// Outward normal, offset (`n · x == d`), and the 2D frame (`u × v == n`); `flip`: its surface's normal
    /// is -n.
    Plane { n: DVec3, d: f64, origin: DVec3, u: DVec3, v: DVec3, flip: bool },
    /// A cylinder or cone: axis origin and direction, angle frame, radius `r0 + k h` at height h along the
    /// axis; `flip`: the outward normal points to the axis. Parameters (r θ, ±h) with θ in [th0, th0 + 2π)
    /// and `r` a scale (a cylinder's radius, a cone's largest on the face); `full`: the face goes round (a
    /// seam at th0).
    Cyl { o: DVec3, a: DVec3, x: DVec3, y: DVec3, r: f64, r0: f64, k: f64, flip: bool, th0: f64, full: bool },
    /// A NURBS surface (`GFace::ns`): parameters (su u, sv v), (u, v) its own (OCCT's), scaled to lengths along it;
    /// `flip`: the outward normal is minus Su × Sv.
    Nurbs { su: f64, sv: f64, flip: bool },
}

/// A NURBS curve the Boolean works with: an input's B-spline edge, or OCCT's section of two faces (`sec`: the
/// faces, its curves on them in their own parameters (OCCT's), the source's id for it).
struct NCurve {
    c: Arc<NurbsCurve>,
    sec: Option<([usize; 2], [Arc<NurbsCurve>; 2], u32)>,
    /// OCCT's tolerance for a section (0 for an input's curve).
    tol: f64,
}

/// A free-form face's edges' curves on it: by (edge, a seam's second), the curve and its range.
type PCurves = HashMap<(usize, bool), (Arc<NurbsCurve>, f64, f64)>;

struct GFace {
    arg: u32,
    index: u32,
    solid: usize,
    s: Surf,
    /// Its input boundary, every loop together.
    edges: Vec<(usize, bool)>,
    /// The same in its parameter plane (a seam twice), made when first asked for ([`Ctx::bound`]).
    bound: std::cell::OnceCell<R<Vec<C2>>>,
    lo: DVec3,
    hi: DVec3,
    /// A NURBS face's surface, its edges' curves on it (by edge, a seam's second: OCCT's, in its own parameters,
    /// from the edge's start to its end), and samples for finding a point's parameters.
    ns: Option<Arc<FreeSurface>>,
    /// A free-form face's parameter box (its edges' curves on it span), (u0, u1, v0, v1).
    uvbox: [f64; 4],
    pcurves: PCurves,
    grid: std::cell::OnceCell<Vec<(DVec2, DVec3)>>,
    patches: std::cell::OnceCell<Vec<free::Patch>>,
}

impl GFace {
    fn to2(&self, p: DVec3) -> DVec2 {
        match self.s {
            Surf::Plane { origin, u, v, .. } => {
                let q = p - origin;
                DVec2::new(q.dot(u), q.dot(v))
            }
            Surf::Cyl { o, a, x, y, r, flip, th0, .. } => {
                let w = p - o;
                let th = th0 + (w.dot(y).atan2(w.dot(x)) - th0).rem_euclid(TAU);
                let h = w.dot(a);
                DVec2::new(r * th, if flip { -h } else { h })
            }
            Surf::Nurbs { su, sv, .. } => {
                let uv = self.uv_of(p);
                DVec2::new(uv.x * su, uv.y * sv)
            }
        }
    }

    /// A point's parameters on a NURBS face's surface (from the nearest of its samples).
    fn uv_of(&self, p: DVec3) -> DVec2 {
        let Some(ns) = &self.ns else { return DVec2::ZERO };
        let grid = self.grid.get_or_init(|| free::samples(ns, self.uvbox));
        let seed = grid.iter().min_by(|a, b| (a.1 - p).length_squared().total_cmp(&(b.1 - p).length_squared())).map_or(DVec2::ZERO, |x| x.0);
        ns.param_of(p, seed, self.uvbox).0
    }

    fn to3(&self, q: DVec2) -> DVec3 {
        match self.s {
            Surf::Plane { origin, u, v, .. } => origin + u * q.x + v * q.y,
            Surf::Cyl { o, a, x, y, r, r0, k, flip, .. } => {
                let th = q.x / r;
                let h = if flip { -q.y } else { q.y };
                o + a * h + (x * th.cos() + y * th.sin()) * (r0 + k * h)
            }
            Surf::Nurbs { su, sv, .. } => self.ns.as_ref().map_or(DVec3::ZERO, |ns| ns.at(q.x / su, q.y / sv)),
        }
    }

    /// The curve `g` from `p0` (in 3D) in this face's parameter plane, `copy` 1 for the right side of a seam.
    fn curve2(&self, p0: DVec3, p1: DVec3, g: &Geo, copy: usize) -> R<C2> {
        match (&self.s, g) {
            (Surf::Nurbs { .. }, _) | (_, Geo::Nurbs { .. }) => Err(Decline("a curve on a free-form face")),
            (Surf::Plane { .. }, Geo::Line) => Ok(C2::Seg { a: self.to2(p0), b: self.to2(p1) }),
            (Surf::Plane { .. }, Geo::Apex) => Err(Decline("an apex on a plane")),
            (Surf::Plane { n, .. }, Geo::Arc { k, sweep, .. }) => {
                let c = self.to2(k.c);
                let d = self.to2(p0) - c;
                let s = if k.n.dot(*n) > 0.0 { *sweep } else { -*sweep };
                Ok(C2::Arc { c, r: k.r, a0: d.y.atan2(d.x), sweep: s })
            }
            (Surf::Plane { u, v, .. }, Geo::Ell { e, t0, sweep, .. }) => {
                let lin = |w: DVec3| DVec2::new(w.dot(*u), w.dot(*v));
                if e.hyp {
                    return Ok(C2::Hyp { c: self.to2(e.c), u: lin(e.x) * e.a, v: lin(e.y) * e.b, w: DVec2::ZERO, k: 0.0, t0: *t0, sweep: *sweep });
                }
                Ok(C2::Trig { c: self.to2(e.c), u: lin(e.x) * e.a, v: lin(e.y) * e.b, w: DVec2::ZERO, t0: *t0, sweep: *sweep })
            }
            (Surf::Cyl { k: slope, .. }, Geo::Ell { e, .. }) if *slope != 0.0 && !e.hyp => Err(Decline("an ellipse on a cone")),
            (Surf::Cyl { k: slope, .. }, Geo::Ell { e, .. }) if *slope == 0.0 && e.hyp => Err(Decline("a hyperbola on a cylinder")),
            (Surf::Cyl { r, th0, a, o, full, .. }, g) => {
                if *g == Geo::Apex {
                    if !*full {
                        return Err(Decline("an apex on a part of a cone"));
                    }
                    let y = self.to2(p0).y;
                    return Ok(C2::Seg { a: DVec2::new(r * th0, y), b: DVec2::new(r * (th0 + TAU), y) });
                }
                // A line's angle from its end off the axis (a cone's generator may start at the apex).
                let off = |p: DVec3| (p - *o - *a * (p - *o).dot(*a)).length();
                let mut q0 = if *g == Geo::Line && off(p1) > off(p0) {
                    DVec2::new(self.to2(p1).x, self.to2(p0).y)
                } else {
                    self.to2(p0)
                };
                // At the start of its angles (rounding may give the end).
                let (at0, at1) = ((q0.x - r * th0).abs() <= 1e-9 * r.max(1.0), (q0.x - r * (th0 + TAU)).abs() <= 1e-9 * r.max(1.0));
                if at1 {
                    q0.x = r * th0;
                }
                let seam = *full && (at0 || at1);
                if seam && copy == 1 {
                    q0.x = r * (th0 + TAU);
                }
                match g {
                    Geo::Apex | Geo::Nurbs { .. } => unreachable!(),
                    Geo::Line => {
                        let q1 = DVec2::new(q0.x, self.to2(p1).y);
                        Ok(C2::Seg { a: q0, b: q1 })
                    }
                    Geo::Arc { k, sweep, .. } => {
                        let along = if k.n.dot(*a) > 0.0 { 1.0 } else { -1.0 };
                        let ds = along * sweep * r;
                        if ds < 0.0 && (at0 || at1) {
                            q0.x = r * (th0 + TAU);
                        }
                        let x1 = q0.x + ds;
                        if x1 < r * th0 - 1e-9 * r || x1 > r * (th0 + TAU) + 1e-9 * r {
                            return Err(Decline("an arc across a cylinder's seam"));
                        }
                        Ok(C2::Seg { a: q0, b: DVec2::new(x1, q0.y) })
                    }
                    Geo::Ell { e, t0, sweep, .. } if e.hyp => {
                        // Seen along the axis: from its foot on the plane (nearest the axis, at |dd|) along y,
                        // b sinh t; its angle φ0 + s atan(b sinh t / |dd|), its height hc + A cosh t.
                        let Surf::Cyl { flip, .. } = self.s else { unreachable!() };
                        let foot = (e.c - *o) - *a * (e.c - *o).dot(*a);
                        let dd = foot.length();
                        let s = if foot.cross(e.y).dot(*a) > 0.0 { 1.0 } else { -1.0 };
                        let k = e.b / dd;
                        let ang = |t: f64| s * (k * t.sinh()).atan();
                        let dth = ang(t0 + sweep) - ang(*t0);
                        if dth < 0.0 && (at0 || at1) {
                            q0.x = r * (th0 + TAU);
                        }
                        let x1 = q0.x + r * dth;
                        if x1 < r * th0 - 1e-9 * r || x1 > r * (th0 + TAU) + 1e-9 * r {
                            return Err(Decline("a hyperbola across a cone's seam"));
                        }
                        let (hc, amp) = ((e.c - *o).dot(*a), e.a * e.x.dot(*a));
                        let sh = if flip { -1.0 } else { 1.0 };
                        let phi0 = q0.x / r - ang(*t0);
                        Ok(C2::Hyp { c: DVec2::new(r * phi0, sh * hc), u: DVec2::new(0.0, sh * amp), v: DVec2::ZERO, w: DVec2::new(r * s, 0.0), k, t0: *t0, sweep: *sweep })
                    }
                    Geo::Ell { e, t0, sweep, .. } => {
                        // Seen along the axis the ellipse is the cylinder's circle: its angle φ0 + s t, its height
                        // hc + A cos t.
                        let Surf::Cyl { flip, .. } = self.s else { unreachable!() };
                        let xp = (e.x - *a * e.x.dot(*a)).normalize();
                        let s = if xp.cross(e.y).dot(*a) > 0.0 { 1.0 } else { -1.0 };
                        let ds = s * sweep * r;
                        if ds < 0.0 && (at0 || at1) {
                            q0.x = r * (th0 + TAU);
                        }
                        let x1 = q0.x + ds;
                        if x1 < r * th0 - 1e-9 * r || x1 > r * (th0 + TAU) + 1e-9 * r {
                            return Err(Decline("an ellipse across a cylinder's seam"));
                        }
                        let (hc, amp) = ((e.c - *o).dot(*a), e.a * e.x.dot(*a));
                        let sh = if flip { -1.0 } else { 1.0 };
                        let phi0 = q0.x / r - s * t0;
                        Ok(C2::Trig { c: DVec2::new(r * phi0, sh * hc), u: DVec2::new(0.0, sh * amp), v: DVec2::ZERO, w: DVec2::new(r * s, 0.0), t0: *t0, sweep: *sweep })
                    }
                }
            }
        }
    }
}

struct GEdge {
    arg: u32,
    index: u32,
    v: [u32; 2],
    g: Geo,
}

struct GSolid {
    arg: u32,
    index: u32,
    faces: Vec<usize>,
    lo: DVec3,
    hi: DVec3,
}

/// Every vertex, input and new, by position.
struct Pool {
    p: Vec<DVec3>,
    /// The input vertices (arg, index) that are this one.
    inputs: Vec<Vec<(u32, u32)>>,
    grid: HashMap<(i64, i64, i64), Vec<u32>>,
    cell: f64,
    tol: f64,
}

impl Pool {
    fn key(&self, p: DVec3) -> (i64, i64, i64) {
        ((p.x / self.cell).floor() as i64, (p.y / self.cell).floor() as i64, (p.z / self.cell).floor() as i64)
    }

    /// The vertex within `tol` of `p`, if any.
    fn find(&self, p: DVec3) -> R<Option<u32>> {
        let k = self.key(p);
        let mut found: Option<u32> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(list) = self.grid.get(&(k.0 + dx, k.1 + dy, k.2 + dz)) else { continue };
                    for &i in list {
                        let d = (self.p[i as usize] - p).length();
                        if d <= self.tol {
                            if found.is_some_and(|f| f != i) {
                                return Err(Decline("a point near two vertices"));
                            }
                            found = Some(i);
                        } else if d <= GREY * self.tol {
                            return Err(Decline("two vertices nearly coincide"));
                        }
                    }
                }
            }
        }
        Ok(found)
    }

    fn push(&mut self, p: DVec3) -> u32 {
        let i = self.p.len() as u32;
        self.p.push(p);
        self.inputs.push(Vec::new());
        let k = self.key(p);
        self.grid.entry(k).or_default().push(i);
        i
    }

    fn add(&mut self, p: DVec3) -> R<u32> {
        if let Some(i) = self.find(p)? {
            return Ok(i);
        }
        Ok(self.push(p))
    }
}

/// A split edge: a curve between two pool vertices, no pool vertex inside it.
struct SEdge {
    seg: Seg,
    /// The input edges it is (a piece of).
    origins: Vec<usize>,
}

/// A piece of an input face.
struct Piece {
    face: usize,
    loops: Vec<Vec<(usize, bool)>>,
    inside: DVec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Out,
    In,
    OnSame,
    OnOpposite,
}

/// A curve on two faces' surfaces, as parameters: a line `p + t d` (`d` unit), a circle by angle, or an
/// ellipse by its parameter.
#[derive(Clone)]
enum Path {
    Line { p: DVec3, d: DVec3 },
    Circle(Circ),
    Ellipse(Ell),
    /// `Ctx::ncurves[id]` (`c` its curve): OCCT's section of two faces, one of them free-form.
    Nurbs { id: u32, c: Arc<NurbsCurve> },
}

impl Path {
    fn at(&self, t: f64) -> DVec3 {
        match self {
            Path::Line { p, d } => *p + *d * t,
            Path::Circle(k) => k.at(t),
            Path::Ellipse(e) => e.at(t),
            Path::Nurbs { c, .. } => c.at(t),
        }
    }

    fn geo(&self, t0: f64, t1: f64) -> Geo {
        match self {
            Path::Line { .. } => Geo::Line,
            Path::Circle(k) => Geo::Arc { k: *k, a0: t0, sweep: t1 - t0 },
            // (Its faces set by the section.)
            Path::Ellipse(e) => Geo::Ell { e: *e, t0, sweep: t1 - t0, faces: [usize::MAX; 2] },
            Path::Nurbs { id, .. } => Geo::Nurbs { id: *id, t0, t1 },
        }
    }

    /// Its length from `t0` to `t1`.
    fn length(&self, t0: f64, t1: f64) -> f64 {
        match self {
            Path::Line { .. } => t1 - t0,
            Path::Circle(k) => (t1 - t0) * k.r,
            Path::Ellipse(e) => e.length(t0, t1),
            Path::Nurbs { c, .. } => free::length(c, t0, t1),
        }
    }
}

/// Where a path is cut: a vertex, a point where it crosses a face's boundary (made a vertex only when a
/// piece ends there), or the end of its parameter range (no vertex: a circle cut open, a line's far end).
#[derive(Clone, Copy, Debug, PartialEq)]
enum End {
    Open,
    Point(DVec3),
    Vertex(u32),
}

impl End {
    fn rank(&self) -> u8 {
        match self {
            End::Open => 0,
            End::Point(_) => 1,
            End::Vertex(_) => 2,
        }
    }
}

/// Pool vertices sorted by x, by the argument they belong to (None: new or several arguments').
type ByOwner = Vec<(Option<u32>, Vec<(f64, u32)>)>;

/// An interval of a path inside a face: its ends (parameter, end).
type Ival = [(f64, End); 2];

struct Ctx<'a> {
    tol: f64,
    pool: Pool,
    faces: Vec<GFace>,
    edges: Vec<GEdge>,
    solids: Vec<GSolid>,
    side_cache: HashMap<(u32, usize), i8>,
    in_face_done: HashSet<(usize, usize)>,
    /// Curves added to each face.
    face_segs: Vec<Vec<Seg>>,
    coplanar: Vec<(usize, usize)>,
    /// Where two faces cross (the curve, the faces).
    sections: Vec<(Seg, usize, usize)>,
    /// NURBS curves (`Geo::Nurbs`), and where OCCT's sections of free-form faces come from.
    ncurves: Vec<NCurve>,
    src: Option<&'a dyn SectionSource>,
}

fn frame(n: DVec3) -> (DVec3, DVec3) {
    let a = if n.x.abs() <= n.y.abs() && n.x.abs() <= n.z.abs() {
        DVec3::X
    } else if n.y.abs() <= n.z.abs() {
        DVec3::Y
    } else {
        DVec3::Z
    };
    let u = n.cross(a).normalize();
    (u, n.cross(u))
}

fn boxes_meet(alo: DVec3, ahi: DVec3, blo: DVec3, bhi: DVec3, tol: f64) -> bool {
    alo.x <= bhi.x + tol && blo.x <= ahi.x + tol && alo.y <= bhi.y + tol && blo.y <= ahi.y + tol && alo.z <= bhi.z + tol && blo.z <= ahi.z + tol
}

impl Ctx<'_> {
    fn p(&self, v: u32) -> DVec3 {
        self.pool.p[v as usize]
    }

    /// The side of planar face `f`'s plane vertex `v` is on: 0 within tol.
    fn side(&mut self, v: u32, f: usize) -> R<i8> {
        if let Some(&s) = self.side_cache.get(&(v, f)) {
            return Ok(s);
        }
        let Surf::Plane { n, d, .. } = self.faces[f].s else { return Ok(1) };
        let x = n.dot(self.p(v)) - d;
        let s = if x.abs() <= self.tol {
            0
        } else if x.abs() <= GREY * self.tol {
            return Err(Decline("a vertex nearly on a plane"));
        } else if x > 0.0 {
            1
        } else {
            -1
        };
        self.side_cache.insert((v, f), s);
        Ok(s)
    }

    /// Face `f`'s outward normal at `p` (on its surface).
    fn normal_at(&self, f: usize, p: DVec3) -> DVec3 {
        match self.faces[f].s {
            Surf::Plane { n, .. } => n,
            Surf::Cyl { o, a, k, flip, .. } => {
                let w = p - o;
                let n = ((w - a * w.dot(a)).normalize() - a * k).normalize();
                if flip { -n } else { n }
            }
            Surf::Nurbs { flip, .. } => {
                let g = &self.faces[f];
                let uv = g.uv_of(p);
                let d = g.ns.as_ref().map_or([DVec3::Z; 6], |ns| ns.derivs(uv.x, uv.y));
                let n = d[1].cross(d[2]).normalize_or_zero();
                if flip { -n } else { n }
            }
        }
    }

    /// Segment `s` in face `f`'s parameter plane (`copy` 1: a seam's right side; `edge`: the input edge it is
    /// (a piece of), whose curve on a free-form face is OCCT's).
    fn c2_of(&self, f: usize, s: &Seg, copy: usize, edge: Option<usize>) -> R<C2> {
        let g = &self.faces[f];
        if let Surf::Nurbs { su, sv, .. } = g.s {
            if let Geo::Nurbs { id, t0, t1 } = s.g
                && let Some((faces, on, _)) = &self.ncurves[id as usize].sec
            {
                let k = if faces[0] == f { 0 } else if faces[1] == f { 1 } else { return Err(Decline("a section on a third face")) };
                return Ok(C2::Nurbs { c: free::scaled(&on[k], su, sv), t0, t1 });
            }
            let e = edge.ok_or(Decline("a curve on a free-form face not its own"))?;
            let (pc, pt0, pt1) = g.pcurves.get(&(e, copy == 1)).ok_or(Decline("an edge without its curve on a free-form face"))?;
            let c = free::scaled(pc, su, sv);
            let (a, b) = match s.g {
                Geo::Nurbs { t0, t1, .. } => (t0, t1),
                // (A degenerate edge (the surface closing at a point): its whole curve on the face.)
                Geo::Apex => (*pt0, *pt1),
                _ => {
                    // A piece of a straight or round edge: its ends' parameters on the edge's curve on the face.
                    let whole = C2::Nurbs { c: c.clone(), t0: *pt0, t1: *pt1 };
                    let at = |p: DVec3| {
                        let (t, _) = whole.param_of(g.to2(p));
                        pt0 + (pt1 - pt0) * t
                    };
                    (at(self.p(s.v[0])), at(self.p(s.v[1])))
                }
            };
            return Ok(C2::Nurbs { c, t0: a, t1: b });
        }
        if let Geo::Nurbs { id, t0, t1 } = s.g {
            return free::mapped(g, &self.ncurves[id as usize].c, t0, t1);
        }
        g.curve2(self.p(s.v[0]), self.p(s.v[1]), &s.g, copy)
    }

    /// Face `f`'s boundary in its parameter plane.
    fn bound(&self, f: usize) -> R<&[C2]> {
        let make = || -> R<Vec<C2>> {
            let mut bound = Vec::with_capacity(self.faces[f].edges.len());
            let mut seen: HashMap<usize, usize> = HashMap::default();
            for &(e, _) in &self.faces[f].edges {
                let s = self.edge_seg(e);
                let copy = seen.entry(e).or_insert(0);
                bound.push(self.c2_of(f, &s, *copy, Some(e))?);
                *copy += 1;
            }
            Ok(bound)
        };
        self.faces[f].bound.get_or_init(make).as_deref().map_err(|e| *e)
    }

    fn seg_at(&self, s: &Seg, t: f64) -> DVec3 {
        match s.g {
            Geo::Line => self.p(s.v[0]).lerp(self.p(s.v[1]), t),
            Geo::Arc { k, a0, sweep } => k.at(a0 + sweep * t),
            Geo::Ell { e, t0, sweep, .. } => e.at(t0 + sweep * t),
            Geo::Apex => self.p(s.v[0]),
            Geo::Nurbs { id, t0, t1 } => self.ncurves[id as usize].c.at(t0 + (t1 - t0) * t),
        }
    }

    /// The parameter (0..1) of `p` on `s`, and its distance from the curve.
    fn seg_param(&self, s: &Seg, p: DVec3) -> (f64, f64) {
        match s.g {
            Geo::Line => {
                let (a, b) = (self.p(s.v[0]), self.p(s.v[1]));
                let d = b - a;
                let t = (p - a).dot(d) / d.length_squared();
                (t, (a + d * t - p).length())
            }
            Geo::Arc { k, a0, sweep } => {
                let w = p - k.c;
                let off = w.dot(k.n);
                let rad = (w - k.n * off).length() - k.r;
                let t = (k.angle(p) - a0).rem_euclid(TAU) / sweep;
                (t, (off * off + rad * rad).sqrt())
            }
            Geo::Ell { e, t0, sweep, .. } => {
                let tp = e.param(p);
                let d = if e.hyp { tp - t0 } else { (tp - t0).rem_euclid(TAU) };
                (d / sweep, (e.at(tp) - p).length())
            }
            Geo::Apex => (0.0, (p - self.p(s.v[0])).length()),
            Geo::Nurbs { id, t0, t1 } => {
                let (t, d) = self.ncurves[id as usize].c.param_of(p);
                ((t - t0) / (t1 - t0), d)
            }
        }
    }

    fn seg_length(&self, s: &Seg) -> f64 {
        match s.g {
            Geo::Line => (self.p(s.v[1]) - self.p(s.v[0])).length(),
            Geo::Arc { k, sweep, .. } => k.r * sweep,
            Geo::Ell { e, t0, sweep, .. } => e.length(t0, t0 + sweep),
            Geo::Apex => 0.0,
            Geo::Nurbs { id, t0, t1 } => free::length(&self.ncurves[id as usize].c, t0, t1),
        }
    }

    fn edge_seg(&self, e: usize) -> Seg {
        Seg { v: self.edges[e].v, g: self.edges[e].g }
    }

    /// The intervals of `path` (parameters `t0..t1`) inside face `f` (with its boundary when `closed`).
    /// `ends`: the path's own end vertices, if it has them.
    fn intervals(&mut self, f: usize, path: &Path, t0: f64, t1: f64, ends: [End; 2], closed: bool) -> R<Vec<Ival>> {
        // (OCCT's section of a free-form face: within its own tolerance of where the faces meet.)
        let tol = match path {
            Path::Nurbs { id, .. } => self.tol.max(self.ncurves[*id as usize].tol),
            _ => self.tol,
        };
        let len = path.length(t0, t1);
        if len <= tol {
            return Ok(Vec::new());
        }
        // A circle or ellipse about a face that doesn't go round: in parts either side of the face's gap.
        let gap = match (path, &self.faces[f].s) {
            (&Path::Circle(k), &Surf::Cyl { o, a, x, y, r0, k: slope, th0, full: false, .. }) => {
                let h = (k.c - o).dot(a);
                let t = t0 + (k.angle(o + a * h + (x * th0.cos() + y * th0.sin()) * (r0 + slope * h)) - t0).rem_euclid(TAU);
                Some((t, (t - t0) * k.r > tol && (t1 - t) * k.r > tol))
            }
            (&Path::Ellipse(e), &Surf::Cyl { th0, full: false, .. }) if e.hyp => {
                self.hyp_at_angle(f, &e, th0).filter(|&t| t > t0 && t < t1).map(|t| (t, e.length(t0, t) > tol && e.length(t, t1) > tol))
            }
            (&Path::Ellipse(e), &Surf::Cyl { th0, full: false, .. }) => {
                let t = t0 + (self.ell_at_angle(f, &e, th0) - t0).rem_euclid(TAU);
                Some((t, e.length(t0, t) > tol && e.length(t, t1) > tol))
            }
            _ => None,
        };
        if let Some((t, true)) = gap {
            let mut out = self.intervals(f, path, t0, t, [ends[0], End::Open], closed)?;
            for iv in self.intervals(f, path, t, t1, [End::Open, ends[1]], closed)? {
                match out.last_mut() {
                    Some(last) if last[1].0 == iv[0].0 && last[1].1 == End::Open => last[1] = iv[1],
                    _ => out.push(iv),
                }
            }
            return Ok(out);
        }
        let geo = path.geo(t0, t1);
        let c2 = if let Path::Nurbs { .. } = path {
            // (A NURBS curve's part needs no vertices.)
            self.c2_of(f, &Seg { v: [u32::MAX; 2], g: geo }, 0, None)?
        } else {
            self.faces[f].curve2(path.at(t0), path.at(t1), &geo, 0)?
        };
        // Where it meets the face's boundary.
        let mut cuts: Vec<(f64, End)> = vec![(t0, ends[0]), (t1, ends[1])];
        let bound = self.bound(f)?.to_vec();
        for b in &bound {
            for (u, _) in c2.intersections(b, tol) {
                let t = t0 + (t1 - t0) * u;
                cuts.push((t, End::Point(path.at(t))));
            }
        }
        // The face's vertices on it (where it runs along the boundary; a NURBS curve's: those in its box).
        let near = match path {
            Path::Nurbs { c, .. } => Some(free::piece_box(c, t0, t1)),
            _ => None,
        };
        for k in 0..self.faces[f].edges.len() {
            let (e, _) = self.faces[f].edges[k];
            for w in self.edges[e].v {
                if let Some((lo, hi)) = near
                    && !boxes_meet(self.p(w), self.p(w), lo, hi, GREY * tol)
                {
                    continue;
                }
                let q = self.faces[f].to2(self.p(w));
                let (u, dist) = c2.param_of(q);
                if dist <= tol && u > 0.0 && u < 1.0 {
                    cuts.push((t0 + (t1 - t0) * u, End::Vertex(w)));
                }
            }
        }
        cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let scale = len / (t1 - t0);
        // The length along it between two parameters.
        let apart = |x: f64, y: f64| match path {
            Path::Ellipse(e) => e.length(x, y),
            _ => (y - x) * scale,
        };
        // Cuts at one place are one, a vertex if any of them is.
        let mut merged: Vec<(f64, End)> = Vec::with_capacity(cuts.len());
        for c in cuts {
            match merged.last_mut() {
                Some(last) if apart(last.0, c.0) <= tol => {
                    if c.1.rank() > last.1.rank() {
                        *last = c;
                    }
                }
                _ => merged.push(c),
            }
        }
        let cuts = merged;
        let mut out: Vec<Ival> = Vec::new();
        for k in 0..cuts.len() - 1 {
            let (a, b) = (cuts[k], cuts[k + 1]);
            if apart(a.0, b.0) <= tol {
                continue;
            }
            let mid = c2.at(((a.0 + b.0) * 0.5 - t0) / (t1 - t0));
            let dist = distance2(mid, &bound);
            let inside = if dist <= tol {
                closed
            } else if dist <= GREY * tol {
                // A B-spline (a shallow helix rising from a rod's end): judged where it is farthest from the
                // boundary, if that is clearly off it; else ambiguous.
                let far = if let Path::Nurbs { .. } = path {
                    (1..8).map(|k| c2.at(((a.0 + (b.0 - a.0) * k as f64 / 8.0) - t0) / (t1 - t0))).map(|q| (distance2(q, &bound), q)).max_by(|x, y| x.0.total_cmp(&y.0))
                } else {
                    None
                };
                match far {
                    Some((d, q)) if d > 3.0 * tol => inside2(q, &bound),
                    _ => return Err(Decline("a curve nearly along a face's boundary")),
                }
            } else {
                inside2(mid, &bound)
            };
            if !inside {
                continue;
            }
            // Joined to the interval before it when they meet at a range's end.
            if let Some(last) = out.last_mut()
                && last[1].0 == a.0
                && last[1].1 == End::Open
            {
                last[1] = b;
                continue;
            }
            out.push([a, b]);
        }
        Ok(out)
    }

    /// Adds where `path` lies inside both faces to both, as sections.
    fn section(&mut self, fs: usize, ft: usize, path: Path, t0: f64, t1: f64) -> R<()> {
        self.section_cut(fs, ft, path, t0, t1, None)
    }

    /// The same, a closed path also cut at parameter `at` (where OCCT's curve starts).
    fn section_cut(&mut self, fs: usize, ft: usize, path: Path, t0: f64, t1: f64, at: Option<f64>) -> R<()> {
        let tol = self.tol;
        // A circle or ellipse in parts between the faces' seams (a vertex where it crosses one).
        let mut cuts: Vec<(f64, End)> = vec![(t0, End::Open)];
        if let (Some(t), Path::Ellipse(e)) = (at, &path) {
            let t = t0 + (t - t0).rem_euclid(TAU);
            if e.length(t0, t) <= tol || e.length(t, t1) <= tol {
                cuts[0].1 = End::Point(path.at(t));
            } else {
                cuts.push((t, End::Point(path.at(t))));
            }
        }
        if let Path::Circle(k) = path {
            for f in [fs, ft] {
                let Some(p) = self.seam_at(f, &path) else { continue };
                let t = t0 + (k.angle(p) - t0).rem_euclid(TAU);
                if (t - t0) * k.r <= tol || (t1 - t) * k.r <= tol {
                    cuts[0].1 = End::Point(p);
                } else {
                    cuts.push((t, End::Point(p)));
                }
            }
            cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        if let Path::Ellipse(e) = path
            && e.hyp
        {
            // (Not closed: its ends are where its range ends.)
            for f in [fs, ft] {
                let Some(p) = self.seam_at(f, &path) else { continue };
                let t = e.param(p);
                if e.length(t0, t.max(t0)) > tol && e.length(t.min(t1), t1) > tol {
                    cuts.push((t, End::Point(p)));
                }
            }
            cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
        } else if let Path::Ellipse(e) = path {
            for f in [fs, ft] {
                let Some(p) = self.seam_at(f, &path) else { continue };
                let t = t0 + (e.param(p) - t0).rem_euclid(TAU);
                if e.length(t0, t) <= tol || e.length(t, t1) <= tol {
                    cuts[0].1 = End::Point(p);
                } else {
                    cuts.push((t, End::Point(p)));
                }
            }
            cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        let open = matches!(path, Path::Ellipse(e) if e.hyp);
        for i in 0..cuts.len() {
            let (a, ea) = cuts[i];
            let (b, eb) = if i + 1 < cuts.len() { cuts[i + 1] } else { (t1, if open { End::Open } else { cuts[0].1 }) };
            self.section_part(fs, ft, path.clone(), a, b, [ea, eb])?;
        }
        Ok(())
    }

    fn section_part(&mut self, fs: usize, ft: usize, path: Path, t0: f64, t1: f64, ends: [End; 2]) -> R<()> {
        let ia = self.intervals(fs, &path, t0, t1, ends, false)?;
        if ia.is_empty() {
            return Ok(());
        }
        let ib = self.intervals(ft, &path, t0, t1, ends, false)?;
        let tol = self.tol;
        // The length along it between two parameters.
        let apart = |x: f64, y: f64| path.length(x, y);
        // The end of a section: the tighter of the two faces' ends, made a vertex.
        let pick = |cx: &mut Ctx, x: (f64, End), y: (f64, End), lo: bool| -> R<Option<u32>> {
            let (later, earlier) = if x.0 > y.0 { (x, y) } else { (y, x) };
            let end = if apart(earlier.0, later.0) > tol {
                if lo { later.1 } else { earlier.1 }
            } else if x.1.rank() >= y.1.rank() {
                x.1
            } else {
                y.1
            };
            let v = match end {
                End::Open => return Ok(None),
                End::Vertex(v) => v,
                End::Point(p) => cx.pool.add(p)?,
            };
            // Both faces end there: the same vertex.
            if apart(earlier.0, later.0) <= tol {
                for o in [x.1, y.1] {
                    let w = match o {
                        End::Vertex(w) => w,
                        End::Point(p) => cx.pool.add(p)?,
                        End::Open => continue,
                    };
                    if w != v {
                        return Err(Decline("a section's end is unclear"));
                    }
                }
            }
            Ok(Some(v))
        };
        for x in &ia {
            for y in &ib {
                let lo = x[0].0.max(y[0].0);
                let hi = x[1].0.min(y[1].0);
                if apart(lo, hi) <= tol {
                    continue;
                }
                let vlo = pick(self, x[0], y[0], true)?;
                let vhi = pick(self, x[1], y[1], false)?;
                let (Some(vlo), Some(vhi)) = (vlo, vhi) else { return Err(Decline("a section without ends")) };
                let mut seg = Seg { v: [vlo, vhi], g: path.geo(lo, hi) };
                if let Geo::Ell { faces, .. } = &mut seg.g {
                    *faces = [fs, ft];
                }
                if vlo == vhi && seg.g == Geo::Line {
                    continue;
                }
                self.face_segs[fs].push(seg);
                self.face_segs[ft].push(seg);
                self.sections.push((seg, fs, ft));
            }
        }
        Ok(())
    }

    /// Edge `e` lies on face `f`'s surface: its pieces inside `f` go into `f`.
    fn edge_in_face(&mut self, e: usize, f: usize) -> R<()> {
        if !self.in_face_done.insert((e, f)) {
            return Ok(());
        }
        let s = self.edge_seg(e);
        let (path, t0, t1) = match s.g {
            Geo::Line => {
                let (a, b) = (self.p(s.v[0]), self.p(s.v[1]));
                let l = (b - a).length();
                (Path::Line { p: a, d: (b - a) / l }, 0.0, l)
            }
            Geo::Arc { k, a0, sweep } => (Path::Circle(k), a0, a0 + sweep),
            Geo::Ell { e, t0, sweep, .. } if !e.hyp && matches!(self.faces[f].s, Surf::Plane { .. }) => (Path::Ellipse(e), t0, t0 + sweep),
            Geo::Ell { .. } => return Err(Decline("an argument's ellipse")),
            Geo::Nurbs { .. } if matches!(self.faces[f].s, Surf::Nurbs { .. }) => return Err(Decline("a curved edge on another argument's free-form face")),
            Geo::Nurbs { id, t0, t1 } => (Path::Nurbs { id, c: self.ncurves[id as usize].c.clone() }, t0, t1),
            Geo::Apex => return Ok(()),
        };
        // An arc across a round face's seam: in two parts, a vertex where it crosses.
        let mut parts = vec![(t0, End::Vertex(s.v[0]), t1, End::Vertex(s.v[1]))];
        if let Path::Nurbs { c, .. } = &path {
            // A B-spline across a round face's seam (a helix round a rod): in parts, a vertex at each crossing.
            let mut at = vec![(t0, End::Vertex(s.v[0]))];
            for t in free::seam_cuts(self, [f, usize::MAX], c, t0.min(t1), t0.max(t1)) {
                if path.length(t0.min(t), t0.max(t)) > self.tol && path.length(t1.min(t), t1.max(t)) > self.tol {
                    at.push((t, End::Point(c.at(t))));
                }
            }
            at.sort_by(|a, b| if t0 <= t1 { a.0.total_cmp(&b.0) } else { b.0.total_cmp(&a.0) });
            at.push((t1, End::Vertex(s.v[1])));
            parts = at.windows(2).map(|w| (w[0].0, w[0].1, w[1].0, w[1].1)).collect();
        }
        if let (Path::Circle(k), Some(seam)) = (path.clone(), self.seam_at(f, &path)) {
            let ts = t0 + (k.angle(seam) - t0).rem_euclid(TAU);
            if (ts - t0) * k.r > self.tol && (t1 - ts) * k.r > self.tol {
                parts = vec![(t0, End::Vertex(s.v[0]), ts, End::Point(seam)), (ts, End::Point(seam), t1, End::Vertex(s.v[1]))];
            }
        }
        let mut ivs = Vec::new();
        for (a0, e0, a1, e1) in parts {
            ivs.extend(self.intervals(f, &path, a0, a1, [e0, e1], true)?);
        }
        for iv in ivs {
            let mut ends = [0u32; 2];
            for k in 0..2 {
                ends[k] = match iv[k].1 {
                    End::Vertex(v) => v,
                    End::Point(p) => self.pool.add(p)?,
                    End::Open => return Err(Decline("an edge's piece without ends")),
                };
            }
            let [a, b] = ends;
            let g = path.geo(iv[0].0, iv[1].0);
            if a == b && g == Geo::Line {
                continue;
            }
            self.face_segs[f].push(Seg { v: [a, b], g });
        }
        Ok(())
    }

    /// Where `path` (a circle about face `f`'s axis) crosses `f`'s seam, if `f` goes round.
    fn seam_at(&self, f: usize, path: &Path) -> Option<DVec3> {
        match (path.clone(), &self.faces[f].s) {
            (Path::Circle(k), Surf::Cyl { o, a, x, y, r0, k: slope, th0, full: true, .. }) => {
                let h = (k.c - *o).dot(*a);
                Some(*o + *a * h + (*x * th0.cos() + *y * th0.sin()) * (r0 + slope * h))
            }
            (Path::Ellipse(e), Surf::Cyl { th0, full: true, .. }) if e.hyp => self.hyp_at_angle(f, &e, *th0).map(|t| e.at(t)),
            (Path::Ellipse(e), Surf::Cyl { th0, full: true, .. }) => Some(e.at(self.ell_at_angle(f, &e, *th0))),
            _ => None,
        }
    }

    /// The parameter of ellipse `e` (on face `f`'s cylinder) where the cylinder's angle is `th`.
    fn ell_at_angle(&self, f: usize, e: &Ell, th: f64) -> f64 {
        let Surf::Cyl { a, x, y, .. } = self.faces[f].s else { unreachable!() };
        // Seen along the axis it is the cylinder's circle: its major axis' direction at t = 0, its minor's at π/2.
        let d = x * th.cos() + y * th.sin();
        let xp = (e.x - a * e.x.dot(a)).normalize();
        d.dot(e.y).atan2(d.dot(xp))
    }

    /// The parameter of hyperbola `e` (on face `f`'s cone) where the cone's angle is `th`, if it gets there.
    fn hyp_at_angle(&self, f: usize, e: &Ell, th: f64) -> Option<f64> {
        let Surf::Cyl { o, a, x, y, .. } = self.faces[f].s else { unreachable!() };
        // Seen along the axis: from its foot on the plane (nearest the axis) along y, b sinh t.
        let foot = (e.c - o) - a * (e.c - o).dot(a);
        let d = x * th.cos() + y * th.sin();
        let den = e.y.cross(d).dot(a);
        if den.abs() < 1e-12 {
            return None;
        }
        let sv = -foot.cross(d).dot(a) / den;
        ((foot + e.y * sv).dot(d) > 0.0).then(|| (sv / e.b).asinh())
    }

    /// Whether edge `e` lies on face `f`'s surface.
    fn edge_on_surface(&mut self, e: usize, f: usize) -> R<bool> {
        let [a, b] = self.edges[e].v;
        let tol = self.tol;
        match (&self.faces[f].s, self.edges[e].g) {
            (Surf::Plane { .. }, Geo::Line) => Ok(self.side(a, f)? == 0 && self.side(b, f)? == 0),
            (Surf::Plane { n, d, .. }, Geo::Arc { k, .. }) => {
                let off = n.dot(k.c) - d;
                Ok(k.n.cross(*n).length() <= 1e-9 && off.abs() <= tol)
            }
            (Surf::Cyl { o, a: ax, r0, k, .. }, Geo::Line) => {
                // Both ends and the middle on it: a line meets a cylinder or cone in two points at most.
                let off = |p: DVec3| {
                    let w = p - *o;
                    let h = w.dot(*ax);
                    ((w - *ax * h).length() - (r0 + k * h)).abs()
                };
                let (pa, pb) = (self.p(a), self.p(b));
                Ok(off(pa) <= tol && off(pb) <= tol && off((pa + pb) * 0.5) <= tol)
            }
            (Surf::Cyl { o, a: ax, r0, k: slope, .. }, Geo::Arc { k, .. }) => {
                let w = k.c - *o;
                let h = w.dot(*ax);
                Ok(k.n.cross(*ax).length() <= 1e-9 && (w - *ax * h).length() <= tol && (k.r - (r0 + slope * h)).abs() <= tol)
            }
            (_, Geo::Apex) => Ok(false),
            (Surf::Nurbs { .. }, _) | (_, Geo::Nurbs { .. } | Geo::Ell { .. }) => {
                // Points along it: all on the surface, it lies on it (not yet taken); within the grey band
                // without being clearly off, ambiguous.
                let s = self.edge_seg(e);
                let (mut on, mut near) = (0, 0);
                for k in 0..=6 {
                    let d = free::surface_distance(&self.faces[f], self.seg_at(&s, k as f64 / 6.0));
                    if d <= tol {
                        on += 1;
                    } else if d <= GREY * tol {
                        near += 1;
                    }
                }
                if on == 7 {
                    // (On a plane, cylinder or cone: its pieces there go into the face, as a straight or round
                    // edge's.)
                    if let (Geo::Nurbs { .. }, Surf::Plane { .. } | Surf::Cyl { .. }) = (self.edges[e].g, &self.faces[f].s) {
                        return Ok(true);
                    }
                    if let (Geo::Ell { e: el, .. }, Surf::Plane { .. }) = (self.edges[e].g, &self.faces[f].s)
                        && !el.hyp
                    {
                        return Ok(true);
                    }
                    if let Geo::Ell { .. } = self.edges[e].g {
                        return Err(Decline("an argument's ellipse on another's face"));
                    }
                    return Err(Decline("an edge on another argument's free-form face"));
                }
                if near > 0 && on + near == 7 {
                    return Err(Decline("an edge nearly on another argument's face"));
                }
                Ok(false)
            }
        }
    }

    /// Faces `fs` and `ft` (one free-form) meet where OCCT's section says (its curves, its curves on them).
    fn nurbs_pair(&mut self, fs: usize, ft: usize) -> R<()> {
        let Some(src) = self.src else { return Err(Decline("a curved face")) };
        let (a, b) = ((self.faces[fs].arg, self.faces[fs].index), (self.faces[ft].arg, self.faces[ft].index));
        for c in src.sections(a, b)? {
            if c.t1 <= c.t0 || c.t1.is_nan() {
                continue;
            }
            let id = self.ncurves.len() as u32;
            let cc = Arc::new(c.curve);
            let [on0, on1] = c.on;
            self.ncurves.push(NCurve { c: cc.clone(), sec: Some(([fs, ft], [Arc::new(on0), Arc::new(on1)], c.id)), tol: c.tol });
            let cuts = free::seam_cuts(self, [fs, ft], &cc, c.t0, c.t1);
            self.section_nurbs(fs, ft, Path::Nurbs { id, c: cc }, c.t0, c.t1, &cuts)?;
        }
        Ok(())
    }

    /// A section along OCCT's curve from `t0` to `t1`, cut at `cuts` (seams it crosses): a closed one has a
    /// vertex where it starts (as OCCT's), an open one ends where it does.
    fn section_nurbs(&mut self, fs: usize, ft: usize, path: Path, t0: f64, t1: f64, cuts: &[f64]) -> R<()> {
        let closed = (path.at(t0) - path.at(t1)).length() <= self.tol;
        let first = if closed { End::Point(path.at(t0)) } else { End::Open };
        let mut at: Vec<(f64, End)> = vec![(t0, first)];
        for &t in cuts {
            if path.length(t0, t) > self.tol && path.length(t, t1) > self.tol {
                at.push((t, End::Point(path.at(t))));
            }
        }
        for i in 0..at.len() {
            let (a, ea) = at[i];
            let (b, eb) = if i + 1 < at.len() { at[i + 1] } else { (t1, if closed { first } else { End::Open }) };
            self.section_part(fs, ft, path.clone(), a, b, [ea, eb])?;
        }
        Ok(())
    }

    fn face_pair(&mut self, fs: usize, ft: usize) -> R<()> {
        for (f, g) in [(fs, ft), (ft, fs)] {
            let (lo, hi) = (self.faces[g].lo, self.faces[g].hi);
            for k in 0..self.faces[f].edges.len() {
                let e = self.faces[f].edges[k].0;
                // (Only edges that reach the other face's box can lie on it.)
                let [a, b] = self.edges[e].v;
                if let Geo::Line = self.edges[e].g
                    && !boxes_meet(self.p(a).min(self.p(b)), self.p(a).max(self.p(b)), lo, hi, GREY * self.tol)
                {
                    continue;
                }
                if self.edge_on_surface(e, g)? {
                    self.edge_in_face(e, g)?;
                }
            }
        }
        match (&self.faces[fs].s, &self.faces[ft].s) {
            (Surf::Nurbs { .. }, _) | (_, Surf::Nurbs { .. }) => self.nurbs_pair(fs, ft),
            (Surf::Plane { .. }, Surf::Plane { .. }) => self.plane_plane(fs, ft),
            (Surf::Plane { .. }, Surf::Cyl { .. }) => self.plane_cyl(fs, ft),
            (Surf::Cyl { .. }, Surf::Plane { .. }) => self.plane_cyl(ft, fs),
            (Surf::Cyl { .. }, Surf::Cyl { .. }) => self.cyl_cyl(fs, ft),
        }
    }

    fn plane_plane(&mut self, fs: usize, ft: usize) -> R<()> {
        let (Surf::Plane { n: n1, d: d1, .. }, Surf::Plane { n: n2, d: d2, origin: o2, .. }) = (&self.faces[fs].s, &self.faces[ft].s) else { unreachable!() };
        let (n1, d1, n2, d2, o2) = (*n1, *d1, *n2, *d2, *o2);
        let dir = n1.cross(n2);
        if dir.length() <= 1e-9 {
            let off = (n1.dot(o2) - d1).abs();
            if off > GREY * self.tol {
                return Ok(());
            }
            if off > self.tol {
                return Err(Decline("faces nearly coplanar"));
            }
            self.coplanar.push((fs, ft));
            return Ok(());
        }
        let dir = dir.normalize();
        // A point on both planes, then the parameter range over both faces' boxes.
        let a = n1.cross(dir);
        let b = n2.cross(dir);
        // p = s a + t b with n1·p = d1, n2·p = d2.
        let (a1, b1, a2, b2) = (n1.dot(a), n1.dot(b), n2.dot(a), n2.dot(b));
        let det = a1 * b2 - a2 * b1;
        let p = a * ((d1 * b2 - d2 * b1) / det) + b * ((a1 * d2 - a2 * d1) / det);
        let (lo, hi) = (self.faces[fs].lo.max(self.faces[ft].lo), self.faces[fs].hi.min(self.faces[ft].hi));
        let (t0, t1) = box_range(p, dir, lo, hi, self.tol);
        if t0 >= t1 {
            return Ok(());
        }
        self.section(fs, ft, Path::Line { p, d: dir }, t0, t1)
    }

    fn plane_cyl(&mut self, fp: usize, fc: usize) -> R<()> {
        let Surf::Plane { n, d, .. } = self.faces[fp].s else { unreachable!() };
        let Surf::Cyl { o, a, x, y, r, r0, k: slope, th0, .. } = self.faces[fc].s else { unreachable!() };
        let tol = self.tol;
        let na = n.dot(a);
        if na.abs() >= 1.0 - 1e-12 {
            // A plane across the axis: a circle.
            let h = (d - n.dot(o)) / na;
            let c = o + a * h;
            let rc = r0 + slope * h;
            let (lo, hi) = (self.faces[fc].lo, self.faces[fc].hi);
            if !boxes_meet(c, c, lo, hi, GREY * tol + rc.abs()) {
                return Ok(());
            }
            if rc <= GREY * tol {
                return Err(Decline("a plane through a cone's apex"));
            }
            let k = Circ { c, n: a, x, y, r: rc };
            return self.section(fp, fc, Path::Circle(k), th0, th0 + TAU);
        }
        if na.abs() > 1e-12 {
            if slope != 0.0 {
                return Err(Decline("a plane at an angle to a cone"));
            }
            if na.abs() < 0.25 {
                return Err(Decline("a plane nearly along a cylinder"));
            }
            // OCCT's ellipse (IntAna_QuadQuadGeo): its centre where the plane meets the axis, its normal the
            // plane surface's, its major axis (n × a) × n, its radii r / |n · a| and r.
            let Surf::Plane { flip, .. } = self.faces[fp].s else { unreachable!() };
            let sn = if flip { -n } else { n };
            let c = o + a * ((d - n.dot(o)) / na);
            let xe = sn.cross(a).cross(sn).normalize();
            let e = Ell { c, n: sn, x: xe, y: sn.cross(xe), a: r / na.abs(), b: r, hyp: false };
            let (lo, hi) = (self.faces[fc].lo, self.faces[fc].hi);
            if !boxes_meet(c - DVec3::splat(e.a), c + DVec3::splat(e.a), lo, hi, GREY * tol) {
                return Ok(());
            }
            let ts = self.ell_at_angle(fc, &e, th0);
            return self.section(fp, fc, Path::Ellipse(e), ts, ts + TAU);
        }
        if slope != 0.0 {
            // A plane along a cone's axis: through it, the two rulings in the plane (else a hyperbola).
            let (lo, hi) = (self.faces[fc].lo, self.faces[fc].hi);
            let corners = (0..8).map(|i| DVec3::new([lo.x, hi.x][i & 1], [lo.y, hi.y][(i >> 1) & 1], [lo.z, hi.z][i >> 2]));
            let (smin, smax) = corners.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), c| (a.min(n.dot(c) - d), b.max(n.dot(c) - d)));
            if smin > GREY * tol || smax < -GREY * tol {
                return Ok(());
            }
            let dd = n.dot(o) - d;
            // Clear of the face: farther from the axis than its largest radius (its heights its vertices').
            let hs = self.faces[fc].edges.iter().flat_map(|&(e, _)| self.edges[e].v).map(|w| (self.p(w) - o).dot(a));
            let (h0, h1) = hs.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), h| (lo.min(h), hi.max(h)));
            let rmax = (r0 + slope * h0).abs().max((r0 + slope * h1).abs());
            if dd.abs() > rmax + GREY * tol {
                return Ok(());
            }
            let apex = o - a * (r0 / slope);
            if dd.abs() > GREY * tol {
                // OCCT's hyperbola (IntAna_QuadQuadGeo): centred where the plane is nearest the apex, its normal
                // the plane surface's, its major axis along the cone's axis into the face's nappe, its radii
                // |dd| / |slope| and |dd|.
                let Surf::Plane { flip, .. } = self.faces[fp].s else { unreachable!() };
                let sn = if flip { -n } else { n };
                let xe = if slope > 0.0 { a } else { -a };
                let e = Ell { c: apex - n * dd, n: sn, x: xe, y: sn.cross(xe), a: dd.abs() / slope.abs(), b: dd.abs(), hyp: true };
                // Its range: where it is no farther from the apex along the axis than the face.
                let far = (h0 - (-r0 / slope)).abs().max((h1 - (-r0 / slope)).abs());
                if far <= e.a {
                    return Ok(());
                }
                let t = (far / e.a).acosh() * 1.01 + 1e-3;
                return self.section(fp, fc, Path::Ellipse(e), -t, t);
            }
            if dd.abs() > tol {
                return Err(Decline("a plane nearly through a cone's axis"));
            }
            if boxes_meet(apex, apex, lo, hi, GREY * tol) {
                return Err(Decline("a plane through a cone's apex"));
            }
            for th in circle_line(1.0, x, y, n, 0.0) {
                self.generator(fp, fc, th)?;
            }
            return Ok(());
        }
        // A plane along the axis: lines along it where the plane meets the circle.
        let dd = n.dot(o) - d;
        if dd.abs() > r + GREY * tol {
            return Ok(());
        }
        if (dd.abs() - r).abs() <= GREY * tol {
            // Touching along a line: fine where the faces don't both reach it.
            let th = circle_line(r, x, y, n, -dd.signum() * r).first().copied().unwrap_or(0.0);
            return self.touch(fp, fc, th);
        }
        let ths = circle_line(r, x, y, n, -dd);
        for th in ths {
            self.generator(fp, fc, th)?;
        }
        Ok(())
    }

    /// The section along cylinder or cone face `fc`'s generator at angle `th` (in its frame), with face `f`.
    fn generator(&mut self, f: usize, fc: usize, th: f64) -> R<()> {
        let Surf::Cyl { o, a, x, y, r, r0, k, th0, full, .. } = self.faces[fc].s else { unreachable!() };
        let off = (th - th0).rem_euclid(TAU).min((th0 - th).rem_euclid(TAU)) * r;
        if full && off <= self.tol {
            // Its seam: an edge already, which lies on `f` (its edges on the other's faces add it there).
            return Ok(());
        }
        if full && off <= GREY * self.tol {
            return Err(Decline("a section along a cylinder's seam"));
        }
        let rad = x * th.cos() + y * th.sin();
        let (p, d) = if k == 0.0 { (o + rad * r, a) } else { (o + rad * r0, (a + rad * k).normalize()) };
        let (lo, hi) = (self.faces[f].lo.max(self.faces[fc].lo), self.faces[f].hi.min(self.faces[fc].hi));
        let (t0, t1) = box_range(p, d, lo, hi, self.tol);
        if t0 >= t1 {
            return Ok(());
        }
        self.section(f, fc, Path::Line { p, d }, t0, t1)
    }

    /// Face `f` touches cylinder face `fc`'s surface along its generator at `th`: declines where both faces
    /// reach the line.
    fn touch(&mut self, f: usize, fc: usize, th: f64) -> R<()> {
        let Surf::Cyl { o, a, x, y, r, .. } = self.faces[fc].s else { unreachable!() };
        let p = o + (x * th.cos() + y * th.sin()) * r;
        let (lo, hi) = (self.faces[f].lo.max(self.faces[fc].lo), self.faces[f].hi.min(self.faces[fc].hi));
        let (t0, t1) = box_range(p, a, lo, hi, self.tol);
        if t0 >= t1 {
            return Ok(());
        }
        let path = Path::Line { p, d: a };
        let ia = self.intervals(f, &path, t0, t1, [End::Open; 2], true)?;
        if ia.is_empty() {
            return Ok(());
        }
        let ib = self.intervals(fc, &path, t0, t1, [End::Open; 2], true)?;
        let meet = ia.iter().any(|x| ib.iter().any(|y| x[1].0.min(y[1].0) - x[0].0.max(y[0].0) > self.tol));
        // Where both reach it: a section along it, as where they cross.
        if meet { self.generator(f, fc, th) } else { Ok(()) }
    }

    fn cyl_cyl(&mut self, f1: usize, f2: usize) -> R<()> {
        let Surf::Cyl { o: o1, a: a1, x, y, r: r1, r0: r01, k: k1, th0, .. } = self.faces[f1].s else { unreachable!() };
        let Surf::Cyl { o: o2, a: a2, r: r2, r0: r02, k: k2, .. } = self.faces[f2].s else { unreachable!() };
        let tol = self.tol;
        if a1.cross(a2).length() > 1e-12 {
            if k1 != 0.0 || k2 != 0.0 {
                return Err(Decline("a cone at an angle"));
            }
            return self.cyl_cyl_angled(f1, f2);
        }
        let w = o2 - o1;
        let w = w - a1 * w.dot(a1);
        let dl = w.length();
        if (k1 != 0.0 || k2 != 0.0) && dl > GREY * tol {
            return Err(Decline("a cone off another's axis"));
        }
        if k1 != 0.0 || k2 != 0.0 {
            // On one axis: where the radii agree (along the first's axis from its origin, s).
            let sg = a1.dot(a2);
            let c2 = (o1 - o2).dot(a2);
            let (c0, c1) = (r01 - r02 - k2 * c2, k1 - k2 * sg);
            if c1.abs() < 1e-12 {
                if c0.abs() <= tol {
                    self.coplanar.push((f1, f2));
                    return Ok(());
                }
                if c0.abs() <= GREY * tol {
                    return Err(Decline("cones nearly coincident"));
                }
                return Ok(());
            }
            let s = -c0 / c1;
            let rc = r01 + k1 * s;
            if rc <= GREY * tol {
                return Err(Decline("cones meeting at an apex"));
            }
            let k = Circ { c: o1 + a1 * s, n: a1, x, y, r: rc };
            return self.section(f1, f2, Path::Circle(k), th0, th0 + TAU);
        }
        if dl <= GREY * tol {
            if (r1 - r2).abs() <= tol {
                // One surface: their pieces on each other are coincident, like coplanar faces'.
                self.coplanar.push((f1, f2));
                return Ok(());
            }
            if (r1 - r2).abs() <= GREY * tol {
                return Err(Decline("cylinders nearly coincident"));
            }
            return Ok(());
        }
        if dl > r1 + r2 + GREY * tol || dl < (r1 - r2).abs() - GREY * tol {
            return Ok(());
        }
        if (dl - r1 - r2).abs() <= GREY * tol || (dl - (r1 - r2).abs()).abs() <= GREY * tol {
            let ex = w / dl;
            let base = ex.dot(y).atan2(ex.dot(x));
            let th = if (dl - r1 - r2).abs() <= GREY * tol || r1 > r2 { base } else { base + PI };
            return self.touch(f2, f1, th);
        }
        // Where the two circles meet, as angles about the first axis.
        let ex = w / dl;
        let ca = ((dl * dl + r1 * r1 - r2 * r2) / (2.0 * dl * r1)).clamp(-1.0, 1.0);
        let base = ex.dot(y).atan2(ex.dot(x));
        for th in [base + ca.acos(), base - ca.acos()] {
            self.generator(f2, f1, th)?;
        }
        Ok(())
    }

    /// Cylinders at an angle. Apart, nothing; of one radius with their axes meeting, OCCT's two ellipses
    /// (IntAna_QuadQuadGeo: centred where the axes meet, normals a1 + a2 and a1 - a2 and major axes the other,
    /// radii r / cos(A / 2), r / sin(A / 2) and r, A the axes' angle; a1 the earlier argument's face's axis); else
    /// (a curve of the fourth order) declined.
    fn cyl_cyl_angled(&mut self, f1: usize, f2: usize) -> R<()> {
        let (f1, f2) = if (self.faces[f2].arg, self.faces[f2].index) < (self.faces[f1].arg, self.faces[f1].index) { (f2, f1) } else { (f1, f2) };
        let Surf::Cyl { o: o1, a: a1, r: r1, th0, .. } = self.faces[f1].s else { unreachable!() };
        let Surf::Cyl { o: o2, a: a2, r: r2, .. } = self.faces[f2].s else { unreachable!() };
        let tol = self.tol;
        let w = o2 - o1;
        let dist = a1.cross(a2).normalize().dot(w).abs();
        if dist > r1 + r2 + GREY * tol {
            return Ok(());
        }
        let rel = (r1 - r2).abs() / r1.max(r2);
        if dist > 1e-9 || rel > 1e-14 {
            return Err(Decline("cylinders at an angle"));
        }
        let (b, d, e) = (a1.dot(a2), a1.dot(w), a2.dot(w));
        let c = o1 + a1 * ((d - b * e) / (1.0 - b * b));
        let (ch, sh) = (((1.0 + b) * 0.5).sqrt(), ((1.0 - b) * 0.5).sqrt());
        let (sum, diff) = ((a1 + a2).normalize(), (a1 - a2).normalize());
        for (n, x, major) in [(sum, diff, r1 / ch), (diff, sum, r1 / sh)] {
            let el = Ell { c, n, x, y: n.cross(x), a: major, b: r1, hyp: false };
            let (lo, hi) = (self.faces[f1].lo.max(self.faces[f2].lo), self.faces[f1].hi.min(self.faces[f2].hi));
            if !boxes_meet(c - DVec3::splat(major), c + DVec3::splat(major), lo, hi, GREY * tol) {
                continue;
            }
            let ts = self.ell_at_angle(f1, &el, th0);
            self.section_cut(f1, f2, Path::Ellipse(el), ts, ts + TAU, Some(0.0))?;
        }
        Ok(())
    }

    /// The pool vertices strictly inside `s`, in order from its start.
    /// The pool vertices strictly inside `s`, in order from its start: of `lists` (each sorted by x), the
    /// ones not `own`'s (an argument's vertices never lie inside its own edges).
    fn split_among(&self, s: &Seg, lists: &ByOwner, own: Option<u32>) -> R<Vec<u32>> {
        if s.g == Geo::Apex {
            return Ok(vec![s.v[0], s.v[1]]);
        }
        let mut chain: Vec<(f64, u32)> = Vec::new();
        for (who, list) in lists {
            if own.is_some() && *who == own {
                continue;
            }
            let c = self.split(s, list)?;
            for &w in &c[1..c.len() - 1] {
                chain.push((self.seg_param(s, self.p(w)).0, w));
            }
        }
        chain.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut out = vec![s.v[0]];
        out.extend(chain.into_iter().map(|x| x.1));
        out.push(s.v[1]);
        Ok(out)
    }

    fn split(&self, s: &Seg, sorted_x: &[(f64, u32)]) -> R<Vec<u32>> {
        let tol = self.tol;
        let (lo, hi) = match s.g {
            Geo::Line => {
                let (a, b) = (self.p(s.v[0]), self.p(s.v[1]));
                (a.min(b), a.max(b))
            }
            Geo::Arc { k, .. } => (k.c - DVec3::splat(k.r), k.c + DVec3::splat(k.r)),
            Geo::Ell { e, t0, sweep, .. } => e.piece_box(t0, sweep),
            Geo::Nurbs { id, t0, t1 } => free::piece_box(&self.ncurves[id as usize].c, t0, t1),
            Geo::Apex => return Ok(vec![s.v[0], s.v[1]]),
        };
        let (lo, hi) = (lo - DVec3::splat(tol), hi + DVec3::splat(tol));
        let start = sorted_x.partition_point(|x| x.0 < lo.x);
        let len = self.seg_length(s);
        let mut on: Vec<(f64, u32)> = Vec::new();
        for &(px, w) in &sorted_x[start..] {
            if px > hi.x {
                break;
            }
            if w == s.v[0] || w == s.v[1] {
                continue;
            }
            let p = self.p(w);
            if p.y < lo.y || p.y > hi.y || p.z < lo.z || p.z > hi.z {
                continue;
            }
            let (t, off) = self.seg_param(s, p);
            if t <= 0.0 || t >= 1.0 || t * len <= tol || (1.0 - t) * len <= tol {
                continue;
            }
            if off <= tol {
                on.push((t, w));
            } else if off <= GREY * tol {
                return Err(Decline("a vertex nearly on an edge"));
            }
        }
        on.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut chain = vec![s.v[0]];
        chain.extend(on.into_iter().map(|x| x.1));
        chain.push(s.v[1]);
        Ok(chain)
    }

    /// The pieces of `s` between the vertices of `chain` (its split points in order).
    fn pieces_of(&self, s: &Seg, chain: &[u32]) -> Vec<Seg> {
        chain
            .windows(2)
            .map(|w| {
                let g = match s.g {
                    Geo::Line => Geo::Line,
                    Geo::Apex => Geo::Apex,
                    Geo::Nurbs { id, t0, t1 } => {
                        let t = |v: u32, end: bool| {
                            if v == s.v[0] && !end {
                                0.0
                            } else if v == s.v[1] && end {
                                1.0
                            } else {
                                self.seg_param(s, self.p(v)).0
                            }
                        };
                        let (ta, tb) = (t(w[0], false), t(w[1], true));
                        Geo::Nurbs { id, t0: t0 + (t1 - t0) * ta, t1: t0 + (t1 - t0) * tb }
                    }
                    Geo::Arc { k, a0, sweep } => {
                        let t = |v: u32, end: bool| {
                            if v == s.v[0] && !end {
                                0.0
                            } else if v == s.v[1] && end {
                                1.0
                            } else {
                                self.seg_param(s, self.p(v)).0
                            }
                        };
                        let (ta, tb) = (t(w[0], false), t(w[1], true));
                        Geo::Arc { k, a0: a0 + sweep * ta, sweep: sweep * (tb - ta) }
                    }
                    Geo::Ell { e, t0, sweep, faces } => {
                        let t = |v: u32, end: bool| {
                            if v == s.v[0] && !end {
                                0.0
                            } else if v == s.v[1] && end {
                                1.0
                            } else {
                                self.seg_param(s, self.p(v)).0
                            }
                        };
                        let (ta, tb) = (t(w[0], false), t(w[1], true));
                        Geo::Ell { e, t0: t0 + sweep * ta, sweep: sweep * (tb - ta), faces }
                    }
                };
                Seg { v: [w[0], w[1]], g }
            })
            .collect()
    }

    /// Where 3D point `p` (not on any of solid `s`'s faces) is against solid `s`, by a ray's crossings.
    fn ray_state(&self, p: DVec3, s: usize) -> R<State> {
        const DIRS: [DVec3; 4] = [
            DVec3::new(0.5773502691896258, 0.5773502691896258, 0.5773502691896258),
            DVec3::new(0.2672612419124244, 0.5345224838248488, 0.8017837257372732),
            DVec3::new(-0.8017837257372732, 0.2672612419124244, 0.5345224838248488),
            DVec3::new(0.5345224838248488, -0.8017837257372732, 0.2672612419124244),
        ];
        let tol = self.tol;
        let sol = &self.solids[s];
        if !boxes_meet(p, p, sol.lo, sol.hi, tol) {
            return Ok(State::Out);
        }
        'dirs: for r in DIRS {
            let mut odd = false;
            for &f in &sol.faces {
                let g = &self.faces[f];
                // (Each crossing's t, and on a free-form face where on it (its parameters found with it).)
                let mut hits: Vec<(f64, Option<DVec2>)> = Vec::new();
                match g.s {
                    Surf::Plane { n, d, .. } => {
                        let den = n.dot(r);
                        let dist = d - n.dot(p);
                        if den.abs() < 1e-6 {
                            if dist.abs() <= GREY * tol {
                                continue 'dirs;
                            }
                            continue;
                        }
                        hits.push((dist / den, None));
                    }
                    Surf::Cyl { o, a, r0, k, .. } => {
                        // |w⊥ + t r⊥| = r0 + k (h + t r·a), on the cone's own side of its apex.
                        let w = p - o;
                        let (h, dz) = (w.dot(a), r.dot(a));
                        let (wp, rp) = (w - a * h, r - a * dz);
                        let rad = r0 + k * h;
                        let (qa, qb, qc) = (rp.length_squared() - k * k * dz * dz, wp.dot(rp) - k * dz * rad, wp.length_squared() - rad * rad);
                        let mut roots = Vec::new();
                        if qa.abs() < 1e-12 {
                            if qb.abs() > 1e-300 {
                                roots.push(-qc / (2.0 * qb));
                            }
                        } else {
                            let disc = qb * qb - qa * qc;
                            if disc < 0.0 {
                                continue;
                            }
                            if disc.sqrt() / qa.abs() <= GREY * tol {
                                continue 'dirs;
                            }
                            roots.push((-qb - disc.sqrt()) / qa);
                            roots.push((-qb + disc.sqrt()) / qa);
                        }
                        hits.extend(roots.into_iter().filter(|t| r0 + k * (h + t * dz) > -tol).map(|t| (t, None)));
                    }
                    Surf::Nurbs { su, sv, .. } => {
                        let Some(ns) = &g.ns else { continue };
                        if !free::ray_meets(p, r, g.lo, g.hi) {
                            continue;
                        }
                        for (uv, t) in free::ray_hits(ns, g.patches.get_or_init(|| free::patches(ns, g.uvbox)), g.uvbox, p, r) {
                            // Grazing it: another ray.
                            let d = ns.derivs(uv.x, uv.y);
                            if d[1].cross(d[2]).normalize_or_zero().dot(r).abs() < 1e-3 {
                                continue 'dirs;
                            }
                            hits.push((t, Some(DVec2::new(uv.x * su, uv.y * sv))));
                        }
                    }
                }
                let gb = if hits.is_empty() { &[][..] } else { self.bound(f)? };
                for (t, at) in hits {
                    let q3 = p + r * t;
                    if t.abs() > GREY * tol && !boxes_meet(q3, q3, g.lo, g.hi, GREY * tol) {
                        continue;
                    }
                    if t.abs() <= GREY * tol {
                        if distance2(g.to2(p), gb) > tol && inside2(g.to2(p), gb) {
                            return Err(Decline("a piece's point on another solid's face"));
                        }
                        continue;
                    }
                    if t < 0.0 {
                        continue;
                    }
                    let q = at.unwrap_or_else(|| g.to2(p + r * t));
                    if distance2(q, gb) <= GREY * tol {
                        continue 'dirs;
                    }
                    if inside2(q, gb) {
                        odd = !odd;
                    }
                }
            }
            return Ok(if odd { State::In } else { State::Out });
        }
        Err(Decline("no clean ray"))
    }
}

/// A cylinder's or cone's face (radius `r0 + k h`): one that goes round starts at its seam (an edge it uses
/// twice); its parameter scale is its largest radius.
fn round(cx: &Ctx, edges: &[(usize, bool)], [o, a, x]: [DVec3; 3], r0: f64, k: f64, flip: bool) -> Surf {
    let mut seen = HashSet::default();
    let seam = edges.iter().find(|&&(e, _)| !seen.insert(e)).map(|&(e, _)| e);
    let y = a.cross(x);
    let th0 = seam.map_or(0.0, |e| {
        let v = cx.edges[e].v;
        let p = if (cx.pool.p[v[0] as usize] - o).cross(a).length() > (cx.pool.p[v[1] as usize] - o).cross(a).length() { cx.pool.p[v[0] as usize] } else { cx.pool.p[v[1] as usize] };
        let w = p - o;
        w.dot(y).atan2(w.dot(x))
    });
    let r = if k == 0.0 {
        r0
    } else {
        edges.iter().filter_map(|&(e, _)| if let Geo::Arc { k, .. } = cx.edges[e].g { Some(k.r) } else { None }).fold(r0.abs(), f64::max)
    };
    Surf::Cyl { o, a, x, y, r, r0, k, flip, th0, full: seam.is_some() }
}

/// The parameters of the line `p + t d` inside the box (`lo`, `hi`), grown well beyond the grey zone (the
/// line's ends outside every face).
fn box_range(p: DVec3, d: DVec3, lo: DVec3, hi: DVec3, tol: f64) -> (f64, f64) {
    let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
    let grow = 1e3 * GREY * tol + 0.1 * (hi - lo).max(DVec3::ZERO).length();
    for i in 0..3 {
        let (pi, di, l, h) = (p[i], d[i], lo[i] - grow, hi[i] + grow);
        if di.abs() < 1e-12 {
            if pi < l || pi > h {
                return (1.0, 0.0);
            }
            continue;
        }
        let (a, b) = ((l - pi) / di, (h - pi) / di);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    (t0, t1)
}

/// The angles θ where `r (cos θ x + sin θ y) · n == c`.
fn circle_line(r: f64, x: DVec3, y: DVec3, n: DVec3, c: f64) -> Vec<f64> {
    let (a, b) = (r * n.dot(x), r * n.dot(y));
    let m = (a * a + b * b).sqrt();
    if m < 1e-300 || c.abs() > m {
        return Vec::new();
    }
    let base = b.atan2(a);
    let off = (c / m).acos();
    vec![base + off, base - off]
}

/// The Boolean `op` of the arguments (`args[0]` the object, the rest tools), points within `tol` one.
pub fn boolean(op: Op, args: &[&Body], tol: f64) -> R<Output> {
    boolean_touching(op, args, tol, false)
}

/// The same; with `touching_only`, declines unless the arguments only touch (no piece of one inside another:
/// a pattern's fuse, which OCCT glues only then).
pub fn boolean_touching(op: Op, args: &[&Body], tol: f64, touching_only: bool) -> R<Output> {
    boolean_with(op, args, tol, touching_only, None)
}

/// The same; free-form (NURBS) faces taken where `src` gives their sections (else declined).
pub fn boolean_with(op: Op, args: &[&Body], tol: f64, touching_only: bool, src: Option<&dyn SectionSource>) -> R<Output> {
    if args.len() < 2 {
        return Err(Decline("a Boolean needs an object and a tool"));
    }
    if op == Op::Common && args.len() > 2 {
        return Err(Decline("common with several tools"));
    }
    let mut cx = Ctx {
        tol,
        pool: Pool { p: Vec::new(), inputs: Vec::new(), grid: HashMap::default(), cell: GREY * tol * 1.01, tol },
        faces: Vec::new(),
        edges: Vec::new(),
        solids: Vec::new(),
        side_cache: HashMap::default(),
        in_face_done: Default::default(),
        face_segs: Vec::new(),
        coplanar: Vec::new(),
        sections: Vec::new(),
        ncurves: Vec::new(),
        src,
    };
    // Vertices, edges, faces and solids of every argument (a vertex outside every other argument's box can't
    // be one of theirs).
    // Two sheets (faces, no solids) apart, or on planes that cross: an empty common (faces in common only where
    // they overlap), everything deleted, as OCCT's (model-core asks whether faces overlap so).
    if op == Op::Common && args.len() == 2 && args.iter().all(|b| b.solids.is_empty() && !b.faces.is_empty()) {
        let plane = |f: &Face| match f.surface {
            Surface::Plane { origin, normal } => Some((origin, normal)),
            _ => None,
        };
        let crossing = args[0].faces.iter().all(|f| {
            args[1].faces.iter().all(|g| match (plane(f), plane(g)) {
                (Some((o1, n1)), Some((o2, n2))) => n1.cross(n2).length() > 1e-9 || (o2 - o1).dot(n1).abs() > GREY * tol,
                _ => false,
            })
        });
        let bx = |b: &Body| {
            let mut lo = DVec3::INFINITY;
            let mut hi = DVec3::NEG_INFINITY;
            for &p in &b.vertices {
                lo = lo.min(p);
                hi = hi.max(p);
            }
            for e in &b.edges {
                if let Curve::Circle { center, radius, .. } = e.curve {
                    lo = lo.min(center - DVec3::splat(radius));
                    hi = hi.max(center + DVec3::splat(radius));
                }
            }
            (lo, hi)
        };
        let ((l0, h0), (l1, h1)) = (bx(args[0]), bx(args[1]));
        if !crossing && boxes_meet(l0, h0, l1, h1, GREY * tol) {
            return Err(Decline("sheets that meet"));
        }
        let mut out = Output::default();
        for (a, b) in args.iter().enumerate() {
            let del = |kind: Kind, n: usize| (0..n as u32).map(move |i| Record { kind, arg: a as u32, index: i, relation: Relation::Deleted, out_kind: kind, out: u32::MAX });
            out.history.extend(del(Kind::Face, b.faces.len()).chain(del(Kind::Edge, b.edges.len())).chain(del(Kind::Vertex, b.vertices.len())));
        }
        return Ok(out);
    }
    // A line alone (an edge, no faces) and a solid: their common (line.rs).
    let line = op == Op::Common && args.len() == 2 && args[0].faces.is_empty() && args[0].edges.len() == 1 && matches!(args[0].edges[0].curve, Curve::Line);
    let arg_box: Vec<(DVec3, DVec3)> = args.iter().map(|b| b.vertices.iter().fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(lo, hi), &p| (lo.min(p), hi.max(p)))).collect();
    let mut first_edge = Vec::new();
    for (ai, body) in args.iter().enumerate() {
        let a = ai as u32;
        let mut vmap = Vec::with_capacity(body.vertices.len());
        for (i, &p) in body.vertices.iter().enumerate() {
            let near = arg_box.iter().enumerate().any(|(k, b)| k != ai && boxes_meet(p, p, b.0, b.1, GREY * tol));
            let found = if near { cx.pool.find(p)? } else { None };
            let w = match found {
                Some(w) => {
                    if cx.pool.inputs[w as usize].iter().any(|x| x.0 == a) {
                        return Err(Decline("two vertices of one argument coincide"));
                    }
                    w
                }
                None => cx.pool.push(p),
            };
            cx.pool.inputs[w as usize].push((a, i as u32));
            vmap.push(w);
        }
        first_edge.push(cx.edges.len());
        for (i, e) in body.edges.iter().enumerate() {
            let v = [vmap[e.v[0] as usize], vmap[e.v[1] as usize]];
            let g = match e.curve {
                Curve::Ellipse { center, normal, xdir, major, minor, .. } => {
                    // An argument's ellipse (a tilted hole's rim): OCCT's frame, so its parameter OCCT's.
                    let e = Ell { c: center, n: normal, x: xdir, y: normal.cross(xdir), a: major, b: minor, hyp: false };
                    let (p0, p1) = (body.vertices[body.edges[i].v[0] as usize], body.vertices[body.edges[i].v[1] as usize]);
                    let t0 = e.param(p0);
                    let sweep = if v[0] == v[1] { TAU } else { (e.param(p1) - t0).rem_euclid(TAU) };
                    Geo::Ell { e, t0, sweep, faces: [usize::MAX; 2] }
                }
                Curve::Hyperbola { .. } => return Err(Decline("an argument's hyperbola")),
                Curve::Nurbs { .. } if cx.src.is_none() => return Err(Decline("a curved edge")),
                Curve::Nurbs { index, t0, t1 } => {
                    cx.ncurves.push(NCurve { c: Arc::new(body.nurbs_curves[index as usize].clone()), sec: None, tol: 0.0 });
                    Geo::Nurbs { id: cx.ncurves.len() as u32 - 1, t0, t1 }
                }
                Curve::Degenerate => {
                    if v[0] != v[1] {
                        return Err(Decline("a degenerate edge between two vertices"));
                    }
                    Geo::Apex
                }
                Curve::Line => {
                    if v[0] == v[1] {
                        return Err(Decline("a closed or tiny edge"));
                    }
                    Geo::Line
                }
                Curve::Circle { center, normal, xdir, radius } => {
                    let k = Circ { c: center, n: normal, x: xdir, y: normal.cross(xdir), r: radius };
                    let a0 = k.angle(cx.pool.p[v[0] as usize]);
                    let sweep = if v[0] == v[1] { TAU } else { (k.angle(cx.pool.p[v[1] as usize]) - a0).rem_euclid(TAU) };
                    if sweep * radius <= tol {
                        return Err(Decline("a tiny arc"));
                    }
                    Geo::Arc { k, a0, sweep }
                }
            };
            cx.edges.push(GEdge { arg: a, index: i as u32, v, g });
        }
        let face0 = cx.faces.len();
        for (i, f) in body.faces.iter().enumerate() {
            let edges: Vec<(usize, bool)> = f.loops.iter().flatten().map(|u| (first_edge[ai] + u.edge as usize, u.reversed)).collect();
            let s = match f.surface {
                Surface::Nurbs { .. } | Surface::Torus { .. } | Surface::Sphere { .. } | Surface::Extrusion { .. } | Surface::Revolution { .. } if cx.src.is_none() => {
                    return Err(Decline("a curved face"));
                }
                // (Its scaling once its parameter box is known, below.)
                Surface::Nurbs { .. } | Surface::Torus { .. } | Surface::Sphere { .. } | Surface::Extrusion { .. } | Surface::Revolution { .. } => Surf::Nurbs { su: 1.0, sv: 1.0, flip: f.reversed },
                Surface::Plane { origin, normal } => {
                    let n = if f.reversed { -normal } else { normal };
                    let (u, v) = frame(n);
                    Surf::Plane { n, d: n.dot(origin), origin, u, v, flip: f.reversed }
                }
                Surface::Cylinder { origin, axis, xdir, radius } => round(&cx, &edges, [origin, axis, xdir], radius, 0.0, f.reversed),
                Surface::Cone { origin, axis, xdir, radius, angle } => round(&cx, &edges, [origin, axis, xdir], radius, angle.tan(), f.reversed),
            };
            let free = match f.surface {
                Surface::Nurbs { index } => Some(FreeSurface::Nurbs(body.nurbs_surfaces[index as usize].clone())),
                Surface::Torus { origin, xdir, ydir, zdir, major, minor } => Some(FreeSurface::Torus { o: origin, x: xdir, y: ydir, z: zdir, major, minor }),
                Surface::Sphere { origin, xdir, ydir, zdir, radius } => Some(FreeSurface::Sphere { o: origin, x: xdir, y: ydir, z: zdir, r: radius }),
                Surface::Extrusion { curve, dir } => Some(FreeSurface::Extrusion { c: body.nurbs_curves[curve as usize].clone(), d: dir }),
                Surface::Revolution { curve, origin, axis } => Some(FreeSurface::Revolution { c: body.nurbs_curves[curve as usize].clone(), o: origin, a: axis }),
                _ => None,
            };
            let (ns, pcurves): (Option<Arc<FreeSurface>>, PCurves) = match free {
                Some(fs) => {
                    let pcurves = body
                        .pcurves
                        .iter()
                        .filter(|p| p.face == i as u32)
                        .map(|p| ((first_edge[ai] + p.edge as usize, p.second), (Arc::new(body.nurbs_curves[p.curve as usize].clone()), p.t0, p.t1)))
                        .collect();
                    (Some(Arc::new(fs)), pcurves)
                }
                None => (None, HashMap::default()),
            };
            let uvbox = free::uv_box(ns.as_deref(), &pcurves);
            let s = match &ns {
                Some(fs) => free::nurbs_surf(fs, uvbox, f.reversed),
                None => s,
            };
            cx.faces.push(GFace { arg: a, index: i as u32, solid: usize::MAX, s, edges, bound: Default::default(), lo: DVec3::INFINITY, hi: DVec3::NEG_INFINITY, ns, uvbox, pcurves, grid: Default::default(), patches: Default::default() });
        }
        for (i, s) in body.solids.iter().enumerate() {
            let si = cx.solids.len();
            // (Its voids' faces too: a ray crosses them as it does the outer shell's.)
            let faces: Vec<usize> = s.shells.iter().flatten().map(|&f| face0 + f as usize).collect();
            for &f in &faces {
                if cx.faces[f].solid != usize::MAX {
                    return Err(Decline("a face in two solids"));
                }
                cx.faces[f].solid = si;
            }
            cx.solids.push(GSolid { arg: a, index: i as u32, faces, lo: DVec3::INFINITY, hi: DVec3::NEG_INFINITY });
        }
        if cx.faces[face0..].iter().any(|f| f.solid == usize::MAX) {
            return Err(Decline("a face outside a solid"));
        }
        if body.solids.is_empty() && !(line && ai == 0) {
            return Err(Decline("an argument without solids"));
        }
    }
    // Each face: its parameter range (a partial cylinder's starts in the gap), boundary, box.
    for f in 0..cx.faces.len() {
        let mut lo = DVec3::INFINITY;
        let mut hi = DVec3::NEG_INFINITY;
        for &(e, _) in &cx.faces[f].edges {
            let s = cx.edge_seg(e);
            let n = match s.g {
                Geo::Line | Geo::Apex => 1,
                Geo::Arc { sweep, .. } | Geo::Ell { sweep, .. } => (sweep / (PI / 8.0)).ceil() as usize + 1,
                Geo::Nurbs { id, t0, t1 } => {
                    let (blo, bhi) = free::piece_box(&cx.ncurves[id as usize].c, t0, t1);
                    lo = lo.min(blo);
                    hi = hi.max(bhi);
                    1
                }
            };
            for k in 0..=n {
                let p = cx.seg_at(&s, k as f64 / n as f64);
                lo = lo.min(p);
                hi = hi.max(p);
            }
            if let Geo::Arc { k, .. } = s.g {
                // Arcs bulge between the samples by at most r (1 - cos(π/16)).
                let bulge = DVec3::splat(k.r * (1.0 - (PI / 16.0).cos()));
                lo -= bulge;
                hi += bulge;
            }
            if let Geo::Ell { e, t0, sweep, .. } = s.g {
                // An ellipse by at most h² / 8 times its largest second derivative (a) for steps h ≤ π/8.
                let (elo, ehi) = e.piece_box(t0, sweep);
                let bulge = DVec3::splat(e.a * (PI / 8.0).powi(2) / 8.0);
                lo = lo.min(elo) - bulge;
                hi = hi.max(ehi) + bulge;
            }
        }
        if let Some((blo, bhi)) = free::face_box(&cx.faces[f]) {
            lo = lo.min(blo);
            hi = hi.max(bhi);
        }
        cx.faces[f].lo = lo;
        cx.faces[f].hi = hi;
        if let Surf::Cyl { o, x, y, full: false, .. } = cx.faces[f].s {
            // The angles its boundary covers; th0 in the gap.
            let mut covered: Vec<(f64, f64)> = Vec::new();
            for &(e, _) in &cx.faces[f].edges {
                if let Geo::Arc { k, a0, sweep } = cx.edges[e].g {
                    let w = k.at(a0) - o;
                    let s = w.dot(y).atan2(w.dot(x));
                    let along = if let Surf::Cyl { a, .. } = cx.faces[f].s { k.n.dot(a) > 0.0 } else { true };
                    covered.push(if along { (s, sweep) } else { (s - sweep, sweep) });
                }
            }
            let Some(th0) = gap(&covered) else { return Err(Decline("a cylinder face without arcs")) };
            if let Surf::Cyl { th0: t, .. } = &mut cx.faces[f].s {
                *t = th0;
            }
        }
        let s = cx.faces[f].solid;
        cx.solids[s].lo = cx.solids[s].lo.min(lo);
        cx.solids[s].hi = cx.solids[s].hi.max(hi);
    }
    for f in 0..cx.faces.len() {
        if let Surf::Plane { n, d, .. } = cx.faces[f].s {
            for &(e, _) in &cx.faces[f].edges {
                for v in cx.edges[e].v {
                    if (n.dot(cx.pool.p[v as usize]) - d).abs() > tol {
                        return Err(Decline("a face's vertex off its plane"));
                    }
                }
            }
        }
    }
    if line {
        return line::line_common(&cx, tol);
    }
    cx.face_segs = vec![Vec::new(); cx.faces.len()];
    // Pairs of solids that may meet, and their faces.
    let ns = cx.solids.len();
    let mut meets = vec![false; ns * ns];
    for s in 0..ns {
        for t in s + 1..ns {
            let (a, b) = (&cx.solids[s], &cx.solids[t]);
            if a.arg == b.arg || !boxes_meet(a.lo, a.hi, b.lo, b.hi, GREY * tol) {
                continue;
            }
            meets[s * ns + t] = true;
            meets[t * ns + s] = true;
            for i in 0..cx.solids[s].faces.len() {
                let fs = cx.solids[s].faces[i];
                for j in 0..cx.solids[t].faces.len() {
                    let ft = cx.solids[t].faces[j];
                    let (x, y) = (&cx.faces[fs], &cx.faces[ft]);
                    if boxes_meet(x.lo, x.hi, y.lo, y.hi, GREY * tol) {
                        cx.face_pair(fs, ft)?;
                    }
                }
            }
        }
    }
    // Curves added to one face that cross each other (edges of two arguments lying on a third's face): a
    // vertex where they cross.
    for f in 0..cx.faces.len() {
        let segs = cx.face_segs[f].clone();
        if segs.len() < 2 {
            continue;
        }
        let c2: Vec<C2> = segs.iter().map(|s| cx.c2_of(f, s, 0, None)).collect::<R<_>>()?;
        for i in 0..segs.len() {
            for j in i + 1..segs.len() {
                for (t, u) in c2[i].intersections(&c2[j], tol) {
                    let (li, lj) = (cx.seg_length(&segs[i]), cx.seg_length(&segs[j]));
                    if t * li > tol && (1.0 - t) * li > tol && u * lj > tol && (1.0 - u) * lj > tol {
                        let n = cx.pool.p.len();
                        cx.pool.add(cx.seg_at(&segs[i], t))?;
                        // (With a vertex of three arguments or more on this face, or of four anywhere, OCCT's
                        // choices differ: not done.)
                        let three = |w: u32| cx.pool.inputs[w as usize].len() >= 3;
                        if cx.pool.p.len() > n
                            && (segs.iter().any(|s| s.v.iter().any(|&w| three(w)))
                                || cx.faces[f].edges.iter().any(|&(e, _)| cx.edges[e].v.iter().any(|&w| three(w)))
                                || cx.pool.inputs.iter().any(|ins| ins.len() >= 4))
                        {
                            return Err(Decline("edges of two arguments crossing on a third's face"));
                        }
                    }
                }
            }
        }
    }
    // Split edges.
    let mut by_x: Vec<(f64, u32)> = cx.pool.p.iter().enumerate().map(|(i, p)| (p.x, i as u32)).collect();
    by_x.sort_by(|a, b| a.0.total_cmp(&b.0));
    // The same by owner: an argument's alone (merged ones and new ones under None).
    let owner = |w: u32| match cx.pool.inputs[w as usize].as_slice() {
        [(a, _)] => Some(*a),
        _ => None,
    };
    let mut lists: ByOwner = Vec::new();
    for &(x, w) in &by_x {
        let o = owner(w);
        match lists.iter_mut().find(|l| l.0 == o) {
            Some(l) => l.1.push((x, w)),
            None => lists.push((o, vec![(x, w)])),
        }
    }
    // Each edge's solid, and whether anything of another argument comes near it.
    let mut edge_solid = vec![usize::MAX; cx.edges.len()];
    for f in &cx.faces {
        for &(e, _) in &f.edges {
            edge_solid[e] = f.solid;
        }
    }
    let mut sedges: Vec<SEdge> = Vec::new();
    let mut sedge_of: HashMap<(u32, u32), Vec<usize>> = HashMap::default();
    // The split edge of `s` (made if new), and whether it runs the other way.
    let mut get_sedge = |cx: &Ctx, sedges: &mut Vec<SEdge>, s: Seg| -> (usize, bool) {
        let key = (s.v[0].min(s.v[1]), s.v[0].max(s.v[1]));
        let mid = cx.seg_at(&s, 0.5);
        let list = sedge_of.entry(key).or_default();
        for &i in list.iter() {
            let o = &sedges[i].seg;
            if std::mem::discriminant(&o.g) == std::mem::discriminant(&s.g) && (cx.seg_at(o, 0.5) - mid).length() <= 100.0 * cx.tol {
                let rev = match (o.g, s.g) {
                    (Geo::Arc { k: k1, .. }, Geo::Arc { k: k2, .. }) => k1.n.dot(k2.n) < 0.0,
                    (Geo::Ell { e: e1, t0: a1, sweep: s1, .. }, Geo::Ell { e: e2, t0: a2, sweep: s2, .. }) => e1.deriv(a1 + 0.5 * s1).dot(e2.deriv(a2 + 0.5 * s2)) < 0.0,
                    _ => o.v[0] != s.v[0],
                };
                return (i, rev);
            }
        }
        list.push(sedges.len());
        sedges.push(SEdge { seg: s, origins: Vec::new() });
        (sedges.len() - 1, false)
    };
    let involved: Vec<bool> = (0..ns).map(|s| meets[s * ns..(s + 1) * ns].iter().any(|&m| m)).collect();
    // Input vertices on another argument's edge or face (OCCT makes them anew), and the new vertices
    // inside each input edge and face (which OCCT says they generated).
    let mut touched = vec![false; cx.pool.p.len()];
    let note = |touched: &mut Vec<bool>, pool: &Pool, w: u32, arg: u32| {
        if !pool.inputs[w as usize].is_empty() && !pool.inputs[w as usize].iter().any(|x| x.0 == arg) {
            touched[w as usize] = true;
        }
    };
    // Input vertices inside other arguments' edges, and those edges.
    let mut inside_edges: HashMap<u32, Vec<usize>> = HashMap::default();
    let mut edge_inner: Vec<Vec<u32>> = Vec::with_capacity(cx.edges.len());
    let mut edge_pieces: Vec<Vec<(usize, bool)>> = Vec::with_capacity(cx.edges.len());
    for (e, &es) in edge_solid.iter().enumerate() {
        let s = cx.edge_seg(e);
        let (lo, hi) = match s.g {
            Geo::Line | Geo::Apex => (cx.p(s.v[0]).min(cx.p(s.v[1])), cx.p(s.v[0]).max(cx.p(s.v[1]))),
            Geo::Arc { k, .. } => (k.c - DVec3::splat(k.r), k.c + DVec3::splat(k.r)),
            Geo::Ell { e, t0, sweep, .. } => e.piece_box(t0, sweep),
            Geo::Nurbs { id, t0, t1 } => free::piece_box(&cx.ncurves[id as usize].c, t0, t1),
        };
        let near = es != usize::MAX && (0..ns).any(|t| meets[es * ns + t] && boxes_meet(lo, hi, cx.solids[t].lo, cx.solids[t].hi, GREY * tol));
        let chain = if near { cx.split_among(&s, &lists, Some(cx.edges[e].arg))? } else { vec![s.v[0], s.v[1]] };
        for &w in &chain {
            note(&mut touched, &cx.pool, w, cx.edges[e].arg);
        }
        for &w in &chain[1..chain.len() - 1] {
            if !cx.pool.inputs[w as usize].is_empty() {
                inside_edges.entry(w).or_default().push(e);
            }
        }
        edge_inner.push(chain[1..chain.len() - 1].to_vec());
        let mut pieces = Vec::with_capacity(chain.len() - 1);
        for piece in cx.pieces_of(&s, &chain) {
            let (si, rev) = get_sedge(&cx, &mut sedges, piece);
            if !sedges[si].origins.contains(&e) {
                sedges[si].origins.push(e);
            }
            pieces.push((si, rev));
        }
        edge_pieces.push(pieces);
    }
    // An input vertex inside edges of two other arguments (coincident there: a common block): OCCT
    // (non-destructive) replaces it for each edge, and again when the block's tolerance grows past the new
    // vertex's, which rounding decides (4e-16 off: grows), and then its history says Deleted, not Modified.
    // Taken only where nothing can be rounded: straight edges along an axis, exactly through it.
    for (&w, es) in &inside_edges {
        let mut args: Vec<u32> = es.iter().map(|&e| cx.edges[e].arg).collect();
        args.sort_unstable();
        args.dedup();
        if args.len() < 2 {
            continue;
        }
        // (Crossing there: Modified, as for one edge.)
        let at = |e: usize| {
            let mut v: Vec<usize> = edge_pieces[e].iter().map(|x| x.0).filter(|&si| sedges[si].seg.v.contains(&w)).collect();
            v.sort_unstable();
            v
        };
        let coincident = es.iter().enumerate().any(|(i, &e)| es[i + 1..].iter().any(|&g| cx.edges[g].arg != cx.edges[e].arg && at(g) == at(e)));
        if !coincident {
            continue;
        }
        let p = cx.p(w);
        let exact = |e: usize| {
            let [a, b] = cx.edges[e].v;
            let (a, b) = (cx.p(a), cx.p(b));
            let d = b - a;
            cx.edges[e].g == Geo::Line && (p - a).cross(d) == DVec3::ZERO && [d.x, d.y, d.z].iter().filter(|&&c| c == 0.0).count() == 2
        };
        if !es.iter().all(|&e| exact(e)) {
            return Err(Decline("a vertex inside coincident edges of two other arguments"));
        }
    }
    let mut face_extra: Vec<Vec<usize>> = vec![Vec::new(); cx.faces.len()];
    for (f, extra) in face_extra.iter_mut().enumerate() {
        for k in 0..cx.face_segs[f].len() {
            let s = cx.face_segs[f][k];
            let chain = cx.split(&s, &by_x)?;
            for piece in cx.pieces_of(&s, &chain) {
                extra.push(get_sedge(&cx, &mut sedges, piece).0);
            }
        }
    }
    // The faces each section edge was made by (OCCT: they generated it).
    let mut section_faces: Vec<(usize, usize)> = Vec::new();
    for k in 0..cx.sections.len() {
        let (s, fs, ft) = cx.sections[k];
        let chain = cx.split(&s, &by_x)?;
        for piece in cx.pieces_of(&s, &chain) {
            let (si, _) = get_sedge(&cx, &mut sedges, piece);
            section_faces.push((si, fs));
            section_faces.push((si, ft));
        }
    }
    section_faces.sort_unstable();
    section_faces.dedup();
    // Pieces of faces.
    let mut pieces: Vec<Piece> = Vec::new();
    let mut face_pieces: Vec<Vec<usize>> = vec![Vec::new(); cx.faces.len()];
    let mut face_split: Vec<bool> = vec![false; cx.faces.len()];
    let mut face_inner: Vec<Vec<u32>> = vec![Vec::new(); cx.faces.len()];
    for f in 0..cx.faces.len() {
        let mut segs: Vec<usize> = cx.faces[f].edges.iter().flat_map(|&(e, _)| edge_pieces[e].iter().map(|x| x.0)).collect();
        segs.sort_unstable();
        segs.dedup();
        let n_boundary = segs.len();
        let boundary_split = cx.faces[f].edges.iter().any(|&(e, _)| edge_pieces[e].len() != 1);
        segs.extend(face_extra[f].iter().copied());
        segs.sort_unstable();
        segs.dedup();
        face_split[f] = boundary_split || segs.len() != n_boundary;
        // A face nothing reaches is one piece as it was (out of every other solid).
        let fs = cx.faces[f].solid;
        let near = involved[fs]
            && (0..ns).any(|t| meets[fs * ns + t] && boxes_meet(cx.faces[f].lo, cx.faces[f].hi, cx.solids[t].lo, cx.solids[t].hi, GREY * tol));
        if !near && !face_split[f] {
            let loops = vec![cx.faces[f].edges.iter().map(|&(e, r)| (edge_pieces[e][0].0, r ^ edge_pieces[e][0].1)).collect()];
            face_pieces[f].push(pieces.len());
            pieces.push(Piece { face: f, loops, inside: DVec3::NAN });
            continue;
        }
        // The curves in the face's parameter plane (a seam's pieces on both sides).
        let seams: HashSet<usize> = {
            let mut seen = HashSet::default();
            cx.faces[f].edges.iter().filter(|&&(e, _)| !seen.insert(e)).flat_map(|&(e, _)| edge_pieces[e].iter().map(|x| x.0)).collect()
        };
        let mut local: HashMap<(u32, bool), u32> = HashMap::default();
        // (On a free-form face a vertex is a point per place it is at, both sides of a seam: its curves' ends there
        // within 100 tol.)
        let mut places: HashMap<u32, Vec<(DVec2, u32)>> = HashMap::default();
        let mut pts: Vec<DVec2> = Vec::new();
        let mut lsegs: Vec<([u32; 2], C2)> = Vec::with_capacity(segs.len());
        let mut lsrc: Vec<usize> = Vec::with_capacity(segs.len());
        for &si in &segs {
            let s = sedges[si].seg;
            for copy in 0..if seams.contains(&si) { 2 } else { 1 } {
                // (The origin that is this face's own edge: its curve on a free-form face is that edge's.)
                let own = sedges[si].origins.iter().copied().find(|&e| cx.faces[f].edges.iter().any(|x| x.0 == e)).or(sedges[si].origins.first().copied());
                let c = cx.c2_of(f, &s, copy, own)?;
                let mut ends = [0u32; 2];
                for (k, (w, q)) in [(s.v[0], c.start()), (s.v[1], c.end())].into_iter().enumerate() {
                    // A vertex on a seam (or a cone's apex) is at two places in the parameter plane.
                    let right = match cx.faces[f].s {
                        Surf::Cyl { r, th0, .. } => q.x > r * (th0 + PI),
                        Surf::Plane { .. } | Surf::Nurbs { .. } => false,
                    };
                    if let Surf::Nurbs { .. } = cx.faces[f].s {
                        let at = places.entry(w).or_default();
                        ends[k] = match at.iter().find(|x| (x.0 - q).length() <= 100.0 * tol) {
                            Some(x) => x.1,
                            None => {
                                pts.push(q);
                                at.push((q, (pts.len() - 1) as u32));
                                (pts.len() - 1) as u32
                            }
                        };
                        local.insert((w, false), ends[k]);
                        continue;
                    }
                    ends[k] = *local.entry((w, right)).or_insert_with(|| {
                        pts.push(q);
                        (pts.len() - 1) as u32
                    });
                }
                lsegs.push((ends, c));
                lsrc.push(si);
            }
        }
        let boundary: HashSet<u32> = cx.faces[f].edges.iter().flat_map(|&(e, _)| edge_pieces[e].iter().flat_map(|x| sedges[x.0].seg.v)).collect();
        let mut inner: Vec<u32> = local.keys().map(|k| k.0).filter(|w| !boundary.contains(w)).collect();
        inner.sort_unstable();
        inner.dedup();
        for &w in &inner {
            note(&mut touched, &cx.pool, w, cx.faces[f].arg);
        }
        face_inner[f] = inner;
        for mut r in regions(&pts, &lsegs)? {
            let mut dist = distance2(r.inside, cx.bound(f)?);
            if dist <= GREY * tol {
                // (A thin piece: a point farther in.)
                r.inside = deeper(&lsegs, &r);
                dist = distance2(r.inside, cx.bound(f)?);
            }
            if dist <= GREY * tol {
                return Err(Decline("a piece's point near its face's boundary"));
            }
            if !inside2(r.inside, cx.bound(f)?) {
                continue;
            }
            if r.dangling {
                return Err(Decline("an edge ends inside a face"));
            }
            let inside3 = cx.faces[f].to3(r.inside);
            let mut loops = vec![r.outer];
            loops.extend(r.holes);
            let loops = loops.into_iter().map(|l| l.into_iter().map(|(s, rev)| (lsrc[s as usize], rev)).collect()).collect();
            face_pieces[f].push(pieces.len());
            pieces.push(Piece { face: f, loops, inside: inside3 });
        }
        if face_pieces[f].is_empty() {
            return Err(Decline("a face lost"));
        }
    }
    // Where each piece is against the other arguments' solids.
    let mut keep: Vec<Option<bool>> = vec![None; pieces.len()]; // Some(reversed)
    let mut partner: Vec<Option<usize>> = vec![None; pieces.len()];
    let piece_key = |p: &Piece| -> Vec<usize> {
        let mut k: Vec<usize> = p.loops.iter().flatten().map(|x| x.0).collect();
        k.sort_unstable();
        k
    };
    let mut on_pairs: Vec<(usize, usize)> = Vec::new();
    // Each piece's solids it is in or on (same side).
    let mut within: Vec<Vec<usize>> = vec![Vec::new(); pieces.len()];
    // Solids another argument's piece is in or on (OCCT builds them anew).
    let mut reached = vec![false; cx.solids.len()];
    for pi in 0..pieces.len() {
        let f = pieces[pi].face;
        let s = cx.faces[f].solid;
        let arg = cx.faces[f].arg;
        let mut states: Vec<(usize, State)> = Vec::new();
        for t in 0..cx.solids.len() {
            if cx.solids[t].arg == arg {
                continue;
            }
            if !meets[s * ns + t] || pieces[pi].inside.is_nan() {
                states.push((t, State::Out));
                continue;
            }
            let mut st = None;
            for &(a, b) in &cx.coplanar {
                let g = if a == f && cx.faces[b].solid == t {
                    b
                } else if b == f && cx.faces[a].solid == t {
                    a
                } else {
                    continue;
                };
                let q = cx.faces[g].to2(pieces[pi].inside);
                let d = distance2(q, cx.bound(g)?);
                if d <= GREY * tol {
                    return Err(Decline("a piece's point near a coincident face's boundary"));
                }
                if inside2(q, cx.bound(g)?) {
                    let p = pieces[pi].inside;
                    st = Some(if cx.normal_at(g, p).dot(cx.normal_at(f, p)) > 0.0 { State::OnSame } else { State::OnOpposite });
                    break;
                }
            }
            let st = match st {
                Some(st) => st,
                None => cx.ray_state(pieces[pi].inside, t)?,
            };
            states.push((t, st));
        }
        within[pi] = states.iter().filter(|x| matches!(x.1, State::In | State::OnSame)).map(|x| x.0).collect();
        for &(t, st) in &states {
            if st != State::Out {
                reached[t] = true;
            }
        }
        let ons: Vec<(usize, State)> = states.iter().copied().filter(|x| matches!(x.1, State::OnSame | State::OnOpposite)).collect();
        if ons.len() > 1 && op != Op::Fuse {
            return Err(Decline("a piece on two solids"));
        }
        let in_any = states.iter().any(|x| x.1 == State::In);
        if touching_only && in_any {
            return Err(Decline("pattern instances overlapping"));
        }
        let in_obj = states.iter().any(|x| x.1 == State::In && cx.solids[x.0].arg == 0);
        let in_tool = states.iter().any(|x| x.1 == State::In && cx.solids[x.0].arg != 0);
        let on = ons.first().copied();
        // Two tools' pieces coinciding (a cut), facing each other: inside the tools.
        let tools_on = op == Op::Cut && arg != 0 && on.is_some_and(|(t, _)| cx.solids[t].arg != 0);
        if let Some((t, st)) = on {
            // (Facing one way OCCT may build the object's solid anew: its history says it's deleted.)
            if op != Op::Fuse && arg != 0 && cx.solids[t].arg != 0 && (op == Op::Common || st == State::OnSame) {
                return Err(Decline("tools coincide"));
            }
            on_pairs.push((pi, t));
        }
        keep[pi] = match op {
            // Coincident pieces facing one way: the first argument's is kept; facing each other: none.
            Op::Fuse => (!in_any && ons.iter().all(|&(t, st)| st == State::OnSame && cx.solids[t].arg > arg)).then_some(false),
            Op::Cut => {
                if arg == 0 {
                    match on {
                        None => (!in_tool).then_some(false),
                        Some((_, State::OnOpposite)) => (!in_tool).then_some(false),
                        _ => None,
                    }
                } else if tools_on {
                    None
                } else {
                    (on.is_none() && in_obj && !in_tool).then_some(true)
                }
            }
            Op::Common => match on {
                None => (in_any).then_some(false),
                Some((_, State::OnSame)) => (arg == 0).then_some(false),
                _ => None,
            },
        };
    }
    // Coincident pieces: each goes to the first argument's (each has one, split the same way).
    let mut by_key: HashMap<Vec<usize>, (usize, usize)> = HashMap::default();
    for &(pi, _) in &on_pairs {
        let e = by_key.entry(piece_key(&pieces[pi])).or_insert((pi, 0));
        e.1 += 1;
        if cx.faces[pieces[pi].face].arg < cx.faces[pieces[e.0].face].arg {
            e.0 = pi;
        }
    }
    for &(pi, _) in &on_pairs {
        let (o, n) = by_key[&piece_key(&pieces[pi])];
        if n < 2 {
            return Err(Decline("coincident pieces differ"));
        }
        if o != pi {
            partner[pi] = Some(o);
        }
    }
    // The result's faces and their edges.
    let kept: Vec<usize> = (0..pieces.len()).filter(|&p| keep[p].is_some()).collect();
    if kept.is_empty() {
        return Err(Decline("an empty result"));
    }
    let mut uses: HashMap<usize, [i32; 2]> = HashMap::default();
    for &p in &kept {
        let flip = keep[p] == Some(true);
        for l in &pieces[p].loops {
            for &(s, r) in l {
                let u = uses.entry(s).or_insert([0, 0]);
                u[(r ^ flip) as usize] += 1;
            }
        }
    }
    // (A cone's apex is used once.)
    if uses.iter().any(|(&s, u)| sedges[s].seg.g != Geo::Apex && (u[0] != u[1] || u[0] == 0)) {
        return Err(Decline("a non-manifold result"));
    }
    // Seams: an edge one face uses both ways; any other edge two faces once each.
    for (&s, u) in &uses {
        if u[0] > 1 && sedges[s].seg.g != Geo::Apex {
            let users = kept.iter().filter(|&&p| pieces[p].loops.iter().flatten().any(|x| x.0 == s)).count();
            if users != 1 || u[0] != 1 {
                return Err(Decline("a non-manifold result"));
            }
        }
    }
    // Shells: pieces joined by their edges.
    let mut comp: Vec<usize> = (0..kept.len()).collect();
    fn find(c: &mut [usize], mut i: usize) -> usize {
        while c[i] != i {
            c[i] = c[c[i]];
            i = c[i];
        }
        i
    }
    let mut first_user: HashMap<usize, usize> = HashMap::default();
    for (k, &p) in kept.iter().enumerate() {
        for l in &pieces[p].loops {
            for &(s, _) in l {
                if let Some(&o) = first_user.get(&s) {
                    let (a, b) = (find(&mut comp, k), find(&mut comp, o));
                    comp[a] = b;
                } else {
                    first_user.insert(s, k);
                }
            }
        }
    }
    // Output entities.
    let mut out = Output::default();
    let mut vmap = Dense::new(cx.pool.p.len());
    let mut emap = Dense::new(sedges.len());
    let mut used_s: Vec<usize> = uses.keys().copied().collect();
    used_s.sort_unstable();
    let mut used_v: Vec<u32> = used_s.iter().flat_map(|&s| sedges[s].seg.v).collect();
    used_v.sort_unstable();
    used_v.dedup();
    let vertex_same = |w: u32| cx.pool.inputs[w as usize].len() == 1 && !touched[w as usize];
    for &w in &used_v {
        vmap.insert(w as usize, out.body.vertices.len() as u32);
        out.body.vertices.push(cx.pool.p[w as usize]);
        out.vertex_origin.push(match cx.pool.inputs[w as usize].as_slice() {
            [(a, i)] if vertex_same(w) => Origin::Same { arg: *a, index: *i },
            _ => Origin::New,
        });
    }
    let edge_same: Vec<bool> = (0..sedges.len())
        .map(|s| {
            let x = &sedges[s];
            x.origins.len() == 1 && edge_pieces[x.origins[0]].len() == 1 && vertex_same(x.seg.v[0]) && vertex_same(x.seg.v[1])
        })
        .collect();
    let mut sedge_flip = vec![false; sedges.len()];
    for &s in &used_s {
        let x = &sedges[s];
        // Pieces of an input edge run its way.
        let flip = match x.origins.first() {
            Some(&e) => edge_pieces[e].iter().find(|p| p.0 == s).is_some_and(|p| p.1),
            None => false,
        };
        sedge_flip[s] = flip;
        let seg = x.seg;
        let (v, curve) = match seg.g {
            Geo::Apex if !edge_same[s] => return Err(Decline("a cone's apex changed")),
            Geo::Apex => (seg.v, Curve::Degenerate),
            Geo::Line => (if flip { [seg.v[1], seg.v[0]] } else { seg.v }, Curve::Line),
            Geo::Arc { k, .. } => {
                let (v, n) = if flip { ([seg.v[1], seg.v[0]], -k.n) } else { (seg.v, k.n) };
                (v, Curve::Circle { center: k.c, normal: n, xdir: k.x, radius: k.r })
            }
            // (Only new: OCCT's frame as the section made it.)
            Geo::Ell { .. } if flip => return Err(Decline("a piece of an argument's ellipse")),
            Geo::Ell { e, .. } if !x.origins.is_empty() => {
                // A piece of an argument's ellipse (its curve the argument's).
                (seg.v, Curve::Ellipse { center: e.c, normal: e.n, xdir: e.x, major: e.a, minor: e.b, faces: [(u32::MAX, u32::MAX); 2] })
            }
            Geo::Ell { e, faces, .. } if e.hyp => {
                let of = |f: usize| (cx.faces[f].arg, cx.faces[f].index);
                (seg.v, Curve::Hyperbola { center: e.c, normal: e.n, xdir: e.x, major: e.a, minor: e.b, faces: [of(faces[0]), of(faces[1])] })
            }
            Geo::Ell { e, faces, .. } => {
                let of = |f: usize| (cx.faces[f].arg, cx.faces[f].index);
                (seg.v, Curve::Ellipse { center: e.c, normal: e.n, xdir: e.x, major: e.a, minor: e.b, faces: [of(faces[0]), of(faces[1])] })
            }
            Geo::Nurbs { id, t0, t1 } => {
                out.body.nurbs_curves.push((*cx.ncurves[id as usize].c).clone());
                let index = out.body.nurbs_curves.len() as u32 - 1;
                if flip { ([seg.v[1], seg.v[0]], Curve::Nurbs { index, t0: t1, t1: t0 }) } else { (seg.v, Curve::Nurbs { index, t0, t1 }) }
            }
        };
        out.edge_section.push(match seg.g {
            Geo::Nurbs { id, .. } => cx.ncurves[id as usize].sec.as_ref().map_or(u32::MAX, |s| s.2),
            _ => u32::MAX,
        });
        emap.insert(s, out.body.edges.len() as u32);
        out.body.edges.push(Edge { v: [vmap[v[0] as usize], vmap[v[1] as usize]], curve });
        out.edge_origin.push(match x.origins.first() {
            None => Origin::New,
            Some(&e) => {
                let g = &cx.edges[e];
                if edge_same[s] { Origin::Same { arg: g.arg, index: g.index } } else { Origin::Piece { arg: g.arg, index: g.index } }
            }
        });
    }
    let mut piece_out = Dense::new(pieces.len());
    let mut face_same = vec![false; cx.faces.len()];
    for &p in &kept {
        let f = pieces[p].face;
        let flip = keep[p] == Some(true);
        let g = &cx.faces[f];
        let same = !face_split[f] && pieces[p].loops.iter().flatten().all(|&(s, _)| edge_same[s]);
        face_same[f] = same;
        let src = &args[g.arg as usize].faces[g.index as usize];
        // Each loop from where the input face's starts (its first edge's first piece), as OCCT's pieces.
        let at: HashMap<usize, usize> = if pieces[p].loops.iter().all(|l| l.len() <= 1) { HashMap::default() } else { g.edges.iter().enumerate().rev().map(|(i, x)| (x.0, i)).collect() };
        let rank = |s: usize| -> (usize, usize) {
            sedges[s]
                .origins
                .iter()
                .filter_map(|&e| {
                    let at = *at.get(&e)?;
                    let k = edge_pieces[e].iter().position(|x| x.0 == s)?;
                    Some((at, if g.edges[at].1 { edge_pieces[e].len() - 1 - k } else { k }))
                })
                .min()
                .unwrap_or((usize::MAX, 0))
        };
        let loops: Vec<Vec<Use>> = pieces[p]
            .loops
            .iter()
            .map(|l| {
                let start = (0..l.len()).min_by_key(|&k| rank(l[k].0)).unwrap_or(0);
                let mut l: Vec<Use> = l[start..].iter().chain(&l[..start]).map(|&(s, r)| Use { edge: emap[s], reversed: r ^ sedge_flip[s] }).collect();
                if flip {
                    l.reverse();
                    for u in &mut l {
                        u.reversed = !u.reversed;
                    }
                }
                l
            })
            .collect();
        piece_out.insert(p, out.body.faces.len() as u32);
        out.face_u0.push(if let Surf::Cyl { th0, .. } = g.s { th0 } else { 0.0 });
        // (A NURBS surface into the result's own table.)
        let surface = match (src.surface, g.ns.as_deref()) {
            (Surface::Nurbs { .. }, Some(FreeSurface::Nurbs(ns))) => {
                out.body.nurbs_surfaces.push(ns.clone());
                Surface::Nurbs { index: out.body.nurbs_surfaces.len() as u32 - 1 }
            }
            (Surface::Extrusion { dir, .. }, Some(FreeSurface::Extrusion { c, .. })) => {
                out.body.nurbs_curves.push(c.clone());
                Surface::Extrusion { curve: out.body.nurbs_curves.len() as u32 - 1, dir }
            }
            (Surface::Revolution { origin, axis, .. }, Some(FreeSurface::Revolution { c, .. })) => {
                out.body.nurbs_curves.push(c.clone());
                Surface::Revolution { curve: out.body.nurbs_curves.len() as u32 - 1, origin, axis }
            }
            (s, _) => s,
        };
        out.body.faces.push(Face { surface, reversed: src.reversed ^ flip, loops });
        out.face_origin.push(if same { Origin::Same { arg: g.arg, index: g.index } } else { Origin::Piece { arg: g.arg, index: g.index } });
    }
    // Solids, one per shell, outward.
    let mut shell_of: HashMap<usize, usize> = HashMap::default();
    for (k, &p) in kept.iter().enumerate() {
        let c = find(&mut comp, k);
        let n = shell_of.len();
        let si = *shell_of.entry(c).or_insert(n);
        if si == out.body.solids.len() {
            out.body.solids.push(Solid { shells: vec![Vec::new()] });
        }
        out.body.solids[si].shells[0].push(piece_out[p]);
    }
    // Shells inside out are voids: of the one solid there is (else declined).
    let (solids, voids): (Vec<Solid>, Vec<Solid>) = std::mem::take(&mut out.body.solids).into_iter().partition(|s| shell_volume(&out.body, &s.shells[0]) > 0.0);
    out.body.solids = solids;
    if !voids.is_empty() {
        let [solid] = &mut out.body.solids[..] else { return Err(Decline("a void in a result of several solids")) };
        solid.shells.extend(voids.into_iter().map(|v| v.shells.into_iter().next().unwrap_or_default()));
    }
    // A result solid that is an input solid with all its faces as they were, and nothing of another argument
    // in or on it, is that solid.
    let face_index: HashMap<(u32, u32), usize> = cx.faces.iter().enumerate().map(|(g, f)| ((f.arg, f.index), g)).collect();
    let mut solid_of_face = vec![0u32; out.body.faces.len()];
    for (k, so) in out.body.solids.iter().enumerate() {
        for &f in so.shells.iter().flatten() {
            solid_of_face[f as usize] = k as u32;
        }
        // Every face the input solid's own, unchanged, and all of them.
        let solid_of = |o: Origin| match o {
            Origin::Same { arg, index } => face_index.get(&(arg, index)).map(|&g| cx.faces[g].solid),
            _ => None,
        };
        let first = solid_of(out.face_origin[so.shells[0][0] as usize]);
        let same = first.filter(|&si| !reached[si] && cx.solids[si].faces.len() == so.shells.iter().map(Vec::len).sum::<usize>() && so.shells.iter().flatten().all(|&f| solid_of(out.face_origin[f as usize]) == Some(si)));
        let same = same.map(|si| Origin::Same { arg: cx.solids[si].arg, index: cx.solids[si].index });
        out.solid_origin.push(same.unwrap_or(Origin::New));
    }
    // History.
    let rec = |kind: Kind, arg: u32, index: u32, relation: Relation, out_kind: Kind, o: u32| Record { kind, arg, index, relation, out_kind, out: o };
    let mut h = std::mem::take(&mut out.history);
    for f in 0..cx.faces.len() {
        let g = &cx.faces[f];
        let mut outs: Vec<u32> = Vec::new();
        for &p in &face_pieces[f] {
            if let Some(o) = piece_out.get(p) {
                outs.push(o);
            } else if let Some(o) = partner[p].and_then(|q| piece_out.get(q)) {
                outs.push(o);
            }
        }
        if face_same[f] {
            continue;
        }
        if outs.is_empty() {
            h.push(rec(Kind::Face, g.arg, g.index, Relation::Deleted, Kind::Face, u32::MAX));
        }
        for o in outs {
            h.push(rec(Kind::Face, g.arg, g.index, Relation::Modified, Kind::Face, o));
        }
    }
    for e in 0..cx.edges.len() {
        let g = &cx.edges[e];
        let outs: Vec<u32> = edge_pieces[e].iter().filter_map(|p| emap.get(p.0)).collect();
        if outs.len() == 1 && edge_same[edge_pieces[e][0].0] {
            continue;
        }
        if outs.is_empty() {
            h.push(rec(Kind::Edge, g.arg, g.index, Relation::Deleted, Kind::Edge, u32::MAX));
        }
        for o in outs {
            h.push(rec(Kind::Edge, g.arg, g.index, Relation::Modified, Kind::Edge, o));
        }
    }
    for w in 0..cx.pool.p.len() as u32 {
        let o = vmap.get(w as usize);
        for &(a, i) in &cx.pool.inputs[w as usize] {
            match o {
                None => h.push(rec(Kind::Vertex, a, i, Relation::Deleted, Kind::Vertex, u32::MAX)),
                Some(o) if !vertex_same(w) => h.push(rec(Kind::Vertex, a, i, Relation::Modified, Kind::Vertex, o)),
                _ => {}
            }
        }
    }
    for &(s, f) in &section_faces {
        if let (true, Some(o)) = (sedges[s].origins.is_empty(), emap.get(s)) {
            h.push(rec(Kind::Face, cx.faces[f].arg, cx.faces[f].index, Relation::Generated, Kind::Edge, o));
        }
    }
    for (e, inner) in edge_inner.iter().enumerate() {
        for &w in inner {
            if let (true, Some(o)) = (cx.pool.inputs[w as usize].is_empty(), vmap.get(w as usize)) {
                h.push(rec(Kind::Edge, cx.edges[e].arg, cx.edges[e].index, Relation::Generated, Kind::Vertex, o));
            }
        }
    }
    // A face made a new vertex inside it where an input edge pierces it, across (edges lying on it crossing
    // there made it, not the face; where faces alone meet, no one); of faces of several arguments on one plane
    // there, the first's.
    let mut edges_through: HashMap<u32, Vec<usize>> = HashMap::default();
    for (e, inner) in edge_inner.iter().enumerate() {
        for &w in inner {
            edges_through.entry(w).or_default().push(e);
        }
    }
    let mut makers: HashMap<u32, Vec<usize>> = HashMap::default();
    for (f, inner) in face_inner.iter().enumerate() {
        for &w in inner {
            if !cx.pool.inputs[w as usize].is_empty() || vmap.get(w as usize).is_none() {
                continue;
            }
            let through = edges_through.get(&w).map_or(&[][..], |v| &v[..]);
            let mut across = false;
            for &e in through {
                if !cx.edge_on_surface(e, f)? {
                    across = true;
                }
            }
            if across {
                makers.entry(w).or_default().push(f);
            }
        }
    }
    let mut makers: Vec<(u32, Vec<usize>)> = makers.into_iter().collect();
    makers.sort_unstable();
    for (w, fs) in makers {
        let o = vmap[w as usize];
        for &f in &fs {
            let same_plane = |g: usize| match (&cx.faces[f].s, &cx.faces[g].s) {
                (Surf::Plane { n: n1, d: d1, .. }, Surf::Plane { n: n2, d: d2, .. }) => n1.dot(*n2).abs() > 1.0 - 1e-12 && (d1 - d2 * n1.dot(*n2).signum()).abs() <= tol,
                _ => false,
            };
            if fs.iter().any(|&g| g != f && cx.faces[g].arg < cx.faces[f].arg && same_plane(g)) {
                continue;
            }
            h.push(rec(Kind::Face, cx.faces[f].arg, cx.faces[f].index, Relation::Generated, Kind::Vertex, o));
        }
    }
    // Solids: an input solid became the result's solids that are its images (OCCT's): of a cut's object or
    // of a common's arguments, the solids holding its pieces or pieces inside it; a fuse's are deleted (the
    // fused solid is no one's image). One left as it was is kept.
    let mut solid_sources: Vec<Vec<usize>> = vec![Vec::new(); out.body.solids.len()];
    for &p in &kept {
        let o = piece_out[p];
        let v = &mut solid_sources[solid_of_face[o as usize] as usize];
        v.push(cx.faces[pieces[p].face].solid);
        v.extend(within[p].iter().copied());
    }
    for v in &mut solid_sources {
        v.sort_unstable();
        v.dedup();
    }
    for (si, s) in cx.solids.iter().enumerate() {
        let image = |k: usize| match op {
            Op::Cut => s.arg == 0 && solid_sources[k].contains(&si),
            Op::Common => solid_sources[k].contains(&si),
            Op::Fuse => false,
        };
        if out.solid_origin.contains(&Origin::Same { arg: s.arg, index: s.index }) {
            continue;
        }
        let outs: Vec<u32> = (0..out.body.solids.len()).filter(|&k| image(k)).map(|k| k as u32).collect();
        if outs.is_empty() {
            h.push(rec(Kind::Solid, s.arg, s.index, Relation::Deleted, Kind::Solid, u32::MAX));
        }
        for o in outs {
            h.push(rec(Kind::Solid, s.arg, s.index, Relation::Modified, Kind::Solid, o));
        }
    }
    out.history = h;
    // Two faces of different inputs on one cylinder meeting at a circle, their seams apart (their x directions
    // over a quarter turn apart): OCCT's unify_same_domain, merging them, left an invalid face (38b2cd00,
    // fd7ef128: a hole through a boss in a plate).
    let mut faces_of: Vec<Vec<usize>> = vec![Vec::new(); out.body.edges.len()];
    for (fi, f) in out.body.faces.iter().enumerate() {
        for u in f.loops.iter().flatten() {
            faces_of[u.edge as usize].push(fi);
        }
    }
    let source = |o: Origin| match o {
        Origin::Same { arg, .. } | Origin::Piece { arg, .. } => Some(arg),
        Origin::New => None,
    };
    for (e, fs) in faces_of.iter().enumerate() {
        if let [f, g] = fs[..]
            && matches!(out.body.edges[e].curve, Curve::Circle { .. })
            && let (Surface::Cylinder { origin: o1, axis: a1, xdir: x1, radius: r1 }, Surface::Cylinder { origin: o2, axis: a2, xdir: x2, radius: r2 }) = (out.body.faces[f].surface, out.body.faces[g].surface)
            && source(out.face_origin[f]) != source(out.face_origin[g])
            && a1.cross(a2).length() <= 1e-9
            && (r1 - r2).abs() <= tol
            && (o2 - o1).cross(a1).length() <= tol
            && x1.dot(x2) < 0.0
        {
            return Err(Decline("one cylinder in faces of two inputs, their seams apart"));
        }
    }
    Ok(out)
}

/// A map from dense indices (u32::MAX: none).
struct Dense(Vec<u32>);

impl Dense {
    fn new(n: usize) -> Dense {
        Dense(vec![u32::MAX; n])
    }

    fn insert(&mut self, k: usize, v: u32) {
        self.0[k] = v;
    }

    fn get(&self, k: usize) -> Option<u32> {
        Some(self.0[k]).filter(|&v| v != u32::MAX)
    }
}

impl std::ops::Index<usize> for Dense {
    type Output = u32;

    fn index(&self, k: usize) -> &u32 {
        &self.0[k]
    }
}

/// An angle in the gap of the arcs `covered` (start, sweep > 0): the middle of the widest gap.
fn gap(covered: &[(f64, f64)]) -> Option<f64> {
    if covered.is_empty() {
        return None;
    }
    let mut ends: Vec<(f64, f64)> = covered.iter().map(|&(s, w)| (s.rem_euclid(TAU), w)).collect();
    ends.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Walk the circle from each arc's start; the gap after the furthest reach.
    let mut best: Option<(f64, f64)> = None;
    for i in 0..ends.len() {
        let end = ends[i].0 + ends[i].1;
        let next = ends.iter().map(|e| (e.0 - end).rem_euclid(TAU)).fold(TAU, f64::min);
        let covered_inside = ends.iter().any(|e| {
            let d = (end - e.0).rem_euclid(TAU);
            d > 1e-12 && d < e.1 - 1e-12
        });
        if !covered_inside && best.is_none_or(|b| next > b.1) {
            best = Some((end, next));
        }
    }
    best.map(|(end, w)| (end + w * 0.5).rem_euclid(TAU))
}

/// The signed volume of a closed shell, its arcs as chords: positive for a solid, negative for a void.
fn shell_volume(b: &Body, faces: &[u32]) -> f64 {
    let mut v = 0.0;
    for &f in faces {
        let face = &b.faces[f as usize];
        // Each face as a fan from one of its points (closed over the shell: the chords are shared).
        let mut c: Option<DVec3> = None;
        for l in &face.loops {
            for u in l {
                let e = b.edges[u.edge as usize];
                let (p, q) = (b.vertices[e.v[0] as usize], b.vertices[e.v[1] as usize]);
                let mut poly = match e.curve {
                    Curve::Degenerate => continue,
                    Curve::Line => vec![p, q],
                    Curve::Nurbs { index, t0, t1 } => (0..=32).map(|k| b.nurbs_curves[index as usize].at(t0 + (t1 - t0) * k as f64 / 32.0)).collect(),
                    Curve::Circle { center, normal, xdir, radius } => {
                        let y = normal.cross(xdir);
                        let ang = |x: DVec3| (x - center).dot(y).atan2((x - center).dot(xdir));
                        let a0 = ang(p);
                        let sweep = if e.v[0] == e.v[1] { TAU } else { (ang(q) - a0).rem_euclid(TAU) };
                        (0..=16)
                            .map(|k| {
                                let a = a0 + sweep * k as f64 / 16.0;
                                center + (xdir * a.cos() + y * a.sin()) * radius
                            })
                            .collect()
                    }
                    Curve::Hyperbola { center, normal, xdir, major, minor, .. } => {
                        let y = normal.cross(xdir);
                        let par = |x: DVec3| ((x - center).dot(y) / minor).asinh();
                        let (t0, t1) = (par(p), par(q));
                        (0..=16)
                            .map(|k| {
                                let t = t0 + (t1 - t0) * k as f64 / 16.0;
                                center + xdir * (major * t.cosh()) + y * (minor * t.sinh())
                            })
                            .collect()
                    }
                    Curve::Ellipse { center, normal, xdir, major, minor, .. } => {
                        let y = normal.cross(xdir);
                        let par = |x: DVec3| ((x - center).dot(y) / minor).atan2((x - center).dot(xdir) / major);
                        let t0 = par(p);
                        let sweep = if e.v[0] == e.v[1] { TAU } else { (par(q) - t0).rem_euclid(TAU) };
                        (0..=16)
                            .map(|k| {
                                let t = t0 + sweep * k as f64 / 16.0;
                                center + xdir * (major * t.cos()) + y * (minor * t.sin())
                            })
                            .collect()
                    }
                };
                if u.reversed {
                    poly.reverse();
                }
                let c = *c.get_or_insert(poly[0]);
                for w in poly.windows(2) {
                    v += c.dot(w[0].cross(w[1]));
                }
            }
        }
    }
    v
}
