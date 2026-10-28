//! Fuse, cut and common of solids bounded by planes, with OCCT's result and history (see the crate docs).
//!
//! 1. Every vertex goes into one pool; points within `tol` of each other are one vertex.
//! 2. Each pair of faces of two solids from different arguments: crossing faces add the segment where both
//!    meet (the two faces' crossings of the line where their planes meet, overlapped); an edge lying in the
//!    other face's plane adds its pieces inside that face; coplanar faces add each other's edges.
//! 3. Every edge and added segment is split at the pool's vertices on it.
//! 4. Each face is split into the regions its pieces bound; a region inside the face is a piece of it.
//! 5. Each piece is in, out of, or on (coincident with a face of) each other solid; the op keeps some.
//! 6. The kept pieces make closed shells, one solid each.

use crate::arrange::{inside, regions, segment_distance};
use crate::{Body, Curve, Decline, Edge, Face, Kind, Origin, Output, Record, Relation, Solid, Surface, Use, GREY};
use glam::{DVec2, DVec3};
use std::collections::HashMap;

/// The Boolean operation: `args[0]` is the object, the other arguments the tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Fuse,
    Cut,
    Common,
}

type R<T> = Result<T, Decline>;

struct GFace {
    arg: u32,
    index: u32,
    solid: usize,
    /// Outward normal, plane offset (`n · x == d`), and the plane's 2D frame (`u × v == n`).
    n: DVec3,
    d: f64,
    origin: DVec3,
    u: DVec3,
    v: DVec3,
    /// Its input boundary, every loop together.
    edges: Vec<(usize, bool)>,
    lo: DVec3,
    hi: DVec3,
}

impl GFace {
    fn to2(&self, p: DVec3) -> DVec2 {
        let q = p - self.origin;
        DVec2::new(q.dot(self.u), q.dot(self.v))
    }
}

struct GEdge {
    arg: u32,
    index: u32,
    v: [u32; 2],
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
        let mut found: Option<(f64, u32)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(list) = self.grid.get(&(k.0 + dx, k.1 + dy, k.2 + dz)) else { continue };
                    for &i in list {
                        let d = (self.p[i as usize] - p).length();
                        if d <= self.tol {
                            if found.is_some_and(|f| f.1 != i) {
                                return Err(Decline("a point near two vertices"));
                            }
                            found = Some((d, i));
                        } else if d <= GREY * self.tol {
                            return Err(Decline("two vertices nearly coincide"));
                        }
                    }
                }
            }
        }
        Ok(found.map(|f| f.1))
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

/// A split edge: a segment between two pool vertices, no pool vertex inside it.
struct SEdge {
    v: [u32; 2],
    /// The input edges it is (a piece of).
    origins: Vec<usize>,
}

/// A piece of an input face.
struct Piece {
    face: usize,
    loops: Vec<Vec<(usize, bool)>>,
    inside: DVec3,
}

/// An end of a face's crossing of a plane: one of its vertices, or where its edge crosses the plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Vertex(u32),
    Cross(usize, usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Out,
    In,
    OnSame,
    OnOpposite,
}

struct Ctx {
    tol: f64,
    pool: Pool,
    faces: Vec<GFace>,
    edges: Vec<GEdge>,
    solids: Vec<GSolid>,
    /// Pool vertices of each face's boundary.
    face_vertices: Vec<Vec<u32>>,
    side_cache: HashMap<(u32, usize), i8>,
    cross_cache: HashMap<(usize, usize), u32>,
    in_face_done: std::collections::HashSet<(usize, usize)>,
    /// Segments added to each face (pool vertex pairs).
    face_segs: Vec<Vec<[u32; 2]>>,
    coplanar: Vec<(usize, usize)>,
    /// Where two faces cross (the segment, the faces).
    sections: Vec<([u32; 2], usize, usize)>,
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
    /// The side of plane `f` vertex `v` is on: 0 within tol.
    fn side(&mut self, v: u32, f: usize) -> R<i8> {
        if let Some(&s) = self.side_cache.get(&(v, f)) {
            return Ok(s);
        }
        let g = &self.faces[f];
        let d = g.n.dot(self.pool.p[v as usize]) - g.d;
        let s = if d.abs() <= self.tol {
            0
        } else if d.abs() <= GREY * self.tol {
            return Err(Decline("a vertex nearly on a plane"));
        } else if d > 0.0 {
            1
        } else {
            -1
        };
        self.side_cache.insert((v, f), s);
        Ok(s)
    }

