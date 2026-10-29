//! Constrained Delaunay triangulation of a planar polygon with holes (a port of the kernel's
//! `cpp/planar_cdt.hpp`): every boundary node is a vertex, no others. Points inserted in z-order (BRIO) into a
//! Delaunay triangulation (walk location, Lawson legalisation), boundary segments recovered by flips (Sloan),
//! the triangles inside kept by flooding from the boundary, a last Lawson pass. `None` whenever the input or the
//! result isn't clean (the caller declines the face).

use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

fn orient(a: Pt, b: Pt, c: Pt) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// `d` clearly inside the circle through the counter-clockwise `a`, `b`, `c`: beyond the rounding of the
/// determinant (Shewchuk's bound on its permanent), so cocircular points (hole rims) don't flip back and forth.
fn in_circle(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let (adx, ady, bdx, bdy, cdx, cdy) = (a.x - d.x, a.y - d.y, b.x - d.x, b.y - d.y, c.x - d.x, c.y - d.y);
    let (ad, bd, cd) = (adx * adx + ady * ady, bdx * bdx + bdy * bdy, cdx * cdx + cdy * cdy);
    let det = adx * (bdy * cd - bd * cdy) - ady * (bdx * cd - bd * cdx) + ad * (bdx * cdy - bdy * cdx);
    let perm = ((bdx * cdy).abs() + (cdx * bdy).abs()) * ad + ((cdx * ady).abs() + (adx * cdy).abs()) * bd + ((adx * bdy).abs() + (bdx * ady).abs()) * cd;
    det > 1e-14 * perm
}

fn spread(mut v: u32) -> u32 {
    v = (v | (v << 8)) & 0x00FF_00FF;
    v = (v | (v << 4)) & 0x0F0F_0F0F;
    v = (v | (v << 2)) & 0x3333_3333;
    (v | (v << 1)) & 0x5555_5555
}

fn next(e: usize) -> usize {
    if e % 3 == 2 { e - 2 } else { e + 1 }
}

fn prev(e: usize) -> usize {
    if e.is_multiple_of(3) { e + 2 } else { e - 1 }
}

const NONE: usize = usize::MAX;

/// Triangles t: vertices `v[3t..3t+2]` counter-clockwise; half-edge e runs from `v[e]` to `v[next(e)]`; `tw[e]`
/// its twin (`NONE` on the outside).
struct Mesh {
    p: Vec<Pt>,
    v: Vec<usize>,
    tw: Vec<usize>,
    /// A half-edge leaving each vertex.
    vedge: Vec<usize>,
    /// Per half-edge: a boundary segment.
    fixed: Vec<bool>,
    /// Diagonals made by segment recovery.
    recovered: Vec<(usize, usize)>,
}

impl Mesh {
    fn add_tri(&mut self, a: usize, b: usize, c: usize) -> usize {
        let t = self.v.len();
        self.v.extend([a, b, c]);
        self.tw.extend([NONE; 3]);
        self.fixed.extend([false; 3]);
        self.vedge[a] = t;
        self.vedge[b] = t + 1;
        self.vedge[c] = t + 2;
        t
    }

    fn link(&mut self, a: usize, b: usize) {
        if a != NONE {
            self.tw[a] = b;
        }
        if b != NONE {
            self.tw[b] = a;
        }
    }

    /// Flip the edge of half-edge `e` (both triangles kept, reused). Returns the new diagonal's half-edge in
    /// `e`'s triangle (from d to c).
    fn flip(&mut self, e: usize) -> usize {
        let o = self.tw[e];
        let (t, u) = (e - e % 3, o - o % 3);
        let (a, b, c, d) = (self.v[e], self.v[next(e)], self.v[prev(e)], self.v[prev(o)]);
        let (bc, ca, ad, db) = (self.tw[next(e)], self.tw[prev(e)], self.tw[next(o)], self.tw[prev(o)]);
        let (fbc, fca, fad, fdb) = (self.fixed[next(e)], self.fixed[prev(e)], self.fixed[next(o)], self.fixed[prev(o)]);
        // t = (a, d, c), u = (d, b, c).
        self.v[t] = a;
        self.v[t + 1] = d;
        self.v[t + 2] = c;
        self.v[u] = d;
        self.v[u + 1] = b;
        self.v[u + 2] = c;
        self.link(t, ad);
        self.fixed[t] = fad;
        self.link(t + 1, u + 2);
        self.fixed[t + 1] = false;
        self.fixed[u + 2] = false;
        self.link(t + 2, ca);
        self.fixed[t + 2] = fca;
        self.link(u, db);
        self.fixed[u] = fdb;
        self.link(u + 1, bc);
        self.fixed[u + 1] = fbc;
        self.vedge[a] = t;
        self.vedge[d] = t + 1;
        self.vedge[c] = t + 2;
        self.vedge[b] = u + 1;
        t + 1
    }

