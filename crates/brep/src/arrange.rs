//! The regions a set of curves divides a face's parameter plane into (a face split by the edges on it).

use crate::curve2::{inside, ray_misses_box, SpanBoxes, C2};
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
    /// Those curves (each once).
    pub inner: Vec<u32>,
}

/// The regions of the curves `segs` (each from point `ends[0]` to `ends[1]`; no two overlap or cross, they
/// meet at their ends only).
pub(crate) fn regions(pts: &[DVec2], segs: &[([u32; 2], C2)]) -> Result<Vec<Region>, Decline> {
    let n_h = segs.len() * 2;
    let from = |h: usize| segs[h / 2].0[h % 2] as usize;
    let to = |h: usize| segs[h / 2].0[1 - h % 2] as usize;
    let curve = |h: usize| if h.is_multiple_of(2) { segs[h / 2].1.clone() } else { segs[h / 2].1.reversed() };
    // Half-edges leaving each point, counter-clockwise by direction, then by how much they turn left, then by how
    // far left of the point they start (two rulings 0.16 mm apart ending at one vertex of 0.21 mm, OCCT's merged
    // corner: parallel, the left one further round) (each one's on all threads).
    let mut out: Vec<Vec<(f64, f64, f64, usize)>> = vec![Vec::new(); pts.len()];
    let leaving = crate::boolean::par_map(n_h, &|h| {
        let c = curve(h);
        let d = c.leaving();
        // -π is π (one direction).
        let a = d.y.atan2(d.x);
        (if a < 1e-9 - PI { a + TAU } else { a }, c.curvature(), d.normalize_or_zero().perp_dot(c.start() - pts[from(h)]))
    })?;
    for (h, (a, k, o)) in leaving.into_iter().enumerate() {
        out[from(h)].push((a, k, o, h));
    }
    let mut pos = vec![0usize; n_h];
    for list in &mut out {
        // Directions a rounding apart are one (a curve leaving along a side, tangent to it), ordered by turn. (An
        // arc tangent to a line at a vertex a tolerance off the tangent point leaves it that far over its radius
        // apart in angle (2e-5 for 0.4 um on an 18 mm arc): a micrometre over the curves' radii, up to 1e-3 rad (a
        // knurl section's curvature at its end, 5.5e6 on its curve in the flank's parameters, merged opposite
        // directions).)
        list.sort_by(|a, b| a.0.total_cmp(&b.0));
        for i in 1..list.len() {
            if list[i].0 - list[i - 1].0 < 1e-9 + (1e-6 * (list[i].1.abs() + list[i - 1].1.abs())).min(1e-3) {
                list[i].0 = list[i - 1].0;
            }
        }
        list.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.total_cmp(&b.2)));
        let n = list.len();
        for i in 0..n {
            if n > 1 {
                let j = (i + 1) % n;
                let gap = list[j].0 + if j == 0 { TAU } else { 0.0 } - list[i].0;
                if gap < 1e-9 && (list[j].1 - list[i].1).abs() < 1e-9 && (list[j].2 - list[i].2).abs() < 1e-9 {
                    return Err(Decline("two edges leave a vertex in one direction"));
                }
            }
            pos[list[i].3] = i;
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
            h = list[(pos[h ^ 1] + list.len() - 1) % list.len()].3;
        }
        cycles.push(cyc);
    }
    let curves_of = |c: &[usize]| -> Vec<C2> { c.iter().map(|&h| curve(h)).collect() };
    // (Each curve's area term once, a half-edge's the way it runs, and its length by 16 chords: only to try the
    // longest first below (exact lengths were a gear's end face's pieces' most time).)
    let chord_length = |c: &C2| (0..16).map(|k| (c.at((k + 1) as f64 / 16.0) - c.at(k as f64 / 16.0)).length()).sum::<f64>();
    let terms: Vec<(f64, f64)> = crate::boolean::par_map(segs.len(), &|k| (segs[k].1.area_term(), chord_length(&segs[k].1)))?;
    let area_of = |h: usize| if h.is_multiple_of(2) { terms[h / 2].0 } else { -terms[h / 2].0 };
    let areas: Vec<f64> = cycles.iter().map(|c| c.iter().map(|&h| area_of(h)).sum()).collect();
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
            regions.push(Region { outer: loop_of(c), holes: Vec::new(), inside: DVec2::ZERO, dangling: false, inner: Vec::new() });
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
    let inside_of = |ri: usize| -> (Vec<u32>, Option<DVec2>) {
        let hs: Vec<usize> = members[ri].iter().flat_map(|&ci| cycles[ci].iter().copied()).collect();
        let dangling: Vec<u32> = hs.iter().filter(|&&h| h % 2 == 0 && members[ri].contains(&cycle_of[h ^ 1])).map(|&h| (h / 2) as u32).collect();
        // (Longest first, each length found once.)
        let mut by_length: Vec<(f64, usize)> = cycles[members[ri][0]].iter().map(|&h| (terms[h / 2].1, h)).collect();
        by_length.sort_by(|a, b| b.0.total_cmp(&a.0));
        for &(len, h) in &by_length {
            let c = curve(h);
            for f in [0.5, 0.381_966, 0.618_034] {
                let t = c.at_fraction(f);
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
    for (r, (inner, inside)) in regions.iter_mut().zip(found) {
        r.dangling = !inner.is_empty();
        r.inner = inner;
        let Some(inside) = inside else { return Err(Decline("no point inside a face piece")) };
        r.inside = inside;
    }
    Ok(regions)
}

/// A point inside region `r` (of the curves `segs`) farther from its curves than [`Region::inside`] when a few
/// tries find one (a thin region's first point is near its sides): along rays into it from points of its loops,
/// at fractions of the way across; the first at least `enough` in (a base's face round 433 grip faces' edges searched
/// every curve's rays for the deepest: 0.7 s).
pub(crate) fn deeper(segs: &[([u32; 2], C2)], r: &Region, enough: f64) -> DVec2 {
    let curves: Vec<C2> = r.outer.iter().chain(r.holes.iter().flatten()).map(|&(s, rev)| if rev { segs[s as usize].1.reversed() } else { segs[s as usize].1.clone() }).collect();
    // (The nearest curve's distance: the curve with the nearest box first, then those whose boxes are nearer than
    // the nearest found yet: the same distance, not each curve's (a gear's face, 294 curves round its 42 tooth gaps:
    // 7 s a face). The boxes hold the curves: their poles' (on a cylinder, the poles' angles where they span less
    // than half a turn: a convex wedge).)
    let boxes: Vec<Option<(DVec2, DVec2)>> = curves.iter().map(|c| c.quick_box()).collect();
    let spans: Vec<SpanBoxes> = curves.iter().map(|c| c.span_boxes2()).collect();
    let depth = |q: DVec2| {
        let away: Vec<f64> = boxes.iter().map(|b| b.map_or(0.0, |(lo, hi)| (lo - q).max(q - hi).max(DVec2::ZERO).length())).collect();
        let first = (0..away.len()).min_by(|&a, &b| away[a].total_cmp(&away[b]));
        let mut best = first.map_or(f64::INFINITY, |i| curves[i].distance_by(q, Some(&spans[i])));
        for (i, &a) in away.iter().enumerate() {
            if a < best && Some(i) != first {
                best = best.min(curves[i].distance_by(q, Some(&spans[i])));
            }
        }
        best
    };
    let mut best = (depth(r.inside), r.inside);
    for c in &curves {
        let len = c.length();
        for t in [0.5, 0.25, 0.75, 0.1, 0.9, 0.02, 0.98] {
            let m = c.at(t);
            let dir = c.tangent(t).perp();
            let mut hits = Vec::new();
            for (g, b) in curves.iter().zip(&boxes) {
                if !b.is_some_and(|b| ray_misses_box(b, m, dir)) {
                    g.ray_hits(m, dir, &mut hits);
                }
            }
            let nearest = hits.into_iter().filter(|&s| s > 1e-9 * len).fold(f64::INFINITY, f64::min);
            if !nearest.is_finite() {
                continue;
            }
            for k in [0.5_f64, 0.25, 0.1, 0.03, 0.01] {
                // (No deeper than its way from `m` on its curve or to the curve the ray met.)
                if nearest * k.min(1.0 - k) * (1.0 + 1e-9) <= best.0 {
                    continue;
                }
                let q = m + dir * (nearest * k);
                let d = depth(q);
                if d > best.0 {
                    best = (d, q);
                }
                if best.0 >= enough {
                    return best.1;
                }
            }
        }
    }
    best.1
}
