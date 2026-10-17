//! Evaluation helpers for drawing and picking: tessellation, closest point,
//! distance and bounding boxes of entities.

use crate::entities::EntityKind;
use crate::geom::BBox;
use crate::ids::EntityId;
use crate::sketch::Sketch;
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
        let mut best: Option<(u8, f64, EntityId)> = None;
        for e in self.entities() {
            let Some(bb) = self.entity_bbox(e.id) else { continue };
            if !bb.expanded(radius).contains(p) {
                continue;
            }
            let Some(d) = self.distance_to(e.id, p) else { continue };
            if d > radius {
                continue;
            }
            let pri = if e.kind() == EntityKind::Point { 0 } else { 1 };
            if best.is_none_or(|b| (pri, d) < (b.0, b.1)) {
                best = Some((pri, d, e.id));
            }
        }
        best.map(|b| b.2)
    }
}