    /// Lawson: legalise the half-edges on the stack (an edge is fine when fixed, on the outside or locally
    /// Delaunay).
    fn legalize(&mut self, st: &mut Vec<usize>) {
        while let Some(e) = st.pop() {
            let o = self.tw[e];
            if o == NONE || self.fixed[e] {
                continue;
            }
            let (a, b, c, d) = (self.v[e], self.v[next(e)], self.v[prev(e)], self.v[prev(o)]);
            let p = &self.p;
            if !in_circle(p[a], p[b], p[c], p[d]) {
                continue;
            }
            if orient(p[a], p[d], p[c]) <= 0.0 || orient(p[d], p[b], p[c]) <= 0.0 {
                continue;
            }
            let ne = self.flip(e);
            let (t, u) = (ne - 1, self.tw[ne] - 2);
            st.extend([t, t + 2, u, u + 1]);
        }
    }

    /// Inserts vertex `i`; false when it can't be placed (a duplicate, a lost walk).
    fn insert(&mut self, i: usize, start: &mut usize) -> bool {
        let q = self.p[i];
        let mut t = *start - *start % 3;
        // Visibility walk, each step's edges tried from a varying one (a stochastic walk: it ends even where
        // rounding makes the plain walk circle); past its bound, a scan.
        let mut found = false;
        for steps in 0..=self.v.len() {
            let mut out = NONE;
            let k0 = (((steps + i) as u32).wrapping_mul(2_654_435_761) >> 30) as usize % 3;
            for j in 0..3 {
                let k = (k0 + j) % 3;
                if orient(self.p[self.v[t + k]], self.p[self.v[t + (k + 1) % 3]], q) < 0.0 {
                    out = t + k;
                    break;
                }
            }
            if out == NONE {
                found = true;
                break;
            }
            if self.tw[out] == NONE {
                return false;
            }
            t = self.tw[out] - self.tw[out] % 3;
        }
        if !found {
            let p = &self.p;
            let v = &self.v;
            match (0..v.len()).step_by(3).find(|&t| orient(p[v[t]], p[v[t + 1]], q) >= 0.0 && orient(p[v[t + 1]], p[v[t + 2]], q) >= 0.0 && orient(p[v[t + 2]], p[v[t]], q) >= 0.0) {
                Some(f) => t = f,
                None => return false,
            }
        }
        if (0..3).any(|k| self.p[self.v[t + k]] == q) {
            return false;
        }
        let mut st = Vec::new();
        // On an edge: split the two triangles into four.
        let mut on = NONE;
        for k in 0..3 {
            if orient(self.p[self.v[t + k]], self.p[self.v[t + (k + 1) % 3]], q) == 0.0 {
                on = t + k;
            }
        }
        if on != NONE {
            let o = self.tw[on];
            if o == NONE {
                return false;
            }
            let (a, b, c, d) = (self.v[on], self.v[next(on)], self.v[prev(on)], self.v[prev(o)]);
            let (bc, ca, ad, db) = (self.tw[next(on)], self.tw[prev(on)], self.tw[next(o)], self.tw[prev(o)]);
            let (t0, t1) = (on - on % 3, o - o % 3);
            // (a, i, c), (i, b, c) | (b, i, d), (i, a, d)
            self.v[t0] = a;
            self.v[t0 + 1] = i;
            self.v[t0 + 2] = c;
            self.v[t1] = b;
            self.v[t1 + 1] = i;
            self.v[t1 + 2] = d;
            let t2 = self.add_tri(i, b, c);
            let t3 = self.add_tri(i, a, d);
            self.link(t0 + 2, ca);
            self.link(t2 + 1, bc);
            self.link(t1 + 2, db);
            self.link(t3 + 1, ad);
            self.link(t0, t3);
            self.link(t0 + 1, t2 + 2);
            self.link(t2, t1);
            self.link(t1 + 1, t3 + 2);
            self.vedge[a] = t0;
            self.vedge[i] = t0 + 1;
            self.vedge[c] = t0 + 2;
            self.vedge[b] = t1;
            self.vedge[d] = t1 + 2;
            st.extend([t0 + 2, t2 + 1, t1 + 2, t3 + 1]);
            self.legalize(&mut st);
            *start = t0;
            return true;
        }
        let (a, b, c) = (self.v[t], self.v[t + 1], self.v[t + 2]);
        let (ab, bc, ca) = (self.tw[t], self.tw[t + 1], self.tw[t + 2]);
        // (a, b, i), (b, c, i), (c, a, i)
        self.v[t + 2] = i;
        let t1 = self.add_tri(b, c, i);
        let t2 = self.add_tri(c, a, i);
        self.link(t, ab);
        self.link(t1, bc);
        self.link(t2, ca);
        self.link(t + 1, t1 + 2);
        self.link(t1 + 1, t2 + 2);
        self.link(t2 + 1, t + 2);
        self.vedge[a] = t;
        self.vedge[b] = t + 1;
        self.vedge[i] = t + 2;
        st.extend([t, t1, t2]);
        self.legalize(&mut st);
        *start = t;
        true
    }

