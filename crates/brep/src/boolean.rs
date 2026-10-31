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
use crate::curve2::{distance as distance2, inside as inside2, Map, C2};
use crate::freeform::FreeSurface;
use crate::nurbs::NurbsCurve;
use crate::pattern::Rigid;
use crate::{Body, Curve, Decline, Edge, Face, FacePair, Kind, Origin, Output, Record, Relation, SectionCurve, SectionSource, Solid, Surface, Use, GREY};
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
/// `c + a cosh t x + b sinh t y` instead (OCCT's; not closed). `par`: the parabola `c + t² / (4 a) x + t y`, `c` its
/// vertex, `a` its focal length, `x` the way it opens (OCCT's; not closed).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ell {
    c: DVec3,
    n: DVec3,
    x: DVec3,
    y: DVec3,
    a: f64,
    b: f64,
    hyp: bool,
    par: bool,
}

impl Ell {
    /// A hyperbola or parabola: not closed.
    fn open(&self) -> bool {
        self.hyp || self.par
    }

    fn at(&self, t: f64) -> DVec3 {
        if self.par {
            return self.c + self.x * (t * t / (4.0 * self.a)) + self.y * t;
        }
        if self.hyp {
            return self.c + self.x * (self.a * t.cosh()) + self.y * (self.b * t.sinh());
        }
        self.c + self.x * (self.a * t.cos()) + self.y * (self.b * t.sin())
    }

    fn deriv(&self, t: f64) -> DVec3 {
        if self.par {
            return self.x * (t / (2.0 * self.a)) + self.y;
        }
        if self.hyp {
            return self.x * (self.a * t.sinh()) + self.y * (self.b * t.cosh());
        }
        self.y * (self.b * t.cos()) - self.x * (self.a * t.sin())
    }

