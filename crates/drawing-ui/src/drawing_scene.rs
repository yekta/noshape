//! Converts an assembled drawing sheet into GPU render data: the paper as a
//! white quad, every polyline (dashed on the CPU with true ISO patterns in
//! sheet millimeters) and text (stroke font) as screen-space line segments,
//! and arrowheads as filled triangles. Sheet millimeters are world units on
//! the z = 0 plane, seen by an orthographic top camera.

use noshape_model::drawing::build::BuiltSheet;
use noshape_model::drawing::font;
use noshape_model::drawing::sheet::LineStyle;
use noshape_render::{LineData, LineSegment, MeshData};
use std::collections::HashSet;

// Onshape draws sheet ink pure black.
const INK: [u8; 4] = [0, 0, 0, 255];
/// The paper's outline in the editor (live: about rgb 89 on the 1 px edge).
const PAPER_EDGE: [u8; 4] = [89, 89, 89, 255];
const TANGENT_INK: [u8; 4] = [70, 70, 78, 255];
/// A dangling annotation's ink: live's orange-red #DD5228 (qa4 round 5 s1,
/// drawing/qa4-r5-s1-c-dim-2x).
pub const DANGLING_RGB: u32 = 0xdd5228;
const DANGLING_INK: [u8; 4] = [(DANGLING_RGB >> 16) as u8, (DANGLING_RGB >> 8) as u8, DANGLING_RGB as u8, 255];

/// Pixel width of a line style at a zoom (pixels per mm). Onshape's lines
/// are hairlines at the fitted zoom (1 px for visible edges, frame and
/// dimensions on a 4.3 px/mm A sheet) and thicken as you zoom in.
pub fn px_width(style: LineStyle, zoom: f64) -> f32 {
    pen_px(style, style.width_mm(), zoom)
}

