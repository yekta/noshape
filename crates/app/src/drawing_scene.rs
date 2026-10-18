//! Converts an assembled drawing sheet into GPU render data: the paper as a
//! white quad, every polyline (dashed on the CPU with true ISO patterns in
//! sheet millimeters) and text (stroke font) as screen-space line segments,
//! and arrowheads as filled triangles. Sheet millimeters are world units on
//! the z = 0 plane, seen by an orthographic top camera.

use noshape_model::drawing::build::BuiltSheet;
use noshape_model::drawing::font;
use noshape_model::drawing::sheet::LineStyle;
use noshape_render::{LineData, LineSegment, MeshData, PointData, PointSprite};
use std::collections::HashSet;

const INK: [u8; 4] = [18, 18, 22, 255];
const TANGENT_INK: [u8; 4] = [70, 70, 78, 255];
const DANGLING_INK: [u8; 4] = [220, 40, 40, 255];

/// Pixel width of a line style at a zoom (pixels per mm).
pub fn px_width(style: LineStyle, zoom: f64) -> f32 {
    (style.width_mm() * zoom * 0.75).clamp(1.0, 7.0) as f32
}

/// Split a polyline into dashes following `pattern` (on, off, on, off...).
pub fn dash(points: &[[f64; 2]], closed: bool, pattern: &[f64]) -> Vec<Vec<[f64; 2]>> {
    let mut pts: Vec<[f64; 2]> = points.to_vec();
    if closed && pts.len() > 2 {
        pts.push(pts[0]);
    }
    if pattern.is_empty() {
        return vec![pts];
    }
    let total: f64 = pattern.iter().sum();
    if total <= 0.0 {
        return vec![pts];
    }
    let mut out: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut cur: Vec<[f64; 2]> = Vec::new();
    let mut k = 0usize;
    let mut left = pattern[0];
    let mut on = true;
    for w in pts.windows(2) {
        let (mut a, b) = (w[0], w[1]);
        let mut seg = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        if seg <= 1e-12 {
            continue;
        }
        let dir = [(b[0] - a[0]) / seg, (b[1] - a[1]) / seg];
        while seg > 1e-12 {
            let step = left.min(seg);
            let p = [a[0] + dir[0] * step, a[1] + dir[1] * step];
            if on {
                if cur.is_empty() {
                    cur.push(a);
                }
                cur.push(p);
            }
            a = p;
            seg -= step;
            left -= step;
            if left <= 1e-12 {
                if on && cur.len() >= 2 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
                k = (k + 1) % pattern.len();
                left = pattern[k];
                on = k % 2 == 0;
            }
        }
    }
    if cur.len() >= 2 {
        out.push(cur);
    }
    out
}

fn push(lines: &mut LineData, pts: &[[f64; 2]], color: [u8; 4], pick: u32, width: f32) {
    for w in pts.windows(2) {
        lines.segments.push(LineSegment {
            a: [w[0][0] as f32, w[0][1] as f32, 0.0],
            b: [w[1][0] as f32, w[1][1] as f32, 0.0],
            color,
            pick_id: pick,
            width,
            style: 0,
        });
    }
}

/// All sheet polylines and texts as line segments.
pub fn sheet_lines(built: &BuiltSheet, zoom: f64, dangling_picks: &HashSet<u32>) -> LineData {
    let g = &built.geometry;
    let mut lines = LineData::default();
    lines.segments.reserve(g.lines.len() * 4);
    for l in &g.lines {
        if l.points.len() < 2 {
            continue;
        }
        let color = if dangling_picks.contains(&l.pick) {
            DANGLING_INK
        } else if l.style == LineStyle::Tangent {
            TANGENT_INK
        } else {
            INK
        };
        let width = px_width(l.style, zoom);
        for piece in dash(&l.points, l.closed, l.style.dashes()) {
            push(&mut lines, &piece, color, l.pick, width);
        }
    }
    for t in &g.texts {
        if t.text.trim().is_empty() {
            continue;
        }
        let color = if dangling_picks.contains(&t.pick) { DANGLING_INK } else { INK };
        let width = (t.height * 0.09 * zoom).clamp(1.0, 5.0) as f32;
        for s in font::text_strokes(t) {
            push(&mut lines, &s, color, t.pick, width);
        }
    }
    lines
}

/// Arrowheads and other fills as triangles. The renderer shades meshes, so
/// the ink color is chosen (slightly negative) such that a face pointing at
/// the top camera comes out black.
pub fn sheet_fills(built: &BuiltSheet) -> MeshData {
    let mut m = MeshData::default();
    for f in &built.geometry.fills {
        if f.points.len() < 3 {
            continue;
        }
        let base = m.positions.len() as u32;
        for p in &f.points {
            m.positions.push([p[0] as f32, p[1] as f32, 0.0]);
            m.normals.push([0.0, 0.0, 1.0]);
            m.pick_ids.push(0);
        }
        for i in 1..f.points.len() as u32 - 1 {
            m.indices.extend_from_slice(&[base, base + i, base + i + 1]);
        }
    }
    m
}

/// Linear ink color for meshes (see [`sheet_fills`]).
pub const FILL_COLOR: [f32; 4] = [-0.07, -0.07, -0.07, 1.0];

/// A rectangle as a mesh at depth `z`.
pub fn quad(x0: f64, y0: f64, x1: f64, y1: f64, z: f32) -> MeshData {
    let p = |x: f64, y: f64| [x as f32, y as f32, z];
    MeshData {
        positions: vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        pick_ids: vec![0; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

/// Tool feedback drawn on top: rubber bands, circles and snap markers.
#[derive(Default, Clone)]
pub struct Overlay {
    pub lines: Vec<(Vec<[f64; 2]>, bool)>,
    pub points: Vec<[f64; 2]>,
}

impl Overlay {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.points.is_empty()
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
        let mut p = PointData::default();
        for q in &self.points {
            p.points.push(PointSprite { position: [q[0] as f32, q[1] as f32, 0.5], color: [255, 140, 0, 255], pick_id: 0, size: 9.0, style: 2, _pad: 0 });
        }
        (l, p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashes_follow_pattern() {
        let d = dash(&[[0.0, 0.0], [10.0, 0.0]], false, &[3.0, 1.5]);
        assert_eq!(d.len(), 3);
        assert!((d[0][1][0] - 3.0).abs() < 1e-9);
        assert!((d[1][0][0] - 4.5).abs() < 1e-9);
        assert_eq!(dash(&[[0.0, 0.0], [1.0, 0.0]], false, &[]).len(), 1);
    }
}
