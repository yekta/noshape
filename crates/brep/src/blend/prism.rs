//! Rounds of straight edges running between two faces ruled along them (planes along the edge, cylinders about
//! axes along it), each edge ending on planar caps square to it at vertices of three edges (stef-radijator's fins
//! round a hub, between the fin's end and the gaps' cylinders): each round is the section's 2D fillet swept between
//! the caps, a cylinder touching both faces along a line; the caps' corners at its ends become its arcs.

use super::{no, BlendEdge, EdgeUses, Spec};
use crate::{Body, Curve, Decline, Edge, Face, Kind, Origin, Output, Record, Relation, Surface, Use};
use glam::{DVec2, DVec3};

/// A face along direction `d` as seen in the plane square to `d` through `o`: a line or a circle, its offset
/// towards the outward normal by `s` (2D, in the plane's basis `u`, `w`).
enum Trace {
    Line { p: DVec2, n: DVec2 },
    Circle { c: DVec2, r: f64 },
}

/// A face ruled along `d` (a plane along it, a cylinder about an axis along it) cut square to `d` at `o`, its
/// outward side offset by `s`; None for any other face.
fn trace(f: &Face, d: DVec3, o: DVec3, u: DVec3, w: DVec3, s: f64) -> Option<Trace> {
    let flat = |p: DVec3| DVec2::new((p - o).dot(u), (p - o).dot(w));
    let sign = if f.reversed { -1.0 } else { 1.0 };
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
    };
    roots.into_iter().min_by(|x, y| (*x - near).length().total_cmp(&(*y - near).length()))
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

/// One round's end: the cap, the vertex, the edges along the round's two faces there, the ball's centre and the
/// two tangent points; or (no cap) a corner of earlier rounds.
struct End {
    cap: u32,
    v: u32,
    g: [u32; 2],
    c: DVec3,
    t: [DVec3; 2],
    corner: Option<Corner>,
}

/// An end at two earlier rounds (tori, the round's radius) beside its faces: their meeting edge `m` (to vertex
/// `p`, the sphere's pole) and the tori beside the end edges.
struct Corner {
    m: u32,
    tori: [u32; 2],
    p: u32,
}

/// One round: its edge, radius, two faces, convex (+1) or concave (−1), its two ends (the edge's order).
struct Plan {
    edge: u32,
    r: f64,
    fs: [u32; 2],
    sigma: f64,
    ends: [End; 2],
}

