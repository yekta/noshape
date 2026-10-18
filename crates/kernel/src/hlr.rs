//! Hidden line removal (HLR) projection for drawings.

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::ffi::ffi;
use crate::geom::p3;
use crate::shape::{Shape, ck};

/// Visibility of a projected edge segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Visibility {
    Visible,
    Hidden,
}

/// Classification of a projected edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeClass {
    /// Sharp edge between faces meeting at an angle (or free edge).
    Sharp,
    /// Tangent (G1-continuous) edge.
    Smooth,
    /// Silhouette (outline) of a curved surface; has no source edge.
    Outline,
    /// Seam / sewn edge (continuity higher than G1, e.g. cylinder seam).
    Seam,
}

/// One projected polyline in view coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectedEdge {
    pub visibility: Visibility,
    pub class: EdgeClass,
    /// Points in the view plane: x = right, y = up.
    pub polyline: Vec<DVec2>,
    /// Index of the source edge in the projected shape's edges (MapShapes
    /// order), when known (`None` for silhouettes).
    pub source_edge: Option<u32>,
}

/// Project `shape` orthographically along `view_dir` (from the eye into the
/// scene) with `up` as the view's up direction. `exact = true` uses the exact
/// HLRBRep_Algo (slower, exact curves); `false` uses HLRBRep_PolyAlgo on a
/// triangulation (fast). Curves are discretized with deflection `deflection`
/// (model units; `<= 0` uses 0.01).
///
/// The raw HLR fragments are post-processed: hidden pieces lying under
/// visible lines are dropped (e.g. the far edges of a plate seen face-on, or
/// the bottom circle of a through hole) and continuous pieces of the same
/// edge (same visibility, class and source) are merged into polylines, with
/// collinear runs collapsed to single segments.
pub fn project_hlr_with(shape: &Shape, view_dir: DVec3, up: DVec3, exact: bool, deflection: f64) -> Result<Vec<ProjectedEdge>> {
    let raw = project_hlr_raw(shape, view_dir, up, exact, deflection)?;
    let defl = if deflection > 0.0 { deflection } else { 0.01 };
    let size = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    let cover_tol = defl.max(size * 1e-6);
    let join_tol = (size * 1e-7).max(1e-12);
    Ok(merge_fragments(drop_covered_hidden(raw, cover_tol), join_tol))
}

/// Unprocessed HLR fragments (see [`project_hlr_with`]).
pub fn project_hlr_raw(shape: &Shape, view_dir: DVec3, up: DVec3, exact: bool, deflection: f64) -> Result<Vec<ProjectedEdge>> {
    let out = ck(ffi::project_hlr(shape.raw(), p3(view_dir), p3(up), exact, deflection))?;
    Ok(out
        .edges
        .iter()
        .map(|e| {
            let s = e.start as usize;
            let polyline = (s..s + e.count as usize)
                .map(|i| DVec2::new(out.points[2 * i], out.points[2 * i + 1]))
                .collect();
            ProjectedEdge {
                visibility: if e.visible { Visibility::Visible } else { Visibility::Hidden },
                class: match e.kind {
                    1 => EdgeClass::Smooth,
                    2 => EdgeClass::Outline,
                    3 => EdgeClass::Seam,
                    _ => EdgeClass::Sharp,
                },
                polyline,
                source_edge: (e.source >= 0).then_some(e.source as u32),
            }
        })
        .collect())
}

/// [`project_hlr_with`] with a deflection of 1/500 of the shape size.
pub fn project_hlr(shape: &Shape, view_dir: DVec3, up: DVec3, exact: bool) -> Result<Vec<ProjectedEdge>> {
    let d = shape.bounding_box(false).map(|b| b.diagonal() / 500.0).unwrap_or(0.01);
    project_hlr_with(shape, view_dir, up, exact, d.max(1e-6))
}

// ------------------------------------------------------------ post-processing

fn cross2(a: DVec2, b: DVec2) -> f64 {
    a.x * b.y - a.y * b.x
}