    /// The half-edge from `a` to `b`, if any.
    fn find_edge(&self, a: usize, b: usize) -> Option<usize> {
        let e0 = self.vedge[a];
        let mut e = e0;
        // Round `a` counter-clockwise: prev(e) arrives at a, its twin leaves a.
        for _ in 0..4096 {
            if self.v[next(e)] == b {
                return Some(e);
            }
            let r = self.tw[prev(e)];
            if r == NONE {
                break;
            }
            e = r;
            if e == e0 {
                return None;
            }
        }
        // Hit the outside going one way: go the other way from e0.
        e = e0;
        for _ in 0..4096 {
            let o = self.tw[e];
            if o == NONE {
                return None;
            }
            e = next(o);
            if self.v[next(e)] == b {
                return Some(e);
            }
            if e == e0 {
                return None;
            }
        }
        None
    }

    fn crosses(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
        let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
        ((o1 > 0.0 && o2 < 0.0) || (o1 < 0.0 && o2 > 0.0)) && ((o3 > 0.0 && o4 < 0.0) || (o3 < 0.0 && o4 > 0.0))
    }

    /// Sloan: make a-b an edge by flipping the edges crossing it.
    fn recover(&mut self, a: usize, b: usize) -> bool {
        if self.find_edge(a, b).is_some() {
            return true;
        }
        let p = |i: usize| self.p[i];
        // The edges crossing a-b, found by walking from a's fan.
        let e0 = self.vedge[a];
        let mut e = e0;
        let mut start = NONE;
        for _ in 0..4096 {
            // Triangle of e: (a, x, y); does a-b leave a through x-y?
            let (x, y) = (self.v[next(e)], self.v[prev(e)]);
            let (ox, oy) = (orient(p(a), p(x), p(b)), orient(p(a), p(y), p(b)));
            if ox > 0.0 && oy < 0.0 {
                start = next(e);
                break;
            }
            // A vertex on a-b: not a clean segment. One on its line behind a is no matter.
            let ahead = |z: usize| (p(z).x - p(a).x) * (p(b).x - p(a).x) + (p(z).y - p(a).y) * (p(b).y - p(a).y) > 0.0;
            if (ox == 0.0 && ahead(x)) || (oy == 0.0 && ahead(y)) {
                return false;
            }
            let r = self.tw[prev(e)];
            if r == NONE {
                return false;
            }
            e = r;
            if e == e0 {
                return false;
            }
        }
        if start == NONE {
            return false;
        }
        // Walk across the triangles to b.
        let mut cross = Vec::new();
        let mut h = start;
        for _ in 0..self.v.len() {
            cross.push(h);
            let o = self.tw[h];
            if o == NONE {
                return false;
            }
            let z = self.v[prev(o)];
            if z == b {
                break;
            }
            let s = orient(p(a), p(b), p(z));
            if s == 0.0 {
                return false;
            }
            // Leave through the edge of o's triangle that a-b still crosses.
            h = if s > 0.0 { next(o) } else { prev(o) };
            if !Self::crosses(p(a), p(b), p(self.v[h]), p(self.v[next(h)])) {
                h = if s > 0.0 { prev(o) } else { next(o) };
            }
        }
        // As vertex pairs: flips rewrite triangles in place, so half-edge indices don't stay with their edges.
        let mut todo: VecDeque<(usize, usize)> = cross.iter().map(|&c| (self.v[c], self.v[next(c)])).collect();
        let limit = 64 * todo.len() + 64;
        let mut guard = 0;
        while let Some((x, y)) = todo.pop_front() {
            guard += 1;
            if guard > limit {
                return false;
            }
            let Some(h) = self.find_edge(x, y) else { return false };
            if self.tw[h] == NONE {
                return false;
            }
            let (c, d) = (self.v[prev(h)], self.v[prev(self.tw[h])]);
            // Convex quad x, d, y, c?
            if orient(self.p[x], self.p[d], self.p[c]) <= 0.0 || orient(self.p[d], self.p[y], self.p[c]) <= 0.0 {
                todo.push_back((x, y));
                continue;
            }
            self.flip(h);
            self.recovered.push((d, c));
            if Self::crosses(self.p[a], self.p[b], self.p[d], self.p[c]) {
                todo.push_back((d, c));
            }
        }
        self.find_edge(a, b).is_some()
    }
}

