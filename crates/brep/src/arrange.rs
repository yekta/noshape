//! The regions a set of curves divides a face's parameter plane into (a face split by the edges on it).

use crate::curve2::{inside, C2};
use crate::Decline;
use glam::DVec2;
use std::f64::consts::{PI, TAU};

/// A bounded region: its outer loop (counter-clockwise), its holes (clockwise), each a list of (curve,
/// reversed), and a point strictly inside it.
pub(crate) struct Region {
    pub outer: Vec<(u32, bool)>,
    pub holes: Vec<Vec<(u32, bool)>>,
    pub inside: DVec2,
    /// A curve with the region on both sides (it ends inside the region).
    pub dangling: bool,
}

/// The regions of the curves `segs` (each from point `ends[0]` to `ends[1]`; no two overlap or cross, they
/// meet at their ends only).
pub(crate) fn regions(pts: &[DVec2], segs: &[([u32; 2], C2)]) -> Result<Vec<Region>, Decline> {
    let n_h = segs.len() * 2;
    let from = |h: usize| segs[h / 2].0[h % 2] as usize;
    let to = |h: usize| segs[h / 2].0[1 - h % 2] as usize;
    let curve = |h: usize| if h.is_multiple_of(2) { segs[h / 2].1.clone() } else { segs[h / 2].1.reversed() };
    // Half-edges leaving each point, counter-clockwise by direction, then by how much they turn left (each one's
    // on all threads).
    let mut out: Vec<Vec<(f64, f64, usize)>> = vec![Vec::new(); pts.len()];
    let leaving = crate::boolean::par_map(n_h, &|h| {
        let c = curve(h);
        let d = c.tangent(0.0);
        // -π is π (one direction).
        let a = d.y.atan2(d.x);
        (if a < 1e-9 - PI { a + TAU } else { a }, c.curvature())
    })?;
    for (h, (a, k)) in leaving.into_iter().enumerate() {
        out[from(h)].push((a, k, h));
    }
    let mut pos = vec![0usize; n_h];
    for list in &mut out {
        // Directions a rounding apart are one (a curve leaving along a side, tangent to it), ordered by turn.
        list.sort_by(|a, b| a.0.total_cmp(&b.0));
        for i in 1..list.len() {
            if list[i].0 - list[i - 1].0 < 1e-9 {
                list[i].0 = list[i - 1].0;
            }
        }
        list.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let n = list.len();
        for i in 0..n {
            if n > 1 {
                let j = (i + 1) % n;
                let gap = list[j].0 + if j == 0 { TAU } else { 0.0 } - list[i].0;
                if gap < 1e-9 && (list[j].1 - list[i].1).abs() < 1e-9 {
                    return Err(Decline("two edges leave a vertex in one direction"));
                }
            }
            pos[list[i].2] = i;
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
            let list = &out[to(h)];
            h = list[(pos[h ^ 1] + list.len() - 1) % list.len()].2;
        }
        cycles.push(cyc);
    }
    let curves_of = |c: &[usize]| -> Vec<C2> { c.iter().map(|&h| curve(h)).collect() };
    let areas: Vec<f64> = crate::boolean::par_map(cycles.len(), &|i| cycles[i].iter().map(|&h| curve(h).area_term()).sum())?;
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
        let (a, b) = (find(&mut comp, s.0[0] as usize), find(&mut comp, s.0[1] as usize));
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
    let cycle_curves: Vec<Vec<C2>> = crate::boolean::par_map(cycles.len(), &|i| curves_of(&cycles[i]))?;
    // Each cycle's box (its curves' control points'; None where one has none): a point outside isn't inside.
    let cycle_box: Vec<Option<(DVec2, DVec2)>> = cycle_curves
        .iter()
        .map(|cs| cs.iter().try_fold((DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)), |(lo, hi), c| c.quick_box().map(|(a, b)| (lo.min(a), hi.max(b)))))
        .collect();
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); regions.len()];
    for (ci, c) in cycles.iter().enumerate() {
        if areas[ci] > 0.0 {
            members[region_of_cycle[ci]].push(ci);
            continue;
        }
        // The smallest positive cycle of another component around it.
        let q = pts[from(c[0])];
        let mut best: Option<usize> = None;
        for cj in 0..cycles.len() {
            if areas[cj] <= 0.0 || comp_of[cj] == comp_of[ci] || best.is_some_and(|b| areas[b] <= areas[cj]) {
                continue;
            }
            if cycle_box[cj].is_some_and(|(lo, hi)| q.x < lo.x || q.y < lo.y || q.x > hi.x || q.y > hi.y) {
                continue;
            }
            if inside(q, &cycle_curves[cj]) {
                best = Some(cj);
            }
        }
        if let Some(b) = best {
            let r = region_of_cycle[b];
            regions[r].holes.push(loop_of(c));
            members[r].push(ci);
        }
    }
    // Each region's point inside (on all threads): from the middle of the outer loop's longest curve, halfway to
    // the nearest curve across (elsewhere on it, then on the others, when that ray passes exactly between two
    // curves).
    let inside_of = |ri: usize| -> (bool, Option<DVec2>) {
        let hs: Vec<usize> = members[ri].iter().flat_map(|&ci| cycles[ci].iter().copied()).collect();
        let dangling = hs.iter().any(|&h| members[ri].contains(&cycle_of[h ^ 1]));
        // (Longest first, each length found once.)
        let mut by_length: Vec<(f64, usize)> = cycles[members[ri][0]].iter().map(|&h| (curve(h).length(), h)).collect();
        by_length.sort_by(|a, b| b.0.total_cmp(&a.0));
        for &(len, h) in &by_length {
            let c = curve(h);
            for t in [0.5, 0.381_966, 0.618_034] {
                let m = c.at(t);
                let dir = c.tangent(t).perp();
                let mut hits = Vec::new();
                for &g in &hs {
                    curve(g).ray_hits(m, dir, &mut hits);
                }
                let nearest = hits.into_iter().filter(|&s| s > 1e-9 * len).fold(f64::INFINITY, f64::min);
                if nearest.is_finite() {
                    return (dangling, Some(m + dir * (nearest * 0.5)));
                }
            }
        }
        (dangling, None)
    };
    let found = crate::boolean::par_map(regions.len(), &inside_of)?;
    for (r, (dangling, inside)) in regions.iter_mut().zip(found) {
        r.dangling = dangling;
        let Some(inside) = inside else { return Err(Decline("no point inside a face piece")) };
        r.inside = inside;
    }
    Ok(regions)
}

