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

/// One blended edge of a chain: its blend's distances into P (`a`) and along its wall (`b`), the side it
/// takes, and its new entities.
#[derive(Clone, Debug)]
struct Link {
    edge: u32,
    /// P's use of the edge: reversed.
    rev: bool,
    wall: u32,
    spec: Spec,
    a: f64,
    b: f64,
    /// Into P from the edge, square to it (at its start; an arc's turns with it).
    q0: DVec3,
}

/// A chain: a whole loop of the planar face P, its vertices (the one at each link's start, in P's order).
#[derive(Clone, Debug)]
struct Chain {
    face: u32,
    lp: u32,
    links: Vec<Link>,
    /// Per link, the vertex where P's use of it starts.
    starts: Vec<u32>,
    /// P's outward normal; the walls' way from P (−σ n); convex: +1, concave: −1.
    n: DVec3,
    t: DVec3,
    sigma: f64,
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
    let chains = chains(body, &uses, &spec_of, margin)?;
    let ve = vertex_edges(body);
    let boxes: Vec<(DVec3, DVec3)> = body.edges.iter().map(|e| edge_box(body, e)).collect();
    for c in &chains {
        check_clearance(body, &ve, &boxes, c, &chains, margin)?;
    }
    build(body, &uses, &chains, margin)
}