    fn edge_points(&self, e: usize) -> (DVec3, DVec3) {
        let v = self.edges[e].v;
        (self.pool.p[v[0] as usize], self.pool.p[v[1] as usize])
    }

    /// Where edge `e` (its ends strictly on both sides) crosses face `f`'s plane.
    fn crossing(&mut self, e: usize, f: usize) -> R<u32> {
        if let Some(&v) = self.cross_cache.get(&(e, f)) {
            return Ok(v);
        }
        let (a, b) = self.edge_points(e);
        let g = &self.faces[f];
        let (da, db) = (g.n.dot(a) - g.d, g.n.dot(b) - g.d);
        let p = a + (b - a) * (da / (da - db));
        let v = self.pool.add(p)?;
        self.cross_cache.insert((e, f), v);
        Ok(v)
    }

    /// The intervals where face `f` crosses face `g`'s plane, along `dir`: (t, end) pairs, an end a vertex
    /// or an edge crossing the plane (made a vertex only when a section ends there).
    fn intervals(&mut self, f: usize, g: usize, dir: DVec3) -> R<Vec<[(f64, End); 2]>> {
        let mut pts: Vec<(f64, End)> = Vec::new();
        for k in 0..self.faces[f].edges.len() {
            let e = self.faces[f].edges[k].0;
            let [a, b] = self.edges[e].v;
            let (sa, sb) = (self.side(a, g)?, self.side(b, g)?);
            if (sa >= 0) == (sb >= 0) {
                continue;
            }
            let (p, end) = if sa == 0 {
                (self.pool.p[a as usize], End::Vertex(a))
            } else if sb == 0 {
                (self.pool.p[b as usize], End::Vertex(b))
            } else {
                let (pa, pb) = self.edge_points(e);
                let h = &self.faces[g];
                let (da, db) = (h.n.dot(pa) - h.d, h.n.dot(pb) - h.d);
                (pa + (pb - pa) * (da / (da - db)), End::Cross(e, g))
            };
            pts.push((dir.dot(p), end));
        }
        if pts.len() % 2 == 1 {
            return Err(Decline("a face crosses a plane an odd number of times"));
        }
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        Ok(pts.chunks(2).filter(|c| c[0].1 != c[1].1).map(|c| [c[0], c[1]]).collect())
    }

    fn vertex_of(&mut self, end: End) -> R<u32> {
        match end {
            End::Vertex(v) => Ok(v),
            End::Cross(e, g) => self.crossing(e, g),
        }
    }

    /// Edge `e` lies in face `f`'s plane: its pieces inside `f` go into `f`.
    fn edge_in_face(&mut self, e: usize, f: usize) -> R<()> {
        if !self.in_face_done.insert((e, f)) {
            return Ok(());
        }
        let tol = self.tol;
        let [va, vb] = self.edges[e].v;
        let (pa, pb) = self.edge_points(e);
        let g = &self.faces[f];
        let (a, b) = (g.to2(pa), g.to2(pb));
        let len = (b - a).length();
        if len <= tol {
            return Err(Decline("a tiny edge"));
        }
        let dir = (b - a) / len;
        let mut cuts: Vec<(f64, u32)> = vec![(0.0, va), (len, vb)];
        let fv = self.face_vertices[f].clone();
        // The face's vertices on the edge.
        for &w in &fv {
            if w == va || w == vb {
                continue;
            }
            let q = self.faces[f].to2(self.pool.p[w as usize]);
            let s = (q - a).dot(dir);
            let off = (q - a).perp_dot(dir).abs();
            if off <= tol && s > tol && s < len - tol {
                cuts.push((s, w));
            }
        }
        // The face's edges crossing it.
        let fe: Vec<(usize, bool)> = self.faces[f].edges.clone();
        for &(ge, _) in &fe {
            let [qa, qb] = self.edges[ge].v;
            let g = &self.faces[f];
            let (q0, q1) = (g.to2(self.pool.p[qa as usize]), g.to2(self.pool.p[qb as usize]));
            let (d0, d1) = ((q0 - a).perp_dot(dir), (q1 - a).perp_dot(dir));
            if d0.abs() <= tol || d1.abs() <= tol || (d0 > 0.0) == (d1 > 0.0) {
                continue;
            }
            let ql = (q1 - q0).length();
            let qd = (q1 - q0) / ql;
            let (e0, e1) = ((a - q0).perp_dot(qd), (b - q0).perp_dot(qd));
            if e0.abs() <= tol || e1.abs() <= tol || (e0 > 0.0) == (e1 > 0.0) {
                continue;
            }
            let s = e0 / (e0 - e1) * len;
            let p = pa + (pb - pa) * (s / len);
            let w = self.pool.add(p)?;
            if w != va && w != vb {
                cuts.push((s, w));
            }
        }
        cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
        cuts.dedup_by_key(|c| c.1);
        for k in 0..cuts.len() - 1 {
            let (w0, w1) = (cuts[k].1, cuts[k + 1].1);
            if w0 == w1 {
                continue;
            }
            let mid = a + dir * ((cuts[k].0 + cuts[k + 1].0) * 0.5);
            if self.in_face_2d(f, mid, true)? {
                self.face_segs[f].push([w0, w1]);
            }
        }
        Ok(())
    }

