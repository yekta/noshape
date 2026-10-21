//! Dimensions on drawing sketch geometry (live behaviours "Drawing sketch
//! dimensions drive geometry", SK7): the Dimension tool on a sketch line
//! places a linear dimension, on a circle a diameter; the value drives the
//! geometry (the line's first-drawn end and the circle's centre stay put)
//! and the sign of a typed value is ignored. The dimension is an ordinary
//! drawing dimension in sheet space ([`SHEET_VIEW`]: view metres = sheet
//! millimetres / 1000) linked to its entity and to the sketch constraint that
//! holds the value (so the dimensioned end can't be dragged).

use super::{Annotation, AnnotationKind, DimOrientation, Sheet, SketchDim, SHEET_VIEW};
use noshape_sketch::{ConstraintId, ConstraintKind, EntityId, Geometry};

impl Sheet {
    /// Dimension a sketch line (linear, aligned) or circle / arc (diameter)
    /// with its value text at `at` (sheet mm). Returns the annotation id.
    pub fn add_sketch_dimension(&mut self, entity: u32, at: [f64; 2]) -> Option<u64> {
        let sk = &self.sketch.sketch;
        let e = EntityId(entity);
        let (kind, value, ck) = match sk.entity(e).map(|x| x.geometry.clone())? {
            Geometry::Line { start, end } => {
                let (a, b) = (sk.point(start)?, sk.point(end)?);
                let d = b - a;
                let l = d.length();
                if l < 1e-9 {
                    return None;
                }
                // Offset: the signed distance of `at` from the line (sheet mm).
                let n = glam::DVec2::new(-d.y, d.x) / l;
                let offset = (glam::DVec2::new(at[0], at[1]) - a).dot(n);
                (AnnotationKind::LinearDimension { view: SHEET_VIEW, a: [a.x / 1000.0, a.y / 1000.0], b: [b.x / 1000.0, b.y / 1000.0], offset, orientation: DimOrientation::Aligned, override_text: None }, l, ConstraintKind::Length { curve: e, value: l })
            }
            Geometry::Circle { center, radius } => {
                let c = sk.point(center)?;
                let angle = (at[1] - c.y).atan2(at[0] - c.x);
                (AnnotationKind::RadialDimension { view: SHEET_VIEW, center: [c.x / 1000.0, c.y / 1000.0], radius: radius / 1000.0, diameter: true, angle, dim_line: false }, radius * 2.0, ConstraintKind::Diameter { curve: e, value: radius * 2.0 })
            }
            _ => return None,
        };
        let _ = value;
        let constraint = self.sketch.sketch.add_constraint(ck).ok()?.0;
        self.sketch_dim_constraints.push(constraint);
        let id = self.next_annotation_id();
        self.annotations.push(Annotation { id, kind, position: at, sketch_dim: Some(SketchDim { entity, constraint }), ..Default::default() });
        Some(id)
    }

    /// The sketch value of a sketch dimension (sheet mm).
    pub fn sketch_dimension_value(&self, id: u64) -> Option<f64> {
        let link = self.annotations.iter().find(|a| a.id == id)?.sketch_dim?;
        self.sketch.sketch.constraint(ConstraintId(link.constraint))?.value()
    }

    /// Type a value (sheet mm) into a sketch dimension: the geometry follows,
    /// the line's start point and the circle's centre fixed; the sign is
    /// ignored (live: "-1.5" gives 1.5 in the same direction).
    pub fn set_sketch_dimension(&mut self, id: u64, value: f64) -> bool {
        let Some(link) = self.annotations.iter().find(|a| a.id == id).and_then(|a| a.sketch_dim) else { return false };
        let v = value.abs();
        if v < 1e-9 {
            return false;
        }
        let sk = &mut self.sketch.sketch;
        let e = EntityId(link.entity);
        let anchor = match sk.entity(e).map(|x| x.geometry.clone()) {
            Some(Geometry::Line { start, end }) => {
                // Move the end along the line so the solve starts there.
                if let (Some(a), Some(b)) = (sk.point(start), sk.point(end)) {
                    let d = (b - a).normalize_or_zero();
                    let _ = sk.set_point(end, a + d * v);
                }
                start
            }
            Some(Geometry::Circle { center, .. }) => {
                if let Some(x) = sk.entity_mut(e)
                    && let Geometry::Circle { radius, .. } = &mut x.geometry
                {
                    *radius = v / 2.0;
                }
                center
            }
            _ => return false,
        };
        if sk.set_dimension_value(ConstraintId(link.constraint), v).is_err() {
            return false;
        }
        // Hold the anchor for this solve only.
        let fix = sk.add_constraint(ConstraintKind::Fix { entity: anchor }).ok();
        let _ = sk.solve();
        if let Some(f) = fix {
            let _ = sk.remove_constraint(f);
        }
        self.sync_sketch_dimensions();
        true
    }