/// Uniform grid over segments for neighborhood queries.
struct SegGrid {
    cell: f64,
    origin: DVec2,
    map: std::collections::HashMap<(i64, i64), Vec<u32>>,
}

impl SegGrid {
    fn key(&self, p: DVec2) -> (i64, i64) {
        (((p.x - self.origin.x) / self.cell).floor() as i64, ((p.y - self.origin.y) / self.cell).floor() as i64)
    }
    fn cells(&self, a: DVec2, b: DVec2, pad: f64) -> impl Iterator<Item = (i64, i64)> + use<> {
        let lo = self.key(a.min(b) - DVec2::splat(pad));
        let hi = self.key(a.max(b) + DVec2::splat(pad));
        (lo.0..=hi.0).flat_map(move |i| (lo.1..=hi.1).map(move |j| (i, j)))
    }
}

/// Remove the parts of hidden polylines that lie on visible sharp/outline
/// lines (within `tol`): such lines are drawn visible only.
pub(crate) fn drop_covered_hidden(edges: Vec<ProjectedEdge>, tol: f64) -> Vec<ProjectedEdge> {
    let mut vis: Vec<(DVec2, DVec2)> = Vec::new();
    for e in &edges {
        if e.visibility == Visibility::Visible && matches!(e.class, EdgeClass::Sharp | EdgeClass::Outline) {
            for w in e.polyline.windows(2) {
                if w[0].distance_squared(w[1]) > 0.0 {
                    vis.push((w[0], w[1]));
                }
            }
        }
    }
    if vis.is_empty() || !edges.iter().any(|e| e.visibility == Visibility::Hidden) {
        return edges;
    }
    let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    for (a, b) in &vis {
        lo = lo.min(a.min(*b));
        hi = hi.max(a.max(*b));
    }
    let extent = (hi - lo).max_element().max(tol);
    let mut grid = SegGrid { cell: (extent / 128.0).max(tol * 4.0), origin: lo, map: Default::default() };
    for (i, (a, b)) in vis.iter().enumerate() {
        for k in grid.cells(*a, *b, tol).collect::<Vec<_>>() {
            grid.map.entry(k).or_default().push(i as u32);
        }
    }
    let sin_tol = 0.2; // ~11.5 degrees: chords of coincident curves differ slightly
    let mut out = Vec::with_capacity(edges.len());
    let mut cand: Vec<u32> = Vec::new();
    for e in edges {
        if e.visibility == Visibility::Visible {
            out.push(e);
            continue;
        }
        // Uncovered runs along the polyline.
        let mut runs: Vec<Vec<DVec2>> = Vec::new();
        let mut cur: Vec<DVec2> = Vec::new();
        for w in e.polyline.windows(2) {
            let (a, b) = (w[0], w[1]);
            let d = b - a;
            let l2 = d.length_squared();
            cand.clear();
            for k in grid.cells(a, b, tol) {
                if let Some(v) = grid.map.get(&k) {
                    cand.extend_from_slice(v);
                }
            }
            cand.sort_unstable();
            cand.dedup();
            let mut covered: Vec<(f64, f64)> = Vec::new();
            for &ci in &cand {
                let (c, f) = vis[ci as usize];
                let g = f - c;
                let gl = g.length();
                if gl <= 0.0 {
                    continue;
                }
                let dist = |p: DVec2| {
                    let t = ((p - c).dot(g) / (gl * gl)).clamp(0.0, 1.0);
                    p.distance(c + g * t)
                };
                if l2 <= tol * tol {
                    // Tiny piece: covered when both ends are on the visible segment.
                    if dist(a) <= tol && dist(b) <= tol {
                        covered.push((0.0, 1.0));
                    }
                    continue;
                }
                let dl = l2.sqrt();
                if (cross2(d, g) / (dl * gl)).abs() > sin_tol {
                    continue;
                }
                // Distance from the visible segment's ends to the hidden line
                // and overlap interval along it.
                let off = |p: DVec2| (cross2(d, p - a) / dl).abs();
                if off(c) > tol || off(f) > tol {
                    continue;
                }
                let (t0, t1) = ((c - a).dot(d) / l2, (f - a).dot(d) / l2);
                let (t0, t1) = (t0.min(t1).max(0.0), t0.max(t1).min(1.0));
                if t1 > t0 {
                    covered.push((t0, t1));
                }
            }
            covered.sort_by(|x, y| x.0.total_cmp(&y.0));
            // Complement of the union within [0, 1].
            let eps = if l2 > 0.0 { tol / l2.sqrt() } else { 1.0 };
            let mut free: Vec<(f64, f64)> = Vec::new();
            let mut t = 0.0;
            for (s0, s1) in covered {
                if s0 > t + eps {
                    free.push((t, s0));
                }
                t = t.max(s1);
            }
            if t < 1.0 - eps {
                free.push((t, 1.0));
            } else if let Some(last) = free.last_mut()
                && t >= 1.0 - eps
                && last.1 >= 1.0
            {
                let _ = last;
            }
            for (f0, f1) in free {
                let (p, q) = (a + d * f0, a + d * f1);
                if f0 > 0.0 || cur.is_empty() {
                    if cur.len() >= 2 {
                        runs.push(std::mem::take(&mut cur));
                    }
                    cur.clear();
                    cur.push(p);
                }
                cur.push(q);
                if f1 < 1.0 {
                    runs.push(std::mem::take(&mut cur));
                }
            }
            if !cur.is_empty() && cur.last().is_some_and(|l| l.distance_squared(b) > 0.0) {
                if cur.len() >= 2 {
                    runs.push(std::mem::take(&mut cur));
                }
                cur.clear();
            }
        }
        if cur.len() >= 2 {
            runs.push(cur);
        }
        for r in runs {
            let len: f64 = r.windows(2).map(|w| w[0].distance(w[1])).sum();
            if len > tol {
                out.push(ProjectedEdge { visibility: e.visibility, class: e.class, polyline: r, source_edge: e.source_edge });
            }
        }
    }
    out
}

