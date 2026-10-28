//! The regions a set of segments divides a plane into (a face split by the edges on it).

use crate::Decline;
use glam::DVec2;

/// A bounded region: its outer loop (counter-clockwise), its holes (clockwise), each a list of (segment,
/// reversed), and a point strictly inside it.
pub(crate) struct Region {
    pub outer: Vec<(u32, bool)>,
    pub holes: Vec<Vec<(u32, bool)>>,
    pub inside: DVec2,
    /// A segment with the region on both sides (it ends inside the region).
    pub dangling: bool,
}

/// The regions of the segments `segs` between the points `pts` (no two segments overlap or cross; they
/// meet at their ends only).
pub(crate) fn regions(pts: &[DVec2], segs: &[[u32; 2]]) -> Result<Vec<Region>, Decline> {
    let n_h = segs.len() * 2;
    let from = |h: usize| segs[h / 2][h % 2] as usize;
    let to = |h: usize| segs[h / 2][1 - h % 2] as usize;
    // Half-edges leaving each point, by angle.
    let mut out: Vec<Vec<(f64, usize)>> = vec![Vec::new(); pts.len()];
    for h in 0..n_h {
        let d = pts[to(h)] - pts[from(h)];
        out[from(h)].push((d.y.atan2(d.x), h));
    }
    let mut pos = vec![0usize; n_h];
    for list in &mut out {
        list.sort_by(|a, b| a.0.total_cmp(&b.0));
        for i in 0..list.len() {
            if list.len() > 1 {
                let next = list[(i + 1) % list.len()].0 + if i + 1 == list.len() { std::f64::consts::TAU } else { 0.0 };
                if next - list[i].0 < 1e-12 {
                    return Err(Decline("two edges leave a vertex in one direction"));
                }
            }
            pos[list[i].1] = i;
        }
    }
    // Cycles, each with its region on the left.
    let mut cycle_of = vec![usize::MAX; n_h];
    let mut cycles: Vec<Vec<usize>> = Vec::new();
    for h0 in 0..n_h {
        if cycle_of[h0] != usize::MAX {
            continue;
        }
        let c = cycles.len();
        let mut cyc = Vec::new();
        let mut h = h0;
        loop {
            if cycle_of[h] != usize::MAX {
                if h != h0 {
                    return Err(Decline("a face's edges don't close"));
                }
                break;
            }
            cycle_of[h] = c;
            cyc.push(h);
            let twin = h ^ 1;
            let list = &out[to(h)];
            h = list[(pos[twin] + list.len() - 1) % list.len()].1;
        }
        cycles.push(cyc);
    }
    let area = |c: &[usize]| -> f64 { c.iter().map(|&h| pts[from(h)].perp_dot(pts[to(h)])).sum::<f64>() * 0.5 };
    let areas: Vec<f64> = cycles.iter().map(|c| area(c)).collect();
    // Connected components (a hole is never inside a cycle of its own component).
    let mut comp: Vec<usize> = (0..pts.len()).collect();
    fn find(c: &mut [usize], mut i: usize) -> usize {
        while c[i] != i {
            c[i] = c[c[i]];
            i = c[i];
        }
        i
    }
    for s in segs {
        let (a, b) = (find(&mut comp, s[0] as usize), find(&mut comp, s[1] as usize));
        comp[a] = b;
    }
    let comp_of: Vec<usize> = cycles.iter().map(|c| find(&mut comp, from(c[0]))).collect();
    let loop_of = |c: &[usize]| -> Vec<(u32, bool)> { c.iter().map(|&h| ((h / 2) as u32, h % 2 == 1)).collect() };
    let mut regions: Vec<Region> = Vec::new();
    let mut region_of_cycle = vec![usize::MAX; cycles.len()];
    for (ci, c) in cycles.iter().enumerate() {
        if areas[ci] > 0.0 {
            region_of_cycle[ci] = regions.len();
            regions.push(Region { outer: loop_of(c), holes: Vec::new(), inside: DVec2::ZERO, dangling: false });
        }
    }
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); regions.len()];
    for (ci, c) in cycles.iter().enumerate() {
        if areas[ci] > 0.0 {
            members[region_of_cycle[ci]].push(ci);
            continue;
        }
        // The smallest positive cycle of another component around it.
        let q = pts[from(c[0])];
        let mut best: Option<usize> = None;
        for (cj, d) in cycles.iter().enumerate() {
            if areas[cj] <= 0.0 || comp_of[cj] == comp_of[ci] {
                continue;
            }
            if best.is_some_and(|b| areas[b] <= areas[cj]) {
                continue;
            }
            if inside(q, d.iter().map(|&h| (pts[from(h)], pts[to(h)]))) {
                best = Some(cj);
            }
        }
        if let Some(b) = best {
            let r = region_of_cycle[b];
            regions[r].holes.push(loop_of(c));
            members[r].push(ci);
        }
    }
    for (ri, r) in regions.iter_mut().enumerate() {
        let hs: Vec<usize> = members[ri].iter().flat_map(|&ci| cycles[ci].iter().copied()).collect();
        r.dangling = hs.iter().any(|&h| cycle_of[h ^ 1] == cycle_of[h] || members[ri].contains(&cycle_of[h ^ 1]));
        // A point inside: from the middle of the outer loop's longest edge, halfway to the nearest edge
        // across.
        let outer = &cycles[members[ri][0]];
        let &h = outer
            .iter()
            .max_by(|&&a, &&b| (pts[to(a)] - pts[from(a)]).length_squared().total_cmp(&(pts[to(b)] - pts[from(b)]).length_squared()))
            .expect("a cycle has edges");
        let (a, b) = (pts[from(h)], pts[to(h)]);
        let m = (a + b) * 0.5;
        let dir = (b - a).perp().normalize();
        let mut nearest = f64::INFINITY;
        for &g in &hs {
            if g / 2 == h / 2 {
                continue;
            }
            if let Some(s) = ray_hits(m, dir, pts[from(g)], pts[to(g)]) {
                nearest = nearest.min(s);
            }
        }
        if !nearest.is_finite() || nearest <= 0.0 {
            return Err(Decline("no point inside a face piece"));
        }
        r.inside = m + dir * (nearest * 0.5);
    }
    Ok(regions)
}

/// Where the ray `m + s dir` (s > 0) meets the segment `a b`.
fn ray_hits(m: DVec2, dir: DVec2, a: DVec2, b: DVec2) -> Option<f64> {
    let e = b - a;
    let den = dir.perp_dot(e);
    if den.abs() < 1e-300 {
        return None;
    }
    let w = a - m;
    let s = w.perp_dot(e) / den;
    let u = w.perp_dot(dir) / den;
    (s > 0.0 && (0.0..=1.0).contains(&u)).then_some(s)
}

/// Whether `p` is inside the polygon of `edges` (even-odd).
pub(crate) fn inside(p: DVec2, edges: impl Iterator<Item = (DVec2, DVec2)>) -> bool {
    let mut odd = false;
    for (a, b) in edges {
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if x > p.x {
                odd = !odd;
            }
        }
    }
    odd
}

/// The distance from `p` to the segment `a b`.
pub(crate) fn segment_distance(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let e = b - a;
    let l2 = e.length_squared();
    let t = if l2 > 0.0 { ((p - a).dot(e) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (a + e * t - p).length()
}
