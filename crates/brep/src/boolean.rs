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

use crate::arrange::regions;
use crate::curve2::{distance as distance2, inside as inside2, C2};
use crate::{Body, Curve, Decline, Edge, Face, Kind, Origin, Output, Record, Relation, Solid, Surface, Use, GREY};
use glam::{DVec2, DVec3};
use crate::fxhash::{HashMap, HashSet};
use std::f64::consts::{PI, TAU};

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

/// A curve between two pool vertices: a segment, or an arc from angle `a0` through `sweep` (> 0) of its
/// circle.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Geo {
    Line,
    Arc { k: Circ, a0: f64, sweep: f64 },
    /// No curve: an edge at a cone's apex.
    Apex,
}

#[derive(Clone, Copy, Debug)]
struct Seg {
    v: [u32; 2],
    g: Geo,
}

enum Surf {
    /// Outward normal, offset (`n · x == d`), and the 2D frame (`u × v == n`).
    Plane { n: DVec3, d: f64, origin: DVec3, u: DVec3, v: DVec3 },
    /// A cylinder or cone: axis origin and direction, angle frame, radius `r0 + k h` at height h along the
    /// axis; `flip`: the outward normal points to the axis. Parameters (r θ, ±h) with θ in [th0, th0 + 2π)
    /// and `r` a scale (a cylinder's radius, a cone's largest on the face); `full`: the face goes round (a
    /// seam at th0).
    Cyl { o: DVec3, a: DVec3, x: DVec3, y: DVec3, r: f64, r0: f64, k: f64, flip: bool, th0: f64, full: bool },
}

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
        }
    }

    fn to3(&self, q: DVec2) -> DVec3 {
        match self.s {
            Surf::Plane { origin, u, v, .. } => origin + u * q.x + v * q.y,
            Surf::Cyl { o, a, x, y, r, r0, k, flip, .. } => {
                let th = q.x / r;
                let h = if flip { -q.y } else { q.y };
                o + a * h + (x * th.cos() + y * th.sin()) * (r0 + k * h)
            }
        }
    }

    /// The curve `g` from `p0` (in 3D) in this face's parameter plane, `copy` 1 for the right side of a seam.
    fn curve2(&self, p0: DVec3, p1: DVec3, g: &Geo, copy: usize) -> R<C2> {
        match (&self.s, g) {
            (Surf::Plane { .. }, Geo::Line) => Ok(C2::Seg { a: self.to2(p0), b: self.to2(p1) }),
            (Surf::Plane { .. }, Geo::Apex) => Err(Decline("an apex on a plane")),
            (Surf::Plane { n, .. }, Geo::Arc { k, sweep, .. }) => {
                let c = self.to2(k.c);
                let d = self.to2(p0) - c;
                let s = if k.n.dot(*n) > 0.0 { *sweep } else { -*sweep };
                Ok(C2::Arc { c, r: k.r, a0: d.y.atan2(d.x), sweep: s })
            }
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
                let seam = *full && (q0.x - r * th0).abs() <= 1e-9 * r.max(1.0);
                if seam && copy == 1 {
                    q0.x = r * (th0 + TAU);
                }
                match g {
                    Geo::Apex => unreachable!(),
                    Geo::Line => {
                        let q1 = DVec2::new(q0.x, self.to2(p1).y);
                        Ok(C2::Seg { a: q0, b: q1 })
                    }
                    Geo::Arc { k, sweep, .. } => {
                        let along = if k.n.dot(*a) > 0.0 { 1.0 } else { -1.0 };
                        let ds = along * sweep * r;
                        if seam && ds < 0.0 {
                            q0.x = r * (th0 + TAU);
                        }
                        let x1 = q0.x + ds;
                        if x1 < r * th0 - 1e-9 * r || x1 > r * (th0 + TAU) + 1e-9 * r {
                            return Err(Decline("an arc across a cylinder's seam"));
                        }
                        Ok(C2::Seg { a: q0, b: DVec2::new(x1, q0.y) })
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

/// A curve on two faces' surfaces, as parameters: a line `p + t d`, or a circle by angle.
#[derive(Clone, Copy)]
enum Path {
    Line { p: DVec3, d: DVec3 },
    Circle(Circ),
}

impl Path {
    fn at(&self, t: f64) -> DVec3 {
        match self {
            Path::Line { p, d } => *p + *d * t,
            Path::Circle(k) => k.at(t),
        }
    }

    fn geo(&self, t0: f64, t1: f64) -> Geo {
        match self {
            Path::Line { .. } => Geo::Line,
            Path::Circle(k) => Geo::Arc { k: *k, a0: t0, sweep: t1 - t0 },
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

struct Ctx {
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

impl Ctx {
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
        }
    }

    /// Face `f`'s boundary in its parameter plane.
    fn bound(&self, f: usize) -> R<&[C2]> {
        let make = || -> R<Vec<C2>> {
            let mut bound = Vec::with_capacity(self.faces[f].edges.len());
            let mut seen: HashMap<usize, usize> = HashMap::default();
            for &(e, _) in &self.faces[f].edges {
                let s = self.edge_seg(e);
                let copy = seen.entry(e).or_insert(0);
                bound.push(self.faces[f].curve2(self.p(s.v[0]), self.p(s.v[1]), &s.g, *copy)?);
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
            Geo::Apex => self.p(s.v[0]),
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
            Geo::Apex => (0.0, (p - self.p(s.v[0])).length()),
        }
    }

    fn seg_length(&self, s: &Seg) -> f64 {
        match s.g {
            Geo::Line => (self.p(s.v[1]) - self.p(s.v[0])).length(),
            Geo::Arc { k, sweep, .. } => k.r * sweep,
            Geo::Apex => 0.0,
        }
    }

    fn edge_seg(&self, e: usize) -> Seg {
        Seg { v: self.edges[e].v, g: self.edges[e].g }
    }

    /// The intervals of `path` (parameters `t0..t1`) inside face `f` (with its boundary when `closed`).
    /// `ends`: the path's own end vertices, if it has them.
    fn intervals(&mut self, f: usize, path: &Path, t0: f64, t1: f64, ends: [End; 2], closed: bool) -> R<Vec<Ival>> {
        let tol = self.tol;
        let len = match path {
            Path::Line { .. } => t1 - t0,
            Path::Circle(k) => (t1 - t0) * k.r,
        };
        if len <= tol {
            return Ok(Vec::new());
        }
        let geo = path.geo(t0, t1);
        let c2 = self.faces[f].curve2(path.at(t0), path.at(t1), &geo, 0)?;
        // Where it meets the face's boundary.
        let mut cuts: Vec<(f64, End)> = vec![(t0, ends[0]), (t1, ends[1])];
        let bound = self.bound(f)?.to_vec();
        for b in &bound {
            for (u, _) in c2.intersections(b, tol) {
                let t = t0 + (t1 - t0) * u;
                cuts.push((t, End::Point(path.at(t))));
            }
        }
        // The face's vertices on it (where it runs along the boundary).
        for k in 0..self.faces[f].edges.len() {
            let (e, _) = self.faces[f].edges[k];
            for w in self.edges[e].v {
                let q = self.faces[f].to2(self.p(w));
                let (u, dist) = c2.param_of(q);
                if dist <= tol && u > 0.0 && u < 1.0 {
                    cuts.push((t0 + (t1 - t0) * u, End::Vertex(w)));
                }
            }
        }
        cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let scale = len / (t1 - t0);
        // Cuts at one place are one, a vertex if any of them is.
        let mut merged: Vec<(f64, End)> = Vec::with_capacity(cuts.len());
        for c in cuts {
            match merged.last_mut() {
                Some(last) if (c.0 - last.0) * scale <= tol => {
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
            if (b.0 - a.0) * scale <= tol {
                continue;
            }
            let mid = c2.at(((a.0 + b.0) * 0.5 - t0) / (t1 - t0));
            let dist = distance2(mid, &bound);
            let inside = if dist <= tol {
                closed
            } else if dist <= GREY * tol {
                return Err(Decline("a curve nearly along a face's boundary"));
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
        let tol = self.tol;
        // A circle in parts between the faces' seams (a vertex where it crosses one).
        let mut cuts: Vec<(f64, End)> = vec![(t0, End::Open)];
        if let Path::Circle(k) = path {
            for f in [fs, ft] {
                let Some(p) = self.seam_at(f, path) else { continue };
                let t = t0 + (k.angle(p) - t0).rem_euclid(TAU);
                if (t - t0) * k.r <= tol || (t1 - t) * k.r <= tol {
                    cuts[0].1 = End::Point(p);
                } else {
                    cuts.push((t, End::Point(p)));
                }
            }
            cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        for i in 0..cuts.len() {
            let (a, ea) = cuts[i];
            let (b, eb) = if i + 1 < cuts.len() { cuts[i + 1] } else { (t1, cuts[0].1) };
            self.section_part(fs, ft, path, a, b, [ea, eb])?;
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
        let scale = match path {
            Path::Line { .. } => 1.0,
            Path::Circle(k) => k.r,
        };
        // The end of a section: the tighter of the two faces' ends, made a vertex.
        let pick = |cx: &mut Ctx, x: (f64, End), y: (f64, End), lo: bool| -> R<Option<u32>> {
            let (later, earlier) = if x.0 > y.0 { (x, y) } else { (y, x) };
            let end = if (later.0 - earlier.0) * scale > tol {
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
            if (later.0 - earlier.0) * scale <= tol {
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
                if (hi - lo) * scale <= tol {
                    continue;
                }
                let vlo = pick(self, x[0], y[0], true)?;
                let vhi = pick(self, x[1], y[1], false)?;
                let (Some(vlo), Some(vhi)) = (vlo, vhi) else { return Err(Decline("a section without ends")) };
                let seg = Seg { v: [vlo, vhi], g: path.geo(lo, hi) };
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
            Geo::Apex => return Ok(()),
        };
        // An arc across a round face's seam: in two parts, a vertex where it crosses.
        let mut parts = vec![(t0, End::Vertex(s.v[0]), t1, End::Vertex(s.v[1]))];
        if let (Path::Circle(k), Some(seam)) = (path, self.seam_at(f, path)) {
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
    fn seam_at(&self, f: usize, path: Path) -> Option<DVec3> {
        let (Path::Circle(k), Surf::Cyl { o, a, x, y, r0, k: slope, th0, full: true, .. }) = (path, &self.faces[f].s) else { return None };
        let h = (k.c - *o).dot(*a);
        Some(*o + *a * h + (*x * th0.cos() + *y * th0.sin()) * (r0 + slope * h))
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
        }
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
            return Err(Decline("a plane at an angle to a cylinder"));
        }
        if slope != 0.0 {
            return Err(Decline("a plane along a cone"));
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

    /// The section along cylinder face `fc`'s generator at angle `th` (in its frame), with face `f`.
    fn generator(&mut self, f: usize, fc: usize, th: f64) -> R<()> {
        let Surf::Cyl { o, a, x, y, r, th0, full, .. } = self.faces[fc].s else { unreachable!() };
        if full && ((th - th0).rem_euclid(TAU).min((th0 - th).rem_euclid(TAU))) * r <= GREY * self.tol {
            return Err(Decline("a section along a cylinder's seam"));
        }
        let p = o + (x * th.cos() + y * th.sin()) * r;
        let (lo, hi) = (self.faces[f].lo.max(self.faces[fc].lo), self.faces[f].hi.min(self.faces[fc].hi));
        let (t0, t1) = box_range(p, a, lo, hi, self.tol);
        if t0 >= t1 {
            return Ok(());
        }
        self.section(f, fc, Path::Line { p, d: a }, t0, t1)
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
        if meet { Err(Decline("faces touching along a line")) } else { Ok(()) }
    }

    fn cyl_cyl(&mut self, f1: usize, f2: usize) -> R<()> {
        let Surf::Cyl { o: o1, a: a1, x, y, r: r1, r0: r01, k: k1, th0, .. } = self.faces[f1].s else { unreachable!() };
        let Surf::Cyl { o: o2, a: a2, r: r2, r0: r02, k: k2, .. } = self.faces[f2].s else { unreachable!() };
        let tol = self.tol;
        if a1.cross(a2).length() > 1e-12 {
            return Err(Decline("cylinders at an angle"));
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
                let mut hits: Vec<f64> = Vec::new();
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
                        hits.push(dist / den);
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
                        hits.extend(roots.into_iter().filter(|t| r0 + k * (h + t * dz) > -tol));
                    }
                }
                let gb = if hits.is_empty() { &[][..] } else { self.bound(f)? };
                for t in hits {
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
                    let q = g.to2(p + r * t);
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
    };
    // One axis for every cylinder, every plane square or parallel to it.
    let mut axis: Option<DVec3> = None;
    for body in args {
        for f in &body.faces {
            if let Surface::Cylinder { axis: a, .. } = f.surface {
                match axis {
                    None => axis = Some(a),
                    Some(b) if a.cross(b).length() > 1e-12 => return Err(Decline("cylinders at an angle")),
                    _ => {}
                }
            }
        }
    }
    if let Some(a) = axis {
        for body in args {
            for f in &body.faces {
                if let Surface::Plane { normal, .. } = f.surface {
                    let c = normal.dot(a).abs();
                    if c > 1e-12 && c < 1.0 - 1e-12 {
                        return Err(Decline("a plane at an angle to a cylinder"));
                    }
                }
            }
        }
    }
    // Vertices, edges, faces and solids of every argument (a vertex outside every other argument's box can't
    // be one of theirs).
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
                Surface::Plane { origin, normal } => {
                    let n = if f.reversed { -normal } else { normal };
                    let (u, v) = frame(n);
                    Surf::Plane { n, d: n.dot(origin), origin, u, v }
                }
                Surface::Cylinder { origin, axis, xdir, radius } => round(&cx, &edges, [origin, axis, xdir], radius, 0.0, f.reversed),
                Surface::Cone { origin, axis, xdir, radius, angle } => round(&cx, &edges, [origin, axis, xdir], radius, angle.tan(), f.reversed),
            };
            cx.faces.push(GFace { arg: a, index: i as u32, solid: usize::MAX, s, edges, bound: Default::default(), lo: DVec3::INFINITY, hi: DVec3::NEG_INFINITY });
        }
        for (i, s) in body.solids.iter().enumerate() {
            if s.shells.len() != 1 {
                return Err(Decline("a solid with voids"));
            }
            let si = cx.solids.len();
            let faces: Vec<usize> = s.shells[0].iter().map(|&f| face0 + f as usize).collect();
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
        if body.solids.is_empty() {
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
                Geo::Arc { sweep, .. } => (sweep / (PI / 8.0)).ceil() as usize + 1,
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
    let mut edge_inner: Vec<Vec<u32>> = Vec::with_capacity(cx.edges.len());
    let mut edge_pieces: Vec<Vec<(usize, bool)>> = Vec::with_capacity(cx.edges.len());
    for (e, &es) in edge_solid.iter().enumerate() {
        let s = cx.edge_seg(e);
        let (lo, hi) = match s.g {
            Geo::Line | Geo::Apex => (cx.p(s.v[0]).min(cx.p(s.v[1])), cx.p(s.v[0]).max(cx.p(s.v[1]))),
            Geo::Arc { k, .. } => (k.c - DVec3::splat(k.r), k.c + DVec3::splat(k.r)),
        };
        let near = es != usize::MAX && (0..ns).any(|t| meets[es * ns + t] && boxes_meet(lo, hi, cx.solids[t].lo, cx.solids[t].hi, GREY * tol));
        let chain = if near { cx.split_among(&s, &lists, Some(cx.edges[e].arg))? } else { vec![s.v[0], s.v[1]] };
        for &w in &chain {
            note(&mut touched, &cx.pool, w, cx.edges[e].arg);
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
        let mut pts: Vec<DVec2> = Vec::new();
        let mut lsegs: Vec<([u32; 2], C2)> = Vec::with_capacity(segs.len());
        let mut lsrc: Vec<usize> = Vec::with_capacity(segs.len());
        for &si in &segs {
            let s = sedges[si].seg;
            for copy in 0..if seams.contains(&si) { 2 } else { 1 } {
                let c = cx.faces[f].curve2(cx.p(s.v[0]), cx.p(s.v[1]), &s.g, copy)?;
                let mut ends = [0u32; 2];
                for (k, (w, q)) in [(s.v[0], c.start()), (s.v[1], c.end())].into_iter().enumerate() {
                    // A vertex on a seam (or a cone's apex) is at two places in the parameter plane.
                    let right = match cx.faces[f].s {
                        Surf::Cyl { r, th0, .. } => q.x > r * (th0 + PI),
                        Surf::Plane { .. } => false,
                    };
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
        for r in regions(&pts, &lsegs)? {
            let dist = distance2(r.inside, cx.bound(f)?);
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
        let ons: Vec<(usize, State)> = states.iter().copied().filter(|x| matches!(x.1, State::OnSame | State::OnOpposite)).collect();
        if ons.len() > 1 && op != Op::Fuse {
            return Err(Decline("a piece on two solids"));
        }
        let in_any = states.iter().any(|x| x.1 == State::In);
        let in_obj = states.iter().any(|x| x.1 == State::In && cx.solids[x.0].arg == 0);
        let in_tool = states.iter().any(|x| x.1 == State::In && cx.solids[x.0].arg != 0);
        let on = ons.first().copied();
        if let Some((t, _)) = on {
            if op != Op::Fuse && arg != 0 && cx.solids[t].arg != 0 {
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
    let mut vmap: HashMap<u32, u32> = HashMap::default();
    let mut emap: HashMap<usize, u32> = HashMap::default();
    let mut used_s: Vec<usize> = uses.keys().copied().collect();
    used_s.sort_unstable();
    let mut used_v: Vec<u32> = used_s.iter().flat_map(|&s| sedges[s].seg.v).collect();
    used_v.sort_unstable();
    used_v.dedup();
    let vertex_same = |w: u32| cx.pool.inputs[w as usize].len() == 1 && !touched[w as usize];
    for &w in &used_v {
        vmap.insert(w, out.body.vertices.len() as u32);
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
    let mut sedge_flip: HashMap<usize, bool> = HashMap::default();
    for &s in &used_s {
        let x = &sedges[s];
        // Pieces of an input edge run its way.
        let flip = match x.origins.first() {
            Some(&e) => edge_pieces[e].iter().find(|p| p.0 == s).is_some_and(|p| p.1),
            None => false,
        };
        sedge_flip.insert(s, flip);
        let seg = x.seg;
        let (v, curve) = match seg.g {
            Geo::Apex if !edge_same[s] => return Err(Decline("a cone's apex changed")),
            Geo::Apex => (seg.v, Curve::Degenerate),
            Geo::Line => (if flip { [seg.v[1], seg.v[0]] } else { seg.v }, Curve::Line),
            Geo::Arc { k, .. } => {
                let (v, n) = if flip { ([seg.v[1], seg.v[0]], -k.n) } else { (seg.v, k.n) };
                (v, Curve::Circle { center: k.c, normal: n, xdir: k.x, radius: k.r })
            }
        };
        emap.insert(s, out.body.edges.len() as u32);
        out.body.edges.push(Edge { v: [vmap[&v[0]], vmap[&v[1]]], curve });
        out.edge_origin.push(match x.origins.first() {
            None => Origin::New,
            Some(&e) => {
                let g = &cx.edges[e];
                if edge_same[s] { Origin::Same { arg: g.arg, index: g.index } } else { Origin::Piece { arg: g.arg, index: g.index } }
            }
        });
    }
    let mut piece_out: HashMap<usize, u32> = HashMap::default();
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
                let mut l: Vec<Use> = l[start..].iter().chain(&l[..start]).map(|&(s, r)| Use { edge: emap[&s], reversed: r ^ sedge_flip[&s] }).collect();
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
        out.body.faces.push(Face { surface: src.surface, reversed: src.reversed ^ flip, loops });
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
        out.body.solids[si].shells[0].push(piece_out[&p]);
    }
    for s in &out.body.solids {
        if shell_volume(&out.body, &s.shells[0]) <= 0.0 {
            return Err(Decline("a shell inside out (a void)"));
        }
    }
    // A result solid that is an input solid with all its faces as they were is that solid.
    let face_index: HashMap<(u32, u32), usize> = cx.faces.iter().enumerate().map(|(g, f)| ((f.arg, f.index), g)).collect();
    let mut solid_of_face = vec![0u32; out.body.faces.len()];
    for (k, so) in out.body.solids.iter().enumerate() {
        for &f in &so.shells[0] {
            solid_of_face[f as usize] = k as u32;
        }
        // Every face the input solid's own, unchanged, and all of them.
        let solid_of = |o: Origin| match o {
            Origin::Same { arg, index } => face_index.get(&(arg, index)).map(|&g| cx.faces[g].solid),
            _ => None,
        };
        let first = solid_of(out.face_origin[so.shells[0][0] as usize]);
        let same = first.filter(|&si| cx.solids[si].faces.len() == so.shells[0].len() && so.shells[0].iter().all(|&f| solid_of(out.face_origin[f as usize]) == Some(si)));
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
            if let Some(&o) = piece_out.get(&p) {
                outs.push(o);
            } else if let Some(o) = partner[p].and_then(|q| piece_out.get(&q).copied()) {
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
        let outs: Vec<u32> = edge_pieces[e].iter().filter_map(|p| emap.get(&p.0).copied()).collect();
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
        let o = vmap.get(&w).copied();
        for &(a, i) in &cx.pool.inputs[w as usize] {
            match o {
                None => h.push(rec(Kind::Vertex, a, i, Relation::Deleted, Kind::Vertex, u32::MAX)),
                Some(o) if !vertex_same(w) => h.push(rec(Kind::Vertex, a, i, Relation::Modified, Kind::Vertex, o)),
                _ => {}
            }
        }
    }
    for &(s, f) in &section_faces {
        if let (true, Some(&o)) = (sedges[s].origins.is_empty(), emap.get(&s)) {
            h.push(rec(Kind::Face, cx.faces[f].arg, cx.faces[f].index, Relation::Generated, Kind::Edge, o));
        }
    }
    for (e, inner) in edge_inner.iter().enumerate() {
        for &w in inner {
            if let (true, Some(&o)) = (cx.pool.inputs[w as usize].is_empty(), vmap.get(&w)) {
                h.push(rec(Kind::Edge, cx.edges[e].arg, cx.edges[e].index, Relation::Generated, Kind::Vertex, o));
            }
        }
    }
    for (f, inner) in face_inner.iter().enumerate() {
        for &w in inner {
            if let (true, Some(&o)) = (cx.pool.inputs[w as usize].is_empty(), vmap.get(&w)) {
                h.push(rec(Kind::Face, cx.faces[f].arg, cx.faces[f].index, Relation::Generated, Kind::Vertex, o));
            }
        }
    }
    // Solids: an input solid became the result's solids that are its images (OCCT's): of a cut's object or
    // of a common's arguments, the solids holding its pieces or pieces inside it; a fuse's are deleted (the
    // fused solid is no one's image). One left as it was is kept.
    let mut solid_sources: Vec<Vec<usize>> = vec![Vec::new(); out.body.solids.len()];
    for (&p, &o) in &piece_out {
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
    Ok(out)
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
    best.map(|(end, w)| end + w * 0.5)
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