    /// Whether the 2D point `q` (face `f`'s frame) is inside face `f` (with its boundary when `closed`).
    fn in_face_2d(&self, f: usize, q: DVec2, closed: bool) -> R<bool> {
        let g = &self.faces[f];
        let mut dmin = f64::INFINITY;
        for &(e, _) in &g.edges {
            let (p0, p1) = self.edge_points(e);
            dmin = dmin.min(segment_distance(q, g.to2(p0), g.to2(p1)));
        }
        if dmin <= self.tol {
            return Ok(closed);
        }
        if dmin <= GREY * self.tol {
            return Err(Decline("a point nearly on a face's boundary"));
        }
        Ok(inside(
            q,
            g.edges.iter().map(|&(e, _)| {
                let (p0, p1) = self.edge_points(e);
                (g.to2(p0), g.to2(p1))
            }),
        ))
    }

    fn face_pair(&mut self, fs: usize, ft: usize) -> R<()> {
        let sa: Vec<i8> = self.face_vertices[fs].clone().into_iter().map(|v| self.side(v, ft)).collect::<R<_>>()?;
        let sb: Vec<i8> = self.face_vertices[ft].clone().into_iter().map(|v| self.side(v, fs)).collect::<R<_>>()?;
        let flat_a = sa.iter().all(|&s| s == 0);
        let flat_b = sb.iter().all(|&s| s == 0);
        if flat_a != flat_b {
            return Err(Decline("faces nearly coplanar"));
        }
        if flat_a {
            self.coplanar.push((fs, ft));
            for (e, _) in self.faces[fs].edges.clone() {
                self.edge_in_face(e, ft)?;
            }
            for (e, _) in self.faces[ft].edges.clone() {
                self.edge_in_face(e, fs)?;
            }
            return Ok(());
        }
        for (f, g) in [(fs, ft), (ft, fs)] {
            for (e, _) in self.faces[f].edges.clone() {
                let [a, b] = self.edges[e].v;
                if self.side(a, g)? == 0 && self.side(b, g)? == 0 {
                    self.edge_in_face(e, g)?;
                }
            }
        }
        let crosses = |s: &[i8]| s.iter().any(|&x| x > 0) && s.iter().any(|&x| x < 0);
        if !crosses(&sa) || !crosses(&sb) {
            return Ok(());
        }
        let dir = self.faces[fs].n.cross(self.faces[ft].n);
        if dir.length() < 1e-9 {
            return Err(Decline("faces nearly parallel"));
        }
        let dir = dir.normalize();
        let ia = self.intervals(fs, ft, dir)?;
        let ib = self.intervals(ft, fs, dir)?;
        let tol = self.tol;
        for x in &ia {
            for y in &ib {
                if x[1].0 < y[0].0 - tol || y[1].0 < x[0].0 - tol {
                    continue;
                }
                let lo = if x[0].0 > y[0].0 + tol {
                    self.vertex_of(x[0].1)?
                } else if y[0].0 > x[0].0 + tol {
                    self.vertex_of(y[0].1)?
                } else {
                    let (a, b) = (self.vertex_of(x[0].1)?, self.vertex_of(y[0].1)?);
                    if a != b {
                        return Err(Decline("a section's end is unclear"));
                    }
                    a
                };
                let hi = if x[1].0 < y[1].0 - tol {
                    self.vertex_of(x[1].1)?
                } else if y[1].0 < x[1].0 - tol {
                    self.vertex_of(y[1].1)?
                } else {
                    let (a, b) = (self.vertex_of(x[1].1)?, self.vertex_of(y[1].1)?);
                    if a != b {
                        return Err(Decline("a section's end is unclear"));
                    }
                    a
                };
                let (tl, th) = (dir.dot(self.pool.p[lo as usize]), dir.dot(self.pool.p[hi as usize]));
                if th - tl <= tol || lo == hi {
                    continue;
                }
                self.face_segs[fs].push([lo, hi]);
                self.face_segs[ft].push([lo, hi]);
                self.sections.push(([lo, hi], fs, ft));
            }
        }
        Ok(())
    }

