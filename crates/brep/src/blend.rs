//! noshape's own fillets and chamfers (constant radius, distance-distance), built directly where the blend
//! surface is an elementary one; anything else is a [`Decline`] and OCCT's blend runs.
//!
//! What it takes so far: closed chains of tangent edges that make a whole loop of a planar face P (a hole's
//! rim, a boss's top or base, a slot's outline), each edge a line or an arc about an axis square to P, and each
//! edge's other face (its wall) a plane or a cylinder square to P. Every edge gets its blend face: a cylinder or
//! a torus (fillet), a plane or a cone (chamfer); P's loop moves into P, the walls' edges move along the walls,
//! and the edges where walls meet (or a cylinder's seam) are shortened. Neighbouring blend faces meet along the
//! blend's cross-section at the chain's vertices (an arc, a chamfer's line).
//!
//! The result is OCCT's: the same faces, edges and vertices, the same surfaces, P and the walls Modified, each
//! blended edge Generated its blend face, the shortened edges Modified to their pieces.

use crate::{Body, Curve, Decline, Edge, Face, Kind, Origin, Output, Record, Relation, Solid, Surface, Use};
use glam::DVec3;

/// A blend of one edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spec {
    /// A round of this radius.
    Fillet { radius: f64 },
    /// A flat cut `d1` into the reference face and `d2` into the edge's other face.
    Chamfer { d1: f64, d2: f64 },
}

/// One edge to blend: the body's edge, how, and (a chamfer's) reference face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendEdge {
    pub edge: u32,
    pub spec: Spec,
    pub reference: u32,
}

fn no<T>(why: &'static str) -> Result<T, Decline> {
    Err(Decline(why))
}

/// Uses of each edge: (face, loop, position, reversed).
type EdgeUses = Vec<Vec<(u32, u32, u32, bool)>>;

fn uses_of(b: &Body) -> EdgeUses {
    let mut out = vec![Vec::new(); b.edges.len()];
    for (f, face) in b.faces.iter().enumerate() {
        for (l, lp) in face.loops.iter().enumerate() {
            for (k, u) in lp.iter().enumerate() {
                out[u.edge as usize].push((f as u32, l as u32, k as u32, u.reversed));
            }
        }
    }
    out
}

/// A point of `c` (from `p0` to `p1`) at fraction `s` of its way (lines and arcs).
fn curve_point(b: &Body, e: &Edge, s: f64) -> Option<DVec3> {
    let (p0, p1) = (b.vertices[e.v[0] as usize], b.vertices[e.v[1] as usize]);
    match e.curve {
        Curve::Line => Some(p0.lerp(p1, s)),
        Curve::Circle { center, normal, radius, .. } => {
            let (a0, sweep) = arc_angles(center, normal, p0, p1, e.v[0] == e.v[1]);
            let x = (p0 - center).normalize();
            let y = normal.cross(x);
            let a = a0 * 0.0 + sweep * s;
            Some(center + radius * (a.cos() * x + a.sin() * y))
        }
        Curve::Nurbs { index, t0, t1 } => Some(b.nurbs_curves[index as usize].at(t0 + (t1 - t0) * s)),
        _ => None,
    }
}

/// An arc's start angle (0: measured from its start) and its counter-clockwise sweep about `normal`.
fn arc_angles(center: DVec3, normal: DVec3, p0: DVec3, p1: DVec3, closed: bool) -> (f64, f64) {
    if closed {
        return (0.0, std::f64::consts::TAU);
    }
    let x = (p0 - center).normalize();
    let y = normal.cross(x);
    let d = p1 - center;
    let mut a = d.dot(y).atan2(d.dot(x));
    if a <= 0.0 {
        a += std::f64::consts::TAU;
    }
    (0.0, a)
}

/// The unit tangent of edge `e` (its own direction) at its start (`at_end` false) or end.
fn tangent(b: &Body, e: &Edge, at_end: bool) -> Option<DVec3> {
    let (p0, p1) = (b.vertices[e.v[0] as usize], b.vertices[e.v[1] as usize]);
    match e.curve {
        Curve::Line => Some((p1 - p0).normalize()),
        Curve::Circle { center, normal, .. } => {
            let p = if at_end { p1 } else { p0 };
            Some(normal.cross(p - center).normalize())
        }
        _ => None,
    }
}

/// Distance from `p` to edge `e` (a line segment or an arc).
fn point_edge_distance(b: &Body, e: &Edge, p: DVec3) -> Option<f64> {
    let (p0, p1) = (b.vertices[e.v[0] as usize], b.vertices[e.v[1] as usize]);
    match e.curve {
        Curve::Line => {
            let d = p1 - p0;
            let s = ((p - p0).dot(d) / d.length_squared()).clamp(0.0, 1.0);
            Some((p0 + d * s - p).length())
        }
        Curve::Circle { center, normal, radius, .. } => {
            // The nearest point of the whole circle, if it's on the arc; else the nearer end.
            let rel = p - center;
            let h = rel.dot(normal);
            let inplane = rel - normal * h;
            let ends = (p0 - p).length().min((p1 - p).length());
            if inplane.length() < 1e-12 {
                return Some((h * h + radius * radius).sqrt());
            }
            let q = center + inplane.normalize() * radius;
            let (_, sweep) = arc_angles(center, normal, p0, p1, e.v[0] == e.v[1]);
            let x = (p0 - center).normalize();
            let y = normal.cross(x);
            let mut a = inplane.dot(y).atan2(inplane.dot(x));
            if a < 0.0 {
                a += std::f64::consts::TAU;
            }
            Some(if a <= sweep { (q - p).length() } else { ends })
        }
        _ => None,
    }
}