    /// The parameter of the point of it nearest `p` (from its angle, then Newton's method).
    fn param(&self, p: DVec3) -> f64 {
        let w = p - self.c;
        let mut t = if self.par {
            w.dot(self.y)
        } else if self.hyp {
            (w.dot(self.y) / self.b).asinh()
        } else {
            (w.dot(self.y) / self.b).atan2(w.dot(self.x) / self.a)
        };
        for _ in 0..8 {
            let (d1, q) = (self.deriv(t), self.at(t) - p);
            let d2 = if self.par {
                self.x / (2.0 * self.a)
            } else if self.hyp {
                self.at(t) - self.c
            } else {
                self.c - self.at(t)
            };
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
        if !self.open() {
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
        let second = if self.par { 1.0 / (2.0 * self.a) } else { self.a * big.cosh() + self.b * big.sinh() };
        let bulge = DVec3::splat(second * (sweep / N as f64).powi(2) / 8.0);
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
    /// No curve: an edge at a cone's apex; a piece of it from angle `a0` to `a1` about the cone's axis (as its
    /// face's parameter plane has them), NaN for an input's whole edge.
    Apex { a0: f64, a1: f64 },
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
    /// `flip`: the outward normal is minus Su × Sv, and sv is negative (its plane mirrored, as a reversed cylinder's
    /// heights are, so a piece's loops run counter-clockwise seen from outside).
    Nurbs { su: f64, sv: f64, flip: bool },
}

/// A NURBS curve the Boolean works with: an input's B-spline edge, or OCCT's section of two faces (`sec`: the
/// faces, its curves on them in their own parameters (OCCT's), the source's id for it).
struct NCurve {
    c: Arc<NurbsCurve>,
    sec: Option<([usize; 2], [Arc<NurbsCurve>; 2], u32)>,
    /// A section moved from a copied pair's (or that pair's own): the id of the section it is moved from.
    from: Option<u32>,
    /// OCCT's tolerance for a section (0 for an input's curve).
    tol: f64,
    /// Boxes holding it (`NurbsCurve::piece_boxes`).
    boxes: std::sync::OnceLock<Vec<(f64, f64, DVec3, DVec3)>>,
}

/// A free-form face's edges' curves on it: by (edge, a seam's second), the curve and its range.
type PCurves = HashMap<(usize, bool), (Arc<NurbsCurve>, f64, f64)>;


/// A section's part or an edge lying on a face, its places there to be found.
enum Pending {
    /// The two faces, the path, its range and ends.
    Section(usize, usize, Path, f64, f64, [End; 2]),
    /// The face, the edge's path, its parts (ends), its curve on a free-form face, the edge.
    InFace(usize, Path, Vec<(f64, End, f64, End)>, Option<(usize, Arc<NurbsCurve>)>, usize),
}

struct GFace {
    arg: u32,
    index: u32,
    solid: usize,
    s: Surf,
    /// Its input boundary, every loop together.
    edges: Vec<(usize, bool)>,
    /// The same in its parameter plane (a seam twice), made when first asked for ([`Ctx::bound`]).
    bound: std::sync::OnceLock<R<Vec<C2>>>,
    lo: DVec3,
    hi: DVec3,
    /// A NURBS face's surface, its edges' curves on it (by edge, a seam's second: OCCT's, in its own parameters,
    /// from the edge's start to its end), and samples for finding a point's parameters.
    ns: Option<Arc<FreeSurface>>,
    /// A free-form face's parameter box (its edges' curves on it span), (u0, u1, v0, v1).
    uvbox: [f64; 4],
    pcurves: PCurves,
    grid: std::sync::OnceLock<Vec<(DVec2, DVec3)>>,
    patches: std::sync::OnceLock<free::Patches>,
    /// A Bézier face's surface in pieces, each its control points (holding it).
    hulls: std::sync::OnceLock<Option<Vec<free::Hull>>>,
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
            (Surf::Plane { .. }, Geo::Apex { .. }) => Err(Decline("an apex on a plane")),
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
                if e.par {
                    let Surf::Plane { origin, u, v, .. } = self.s else { unreachable!() };
                    return Ok(C2::Conic { c: e.c, u: e.x / (4.0 * e.a), v: e.y, t0: *t0, sweep: *sweep, m: Map::Plane { origin, u, v }, par: true });
                }
                Ok(C2::Trig { c: self.to2(e.c), u: lin(e.x) * e.a, v: lin(e.y) * e.b, w: DVec2::ZERO, t0: *t0, sweep: *sweep })
            }
            (&Surf::Cyl { o, a, x, y, r, flip, th0, k: slope, .. }, &Geo::Ell { e, t0, sweep, .. }) if slope != 0.0 && !e.hyp => {
                // (Seen along the axis its centre is off it: mapped point by point, on the side of its middle at
                // the face's seam.)
                let mid = self.to2(e.at(t0 + 0.5 * sweep)).x;
                let m = Map::Cyl { o, a, x, y, r, flip, th0, mid };
                if e.par {
                    return Ok(C2::Conic { c: e.c, u: e.x / (4.0 * e.a), v: e.y, t0, sweep, m, par: true });
                }
                Ok(C2::Conic { c: e.c, u: e.x * e.a, v: e.y * e.b, t0, sweep, m, par: false })
            }
            (Surf::Cyl { k: slope, .. }, Geo::Ell { e, .. }) if *slope == 0.0 && e.open() => Err(Decline("a hyperbola on a cylinder")),
            (Surf::Cyl { r, th0, a, o, full, .. }, g) => {
                if let Geo::Apex { a0, a1 } = *g {
                    let y = self.to2(p0).y;
                    // (A piece of it: its own angles.)
                    if !a0.is_nan() {
                        return Ok(C2::Seg { a: DVec2::new(r * a0, y), b: DVec2::new(r * a1, y) });
                    }
                    if !*full {
                        return Err(Decline("an apex on a part of a cone"));
                    }
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
                    Geo::Apex { .. } | Geo::Nurbs { .. } => unreachable!(),
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
    /// Its own tolerance (an input's, OCCT's).
    t: f64,
}

struct GSolid {
    arg: u32,
    index: u32,
    faces: Vec<usize>,
    lo: DVec3,
    hi: DVec3,
    /// A sheet argument's faces (no solid: nothing is inside it).
    sheet: bool,
}

/// Every vertex, input and new, by position.
struct Pool {
    p: Vec<DVec3>,
    /// The input vertices (arg, index) that are this one.
    inputs: Vec<Vec<(u32, u32)>>,
    grid: HashMap<(i64, i64, i64), Vec<u32>>,
    cell: f64,
    tol: f64,
    /// Per vertex its own tolerance (an input's, OCCT's; 0 for a new one), and those over half the Boolean's
    /// (tolerant ones: within the sum of the two's tolerances another is the same vertex, OCCT's rule).
    t: Vec<f64>,
    /// Every vertex again in cells 64 times as wide (the tolerant ones are found there), and those whose tolerance
    /// is past a quarter of such a cell (checked one by one).
    coarse: HashMap<(i64, i64, i64), Vec<u32>>,
    loose: Vec<u32>,
    /// The largest of the vertices' own tolerances.
    tmax: f64,
}

impl Pool {
    fn key(&self, p: DVec3) -> (i64, i64, i64) {
        ((p.x / self.cell).floor() as i64, (p.y / self.cell).floor() as i64, (p.z / self.cell).floor() as i64)
    }

    /// The vertex within `tol` of `p`, if any.
    fn find(&self, p: DVec3) -> R<Option<u32>> {
        self.find_tol(p, 0.0)
    }

    /// The vertex `p` (its own tolerance `tp`) is: one within the Boolean's tolerance, or within the sum of the
    /// two's own where that's more (OCCT's rule for tolerant vertices); none in the grey band past that.
    fn find_tol(&self, p: DVec3, tp: f64) -> R<Option<u32>> {
        let half = 0.5 * self.tol;
        let wide = 64.0 * self.cell;
        let mut found: Option<u32> = None;
        let mut take = |i: u32| -> R<()> {
            let ti = self.t[i as usize];
            let reach = self.tol.max(tp + ti);
            // (Two inputs' vertices: OCCT's rule is the sum of their tolerances, nothing in between but rounding. A
            // new vertex's tolerance is OCCT's own choice, at most ~1e-5: past that beside a tolerant one; else the
            // grey band.)
            let band = if tp > 0.0 && ti > 0.0 {
                reach * (1.0 + 1e-9)
            } else if tp + ti > self.tol {
                reach + 1e-5
            } else {
                GREY * reach
            };
            let d = (self.p[i as usize] - p).length();
            if d <= reach {
                if found.is_some_and(|f| f != i) {
                    return Err(Decline("a point near two vertices"));
                }
                found = Some(i);
            } else if d <= band {
                return Err(Decline("two vertices nearly coincide"));
            }
            Ok(())
        };
        if tp > 0.25 * wide {
            for i in 0..self.p.len() as u32 {
                take(i)?;
            }
            return Ok(found);
        }
        if tp <= half {
            let k = self.key(p);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let Some(list) = self.grid.get(&(k.0 + dx, k.1 + dy, k.2 + dz)) else { continue };
                        for &i in list {
                            if self.t[i as usize] <= half {
                                take(i)?;
                            }
                        }
                    }
                }
            }
        }
        // (Tolerant ones, or every one for a tolerant point: the wide cells round it.)
        let k = ((p.x / wide).floor() as i64, (p.y / wide).floor() as i64, (p.z / wide).floor() as i64);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(list) = self.coarse.get(&(k.0 + dx, k.1 + dy, k.2 + dz)) else { continue };
                    for &i in list {
                        let ti = self.t[i as usize];
                        if (tp > half || ti > half) && ti <= 0.25 * wide {
                            take(i)?;
                        }
                    }
                }
            }
        }
        for &i in &self.loose {
            take(i)?;
        }
        Ok(found)
    }

    fn push(&mut self, p: DVec3) -> u32 {
        self.push_tol(p, 0.0)
    }

    /// Vertex `w`'s tolerance at least `t` (another merged into it, as OCCT's merged vertex covers both).
    fn grow(&mut self, w: u32, t: f64) {
        let quarter = 16.0 * self.cell;
        self.tmax = self.tmax.max(t);
        if t > self.t[w as usize] {
            if t > quarter && self.t[w as usize] <= quarter {
                self.loose.push(w);
            }
            self.t[w as usize] = t;
        }
    }

    fn push_tol(&mut self, p: DVec3, t: f64) -> u32 {
        let i = self.p.len() as u32;
        self.p.push(p);
        self.t.push(t);
        self.tmax = self.tmax.max(t);
        let wide = 64.0 * self.cell;
        self.coarse.entry(((p.x / wide).floor() as i64, (p.y / wide).floor() as i64, (p.z / wide).floor() as i64)).or_default().push(i);
        if t > 0.25 * wide {
            self.loose.push(i);
        }
        self.inputs.push(Vec::new());
        let k = self.key(p);
        self.grid.entry(k).or_default().push(i);
        i
    }

    /// `add`, the point with its own tolerance `t` (a section's): one within the sum of the two's with another is
    /// that one, its tolerance grown to cover both.
    fn add_tol(&mut self, p: DVec3, t: f64) -> R<u32> {
        if let Some(i) = self.find_tol(p, t)? {
            if t > 0.0 {
                let d = (self.p[i as usize] - p).length();
                self.grow(i, t + d);
            }
            return Ok(i);
        }
        Ok(self.push_tol(p, t))
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
    /// An edge ends inside it (one that touches it, as a box's edge on another's face, without cutting it).
    dangling: bool,
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

/// A rotation the whole arrangement is the same under: every tool in a family of copies taken onto another member,
/// every other argument's solid onto itself. Its axis (a point, the direction, a frame across it), a whole turn in
/// `n` steps, and by each power j (1..n) of its inverse, where each solid and face goes (faces: `usize::MAX` where
/// none).
struct Sym {
    o: DVec3,
    a: DVec3,
    x: DVec3,
    y: DVec3,
    n: usize,
    solid_back: Vec<Vec<usize>>,
    face_back: Vec<Vec<usize>>,
    arg_back: Vec<Vec<usize>>,
}

impl Sym {
    /// The rotation by `j` steps.
    fn turn(&self, j: f64) -> Rigid {
        let m = glam::DMat3::from_axis_angle(self.a, TAU * j / self.n as f64);
        Rigid { m, t: self.o - m * self.o }
    }

    /// The steps `p` is from the first period (its angle about the axis from `x`, in steps, the periods starting
    /// at an odd fraction of a step: a pattern's features lie on whole and half steps), unless it is on the axis or
    /// at a period's start (within rounding).
    fn period(&self, p: DVec3, size: f64) -> Option<usize> {
        let w = p - self.o;
        let (u, v) = (w.dot(self.x), w.dot(self.y));
        if u.hypot(v) <= 1e-6 * size {
            return None;
        }
        let k = v.atan2(u).rem_euclid(TAU) * self.n as f64 / TAU + 0.381_966;
        let j = k.floor();
        (k - j > 1e-6 && j + 1.0 - k > 1e-6).then_some(j as usize % self.n)
    }
}

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
    /// For each of a face's curves, the section it is (its index in `sections`), or `usize::MAX`.
    seg_section: Vec<Vec<usize>>,
    coplanar: Vec<(usize, usize)>,
    /// Where two faces cross (the curve, the faces).
    sections: Vec<(Seg, usize, usize)>,
    /// Sections' parts and other arguments' edges lying on faces, as the faces are paired: their places in the
    /// faces found later, on all threads, then their edges made in this order.
    pending: Vec<Pending>,
    /// Each argument's family of rigid copies among the tools (`pattern::families`) and the motion from the
    /// family's first onto it; the families' members (argument, motion).
    copies: Vec<Option<(usize, Rigid)>>,
    families: Vec<Vec<(usize, Rigid)>>,
    /// The arguments' size (largest coordinate): how far a motion's difference is measured.
    size: f64,
    /// For a face of another argument and a family's tool face (by index): the member its sections are made
    /// with (moved to the others), by its place in the family.
    canonical: HashMap<(usize, usize, u32), usize>,
    /// OCCT's sections of pairs copies are moved from (by the pair, its roles as asked).
    canon_secs: HashMap<FacePair, Vec<SectionCurve>>,
    /// Pairs of faces found clearly apart or not (`free::apart`), by the two faces as paired.
    apart: HashMap<(usize, usize), bool>,
    /// Whether an edge lies on a face, where that takes points along it (`on_free_surface`), found ahead.
    on_surface: HashMap<(usize, usize), R<bool>>,
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
        // (A tolerant vertex: on the plane within its own tolerance and the face's (OCCT's rule; the face's at most
        // ~1e-5 here), apart past that.)
        let tv = self.pool.t[v as usize];
        let (reach, band) = if tv > 0.5 * self.tol { (tv + 0.5 * self.tol, tv + 0.5 * self.tol + 1e-5) } else { (self.tol, GREY * self.tol) };
        let s = if x.abs() <= reach {
            0
        } else if x.abs() <= band {
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
        if let (Geo::Apex { a0, .. }, Surf::Cyl { full: false, .. }) = (s.g, &g.s)
            && a0.is_nan()
        {
            // A part of a cone at its apex: at the apex's height, from the angle of the face's ruling that comes to
            // the apex to that of the one that leaves it (its own way along the edge).
            let e = edge.ok_or(Decline("an apex on a part of a cone"))?;
            let es = &g.edges;
            let k = es.iter().position(|x| x.0 == e).ok_or(Decline("an apex on a part of a cone"))?;
            let w = s.v[0];
            let ends = |&(e, rev): &(usize, bool)| if rev { [self.edges[e].v[1], self.edges[e].v[0]] } else { self.edges[e].v };
            // (A ruling as a line, or as OCCT's section of it (a B-spline): its angle at its middle.)
            let rulings = |at: usize| es.iter().filter(|x| !matches!(self.edges[x.0].g, Geo::Apex { .. }) && ends(x)[at] == w).map(|x| self.seg_at(&self.edge_seg(x.0), 0.5)).collect::<Vec<DVec3>>();
            let (into, out) = (rulings(1), rulings(0));
            let (&[from], &[to]) = (into.as_slice(), out.as_slice()) else { return Err(Decline("an apex on a part of a cone")) };
            let y = g.to2(self.p(s.v[0])).y;
            let (a, b) = (DVec2::new(g.to2(from).x, y), DVec2::new(g.to2(to).x, y));
            return Ok(if es[k].1 { C2::Seg { a: b, b: a } } else { C2::Seg { a, b } });
        }
        if let Surf::Nurbs { su, sv, .. } = g.s {
            if let Geo::Nurbs { id, t0, t1 } = s.g
                && let Some((faces, on, _)) = &self.ncurves[id as usize].sec
            {
                let k = if faces[0] == f { 0 } else if faces[1] == f { 1 } else { return Err(Decline("a section on a third face")) };
                return Ok(C2::Nurbs { c: free::scaled(&on[k], su, sv), t0, t1 });
            }
            // (Another argument's straight, round, elliptic or B-spline edge lying on it: the one whose curve on it was
            // projected.)
            let e = edge.or_else(|| self.projected_edge(f, s)).ok_or(Decline("a curve on a free-form face not its own"))?;
            let (pc, pt0, pt1) = g.pcurves.get(&(e, copy == 1)).ok_or(Decline("an edge without its curve on a free-form face"))?;
            let c = free::scaled(pc, su, sv);
            let (a, b) = match s.g {
                // (Its own curve's piece; another's coincident with it goes by its ends, below.)
                Geo::Nurbs { id, t0, t1 } if matches!(self.edges[e].g, Geo::Nurbs { id: own, .. } if own == id) => (t0, t1),
                // (A degenerate edge (the surface closing at a point), or a closed one whole: its whole curve on the
                // face.)
                Geo::Apex { .. } => (*pt0, *pt1),
                _ if s.v[0] == s.v[1] => (*pt0, *pt1),
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

    /// The edge of another argument, lying on free-form face `f`, whose curve on it was projected (its pieces in `f`
    /// keep to it) that holds segment `s`.
    fn projected_edge(&self, f: usize, s: &Seg) -> Option<usize> {
        let g = &self.faces[f];
        g.pcurves.keys().filter(|k| !k.1 && self.edges[k.0].arg != g.arg).map(|k| k.0).find(|&e| match (self.edges[e].g, s.g) {
            (Geo::Arc { k: k1, .. }, Geo::Arc { k: k2, .. }) => k1 == k2,
            (Geo::Ell { e: e1, .. }, Geo::Ell { e: e2, .. }) => e1 == e2,
            (Geo::Nurbs { id: i1, .. }, Geo::Nurbs { id: i2, .. }) => i1 == i2,
            (Geo::Line, Geo::Line) => {
                let [a, b] = self.edges[e].v;
                let (a, b) = (self.p(a), self.p(b));
                let d = (b - a).normalize();
                s.v.iter().all(|&w| (self.p(w) - a).cross(d).length() <= self.tol)
            }
            _ => false,
        })
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
            Geo::Apex { .. } => self.p(s.v[0]),
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
                let d = if e.open() { tp - t0 } else { (tp - t0).rem_euclid(TAU) };
                (d / sweep, (e.at(tp) - p).length())
            }
            Geo::Apex { .. } => (0.0, (p - self.p(s.v[0])).length()),
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
            Geo::Apex { .. } => 0.0,
            Geo::Nurbs { id, t0, t1 } => free::length(&self.ncurves[id as usize].c, t0, t1),
        }
    }

    fn edge_seg(&self, e: usize) -> Seg {
        Seg { v: self.edges[e].v, g: self.edges[e].g }
    }

    /// The intervals of `path` (parameters `t0..t1`) inside face `f` (with its boundary when `closed`).
    /// `ends`: the path's own end vertices, if it has them.
    fn intervals(&self, f: usize, path: &Path, t0: f64, t1: f64, ends: [End; 2], closed: bool) -> R<Vec<Ival>> {
        self.intervals_on(f, path, (t0, t1), ends, closed, None)
    }

    /// The same, `on_free` the curve on a free-form face `path` lies on (projected there).
    fn intervals_on(&self, f: usize, path: &Path, (t0, t1): (f64, f64), ends: [End; 2], closed: bool, on_free: Option<&(usize, Arc<NurbsCurve>)>) -> R<Vec<Ival>> {
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
            (&Path::Ellipse(e), &Surf::Cyl { th0, full: false, .. }) if e.open() => {
                self.hyp_at_angle(f, &e, th0).filter(|&t| t > t0 && t < t1).map(|t| (t, e.length(t0, t) > tol && e.length(t, t1) > tol))
            }
            (&Path::Ellipse(e), &Surf::Cyl { th0, full: false, .. }) => {
                let t = t0 + (self.ell_at_angle(f, &e, th0) - t0).rem_euclid(TAU);
                Some((t, e.length(t0, t) > tol && e.length(t, t1) > tol))
            }
            _ => None,
        };
        if let Some((t, true)) = gap {
            let mut out = self.intervals_on(f, path, (t0, t), [ends[0], End::Open], closed, on_free)?;
            for iv in self.intervals_on(f, path, (t, t1), [End::Open, ends[1]], closed, on_free)? {
                match out.last_mut() {
                    Some(last) if last[1].0 == iv[0].0 && last[1].1 == End::Open => last[1] = iv[1],
                    _ => out.push(iv),
                }
            }
            return Ok(out);
        }
        let geo = path.geo(t0, t1);
        let c2 = if let (Some((g, c)), Surf::Nurbs { su, sv, .. }) = (on_free, &self.faces[f].s)
            && *g == f
        {
            C2::Nurbs { c: free::scaled(c, *su, *sv), t0, t1 }
        } else if let Path::Nurbs { .. } = path {
            // (A NURBS curve's part needs no vertices.)
            self.c2_of(f, &Seg { v: [u32::MAX; 2], g: geo }, 0, None)?
        } else {
            self.faces[f].curve2(path.at(t0), path.at(t1), &geo, 0)?
        };
        // Where it meets the face's boundary.
        let mut cuts: Vec<(f64, End)> = vec![(t0, ends[0]), (t1, ends[1])];
        let bound = self.bound(f)?.to_vec();
        // (OCCT's curve ending on the boundary, as it does at the face's parameter bounds: a point there.)
        if let Path::Nurbs { .. } = path {
            for (k, u) in [(0, 0.0), (1, 1.0)] {
                if ends[k] == End::Open && distance2(c2.at(u), &bound) <= tol {
                    cuts[k].1 = End::Point(path.at(cuts[k].0));
                }
            }
        }
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
        // Clear of the boundary: on a free-form face OCCT's section's curve on it against its edges' (OCCT's too), as
        // OCCT tells it: beyond the model's tolerance.
        let ctol = if let (Path::Nurbs { .. }, Surf::Nurbs { .. }) = (path, &self.faces[f].s) { self.tol } else { tol };
        let mut out: Vec<Ival> = Vec::new();
        for k in 0..cuts.len() - 1 {
            let (a, b) = (cuts[k], cuts[k + 1]);
            if apart(a.0, b.0) <= tol {
                continue;
            }
            // (OCCT's curve clear of the boundary but touching it between its crossings: OCCT may put a vertex
            // there.)
            let mid = c2.at(((a.0 + b.0) * 0.5 - t0) / (t1 - t0));
            let dist = distance2(mid, &bound);
            if let (Path::Nurbs { .. }, true) = (path, dist > GREY * tol) {
                // (The distance's local minima along it, each narrowed down.)
                let margin = 3.0 * GREY * tol;
                let d = |t: f64| distance2(c2.at((t - t0) / (t1 - t0)), &bound);
                let ts: Vec<f64> = (0..=32).map(|k| a.0 + (b.0 - a.0) * k as f64 / 32.0).collect();
                let ds: Vec<f64> = ts.iter().map(|&t| d(t)).collect();
                for k in 1..32 {
                    if ds[k] > ds[k - 1] || ds[k] > ds[k + 1] {
                        continue;
                    }
                    // (Its part there clearly farther than the band all over: the box it lies in, about the
                    // point here, reaches less far than it is from the boundary.)
                    let u = |t: f64| (t - t0) / (t1 - t0);
                    if let Some((lo, hi)) = c2.part_box(u(ts[k - 1]), u(ts[k + 1])) {
                        let q = c2.at(u(ts[k]));
                        if ds[k] - (q - lo).abs().max((hi - q).abs()).length() > 2.0 * GREY * tol {
                            continue;
                        }
                    }
                    let (mut lo, mut hi) = (ts[k - 1], ts[k + 1]);
                    for _ in 0..50 {
                        let (m1, m2) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
                        if d(m1) < d(m2) {
                            hi = m2;
                        } else {
                            lo = m1;
                        }
                    }
                    let t = 0.5 * (lo + hi);
                    if apart(a.0, t) > margin && apart(t, b.0) > margin && d(t) <= GREY * tol {
                        return Err(Decline("a section touching a face's boundary"));
                    }
                }
            }
            let inside = if dist <= tol {
                closed
            } else if dist <= GREY * tol {
                // Judged where it is farthest from the boundary (a shallow helix rising from a rod's end, an arc
                // nearly tangent to a face's edge), if that is clearly off it; else ambiguous.
                let far = (1..8).map(|k| c2.at(((a.0 + (b.0 - a.0) * k as f64 / 8.0) - t0) / (t1 - t0))).map(|q| (distance2(q, &bound), q)).max_by(|x, y| x.0.total_cmp(&y.0));
                // (OCCT's curve is within its tolerance of the faces' section: two of them off is clear.)
                let clear = if let Path::Nurbs { .. } = path { 2.0 } else { 3.0 };
                // (Within the grey band all along: on the boundary, as within the tolerance (OCCT's section within
                // its own tolerance of the edge: a common block).)
                match far {
                    Some((d, q)) if d > clear * ctol => inside2(q, &bound),
                    _ => closed,
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
            && e.open()
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
        let open = matches!(path, Path::Ellipse(e) if e.open());
        for i in 0..cuts.len() {
            let (a, ea) = cuts[i];
            let (b, eb) = if i + 1 < cuts.len() { cuts[i + 1] } else { (t1, if open { End::Open } else { cuts[0].1 }) };
            self.section_part(fs, ft, path.clone(), a, b, [ea, eb])?;
        }
        Ok(())
    }

    fn section_part(&mut self, fs: usize, ft: usize, path: Path, t0: f64, t1: f64, ends: [End; 2]) -> R<()> {
        self.pending.push(Pending::Section(fs, ft, path, t0, t1, ends));
        Ok(())
    }

    /// Where both faces' intervals of a section overlap: its edges, in both faces.
    fn section_from(&mut self, fs: usize, ft: usize, path: Path, ia: Vec<Ival>, ib: Vec<Ival>) -> R<()> {
        let tol = self.tol;
        // Whether the length along it between two parameters is within the tolerance (a B-spline's, the length
        // either way: clearly not where the chord is longer, clearly so where its control polygon there is
        // shorter).
        let near = |x: f64, y: f64| {
            if let Path::Nurbs { c, .. } = &path {
                if x == y {
                    return true;
                }
                if (c.at(y) - c.at(x)).length() > tol * (1.0 + 1e-6) {
                    return false;
                }
                if c.part_polygon(x, y).is_some_and(|l| l < tol * (1.0 - 1e-6)) {
                    return true;
                }
            }
            path.length(x, y) <= tol
        };
        // (Its points with its own tolerance: an OCCT section's.)
        let ptol = if let Path::Nurbs { id, .. } = &path { self.ncurves[*id as usize].tol } else { 0.0 };
        // The end of a section: the tighter of the two faces' ends, made a vertex.
        let pick = |cx: &mut Ctx, x: (f64, End), y: (f64, End), lo: bool| -> R<Option<u32>> {
            let (later, earlier) = if x.0 > y.0 { (x, y) } else { (y, x) };
            let end = if !near(earlier.0, later.0) {
                if lo { later.1 } else { earlier.1 }
            } else if x.1.rank() >= y.1.rank() {
                x.1
            } else {
                y.1
            };
            let v = match end {
                End::Open => return Ok(None),
                End::Vertex(v) => v,
                End::Point(p) => cx.pool.add_tol(p, ptol)?,
            };
            // Both faces end there: the same vertex.
            if near(earlier.0, later.0) {
                for o in [x.1, y.1] {
                    let w = match o {
                        End::Vertex(w) => w,
                        End::Point(p) => cx.pool.add_tol(p, ptol)?,
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
                if near(lo, hi) {
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
                self.seg_section[fs].push(self.sections.len());
                self.seg_section[ft].push(self.sections.len());
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
            Geo::Ell { e, t0, sweep, .. } if !e.hyp && (matches!(self.faces[f].s, Surf::Plane { .. }) || !e.par && matches!(self.faces[f].s, Surf::Cyl { .. } | Surf::Nurbs { .. })) => (Path::Ellipse(e), t0, t0 + sweep),
            Geo::Ell { .. } => return Err(Decline("an argument's ellipse")),
            Geo::Nurbs { id, t0, t1 } => (Path::Nurbs { id, c: self.ncurves[id as usize].c.clone() }, t0, t1),
            Geo::Apex { .. } => return Ok(()),
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
        let mut on_free = None;
        // On a free-form face: its curve there, by projection (as the face's own edges' curves are used).
        if matches!(self.faces[f].s, Surf::Nurbs { .. }) {
            let at = |t: f64| match &path {
                Path::Line { p, d } => (*p + *d * t, *d),
                Path::Circle(k) => (k.at(t), (k.y * t.cos() - k.x * t.sin()) * k.r),
                Path::Ellipse(e) => (e.at(t), e.deriv(t)),
                Path::Nurbs { c, .. } => {
                    let d = c.derivs(t);
                    (d[0], d[1])
                }
            };
            let Some(c) = free::projected(&self.faces[f], &at, t0, t1, self.tol) else { return Err(Decline("an edge on another argument's free-form face")) };
            self.faces[f].pcurves.insert((e, false), (c.clone(), t0, t1));
            on_free = Some((f, c));
        }
        self.pending.push(Pending::InFace(f, path, parts, on_free, e));
        Ok(())
    }

    /// An edge's pieces in face `f` (where `path` lies inside it): its edges there.
    fn in_face_from(&mut self, f: usize, path: &Path, ivs: Vec<Ival>) -> R<()> {
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
            self.seg_section[f].push(usize::MAX);
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
            (Path::Ellipse(e), Surf::Cyl { th0, full: true, .. }) if e.open() => self.hyp_at_angle(f, &e, *th0).map(|t| e.at(t)),
            (Path::Ellipse(e), Surf::Cyl { th0, full: true, .. }) => Some(e.at(self.ell_at_angle(f, &e, *th0))),
            _ => None,
        }
    }

    /// The parameter of ellipse `e` (on face `f`'s cylinder) where the cylinder's angle is `th`.
    fn ell_at_angle(&self, f: usize, e: &Ell, th: f64) -> f64 {
        let Surf::Cyl { o, a, x, y, k, .. } = self.faces[f].s else { unreachable!() };
        if k != 0.0 {
            // A cone's (its centre off the axis): where it crosses the half-plane of the axis at angle th,
            // (P - o) · (a × d) = 0 with (P - o) · d > 0.
            let d = x * th.cos() + y * th.sin();
            let m = a.cross(d);
            let (p, q, g) = ((e.x * e.a).dot(m), (e.y * e.b).dot(m), (e.c - o).dot(m));
            let rr = (p * p + q * q).sqrt();
            let base = q.atan2(p);
            let dev = (-g / rr).clamp(-1.0, 1.0).acos();
            return [base + dev, base - dev].into_iter().max_by(|&s, &t| (e.at(s) - o).dot(d).total_cmp(&(e.at(t) - o).dot(d))).unwrap_or(base);
        }
        // Seen along the axis it is the cylinder's circle: its major axis' direction at t = 0, its minor's at π/2.
        let d = x * th.cos() + y * th.sin();
        let xp = (e.x - a * e.x.dot(a)).normalize();
        d.dot(e.y).atan2(d.dot(xp))
    }

    /// The parameter of hyperbola `e` (on face `f`'s cone) where the cone's angle is `th`, if it gets there.
    fn hyp_at_angle(&self, f: usize, e: &Ell, th: f64) -> Option<f64> {
        let Surf::Cyl { o, a, x, y, .. } = self.faces[f].s else { unreachable!() };
        if e.par {
            // A parabola's: (P - o) · (a × d) = 0, quadratic in t, with (P - o) · d > 0.
            let d = x * th.cos() + y * th.sin();
            let m = a.cross(d);
            let (qa, qb, qc) = (e.x.dot(m) / (4.0 * e.a), e.y.dot(m), (e.c - o).dot(m));
            let roots: Vec<f64> = if qa.abs() < 1e-300 {
                if qb.abs() < 1e-300 { Vec::new() } else { vec![-qc / qb] }
            } else {
                let disc = qb * qb - 4.0 * qa * qc;
                if disc < 0.0 { Vec::new() } else { vec![(-qb - disc.sqrt()) / (2.0 * qa), (-qb + disc.sqrt()) / (2.0 * qa)] }
            };
            return roots.into_iter().filter(|&t| (e.at(t) - o).dot(d) > 0.0).min_by(|s, t| s.abs().total_cmp(&t.abs()));
        }
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
            (_, Geo::Apex { .. }) => Ok(false),
            (Surf::Nurbs { .. }, _) | (_, Geo::Nurbs { .. } | Geo::Ell { .. }) => match self.on_surface.get(&(e, f)) {
                Some(&x) => x,
                None => self.on_free_surface(e, f),
            },
        }
    }

    /// Whether edge `e` takes points along it to tell whether it lies on face `f`'s surface.
    fn by_points(&self, e: usize, f: usize) -> bool {
        !matches!(self.edges[e].g, Geo::Apex { .. }) && (matches!(self.faces[f].s, Surf::Nurbs { .. }) || matches!(self.edges[e].g, Geo::Nurbs { .. } | Geo::Ell { .. }))
    }

    /// [`Self::edge_on_surface`] by points along the edge.
    fn on_free_surface(&self, e: usize, f: usize) -> R<bool> {
        let tol = self.tol;
        // Points along it: all on the surface, it lies on it (not yet taken); within the grey band
        // without being clearly off, ambiguous.
        let s = self.edge_seg(e);
        let (mut on, mut near) = (0, 0);
        // (The middle first: one point clearly off, it isn't on it.)
        for k in [3, 0, 6, 1, 2, 4, 5] {
            let p = self.seg_at(&s, k as f64 / 6.0);
            if free::clearly_off(&self.faces[f], p, GREY * tol) {
                return Ok(false);
            }
            let d = free::surface_distance(&self.faces[f], p);
            if d <= tol {
                on += 1;
            } else if d <= GREY * tol {
                near += 1;
            } else {
                return Ok(false);
            }
        }
        if on == 7 {
            // (On a plane, cylinder or cone: its pieces there go into the face, as a straight or round
            // edge's.)
            if let (Geo::Nurbs { .. }, Surf::Plane { .. } | Surf::Cyl { .. }) = (self.edges[e].g, &self.faces[f].s) {
                return Ok(true);
            }
            if let (Geo::Ell { e: el, .. }, s @ (Surf::Plane { .. } | Surf::Cyl { .. })) = (self.edges[e].g, &self.faces[f].s)
                && !el.hyp
                && (!el.par || matches!(s, Surf::Plane { .. }))
            {
                return Ok(true);
            }
            // (An ellipse on a free-form face: its curve there projected, edge_in_face.)
            if let (Geo::Ell { e: el, .. }, Surf::Nurbs { .. }) = (self.edges[e].g, &self.faces[f].s)
                && !el.open()
                && self.src.is_some()
            {
                return Ok(true);
            }
            if let Geo::Ell { .. } = self.edges[e].g {
                return Err(Decline("an argument's ellipse on another's face"));
            }
            // (A straight, round or B-spline edge on a free-form face: its curve there projected,
            // edge_in_face.)
            if matches!(self.edges[e].g, Geo::Line | Geo::Arc { .. } | Geo::Nurbs { .. }) && self.src.is_some() {
                return Ok(true);
            }
            return Err(Decline("an edge on another argument's free-form face"));
        }
        if near > 0 && on + near == 7 {
            return Err(Decline("an edge nearly on another argument's face"));
        }
        Ok(false)
    }

    /// Whether faces `fs` and `ft` are clearly apart (`free::apart`; found once).
    fn faces_apart(&mut self, fs: usize, ft: usize) -> bool {
        if let Some(&x) = self.apart.get(&(fs, ft)) {
            return x;
        }
        let x = free::apart(&self.faces[fs], &self.faces[ft], GREY * self.tol.max(5e-6));
        self.apart.insert((fs, ft), x);
        x
    }

    /// Faces `fs` and `ft` (one free-form) meet where OCCT's section says (its curves, its curves on them).
    /// Points of face `f` on its own surface: its vertices, its edges' middles and a grid over its parameters.
    fn face_samples(&self, f: usize) -> Vec<DVec3> {
        let g = &self.faces[f];
        let mut out: Vec<DVec3> = g.edges.iter().flat_map(|&(e, _)| [self.p(self.edges[e].v[0]), self.seg_at(&self.edge_seg(e), 0.5)]).collect();
        const N: usize = 4;
        match g.s {
            Surf::Nurbs { .. } => {
                if let Some(ns) = &g.ns {
                    let [u0, u1, v0, v1] = g.uvbox;
                    for i in 1..N {
                        for j in 1..N {
                            out.push(ns.at(u0 + (u1 - u0) * i as f64 / N as f64, v0 + (v1 - v0) * j as f64 / N as f64));
                        }
                    }
                }
            }
            Surf::Cyl { o, a, x, y, r0, k, .. } => {
                let (mut lo, mut hi, mut h0, mut h1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
                for &(e, _) in &g.edges {
                    for w in self.edges[e].v {
                        let p = self.p(w) - o;
                        let h = p.dot(a);
                        let r = p - a * h;
                        if r.length() > 1e-12 {
                            let th = r.dot(y).atan2(r.dot(x));
                            (lo, hi) = (lo.min(th), hi.max(th));
                        }
                        (h0, h1) = (h0.min(h), h1.max(h));
                    }
                }
                if lo.is_finite() {
                    for i in 1..N {
                        for j in 1..N {
                            let (th, h) = (lo + (hi - lo) * i as f64 / N as f64, h0 + (h1 - h0) * j as f64 / N as f64);
                            out.push(o + a * h + (x * th.cos() + y * th.sin()) * (r0 + k * h));
                        }
                    }
                }
            }
            Surf::Plane { .. } => {}
        }
        out
    }

    /// Whether faces `fs` and `ft` lie on one surface where they overlap: every point of each (its samples within
    /// the other's box) on the other's surface, and some.
    fn same_surface(&self, fs: usize, ft: usize) -> bool {
        let tol = self.tol;
        let mut n = 0;
        for (f, g) in [(fs, ft), (ft, fs)] {
            let (lo, hi) = (self.faces[g].lo, self.faces[g].hi);
            for p in self.face_samples(f) {
                if !boxes_meet(p, p, lo, hi, tol) {
                    continue;
                }
                if free::surface_distance(&self.faces[g], p) > tol {
                    return false;
                }
                n += 1;
            }
        }
        n >= 8
    }

    fn nurbs_pair(&mut self, fs: usize, ft: usize) -> R<()> {
        let Some(src) = self.src else { return Err(Decline("a curved face")) };
        // (Clearly apart, their control points tell: no section, without asking.)
        if self.faces_apart(fs, ft) {
            return Ok(());
        }
        // (One surface (a cone and a B-spline of it, two copies of a patch): coincident, like coplanar faces.)
        if self.same_surface(fs, ft) {
            self.coplanar.push((fs, ft));
            return Ok(());
        }
        let (a, b) = ((self.faces[fs].arg, self.faces[fs].index), (self.faces[ft].arg, self.faces[ft].index));
        let moved = self.moved_pair(fs, ft);
        let mut froms: Vec<Option<u32>> = Vec::new();
        let mut secs = match moved {
            Some(m) => {
                let x = self.moved_sections(src, a, b, &m)?;
                froms = x.1;
                x.0
            }
            None => src.sections(a, b)?.into_iter().filter(|c| c.t1 > c.t0).collect(),
        };
        // (Moved onto a face of another argument: only where they stay inside it (and the curves they were moved
        // from); else from another member (up to the third), else made for these.)
        let other = [fs, ft].into_iter().find(|&f| self.copies.get(self.faces[f].arg as usize).copied().flatten().is_none());
        if let (Some((canon, mut r)), Some(f)) = (moved, other)
            && canon != (a, b)
        {
            let tool = if f == fs { ft } else { fs };
            let key = (f, self.copies[self.faces[tool].arg as usize].map_or(0, |x| x.0), self.faces[tool].index);
            loop {
                let (moved_clear, from_clear) = self.clear_inside(f, &secs, &r)?;
                if moved_clear && from_clear {
                    break;
                }
                if !moved_clear {
                    // (These reach the face's boundary: made for themselves.)
                    secs = src.sections(a, b)?.into_iter().filter(|c| c.t1 > c.t0).collect();
                    froms.clear();
                    break;
                }
                let k = self.canonical.get(&key).copied().unwrap_or(0) + 1;
                self.canonical.insert(key, k);
                match self.moved_pair(fs, ft) {
                    Some(m) if k < 3 => {
                        r = m.1;
                        (secs, froms) = self.moved_sections(src, a, b, &m)?;
                    }
                    _ => {
                        secs = src.sections(a, b)?.into_iter().filter(|c| c.t1 > c.t0).collect();
                        froms.clear();
                        break;
                    }
                }
            }
        }
        froms.resize(secs.len(), None);
        // (An end of one where another of them ends: OCCT's curves of one loop in parts; a vertex there.)
        let ends: Vec<[DVec3; 2]> = secs.iter().map(|c| [c.curve.at(c.t0), c.curve.at(c.t1)]).collect();
        let joined = |i: usize, k: usize| ends.iter().enumerate().any(|(j, e)| j != i && e.iter().any(|q| (*q - ends[i][k]).length() <= self.tol));
        let joins: Vec<[bool; 2]> = (0..secs.len()).map(|i| [joined(i, 0), joined(i, 1)]).collect();
        for ((c, join), from) in secs.into_iter().zip(joins).zip(froms) {
            let id = self.ncurves.len() as u32;
            let cc = Arc::new(c.curve);
            let [on0, on1] = c.on;
            self.ncurves.push(NCurve { c: cc.clone(), sec: Some(([fs, ft], [Arc::new(on0), Arc::new(on1)], c.id)), from, tol: c.tol, boxes: Default::default() });
            let cuts = free::seam_cuts(self, [fs, ft], &cc, c.t0, c.t1);
            self.section_nurbs(fs, ft, Path::Nurbs { id, c: cc }, (c.t0, c.t1), &cuts, join)?;
        }
        Ok(())
    }

    /// OCCT's sections of faces `a` and `b` as pair `m.0`'s (made once) moved by `m.1`: the curves moved here, their
    /// ids from the source.
    fn moved_sections(&mut self, src: &dyn SectionSource, a: (u32, u32), b: (u32, u32), m: &crate::MovedPair) -> R<(Vec<SectionCurve>, Vec<Option<u32>>)> {
        let (canon, r) = *m;
        if let std::collections::hash_map::Entry::Vacant(e) = self.canon_secs.entry(canon) {
            e.insert(src.sections(canon.0, canon.1)?.into_iter().filter(|c| c.t1 > c.t0).collect());
        }
        let made = &self.canon_secs[&canon];
        let from = made.iter().map(|c| Some(c.id)).collect();
        if made.is_empty() || canon == (a, b) {
            return Ok((made.clone(), from));
        }
        let ids = src.moved(&made.iter().map(|c| c.id).collect::<Vec<_>>(), a, b, m)?;
        let moved = made
            .iter()
            .zip(ids)
            .map(|(c, id)| {
                let mut curve = c.curve.clone();
                for p in &mut curve.poles {
                    *p = r.point(*p);
                }
                SectionCurve { curve, t0: c.t0, t1: c.t1, on: c.on.clone(), id, tol: c.tol }
            })
            .collect();
        Ok((moved, from))
    }

    /// Faces `fs` and `ft` of two tools that are rigid copies of another pair (each tool a member of a family of
    /// copies): that pair (the first face's family's first and the member the second's tool goes to with it) and
    /// the motion from it; None where they aren't, or are that pair.
    fn moved_pair(&self, fs: usize, ft: usize) -> Option<crate::MovedPair> {
        let (a, b) = (self.faces[fs].arg as usize, self.faces[ft].arg as usize);
        // A face of another argument (no copies of it) and a copied tool's: that face's surface where the motion
        // leaves it (a plane across a rotation's axis, a cylinder about it) with the family's first's face.
        let copy = |x: usize| self.copies.get(x).copied().flatten();
        if let (None, Some((fam, r))) | (Some((fam, r)), None) = (copy(a), copy(b)) {
            let (fo, ftool) = if copy(a).is_none() { (fs, ft) } else { (ft, fs) };
            // (The member whose sections with this face are made: the one found clear of its boundary, at first
            // the family's first.)
            let k = *self.canonical.get(&(fo, fam, self.faces[ftool].index)).unwrap_or(&0);
            let (c, rc) = *self.families[fam].get(k)?;
            if !self.kept_by(fo, &r) {
                return None;
            }
            let (o, t) = ((self.faces[fo].arg, self.faces[fo].index), (c as u32, self.faces[ftool].index));
            return Some((if fo == fs { (o, t) } else { (t, o) }, r.after(&rc.inverse())));
        }
        let ((fa, ra), (fb, rb)) = (*self.copies.get(a)?.as_ref()?, *self.copies.get(b)?.as_ref()?);
        let rel = ra.inverse().after(&rb);
        let &(m, _) = self.families[fb].iter().find(|x| x.1.near(&rel, self.tol, self.size))?;
        let base = self.families[fa][0].0;
        // (That pair itself: its own sections, kept for the copies.)
        Some((((base as u32, self.faces[fs].index), (m as u32, self.faces[ft].index)), ra))
    }

    /// Whether motion `r` takes face `f`'s surface onto itself (a plane or a cylinder; a cone's apex kept).
    /// The rotation the arrangement is the same under, if one: its step from the largest family of copies whose
    /// motions are rotations about one axis (a whole turn in whole steps), checked on every argument.
    fn symmetry(&self) -> Option<Sym> {
        if std::env::var_os("NOSHAPE_X_NOSYM").is_some() {
            return None;
        }
        let fam = self.families.iter().filter(|f| f.len() >= 3).max_by_key(|f| f.len())?;
        let (o, a, _) = fam.iter().find_map(|x| x.1.rotation())?;
        // The members' angles about that axis; the least is the step.
        let mut step = f64::INFINITY;
        for (_, r) in &fam[1..] {
            let (ro, ra, th) = r.rotation()?;
            if ra.cross(a).length() > 1e-9 || (ro - o).cross(a).length() > self.tol {
                return None;
            }
            step = step.min(if ra.dot(a) > 0.0 { th } else { TAU - th });
        }
        let n = (TAU / step).round();
        if n < 3.0 || (n * step - TAU).abs() > 1e-6 {
            return None;
        }
        let (x, y) = a.any_orthonormal_pair();
        let mut sym = Sym { o, a, x, y, n: n as usize, solid_back: Vec::new(), face_back: Vec::new(), arg_back: Vec::new() };
        // Arguments in no family of several: kept by it (their faces' surfaces, their edges whole circles about the
        // axis or seams).
        let g = sym.turn(1.0);
        let kept_arg = |b: u32| {
            (0..self.faces.len()).filter(|&i| self.faces[i].arg == b).all(|i| {
                let f = &self.faces[i];
                self.kept_by(i, &g)
                    && f.edges.iter().all(|&(e, _)| {
                        f.edges.iter().filter(|x| x.0 == e).count() == 2
                            || matches!(self.edges[e].g, Geo::Arc { k, sweep, .. } if (sweep.abs() - TAU).abs() <= 1e-12 && (k.c - o).cross(a).length() <= self.tol && k.n.cross(a).length() <= 1e-9)
                    })
            })
        };
        let nargs = self.solids.iter().map(|x| x.arg as usize + 1).max().unwrap_or(0);
        let mut kept = vec![false; nargs];
        for (b, k) in kept.iter_mut().enumerate() {
            if self.copies.get(b).copied().flatten().is_none() {
                if !kept_arg(b as u32) {
                    return None;
                }
                *k = true;
            }
        }
        // Each argument by each power of the inverse: itself where kept, else the member its motion goes to.
        let mut face_at: HashMap<(u32, u32), usize> = HashMap::default();
        for (i, f) in self.faces.iter().enumerate() {
            face_at.insert((f.arg, f.index), i);
        }
        let solids_of: Vec<Vec<usize>> = (0..nargs).map(|b| (0..self.solids.len()).filter(|&t| self.solids[t].arg as usize == b).collect()).collect();
        for j in 1..sym.n {
            let back = sym.turn(-(j as f64));
            let mut arg_back = vec![usize::MAX; nargs];
            for b in 0..nargs {
                if kept[b] {
                    arg_back[b] = b;
                    continue;
                }
                let (k, r) = self.copies[b]?;
                let want = back.after(&r);
                let c = self.families[k].iter().find(|x| x.1.near(&want, self.tol, self.size))?.0;
                if solids_of[c].len() != solids_of[b].len() {
                    return None;
                }
                arg_back[b] = c;
            }
            let mut sb = vec![usize::MAX; self.solids.len()];
            for b in 0..nargs {
                for (k, &t) in solids_of[b].iter().enumerate() {
                    sb[t] = solids_of[arg_back[b]][k];
                }
            }
            let fb = self.faces.iter().map(|f| *face_at.get(&(arg_back[f.arg as usize] as u32, f.index)).unwrap_or(&usize::MAX)).collect();
            sym.solid_back.push(sb);
            sym.face_back.push(fb);
            sym.arg_back.push(arg_back);
        }
        Some(sym)
    }

    fn kept_by(&self, f: usize, r: &Rigid) -> bool {
        match self.faces[f].s {
            Surf::Plane { n, d, origin, .. } => (r.dir(n) - n).length() <= 1e-9 && (n.dot(r.point(origin)) - d).abs() <= self.tol,
            Surf::Cyl { o, a, k, .. } => {
                let w = r.point(o) - o;
                (r.dir(a) - a).length() <= 1e-9 && if k == 0.0 { w.cross(a).length() <= self.tol } else { w.length() <= self.tol }
            }
            Surf::Nurbs { .. } => false,
        }
    }

    /// Whether OCCT's moved section curves on the other argument's face `f`, and the curves they were moved from,
    /// stay clear of its boundary (the face trims and splits OCCT's curves there): (moved, from).
    fn clear_inside(&self, f: usize, curves: &[SectionCurve], r: &Rigid) -> R<(bool, bool)> {
        let bound = self.bound(f)?;
        // (Only the boundary the motion doesn't keep counts: a whole circle about a rotation's axis is the same
        // there, and OCCT's curves meet it alike.)
        let axis = r.rotation();
        let kept = |e: usize| match (self.edges[e].g, axis) {
            (Geo::Arc { k, sweep, .. }, Some((o, a, _))) => (sweep.abs() - TAU).abs() <= 1e-12 && (k.c - o).cross(a).length() <= self.tol && k.n.cross(a).length() <= 1e-9,
            _ => false,
        };
        let moving: Vec<C2> = self.faces[f].edges.iter().zip(bound).filter(|x| !kept(x.0.0)).map(|x| x.1.clone()).collect();
        let margin = 100.0 * GREY * self.tol;
        // A curve on the face's parameter plane clear of that boundary: not meeting it, its ends not on it.
        let clear = |c: &NurbsCurve, t0: f64, t1: f64| -> R<bool> {
            // (Across a round face's seam: not clear.)
            if !free::seam_cuts(self, [f, usize::MAX], c, t0, t1).is_empty() {
                return Ok(false);
            }
            let c2 = free::mapped(&self.faces[f], &Arc::new(c.clone()), t0, t1)?;
            if [c2.start(), c2.end()].iter().any(|&q| distance2(q, &moving) <= margin) {
                return Ok(false);
            }
            Ok(moving.iter().all(|b| c2.intersections(b, margin).is_empty()))
        };
        let back = r.inverse();
        let (mut moved, mut from) = (true, true);
        for c in curves {
            moved &= clear(&c.curve, c.t0, c.t1)?;
            let mut source = c.curve.clone();
            for p in &mut source.poles {
                *p = back.point(*p);
            }
            from &= clear(&source, c.t0, c.t1)?;
        }
        Ok((moved, from))
    }

    /// A section along OCCT's curve from `t0` to `t1`, cut at `cuts` (seams it crosses): a closed one has a
    /// vertex where it starts (as OCCT's), an open one ends where it does.
    fn section_nurbs(&mut self, fs: usize, ft: usize, path: Path, (t0, t1): (f64, f64), cuts: &[f64], join: [bool; 2]) -> R<()> {
        let closed = (path.at(t0) - path.at(t1)).length() <= self.tol;
        let first = if closed || join[0] { End::Point(path.at(t0)) } else { End::Open };
        let last = if closed { first } else if join[1] { End::Point(path.at(t1)) } else { End::Open };
        let mut at: Vec<(f64, End)> = vec![(t0, first)];
        for &t in cuts {
            if path.length(t0, t) > self.tol && path.length(t, t1) > self.tol {
                at.push((t, End::Point(path.at(t))));
            }
        }
        for i in 0..at.len() {
            let (a, ea) = at[i];
            let (b, eb) = if i + 1 < at.len() { at[i + 1] } else { (t1, last) };
            self.section_part(fs, ft, path.clone(), a, b, [ea, eb])?;
        }
        Ok(())
    }

    fn face_pair(&mut self, fs: usize, ft: usize) -> R<()> {
        // (Clearly apart: no section, and no edge of one in the other.)
        if self.faces_apart(fs, ft) {
            return Ok(());
        }
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
                return self.plane_cone(fp, fc);
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
            let e = Ell { c, n: sn, x: xe, y: sn.cross(xe), a: r / na.abs(), b: r, hyp: false, par: false };
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
                let e = Ell { c: apex - n * dd, n: sn, x: xe, y: sn.cross(xe), a: dd.abs() / slope.abs(), b: dd.abs(), hyp: true, par: false };
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
            // (Through its apex too: the two rulings come to it, the apex in pieces between them (apex_parts).)
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

    /// Whether cone face `fc` reaches its apex (the apex's height within its vertices' heights).
    fn apex_on_face(&self, fc: usize) -> bool {
        let Surf::Cyl { o, a, r0, k, .. } = self.faces[fc].s else { unreachable!() };
        let hs = self.faces[fc].edges.iter().flat_map(|&(e, _)| self.edges[e].v).map(|w| (self.p(w) - o).dot(a));
        let (h0, h1) = hs.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), h| (lo.min(h), hi.max(h)));
        let ha = -r0 / k;
        ha >= h0 - GREY * self.tol && ha <= h1 + GREY * self.tol
    }

    /// A plane at an angle to a cone's axis, cutting each of the face's nappe's rulings (else declined): an
    /// ellipse, its axes from the two rulings in the plane through the axis and the plane's normal.
    fn plane_cone(&mut self, fp: usize, fc: usize) -> R<()> {
        let Surf::Plane { n, d, flip, .. } = self.faces[fp].s else { unreachable!() };
        let Surf::Cyl { o, a, r0, k: slope, th0, .. } = self.faces[fc].s else { unreachable!() };
        let tol = self.tol;
        let na = n.dot(a);
        let along = slope.abs() * (1.0 - na * na).sqrt();
        if (na.abs() - along).abs() <= 1e-12 {
            return self.plane_cone_parabola(fp, fc);
        }
        // (A hyperbola: not yet.)
        if na.abs() <= along * (1.0 + 1e-6) {
            return Err(Decline("a plane at an angle to a cone"));
        }
        // The apex, the nappe's direction (radii growing), the rulings in the plane of the axis and the normal.
        let apex = o - a * (r0 / slope);
        let up = if slope > 0.0 { a } else { -a };
        let side = (n - a * na).normalize();
        let (ca, sa) = (1.0 / (1.0 + slope * slope).sqrt(), slope.abs() / (1.0 + slope * slope).sqrt());
        let hit = |g: DVec3| {
            let den = n.dot(g);
            (den.abs() > 1e-300).then(|| (d - n.dot(apex)) / den).filter(|&s| s > 0.0).map(|s| apex + g * s)
        };
        let (Some(v1), Some(v2)) = (hit(up * ca + side * sa), hit(up * ca - side * sa)) else {
            // The other nappe: not this face's.
            return Ok(());
        };
        let c = (v1 + v2) * 0.5;
        // As OCCT's: its normal the plane surface's, its major axis along the axis' direction in the plane.
        let sn = if flip { -n } else { n };
        let mut xe = (v1 - v2).normalize();
        if xe.dot(a) < 0.0 {
            xe = -xe;
        }
        let ae = (v1 - v2).length() * 0.5;
        let h = (c - o).dot(a);
        let rc = r0 + slope * h;
        let off = (c - o - a * h).length();
        let be = (rc * rc - off * off).max(0.0).sqrt();
        if be <= GREY * tol {
            // Through the apex: only the apex in common, nothing where the face doesn't reach it.
            if self.apex_on_face(fc) {
                return Err(Decline("a plane through a cone's apex"));
            }
            return Ok(());
        }
        let e = Ell { c, n: sn, x: xe, y: sn.cross(xe), a: ae, b: be, hyp: false, par: false };
        let (lo, hi) = (self.faces[fc].lo, self.faces[fc].hi);
        if !boxes_meet(c - DVec3::splat(ae), c + DVec3::splat(ae), lo, hi, GREY * tol) {
            return Ok(());
        }
        let ts = self.ell_at_angle(fc, &e, th0);
        self.section(fp, fc, Path::Ellipse(e), ts, ts + TAU)
    }

    /// A plane along one of a cone's rulings: a parabola, its vertex where the other ruling in the plane of the
    /// axis and the plane's normal meets the plane, opening along the first.
    fn plane_cone_parabola(&mut self, fp: usize, fc: usize) -> R<()> {
        let Surf::Plane { n, d, flip, .. } = self.faces[fp].s else { unreachable!() };
        let Surf::Cyl { o, a, r0, k: slope, .. } = self.faces[fc].s else { unreachable!() };
        let tol = self.tol;
        let apex = o - a * (r0 / slope);
        let up = if slope > 0.0 { a } else { -a };
        let side = (n - a * n.dot(a)).normalize();
        let (ca, sa) = (1.0 / (1.0 + slope * slope).sqrt(), slope.abs() / (1.0 + slope * slope).sqrt());
        let (g1, g2) = (up * ca + side * sa, up * ca - side * sa);
        // The ruling along the plane and the one across it.
        let (along, across) = if n.dot(g1).abs() < n.dot(g2).abs() { (g1, g2) } else { (g2, g1) };
        let den = n.dot(across);
        let s = (d - n.dot(apex)) / den;
        if s.abs() <= GREY * tol {
            // Through the apex along the ruling: touching the cone along it (as a plane a cylinder; to the apex, the
            // apex in pieces there: apex_parts).
            let Surf::Cyl { x, y, .. } = self.faces[fc].s else { unreachable!() };
            let rad = along - a * along.dot(a);
            return self.touch(fp, fc, rad.dot(y).atan2(rad.dot(x)));
        }
        if s < 0.0 {
            // The other nappe: not this face's.
            return Ok(());
        }
        let v = apex + across * s;
        let sn = if flip { -n } else { n };
        let xe = (along - sn * along.dot(sn)).normalize();
        // Its focal length from a point a little way along it: s' along x, half-width w on the cone, s' = w² / 4f.
        let step = (v - apex).length().max(tol);
        let q = v + xe * step;
        let h = (q - o).dot(a);
        let (rq, off) = (r0 + slope * h, (q - o - a * h).length());
        let w2 = rq * rq - off * off;
        if w2.is_nan() || w2 <= 0.0 {
            return Err(Decline("a plane at an angle to a cone"));
        }
        let f = w2 / (4.0 * step);
        let e = Ell { c: v, n: sn, x: xe, y: sn.cross(xe), a: f, b: 0.0, hyp: false, par: true };
        // Its range: as far along the axis as the face goes.
        let hs = self.faces[fc].edges.iter().flat_map(|&(e, _)| self.edges[e].v).map(|w| (self.p(w) - o).dot(a));
        let (h0, h1) = hs.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), h| (lo.min(h), hi.max(h)));
        let hv = (v - o).dot(a);
        let rise = xe.dot(a);
        if rise.abs() < 1e-12 {
            return Err(Decline("a plane at an angle to a cone"));
        }
        let reach = ((h0 - hv) / rise).max((h1 - hv) / rise);
        if reach <= 0.0 {
            return Ok(());
        }
        let t = (4.0 * f * reach).sqrt() * 1.01 + tol;
        self.section(fp, fc, Path::Ellipse(e), -t, t)
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
        let Surf::Cyl { o, a, x, y, r, r0, k, .. } = self.faces[fc].s else { unreachable!() };
        let rad = x * th.cos() + y * th.sin();
        // (A cone's ruling: from its point at the origin's height.)
        let (p, d) = if k == 0.0 { (o + rad * r, a) } else { (o + rad * r0, (a + rad * k).normalize()) };
        let (lo, hi) = (self.faces[f].lo.max(self.faces[fc].lo), self.faces[f].hi.min(self.faces[fc].hi));
        let (t0, t1) = box_range(p, d, lo, hi, self.tol);
        if t0 >= t1 {
            return Ok(());
        }
        let path = Path::Line { p, d };
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
                if self.src.is_some() {
                    return self.nurbs_pair(f1, f2);
                }
                return Err(Decline("a cone at an angle"));
            }
            return self.cyl_cyl_angled(f1, f2);
        }
        let w = o2 - o1;
        let w = w - a1 * w.dot(a1);
        let dl = w.length();
        if (k1 != 0.0 || k2 != 0.0) && dl > GREY * tol {
            if self.src.is_some() {
                return self.nurbs_pair(f1, f2);
            }
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
            // (A curve of the fourth order: OCCT's section, as a free-form face's.)
            if self.src.is_some() {
                return self.nurbs_pair(f1, f2);
            }
            return Err(Decline("cylinders at an angle"));
        }
        let (b, d, e) = (a1.dot(a2), a1.dot(w), a2.dot(w));
        let c = o1 + a1 * ((d - b * e) / (1.0 - b * b));
        let (ch, sh) = (((1.0 + b) * 0.5).sqrt(), ((1.0 - b) * 0.5).sqrt());
        let (sum, diff) = ((a1 + a2).normalize(), (a1 - a2).normalize());
        for (n, x, major) in [(sum, diff, r1 / ch), (diff, sum, r1 / sh)] {
            let el = Ell { c, n, x, y: n.cross(x), a: major, b: r1, hyp: false, par: false };
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
    /// (`te`: the edge's own tolerance: a vertex within the sum of the two's is on it, OCCT's rule.)
    fn split_among(&self, s: &Seg, lists: &ByOwner, own: Option<u32>, te: f64) -> R<Vec<u32>> {
        if matches!(s.g, Geo::Apex { .. }) {
            return Ok(vec![s.v[0], s.v[1]]);
        }
        let mut chain: Vec<(f64, u32)> = Vec::new();
        for (who, list) in lists {
            if own.is_some() && *who == own {
                continue;
            }
            let c = self.split_tol(s, list, te)?;
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
        self.split_tol(s, sorted_x, 0.0)
    }

    /// `split`, a vertex on `s` within the Boolean's tolerance or the sum of its own and `te` where that's more.
    fn split_tol(&self, s: &Seg, sorted_x: &[(f64, u32)], te: f64) -> R<Vec<u32>> {
        let tol = self.tol;
        let reach = |w: u32| tol.max(te + self.pool.t[w as usize]);
        let tol_far = tol.max(te + self.pool.tmax);
        let (lo, hi) = match s.g {
            Geo::Line => {
                let (a, b) = (self.p(s.v[0]), self.p(s.v[1]));
                (a.min(b), a.max(b))
            }
            Geo::Arc { k, .. } => (k.c - DVec3::splat(k.r), k.c + DVec3::splat(k.r)),
            Geo::Ell { e, t0, sweep, .. } => e.piece_box(t0, sweep),
            Geo::Nurbs { id, t0, t1 } => free::piece_box(&self.ncurves[id as usize].c, t0, t1),
            Geo::Apex { .. } => return Ok(vec![s.v[0], s.v[1]]),
        };
        let (lo, hi) = (lo - DVec3::splat(tol_far), hi + DVec3::splat(tol_far));
        let start = sorted_x.partition_point(|x| x.0 < lo.x);
        // (Its length found when a vertex near it asks.)
        let len = std::cell::OnceCell::new();
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
            // (A NURBS curve's pieces' boxes: clear of them, clear of it.)
            if let Geo::Nurbs { id, t0, t1 } = s.g {
                let nc = &self.ncurves[id as usize];
                let boxes = nc.boxes.get_or_init(|| nc.c.piece_boxes(8));
                let g = GREY * tol_far;
                let (ta, tb) = (t0.min(t1), t0.max(t1));
                if !boxes.is_empty() && boxes.iter().filter(|x| x.1 >= ta && x.0 <= tb).all(|x| (x.2 - p).max_element() > g || (p - x.3).max_element() > g) {
                    continue;
                }
            }
            let (t, off) = self.seg_param(s, p);
            if t <= 0.0 || t >= 1.0 {
                continue;
            }
            let len = *len.get_or_init(|| self.seg_length(s));
            let r = reach(w);
            if t * len <= r || (1.0 - t) * len <= r {
                continue;
            }
            if off <= r {
                on.push((t, w));
            } else if off <= GREY * r {
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
                    Geo::Apex { a0, a1 } => Geo::Apex { a0, a1 },
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
        if sol.sheet || !boxes_meet(p, p, sol.lo, sol.hi, tol) {
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
                        for (uv, t) in free::ray_hits(ns, g.patches.get_or_init(|| free::grouped(g)), g.uvbox, p, r) {
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

/// `f` of 0..n on all threads, in order.
pub(crate) fn par_map<T: Send>(n: usize, f: &(dyn Fn(usize) -> T + Sync)) -> Vec<T> {
    let threads = std::thread::available_parallelism().map_or(1, |x| x.get()).min(n.div_ceil(64)).max(1);
    if threads == 1 {
        return (0..n).map(f).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut parts: Vec<Vec<(usize, T)>> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads)
            .map(|_| {
                sc.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let k = next.fetch_add(16, std::sync::atomic::Ordering::Relaxed);
                        if k >= n {
                            break;
                        }
                        out.extend((k..(k + 16).min(n)).map(|i| (i, f(i))));
                    }
                    out
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("a worker")).collect()
    });
    let mut all: Vec<(usize, T)> = parts.drain(..).flatten().collect();
    all.sort_by_key(|x| x.0);
    all.into_iter().map(|x| x.1).collect()
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
        pool: Pool { p: Vec::new(), inputs: Vec::new(), grid: HashMap::default(), cell: GREY * tol * 1.01, tol, t: Vec::new(), coarse: HashMap::default(), loose: Vec::new(), tmax: 0.0 },
        faces: Vec::new(),
        edges: Vec::new(),
        solids: Vec::new(),
        side_cache: HashMap::default(),
        in_face_done: Default::default(),
        face_segs: Vec::new(),
        seg_section: Vec::new(),
        coplanar: Vec::new(),
        sections: Vec::new(),
        pending: Vec::new(),
        copies: Vec::new(),
        families: Vec::new(),
        size: 0.0,
        canonical: HashMap::default(),
        canon_secs: HashMap::default(),
        apart: HashMap::default(),
        on_surface: HashMap::default(),
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
            let tv = body.vertex_tol.get(i).copied().unwrap_or(0.0);
            let near = arg_box.iter().enumerate().any(|(k, b)| k != ai && boxes_meet(p, p, b.0, b.1, GREY * tol.max(2.0 * tv)));
            let found = if near { cx.pool.find_tol(p, tv)? } else { None };
            let w = match found {
                Some(w) => {
                    if cx.pool.inputs[w as usize].iter().any(|x| x.0 == a) {
                        return Err(Decline("two vertices of one argument coincide"));
                    }
                    if tv > 0.0 {
                        let d = (cx.p(w) - p).length();
                        cx.pool.grow(w, tv + d);
                    }
                    w
                }
                None => cx.pool.push_tol(p, tv),
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
                    let e = Ell { c: center, n: normal, x: xdir, y: normal.cross(xdir), a: major, b: minor, hyp: false, par: false };
                    let (p0, p1) = (body.vertices[body.edges[i].v[0] as usize], body.vertices[body.edges[i].v[1] as usize]);
                    let t0 = e.param(p0);
                    let sweep = if v[0] == v[1] { TAU } else { (e.param(p1) - t0).rem_euclid(TAU) };
                    Geo::Ell { e, t0, sweep, faces: [usize::MAX; 2] }
                }
                Curve::Hyperbola { .. } | Curve::Parabola { .. } => return Err(Decline("an argument's hyperbola")),
                Curve::Nurbs { .. } if cx.src.is_none() => return Err(Decline("a curved edge")),
                Curve::Nurbs { index, t0, t1 } => {
                    cx.ncurves.push(NCurve { c: Arc::new(body.nurbs_curves[index as usize].clone()), sec: None, from: None, tol: 0.0, boxes: Default::default() });
                    Geo::Nurbs { id: cx.ncurves.len() as u32 - 1, t0, t1 }
                }
                Curve::Degenerate => {
                    if v[0] != v[1] {
                        return Err(Decline("a degenerate edge between two vertices"));
                    }
                    Geo::Apex { a0: f64::NAN, a1: f64::NAN }
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
            cx.edges.push(GEdge { arg: a, index: i as u32, v, g, t: body.edge_tol.get(i).copied().unwrap_or(0.0) });
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
            cx.faces.push(GFace { arg: a, index: i as u32, solid: usize::MAX, s, edges, bound: Default::default(), lo: DVec3::INFINITY, hi: DVec3::NEG_INFINITY, ns, uvbox, pcurves, grid: Default::default(), patches: Default::default(), hulls: Default::default() });
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
            cx.solids.push(GSolid { arg: a, index: i as u32, faces, lo: DVec3::INFINITY, hi: DVec3::NEG_INFINITY, sheet: false });
        }
        // A sheet (faces, no solid): one of its own, with no inside.
        if body.solids.is_empty() && !body.faces.is_empty() && !line {
            let si = cx.solids.len();
            let faces: Vec<usize> = (face0..cx.faces.len()).collect();
            for &f in &faces {
                cx.faces[f].solid = si;
            }
            cx.solids.push(GSolid { arg: a, index: u32::MAX, faces, lo: DVec3::INFINITY, hi: DVec3::NEG_INFINITY, sheet: true });
            continue;
        }
        if cx.faces[face0..].iter().any(|f| f.solid == usize::MAX) {
            return Err(Decline("a face outside a solid"));
        }
        if body.solids.is_empty() && !(line && ai == 0) {
            return Err(Decline("an argument without solids"));
        }
    }
    // Sheets: a fuse of sheets alone, or a sheet cut by solids or in common with them (else not yet).
    let sheet_arg: Vec<bool> = (0..args.len()).map(|a| cx.solids.iter().any(|s| s.sheet && s.arg == a as u32)).collect();
    let sheets = sheet_arg.iter().any(|&x| x);
    if sheets && !((op == Op::Fuse && sheet_arg.iter().all(|&x| x)) || (op != Op::Fuse && sheet_arg[0] && !sheet_arg[1..].iter().any(|&x| x))) {
        return Err(Decline("sheets with solids"));
    }
    // Each face: its parameter range (a partial cylinder's starts in the gap), boundary, box.
    for f in 0..cx.faces.len() {
        let mut lo = DVec3::INFINITY;
        let mut hi = DVec3::NEG_INFINITY;
        for &(e, _) in &cx.faces[f].edges {
            let s = cx.edge_seg(e);
            let n = match s.g {
                Geo::Line | Geo::Apex { .. } => 1,
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
                // An ellipse's or B-spline's: the angles of points along it, unwrapped.
                let pts: Vec<DVec3> = match cx.edges[e].g {
                    Geo::Ell { e: el, t0, sweep, .. } => (0..=32).map(|k| el.at(t0 + sweep * k as f64 / 32.0)).collect(),
                    Geo::Nurbs { id, t0, t1 } => (0..=64).map(|k| cx.ncurves[id as usize].c.at(t0 + (t1 - t0) * k as f64 / 64.0)).collect(),
                    _ => Vec::new(),
                };
                if pts.len() > 1 {
                    let ang = |p: DVec3| (p - o).dot(y).atan2((p - o).dot(x));
                    let (first, mut prev, mut acc) = (ang(pts[0]), ang(pts[0]), 0.0);
                    for &p in &pts[1..] {
                        let a = ang(p);
                        acc += (a - prev + PI).rem_euclid(TAU) - PI;
                        prev = a;
                    }
                    if acc.abs() > 1e-9 {
                        covered.push(if acc > 0.0 { (first, acc) } else { (first + acc, -acc) });
                    }
                }
            }
            let Some(th0) = gap(&covered) else { return Err(Decline("a cylinder face without curved edges")) };
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
    cx.seg_section = vec![Vec::new(); cx.faces.len()];
    // A pattern's instances that must only touch: one with a vertex inside another argument's solid overlaps it,
    // told before any section (the classification would tell it at the end).
    if touching_only {
        for s in 0..cx.solids.len() {
            for t in 0..cx.solids.len() {
                if s == t || cx.solids[s].arg == cx.solids[t].arg || cx.solids[t].sheet || !boxes_meet(cx.solids[s].lo, cx.solids[s].hi, cx.solids[t].lo, cx.solids[t].hi, 0.0) {
                    continue;
                }
                let ws: HashSet<u32> = cx.solids[s].faces.iter().flat_map(|&f| cx.faces[f].edges.iter().flat_map(|&(e, _)| cx.edges[e].v)).collect();
                // Clear of all of the solid's surfaces (a vertex on one, or near one, or a free-form face: not this way).
                let clear = |p: DVec3| {
                    cx.solids[t].faces.iter().all(|&f| match cx.faces[f].s {
                        Surf::Plane { n, d, .. } => (n.dot(p) - d).abs() > 100.0 * cx.tol,
                        Surf::Cyl { o, a, r0, k, .. } => {
                            let w = p - o;
                            let h = w.dot(a);
                            ((w - a * h).length() - (r0 + k * h)).abs() / (1.0 + k * k).sqrt() > 100.0 * cx.tol
                        }
                        Surf::Nurbs { .. } => false,
                    })
                };
                for w in ws {
                    // (A ray that can't tell: not this way.)
                    if cx.pool.inputs[w as usize].len() == 1 && clear(cx.p(w)) && matches!(cx.ray_state(cx.p(w), t), Ok(State::In)) {
                        return Err(Decline("pattern instances overlapping"));
                    }
                }
            }
        }
    }
    // Tools that are rigid copies of each other (a pattern's): a pair of their faces a copy of another pair has
    // OCCT's sections of that one, moved.
    if cx.src.is_some() && args.len() > 2 {
        cx.size = args.iter().flat_map(|b| b.vertices.iter()).fold(0.0, |m: f64, p| m.max(p.abs().max_element()));
        let fams = crate::pattern::families(&args[1..], tol);
        cx.copies = vec![None; args.len()];
        cx.families = fams.into_iter().map(|f| f.into_iter().map(|(i, r)| (i + 1, r)).collect::<Vec<_>>()).collect();
        for (k, f) in cx.families.iter().enumerate() {
            if f.len() > 1 {
                for &(a, r) in f {
                    cx.copies[a] = Some((k, r));
                }
            }
        }
    }
    // The rotation the arrangement is the same under, if any (below: one period's work for the rest).
    let sym = cx.symmetry();
    let size = cx.size.max(cx.pool.p.iter().fold(0.0, |m: f64, p| m.max(p.abs().max_element())));
    // Input vertices the rotation doesn't take onto another's (a cylinder's seam's, on its whole circles), by x: a
    // curve or a copied tool's face near one is worked out itself, not turned from the first period (where another
    // argument's edge or face passes such a vertex OCCT makes it anew: the knurl's seam at its bottom rim).
    let lone: Vec<(f64, u32)> = match &sym {
        Some(sym) => {
            let g = sym.turn(1.0);
            let mut v: Vec<(f64, u32)> =
                (0..cx.pool.p.len() as u32).filter(|&w| !cx.pool.inputs[w as usize].is_empty() && !matches!(cx.pool.find(g.point(cx.p(w))), Ok(Some(_)))).map(|w| (cx.p(w).x, w)).collect();
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            v
        }
        None => Vec::new(),
    };
    let lone_at: Vec<DVec3> = lone.iter().map(|&(_, w)| cx.p(w)).collect();
    let near_lone = |lo: DVec3, hi: DVec3| lone_at.iter().any(|&p| p.cmpge(lo - DVec3::splat(GREY * tol)).all() && p.cmple(hi + DVec3::splat(GREY * tol)).all());
    // Pairs of solids that may meet, and their faces.
    let ns = cx.solids.len();
    // (OCCT's sections free-form faces' pairs below will ask for, a copied pair's the pair it is moved from, made
    // ahead on all threads; any other as asked.)
    // (Whether edges lie on other arguments' faces, where that takes points along them, on all threads ahead for
    // the pairing below: each edge of either face of a pair that reaches the other's box.)
    {
        let mut ahead: Vec<(usize, usize)> = Vec::new();
        let mut seen: HashSet<(usize, usize)> = HashSet::default();
        for s in 0..ns {
            for t in s + 1..ns {
                let (a, b) = (&cx.solids[s], &cx.solids[t]);
                if a.arg == b.arg || !boxes_meet(a.lo, a.hi, b.lo, b.hi, GREY * tol) {
                    continue;
                }
                for &fs in &a.faces {
                    for &ft in &b.faces {
                        let (x, y) = (&cx.faces[fs], &cx.faces[ft]);
                        if !boxes_meet(x.lo, x.hi, y.lo, y.hi, GREY * tol) {
                            continue;
                        }
                        for (f, g) in [(fs, ft), (ft, fs)] {
                            let (lo, hi) = (cx.faces[g].lo, cx.faces[g].hi);
                            for &(e, _) in &cx.faces[f].edges {
                                let [p, q] = cx.edges[e].v;
                                if let Geo::Line = cx.edges[e].g
                                    && !boxes_meet(cx.p(p).min(cx.p(q)), cx.p(p).max(cx.p(q)), lo, hi, GREY * tol)
                                {
                                    continue;
                                }
                                if cx.by_points(e, g) && seen.insert((e, g)) {
                                    ahead.push((e, g));
                                }
                            }
                        }
                    }
                }
            }
        }
        let found = par_map(ahead.len(), &|k| cx.on_free_surface(ahead[k].0, ahead[k].1));
        cx.on_surface = ahead.into_iter().zip(found).collect();
    }
    if let Some(src) = cx.src {
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        for s in 0..ns {
            for t in s + 1..ns {
                let (a, b) = (&cx.solids[s], &cx.solids[t]);
                if a.arg == b.arg || !boxes_meet(a.lo, a.hi, b.lo, b.hi, GREY * tol) {
                    continue;
                }
                for &fs in &a.faces {
                    for &ft in &b.faces {
                        let (x, y) = (&cx.faces[fs], &cx.faces[ft]);
                        if boxes_meet(x.lo, x.hi, y.lo, y.hi, GREY * tol) && matches!((&x.s, &y.s), (Surf::Nurbs { .. }, _) | (_, Surf::Nurbs { .. })) {
                            pairs.push((fs, ft));
                        }
                    }
                }
            }
        }
        // (Which are clearly apart, on all threads, kept for the pairing.)
        let apart = par_map(pairs.len(), &|k| free::apart(&cx.faces[pairs[k].0], &cx.faces[pairs[k].1], GREY * tol.max(5e-6)));
        let mut ahead: Vec<FacePair> = Vec::new();
        let mut seen: HashSet<FacePair> = HashSet::default();
        for (&(fs, ft), x) in pairs.iter().zip(apart) {
            cx.apart.insert((fs, ft), x);
            if x {
                continue;
            }
            let (x, y) = (&cx.faces[fs], &cx.faces[ft]);
            let pair = cx.moved_pair(fs, ft).map_or(((x.arg, x.index), (y.arg, y.index)), |m| m.0);
            if seen.insert(pair) {
                ahead.push(pair);
            }
        }
        src.ahead(&ahead);
    }
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
    // The sections' parts and the edges lying on faces: their places in the faces on all threads, their edges in
    // the order the faces were paired.
    let pending = std::mem::take(&mut cx.pending);
    let place = |k: usize| -> R<(Vec<Ival>, Vec<Ival>)> {
        match pending[k] {
            Pending::Section(fs, ft, ref path, t0, t1, ends) => {
                let ia = cx.intervals(fs, path, t0, t1, ends, false)?;
                if ia.is_empty() {
                    return Ok((ia, Vec::new()));
                }
                Ok((ia, cx.intervals(ft, path, t0, t1, ends, false)?))
            }
            Pending::InFace(f, ref path, ref parts, ref on_free, _) => {
                let mut ivs = Vec::new();
                for &(a0, e0, a1, e1) in parts {
                    ivs.extend(cx.intervals_on(f, path, (a0, a1), [e0, e1], true, on_free.as_ref())?);
                }
                Ok((ivs, Vec::new()))
            }
        }
    };
    // (With the arrangement the same under a rotation: a part of a section moved from the same OCCT section as one
    // in the first period, on the faces those turn into, or an edge on a face the turned image of one there, takes
    // its places there: the parameters as they are, its points turned, the vertices the ones there. The rest found
    // as they are. A part's key: a section's faces turned back, the section it is moved from and its range; an
    // edge on a face: the face and edge turned back; then the steps.)
    let mut edge_at: HashMap<(u32, u32), usize> = HashMap::default();
    for (i, e) in cx.edges.iter().enumerate() {
        edge_at.insert((e.arg, e.index), i);
    }
    let from_of = |k: usize| -> Option<(usize, usize, u32, u64, u64, usize)> {
        let sym = sym.as_ref()?;
        match pending[k] {
            Pending::Section(fs, ft, Path::Nurbs { id, ref c }, t0, t1, _) => {
                let from = cx.ncurves[id as usize].from?;
                let j = sym.period(c.at(0.5 * (t0 + t1)), size)?;
                let (fs0, ft0) = if j == 0 { (fs, ft) } else { (sym.face_back[j - 1][fs], sym.face_back[j - 1][ft]) };
                (fs0 != usize::MAX && ft0 != usize::MAX).then_some((fs0, ft0, from, t0.to_bits(), t1.to_bits(), j))
            }
            Pending::InFace(f, ref path, ref parts, _, e) => {
                let (a0, _, a1, _) = *parts.first()?;
                let j = sym.period(path.at(0.5 * (a0 + a1)), size)?;
                let (f0, e0) = if j == 0 {
                    (f, e)
                } else {
                    let ed = &cx.edges[e];
                    (sym.face_back[j - 1][f], *edge_at.get(&(sym.arg_back[j - 1][ed.arg as usize] as u32, ed.index))?)
                };
                (f0 != usize::MAX).then_some((f0, e0, u32::MAX, 0, 0, j))
            }
            _ => None,
        }
    };
    let mut firsts: HashMap<(usize, usize, u32, u64, u64), usize> = HashMap::default();
    let mut source: Vec<Option<(usize, usize)>> = vec![None; pending.len()];
    for k in 0..pending.len() {
        if let Some((fs0, ft0, from, a, b, 0)) = from_of(k) {
            firsts.entry((fs0, ft0, from, a, b)).or_insert(k);
        }
    }
    for (k, x) in source.iter_mut().enumerate() {
        if let Some((fs0, ft0, from, a, b, j)) = from_of(k)
            && j > 0
        {
            *x = firsts.get(&(fs0, ft0, from, a, b)).map(|&k0| (k0, j));
        }
    }
    let first = par_map(pending.len(), &|k| source[k].is_none().then(|| place(k)));
    // A place turned by `j` steps from face `f0` onto face `f` (its vertex the one of `f` there), if it can be.
    let turn_end = |e: End, f: usize, also: Option<usize>, g: &Rigid| -> Option<End> {
        Some(match e {
            End::Open => End::Open,
            End::Point(p) => End::Point(g.point(p)),
            End::Vertex(w) => {
                let q = g.point(cx.p(w));
                End::Vertex(cx.faces[f].edges.iter().map(|x| x.0).chain(also).flat_map(|e| cx.edges[e].v).find(|&v| (cx.p(v) - q).length() <= tol)?)
            }
        })
    };
    let turned = |k: usize| -> Option<(Vec<Ival>, Vec<Ival>)> {
        let sym = sym.as_ref()?;
        let (k0, j) = source[k]?;
        let Some(Ok((ia0, ib0))) = &first[k0] else { return None };
        let g = sym.turn(j as f64);
        if let (Pending::InFace(f, path, parts, _, e), Pending::InFace(_, path0, parts0, _, _)) = (&pending[k], &pending[k0]) {
            let (f, e) = (*f, *e);
            // (The same edge turned: its parts at the same parameters, their points and ends turned.)
            if parts.len() != parts0.len() {
                return None;
            }
            for (x, y) in parts.iter().zip(parts0) {
                let same = |a: f64, b: f64| (a - b).abs() <= 1e-12 * (1.0 + a.abs());
                if !same(x.0, y.0) || !same(x.2, y.2) || (g.point(path0.at(y.0)) - path.at(x.0)).length() > tol || (g.point(path0.at(y.2)) - path.at(x.2)).length() > tol {
                    return None;
                }
            }
            let ivs = ia0.iter().map(|iv| Some([(iv[0].0, turn_end(iv[0].1, f, Some(e), &g)?), (iv[1].0, turn_end(iv[1].1, f, Some(e), &g)?)])).collect::<Option<Vec<Ival>>>()?;
            return Some((ivs, Vec::new()));
        }
        let Pending::Section(fs, ft, Path::Nurbs { ref c, .. }, t0, t1, ends) = pending[k] else { return None };
        let Pending::Section(_, _, Path::Nurbs { c: ref c0, .. }, _, _, ends0) = pending[k0] else { return None };
        // (The same curve turned, its ends likewise.)
        let mid = 0.5 * (t0 + t1);
        if (g.point(c0.at(mid)) - c.at(mid)).length() > tol || (0..2).any(|i| turn_end(ends0[i], fs, None, &g).is_none_or(|e| match (e, ends[i]) {
            (End::Open, End::Open) => false,
            (End::Point(p), End::Point(q)) => (p - q).length() > tol,
            _ => true,
        })) {
            return None;
        }
        let ivs = |x: &[Ival], f: usize| -> Option<Vec<Ival>> { x.iter().map(|iv| Some([(iv[0].0, turn_end(iv[0].1, f, None, &g)?), (iv[1].0, turn_end(iv[1].1, f, None, &g)?)])).collect() };
        Some((ivs(ia0, fs)?, ivs(ib0, ft)?))
    };
    let rest = par_map(pending.len(), &|k| first[k].is_none().then(|| turned(k).map_or_else(|| place(k), Ok)));
    let places: Vec<R<(Vec<Ival>, Vec<Ival>)>> = first.into_iter().zip(rest).map(|(a, b)| a.or(b).expect("a part's places")).collect();
    for (p, x) in pending.into_iter().zip(places) {
        let (ia, ib) = x?;
        match p {
            Pending::Section(fs, ft, path, ..) if !ia.is_empty() => cx.section_from(fs, ft, path, ia, ib)?,
            Pending::Section(..) => {}
            Pending::InFace(f, path, ..) => cx.in_face_from(f, &path, ia)?,
        }
    }
    // Coplanar faces where a sheet is one of them: taken where they only meet at each other's vertices (side by
    // side, sharing corners); a vertex of one inside the other or on its edge between its vertices declines (OCCT
    // left two strips overlapping by 5 µm, one's corner on the other's edge, as they were: 0c538d2c).
    for k in 0..cx.coplanar.len() {
        let (f, g) = cx.coplanar[k];
        if !cx.solids[cx.faces[f].solid].sheet && !cx.solids[cx.faces[g].solid].sheet {
            continue;
        }
        for (a, b) in [(f, g), (g, f)] {
            let theirs: HashSet<u32> = cx.faces[b].edges.iter().flat_map(|&(e, _)| cx.edges[e].v).collect();
            for &(e, _) in &cx.faces[a].edges {
                for w in cx.edges[e].v {
                    if theirs.contains(&w) {
                        continue;
                    }
                    let q = cx.faces[b].to2(cx.p(w));
                    let bound = cx.bound(b)?;
                    if distance2(q, bound) <= GREY * tol || inside2(q, bound) {
                        return Err(Decline("coplanar sheets"));
                    }
                }
            }
        }
    }
    // Curves added to one face that cross each other (edges of two arguments lying on a third's face): a
    // vertex where they cross.
    // (Each face's curves' crossings with the curves after them, on all threads; their vertices made in order
    // below.)
    // (A face's curves and their boxes.)
    type FaceCurves = (Vec<C2>, Vec<Option<(DVec2, DVec2)>>);
    // (A copied tool's face: its family's first member's face and the steps from it.)
    let copy_step = |f: usize| -> Option<(usize, usize)> {
        let sym = sym.as_ref()?;
        let (k, r) = cx.copies.get(cx.faces[f].arg as usize).copied().flatten()?;
        let (_, ax, th) = r.rotation()?;
        let steps = (if ax.dot(sym.a) > 0.0 { th } else { TAU - th }) * sym.n as f64 / TAU;
        let j = steps.round() as usize % sym.n;
        if j == 0 || (steps - steps.round()).abs() > 1e-6 {
            return None;
        }
        let f0 = sym.face_back[j - 1][f];
        (f0 != usize::MAX && cx.faces[f0].arg as usize == cx.families[k][0].0 && !near_lone(cx.faces[f].lo, cx.faces[f].hi)).then_some((f0, j))
    };
    // (With the arrangement the same under a rotation: a copied tool's face whose curves are its family's first
    // member's face's turned, one for one (by each of its curves, that face's), crosses where that one does.)
    let like_first = |f: usize| -> Option<(usize, Vec<usize>)> {
        let sym = sym.as_ref()?;
        let (f0, j) = copy_step(f)?;
        if cx.face_segs[f].len() != cx.face_segs[f0].len() {
            return None;
        }
        let back = sym.turn(-(j as f64));
        let mut at: HashMap<[u32; 2], Vec<usize>> = HashMap::default();
        for (i, s0) in cx.face_segs[f0].iter().enumerate() {
            at.entry(s0.v).or_default().push(i);
        }
        let mut used = vec![false; cx.face_segs[f0].len()];
        let mut to = Vec::with_capacity(used.len());
        for s1 in &cx.face_segs[f] {
            let v = [cx.pool.find(back.point(cx.p(s1.v[0]))).ok()??, cx.pool.find(back.point(cx.p(s1.v[1]))).ok()??];
            let mid = back.point(cx.seg_at(s1, 0.5));
            let i = *at.get(&v)?.iter().find(|&&i| !used[i] && std::mem::discriminant(&cx.face_segs[f0][i].g) == std::mem::discriminant(&s1.g) && (cx.seg_at(&cx.face_segs[f0][i], 0.5) - mid).length() <= tol)?;
            used[i] = true;
            to.push(i);
        }
        Some((f0, to))
    };
    let firsts: Vec<Option<(usize, Vec<usize>)>> = par_map(cx.faces.len(), &like_first);
    let curves_of = |f: usize| -> R<FaceCurves> {
        let c2: Vec<C2> = if cx.face_segs[f].len() < 2 { Vec::new() } else { cx.face_segs[f].iter().map(|s| cx.c2_of(f, s, 0, None)).collect::<R<_>>()? };
        // (Curves whose control points' boxes are apart don't cross.)
        let boxes = c2.iter().map(|c| c.quick_box()).collect();
        Ok((c2, boxes))
    };
    let curves = par_map(cx.faces.len(), &|f| if firsts[f].is_some() { Ok((Vec::new(), Vec::new())) } else { curves_of(f) });
    // (Each face's curves' lengths, found once when asked.)
    let lengths: Vec<Vec<std::sync::OnceLock<f64>>> = curves.iter().map(|c| c.as_ref().map_or(Vec::new(), |x| (0..x.0.len()).map(|_| Default::default()).collect())).collect();
    let mut items: Vec<(usize, usize)> = Vec::new();
    for (f, c) in curves.iter().enumerate() {
        if let Ok((c2, ..)) = c {
            items.extend((0..c2.len()).map(|i| (f, i)));
        }
    }
    // (Both curves and both parameters of each.)
    type Crossings = Vec<(usize, usize, f64, f64)>;
    let crossings_with = |f: usize, i: usize, js: &mut dyn Iterator<Item = usize>, curves: &[R<FaceCurves>]| -> Crossings {
        let Ok((c2, boxes)) = &curves[f] else { return Vec::new() };
        let segs = &cx.face_segs[f];
        let mut out = Vec::new();
        for j in js {
            if let (Some((li, hi)), Some((lj, hj))) = (boxes[i], boxes[j])
                && ((li - hj).max_element() > tol || (lj - hi).max_element() > tol)
            {
                continue;
            }
            for (t, u) in c2[i].intersections(&c2[j], tol) {
                let (li, lj) = (*lengths[f][i].get_or_init(|| cx.seg_length(&segs[i])), *lengths[f][j].get_or_init(|| cx.seg_length(&segs[j])));
                if t * li > tol && (1.0 - t) * li > tol && u * lj > tol && (1.0 - u) * lj > tol {
                    out.push((i, j, t, u));
                }
            }
        }
        out
    };
    let crossings_in = |f: usize, i: usize, curves: &[R<FaceCurves>]| -> Crossings { crossings_with(f, i, &mut (i + 1..cx.face_segs[f].len()), curves) };
    // (With the rotation: a curve whose ends and middle turn back onto a curve in the first period (on a copied
    // tool's face, onto its first member's face; on a face the rotation keeps, onto the face's own) crosses where
    // that one does, the curves it crosses turned onto this face's (each pair kept by the earlier of its two, in
    // the order found); those first curves are crossed with all their face's others for that. The rest as
    // before.)
    let seg_map = |f: usize| -> HashMap<[u32; 2], Vec<usize>> {
        let mut m: HashMap<[u32; 2], Vec<usize>> = HashMap::default();
        for (i, s1) in cx.face_segs[f].iter().enumerate() {
            m.entry(s1.v).or_default().push(i);
        }
        m
    };
    let maps: Vec<HashMap<[u32; 2], Vec<usize>>> = par_map(cx.faces.len(), &|f| if matches!(&curves[f], Ok((c2, _)) if !c2.is_empty()) && sym.is_some() { seg_map(f) } else { HashMap::default() });
    // The curve of face `to` that face `from`'s curve `i` turns into by `g` (its ends and middle), if any.
    let turn_seg = |to: usize, from: usize, i: usize, g: &Rigid| -> Option<usize> {
        let s1 = &cx.face_segs[from][i];
        let v = [cx.pool.find(g.point(cx.p(s1.v[0]))).ok()??, cx.pool.find(g.point(cx.p(s1.v[1]))).ok()??];
        let mid = g.point(cx.seg_at(s1, 0.5));
        maps[to].get(&v)?.iter().copied().find(|&k| std::mem::discriminant(&cx.face_segs[to][k].g) == std::mem::discriminant(&s1.g) && (cx.seg_at(&cx.face_segs[to][k], 0.5) - mid).length() <= tol)
    };
    let period_of = |f: usize, i: usize| sym.as_ref().and_then(|sym| sym.period(cx.seg_at(&cx.face_segs[f][i], 0.5), size));
    // (A family's first member's face: any of its curves may be another's image.)
    let first_face = |f: usize| sym.is_some() && cx.copies.get(cx.faces[f].arg as usize).copied().flatten().is_some_and(|(k, _)| cx.families[k][0].0 == cx.faces[f].arg as usize);
    // Where a curve's crossings come from: found with all its face's curves (an image), from an image (the face
    // and the steps), or found as before.
    enum From {
        All,
        Image(usize, usize),
        Own,
    }
    let from_of = |f: usize, i: usize| -> From {
        if let Some((f0, j)) = copy_step(f) {
            return From::Image(f0, j);
        }
        if first_face(f) {
            return From::All;
        }
        match period_of(f, i) {
            Some(0) => From::All,
            Some(j) => From::Image(f, j),
            None => From::Own,
        }
    };
    let found_first = par_map(items.len(), &|k| {
        let (f, i) = items[k];
        match from_of(f, i) {
            From::All => Some(crossings_with(f, i, &mut (0..cx.face_segs[f].len()).filter(|&j| j != i), &curves)),
            From::Image(..) => None,
            From::Own => Some(crossings_in(f, i, &curves)),
        }
    });
    let mut item_of: HashMap<(usize, usize), usize> = HashMap::default();
    for (k, &x) in items.iter().enumerate() {
        item_of.insert(x, k);
    }
    let turned_crossings = |k: usize| -> Option<Crossings> {
        let sym = sym.as_ref()?;
        let (f, i) = items[k];
        let From::Image(f0, j) = from_of(f, i) else { return None };
        let i0 = turn_seg(f0, f, i, &sym.turn(-(j as f64)))?;
        let k0 = *item_of.get(&(f0, i0))?;
        if !matches!(from_of(f0, i0), From::All) {
            return None;
        }
        let all0 = found_first[k0].as_ref()?;
        let g = sym.turn(j as f64);
        let mut out = Vec::new();
        for &(_, k0, t, u) in all0 {
            // (A curve there split here: the one of the curves at its turned ends the crossing lies on.)
            let (k1, u) = match turn_seg(f, f0, k0, &g) {
                Some(k1) => (k1, u),
                None => {
                    let s0 = &cx.face_segs[f0][k0];
                    let q = g.point(cx.seg_at(s0, u));
                    let ends: Vec<u32> = s0.v.iter().filter_map(|&w| cx.pool.find(g.point(cx.p(w))).ok().flatten()).collect();
                    let (k1, (u1, _)) = (0..cx.face_segs[f].len()).filter(|&k| cx.face_segs[f][k].v.iter().any(|w| ends.contains(w))).map(|k| (k, cx.seg_param(&cx.face_segs[f][k], q))).find(|x| x.1.1 <= tol && x.1.0 > 0.0 && x.1.0 < 1.0)?;
                    (k1, u1)
                }
            };
            if k1 > i {
                out.push((i, k1, t, u));
            }
        }
        out.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.total_cmp(&b.2)).then(a.3.total_cmp(&b.3)));
        Some(out)
    };
    let found_rest = par_map(items.len(), &|k| found_first[k].is_none().then(|| turned_crossings(k).unwrap_or_else(|| crossings_in(items[k].0, items[k].1, &curves))));
    let found: Vec<Crossings> = found_first
        .iter()
        .zip(found_rest)
        .enumerate()
        .map(|(k, (a, b))| match a {
            Some(v) => v.iter().copied().filter(|x| x.1 > items[k].1).collect(),
            None => b.expect("a curve's crossings"),
        })
        .collect();
    let mut full: Vec<R<Crossings>> = curves.iter().map(|c| c.as_ref().map(|_| Vec::new()).map_err(|e| *e)).collect();
    for (k, x) in found.into_iter().enumerate() {
        if let Ok(v) = &mut full[items[k].0] {
            v.extend(x);
        }
    }
    // (A face like its first member's: that one's crossings by its own curves, each where the lower of the two in
    // its order has it, in the order its own pairs come.)
    for (f, x) in firsts.iter().enumerate() {
        let Some((f0, to)) = x else { continue };
        let mut from = vec![0; to.len()];
        for (i, &i0) in to.iter().enumerate() {
            from[i0] = i;
        }
        full[f] = match &full[*f0] {
            Ok(v) => {
                let mut m: Crossings = v.iter().map(|&(i0, j0, t, u)| if from[i0] < from[j0] { (from[i0], from[j0], t, u) } else { (from[j0], from[i0], u, t) }).collect();
                m.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.total_cmp(&b.2)).then(a.3.total_cmp(&b.3)));
                Ok(m)
            }
            Err(e) => Err(*e),
        };
    }
    let all_crossings: Vec<R<Vec<(usize, f64)>>> = full.into_iter().map(|x| x.map(|v| v.into_iter().map(|c| (c.0, c.2)).collect())).collect();
    for (f, crossings) in all_crossings.into_iter().enumerate() {
        let segs = cx.face_segs[f].clone();
        for (i, t) in crossings? {
            cx.pool.add(cx.seg_at(&segs[i], t))?;
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
            // (Pieces of an apex: by their angles.)
            let apart = matches!((o.g, s.g), (Geo::Apex { a0, a1 }, Geo::Apex { a0: b0, a1: b1 }) if a0.to_bits() != b0.to_bits() || a1.to_bits() != b1.to_bits());
            // (One course though of two curve kinds (a line and a B-spline of it) or parameterized otherwise (two of
            // OCCT's sections of one curve, their middles apart): one, its ends apart and each one's points along it
            // on the other, as OCCT's common block.)
            let kind = std::mem::discriminant(&o.g) == std::mem::discriminant(&s.g);
            let near_mid = (cx.seg_at(o, 0.5) - mid).length() <= 100.0 * cx.tol;
            // (Within the sum of the two's own tolerances (an OCCT section's: its curve's distance from the faces'
            // section), OCCT's rule for a common block: three faces meeting along one line, a sharp knurl's tooth
            // tip on its cylinder, give curves that far apart.)
            let ctol = |g: &Geo| if let Geo::Nurbs { id, .. } = g { cx.ncurves[*id as usize].tol } else { 0.0 };
            let reach = cx.tol.max(ctol(&o.g) + ctol(&s.g));
            let one_course = || {
                s.v[0] != s.v[1]
                    && !matches!(o.g, Geo::Apex { .. })
                    && !matches!(s.g, Geo::Apex { .. })
                    && [0.25, 0.5, 0.75].iter().all(|&t| cx.seg_param(o, cx.seg_at(&s, t)).1 <= reach)
                    && [0.25, 0.5, 0.75].iter().all(|&t| cx.seg_param(&s, cx.seg_at(o, t)).1 <= reach)
            };
            if !apart && ((kind && near_mid) || one_course()) {
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
    // (Each input edge's split points, on all threads; taken in order.)
    let edge_chain = |e: usize| -> R<Vec<u32>> {
        let es = edge_solid[e];
        let s = cx.edge_seg(e);
        let (lo, hi) = match s.g {
            Geo::Line | Geo::Apex { .. } => (cx.p(s.v[0]).min(cx.p(s.v[1])), cx.p(s.v[0]).max(cx.p(s.v[1]))),
            Geo::Arc { k, .. } => (k.c - DVec3::splat(k.r), k.c + DVec3::splat(k.r)),
            Geo::Ell { e, t0, sweep, .. } => e.piece_box(t0, sweep),
            Geo::Nurbs { id, t0, t1 } => free::piece_box(&cx.ncurves[id as usize].c, t0, t1),
        };
        let near = es != usize::MAX && (0..ns).any(|t| meets[es * ns + t] && boxes_meet(lo, hi, cx.solids[t].lo, cx.solids[t].hi, GREY * tol));
        if near { cx.split_among(&s, &lists, Some(cx.edges[e].arg), cx.edges[e].t) } else { Ok(vec![s.v[0], s.v[1]]) }
    };
    // (With the arrangement the same under a rotation: a curve in a later period whose ends and middle turn back
    // onto a curve's in the first has that one's split points turned, the vertices there; any other found.)
    let by_period = |n: usize, seg_of: &(dyn Fn(usize) -> Seg + Sync), find: &(dyn Fn(usize) -> R<Vec<u32>> + Sync)| -> Vec<R<Vec<u32>>> {
        let Some(sym) = sym.as_ref() else { return par_map(n, find) };
        let period: Vec<Option<usize>> = (0..n).map(|k| sym.period(cx.seg_at(&seg_of(k), 0.5), size)).collect();
        let mut at: HashMap<[u32; 2], Vec<usize>> = HashMap::default();
        for (k, &p) in period.iter().enumerate() {
            if p == Some(0) {
                at.entry(seg_of(k).v).or_default().push(k);
            }
        }
        let first = par_map(n, &|k| period[k].is_none_or(|j| j == 0).then(|| find(k)));
        let turned = |k: usize| -> Option<Vec<u32>> {
            let j = period[k]?;
            let (back, g) = (sym.turn(-(j as f64)), sym.turn(j as f64));
            let s = seg_of(k);
            if !matches!(cx.split(&s, &lone), Ok(c) if c.len() == 2) {
                return None;
            }
            let v = [cx.pool.find(back.point(cx.p(s.v[0]))).ok()??, cx.pool.find(back.point(cx.p(s.v[1]))).ok()??];
            let mid = back.point(cx.seg_at(&s, 0.5));
            let k0 = *at.get(&v)?.iter().find(|&&k0| {
                let s0 = seg_of(k0);
                std::mem::discriminant(&s0.g) == std::mem::discriminant(&s.g) && (cx.seg_at(&s0, 0.5) - mid).length() <= tol
            })?;
            let Some(Ok(chain0)) = &first[k0] else { return None };
            let chain = chain0.iter().map(|&w| cx.pool.find(g.point(cx.p(w))).ok().flatten()).collect::<Option<Vec<u32>>>()?;
            (chain.first() == Some(&s.v[0]) && chain.last() == Some(&s.v[1])).then_some(chain)
        };
        let rest = par_map(n, &|k| first[k].is_none().then(|| turned(k).map_or_else(|| find(k), Ok)));
        first.into_iter().zip(rest).map(|(a, b)| a.or(b).expect("a curve's split points")).collect()
    };
    // A whole cone's apex where curves on the cone come to it (a plane through its axis and apex: two rulings): in
    // pieces between their angles and its seam's, each its own degenerate edge, as OCCT's.
    let apex_parts = |cx: &Ctx, e: usize| -> R<Vec<Seg>> {
        let s = cx.edge_seg(e);
        let w = s.v[0];
        let Some(f) = (0..cx.faces.len()).find(|&f| cx.faces[f].edges.iter().any(|x| x.0 == e)) else { return Ok(vec![s]) };
        let g = &cx.faces[f];
        let Surf::Cyl { r, th0, full, .. } = g.s else { return Ok(vec![s]) };
        let angle = |w: u32| g.to2(cx.p(w)).x / r;
        let mut at: Vec<f64> = Vec::new();
        for c in &cx.face_segs[f] {
            if matches!(c.g, Geo::Apex { .. }) || !c.v.contains(&w) || c.v[0] == c.v[1] {
                continue;
            }
            at.push(angle(if c.v[0] == w { c.v[1] } else { c.v[0] }));
        }
        if at.is_empty() {
            return Ok(vec![s]);
        }
        // Its own way: a whole cone's from its seam round; a part's from the angle of the face's ruling that comes
        // to the apex to that of the one that leaves it (the face's way along the edge), as c2_of has it.
        let (from, to) = if full {
            (th0, th0 + TAU)
        } else {
            let k = g.edges.iter().position(|x| x.0 == e).ok_or(Decline("an apex on a part of a cone"))?;
            let ends = |&(e, rev): &(usize, bool)| if rev { [cx.edges[e].v[1], cx.edges[e].v[0]] } else { cx.edges[e].v };
            let rulings = |end: usize| g.edges.iter().filter(|x| !matches!(cx.edges[x.0].g, Geo::Apex { .. }) && ends(x)[end] == w).map(|x| g.to2(cx.seg_at(&cx.edge_seg(x.0), 0.5)).x / r).collect::<Vec<f64>>();
            let (&[a], &[b]) = (rulings(1).as_slice(), rulings(0).as_slice()) else { return Err(Decline("an apex on a part of a cone")) };
            if g.edges[k].1 { (b, a) } else { (a, b) }
        };
        let (lo, hi) = (from.min(to), from.max(to));
        at.retain(|&t| t - lo > 1e-9 && hi - t > 1e-9);
        if at.is_empty() {
            return Ok(vec![s]);
        }
        at.sort_by(f64::total_cmp);
        at.dedup_by(|x, y| *x - *y <= 1e-9);
        if to < from {
            at.reverse();
        }
        let ends: Vec<f64> = std::iter::once(from).chain(at).chain(std::iter::once(to)).collect();
        Ok(ends.windows(2).map(|x| Seg { v: s.v, g: Geo::Apex { a0: x[0], a1: x[1] } }).collect())
    };
    let edge_chains = by_period(edge_solid.len(), &|e| cx.edge_seg(e), &edge_chain);
    for (e, chain) in edge_chains.into_iter().enumerate() {
        let s = cx.edge_seg(e);
        let chain = chain?;
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
        let parts = if matches!(s.g, Geo::Apex { .. }) { apex_parts(&cx, e)? } else { cx.pieces_of(&s, &chain) };
        for piece in parts {
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
    // vertex's, which rounding decides (4e-16 off: grows), and then its history says Deleted, not Modified. Ours says
    // Modified (OCCT's where nothing rounds; else an equivalent result), and inside coincident edges of three other
    // arguments Deleted when the vertex is one argument's alone (Shell's slabs meeting: four arguments at a line).
    let mut deleted_v: HashSet<u32> = HashSet::default();
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

        // (One argument's vertex alone: one already two arguments' (merged with another's first) stays Modified.)
        if args.len() >= 3 && cx.pool.inputs[w as usize].len() == 1 && es.iter().all(|&e| at(e) == at(es[0])) {
            deleted_v.insert(w);
        }
    }
    // (Where the curves added to faces and the sections are split, on all threads; taken in order.)
    // (A section's split once, for both faces' curves it is.)
    let extra_segs: Vec<(usize, Seg)> = (0..cx.faces.len()).flat_map(|f| cx.face_segs[f].iter().map(move |&s| (f, s))).collect();
    let extra_of: Vec<usize> = (0..cx.faces.len()).flat_map(|f| cx.seg_section[f].iter().copied()).collect();
    let section_chains = by_period(cx.sections.len(), &|k| cx.sections[k].0, &|k| cx.split(&cx.sections[k].0, &by_x));
    let own: Vec<usize> = (0..extra_segs.len()).filter(|&k| extra_of[k] == usize::MAX).collect();
    let mut own_chains = by_period(own.len(), &|i| extra_segs[own[i]].1, &|i| cx.split(&extra_segs[own[i]].1, &by_x)).into_iter();
    let extra_chains: Vec<R<Vec<u32>>> = (0..extra_segs.len()).map(|k| if extra_of[k] == usize::MAX { own_chains.next().expect("a curve's split points") } else { section_chains[extra_of[k]].clone() }).collect();
    let mut face_extra: Vec<Vec<usize>> = vec![Vec::new(); cx.faces.len()];
    for (&(f, s), chain) in extra_segs.iter().zip(extra_chains) {
        let chain = chain?;
        for piece in cx.pieces_of(&s, &chain) {
            face_extra[f].push(get_sedge(&cx, &mut sedges, piece).0);
        }
    }
    // The faces each section edge was made by (OCCT: they generated it).
    let mut section_faces: Vec<(usize, usize)> = Vec::new();
    for (k, chain) in section_chains.into_iter().enumerate() {
        let (s, fs, ft) = cx.sections[k];
        let chain = chain?;
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
    // (Each face's on all threads, applied in order: its pieces, whether it is split, the vertices inside it.)
    // A face's split curves (in order) and whether it is split; its pieces: whether split, its vertices inside, the
    // pieces.
    type FacePieces = (bool, Vec<u32>, Vec<Piece>);
    let face_curves = |f: usize| -> (Vec<usize>, bool) {
        let mut segs: Vec<usize> = cx.faces[f].edges.iter().flat_map(|&(e, _)| edge_pieces[e].iter().map(|x| x.0)).collect();
        segs.sort_unstable();
        segs.dedup();
        let n_boundary = segs.len();
        let boundary_split = cx.faces[f].edges.iter().any(|&(e, _)| edge_pieces[e].len() != 1);
        segs.extend(face_extra[f].iter().copied());
        segs.sort_unstable();
        segs.dedup();
        let split = boundary_split || segs.len() != n_boundary;
        (segs, split)
    };
    let of_face = |f: usize| -> R<FacePieces> {
        let (segs, split) = face_curves(f);
        // A face nothing reaches is one piece as it was (out of every other solid).
        let fs = cx.faces[f].solid;
        let near = involved[fs]
            && (0..ns).any(|t| meets[fs * ns + t] && boxes_meet(cx.faces[f].lo, cx.faces[f].hi, cx.solids[t].lo, cx.solids[t].hi, GREY * tol));
        if !near && !split {
            let loops = vec![cx.faces[f].edges.iter().map(|&(e, r)| (edge_pieces[e][0].0, r ^ edge_pieces[e][0].1)).collect()];
            return Ok((split, Vec::new(), vec![Piece { face: f, loops, inside: DVec3::NAN, dangling: false }]));
        }
        // The curves in the face's parameter plane (a seam's pieces on both sides).
        let seams: HashSet<usize> = {
            let mut seen = HashSet::default();
            cx.faces[f].edges.iter().filter(|&&(e, _)| !seen.insert(e)).flat_map(|&(e, _)| edge_pieces[e].iter().map(|x| x.0)).collect()
        };
        let mut local: HashMap<(u32, bool), u32> = HashMap::default();
        // The apex of a part of a cone: a point per place too.
        let apex_part: Vec<u32> = match cx.faces[f].s {
            // (A whole cone's apex split into pieces too.)
            Surf::Cyl { full, .. } => cx.faces[f]
                .edges
                .iter()
                .filter(|&&(e, _)| matches!(cx.edges[e].g, Geo::Apex { .. }) && (!full || edge_pieces[e].len() > 1))
                .map(|&(e, _)| sedges[edge_pieces[e][0].0].seg.v[0])
                .collect(),
            _ => Vec::new(),
        };
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
                    // A vertex on a seam (or a cone's apex) is at two places in the parameter plane (any other at one,
                    // half a turn from the seam too).
                    let right = match cx.faces[f].s {
                        Surf::Cyl { r, th0, .. } => ((q.x - r * th0).abs() <= tol || (q.x - r * (th0 + TAU)).abs() <= tol) && q.x > r * (th0 + PI),
                        Surf::Plane { .. } | Surf::Nurbs { .. } => false,
                    };
                    // (A part of a cone's apex: at both its rulings' angles.)
                    if apex_part.contains(&w) || matches!(cx.faces[f].s, Surf::Nurbs { .. }) {
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
        let mut out = Vec::new();
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
            let inside3 = cx.faces[f].to3(r.inside);
            let mut loops = vec![r.outer];
            loops.extend(r.holes);
            let loops = loops.into_iter().map(|l| l.into_iter().map(|(s, rev)| (lsrc[s as usize], rev)).collect()).collect();
            out.push(Piece { face: f, loops, inside: inside3, dangling: r.dangling });
        }
        if out.is_empty() {
            return Err(Decline("a face lost"));
        }
        Ok((split, inner, out))
    };
    // (With the arrangement the same under a rotation: a copied tool's face whose split curves are its first
    // member's face's turned, one for one in the same order, has that face's pieces, their curves the ones here,
    // their points turned; its vertices inside likewise. Any other found itself.)
    let first_pieces = par_map(cx.faces.len(), &|f| copy_step(f).is_none().then(|| of_face(f)));
    let turned_pieces = |f: usize| -> Option<FacePieces> {
        let sym = sym.as_ref()?;
        let (f0, j) = copy_step(f)?;
        let Some(Ok((split0, inner0, pieces0))) = &first_pieces[f0] else { return None };
        let ((segs, split), (segs0, _)) = (face_curves(f), face_curves(f0));
        if split != *split0 || segs.len() != segs0.len() {
            return None;
        }
        let g = sym.turn(j as f64);
        let back = sym.turn(-(j as f64));
        // (Each curve here the one of that face its ends and middle turn back to; no seams, so a curve's place in
        // the face's list numbers its half-edges.)
        let mut seen = HashSet::default();
        if cx.faces[f].edges.iter().any(|&(e, _)| !seen.insert(e)) {
            return None;
        }
        let mut at0: HashMap<[u32; 2], Vec<usize>> = HashMap::default();
        for &s0 in &segs0 {
            at0.entry(sedges[s0].seg.v).or_default().push(s0);
        }
        let mut to: HashMap<usize, usize> = HashMap::default();
        for &si in &segs {
            let a = &sedges[si].seg;
            let v = [cx.pool.find(back.point(cx.p(a.v[0]))).ok()??, cx.pool.find(back.point(cx.p(a.v[1]))).ok()??];
            let mid = back.point(cx.seg_at(a, 0.5));
            let s0 = *at0.get(&v)?.iter().find(|&&s0| std::mem::discriminant(&a.g) == std::mem::discriminant(&sedges[s0].seg.g) && (cx.seg_at(&sedges[s0].seg, 0.5) - mid).length() <= tol)?;
            if to.insert(s0, si).is_some() {
                return None;
            }
        }
        let place: HashMap<usize, usize> = segs.iter().enumerate().map(|(k, &si)| (si, k)).collect();
        let half = |&(si, rev): &(usize, bool)| 2 * place[&si] + usize::from(rev);
        let inner = inner0.iter().map(|&w| cx.pool.find(g.point(cx.p(w))).ok().flatten()).collect::<Option<Vec<u32>>>()?;
        // (As the arrangement here would give them: each loop from its least half-edge, the holes and the pieces in
        // the order of theirs.)
        let mut pieces = pieces0
            .iter()
            .map(|p| {
                let mut loops = p
                    .loops
                    .iter()
                    .map(|l| {
                        let mut l = l.iter().map(|&(s0, rev)| Some((*to.get(&s0)?, rev))).collect::<Option<Vec<_>>>()?;
                        let k = (0..l.len()).min_by_key(|&k| half(&l[k]))?;
                        l.rotate_left(k);
                        Some(l)
                    })
                    .collect::<Option<Vec<_>>>()?;
                loops[1..].sort_by_key(|l| half(&l[0]));
                Some(Piece { face: f, loops, inside: if p.inside.is_nan() { p.inside } else { g.point(p.inside) }, dangling: p.dangling })
            })
            .collect::<Option<Vec<Piece>>>()?;
        if pieces.iter().any(|p| p.inside.is_nan()) {
            return None;
        }
        pieces.sort_by_key(|p| half(&p.loops[0][0]));
        Some((split, inner, pieces))
    };
    let rest_pieces = par_map(cx.faces.len(), &|f| first_pieces[f].is_none().then(|| turned_pieces(f).map_or_else(|| of_face(f), Ok)));
    let all_pieces: Vec<R<FacePieces>> = first_pieces.into_iter().zip(rest_pieces).map(|(a, b)| a.or(b).expect("a face's pieces")).collect();
    for (f, x) in all_pieces.into_iter().enumerate() {
        let (split, inner, out) = x?;
        face_split[f] = split;
        for &w in &inner {
            note(&mut touched, &cx.pool, w, cx.faces[f].arg);
        }
        face_inner[f] = inner;
        for p in out {
            face_pieces[f].push(pieces.len());
            pieces.push(p);
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
    // (Each piece's states against the solids, on all threads; taken in order below.)
    let state_of = |pi: usize| -> R<Vec<(usize, State)>> {
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
                // (A free-form face: past its patch the point's parameters are clamped to its edge; off it there,
                // not on that face.)
                if matches!(cx.faces[g].s, Surf::Nurbs { .. }) && free::surface_distance(&cx.faces[g], pieces[pi].inside) > tol {
                    continue;
                }
                let q = cx.faces[g].to2(pieces[pi].inside);
                let d = distance2(q, cx.bound(g)?);
                if d <= GREY * tol {
                    if std::env::var_os("NOSHAPE_X_NM").is_some() {
                        eprintln!("CB d {d:e} tol {tol:e} piece {pi} face {f} arg {arg} at {:?} on face {g} arg {} kinds {:?}/{:?}", pieces[pi].inside, cx.faces[g].arg, std::mem::discriminant(&cx.faces[f].s), std::mem::discriminant(&cx.faces[g].s));
                    }
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
        Ok(states)
    };
    // (With the arrangement the same under a rotation: the pieces of one period by rays, every other piece as the
    // one its point turns back into (on the face its face turns back to, the solids likewise); any not found by
    // rays too.)
    let later = |pi: usize| -> Option<usize> {
        let sym = sym.as_ref()?;
        let p = pieces[pi].inside;
        if p.is_nan() {
            return None;
        }
        sym.period(p, size).filter(|&j| j > 0)
    };
    let first = par_map(pieces.len(), &|pi| if later(pi).is_some() { None } else { Some(state_of(pi)) });
    let cell = 1e-6 * size;
    let key = |f: usize, p: DVec3| (f, (p.x / cell).round() as i64, (p.y / cell).round() as i64, (p.z / cell).round() as i64);
    let mut by_place: HashMap<(usize, i64, i64, i64), Vec<usize>> = HashMap::default();
    for (pi, x) in first.iter().enumerate() {
        if matches!(x, Some(Ok(_))) {
            by_place.entry(key(pieces[pi].face, pieces[pi].inside)).or_default().push(pi);
        }
    }
    // A piece's loops' lengths and its vertices' middle.
    let shape = |pi: usize| -> (Vec<usize>, DVec3) {
        let vs: Vec<u32> = pieces[pi].loops.iter().flatten().map(|&(s, rev)| sedges[s].seg.v[usize::from(rev)]).collect();
        (pieces[pi].loops.iter().map(|l| l.len()).collect(), vs.iter().fold(DVec3::ZERO, |m, &w| m + cx.p(w)) / vs.len().max(1) as f64)
    };
    let turned = |pi: usize| -> Option<Vec<(usize, State)>> {
        let sym = sym.as_ref()?;
        let j = later(pi)?;
        let f0 = sym.face_back[j - 1][pieces[pi].face];
        if f0 == usize::MAX {
            return None;
        }
        let back = sym.turn(-(j as f64));
        let q = back.point(pieces[pi].inside);
        let (_, kx, ky, kz) = key(f0, q);
        let mut found = None;
        'look: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &p0 in by_place.get(&(f0, kx + dx, ky + dy, kz + dz)).map_or(&[][..], |v| v.as_slice()) {
                        // (The same piece turned: its point near, its loops as long, its vertices' middle where
                        // this one's turns back to.)
                        if (pieces[p0].inside - q).length() <= cell && shape(p0).0 == shape(pi).0 && (back.point(shape(pi).1) - shape(p0).1).length() <= 10.0 * tol {
                            found = Some(p0);
                            break 'look;
                        }
                    }
                }
            }
        }
        let Some(Ok(st0)) = &first[found?] else { return None };
        let back = &sym.solid_back[j - 1];
        let arg = cx.faces[pieces[pi].face].arg;
        let mut out = Vec::with_capacity(st0.len());
        for (t, &t0) in back.iter().enumerate() {
            if cx.solids[t].arg == arg {
                continue;
            }
            let i = st0.binary_search_by_key(&t0, |x| x.0).ok()?;
            out.push((t, st0[i].1));
        }
        Some(out)
    };
    let rest = par_map(pieces.len(), &|pi| {
        if first[pi].is_some() {
            None
        } else {
            Some(turned(pi).map_or_else(|| state_of(pi), Ok))
        }
    });
    let all_states: Vec<R<Vec<(usize, State)>>> = first.into_iter().zip(rest).map(|(a, b)| a.or(b).expect("a piece's states")).collect();
    for (pi, states) in all_states.into_iter().enumerate() {
        let states = states?;
        let f = pieces[pi].face;
        let s = cx.faces[f].solid;
        let arg = cx.faces[f].arg;
        within[pi] = states.iter().filter(|x| matches!(x.1, State::In | State::OnSame)).map(|x| x.0).collect();
        for &(t, st) in &states {
            if st != State::Out {
                reached[t] = true;
            }
        }
        let ons: Vec<(usize, State)> = states.iter().copied().filter(|x| matches!(x.1, State::OnSame | State::OnOpposite)).collect();
        if !ons.is_empty() && (cx.solids[s].sheet || ons.iter().any(|&(t, _)| cx.solids[t].sheet)) {
            return Err(Decline("sheets coincident"));
        }
        // (In a cut, a piece on the object and a tool, or an object's piece on two tools sharing a face there (a
        // pattern's end caps on the object's end face): as on one, the object's or the one facing its way.)
        let several_ok = op == Op::Cut && (arg == 0 || ons.iter().any(|&(t, _)| cx.solids[t].arg == 0));
        if ons.len() > 1 && op != Op::Fuse && !several_ok {
            return Err(Decline("a piece on two solids"));
        }
        let in_any = states.iter().any(|x| x.1 == State::In);
        if touching_only && in_any {
            return Err(Decline("pattern instances overlapping"));
        }
        let in_obj = states.iter().any(|x| x.1 == State::In && cx.solids[x.0].arg == 0);
        let in_tool = states.iter().any(|x| x.1 == State::In && cx.solids[x.0].arg != 0);
        if std::env::var("NOSHAPE_X_PIECE").is_ok_and(|v| v.split(',').any(|x| x == pi.to_string())) {
            let nonout: Vec<_> = states.iter().filter(|x| x.1 != State::Out).map(|x| (cx.solids[x.0].arg, x.1)).collect();
            eprintln!("PIECE {pi} face arg {arg} index {} inside {:?} states not out {nonout:?} in_obj {in_obj} in_tool {in_tool}", cx.faces[f].index, pieces[pi].inside);
        }
        let on = if ons.len() > 1 && op == Op::Cut {
            if arg != 0 { ons.iter().find(|&&(t, _)| cx.solids[t].arg == 0) } else { ons.iter().find(|x| x.1 == State::OnSame) }.or(ons.first()).copied()
        } else {
            ons.first().copied()
        };
        // Two tools' pieces coinciding (a cut), facing each other: inside the tools.
        let tools_on = op == Op::Cut && arg != 0 && on.is_some_and(|(t, _)| cx.solids[t].arg != 0);
        if let Some((t, _)) = on {
            // (In a common two tools' pieces coinciding: not taken. In a cut OCCT's record of the object's solid is
            // Modified on every path now, CutByFaces's too (core 2c6e89c2), as ours.)
            if op == Op::Common && arg != 0 && cx.solids[t].arg != 0 {
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
        // (A fuse's: a piece of a face that was split before a whole face, as OCCT's same-domain faces' first.)
        let rank = |p: usize| (op == Op::Fuse && !face_split[pieces[p].face], cx.faces[pieces[p].face].arg);
        if rank(pi) < rank(e.0) {
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
            // (A fuse keeps the group's chosen piece.)
            if op == Op::Fuse && keep[pi].is_some() && keep[o].is_none() {
                keep[o] = keep[pi];
                keep[pi] = None;
            }
        }
    }
    // (A piece an edge ends in: only where it goes, as OCCT's builder leaves such an edge in the face.)
    if pieces.iter().enumerate().any(|(p, x)| x.dangling && (keep[p].is_some() || on_pairs.iter().any(|o| o.0 == p))) {
        return Err(Decline("an edge ends inside a face"));
    }
    // The result's faces and their edges.
    let kept: Vec<usize> = (0..pieces.len()).filter(|&p| keep[p].is_some()).collect();
    if kept.is_empty() {
        // Nothing: everything deleted, as OCCT's (an empty compound).
        let mut out = Output::default();
        for (a, b) in args.iter().enumerate() {
            let del = |kind: Kind, n: usize| (0..n as u32).map(move |i| Record { kind, arg: a as u32, index: i, relation: Relation::Deleted, out_kind: kind, out: u32::MAX });
            out.history.extend(del(Kind::Solid, b.solids.len()).chain(del(Kind::Face, b.faces.len())).chain(del(Kind::Edge, b.edges.len())).chain(del(Kind::Vertex, b.vertices.len())));
        }
        return Ok(out);
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
    // (A cone's apex is used once; a sheet's edges any way.)
    if !sheets && uses.iter().any(|(&s, u)| !matches!(sedges[s].seg.g, Geo::Apex { .. }) && (u[0] != u[1] || u[0] == 0)) {
        if std::env::var_os("NOSHAPE_X_NM").is_some() {
            for (&s, u) in &uses {
                if !matches!(sedges[s].seg.g, Geo::Apex { .. }) && (u[0] != u[1] || u[0] == 0) {
                    let users: Vec<(usize, u32, u32, f64, bool)> = (0..pieces.len())
                        .filter(|&p| pieces[p].loops.iter().flatten().any(|x| x.0 == s))
                        .map(|p| (p, cx.faces[pieces[p].face].arg, cx.faces[pieces[p].face].index, if pieces[p].inside.is_nan() { 0.0 } else { 1.0 }, keep[p].is_some()))
                        .collect();
                    eprintln!("NM sedge {s} {:?} from {:?} to {:?} mid {:?} uses {u:?} origins {:?} pieces (piece, arg, face, -, kept) {users:?}", std::mem::discriminant(&sedges[s].seg.g), cx.p(sedges[s].seg.v[0]), cx.p(sedges[s].seg.v[1]), cx.seg_at(&sedges[s].seg, 0.5), sedges[s].origins);
                    let v = sedges[s].seg.v;
                    for (o, x) in sedges.iter().enumerate() {
                        if o != s && (x.seg.v == v || x.seg.v == [v[1], v[0]] || x.seg.v.contains(&v[0]) && x.seg.v.contains(&v[1])) {
                            eprintln!("   twin {o} {:?} v {:?} mid {:?} d(mid, ours) {:e}", std::mem::discriminant(&x.seg.g), x.seg.v, cx.seg_at(&x.seg, 0.5), cx.seg_param(&sedges[s].seg, cx.seg_at(&x.seg, 0.5)).1);
                        }
                    }
                    for (o, x) in sedges.iter().enumerate() {
                        if o != s && x.seg.v.contains(&v[0]) != x.seg.v.contains(&v[1]) && cx.seg_param(&sedges[s].seg, cx.seg_at(&x.seg, 0.5)).1 < 1e-6 {
                            eprintln!("   overlapping {o} {:?} v {:?} {:?}->{:?}", std::mem::discriminant(&x.seg.g), x.seg.v, cx.p(x.seg.v[0]), cx.p(x.seg.v[1]));
                        }
                    }
                }
            }
        }
        return Err(Decline("a non-manifold result"));
    }
    // Seams: an edge one face uses both ways; any other edge two faces once each.
    for (&s, u) in &uses {
        if !sheets && u[0] > 1 && !matches!(sedges[s].seg.g, Geo::Apex { .. }) {
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
        // (Its tolerance where an input's or merged ones' was more than the Boolean's: the C++ build keeps it.)
        out.body.vertex_tol.push(cx.pool.t[w as usize]);
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
            // (Its vertex made anew, another argument through the apex: a new degenerate edge there, as OCCT's; a
            // piece of it, a cone split through its apex, its angles.)
            Geo::Apex { a0, a1 } => {
                out.edge_apex.resize(out.body.edges.len(), [f64::NAN; 2]);
                out.edge_apex.push([a0, a1]);
                (seg.v, Curve::Degenerate)
            }
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
            Geo::Ell { e, faces, .. } if e.par => {
                let of = |f: usize| (cx.faces[f].arg, cx.faces[f].index);
                (seg.v, Curve::Parabola { center: e.c, normal: e.n, xdir: e.x, focal: e.a, faces: [of(faces[0]), of(faces[1])] })
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
    // Solids, one per shell, outward (a sheet's result: faces alone).
    let mut shell_of: HashMap<usize, usize> = HashMap::default();
    for (k, &p) in kept.iter().enumerate().filter(|_| !sheets) {
        let c = find(&mut comp, k);
        let n = shell_of.len();
        let si = *shell_of.entry(c).or_insert(n);
        if si == out.body.solids.len() {
            out.body.solids.push(Solid { shells: vec![Vec::new()] });
        }
        out.body.solids[si].shells[0].push(piece_out[p]);
    }
    // Shells inside out are voids: of the one solid there is (else declined). (One shell alone is a solid; a shell
    // whose volume by its edges' chords is next to nothing beside its box's (a cone's piece bounded by two planes'
    // edges: the chords span nothing, its sign rounding's) is unclear.)
    let shells = std::mem::take(&mut out.body.solids);
    if shells.len() > 1 {
        for s in &shells {
            let (mut lo, mut hi) = (DVec3::INFINITY, DVec3::NEG_INFINITY);
            for &f in &s.shells[0] {
                for u in out.body.faces[f as usize].loops.iter().flatten() {
                    for &w in &out.body.edges[u.edge as usize].v {
                        lo = lo.min(out.body.vertices[w as usize]);
                        hi = hi.max(out.body.vertices[w as usize]);
                    }
                }
            }
            let size = (hi - lo).max(DVec3::splat(tol));
            if shell_volume(&out.body, &s.shells[0]).abs() <= 1e-6 * size.x * size.y * size.z {
                return Err(Decline("a shell whose orientation is unclear"));
            }
        }
    }
    let one = shells.len() == 1;
    let (solids, voids): (Vec<Solid>, Vec<Solid>) = shells.into_iter().partition(|s| one || shell_volume(&out.body, &s.shells[0]) > 0.0);
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
                Some(_) if deleted_v.contains(&w) => h.push(rec(Kind::Vertex, a, i, Relation::Deleted, Kind::Vertex, u32::MAX)),
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
            // (On an edge lying on the face it is one of the face's vertices on it, not one it made: OCCT's.)
            let through = edges_through.get(&w).map_or(&[][..], |v| &v[..]);
            let (mut across, mut on) = (false, false);
            for &e in through {
                if cx.edge_on_surface(e, f)? {
                    on = true;
                } else {
                    across = true;
                }
            }
            if across && !on {
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
    for &p in kept.iter().filter(|_| !sheets) {
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
        if s.sheet {
            continue;
        }
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
    // An input that generated something isn't deleted (OCCT's BRepTools_History::Merge, as BRepAlgoAPI takes its
    // builder's history: a removed shape with generations stays out of the removed).
    let generated: std::collections::HashSet<(Kind, u32, u32)> = h.iter().filter(|r| r.relation == Relation::Generated).map(|r| (r.kind, r.arg, r.index)).collect();
    h.retain(|r| r.relation != Relation::Deleted || !generated.contains(&(r.kind, r.arg, r.index)));
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
                    Curve::Parabola { center, normal, xdir, focal, .. } => {
                        let y = normal.cross(xdir);
                        let (t0, t1) = ((p - center).dot(y), (q - center).dot(y));
                        (0..=16)
                            .map(|k| {
                                let t = t0 + (t1 - t0) * k as f64 / 16.0;
                                center + xdir * (t * t / (4.0 * focal)) + y * t
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