/// The chains `spec_of` makes: whole loops of planar faces, every edge tangent to the next, walls square to the
/// face. Declines anything else.
fn chains(b: &Body, uses: &EdgeUses, spec_of: &[Option<BlendEdge>], margin: f64) -> Result<Vec<Chain>, Decline> {
    let mut owner: Vec<Option<usize>> = vec![None; b.edges.len()];
    let mut out: Vec<Chain> = Vec::new();
    for (f, face) in b.faces.iter().enumerate() {
        let Some(n) = plane_normal(face) else { continue };
        for (l, lp) in face.loops.iter().enumerate() {
            if lp.is_empty() || !lp.iter().all(|u| spec_of[u.edge as usize].is_some()) {
                continue;
            }
            let mut links = Vec::new();
            let mut starts = Vec::new();
            for u in lp {
                let e = &b.edges[u.edge as usize];
                let spec = spec_of[u.edge as usize].unwrap();
                let wall = uses[u.edge as usize].iter().find(|x| x.0 != f as u32).map(|x| x.0);
                let Some(wall) = wall else { return no("an edge with P on both sides") };
                let (a, bb) = match spec.spec {
                    Spec::Fillet { radius } => (radius, radius),
                    Spec::Chamfer { d1, d2 } => {
                        if d1 == d2 || spec.reference == f as u32 {
                            (d1, d2)
                        } else if spec.reference == wall {
                            (d2, d1)
                        } else {
                            return no("a chamfer's reference face not the edge's");
                        }
                    }
                };
                let p_start = b.vertices[e.v[u.reversed as usize] as usize];
                let Some(t_own) = tangent(b, e, u.reversed) else { return no("an edge not a line or an arc") };
                let t_use = if u.reversed { -t_own } else { t_own };
                let q0 = n.cross(t_use).normalize();
                if (q0.dot(n)).abs() > 1e-9 {
                    return no("an edge out of its plane");
                }
                links.push(Link { edge: u.edge, rev: u.reversed, wall, spec: spec.spec, a, b: bb, q0 });
                let _ = p_start;
                starts.push(e.v[u.reversed as usize]);
            }
            for x in &links {
                if owner[x.edge as usize].is_some_and(|o| o != out.len()) {
                    return no("an edge in two chains");
                }
                owner[x.edge as usize] = Some(out.len());
            }
            out.push(Chain { face: f as u32, lp: l as u32, links, starts, n, t: DVec3::ZERO, sigma: 0.0 });
        }
    }
    if spec_of.iter().enumerate().any(|(i, s)| s.is_some() && owner[i].is_none()) {
        return no("an edge not in a whole loop of a planar face");
    }
    for c in &mut out {
        // Each wall square to P along its edge, the same side everywhere.
        let mut sigma = 0.0;
        for x in &c.links {
            let e = &b.edges[x.edge as usize];
            let w = &b.faces[x.wall as usize];
            match (e.curve, w.surface) {
                (Curve::Line, Surface::Plane { .. }) => {}
                (Curve::Circle { center, normal, radius, .. }, Surface::Cylinder { origin, axis, radius: rc, .. }) => {
                    let off = center - origin;
                    if axis.cross(normal).length() > 1e-9 || (off - axis * off.dot(axis)).length() > margin || (radius - rc).abs() > margin {
                        return no("an arc not round its wall's axis");
                    }
                }
                _ => return no("a wall not a plane or a cylinder round the edge"),
            }
            if let Curve::Circle { normal, .. } = e.curve
                && normal.cross(c.n).length() > 1e-9
            {
                return no("an arc not in P");
            }
            let ps = b.vertices[e.v[x.rev as usize] as usize];
            let Some(m) = outward_at(w, ps) else { return no("a wall not a plane or a cylinder") };
            if m.dot(c.n).abs() > 1e-9 {
                return no("a wall not square to P");
            }
            // q into P: −m for a convex edge, +m for a concave one (at the use's start).
            let qm = x.q0.dot(m);
            if (qm.abs() - 1.0).abs() > 1e-9 {
                return no("a wall not square to the edge");
            }
            let s = -qm.signum();
            if sigma != 0.0 && s != sigma {
                return no("a chain both convex and concave");
            }
            sigma = s;
        }
        c.sigma = sigma;
        c.t = -sigma * c.n;
        // Tangent at every vertex.
        let k = c.links.len();
        for i in 0..k {
            let x = &c.links[i];
            let y = &c.links[(i + 1) % k];
            let (ex, ey) = (&b.edges[x.edge as usize], &b.edges[y.edge as usize]);
            let tx = tangent(b, ex, !x.rev).unwrap() * if x.rev { -1.0 } else { 1.0 };
            let ty = tangent(b, ey, y.rev).unwrap() * if y.rev { -1.0 } else { 1.0 };
            if tx.dot(ty) < 1.0 - 1e-12 {
                return no("a corner between blended edges");
            }
            if x.a != y.a || x.b != y.b || x.spec != y.spec {
                return no("blends that change along a chain");
            }
        }
        // Arcs whose radius the blend takes away.
        for x in &c.links {
            if let Curve::Circle { center, radius, .. } = b.edges[x.edge as usize].curve {
                let ps = b.vertices[b.edges[x.edge as usize].v[x.rev as usize] as usize];
                let out = (ps - center).normalize();
                let into_p = x.q0.dot(out);
                if radius + into_p * x.a <= 100.0 * margin {
                    return no("an arc the blend closes up");
                }
                if let Spec::Fillet { radius: r } = x.spec {
                    // The fillet's centre circle round the axis, not through it.
                    if radius + into_p * r <= r + 100.0 * margin && into_p < 0.0 {
                        return no("a fillet over an arc of its own radius or less");
                    }
                }
            }
        }
    }
    Ok(out)
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

/// The edge down the walls from each of the chain's vertices: one line, along `t`, longer than the blend.
fn down_edges(b: &Body, c: &Chain, ve: &[Vec<u32>], margin: f64) -> Result<Vec<u32>, Decline> {
    let mut out = Vec::new();
    for (i, &v) in c.starts.iter().enumerate() {
        let chain_edges = [c.links[i].edge, c.links[(i + c.links.len() - 1) % c.links.len()].edge];
        let others: Vec<u32> = ve[v as usize].iter().copied().filter(|e| !chain_edges.contains(e)).collect();
        let [s] = others[..] else { return no("a chain vertex with more than one edge down") };
        let e = &b.edges[s as usize];
        if e.curve != Curve::Line {
            return no("an edge down the walls not a line");
        }
        let far = if e.v[0] == v { e.v[1] } else { e.v[0] };
        let d = b.vertices[far as usize] - b.vertices[v as usize];
        if d.normalize().dot(c.t) < 1.0 - 1e-12 {
            return no("an edge down the walls not along them");
        }
        if d.length() <= c.links[i].b + margin {
            return no("a wall shorter than the blend");
        }
        out.push(s);
    }
    Ok(out)
}

/// The blend's strips on P and on the walls are clear of every other edge of those faces, and the chain of
/// any other's.
fn check_clearance(b: &Body, ve: &[Vec<u32>], boxes: &[(DVec3, DVec3)], c: &Chain, all: &[Chain], margin: f64) -> Result<(), Decline> {
    let downs = down_edges(b, c, ve, margin)?;
    let chain_edges: Vec<u32> = c.links.iter().map(|x| x.edge).collect();
    let reach = c.links.iter().fold(0.0f64, |m, x| m.max(x.a).max(x.b));
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
    // P: its other edges.
    let pface = &b.faces[c.face as usize];
    for (l, lp) in pface.loops.iter().enumerate() {
        if l as u32 == c.lp {
            continue;
        }
        for u in lp {
            for x in &c.links {
                if !far(x.edge, u.edge, x.a)? {
                    return no("another edge of P within the blend");
                }
            }
        }
    }
    // The chain's own non-neighbouring edges.
    let k = c.links.len();
    for i in 0..k {
        for j in i + 2..k {
            if i == 0 && j == k - 1 {
                continue;
            }
            if !far(c.links[i].edge, c.links[j].edge, 2.0 * reach)? {
                return no("a chain whose blend meets itself");
            }
        }
    }
    // Each wall: its edges other than the chain's and the ones down.
    for x in &c.links {
        for lp in &b.faces[x.wall as usize].loops {
            for u in lp {
                if chain_edges.contains(&u.edge) || downs.contains(&u.edge) {
                    continue;
                }
                if !far(x.edge, u.edge, x.b)? {
                    return no("another edge of a wall within the blend");
                }
            }
        }
    }
    // Other chains' edges.
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

/// The result: P and the walls trimmed, the blend faces between.
fn build(body: &Body, uses: &EdgeUses, chains: &[Chain], margin: f64) -> Result<Output, Decline> {
    let ve = vertex_edges(body);
    let mut b = body.clone();
    let nv = body.vertices.len();
    let ne = body.edges.len();
    let nf = body.faces.len();
    let mut vertex_origin: Vec<Origin> = (0..nv as u32).map(|i| Origin::Same { arg: 0, index: i }).collect();
    let mut edge_origin: Vec<Origin> = (0..ne as u32).map(|i| Origin::Same { arg: 0, index: i }).collect();
    let mut face_origin: Vec<Origin> = (0..nf as u32).map(|i| Origin::Same { arg: 0, index: i }).collect();
    let mut history = vec![Record { kind: Kind::Solid, arg: 0, index: 0, relation: Relation::Modified, out_kind: Kind::Solid, out: 0 }];
    let mut dead_v = vec![false; nv];
    let mut dead_e = vec![false; ne];
    // Edge replaced by edge, in every loop.
    let mut replace: Vec<Option<u32>> = vec![None; ne];
    let mut touched_faces: Vec<u32> = Vec::new();
    let mut new_faces: Vec<(u32, Face)> = Vec::new();
    for c in chains {
        let k = c.links.len();
        let downs = down_edges(body, c, &ve, margin)?;
        // New vertices at each chain vertex: on P (v1), on the walls (v2); the cross-section edge between.
        let mut v1 = Vec::with_capacity(k);
        let mut v2 = Vec::with_capacity(k);
        let mut cross = Vec::with_capacity(k);
        for (i, (&v, x)) in c.starts.iter().zip(&c.links).enumerate() {
            let p = body.vertices[v as usize];
            let q = into_p(body, c, x, p);
            let p1 = p + q * x.a;
            let p2 = p + c.t * x.b;
            b.vertices.push(p1);
            vertex_origin.push(Origin::New);
            b.vertices.push(p2);
            vertex_origin.push(Origin::New);
            let (i1, i2) = (b.vertices.len() as u32 - 2, b.vertices.len() as u32 - 1);
            v1.push(i1);
            v2.push(i2);
            dead_v[v as usize] = true;
            let curve = match x.spec {
                Spec::Fillet { radius } => {
                    let ctr = p + (q + c.t) * radius;
                    let normal = (p1 - ctr).cross(p2 - ctr).normalize();
                    Curve::Circle { center: ctr, normal, xdir: (p1 - ctr) / radius, radius }
                }
                Spec::Chamfer { .. } => Curve::Line,
            };
            b.edges.push(Edge { v: [i1, i2], curve });
            edge_origin.push(Origin::New);
            cross.push(b.edges.len() as u32 - 1);
            // The edge down, from the wall vertex.
            let s = downs[i];
            let mut se = body.edges[s as usize];
            if se.v[0] == v {
                se.v[0] = i2;
            } else {
                se.v[1] = i2;
            }
            b.edges.push(se);
            edge_origin.push(Origin::Piece { arg: 0, index: s });
            let si = b.edges.len() as u32 - 1;
            replace[s as usize] = Some(si);
            dead_e[s as usize] = true;
            history.push(Record { kind: Kind::Edge, arg: 0, index: s, relation: Relation::Modified, out_kind: Kind::Edge, out: si });
        }
        // Each link: its edge on P, on its wall, its blend face.
        for i in 0..k {
            let x = &c.links[i];
            let e = body.edges[x.edge as usize];
            let j = (i + 1) % k;
            // P's use runs from starts[i] to starts[j]; the edge's own direction: v[0] -> v[1].
            let (s0, s1) = if x.rev { (j, i) } else { (i, j) };
            let (c1, c2) = match e.curve {
                Curve::Line => (Curve::Line, Curve::Line),
                Curve::Circle { center, normal, xdir, radius } => {
                    let ps = body.vertices[e.v[0] as usize];
                    let out = (ps - center).normalize();
                    // Into P from the edge's start along its own direction: the use's q, or minus it.
                    let q = into_p(body, c, x, ps);
                    let r1 = radius + q.dot(out) * x.a;
                    (Curve::Circle { center, normal, xdir, radius: r1 }, Curve::Circle { center: center + c.t * x.b, normal, xdir, radius })
                }
                _ => return no("an edge not a line or an arc"),
            };
            b.edges.push(Edge { v: [v1[s0], v1[s1]], curve: c1 });
            edge_origin.push(Origin::New);
            let e1 = b.edges.len() as u32 - 1;
            b.edges.push(Edge { v: [v2[s0], v2[s1]], curve: c2 });
            edge_origin.push(Origin::New);
            let e2 = b.edges.len() as u32 - 1;
            dead_e[x.edge as usize] = true;
            // P's loop takes e1 for the edge, the wall's loops e2.
            replace_in(&mut b.faces[c.face as usize], x.edge, e1);
            replace_in(&mut b.faces[x.wall as usize], x.edge, e2);
            for f in [c.face, x.wall] {
                if !touched_faces.contains(&f) {
                    touched_faces.push(f);
                }
            }
            // The blend face: e1 the other way from P, the cross-section at P's start of the use, e2 the
            // other way from the wall (P's way), the cross-section at its end back.
            let lp = vec![
                Use { edge: e1, reversed: !x.rev },
                Use { edge: cross[i], reversed: false },
                Use { edge: e2, reversed: x.rev },
                Use { edge: cross[j], reversed: true },
            ];
            let surface = blend_surface(body, c, x, &e)?;
            // Outward: away from the material; a convex blend's old edge is outside it, a concave one's inside.
            let pm = body.vertices[e.v[0] as usize];
            let q = into_p(body, c, x, pm);
            let (pa, pb) = (pm + q * x.a, pm + c.t * x.b);
            let probe = match x.spec {
                Spec::Fillet { radius } => {
                    let ctr = pm + (q + c.t) * radius;
                    ctr + ((pa - ctr) + (pb - ctr)).normalize() * radius
                }
                Spec::Chamfer { .. } => 0.5 * (pa + pb),
            };
            let sn = surface_normal(&surface, probe);
            let reversed = sn.dot(pm - probe) * c.sigma < 0.0;
            let lp = if k == 1 { vec![lp[0], lp[1], lp[2], Use { edge: cross[0], reversed: true }] } else { lp };
            new_faces.push((x.edge, Face { surface, reversed, loops: vec![lp] }));
        }
    }
    // Pieces of the edges down in every face that uses them.
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
    let mut vs = Vec::new();
    let mut vo = Vec::new();
    for (i, p) in b.vertices.iter().enumerate() {
        if i < nv && dead_v[i] {
            continue;
        }
        vmap[i] = vs.len() as u32;
        vs.push(*p);
        vo.push(vertex_origin[i]);
    }
    let mut emap = vec![u32::MAX; b.edges.len()];
    let mut es = Vec::new();
    let mut eo = Vec::new();
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
    let _ = uses;
    let face_u0 = vec![0.0; b.faces.len()];
    let edge_section = vec![u32::MAX; b.edges.len()];
    history.sort_unstable();
    history.dedup();
    let solid_origin = vec![Origin::Piece { arg: 0, index: 0 }];
    let _ = Solid::default();
    Ok(Output { body: b, vertex_origin: vo, edge_origin: eo, face_origin, solid_origin, face_u0, edge_section, history })
}

/// Into P from link `x`'s edge at its point `p`, square to it.
fn into_p(b: &Body, c: &Chain, x: &Link, p: DVec3) -> DVec3 {
    match b.edges[x.edge as usize].curve {
        Curve::Circle { center, normal, .. } => {
            // q turns with the arc: at the use's start it is q0; elsewhere the same side of the radius.
            let ps = b.vertices[b.edges[x.edge as usize].v[x.rev as usize] as usize];
            let side = x.q0.dot((ps - center).normalize());
            let r = p - center;
            (r - normal * r.dot(normal)).normalize() * side
        }
        _ => {
            let _ = c;
            x.q0
        }
    }
}

/// The blend surface of link `x` (edge `e`).
fn blend_surface(b: &Body, c: &Chain, x: &Link, e: &Edge) -> Result<Surface, Decline> {
    let p0 = b.vertices[e.v[0] as usize];
    let p1 = b.vertices[e.v[1] as usize];
    match (x.spec, e.curve) {
        (Spec::Fillet { radius }, Curve::Line) => {
            let q = into_p(b, c, x, p0);
            Ok(Surface::Cylinder { origin: p0 + (q + c.t) * radius, axis: (p1 - p0).normalize(), xdir: -c.t, radius })
        }
        (Spec::Fillet { radius }, Curve::Circle { center, normal, radius: rho, .. }) => {
            let out = (p0 - center).normalize();
            let q = into_p(b, c, x, p0);
            let major = rho + q.dot(out) * radius;
            let zdir = normal;
            let xdir = out;
            Ok(Surface::Torus { origin: center + c.t * radius, xdir, ydir: zdir.cross(xdir), zdir, major, minor: radius })
        }
        (Spec::Chamfer { .. }, Curve::Line) => {
            let q = into_p(b, c, x, p0);
            let (pa, pb) = (p0 + q * x.a, p0 + c.t * x.b);
            let dir = (p1 - p0).normalize();
            let normal = dir.cross(pb - pa).normalize();
            Ok(Surface::Plane { origin: pa, normal })
        }
        (Spec::Chamfer { .. }, Curve::Circle { center, normal, radius: rho, .. }) => {
            let out = (p0 - center).normalize();
            let q = into_p(b, c, x, p0);
            let r_p = rho + q.dot(out) * x.a;
            // As OCCT frames it: at the wall's circle (radius rho, b along t from P), the radius r_p back at P.
            let _ = normal;
            Ok(Surface::Cone { origin: center + c.t * x.b, axis: c.t, xdir: out, radius: rho, angle: ((rho - r_p) / x.b).atan() })
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