/// Triangulates the polygon of `rings` (indices into `pts`, each ring once round, either direction; the outer one
/// the largest, the others its holes): triangles counter-clockwise, as point indices. Every point a vertex, no others.
pub fn triangulate(pts: &[Pt], rings: &[Vec<usize>]) -> Option<Vec<[u32; 3]>> {
    if rings.is_empty() {
        return None;
    }
    let mut nv = 0;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut used = vec![false; pts.len()];
    for r in rings {
        if r.len() < 3 {
            return None;
        }
        nv += r.len();
        for &i in r {
            // A node twice: not a simple polygon.
            if i >= pts.len() || used[i] {
                return None;
            }
            used[i] = true;
            min_x = min_x.min(pts[i].x);
            max_x = max_x.max(pts[i].x);
            min_y = min_y.min(pts[i].y);
            max_y = max_y.max(pts[i].y);
        }
    }
    let size = (max_x - min_x).max(max_y - min_y);
    if size.is_nan() || size <= 0.0 || !size.is_finite() {
        return None;
    }
    // Rings: the outer one (the largest: a face's wires come in any order) first and counter-clockwise, holes
    // clockwise (inside on the left).
    let signed_area = |r: &[usize]| {
        let mut s = 0.0;
        let mut j = r.len() - 1;
        for k in 0..r.len() {
            s += pts[r[j]].x * pts[r[k]].y - pts[r[k]].x * pts[r[j]].y;
            j = k;
        }
        s / 2.0
    };
    let mut rr: Vec<Vec<usize>> = rings.to_vec();
    let outer = (0..rr.len()).max_by(|&a, &b| signed_area(&rr[a]).abs().total_cmp(&signed_area(&rr[b]).abs()))?;
    rr.swap(0, outer);
    let mut want = 0.0;
    for (k, r) in rr.iter_mut().enumerate() {
        let a = signed_area(r);
        if a == 0.0 {
            return None;
        }
        if (k == 0) != (a > 0.0) {
            r.reverse();
        }
        want += if k == 0 { a.abs() } else { -a.abs() };
    }
    if want.is_nan() || want <= 0.0 {
        return None;
    }
    // Worked on a grid of 2^22 steps across the points, the enclosing triangle within a few widths of them:
    // every coordinate an integer under 2^24, so `orient`'s products stay under 2^52 and it is exact in
    // doubles. Distinct points that land on one grid point fail the insertion (the face is declined).
    let np = pts.len();
    let grid = 4_194_304.0 / size;
    let mut p: Vec<Pt> = pts.iter().map(|q| Pt { x: ((q.x - min_x) * grid).round(), y: ((q.y - min_y) * grid).round() }).collect();
    let (cx, cy, big) = (((max_x - min_x) * grid / 2.0).round(), ((max_y - min_y) * grid / 2.0).round(), 4_194_304.0);
    p.push(Pt { x: cx - 2.0 * big, y: cy - big });
    p.push(Pt { x: cx + 2.0 * big, y: cy - big });
    p.push(Pt { x: cx, y: cy + 2.0 * big });
    let mut m = Mesh { p, v: Vec::with_capacity(6 * nv + 12), tw: Vec::with_capacity(6 * nv + 12), vedge: vec![NONE; np + 3], fixed: Vec::with_capacity(6 * nv + 12), recovered: Vec::new() };
    m.add_tri(np, np + 1, np + 2);
    // Insertion order: BRIO (a fixed pseudo-random shuffle cut into rounds of doubling size, each round in
    // z-order): few flips per point and short walks.
    let s = 65535.0 / size;
    let mut order: Vec<(u32, usize)> = rr
        .iter()
        .flatten()
        .map(|&i| (spread(((pts[i].x - min_x) * s).clamp(0.0, 65535.0) as u32) | (spread(((pts[i].y - min_y) * s).clamp(0.0, 65535.0) as u32) << 1), i))
        .collect();
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    for k in (2..=order.len()).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        order.swap(k - 1, (seed % k as u64) as usize);
    }
    let (mut lo, mut len) = (0, 64);
    while lo < order.len() {
        let hi = order.len().min(lo + len);
        order[lo..hi].sort();
        lo += len;
        len *= 2;
    }
    let mut start = 0;
    for &(_, i) in &order {
        if !m.insert(i, &mut start) {
            return None;
        }
    }
    // Boundary segments.
    for r in &rr {
        for k in 0..r.len() {
            let (a, b) = (r[k], r[(k + 1) % r.len()]);
            if !m.recover(a, b) {
                return None;
            }
            let e = m.find_edge(a, b)?;
            m.fixed[e] = true;
            if m.tw[e] != NONE {
                let o = m.tw[e];
                m.fixed[o] = true;
            }
        }
    }
    // Inside: flood from the left of every boundary segment, never across one.
    let nt = m.v.len() / 3;
    let mut inside = vec![false; nt];
    let mut st = Vec::new();
    for r in &rr {
        for k in 0..r.len() {
            let e = m.find_edge(r[k], r[(k + 1) % r.len()])?;
            if !inside[e / 3] {
                inside[e / 3] = true;
                st.push(e / 3);
            }
        }
    }
    while let Some(t) = st.pop() {
        for k in 0..3 {
            let (e, o) = (3 * t + k, m.tw[3 * t + k]);
            if o == NONE || m.fixed[e] || inside[o / 3] {
                continue;
            }
            inside[o / 3] = true;
            st.push(o / 3);
        }
    }
    // The outside of every segment must have stayed out.
    for r in &rr {
        for k in 0..r.len() {
            let e = m.find_edge(r[k], r[(k + 1) % r.len()])?;
            let o = m.tw[e];
            if o != NONE && inside[o / 3] {
                return None;
            }
        }
    }
    // Lawson again after the recovery flips (segments fixed): only the recovery's diagonals (and what their
    // flips touch) can be non-Delaunay.
    let mut st: Vec<usize> = m.recovered.clone().into_iter().filter_map(|(x, y)| m.find_edge(x, y)).filter(|&e| !m.fixed[e] && m.tw[e] != NONE).collect();
    m.legalize(&mut st);
    let mut tris = Vec::with_capacity(nv + 2 * rr.len());
    let mut sum = 0.0;
    for (t, _) in inside.iter().enumerate().filter(|(_, i)| **i) {
        let (a, b, c) = (m.v[3 * t], m.v[3 * t + 1], m.v[3 * t + 2]);
        if a >= np || b >= np || c >= np {
            return None;
        }
        let ar = orient(pts[a], pts[b], pts[c]);
        if ar.is_nan() || ar <= 0.0 {
            return None;
        }
        sum += ar / 2.0;
        tris.push([a as u32, b as u32, c as u32]);
    }
    // Every point used: V + 2 H - 2 triangles, covering the polygon.
    if tris.len() != nv + 2 * (rr.len() - 1) - 2 || (sum - want).abs() > 1e-9 * size * size {
        return None;
    }
    Some(tris)
}