    /// Keep sketch dimensions on their geometry (after the sketch changed),
    /// and drop the constraint of a deleted dimension and the dimension of a
    /// deleted entity.
    pub fn sync_sketch_dimensions(&mut self) {
        let sk = &mut self.sketch.sketch;
        // Dimensions whose entity is gone.
        self.annotations.retain(|a| a.sketch_dim.is_none_or(|l| sk.entity(EntityId(l.entity)).is_some()));
        // Constraints whose dimension is gone (only those a sketch dimension made).
        let used: Vec<u32> = self.annotations.iter().filter_map(|a| a.sketch_dim.map(|l| l.constraint)).collect();
        let orphans: Vec<ConstraintId> = sk
            .constraints()
            .filter(|c| matches!(c.kind, ConstraintKind::Length { .. } | ConstraintKind::Diameter { .. }) && !used.contains(&c.id.0) && self.sketch_dim_constraints.contains(&c.id.0))
            .map(|c| c.id)
            .collect();
        for c in orphans {
            let _ = sk.remove_constraint(c);
        }
        self.sketch_dim_constraints = used;
        for a in &mut self.annotations {
            let Some(link) = a.sketch_dim else { continue };
            let e = EntityId(link.entity);
            match (&mut a.kind, sk.entity(e).map(|x| x.geometry.clone())) {
                (AnnotationKind::LinearDimension { a: pa, b: pb, .. }, Some(Geometry::Line { start, end })) => {
                    if let (Some(p), Some(q)) = (sk.point(start), sk.point(end)) {
                        *pa = [p.x / 1000.0, p.y / 1000.0];
                        *pb = [q.x / 1000.0, q.y / 1000.0];
                    }
                }
                (AnnotationKind::RadialDimension { center: c, radius: r, .. }, Some(Geometry::Circle { center, radius })) => {
                    if let Some(p) = sk.point(center) {
                        *c = [p.x / 1000.0, p.y / 1000.0];
                        *r = radius / 1000.0;
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    #[test]
    fn sketch_line_dimension_drives_the_far_end_and_ignores_the_sign() {
        let mut s = Sheet::default();
        let line = s.sketch.sketch.add_line(DVec2::new(10.0, 10.0), DVec2::new(35.0, 10.0)).0;
        let id = s.add_sketch_dimension(line, [20.0, 20.0]).unwrap();
        assert!((s.sketch_dimension_value(id).unwrap() - 25.0).abs() < 1e-9);
        assert!(s.set_sketch_dimension(id, 50.8));
        let (a, b) = s.sketch.sketch.endpoints(EntityId(line)).unwrap();
        let (pa, pb) = (s.sketch.sketch.point(a).unwrap(), s.sketch.sketch.point(b).unwrap());
        assert!((pa - DVec2::new(10.0, 10.0)).length() < 1e-6, "{pa:?}");
        assert!((pb - DVec2::new(60.8, 10.0)).length() < 1e-6, "{pb:?}");
        // A negative value: the absolute value, same direction.
        assert!(s.set_sketch_dimension(id, -38.1));
        let pb = s.sketch.sketch.point(b).unwrap();
        assert!((pb - DVec2::new(48.1, 10.0)).length() < 1e-6, "{pb:?}");
        // The dimension follows the geometry.
        let Some(AnnotationKind::LinearDimension { b: db, .. }) = s.annotations.iter().find(|x| x.id == id).map(|x| x.kind.clone()) else { panic!() };
        assert!((db[0] - 0.0481).abs() < 1e-9);
        // Deleting the dimension frees the line.
        s.annotations.retain(|x| x.id != id);
        s.sync_sketch_dimensions();
        assert!(!s.sketch.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Length { .. })));
    }

    #[test]
    fn sketch_circle_dimension_grows_about_the_centre() {
        let mut s = Sheet::default();
        let c = s.sketch.sketch.add_circle(DVec2::new(50.0, 50.0), 6.6).0;
        let id = s.add_sketch_dimension(c, [70.0, 60.0]).unwrap();
        assert!(s.set_sketch_dimension(id, 19.05));
        let Some(Geometry::Circle { center, radius }) = s.sketch.sketch.entity(EntityId(c)).map(|e| e.geometry.clone()) else { panic!() };
        assert!((radius - 9.525).abs() < 1e-6 && (s.sketch.sketch.point(center).unwrap() - DVec2::new(50.0, 50.0)).length() < 1e-6);
    }
}