/// The rounds of `edges` (fillets only), every one such an edge; a decline otherwise.
pub(super) fn blend(body: &Body, edges: &[BlendEdge], uses: &EdgeUses, ve: &[Vec<u32>], margin: f64) -> Result<Output, Decline> {
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
        let Spec::Fillet { radius: r } = be.spec else { return no("a prism round not a fillet") };
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
                    _ => None,
                }
            };
            let (Some(qa), Some(_)) = (away(g[0]), away(g[1])) else { return no("a prism round's end edge not a line or an arc") };
            if sigma == 0.0 {
                let fb = &body.faces[fs[1] as usize];
                let nb = match fb.surface {
                    Surface::Plane { normal, .. } => normal,
                    Surface::Cylinder { origin, axis, .. } => {
                        let a = axis.normalize();
                        (p - origin - a * (p - origin).dot(a)).normalize()
                    }
                    _ => return no("a prism round's face not ruled along it"),
                } * if fb.reversed { -1.0 } else { 1.0 };
                sigma = if qa.dot(nb) < 0.0 { 1.0 } else { -1.0 };
            }
            let u = d.any_orthonormal_vector();
            let w = d.cross(u);
            let (Some(ta), Some(tb)) = (trace(&body.faces[fs[0] as usize], d, p, u, w, -sigma * r), trace(&body.faces[fs[1] as usize], d, p, u, w, -sigma * r)) else {
                return no("a prism round's face not ruled along it");
            };
            let Some(c2) = meet(&ta, &tb, DVec2::ZERO) else { return no("a prism round's faces' offsets don't meet") };
            if c2.length() > 10.0 * r {
                return no("a prism round's centre far from its edge");
            }
            let c = p + u * c2.x + w * c2.y;
            let (Some(t0), Some(t1)) = (foot(&body.faces[fs[0] as usize], c, d), foot(&body.faces[fs[1] as usize], c, d)) else { return no("a prism round's tangent points") };
            // Each tangent point on its end edge, short of its far end (no overflow).
            for (k, t) in [t0, t1].into_iter().enumerate() {
                let oe = &body.edges[g[k] as usize];
                let far = body.vertices[if oe.v[0] == v { oe.v[1] } else { oe.v[0] } as usize];
                let reach = (t - p).length();
                let room = match oe.curve {
                    Curve::Line => (far - p).length(),
                    Curve::Circle { center, radius, .. } => {
                        let (a, b) = ((p - center).normalize(), (far - center).normalize());
                        radius * a.dot(b).clamp(-1.0, 1.0).acos()
                    }
                    _ => 0.0,
                };
                if reach < margin || reach + margin >= room {
                    return no("a prism round past its end edges");
                }
            }
            if let Some(cn) = &corner
                && ((body.vertices[cn.p as usize] - c).length() - r).abs() > 100.0 * margin
            {
                return no("a prism round's corner not at its rounds' meeting point");
            }
            ends.push(End { cap, v, g, c, t: [t0, t1], corner });
        }
        let [a, b] = <[End; 2]>::try_from(ends).map_err(|_| Decline("a prism round without two ends"))?;
        if (b.c - a.c).cross(d).length() > margin {
            return no("a prism round's ends not on one axis");
        }
        plans.push(Plan { edge: be.edge, r, fs, sigma, ends: [a, b] });
    }
    if (0..body.edges.len()).any(|i| absorbed[i] && !taken_m[i]) {
        return no("a curved edge not a prism round's corner");
    }
    build(body, plans)
}

fn build(body: &Body, plans: Vec<Plan>) -> Result<Output, Decline> {
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
    for Plan { edge, r, fs, sigma, ends } in plans {
        let e = body.edges[edge as usize];
        let d = (body.vertices[e.v[1] as usize] - body.vertices[e.v[0] as usize]).normalize();
        // Per end: the tangent points' vertices, the end edges cut back to them, the cap's arc between.
        let mut tv = [[0u32; 2]; 2];
        let mut arcs = [0u32; 2];
        let mut spheres = Vec::new();
        for (j, end) in ends.iter().enumerate() {
            dead_v[end.v as usize] = true;
            for (k, t) in tv[j].iter_mut().enumerate() {
                *t = add_v(&mut b, end.t[k], &mut vertex_origin);
                let g = end.g[k] as usize;
                let slot = if b.edges[g].v[0] == end.v { 0 } else { 1 };
                b.edges[g].v[slot] = *t;
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
            b.edges.push(Edge { v: [tv[j][from], tv[j][to]], curve: Curve::Circle { center: end.c, normal, xdir, radius: r } });
            edge_origin.push(Origin::New);
            arcs[j] = b.edges.len() as u32 - 1;
            let cap_loop = b.faces[end.cap as usize].loops.iter().position(|lp| lp.iter().any(|x| x.edge == end.g[0])).unwrap_or(0);
            b.faces[end.cap as usize].loops[cap_loop].push(Use { edge: arcs[j], reversed: false });
            touched.push(end.cap);
        }
        // The lines where the round touches its faces, the old edge's way (its ends in its order; its faces use
        // them as they used it).
        let mut lines = [0u32; 2];
        for k in 0..2 {
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
        for (f, o) in [(fs[0], lines[0]), (fs[1], lines[1])] {
            let Some(rev) = use_in(&b, f, o) else { return no("a prism round's edge not in its face") };
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
        let xdir = (ends[0].t[0] - c).normalize();
        let axis = if d.cross(xdir).dot(ends[0].t[1] - c) >= 0.0 { d } else { -d };
        new_faces.push((edge, Face { surface: Surface::Cylinder { origin: c, axis, xdir, radius: r }, reversed: sigma < 0.0, loops: vec![lp] }));
    }
    touched.sort_unstable();
    touched.dedup();
    for f in touched {
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