    /// The pool vertices strictly inside the segment `a b`, in order from `a`.
    fn split(&self, a: u32, b: u32, sorted_x: &[(f64, u32)]) -> R<Vec<u32>> {
        let (pa, pb) = (self.pool.p[a as usize], self.pool.p[b as usize]);
        let tol = self.tol;
        let lo = pa.min(pb) - DVec3::splat(tol);
        let hi = pa.max(pb) + DVec3::splat(tol);
        let start = sorted_x.partition_point(|x| x.0 < lo.x);
        let d = pb - pa;
        let len = d.length();
        let dir = d / len;
        let mut on: Vec<(f64, u32)> = Vec::new();
        for &(x, w) in &sorted_x[start..] {
            if x > hi.x {
                break;
            }
            if w == a || w == b {
                continue;
            }
            let p = self.pool.p[w as usize];
            if p.y < lo.y || p.y > hi.y || p.z < lo.z || p.z > hi.z {
                continue;
            }
            let s = (p - pa).dot(dir);
            if s <= 0.0 || s >= len {
                continue;
            }
            let off = (p - pa - dir * s).length();
            if off <= tol {
                on.push((s, w));
            } else if off <= GREY * tol {
                return Err(Decline("a vertex nearly on an edge"));
            }
        }
        on.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut chain = vec![a];
        chain.extend(on.into_iter().map(|x| x.1));
        chain.push(b);
        Ok(chain)
    }

