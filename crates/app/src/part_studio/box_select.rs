//! What a box selection takes (Onshape `Viewer.performBoxSelection`): it
//! picks the rectangle over and over, hiding what it found each time (back
//! faces picked too), until nothing new turns up, so the box reaches
//! entities behind others as well. A window box first hides whatever
//! crosses its border, so it takes only what lies wholly inside; a crossing
//! box everything drawn in it. Here the same from the display geometry
//! projected to the screen: faces by their triangles, edges and sketch
//! curves by their polylines, vertices and points, sketch regions by
//! their triangles. Default planes are never taken (live "nav2 selection").

use super::PartStudioView;
use glam::{DVec2, DVec3};
use gpui_kit::*;
use noshape_model::reference::Reference;

/// A screen rectangle (viewport-local pixels).
#[derive(Clone, Copy, Debug)]
struct Rect {
    lo: DVec2,
    hi: DVec2,
}

impl Rect {
    fn contains(&self, p: DVec2) -> bool {
        p.x >= self.lo.x && p.x <= self.hi.x && p.y >= self.lo.y && p.y <= self.hi.y
    }

    /// Whether the segment (a, b) passes through the rectangle (Liang–Barsky).
    fn hits_segment(&self, a: DVec2, b: DVec2) -> bool {
        let d = b - a;
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        for (p, q) in [(-d.x, a.x - self.lo.x), (d.x, self.hi.x - a.x), (-d.y, a.y - self.lo.y), (d.y, self.hi.y - a.y)] {
            if p == 0.0 {
                if q < 0.0 {
                    return false;
                }
            } else {
                let t = q / p;
                if p < 0.0 {
                    t0 = t0.max(t);
                } else {
                    t1 = t1.min(t);
                }
                if t0 > t1 {
                    return false;
                }
            }
        }
        true
    }

    /// Whether the triangle overlaps the rectangle.
    fn hits_triangle(&self, t: [DVec2; 3]) -> bool {
        if t.iter().any(|p| self.contains(*p)) || (0..3).any(|i| self.hits_segment(t[i], t[(i + 1) % 3])) {
            return true;
        }
        // The rectangle inside the triangle: its centre is.
        let c = (self.lo + self.hi) * 0.5;
        let side = |a: DVec2, b: DVec2| (b - a).perp_dot(c - a);
        let s = [side(t[0], t[1]), side(t[1], t[2]), side(t[2], t[0])];
        s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0)
    }
}

/// How an entity covers the box: crossing takes it when any of its pieces
/// is drawn in the box, window when all of its points are inside.
struct Shape {
    points: Vec<DVec2>,
    segments: Vec<(DVec2, DVec2)>,
    triangles: Vec<[DVec2; 3]>,
}

impl Shape {
    fn taken(&self, r: &Rect, crossing: bool) -> bool {
        if self.points.is_empty() {
            return false;
        }
        if !crossing {
            return self.points.iter().all(|p| r.contains(*p));
        }
        self.points.iter().any(|p| r.contains(*p)) || self.segments.iter().any(|(a, b)| r.hits_segment(*a, *b)) || self.triangles.iter().any(|t| r.hits_triangle(*t))
    }
}