/// Join fragments with equal (visibility, class, source) whose ends meet
/// (within `tol`) into polylines; collapse collinear interior points.
pub(crate) fn merge_fragments(edges: Vec<ProjectedEdge>, tol: f64) -> Vec<ProjectedEdge> {
    use std::collections::HashMap;
    type Key = (Visibility, EdgeClass, Option<u32>);
    let mut groups: Vec<(Key, Vec<Vec<DVec2>>)> = Vec::new();
    let mut index: HashMap<Key, usize> = HashMap::new();
    for e in edges {
        if e.polyline.len() < 2 {
            continue;
        }
        let k = (e.visibility, e.class, e.source_edge);
        let gi = *index.entry(k).or_insert_with(|| {
            groups.push((k, Vec::new()));
            groups.len() - 1
        });
        groups[gi].1.push(e.polyline);
    }
    let mut out = Vec::new();
    for ((visibility, class, source_edge), polys) in groups {
        let cell = tol.max(1e-15) * 4.0;
        let q = |p: DVec2| ((p.x / cell).round() as i64, (p.y / cell).round() as i64);
        // End point hash: (poly, is_end).
        let mut ends: HashMap<(i64, i64), Vec<(usize, bool)>> = HashMap::new();
        for (i, p) in polys.iter().enumerate() {
            ends.entry(q(p[0])).or_default().push((i, false));
            ends.entry(q(p[p.len() - 1])).or_default().push((i, true));
        }
        let mut used = vec![false; polys.len()];
        let find = |p: DVec2, used: &[bool], ends: &HashMap<(i64, i64), Vec<(usize, bool)>>| -> Option<(usize, bool)> {
            let (ki, kj) = q(p);
            for di in -1..=1 {
                for dj in -1..=1 {
                    if let Some(v) = ends.get(&(ki + di, kj + dj)) {
                        for &(i, is_end) in v {
                            if used[i] {
                                continue;
                            }
                            let e = if is_end { polys[i][polys[i].len() - 1] } else { polys[i][0] };
                            if e.distance(p) <= tol {
                                return Some((i, is_end));
                            }
                        }
                    }
                }
            }
            None
        };
        // Start chains at open ends first so open paths come out whole.
        let mut order: Vec<usize> = (0..polys.len()).collect();
        order.sort_by_key(|&i| {
            let open = find(polys[i][0], &{
                let mut u = vec![false; polys.len()];
                u[i] = true;
                u
            }, &ends)
            .is_none();
            if open { 0 } else { 1 }
        });
        for &start in &order {
            if used[start] {
                continue;
            }
            used[start] = true;
            let mut chain: std::collections::VecDeque<DVec2> = polys[start].iter().copied().collect();
            loop {
                let last = *chain.back().expect("non-empty");
                let Some((j, at_end)) = find(last, &used, &ends) else { break };
                used[j] = true;
                if at_end {
                    chain.extend(polys[j].iter().rev().skip(1));
                } else {
                    chain.extend(polys[j].iter().skip(1));
                }
            }
            loop {
                let first = *chain.front().expect("non-empty");
                let Some((j, at_end)) = find(first, &used, &ends) else { break };
                used[j] = true;
                if at_end {
                    for p in polys[j].iter().rev().skip(1) {
                        chain.push_front(*p);
                    }
                } else {
                    for p in polys[j].iter().skip(1) {
                        chain.push_front(*p);
                    }
                }
            }
            let pts = simplify_collinear(chain.into_iter().collect(), tol);
            if pts.len() >= 2 {
                out.push(ProjectedEdge { visibility, class, polyline: pts, source_edge });
            }
        }
    }
    out
}

