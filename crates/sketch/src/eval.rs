//! Evaluation helpers for drawing and picking: tessellation, closest point,
//! distance and bounding boxes of entities.

use crate::entities::EntityKind;
use crate::geom::{BBox, Curve2, ScreenMetric};
use crate::ids::EntityId;
use crate::sketch::Sketch;
use crate::solver::{ConstraintState, SolveReport};
use glam::DVec2;

impl Sketch {
    /// Polyline approximation of an entity with chordal deviation `<= tol`
    /// (a point yields a single vertex).
    pub fn tessellate(&self, id: EntityId, tol: f64) -> Vec<DVec2> {
        if let Some(p) = self.point(id) {
            return vec![p];
        }
        self.curve(id).map(|c| c.tessellate(tol)).unwrap_or_default()
    }

    /// Closest point on an entity to `p` and its distance.
    pub fn closest_point(&self, id: EntityId, p: DVec2) -> Option<(DVec2, f64)> {
        if let Some(q) = self.point(id) {
            return Some((q, (q - p).length()));
        }
        let (_, q, d) = self.curve(id)?.closest(p);
        Some((q, d))
    }

    /// Distance from `p` to an entity.
    pub fn distance_to(&self, id: EntityId, p: DVec2) -> Option<f64> {
        self.closest_point(id, p).map(|x| x.1)
    }

    /// Bounding box of an entity.
    pub fn entity_bbox(&self, id: EntityId) -> Option<BBox> {
        if let Some(q) = self.point(id) {
            return Some(BBox { min: q, max: q });
        }
        Some(self.curve(id)?.bbox())
    }

    /// Hover picking: the entity nearest to `p` within `radius`. Points win
    /// over curves when both are within the radius.
    pub fn pick(&self, p: DVec2, radius: f64) -> Option<EntityId> {
        self.pick_impl(p, radius, None, None)
    }

    /// [`Self::pick`] with `radius` measured on screen (`screen`: the view's [`ScreenMetric`]; none, in the sketch).
    pub fn pick_on_screen(&self, p: DVec2, radius: f64, screen: Option<&ScreenMetric>) -> Option<EntityId> {
        self.pick_impl(p, radius, None, screen)
    }

    /// Sketch selection prefers movable points to fully constrained points
    /// when both fall within the pick radius. Curves still follow points.
    pub fn pick_with_report(&self, p: DVec2, radius: f64, report: &SolveReport) -> Option<EntityId> {
        self.pick_impl(p, radius, Some(report), None)
    }

    /// [`Self::pick_with_report`] with `radius` measured on screen.
    pub fn pick_with_report_on_screen(&self, p: DVec2, radius: f64, report: &SolveReport, screen: Option<&ScreenMetric>) -> Option<EntityId> {
        self.pick_impl(p, radius, Some(report), screen)
    }

    /// How far entity `id` is from `p` on screen: points and lines exactly, other curves from their point nearest
    /// `p` in the sketch.
    pub fn screen_distance_to(&self, id: EntityId, p: DVec2, screen: &ScreenMetric) -> Option<f64> {
        if let Some(q) = self.point(id) {
            return Some(screen.len(q - p));
        }
        match self.curve(id)? {
            Curve2::Line { a, b } => Some(screen.len(screen.closest_on_segment(a, b, p) - p)),
            c => Some(screen.len(c.closest(p).1 - p)),
        }
    }

    fn pick_impl(&self, p: DVec2, radius: f64, report: Option<&SolveReport>, screen: Option<&ScreenMetric>) -> Option<EntityId> {
        let mut best: Option<(u8, f64, EntityId)> = None;
        let reach = screen.map_or(radius, |s| s.reach(radius));
        for e in self.entities() {
            let Some(bb) = self.entity_bbox(e.id) else { continue };
            if !bb.expanded(reach).contains(p) {
                continue;
            }
            let d = match screen {
                Some(s) => self.screen_distance_to(e.id, p, s),
                None => self.distance_to(e.id, p),
            };
            let Some(d) = d else { continue };
            if d > radius {
                continue;
            }
            let pri = if e.kind() == EntityKind::Point {
                u8::from(report.is_some_and(|r| r.state(e.id) == Some(ConstraintState::FullyConstrained)))
            } else { 2 };
            if best.is_none_or(|b| (pri, d) < (b.0, b.1)) {
                best = Some((pri, d, e.id));
            }
        }
        best.map(|b| b.2)
    }
}
