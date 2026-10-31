//! Drawing tool feedback drawn over the sheet (from
//! `crates/app/src/drawing_scene.rs`).

use noshape_render::{LineData, LineSegment, PointData, PointSprite};

/// Tool feedback drawn on top: rubber bands, circles and snap markers.
#[derive(Default, Clone)]
pub struct Overlay {
    pub lines: Vec<(Vec<[f64; 2]>, bool)>,
    pub points: Vec<[f64; 2]>,
    /// Placement inference lines (live request 1662: light magenta
    /// rgb(255,198,255), 2 px, dotted, running across the whole canvas).
    pub infer: Vec<([f64; 2], [f64; 2])>,
    /// Red ✕ marks (centre, half size mm): the pointer on an inference
    /// line (live request 1662: 10 × 10 px, rgb(255,0,0)).
    pub crosses: Vec<([f64; 2], f64)>,
    /// Snap point markers (Onshape: orange square = endpoint, triangle =
    /// midpoint, diamond = quadrant, circle = center): (point, kind, size mm).
    pub marks: Vec<([f64; 2], SnapMark, f64)>,
    /// Polylines with their own color and width (px), e.g. table cell
    /// selections.
    pub tinted: Vec<(Vec<[f64; 2]>, [u8; 4], f32)>,
    /// Points with their own color (e.g. a pending crop profile's points).
    pub dots: Vec<([f64; 2], [u8; 4])>,
}

/// Shape of a snap point marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapMark {
    Square,
    Triangle,
    Diamond,
    Circle,
}

impl SnapMark {
    /// The marker of a snapped reference (endpoints and free points are squares).
    pub fn of(r: Option<&noshape_model::drawing::PointRef>) -> SnapMark {
        use noshape_model::drawing::PointRef as R;
        match r {
            Some(R::Midpoint { .. }) => SnapMark::Triangle,
            Some(R::Quadrant { .. }) => SnapMark::Diamond,
            Some(R::Center { .. }) => SnapMark::Circle,
            _ => SnapMark::Square,
        }
    }
}

impl Overlay {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.points.is_empty() && self.infer.is_empty() && self.crosses.is_empty() && self.marks.is_empty() && self.tinted.is_empty() && self.dots.is_empty()
    }
    pub fn circle(&mut self, c: [f64; 2], r: f64, dashed: bool) {
        let pts = (0..=72).map(|i| {
            let t = i as f64 / 72.0 * std::f64::consts::TAU;
            [c[0] + r * t.cos(), c[1] + r * t.sin()]
        });
        self.lines.push((pts.collect(), dashed));
    }
    pub fn rect(&mut self, a: [f64; 2], b: [f64; 2], dashed: bool) {
        self.lines.push((vec![a, [b[0], a[1]], b, [a[0], b[1]], a], dashed));
    }

    pub fn to_render(&self, color: [u8; 4]) -> (LineData, PointData) {
        let mut l = LineData::default();
        for (pts, dashed) in &self.lines {
            for w in pts.windows(2) {
                l.segments.push(LineSegment {
                    a: [w[0][0] as f32, w[0][1] as f32, 0.5],
                    b: [w[1][0] as f32, w[1][1] as f32, 0.5],
                    color,
                    pick_id: 0,
                    width: 1.5,
                    style: LineSegment::ON_TOP | if *dashed { LineSegment::DASHED } else { 0 },
                });
            }
        }
        for (pts, color, width) in &self.tinted {
            for w in pts.windows(2) {
                l.segments.push(LineSegment { a: [w[0][0] as f32, w[0][1] as f32, 0.5], b: [w[1][0] as f32, w[1][1] as f32, 0.5], color: *color, pick_id: 0, width: *width, style: LineSegment::ON_TOP });
            }
        }
        for (a, b) in &self.infer {
            // Across the canvas: 10 m past both ends.
            let d = [b[0] - a[0], b[1] - a[1]];
            let len = d[0].hypot(d[1]);
            let (a, b) = if len > 1e-9 {
                let k = 1.0e4 / len;
                ([a[0] - d[0] * k, a[1] - d[1] * k], [b[0] + d[0] * k, b[1] + d[1] * k])
            } else {
                (*a, *b)
            };
            l.segments.push(LineSegment { a: [a[0] as f32, a[1] as f32, 0.5], b: [b[0] as f32, b[1] as f32, 0.5], color: [255, 198, 255, 255], pick_id: 0, width: 2.0, style: LineSegment::ON_TOP | LineSegment::DASHED });
        }
        for &(c, h) in &self.crosses {
            for (p, q) in [([c[0] - h, c[1] - h], [c[0] + h, c[1] + h]), ([c[0] - h, c[1] + h], [c[0] + h, c[1] - h])] {
                l.segments.push(LineSegment { a: [p[0] as f32, p[1] as f32, 0.5], b: [q[0] as f32, q[1] as f32, 0.5], color: [255, 0, 0, 255], pick_id: 0, width: 1.5, style: LineSegment::ON_TOP });
            }
        }
        const ORANGE: [u8; 4] = [255, 140, 0, 255];
        for &(c, kind, s) in &self.marks {
            let h = s * 0.5;
            let pts: Vec<[f64; 2]> = match kind {
                SnapMark::Square => vec![[c[0] - h, c[1] - h], [c[0] + h, c[1] - h], [c[0] + h, c[1] + h], [c[0] - h, c[1] + h], [c[0] - h, c[1] - h]],
                SnapMark::Triangle => vec![[c[0] - h, c[1] - h * 0.8], [c[0] + h, c[1] - h * 0.8], [c[0], c[1] + h], [c[0] - h, c[1] - h * 0.8]],
                SnapMark::Diamond => vec![[c[0], c[1] - h], [c[0] + h, c[1]], [c[0], c[1] + h], [c[0] - h, c[1]], [c[0], c[1] - h]],
                SnapMark::Circle => (0..=24).map(|i| {
                    let t = i as f64 / 24.0 * std::f64::consts::TAU;
                    [c[0] + h * t.cos(), c[1] + h * t.sin()]
                }).collect(),
            };
            for w in pts.windows(2) {
                l.segments.push(LineSegment { a: [w[0][0] as f32, w[0][1] as f32, 0.5], b: [w[1][0] as f32, w[1][1] as f32, 0.5], color: ORANGE, pick_id: 0, width: 2.0, style: LineSegment::ON_TOP });
            }
        }
        let mut p = PointData::default();
        for q in &self.points {
            p.points.push(PointSprite { position: [q[0] as f32, q[1] as f32, 0.5], color: [255, 140, 0, 255], pick_id: 0, size: 9.0, style: 2, _pad: 0 });
        }
        for (q, color) in &self.dots {
            p.points.push(PointSprite { position: [q[0] as f32, q[1] as f32, 0.5], color: *color, pick_id: 0, size: 9.0, style: 2, _pad: 0 });
        }
        (l, p)
    }
}