impl PartStudioView {
    /// Pick ids of what a box from `a` to `b` (viewport-local pixels) takes.
    pub(super) fn box_ids(&self, (a, b): (DVec2, DVec2), crossing: bool, cx: &App) -> Vec<u32> {
        let Some(built) = &self.built else { return Vec::new() };
        let vp = self.viewport.read(cx);
        let rect = Rect { lo: a.min(b), hi: a.max(b) };
        let s = |p: DVec3| vp.project(p);
        let p3 = |a: &[f32; 3]| DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
        // A polyline and a triangle list in world space.
        let shape = |pts: &[DVec3], tris: &[[DVec3; 3]]| -> Option<Shape> {
            let points: Option<Vec<DVec2>> = pts.iter().map(|p| s(*p)).collect();
            let points = points?;
            let segments: Vec<(DVec2, DVec2)> = points.windows(2).map(|w| (w[0], w[1])).collect();
            let triangles: Option<Vec<[DVec2; 3]>> = tris.iter().map(|t| Some([s(t[0])?, s(t[1])?, s(t[2])?])).collect();
            let mut points = points;
            let triangles = triangles?;
            points.extend(triangles.iter().flatten());
            Some(Shape { points, segments, triangles })
        };
        let keep = |r: &Reference, sh: Option<Shape>, out: &mut Vec<Reference>| {
            if sh.is_some_and(|sh| sh.taken(&rect, crossing)) {
                out.push(r.clone());
            }
        };
        let mut out: Vec<Reference> = Vec::new();
        let tris_of = |positions: &[[f32; 3]], indices: &[u32]| -> Vec<[DVec3; 3]> {
            indices.as_chunks::<3>().0.iter().filter_map(|t| Some([p3(positions.get(t[0] as usize)?), p3(positions.get(t[1] as usize)?), p3(positions.get(t[2] as usize)?)])).collect()
        };
        for body in self.display.bodies.iter().filter(|b| !b.hidden) {
            let m = &body.mesh;
            for (fi, r) in body.face_refs.iter().enumerate() {
                let Some(&(first, count)) = m.face_ranges.get(fi) else { continue };
                let Some(idx) = m.indices.get(first as usize..(first + count) as usize) else { continue };
                keep(r, shape(&[], &tris_of(&m.positions, idx)), &mut out);
            }
            for (ei, r) in body.edge_refs.iter().enumerate() {
                let pts: Vec<DVec3> = m.edges.get(ei).map(|e| e.iter().map(p3).collect()).unwrap_or_default();
                keep(r, shape(&pts, &[]), &mut out);
            }
            for (vi, r) in body.vertex_refs.iter().enumerate() {
                let pts: Vec<DVec3> = m.vertices.get(vi).map(p3).into_iter().collect();
                keep(r, shape(&pts, &[]), &mut out);
            }
        }
        for sk in self.display.sketches.iter().filter(|s| !s.hidden) {
            for c in &sk.curves {
                let pts: Vec<DVec3> = c.points.iter().map(p3).collect();
                keep(&Reference::SketchEntity { sketch: sk.feature, entity: c.entity }, shape(&pts, &[]), &mut out);
            }
            for r in super::sketch_points(sk) {
                if let Some(p) = built.picks.ids_for(&r).iter().find_map(|id| built.picks.get(*id)?.anchor) {
                    keep(&r, shape(&[p], &[]), &mut out);
                }
            }
            for g in &sk.regions {
                let r = Reference::SketchRegion { sketch: sk.feature, key: g.key.clone() };
                keep(&r, shape(&[], &tris_of(&g.positions, &g.indices)), &mut out);
            }
        }
        let mut ids: Vec<u32> = out.iter().flat_map(|r| built.picks.ids_for(r).iter().copied()).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect { lo: DVec2::new(x0, y0), hi: DVec2::new(x1, y1) }
    }

    #[::core::prelude::v1::test]
    fn segments_and_triangles_against_the_box() {
        let r = rect(0.0, 0.0, 10.0, 10.0);
        let v = |x: f64, y: f64| DVec2::new(x, y);
        // Through the box without an end inside.
        assert!(r.hits_segment(v(-5.0, 5.0), v(15.0, 5.0)));
        assert!(!r.hits_segment(v(-5.0, 15.0), v(15.0, 15.0)));
        assert!(r.hits_segment(v(-5.0, -5.0), v(15.0, 15.0)));
        // A triangle around the whole box.
        assert!(r.hits_triangle([v(-50.0, -50.0), v(100.0, -50.0), v(-50.0, 100.0)]));
        assert!(!r.hits_triangle([v(20.0, 20.0), v(30.0, 20.0), v(20.0, 30.0)]));
    }

    #[::core::prelude::v1::test]
    fn window_takes_only_what_is_wholly_inside() {
        let r = rect(0.0, 0.0, 10.0, 10.0);
        let v = |x: f64, y: f64| DVec2::new(x, y);
        let inside = Shape { points: vec![v(1.0, 1.0), v(9.0, 9.0)], segments: vec![(v(1.0, 1.0), v(9.0, 9.0))], triangles: Vec::new() };
        let across = Shape { points: vec![v(5.0, 5.0), v(15.0, 5.0)], segments: vec![(v(5.0, 5.0), v(15.0, 5.0))], triangles: Vec::new() };
        let over = Shape { points: vec![v(-5.0, 5.0), v(15.0, 5.0)], segments: vec![(v(-5.0, 5.0), v(15.0, 5.0))], triangles: Vec::new() };
        assert!(inside.taken(&r, false) && inside.taken(&r, true));
        assert!(!across.taken(&r, false) && across.taken(&r, true));
        assert!(!over.taken(&r, false) && over.taken(&r, true));
    }
}