    /// Where 3D point `p` (not on any of solid `s`'s faces) is against solid `s`, by a ray's crossings.
    fn ray_state(&self, p: DVec3, s: usize) -> R<State> {
        const DIRS: [DVec3; 4] = [
            DVec3::new(0.5773502691896258, 0.5773502691896258, 0.5773502691896258),
            DVec3::new(0.2672612419124244, 0.5345224838248488, 0.8017837257372732),
            DVec3::new(-0.8017837257372732, 0.2672612419124244, 0.5345224838248488),
            DVec3::new(0.5345224838248488, -0.8017837257372732, 0.2672612419124244),
        ];
        let sol = &self.solids[s];
        if p.x < sol.lo.x - self.tol || p.y < sol.lo.y - self.tol || p.z < sol.lo.z - self.tol || p.x > sol.hi.x + self.tol || p.y > sol.hi.y + self.tol || p.z > sol.hi.z + self.tol {
            return Ok(State::Out);
        }
        'dirs: for r in DIRS {
            let mut odd = false;
            for &f in &sol.faces {
                let g = &self.faces[f];
                let den = g.n.dot(r);
                let dist = g.d - g.n.dot(p);
                if den.abs() < 1e-6 {
                    if dist.abs() <= GREY * self.tol {
                        continue 'dirs;
                    }
                    continue;
                }
                let t = dist / den;
                if t <= 0.0 {
                    if t > -GREY * self.tol {
                        // The point is on this plane: on the face would have been found coplanar.
                        let q = g.to2(p);
                        if self.in_face_2d(f, q, true)? {
                            return Err(Decline("a piece's point on another solid's face"));
                        }
                    }
                    continue;
                }
                let q = g.to2(p + r * t);
                let mut dmin = f64::INFINITY;
                for &(e, _) in &g.edges {
                    let (p0, p1) = self.edge_points(e);
                    dmin = dmin.min(segment_distance(q, g.to2(p0), g.to2(p1)));
                }
                if dmin <= GREY * self.tol {
                    continue 'dirs;
                }
                if inside(
                    q,
                    g.edges.iter().map(|&(e, _)| {
                        let (p0, p1) = self.edge_points(e);
                        (g.to2(p0), g.to2(p1))
                    }),
                ) {
                    odd = !odd;
                }
            }
            return Ok(if odd { State::In } else { State::Out });
        }
        Err(Decline("no clean ray"))
    }
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
        pool: Pool { p: Vec::new(), inputs: Vec::new(), grid: HashMap::new(), cell: GREY * tol * 1.01, tol },
        faces: Vec::new(),
        edges: Vec::new(),
        solids: Vec::new(),
        face_vertices: Vec::new(),
        side_cache: HashMap::new(),
        cross_cache: HashMap::new(),
        in_face_done: Default::default(),
        face_segs: Vec::new(),
        coplanar: Vec::new(),
        sections: Vec::new(),
    };
    // Vertices, edges, faces and solids of every argument.
    let mut first_edge = Vec::new();
    for (ai, body) in args.iter().enumerate() {
        let a = ai as u32;
        let mut vmap = Vec::with_capacity(body.vertices.len());
        for (i, &p) in body.vertices.iter().enumerate() {
            let w = match cx.pool.find(p)? {
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
            if e.curve != Curve::Line {
                return Err(Decline("a curved edge"));
            }
            let v = [vmap[e.v[0] as usize], vmap[e.v[1] as usize]];
            if v[0] == v[1] {
                return Err(Decline("a closed or tiny edge"));
            }
            cx.edges.push(GEdge { arg: a, index: i as u32, v });
        }
        let face0 = cx.faces.len();
        for (i, f) in body.faces.iter().enumerate() {
            let Surface::Plane { origin, .. } = f.surface;
            let n = f.normal();
            let (u, v) = frame(n);
            let edges: Vec<(usize, bool)> = f.loops.iter().flatten().map(|u| (first_edge[ai] + u.edge as usize, u.reversed)).collect();
            if edges.len() < 3 {
                return Err(Decline("a face with fewer than three edges"));
            }
            cx.faces.push(GFace { arg: a, index: i as u32, solid: usize::MAX, n, d: n.dot(origin), origin, u, v, edges, lo: DVec3::INFINITY, hi: DVec3::NEG_INFINITY });
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
    // Boxes and each face's vertices (which must lie on its plane).
    for f in 0..cx.faces.len() {
        let mut vs: Vec<u32> = cx.faces[f].edges.iter().flat_map(|&(e, _)| cx.edges[e].v).collect();
        vs.sort_unstable();
        vs.dedup();
        let (mut lo, mut hi) = (DVec3::INFINITY, DVec3::NEG_INFINITY);
        for &w in &vs {
            let p = cx.pool.p[w as usize];
            if (cx.faces[f].n.dot(p) - cx.faces[f].d).abs() > tol {
                return Err(Decline("a face's vertex off its plane"));
            }
            lo = lo.min(p);
            hi = hi.max(p);
        }
        cx.faces[f].lo = lo;
        cx.faces[f].hi = hi;
        let s = cx.faces[f].solid;
        cx.solids[s].lo = cx.solids[s].lo.min(lo);
        cx.solids[s].hi = cx.solids[s].hi.max(hi);
        cx.face_vertices.push(vs);
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
    let mut sedges: Vec<SEdge> = Vec::new();
    let mut sedge_of: HashMap<(u32, u32), usize> = HashMap::new();
    let mut get_sedge = |sedges: &mut Vec<SEdge>, a: u32, b: u32| -> (usize, bool) {
        let key = (a.min(b), a.max(b));
        let i = *sedge_of.entry(key).or_insert_with(|| {
            sedges.push(SEdge { v: [a, b], origins: Vec::new() });
            sedges.len() - 1
        });
        (i, sedges[i].v[0] != a)
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
    for e in 0..cx.edges.len() {
        let [a, b] = cx.edges[e].v;
        let chain = cx.split(a, b, &by_x)?;
        for &w in &chain {
            note(&mut touched, &cx.pool, w, cx.edges[e].arg);
        }
        edge_inner.push(chain[1..chain.len() - 1].to_vec());
        let mut pieces = Vec::with_capacity(chain.len() - 1);
        for w in chain.windows(2) {
            let (s, rev) = get_sedge(&mut sedges, w[0], w[1]);
            if !sedges[s].origins.contains(&e) {
                sedges[s].origins.push(e);
            }
            pieces.push((s, rev));
        }
        edge_pieces.push(pieces);
    }
    let mut face_extra: Vec<Vec<usize>> = vec![Vec::new(); cx.faces.len()];
    for (f, extra) in face_extra.iter_mut().enumerate() {
        for k in 0..cx.face_segs[f].len() {
            let [a, b] = cx.face_segs[f][k];
            let chain = cx.split(a, b, &by_x)?;
            for w in chain.windows(2) {
                extra.push(get_sedge(&mut sedges, w[0], w[1]).0);
            }
        }
    }
    // The faces each section edge was made by (OCCT: they generated it).
    let mut section_faces: Vec<(usize, usize)> = Vec::new();
    for &([a, b], fs, ft) in &cx.sections {
        let chain = cx.split(a, b, &by_x)?;
        for w in chain.windows(2) {
            let (s, _) = get_sedge(&mut sedges, w[0], w[1]);
            section_faces.push((s, fs));
            section_faces.push((s, ft));
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
        let n_boundary = segs.len();
        let boundary_split = n_boundary != cx.faces[f].edges.len();
        segs.extend(face_extra[f].iter().copied());
        segs.sort_unstable();
        segs.dedup();
        face_split[f] = boundary_split || segs.len() != n_boundary;
        if !involved[cx.faces[f].solid] {
            let loops = vec![cx.faces[f].edges.iter().map(|&(e, r)| (edge_pieces[e][0].0, r ^ edge_pieces[e][0].1)).collect()];
            face_pieces[f].push(pieces.len());
            pieces.push(Piece { face: f, loops, inside: DVec3::ZERO });
            continue;
        }
        let mut local: HashMap<u32, u32> = HashMap::new();
        let mut pts: Vec<DVec2> = Vec::new();
        let mut lsegs: Vec<[u32; 2]> = Vec::with_capacity(segs.len());
        for &s in &segs {
            let mut ends = [0u32; 2];
            for (k, &w) in sedges[s].v.iter().enumerate() {
                ends[k] = *local.entry(w).or_insert_with(|| {
                    pts.push(cx.faces[f].to2(cx.pool.p[w as usize]));
                    (pts.len() - 1) as u32
                });
            }
            lsegs.push(ends);
        }
        let boundary: std::collections::HashSet<u32> = cx.faces[f].edges.iter().flat_map(|&(e, _)| edge_pieces[e].iter().flat_map(|x| sedges[x.0].v)).collect();
        let mut inner: Vec<u32> = local.keys().copied().filter(|w| !boundary.contains(w)).collect();
        inner.sort_unstable();
        for &w in &inner {
            note(&mut touched, &cx.pool, w, cx.faces[f].arg);
        }
        face_inner[f] = inner;
        for r in regions(&pts, &lsegs)? {
            if !cx.in_face_2d(f, r.inside, false)? {
                continue;
            }
            if r.dangling {
                return Err(Decline("an edge ends inside a face"));
            }
            let g = &cx.faces[f];
            let inside3 = g.origin + g.u * r.inside.x + g.v * r.inside.y;
            let mut loops = vec![r.outer];
            loops.extend(r.holes);
            let loops = loops.into_iter().map(|l| l.into_iter().map(|(s, rev)| (segs[s as usize], rev)).collect()).collect();
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
            if !meets[s * ns + t] {
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
                if cx.in_face_2d(g, q, false)? {
                    st = Some(if cx.faces[g].n.dot(cx.faces[f].n) > 0.0 { State::OnSame } else { State::OnOpposite });
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
        if ons.len() > 1 {
            return Err(Decline("a piece on two solids"));
        }
        let in_any = states.iter().any(|x| x.1 == State::In);
        let in_obj = states.iter().any(|x| x.1 == State::In && cx.solids[x.0].arg == 0);
        let in_tool = states.iter().any(|x| x.1 == State::In && cx.solids[x.0].arg != 0);
        let on = ons.first().copied();
        if let Some((t, _)) = on {
            if arg != 0 && cx.solids[t].arg != 0 {
                return Err(Decline("tools coincide"));
            }
            on_pairs.push((pi, t));
        }
        keep[pi] = match op {
            Op::Fuse => match on {
                None => (!in_any).then_some(false),
                Some((_, State::OnSame)) => (!in_any && arg == 0).then_some(false),
                _ => None,
            },
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
    // Coincident pieces: the tool's goes to the object's.
    let mut by_key: HashMap<Vec<usize>, usize> = HashMap::new();
    for &(pi, _) in &on_pairs {
        if cx.faces[pieces[pi].face].arg == 0 {
            by_key.insert(piece_key(&pieces[pi]), pi);
        }
    }
    for &(pi, _) in &on_pairs {
        if cx.faces[pieces[pi].face].arg != 0 {
            match by_key.get(&piece_key(&pieces[pi])) {
                Some(&o) => partner[pi] = Some(o),
                None => return Err(Decline("coincident pieces differ")),
            }
        }
    }
    // The result's faces and their edges.
    let kept: Vec<usize> = (0..pieces.len()).filter(|&p| keep[p].is_some()).collect();
    if kept.is_empty() {
        return Err(Decline("an empty result"));
    }
    let mut uses: HashMap<usize, [i32; 2]> = HashMap::new();
    for &p in &kept {
        let flip = keep[p] == Some(true);
        for l in &pieces[p].loops {
            for &(s, r) in l {
                let u = uses.entry(s).or_insert([0, 0]);
                u[(r ^ flip) as usize] += 1;
            }
        }
    }
    if uses.values().any(|u| u[0] != 1 || u[1] != 1) {
        return Err(Decline("a non-manifold result"));
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
    let mut first_user: HashMap<usize, usize> = HashMap::new();
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
    let mut vmap: HashMap<u32, u32> = HashMap::new();
    let mut emap: HashMap<usize, u32> = HashMap::new();
    let mut used_s: Vec<usize> = uses.keys().copied().collect();
    used_s.sort_unstable();
    let mut used_v: Vec<u32> = used_s.iter().flat_map(|&s| sedges[s].v).collect();
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
            x.origins.len() == 1 && edge_pieces[x.origins[0]].len() == 1 && vertex_same(x.v[0]) && vertex_same(x.v[1])
        })
        .collect();
    let mut sedge_flip: HashMap<usize, bool> = HashMap::new();
    for &s in &used_s {
        let x = &sedges[s];
        // Pieces of an input edge run its way.
        let flip = match x.origins.first() {
            Some(&e) => edge_pieces[e].iter().find(|p| p.0 == s).is_some_and(|p| p.1),
            None => false,
        };
        sedge_flip.insert(s, flip);
        let v = if flip { [x.v[1], x.v[0]] } else { x.v };
        emap.insert(s, out.body.edges.len() as u32);
        out.body.edges.push(Edge { v: [vmap[&v[0]], vmap[&v[1]]], curve: Curve::Line });
        out.edge_origin.push(match x.origins.first() {
            None => Origin::New,
            Some(&e) => {
                let g = &cx.edges[e];
                if edge_same[s] { Origin::Same { arg: g.arg, index: g.index } } else { Origin::Piece { arg: g.arg, index: g.index } }
            }
        });
    }
    let mut piece_out: HashMap<usize, u32> = HashMap::new();
    let mut face_same = vec![false; cx.faces.len()];
    for &p in &kept {
        let f = pieces[p].face;
        let flip = keep[p] == Some(true);
        let g = &cx.faces[f];
        let same = !face_split[f] && pieces[p].loops.iter().flatten().all(|&(s, _)| edge_same[s]);
        face_same[f] = same;
        let src = &args[g.arg as usize].faces[g.index as usize];
        let loops: Vec<Vec<Use>> = pieces[p]
            .loops
            .iter()
            .map(|l| {
                let mut l: Vec<Use> = l.iter().map(|&(s, r)| Use { edge: emap[&s], reversed: r ^ sedge_flip[&s] }).collect();
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
        out.body.faces.push(Face { surface: src.surface, reversed: src.reversed ^ flip, loops });
        out.face_origin.push(if same { Origin::Same { arg: g.arg, index: g.index } } else { Origin::Piece { arg: g.arg, index: g.index } });
    }
    // Solids, one per shell, outward.
    let mut shell_of: HashMap<usize, usize> = HashMap::new();
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
        let mut vol = 0.0;
        for &f in &s.shells[0] {
            let face = &out.body.faces[f as usize];
            let Surface::Plane { origin, .. } = face.surface;
            for l in &face.loops {
                for u in l {
                    let e = out.body.edges[u.edge as usize];
                    let (a, b) = if u.reversed { (e.v[1], e.v[0]) } else { (e.v[0], e.v[1]) };
                    vol += origin.dot(out.body.vertices[a as usize].cross(out.body.vertices[b as usize]));
                }
            }
        }
        if vol <= 0.0 {
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
