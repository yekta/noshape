//! The common of a line (an edge alone) and a solid: the pieces of the line inside it, with OCCT's history.

use super::*;

/// Where the line meets the solid's boundary.
#[derive(Clone, Copy, PartialEq)]
enum At {
    Vertex(u32),
    Edge(usize),
    Face(usize),
}

impl At {
    fn rank(self) -> u8 {
        match self {
            At::Vertex(_) => 2,
            At::Edge(_) => 1,
            At::Face(_) => 0,
        }
    }
}

/// The common of the line `cx.edges[0]` (argument 0, nothing else) and argument 1's one solid.
pub(super) fn line_common(cx: &Ctx, tol: f64) -> R<Output> {
    if cx.solids.len() != 1 {
        return Err(Decline("a line and other than one solid"));
    }
    let line = &cx.edges[0];
    let (a, b) = (cx.p(line.v[0]), cx.p(line.v[1]));
    let len = (b - a).length();
    let d = (b - a) / len;
    let sol = &cx.solids[0];
    if !boxes_meet(a.min(b), a.max(b), sol.lo, sol.hi, GREY * tol) {
        return Err(Decline("a line apart from the solid"));
    }
    // (t along the line, what it meets there.)
    let mut cuts: Vec<(f64, At)> = Vec::new();
    // The solid's vertices on it.
    for (w, ins) in cx.pool.inputs.iter().enumerate() {
        if !ins.iter().any(|x| x.0 == 1) {
            continue;
        }
        let q = cx.pool.p[w];
        let t = (q - a).dot(d);
        let dist = (a + d * t - q).length();
        if t < -GREY * tol || t > len + GREY * tol || dist > GREY * tol {
            continue;
        }
        if dist > tol {
            return Err(Decline("a line passing near a vertex"));
        }
        cuts.push((t, At::Vertex(w as u32)));
    }
    let near_vertex = |cuts: &[(f64, At)], t: f64| cuts.iter().any(|c| matches!(c.1, At::Vertex(_)) && (c.0 - t).abs() <= GREY * tol);
    // Its edges it crosses, and the ones it runs along.
    let vertex_cuts = cuts.clone();
    let mut along_edges: Vec<usize> = Vec::new();
    let mut along_faces: Vec<usize> = Vec::new();
    for (e, g) in cx.edges.iter().enumerate() {
        if g.arg != 1 {
            continue;
        }
        let mut at: Vec<DVec3> = Vec::new();
        match g.g {
            Geo::Apex { .. } => continue,
            Geo::Ell { .. } => return Err(Decline("an argument's ellipse")),
            Geo::Nurbs { .. } => return Err(Decline("a line and a curved edge")),
            Geo::Line => {
                let (p0, p1) = (cx.p(g.v[0]), cx.p(g.v[1]));
                let el = (p1 - p0).length();
                let ed = (p1 - p0) / el;
                let n = d.cross(ed);
                if n.length() < 1e-9 {
                    let off = ((p0 - a) - d * (p0 - a).dot(d)).length();
                    if off <= tol {
                        along_edges.push(e);
                    } else if off <= GREY * tol {
                        return Err(Decline("a line nearly along an edge"));
                    }
                    continue;
                }
                // Closest points: a + d t, p0 + ed u.
                let w = a - p0;
                let (bb, dd, ee) = (d.dot(ed), d.dot(w), ed.dot(w));
                let den = 1.0 - bb * bb;
                let t = (bb * ee - dd) / den;
                let u = (ee - bb * dd) / den;
                if u < -GREY * tol || u > el + GREY * tol {
                    continue;
                }
                at.push(p0 + ed * u.clamp(0.0, el));
                let _ = t;
            }
            Geo::Arc { k, .. } => {
                let den = d.dot(k.n);
                if den.abs() > 1e-9 {
                    let t = (k.c - a).dot(k.n) / den;
                    at.push(a + d * t);
                } else if (a - k.c).dot(k.n).abs() <= GREY * tol {
                    // In the circle's plane: where it meets the circle.
                    let w = a - k.c;
                    let (qb, qc) = (w.dot(d), w.length_squared() - k.r * k.r);
                    let disc = qb * qb - qc;
                    if disc >= 0.0 {
                        if disc.sqrt() <= GREY * tol {
                            return Err(Decline("a line touching an arc"));
                        }
                        at.push(a + d * (-qb - disc.sqrt()));
                        at.push(a + d * (-qb + disc.sqrt()));
                    }
                }
            }
        }
        let s = cx.edge_seg(e);
        for q in at {
            let t = (q - a).dot(d);
            let off = (a + d * t - q).length();
            let (u, dist) = cx.seg_param(&s, q);
            let dist = dist.max(off);
            if t < -GREY * tol || t > len + GREY * tol || dist > GREY * tol || u < -1e-9 || u > 1.0 + 1e-9 {
                continue;
            }
            if near_vertex(&vertex_cuts, t) {
                continue;
            }
            if dist > tol {
                return Err(Decline("a line passing near an edge"));
            }
            cuts.push((t, At::Edge(e)));
        }
    }
    // Its faces it pierces.
    for &f in &sol.faces {
        let g = &cx.faces[f];
        let mut ts: Vec<f64> = Vec::new();
        match g.s {
            Surf::Nurbs { .. } => return Err(Decline("a line and a curved face")),
            Surf::Plane { n, d: dn, .. } => {
                let den = n.dot(d);
                let dist = dn - n.dot(a);
                if den.abs() < 1e-9 {
                    if dist.abs() <= tol {
                        along_faces.push(f);
                    } else if dist.abs() <= GREY * tol && reaches(cx, f, a, b)? {
                        return Err(Decline("a line nearly along a face"));
                    }
                    continue;
                }
                ts.push(dist / den);
            }
            Surf::Cyl { o, a: ax, r0, k, .. } => {
                let w = a - o;
                let (h, dz) = (w.dot(ax), d.dot(ax));
                let (wp, rp) = (w - ax * h, d - ax * dz);
                let rad = r0 + k * h;
                let (qa, qb, qc) = (rp.length_squared() - k * k * dz * dz, wp.dot(rp) - k * dz * rad, wp.length_squared() - rad * rad);
                if qa.abs() < 1e-12 {
                    if qb.abs() < 1e-12 {
                        // (|w⊥|² - rad²: about 2 rad × the distance.)
                        if qc.abs() <= tol * (2.0 * rad.abs() + tol) {
                            along_faces.push(f);
                        } else if qc.abs() <= GREY * tol * (2.0 * rad.abs() + GREY * tol) && reaches(cx, f, a, b)? {
                            return Err(Decline("a line nearly along a face"));
                        }
                        continue;
                    }
                    ts.push(-qc / (2.0 * qb));
                } else {
                    let disc = qb * qb - qa * qc;
                    if disc < 0.0 {
                        continue;
                    }
                    if disc.sqrt() / qa.abs() <= GREY * tol {
                        // Touching it: at a vertex (a cone's apex), that vertex is the cut; else declined.
                        let t = -qb / qa;
                        if near_vertex(&vertex_cuts, t) {
                            continue;
                        }
                        return Err(Decline("a line touching a face"));
                    }
                    ts.push((-qb - disc.sqrt()) / qa);
                    ts.push((-qb + disc.sqrt()) / qa);
                }
                ts.retain(|&t| r0 + k * (h + t * dz) > -tol);
            }
        }
        for t in ts {
            if t < -GREY * tol || t > len + GREY * tol {
                continue;
            }
            let q = a + d * t;
            if !boxes_meet(q, q, g.lo, g.hi, GREY * tol) {
                continue;
            }
            let q2 = g.to2(q);
            let bound = cx.bound(f)?;
            if distance2(q2, bound) <= GREY * tol {
                // On its boundary: an edge or vertex has it.
                if !cuts.iter().any(|c| !matches!(c.1, At::Face(_)) && (c.0 - t).abs() <= GREY * tol) {
                    return Err(Decline("a line meeting a face's boundary"));
                }
                continue;
            }
            if inside2(q2, bound) {
                cuts.push((t, At::Face(f)));
            }
        }
    }
    cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
    // One cut per place: a vertex over an edge over a face; two edges or faces there alone: declined.
    let mut merged: Vec<(f64, At)> = Vec::new();
    for c in cuts {
        match merged.last_mut() {
            Some(last) if c.0 - last.0 <= GREY * tol => {
                if c.1.rank() > last.1.rank() {
                    *last = c;
                } else if c.1.rank() == last.1.rank() && c.1 != last.1 && c.1.rank() < 2 {
                    return Err(Decline("a line meeting two edges or faces at once"));
                }
            }
            _ => merged.push(c),
        }
    }
    if merged.iter().any(|c| c.0 <= GREY * tol || c.0 >= len - GREY * tol) {
        return Err(Decline("a line ending on the solid's boundary"));
    }
    // The pieces inside.
    let mut ts: Vec<f64> = vec![0.0];
    ts.extend(merged.iter().map(|c| c.0));
    ts.push(len);
    // (A piece along an edge or on a face is on the solid's boundary: in the common, as OCCT's; the edge's
    // piece is its image too.)
    let mut inside = Vec::with_capacity(ts.len() - 1);
    let mut along: Vec<Option<usize>> = Vec::with_capacity(ts.len() - 1);
    for k in 0..ts.len() - 1 {
        let m = a + d * (0.5 * (ts[k] + ts[k + 1]));
        let on_edge = along_edges.iter().copied().find(|&e| {
            let (u, dist) = cx.seg_param(&cx.edge_seg(e), m);
            dist <= tol && u > 0.0 && u < 1.0
        });
        let mut on_face = false;
        for &f in &along_faces {
            let q = cx.faces[f].to2(m);
            let bound = cx.bound(f)?;
            if distance2(q, bound) <= GREY * tol {
                if on_edge.is_none() {
                    return Err(Decline("a line piece near a face's boundary"));
                }
            } else if inside2(q, bound) {
                on_face = true;
            }
        }
        along.push(on_edge);
        inside.push(if on_edge.is_some() || on_face {
            true
        } else {
            match cx.ray_state(m, 0)? {
                State::In => true,
                State::Out => false,
                _ => return Err(Decline("a line piece on the solid's boundary")),
            }
        });
    }
    let mut out = Output::default();
    // Output vertex of each place along the line (0: its start, last: its end).
    let mut vat: Vec<Option<u32>> = vec![None; ts.len()];
    let mut vertex = |out: &mut Output, k: usize| -> u32 {
        if let Some(v) = vat[k] {
            return v;
        }
        let v = out.body.vertices.len() as u32;
        out.body.vertices.push(match k {
            0 => a,
            _ if k == ts.len() - 1 => b,
            _ => match merged[k - 1].1 {
                At::Vertex(w) => cx.pool.p[w as usize],
                _ => a + d * ts[k],
            },
        });
        out.vertex_origin.push(if k == 0 || k == ts.len() - 1 {
            let w = line.v[if k == 0 { 0 } else { 1 }];
            let (arg, index) = cx.pool.inputs[w as usize][0];
            Origin::Same { arg, index }
        } else {
            Origin::New
        });
        vat[k] = Some(v);
        v
    };
    let mut edge_images: Vec<(usize, u32)> = Vec::new();
    for (k, &kept) in inside.iter().enumerate() {
        if kept {
            let v = [vertex(&mut out, k), vertex(&mut out, k + 1)];
            if let Some(e) = along[k] {
                edge_images.push((e, out.body.edges.len() as u32));
            }
            out.body.edges.push(Edge { v, curve: Curve::Line });
        }
    }
    let whole = inside.len() == 1 && inside[0];
    for _ in 0..out.body.edges.len() {
        out.edge_origin.push(if whole { Origin::Same { arg: 0, index: line.index } } else { Origin::Piece { arg: 0, index: line.index } });
    }
    // History.
    let rec = |kind: Kind, arg: u32, index: u32, relation: Relation, out_kind: Kind, o: u32| Record { kind, arg, index, relation, out_kind, out: o };
    let h = &mut out.history;
    if !whole {
        if out.body.edges.is_empty() {
            h.push(rec(Kind::Edge, 0, line.index, Relation::Deleted, Kind::Edge, u32::MAX));
        }
        for o in 0..out.body.edges.len() as u32 {
            h.push(rec(Kind::Edge, 0, line.index, Relation::Modified, Kind::Edge, o));
        }
    }
    for (k, end) in [(0, 0), (ts.len() - 1, 1)] {
        if vat[k].is_none() {
            let (arg, index) = cx.pool.inputs[line.v[end] as usize][0];
            h.push(rec(Kind::Vertex, arg, index, Relation::Deleted, Kind::Vertex, u32::MAX));
        }
    }
    let mut made_face = vec![false; cx.faces.len()];
    let mut made_edge = vec![false; cx.edges.len()];
    let mut moved: HashMap<u32, u32> = HashMap::default();
    for (k, c) in merged.iter().enumerate() {
        let Some(o) = vat[k + 1] else { continue };
        match c.1 {
            At::Vertex(w) => {
                moved.insert(w, o);
            }
            At::Edge(e) => {
                h.push(rec(Kind::Edge, 0, line.index, Relation::Generated, Kind::Vertex, o));
                h.push(rec(Kind::Edge, 1, cx.edges[e].index, Relation::Generated, Kind::Vertex, o));
                made_edge[e] = true;
            }
            At::Face(f) => {
                h.push(rec(Kind::Edge, 0, line.index, Relation::Generated, Kind::Vertex, o));
                h.push(rec(Kind::Face, 1, cx.faces[f].index, Relation::Generated, Kind::Vertex, o));
                made_face[f] = true;
            }
        }
    }
    for (w, ins) in cx.pool.inputs.iter().enumerate() {
        for &(arg, index) in ins.iter().filter(|x| x.0 == 1) {
            match moved.get(&(w as u32)) {
                Some(&o) => h.push(rec(Kind::Vertex, arg, index, Relation::Modified, Kind::Vertex, o)),
                None => h.push(rec(Kind::Vertex, arg, index, Relation::Deleted, Kind::Vertex, u32::MAX)),
            }
        }
    }
    for &(e, o) in &edge_images {
        h.push(rec(Kind::Edge, 1, cx.edges[e].index, Relation::Modified, Kind::Edge, o));
    }
    for (e, g) in cx.edges.iter().enumerate() {
        if g.arg == 1 && !made_edge[e] && !edge_images.iter().any(|x| x.0 == e) {
            h.push(rec(Kind::Edge, 1, g.index, Relation::Deleted, Kind::Edge, u32::MAX));
        }
    }
    for (f, g) in cx.faces.iter().enumerate() {
        if !made_face[f] {
            h.push(rec(Kind::Face, 1, g.index, Relation::Deleted, Kind::Face, u32::MAX));
        }
    }
    h.push(rec(Kind::Solid, 1, sol.index, Relation::Deleted, Kind::Solid, u32::MAX));
    Ok(out)
}

/// Whether the segment `a`–`b`, lying on face `f`'s surface, reaches the face (within the grey zone).
fn reaches(cx: &Ctx, f: usize, a: DVec3, b: DVec3) -> R<bool> {
    let g = &cx.faces[f];
    if !boxes_meet(a.min(b), a.max(b), g.lo, g.hi, GREY * cx.tol) {
        return Ok(false);
    }
    let bound = cx.bound(f)?;
    let (qa, qb) = (g.to2(a), g.to2(b));
    // (On a cylinder: a generator, one angle.)
    let seg = C2::Seg { a: qa, b: DVec2::new(qa.x, qb.y).lerp(qb, if matches!(g.s, Surf::Plane { .. }) { 1.0 } else { 0.0 }) };
    if inside2(seg.start(), bound) || inside2(seg.end(), bound) || distance2(seg.start(), bound) <= GREY * cx.tol || distance2(seg.end(), bound) <= GREY * cx.tol {
        return Ok(true);
    }
    Ok(bound.iter().any(|c| !seg.intersections(c, GREY * cx.tol).is_empty() || c.distance(seg.start()) <= GREY * cx.tol))
}