/// A lower bound of the distance between edges `a` (a line or an arc) and `o` (any edge whose points can be
/// had): `o` sampled, less how far its chords may stray from it.
fn edge_distance(b: &Body, a: &Edge, o: &Edge) -> Option<f64> {
    const N: usize = 96;
    let mut best = f64::INFINITY;
    let mut prev: Option<DVec3> = None;
    let mut chord = 0.0f64;
    for k in 0..=N {
        let p = curve_point(b, o, k as f64 / N as f64)?;
        best = best.min(point_edge_distance(b, a, p)?);
        if let Some(q) = prev {
            chord = chord.max((p - q).length());
        }
        prev = Some(p);
    }
    // (A sample's neighbours are a chord away: the curve between is within half a chord of one of them.)
    Some(best - 0.5 * chord)
}

/// A box round edge `e` (an arc's whole circle; a NURBS edge's poles).
fn edge_box(b: &Body, e: &Edge) -> (DVec3, DVec3) {
    let (p0, p1) = (b.vertices[e.v[0] as usize], b.vertices[e.v[1] as usize]);
    let (mut lo, mut hi) = (p0.min(p1), p0.max(p1));
    match e.curve {
        Curve::Circle { center, radius, .. } => {
            lo = lo.min(center - DVec3::splat(radius));
            hi = hi.max(center + DVec3::splat(radius));
        }
        Curve::Nurbs { index, .. } => {
            for p in &b.nurbs_curves[index as usize].poles {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
        }
        Curve::Line | Curve::Degenerate => {}
        _ => {
            // (Conics of a Boolean's result: as wide as the body.)
            lo = DVec3::splat(f64::NEG_INFINITY);
            hi = DVec3::splat(f64::INFINITY);
        }
    }
    (lo, hi)
}

fn boxes_meet(a: (DVec3, DVec3), b: (DVec3, DVec3), pad: f64) -> bool {
    (a.0 - DVec3::splat(pad)).cmple(b.1).all() && (b.0 - DVec3::splat(pad)).cmple(a.1).all()
}

/// A planar face's outward normal.
fn plane_normal(f: &Face) -> Option<DVec3> {
    f.normal()
}

/// The outward normal of face `f` at `p` (a plane or a cylinder).
fn outward_at(f: &Face, p: DVec3) -> Option<DVec3> {
    let n = match f.surface {
        Surface::Plane { normal, .. } => normal,
        Surface::Cylinder { origin, axis, .. } => {
            let r = p - origin;
            (r - axis * r.dot(axis)).normalize()
        }
        _ => return None,
    };
    Some(if f.reversed { -n } else { n })
}

/// One blended edge of a chain.
#[derive(Clone, Debug)]
struct Link {
    edge: u32,
    /// P's use of the edge: reversed.
    rev: bool,
    wall: u32,
    spec: Spec,
    /// Into P from the edge, square to it, at the use's start (an arc's turns with it).
    q0: DVec3,
    /// Into the wall from the edge, square to it (the same all along a line, or an arc's square wall).
    t: DVec3,
}

/// An open chain's end: P's next edge there (not blended), the edge down the wall, the cap face they bound.
#[derive(Clone, Copy, Debug)]
struct End {
    g: u32,
    s: u32,
    cap: u32,
}

/// A chain: a run of tangent blended edges of a loop of the planar face P.
#[derive(Clone, Debug)]
struct Chain {
    face: u32,
    links: Vec<Link>,
    /// Its vertices in P's order: link i runs from verts[i] to verts[i + 1] (round to verts[0] when closed).
    verts: Vec<u32>,
    closed: bool,
    /// An open chain's ends (at verts[0], at its last vertex).
    ends: [Option<End>; 2],
    /// The blend's reach into P and along the walls, and (a fillet) its centre's from the edge along the bisector.
    d1: f64,
    d2: f64,
    centre: f64,
    /// Convex (+1: the blend takes material) or concave (−1: it adds some).
    sigma: f64,
}

impl Chain {
    fn k(&self) -> usize {
        self.links.len()
    }
    /// The link before vertex `j` and the one after (an open chain's ends have one).
    fn at(&self, j: usize) -> (Option<usize>, Option<usize>) {
        let k = self.k();
        if self.closed {
            (Some((j + k - 1) % k), Some(j % k))
        } else {
            ((j > 0).then(|| j - 1), (j < k).then_some(j))
        }
    }
}

/// Blends `edges` of `body` (one solid), whose points are one within `tol`.
pub fn blend(body: &Body, edges: &[BlendEdge], tol: f64) -> Result<Output, Decline> {
    if body.solids.len() != 1 || body.solids[0].shells.len() != 1 {
        return no("not one solid of one shell");
    }
    if edges.is_empty() {
        return no("no edges");
    }
    let uses = uses_of(body);
    for (i, u) in uses.iter().enumerate() {
        if u.len() != 2 && !body.edges[i].curve.eq(&Curve::Degenerate) {
            return no("an edge not between two faces");
        }
    }
    let size = body.vertices.iter().fold(0.0f64, |m, p| m.max(p.abs().max_element()));
    let margin = 10.0 * tol + 1e-9 * size;
    let mut spec_of = vec![None; body.edges.len()];
    for e in edges {
        if e.edge as usize >= body.edges.len() {
            return no("no such edge");
        }
        match e.spec {
            Spec::Fillet { radius } if radius > margin => {}
            Spec::Chamfer { d1, d2 } if d1 > margin && d2 > margin => {}
            _ => return no("a blend too small"),
        }
        if spec_of[e.edge as usize].is_some() {
            return no("an edge given twice");
        }
        spec_of[e.edge as usize] = Some(*e);
    }
    let ve = vertex_edges(body);
    let chains = chains(body, &uses, &ve, &spec_of, margin)?;
    let boxes: Vec<(DVec3, DVec3)> = body.edges.iter().map(|e| edge_box(body, e)).collect();
    for c in &chains {
        check_clearance(body, &ve, &boxes, c, &chains, margin)?;
    }
    build(body, &ve, &chains, margin)
}

/// A loop's uses in their order round (the body's loops may list them in any order); None when a vertex has
/// more than one way on.
fn ordered(b: &Body, lp: &[Use]) -> Option<Vec<Use>> {
    let start = |u: &Use| b.edges[u.edge as usize].v[u.reversed as usize];
    let end = |u: &Use| b.edges[u.edge as usize].v[!u.reversed as usize];
    let mut out = vec![lp[0]];
    let mut left: Vec<Use> = lp[1..].to_vec();
    while !left.is_empty() {
        let at = end(out.last().unwrap());
        let next: Vec<usize> = (0..left.len()).filter(|&i| start(&left[i]) == at).collect();
        let [i] = next[..] else { return None };
        out.push(left.remove(i));
    }
    (end(out.last().unwrap()) == start(&out[0])).then_some(out)
}

/// The chains `spec_of` makes: runs of tangent blended edges along loops of planar faces (an edge between two
/// planar faces goes with the first of them), each edge's wall a plane (a line) or a cylinder about an axis
/// square to P (an arc), an open run ending at cap faces square to it. Declines anything else.
fn chains(b: &Body, uses: &EdgeUses, ve: &[Vec<u32>], spec_of: &[Option<BlendEdge>], margin: f64) -> Result<Vec<Chain>, Decline> {
    let mut owner: Vec<Option<usize>> = vec![None; b.edges.len()];
    let mut out: Vec<Chain> = Vec::new();
    for (f, face) in b.faces.iter().enumerate() {
        let Some(n) = plane_normal(face) else { continue };
        for (l, lp) in face.loops.iter().enumerate() {
            if !lp.iter().any(|u| spec_of[u.edge as usize].is_some()) {
                continue;
            }
            // Runs of blended edges, tangent at the vertices between.
            let Some(mut lp) = ordered(b, lp) else { return no("a loop not a simple round") };
            if let Some(i) = lp.iter().position(|u| spec_of[u.edge as usize].is_none()) {
                lp.rotate_left(i);
            }
            let closed_all = lp.iter().all(|u| spec_of[u.edge as usize].is_some());
            let mut runs: Vec<Vec<Use>> = Vec::new();
            for u in &lp {
                if spec_of[u.edge as usize].is_none() {
                    runs.push(Vec::new());
                    continue;
                }
                if runs.is_empty() {
                    runs.push(Vec::new());
                }
                let r = runs.last_mut().unwrap();
                if let Some(prev) = r.last() {
                    let (ep, eu) = (&b.edges[prev.edge as usize], &b.edges[u.edge as usize]);
                    let (Some(tp), Some(tu)) = (tangent(b, ep, !prev.reversed), tangent(b, eu, u.reversed)) else {
                        return no("an edge not a line or an arc");
                    };
                    let tp = if prev.reversed { -tp } else { tp };
                    let tu = if u.reversed { -tu } else { tu };
                    if tp.dot(tu) < 1.0 - 1e-12 {
                        return no("a corner between blended edges");
                    }
                }
                r.push(*u);
            }
            runs.retain(|r| !r.is_empty());
            // A whole loop: closed only if tangent where it closes too.
            let mut closed = false;
            if closed_all {
                let (first, last) = (&lp[0], lp.last().unwrap());
                let (el, ef) = (&b.edges[last.edge as usize], &b.edges[first.edge as usize]);
                let (Some(tl), Some(tf)) = (tangent(b, el, !last.reversed), tangent(b, ef, first.reversed)) else {
                    return no("an edge not a line or an arc");
                };
                let tl = if last.reversed { -tl } else { tl };
                let tf = if first.reversed { -tf } else { tf };
                if tl.dot(tf) < 1.0 - 1e-12 {
                    return no("a corner between blended edges");
                }
                closed = true;
                runs = vec![lp.clone()];
            }
            for run in runs {
                // Claimed by an earlier face: skip it whole (an edge between two planar faces), else decline.
                let claimed = run.iter().filter(|u| owner[u.edge as usize].is_some()).count();
                if claimed == run.len() {
                    continue;
                }
                if claimed > 0 {
                    return no("an edge in two chains");
                }
                let c = chain(b, uses, ve, spec_of, f as u32, l as u32, n, &run, closed, margin)?;
                for x in &c.links {
                    owner[x.edge as usize] = Some(out.len());
                }
                out.push(c);
            }
        }
    }
    if spec_of.iter().enumerate().any(|(i, s)| s.is_some() && owner[i].is_none()) {
        return no("an edge not on a planar face");
    }
    Ok(out)
}

/// One chain along P (face `f`, its outward normal `n`): its links, walls, ends; why not.
#[allow(clippy::too_many_arguments)]
fn chain(b: &Body, uses: &EdgeUses, ve: &[Vec<u32>], spec_of: &[Option<BlendEdge>], f: u32, l: u32, n: DVec3, run: &[Use], closed: bool, margin: f64) -> Result<Chain, Decline> {
    let mut links = Vec::new();
    let mut verts = Vec::new();
    for u in run {
        let e = &b.edges[u.edge as usize];
        let spec = spec_of[u.edge as usize].unwrap();
        let Some(&(wall, ..)) = uses[u.edge as usize].iter().find(|x| x.0 != f) else { return no("an edge with P on both sides") };
        let w = &b.faces[wall as usize];
        let ps = b.vertices[e.v[u.reversed as usize] as usize];
        let Some(t_own) = tangent(b, e, u.reversed) else { return no("an edge not a line or an arc") };
        let t_use = if u.reversed { -t_own } else { t_own };
        let q0 = n.cross(t_use);
        let Some(m) = outward_at(w, ps) else { return no("a wall not a plane or a cylinder") };
        let t = m.cross(-t_use);
        if (q0.length() - 1.0).abs() > 1e-9 || (t.length() - 1.0).abs() > 1e-9 {
            return no("a wall not along the edge");
        }
        match (e.curve, w.surface) {
            (Curve::Line, Surface::Plane { .. }) => {}
            (Curve::Circle { center, normal, radius, .. }, Surface::Cylinder { origin, axis, radius: rc, .. }) => {
                let off = center - origin;
                if axis.cross(normal).length() > 1e-9 || (off - axis * off.dot(axis)).length() > margin || (radius - rc).abs() > margin {
                    return no("an arc not round its wall's axis");
                }
                if normal.cross(n).length() > 1e-9 || t.cross(n).length() > 1e-9 {
                    return no("a wall not square to P");
                }
            }
            _ => return no("a wall not a plane or a cylinder round the edge"),
        }
        links.push(Link { edge: u.edge, rev: u.reversed, wall, spec: spec.spec, q0, t });
        verts.push(e.v[u.reversed as usize]);
    }
    if !closed {
        let last = run.last().unwrap();
        verts.push(b.edges[last.edge as usize].v[!last.reversed as usize]);
    }
    // One blend all along: the same spec, angle and side, the walls going on where links meet.
    let x0 = &links[0];
    let alpha = x0.q0.dot(x0.t).clamp(-1.0, 1.0).acos();
    if !(1e-3..std::f64::consts::PI - 1e-3).contains(&alpha) {
        return no("faces nearly tangent or folded at the edge");
    }
    let sigma = if n.dot(x0.t) < 0.0 { 1.0 } else { -1.0 };
    for (i, x) in links.iter().enumerate() {
        if x.spec != x0.spec || (x.q0.dot(x.t).clamp(-1.0, 1.0).acos() - alpha).abs() > 1e-9 {
            return no("blends that change along a chain");
        }
        if i > 0 && x.t.dot(links[i - 1].t) < 1.0 - 1e-12 {
            return no("walls that turn where two blended edges meet");
        }
    }
    let (d1, d2, centre) = match x0.spec {
        Spec::Fillet { radius } => {
            let d = radius / (0.5 * alpha).tan();
            (d, d, radius / (0.5 * alpha).sin())
        }
        Spec::Chamfer { d1, d2 } => {
            let r = spec_of[x0.edge as usize].unwrap().reference;
            if d1 == d2 || r == f {
                (d1, d2, 0.0)
            } else if r == x0.wall {
                (d2, d1, 0.0)
            } else {
                return no("a chamfer's reference face not the edge's");
            }
        }
    };
    let mut c = Chain { face: f, links, verts, closed, ends: [None, None], d1, d2, centre, sigma };
    // Arcs whose radius the blend takes away.
    for x in &c.links {
        if let Curve::Circle { center, radius, .. } = b.edges[x.edge as usize].curve {
            let ps = b.vertices[b.edges[x.edge as usize].v[x.rev as usize] as usize];
            let into_p = x.q0.dot((ps - center).normalize());
            if radius + into_p * d1 <= 100.0 * margin {
                return no("an arc the blend closes up");
            }
            if let Spec::Fillet { radius: r } = x.spec
                && into_p < 0.0
                && radius + into_p * r <= r + 100.0 * margin
            {
                return no("a fillet over an arc of its own radius or less");
            }
        }
    }
    // The ends: P's next edge into P, the edge down the wall, the cap square to the chain between them.
    if !closed {
        for (side, j) in [(0usize, 0usize), (1, c.k())] {
            let v = c.verts[j];
            let (before, after) = c.at(j);
            let x = &c.links[before.or(after).unwrap()];
            let e = &b.edges[x.edge as usize];
            let p = b.vertices[v as usize];
            let others: Vec<u32> = ve[v as usize].iter().copied().filter(|&o| o != x.edge).collect();
            let [o1, o2] = others[..] else { return no("a chain end with more than three edges") };
            let in_p = |o: u32| b.faces[f as usize].loops[l as usize].iter().any(|u| u.edge == o);
            let (g, s) = if in_p(o1) { (o1, o2) } else { (o2, o1) };
            if !in_p(g) || in_p(s) {
                return no("a chain end not between P's edge and an edge down");
            }
            if spec_of[g as usize].is_some() || spec_of[s as usize].is_some() {
                return no("a corner between blended edges");
            }
            let q = into_p_at(b, x, p);
            for (o, dir, len) in [(g, q, d1), (s, x.t, d2)] {
                let oe = &b.edges[o as usize];
                if oe.curve != Curve::Line {
                    return no("a chain end's edge not a line");
                }
                let far = if oe.v[0] == v { oe.v[1] } else { oe.v[0] };
                let d = b.vertices[far as usize] - p;
                if d.normalize().dot(dir) < 1.0 - 1e-9 || d.length() <= len + margin {
                    return no("a chain end's edge not along the blend's side or too short");
                }
            }
            let fs = |o: u32| uses[o as usize].iter().map(|u| u.0).collect::<Vec<u32>>();
            let (gf, sf) = (fs(g), fs(s));
            let caps: Vec<u32> = gf.iter().copied().filter(|cf| sf.contains(cf) && *cf != f && *cf != x.wall).collect();
            let [cap] = caps[..] else { return no("a chain end without one cap face") };
            let Some(nc) = plane_normal(&b.faces[cap as usize]) else { return no("a cap face not planar") };
            let Some(te) = tangent(b, e, j != 0 && !x.rev || j == 0 && x.rev) else { return no("an edge not a line or an arc") };
            if nc.cross(te).length() > 1e-9 {
                return no("a cap face not square to the chain");
            }
            c.ends[side] = Some(End { g, s, cap });
        }
    }
    Ok(c)
}

/// Into P from link `x`'s edge at its point `p`, square to it.
fn into_p_at(b: &Body, x: &Link, p: DVec3) -> DVec3 {
    match b.edges[x.edge as usize].curve {
        Curve::Circle { center, normal, .. } => {
            // q turns with the arc: at the use's start it is q0; elsewhere the same side of the radius.
            let ps = b.vertices[b.edges[x.edge as usize].v[x.rev as usize] as usize];
            let side = x.q0.dot((ps - center).normalize());
            let r = p - center;
            (r - normal * r.dot(normal)).normalize() * side
        }
        _ => x.q0,
    }
}

/// The edges at each vertex.
fn vertex_edges(b: &Body) -> Vec<Vec<u32>> {
    let mut out = vec![Vec::new(); b.vertices.len()];
    for (i, e) in b.edges.iter().enumerate() {
        out[e.v[0] as usize].push(i as u32);
        if e.v[1] != e.v[0] {
            out[e.v[1] as usize].push(i as u32);
        }
    }
    out
}

/// The edge down the walls from each of the chain's vertices between links: one line, along the walls, longer
/// than the blend (an end's is its End's).
fn down_edges(b: &Body, c: &Chain, ve: &[Vec<u32>], margin: f64) -> Result<Vec<Option<u32>>, Decline> {
    let mut out = Vec::new();
    for (j, &v) in c.verts.iter().enumerate() {
        let (before, after) = c.at(j);
        let (Some(i0), Some(i1)) = (before, after) else {
            out.push(None);
            continue;
        };
        let chain_edges = [c.links[i0].edge, c.links[i1].edge];
        let others: Vec<u32> = ve[v as usize].iter().copied().filter(|e| !chain_edges.contains(e)).collect();
        let [s] = others[..] else { return no("a chain vertex with more than one edge down") };
        let e = &b.edges[s as usize];
        if e.curve != Curve::Line {
            return no("an edge down the walls not a line");
        }
        let far = if e.v[0] == v { e.v[1] } else { e.v[0] };
        let d = b.vertices[far as usize] - b.vertices[v as usize];
        if d.normalize().dot(c.links[i1].t) < 1.0 - 1e-12 {
            return no("an edge down the walls not along them");
        }
        if d.length() <= c.d2 + margin {
            return no("a wall shorter than the blend");
        }
        out.push(Some(s));
    }
    Ok(out)
}

/// The blend's strips on P, the walls and the caps are clear of every other edge of those faces, and of any
/// other chain's.
fn check_clearance(b: &Body, ve: &[Vec<u32>], boxes: &[(DVec3, DVec3)], c: &Chain, all: &[Chain], margin: f64) -> Result<(), Decline> {
    let downs = down_edges(b, c, ve, margin)?;
    let mut skip: Vec<u32> = c.links.iter().map(|x| x.edge).collect();
    skip.extend(downs.iter().flatten());
    for e in c.ends.iter().flatten() {
        skip.extend([e.g, e.s]);
    }
    let reach = c.d1.max(c.d2).max(c.centre);
    // Edge `e` of the chain more than `d` from edge `o` (boxes apart by more are).
    let far = |e: u32, o: u32, d: f64| -> Result<bool, Decline> {
        if !boxes_meet(boxes[e as usize], boxes[o as usize], d + 2.0 * margin) {
            return Ok(true);
        }
        match edge_distance(b, &b.edges[e as usize], &b.edges[o as usize]) {
            Some(x) => Ok(x > d + margin),
            None => no("an edge near the blend whose distance isn't known"),
        }
    };
    // P's and the walls' other edges.
    for lp in &b.faces[c.face as usize].loops {
        for u in lp {
            if skip.contains(&u.edge) {
                continue;
            }
            for x in &c.links {
                if !far(x.edge, u.edge, c.d1)? {
                    return no("another edge of P within the blend");
                }
            }
        }
    }
    for x in &c.links {
        for lp in &b.faces[x.wall as usize].loops {
            for u in lp {
                if !skip.contains(&u.edge) && !far(x.edge, u.edge, c.d2)? {
                    return no("another edge of a wall within the blend");
                }
            }
        }
    }
    // The caps' other edges, from the chain's ends.
    for (side, end) in c.ends.iter().enumerate() {
        let Some(end) = end else { continue };
        let v = c.verts[if side == 0 { 0 } else { c.k() }];
        let p = b.vertices[v as usize];
        for lp in &b.faces[end.cap as usize].loops {
            for u in lp {
                if u.edge == end.g || u.edge == end.s {
                    continue;
                }
                match point_edge_distance(b, &b.edges[u.edge as usize], p) {
                    Some(d) if d > reach + margin => {}
                    Some(_) => return no("another edge of a cap within the blend"),
                    None => return no("an edge near the blend whose distance isn't known"),
                }
            }
        }
    }
    // The chain's own non-neighbouring edges, and other chains'.
    let k = c.k();
    for i in 0..k {
        for j in i + 2..k {
            if c.closed && i == 0 && j == k - 1 {
                continue;
            }
            if !far(c.links[i].edge, c.links[j].edge, 2.0 * reach)? {
                return no("a chain whose blend meets itself");
            }
        }
    }
    for o in all {
        if std::ptr::eq(o, c) {
            continue;
        }
        for y in &o.links {
            for x in &c.links {
                if !far(x.edge, y.edge, 2.0 * reach)? {
                    return no("two chains' blends close");
                }
            }
        }
    }
    Ok(())
}

/// The result: P, the walls and the caps trimmed, the blend faces between.
fn build(body: &Body, ve: &[Vec<u32>], chains: &[Chain], margin: f64) -> Result<Output, Decline> {
    let mut b = body.clone();
    let (nv, ne, nf) = (body.vertices.len(), body.edges.len(), body.faces.len());
    let mut vertex_origin: Vec<Origin> = (0..nv as u32).map(|i| Origin::Same { arg: 0, index: i }).collect();
    let mut edge_origin: Vec<Origin> = (0..ne as u32).map(|i| Origin::Same { arg: 0, index: i }).collect();
    let mut face_origin: Vec<Origin> = (0..nf as u32).map(|i| Origin::Same { arg: 0, index: i }).collect();
    let mut history = vec![Record { kind: Kind::Solid, arg: 0, index: 0, relation: Relation::Modified, out_kind: Kind::Solid, out: 0 }];
    let mut dead_v = vec![false; nv];
    let mut dead_e = vec![false; ne];
    // Edge replaced by edge (a piece), in every loop.
    let mut replace: Vec<Option<u32>> = vec![None; ne];
    let mut touched_faces: Vec<u32> = Vec::new();
    let touch = |f: u32, t: &mut Vec<u32>| {
        if !t.contains(&f) {
            t.push(f);
        }
    };
    let mut new_faces: Vec<(u32, Face)> = Vec::new();
    let mut extra_uses: Vec<(u32, u32, Use)> = Vec::new();
    for c in chains {
        let k = c.k();
        let downs = down_edges(body, c, ve, margin)?;
        // At each vertex: a vertex on P (v1), one on the walls (v2), the cross-section edge between.
        let (mut v1, mut v2, mut cross) = (Vec::new(), Vec::new(), Vec::new());
        for (j, &v) in c.verts.iter().enumerate() {
            let (before, after) = c.at(j);
            let x = &c.links[after.or(before).unwrap()];
            let p = body.vertices[v as usize];
            let q = into_p_at(body, x, p);
            let (p1, p2) = (p + q * c.d1, p + x.t * c.d2);
            b.vertices.extend([p1, p2]);
            vertex_origin.extend([Origin::New, Origin::New]);
            let (i1, i2) = (b.vertices.len() as u32 - 2, b.vertices.len() as u32 - 1);
            v1.push(i1);
            v2.push(i2);
            dead_v[v as usize] = true;
            let curve = match x.spec {
                Spec::Fillet { radius } => {
                    let ctr = p + (q + x.t).normalize() * c.centre;
                    let normal = (p1 - ctr).cross(p2 - ctr).normalize();
                    Curve::Circle { center: ctr, normal, xdir: (p1 - ctr) / radius, radius }
                }
                Spec::Chamfer { .. } => Curve::Line,
            };
            b.edges.push(Edge { v: [i1, i2], curve });
            edge_origin.push(Origin::New);
            cross.push(b.edges.len() as u32 - 1);
            // The edges that ended at the vertex: from the new ones on.
            // (An edge cut short at both ends, by two chains: its one piece.)
            let mut cut_short = |s: u32, at: u32, b: &mut Body| {
                if let Some(si) = replace[s as usize] {
                    let pe = &mut b.edges[si as usize];
                    let end = if body.edges[s as usize].v[0] == v { 0 } else { 1 };
                    pe.v[end] = at;
                    return;
                }
                let mut se = body.edges[s as usize];
                if se.v[0] == v {
                    se.v[0] = at;
                } else {
                    se.v[1] = at;
                }
                b.edges.push(se);
                edge_origin.push(Origin::Piece { arg: 0, index: s });
                let si = b.edges.len() as u32 - 1;
                replace[s as usize] = Some(si);
                dead_e[s as usize] = true;
                history.push(Record { kind: Kind::Edge, arg: 0, index: s, relation: Relation::Modified, out_kind: Kind::Edge, out: si });
            };
            if let Some(s) = downs[j] {
                cut_short(s, i2, &mut b);
            }
            let end = if c.closed { None } else if j == 0 { c.ends[0] } else if j == k { c.ends[1] } else { None };
            if let Some(end) = end {
                cut_short(end.s, i2, &mut b);
                cut_short(end.g, i1, &mut b);
                // The cap takes the cross-section the other way from the blend face beside it.
                let lp = body.faces[end.cap as usize].loops.iter().position(|lp| lp.iter().any(|u| u.edge == end.g)).unwrap() as u32;
                extra_uses.push((end.cap, lp, Use { edge: *cross.last().unwrap(), reversed: j == 0 }));
                touch(end.cap, &mut touched_faces);
            }
        }
        // Each link: its edge on P, on its wall, its blend face.
        for (i, x) in c.links.iter().enumerate() {
            let e = body.edges[x.edge as usize];
            let j = if c.closed { (i + 1) % k } else { i + 1 };
            // P's use runs from verts[i] to verts[j]; the edge's own direction: v[0] -> v[1].
            let (s0, s1) = if x.rev { (j, i) } else { (i, j) };
            let (c1, c2) = match e.curve {
                Curve::Line => (Curve::Line, Curve::Line),
                Curve::Circle { center, normal, xdir, radius } => {
                    let ps = body.vertices[e.v[0] as usize];
                    let q = into_p_at(body, x, ps);
                    let r1 = radius + q.dot((ps - center).normalize()) * c.d1;
                    (Curve::Circle { center, normal, xdir, radius: r1 }, Curve::Circle { center: center + x.t * c.d2, normal, xdir, radius })
                }
                _ => return no("an edge not a line or an arc"),
            };
            b.edges.push(Edge { v: [v1[s0], v1[s1]], curve: c1 });
            b.edges.push(Edge { v: [v2[s0], v2[s1]], curve: c2 });
            edge_origin.extend([Origin::New, Origin::New]);
            let (e1, e2) = (b.edges.len() as u32 - 2, b.edges.len() as u32 - 1);
            dead_e[x.edge as usize] = true;
            replace_in(&mut b.faces[c.face as usize], x.edge, e1);
            replace_in(&mut b.faces[x.wall as usize], x.edge, e2);
            touch(c.face, &mut touched_faces);
            touch(x.wall, &mut touched_faces);
            // The blend face: e1 the other way from P, the cross-section at the use's start, e2 the other way
            // from the wall (P's way), the cross-section at its end back.
            let lp = vec![
                Use { edge: e1, reversed: !x.rev },
                Use { edge: cross[i], reversed: false },
                Use { edge: e2, reversed: x.rev },
                Use { edge: cross[j], reversed: true },
            ];
            let surface = blend_surface(body, c, x, &e)?;
            // Outward: away from the material; a convex blend's old edge is outside it, a concave one's inside.
            let pm = body.vertices[e.v[0] as usize];
            let q = into_p_at(body, x, pm);
            let (pa, pb) = (pm + q * c.d1, pm + x.t * c.d2);
            let probe = match x.spec {
                Spec::Fillet { radius } => {
                    let ctr = pm + (q + x.t).normalize() * c.centre;
                    ctr + ((pa - ctr) + (pb - ctr)).normalize() * radius
                }
                Spec::Chamfer { .. } => 0.5 * (pa + pb),
            };
            let reversed = surface_normal(&surface, probe).dot(pm - probe) * c.sigma < 0.0;
            new_faces.push((x.edge, Face { surface, reversed, loops: vec![lp] }));
        }
    }
    for (f, l, u) in extra_uses {
        b.faces[f as usize].loops[l as usize].push(u);
    }
    // Pieces of the edges cut short in every face that uses them.
    for f in &mut b.faces {
        for lp in &mut f.loops {
            for u in lp.iter_mut() {
                if let Some(Some(r)) = replace.get(u.edge as usize) {
                    u.edge = *r;
                }
            }
        }
    }
    for f in touched_faces {
        face_origin[f as usize] = Origin::Piece { arg: 0, index: f };
        history.push(Record { kind: Kind::Face, arg: 0, index: f, relation: Relation::Modified, out_kind: Kind::Face, out: f });
    }
    for (e, face) in new_faces {
        b.faces.push(face);
        face_origin.push(Origin::New);
        let fi = b.faces.len() as u32 - 1;
        b.solids[0].shells[0].push(fi);
        history.push(Record { kind: Kind::Edge, arg: 0, index: e, relation: Relation::Generated, out_kind: Kind::Face, out: fi });
        history.push(Record { kind: Kind::Edge, arg: 0, index: e, relation: Relation::Deleted, out_kind: Kind::Edge, out: u32::MAX });
    }
    for (v, &d) in dead_v.iter().enumerate() {
        if d {
            history.push(Record { kind: Kind::Vertex, arg: 0, index: v as u32, relation: Relation::Deleted, out_kind: Kind::Vertex, out: u32::MAX });
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
        if i < ne && dead_e[i] {
            continue;
        }
        emap[i] = es.len() as u32;
        es.push(Edge { v: [vmap[e.v[0] as usize], vmap[e.v[1] as usize]], curve: e.curve });
        eo.push(edge_origin[i]);
    }
    if es.iter().any(|e| e.v.contains(&u32::MAX)) {
        return no("an edge left at a vertex the blend takes");
    }
    for f in &mut b.faces {
        for lp in &mut f.loops {
            for u in lp.iter_mut() {
                u.edge = emap[u.edge as usize];
                if u.edge == u32::MAX {
                    return no("a face left with an edge the blend takes");
                }
            }
        }
    }
    for h in &mut history {
        if h.relation != Relation::Deleted && h.out_kind == Kind::Edge {
            h.out = emap[h.out as usize];
        }
    }
    b.vertices = vs;
    b.edges = es;
    let face_u0 = vec![0.0; b.faces.len()];
    let edge_section = vec![u32::MAX; b.edges.len()];
    history.sort_unstable();
    history.dedup();
    let solid_origin = vec![Origin::Piece { arg: 0, index: 0 }];
    let _ = Solid::default();
    Ok(Output { body: b, vertex_origin: vo, edge_origin: eo, face_origin, solid_origin, face_u0, edge_section, history })
}

/// The blend surface of link `x` (edge `e`).
fn blend_surface(b: &Body, c: &Chain, x: &Link, e: &Edge) -> Result<Surface, Decline> {
    let p0 = b.vertices[e.v[0] as usize];
    let p1 = b.vertices[e.v[1] as usize];
    let q = into_p_at(b, x, p0);
    let (pa, pb) = (p0 + q * c.d1, p0 + x.t * c.d2);
    match (x.spec, e.curve) {
        (Spec::Fillet { radius }, Curve::Line) => {
            // As OCCT frames its fillet cylinders: x towards the tangent line on P, the one on the wall at a
            // positive angle below a half turn.
            let ctr = p0 + (q + x.t).normalize() * c.centre;
            let xdir = (pa - ctr) / radius;
            let mut axis = (p1 - p0).normalize();
            if axis.cross(xdir).dot(pb - ctr) < 0.0 {
                axis = -axis;
            }
            Ok(Surface::Cylinder { origin: ctr, axis, xdir, radius })
        }
        (Spec::Fillet { radius }, Curve::Circle { center, normal, radius: rho, .. }) => {
            let out = (p0 - center).normalize();
            let major = rho + q.dot(out) * c.d1;
            let zdir = normal;
            Ok(Surface::Torus { origin: center + x.t * c.d2, xdir: out, ydir: zdir.cross(out), zdir, major, minor: radius })
        }
        (Spec::Chamfer { .. }, Curve::Line) => {
            let dir = (p1 - p0).normalize();
            Ok(Surface::Plane { origin: pa, normal: dir.cross(pb - pa).normalize() })
        }
        (Spec::Chamfer { .. }, Curve::Circle { center, radius: rho, .. }) => {
            let out = (p0 - center).normalize();
            let r_p = rho + q.dot(out) * c.d1;
            // As OCCT frames it: at the wall's circle (radius rho, d2 along t from P), the radius r_p back at P.
            Ok(Surface::Cone { origin: center + x.t * c.d2, axis: x.t, xdir: out, radius: rho, angle: ((rho - r_p) / c.d2).atan() })
        }
        _ => no("an edge not a line or an arc"),
    }
}

/// A surface's own normal at a point on it.
fn surface_normal(s: &Surface, p: DVec3) -> DVec3 {
    match *s {
        Surface::Plane { normal, .. } => normal,
        Surface::Cylinder { origin, axis, .. } => {
            let r = p - origin;
            (r - axis * r.dot(axis)).normalize()
        }
        Surface::Torus { origin, zdir, major, .. } => {
            let r = p - origin;
            let radial = (r - zdir * r.dot(zdir)).normalize();
            (p - (origin + radial * major)).normalize()
        }
        Surface::Cone { origin, axis, radius, angle, .. } => {
            let r = p - origin;
            let z = r.dot(axis);
            let radial = (r - axis * z).normalize();
            let _ = radius;
            (radial * angle.cos() - axis * angle.sin()).normalize()
        }
        _ => DVec3::ZERO,
    }
}

/// `f`'s uses of `from` now use `to` (the same way).
fn replace_in(f: &mut Face, from: u32, to: u32) {
    for lp in &mut f.loops {
        for u in lp.iter_mut() {
            if u.edge == from {
                u.edge = to;
            }
        }
    }
}
