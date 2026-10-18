//! "Insert DXF or DWG" drawings: the parsed geometry ([`DxfDrawing`], read
//! by [`crate::cad`] for both formats) and its insertion into a sketch.

use crate::geom::Curve2;
use crate::ids::EntityId;
use crate::nurbs::Nurbs;
use crate::sketch::{Result, Sketch};
use glam::DVec2;

/// One drawing entity in file units.
#[derive(Clone, Debug, PartialEq)]
pub enum DxfEntity {
    Point(DVec2),
    Curve(Curve2),
}

/// A parsed DXF drawing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DxfDrawing {
    pub entities: Vec<DxfEntity>,
    /// Meters per drawing unit from `$INSUNITS`, if the file states it.
    pub unit_scale: Option<f64>,
}

/// Parses a DXF file (ASCII; see [`crate::cad::parse_cad`] for binary DXF
/// and DWG): model space with block references expanded.
pub fn parse_dxf(text: &str) -> Result<DxfDrawing> {
    crate::cad::read_dxf(text.as_bytes())
}

impl Sketch {
    /// Adds the drawing's entities (scaled to meters by `scale` and moved by
    /// `offset`). Curve ends that meet share one point. Returns the new
    /// curves and points.
    pub fn insert_drawing(&mut self, drawing: &DxfDrawing, scale: f64, offset: DVec2) -> Vec<EntityId> {
        let map = |p: DVec2| p * scale + offset;
        let mut created = Vec::new();
        for e in &drawing.entities {
            match e {
                DxfEntity::Point(p) => created.push(self.add_point(map(*p))),
                DxfEntity::Curve(c) => {
                    let c2 = match c {
                        Curve2::Line { a, b } => Curve2::Line { a: map(*a), b: map(*b) },
                        Curve2::Circle { center, radius } => Curve2::Circle { center: map(*center), radius: radius * scale },
                        Curve2::Arc { center, radius, start_angle, end_angle } => Curve2::Arc { center: map(*center), radius: radius * scale, start_angle: *start_angle, end_angle: *end_angle },
                        Curve2::Ellipse { center, major, minor } => Curve2::Ellipse { center: map(*center), major: *major * scale, minor: minor * scale },
                        Curve2::EllipticalArc { center, major, minor, start_param, end_param } => Curve2::EllipticalArc { center: map(*center), major: *major * scale, minor: minor * scale, start_param: *start_param, end_param: *end_param },
                        Curve2::Nurbs(n) => Curve2::Nurbs(Nurbs { ctrl: n.ctrl.iter().map(|p| map(*p)).collect(), ..n.clone() }),
                    };
                    if let Ok(r) = self.use_external(&c2)
                        && let Some(&id) = r.created.last()
                    {
                        // Inserted geometry is regular (not fixed) sketch geometry.
                        for e in r.created.iter() {
                            let _ = self.set_external(*e, false);
                        }
                        created.push(id);
                    }
                }
            }
        }
        // Join coincident curve ends.
        let tol = 1e-9 * self.bbox().size().max_element().max(1e-3);
        let ends: Vec<EntityId> = created.iter().filter_map(|c| self.endpoints(*c)).flat_map(|(a, b)| [a, b]).collect();
        let mut merged: Vec<EntityId> = Vec::new();
        for (i, a) in ends.iter().enumerate() {
            if merged.contains(a) || self.entity(*a).is_none() {
                continue;
            }
            for b in ends.iter().skip(i + 1) {
                if merged.contains(b) || a == b || self.entity(*b).is_none() {
                    continue;
                }
                if let (Some(pa), Some(pb)) = (self.point(*a), self.point(*b))
                    && (pa - pb).length() <= tol
                    && self.merge_points(*b, *a).is_ok()
                {
                    merged.push(*b);
                }
            }
        }
        created
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n10\n21\n0\n0\nLINE\n8\n0\n10\n10\n20\n0\n11\n10\n21\n5\n0\nCIRCLE\n8\n0\n10\n5\n20\n5\n40\n2\n0\nARC\n10\n0\n20\n0\n40\n3\n50\n0\n51\n90\n0\nLWPOLYLINE\n90\n3\n70\n1\n10\n20\n20\n0\n10\n30\n20\n0\n42\n1\n10\n30\n20\n10\n0\nENDSEC\n0\nEOF\n";

    #[test]
    fn parses_entities_and_units() {
        let d = parse_dxf(SAMPLE).unwrap();
        assert_eq!(d.unit_scale, Some(0.001));
        // 2 lines, circle, arc, closed polyline of 3 segments (one bulged)
        assert_eq!(d.entities.len(), 7);
        assert!(d.entities.iter().any(|e| matches!(e, DxfEntity::Curve(Curve2::Arc { radius, .. }) if (radius - 3.0).abs() < 1e-12)));
        let mut sk = Sketch::new();
        let ids = sk.insert_drawing(&d, 0.001, DVec2::ZERO);
        assert_eq!(ids.len(), 7);
        // the two lines share their corner point
        let lines: Vec<EntityId> = ids.iter().copied().filter(|e| sk.kind(*e) == Some(crate::entities::EntityKind::Line)).collect();
        let (_, e0) = sk.endpoints(lines[0]).unwrap();
        let (s1, _) = sk.endpoints(lines[1]).unwrap();
        assert_eq!(e0, s1);
        assert!(sk.solve().is_solved());
        assert!(!sk.entity(lines[0]).unwrap().external);
    }

    #[test]
    fn rejects_non_dxf() {
        assert!(parse_dxf("hello").is_err());
    }
}