/// Pixel width of a `width_mm` pen on a `style` line at a zoom: screen
/// pixels per pen millimetre per zoom calibrated on visible edges (0.5625
/// for their 0.40 mm pen, as 0.45 was for the 0.5 mm they had); the
/// other styles keep 0.45 until their pens are live's too.
pub fn pen_px(style: LineStyle, width_mm: f64, zoom: f64) -> f32 {
    let k = if matches!(style, LineStyle::Visible | LineStyle::Hidden | LineStyle::Tangent | LineStyle::Phantom) { 0.5625 } else { 0.45 };
    (width_mm * zoom * k).clamp(1.0, 7.0) as f32
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
                on = k.is_multiple_of(2);
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

/// All sheet polylines as line segments, plus texts the filled font
/// can't draw (the others: [`sheet_text`]).
/// `skip`: line ranges drawn separately (shaded views' edges).
pub fn sheet_lines(built: &BuiltSheet, zoom: f64, dangling_picks: &HashSet<u32>, skip: &[std::ops::Range<usize>], paper: bool) -> LineData {
    let g = &built.geometry;
    let mut lines = LineData::default();
    lines.segments.reserve(g.lines.len() * 4);
    // The paper's outline, on screen only (live asb 10-24 "template frames"
    // (a): a thin grey line at the paper edge; the exports have none).
    if paper {
        let (w, h) = (g.width, g.height);
        push(&mut lines, &[[0.0, 0.0], [w, 0.0], [w, h], [0.0, h], [0.0, 0.0]], PAPER_EDGE, 0, 1.0);
    }
    for li in 0..g.lines.len() {
        if !skip.iter().any(|r| r.contains(&li)) {
            push_sheet_line(&mut lines, built, li, zoom, dangling_picks, true);
        }
    }
    // Drawing properties › Dimensions › Underline overridden dimensions.
    if g.style.underline_overridden {
        for &i in &g.overridden {
            let Some(t) = g.texts.get(i) else { continue };
            let b = font::text_box(t);
            let color = g.text_color(i).map(|c| [c[0], c[1], c[2], 255]).unwrap_or(INK);
            push(&mut lines, &[b[0], b[1]], color, t.pick, px_width(LineStyle::Thin, zoom));
        }
    }
    // Texts with symbols the filled font lacks (GD&T) stay stroked, and SHX
    // shape fonts are single strokes.
    for (ti, t) in g.texts.iter().enumerate().filter(|(ti, t)| font::text_is_stroked(t, g.text_font(*ti))) {
        let color = if dangling_picks.contains(&t.pick) { DANGLING_INK } else { g.text_color(ti).map(|c| [c[0], c[1], c[2], 255]).unwrap_or(INK) };
        let width = (t.height * 0.07 * zoom).clamp(1.0, 4.0) as f32;
        for s in font::text_strokes_font(t, g.text_font(ti)) {
            push(&mut lines, &s, color, t.pick, width);
        }
    }
    lines
}

/// The lines `range` of the sheet (with their pick ids, or none).
pub fn range_lines(built: &BuiltSheet, range: std::ops::Range<usize>, zoom: f64, picked: bool) -> LineData {
    let mut lines = LineData::default();
    for li in range {
        push_sheet_line(&mut lines, built, li, zoom, &HashSet::new(), picked);
    }
    lines
}

fn push_sheet_line(lines: &mut LineData, built: &BuiltSheet, li: usize, zoom: f64, dangling_picks: &HashSet<u32>, picked: bool) {
    let g = &built.geometry;
    let Some(l) = g.lines.get(li).filter(|l| l.points.len() >= 2) else { return };
    let color = if dangling_picks.contains(&l.pick) {
        DANGLING_INK
    } else if let Some(c) = g.line_color(li) {
        [c[0], c[1], c[2], 255]
    } else if matches!(l.style, LineStyle::Tangent | LineStyle::Phantom) {
        TANGENT_INK
    } else {
        INK
    };
    // Adjust linestyle pens override the style's width and dashes.
    let (width_mm, dashes) = g.line_pen(li);
    let width = pen_px(l.style, width_mm, zoom);
    for piece in dash(&l.points, l.closed, &dashes) {
        push(lines, &piece, color, if picked { l.pick } else { 0 }, width);
    }
}

/// Sheet texts as filled glyphs (Onshape's sheet text is a filled sans
/// font): (ink, dangling) meshes, picked with the text's pick id.
pub fn sheet_text(built: &BuiltSheet, dangling_picks: &HashSet<u32>) -> (MeshData, MeshData) {
    let (mut ink, mut red) = (MeshData::default(), MeshData::default());
    for (ti, t) in built.geometry.texts.iter().enumerate() {
        // Colored texts go to [`sheet_colored`].
        if t.text.trim().is_empty() || font::text_is_stroked(t, built.geometry.text_font(ti)) || built.geometry.text_color(ti).is_some() {
            continue;
        }
        let (pts, idx) = font::text_fill_font(t, built.geometry.text_font(ti));
        let m = if dangling_picks.contains(&t.pick) { &mut red } else { &mut ink };
        let base = m.positions.len() as u32;
        for p in pts {
            m.positions.push([p[0] as f32, p[1] as f32, 0.0]);
            m.normals.push([0.0, 0.0, 1.0]);
            m.pick_ids.push(t.pick);
        }
        m.indices.extend(idx.into_iter().map(|i| base + i));
    }
    (ink, red)
}

/// Arrowheads and other fills as triangles. The renderer shades meshes, so
/// the ink color is chosen (slightly negative) such that a face pointing at
/// the top camera comes out black.
pub fn sheet_fills(built: &BuiltSheet) -> MeshData {
    let mut m = MeshData::default();
    for (fi, f) in built.geometry.fills.iter().enumerate() {
        if f.points.len() < 3 || built.geometry.fill_color(fi).is_some() {
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

/// Onshape's grey behind property fields in title blocks and notes.
const PROPERTY_BACKGROUND: [u8; 3] = [0xe6, 0xe6, 0xe6];

/// Backdrops (colored areas under all the ink), one mesh per color.
pub fn sheet_backdrops(built: &BuiltSheet) -> ColoredMeshes {
    let mut out: ColoredMeshes = Vec::new();
    // Property fields' grey background (screen only) under the rest.
    let fields = built.geometry.field_backdrops.iter().map(|p| (p, &PROPERTY_BACKGROUND));
    for (pts, c) in fields.chain(built.geometry.backdrops.iter().map(|(p, c)| (p, c))) {
        if pts.len() < 3 {
            continue;
        }
        let i = match out.iter().position(|x| x.1 == *c) {
            Some(i) => i,
            None => {
                out.push((MeshData::default(), *c));
                out.len() - 1
            }
        };
        let m = &mut out[i].0;
        let base = m.positions.len() as u32;
        for p in pts {
            m.positions.push([p[0] as f32, p[1] as f32, 0.0]);
            m.normals.push([0.0, 0.0, 1.0]);
            m.pick_ids.push(0);
        }
        for k in 1..pts.len() as u32 - 1 {
            m.indices.extend_from_slice(&[base, base + k, base + k + 1]);
        }
    }
    out
}

/// Colored fills and filled texts, one mesh per color: (fills, texts).
/// Meshes by color.
pub type ColoredMeshes = Vec<(MeshData, [u8; 3])>;

pub fn sheet_colored(built: &BuiltSheet) -> (ColoredMeshes, ColoredMeshes) {
    let g = &built.geometry;
    let mut fills: ColoredMeshes = Vec::new();
    let mut texts: ColoredMeshes = Vec::new();
    let slot = |v: &mut ColoredMeshes, c: [u8; 3]| -> usize {
        match v.iter().position(|x| x.1 == c) {
            Some(i) => i,
            None => {
                v.push((MeshData::default(), c));
                v.len() - 1
            }
        }
    };
    let add = |m: &mut MeshData, pts: &[[f64; 2]], idx: &[u32], pick: u32| {
        let base = m.positions.len() as u32;
        for p in pts {
            m.positions.push([p[0] as f32, p[1] as f32, 0.0]);
            m.normals.push([0.0, 0.0, 1.0]);
            m.pick_ids.push(pick);
        }
        m.indices.extend(idx.iter().map(|i| base + i));
    };
    for (fi, f) in g.fills.iter().enumerate() {
        let Some(c) = g.fill_color(fi) else { continue };
        if f.points.len() < 3 {
            continue;
        }
        let idx: Vec<u32> = (1..f.points.len() as u32 - 1).flat_map(|i| [0, i, i + 1]).collect();
        let i = slot(&mut fills, c);
        add(&mut fills[i].0, &f.points, &idx, 0);
    }
    for (ti, t) in g.texts.iter().enumerate() {
        let Some(c) = g.text_color(ti) else { continue };
        if t.text.trim().is_empty() || font::text_is_stroked(t, g.text_font(ti)) {
            continue;
        }
        let (pts, idx) = font::text_fill_font(t, g.text_font(ti));
        let i = slot(&mut texts, c);
        add(&mut texts[i].0, &pts, &idx, t.pick);
    }
    (fills, texts)
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

// The tool overlay is GUI-free (shared with the agent API).
pub use noshape_scene::drawing_tools::{Overlay, SnapMark};

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

#[cfg(test)]
mod dangling_tests {
    #[::core::prelude::v1::test]
    fn dangling_annotations_are_live_orange_red() {
        assert_eq!(super::DANGLING_INK, [0xdd, 0x52, 0x28, 255]);
    }
}