/// Drop duplicate points and interior points on straight runs.
fn simplify_collinear(pts: Vec<DVec2>, tol: f64) -> Vec<DVec2> {
    let mut out: Vec<DVec2> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().is_some_and(|l: &DVec2| l.distance(p) <= tol) {
            continue;
        }
        while out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let (ab, ap) = (b - a, p - a);
            let l = ap.length();
            // b lies on segment a-p (strictly collinear, between the ends).
            if l > 0.0 && (cross2(ap, ab) / l).abs() <= tol && ab.dot(ap) > 0.0 && ab.length() < l {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pe(vis: bool, pts: &[[f64; 2]], src: Option<u32>) -> ProjectedEdge {
        ProjectedEdge {
            visibility: if vis { Visibility::Visible } else { Visibility::Hidden },
            class: EdgeClass::Sharp,
            polyline: pts.iter().map(|p| DVec2::new(p[0], p[1])).collect(),
            source_edge: src,
        }
    }

    #[test]
    fn merges_and_simplifies() {
        let e = vec![pe(true, &[[0.0, 0.0], [1.0, 0.0]], Some(1)), pe(true, &[[2.0, 0.0], [1.0, 0.0]], Some(1)), pe(true, &[[2.0, 0.0], [3.0, 0.0]], Some(1)), pe(true, &[[5.0, 0.0], [6.0, 0.0]], Some(1))];
        let m = merge_fragments(e, 1e-9);
        assert_eq!(m.len(), 2);
        let long = m.iter().find(|x| x.polyline.len() == 2 && (x.polyline[0].x - x.polyline[1].x).abs() > 2.5).unwrap();
        assert!((long.polyline[0].x - long.polyline[1].x).abs() - 3.0 < 1e-12);
    }

    #[test]
    fn hidden_under_visible_is_dropped() {
        let e = vec![pe(true, &[[0.0, 0.0], [10.0, 0.0]], Some(1)), pe(false, &[[-5.0, 0.0], [5.0, 0.0]], Some(2)), pe(false, &[[0.0, 1.0], [5.0, 1.0]], Some(3))];
        let r = drop_covered_hidden(e, 1e-6);
        let hidden: Vec<_> = r.iter().filter(|x| x.visibility == Visibility::Hidden).collect();
        assert_eq!(hidden.len(), 2);
        let partial = hidden.iter().find(|x| x.source_edge == Some(2)).unwrap();
        assert!((partial.polyline[0].x + 5.0).abs() < 1e-9 && partial.polyline[1].x.abs() < 1e-9, "{:?}", partial.polyline);
    }
}