/// A point inside region `r` (of the curves `segs`) farther from its curves than [`Region::inside`] when a few
/// tries find one (a thin region's first point is near its sides): along rays into it from points of its loops,
/// at fractions of the way across.
pub(crate) fn deeper(segs: &[([u32; 2], C2)], r: &Region) -> DVec2 {
    let curves: Vec<C2> = r.outer.iter().chain(r.holes.iter().flatten()).map(|&(s, rev)| if rev { segs[s as usize].1.reversed() } else { segs[s as usize].1.clone() }).collect();
    let depth = |q: DVec2| curves.iter().map(|c| c.distance(q)).fold(f64::INFINITY, f64::min);
    let mut best = (depth(r.inside), r.inside);
    for c in &curves {
        for t in [0.5, 0.25, 0.75, 0.1, 0.9, 0.02, 0.98] {
            let m = c.at(t);
            let dir = c.tangent(t).perp();
            let mut hits = Vec::new();
            for g in &curves {
                g.ray_hits(m, dir, &mut hits);
            }
            let nearest = hits.into_iter().filter(|&s| s > 1e-9 * c.length()).fold(f64::INFINITY, f64::min);
            if !nearest.is_finite() {
                continue;
            }
            for k in [0.5, 0.25, 0.1, 0.03, 0.01] {
                let q = m + dir * (nearest * k);
                let d = depth(q);
                if d > best.0 {
                    best = (d, q);
                }
            }
        }
    }
    best.1
}
