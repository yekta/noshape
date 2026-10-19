//! The hole callout Onshape draws in the graphics area while a Hole dialog
//! is open (live dialogs/hole-30, hole-35): text lying in the plane of the
//! hole's start face, on a shelf at the end of a leader whose arrow touches
//! the first hole's rim ("Ø6.35 THRU", "Ø6.350 H7 +0.015/−0.000 THRU").

use super::PartStudioView;
use glam::DVec3;
use gpui_kit::App;
use noshape_model::drawing::font;
use noshape_model::drawing::format::split_stacked;
use noshape_model::drawing::sheet::{Anchor, Text2};
use noshape_render::{Drawable, Instance, LineData, LineSegment, MeshData};

/// Screen sizes (px) measured on the live capture.
const CAP_PX: f64 = 11.0;
const LEADER_PX: f64 = 150.0;
const MIN_LEADER_PX: f64 = 20.0;
const ARROW_PX: f64 = 10.0;
const GAP_PX: f64 = 3.0;

impl PartStudioView {
    pub(super) fn hole_callout_overlay(&self, cx: &App) -> Vec<Instance> {
        let Some(e) = &self.edit else { return Vec::new() };
        let d = e.dialog.read(cx);
        if d.feature.type_id != "hole" {
            return Vec::new();
        }
        let Some(ctx) = &self.context else { return Vec::new() };
        let units = self.doc.read(cx).session.doc.settings.units;
        let Some((h, lines)) = noshape_model::interaction::hole_callout_3d(&ctx.holes, d.feature.id, &units) else { return Vec::new() };
        // The start face's plane: text runs along the model X axis seen in
        // it (live: along the block's long edge), up = normal × that.
        let n = -h.dir.normalize_or_zero();
        let mut u = DVec3::X - n * n.x;
        if u.length() < 1e-6 {
            u = DVec3::Y - n * n.y;
        }
        let u = u.normalize_or_zero();
        let v = n.cross(u);
        let vp = self.viewport.read(cx);
        let right = vp.camera.right();
        let (Some(p0), Some(p1)) = (vp.project(h.start), vp.project(h.start + right * 1e-3)) else { return Vec::new() };
        let wpp = 1e-3 / (p1 - p0).length().max(1e-9);
        // Leader up and to the right of the hole, the arrow on its rim.
        let w = (u * 0.385 + v * 0.923).normalize_or_zero();
        let r = h.diameter * 0.5;
        let tip = h.start + w * r;
        // Live (part-studio/433–435): the knee and shelf sit at the edge of
        // the part, where the leader leaves it.
        let edge = ctx
            .bodies
            .iter()
            .filter_map(|b| b.shape.bounding_box(false).ok())
            .find(|bb| (0..3).all(|i| h.start[i] >= bb.min[i] - 1e-6 && h.start[i] <= bb.max[i] + 1e-6))
            .map(|bb| {
                (0..3)
                    .filter(|&i| w[i].abs() > 1e-9)
                    .map(|i| ((if w[i] > 0.0 { bb.max[i] } else { bb.min[i] }) - h.start[i]) / w[i])
                    .fold(f64::INFINITY, f64::min)
            })
            .filter(|t| t.is_finite());
        let reach = edge.map(|t| t * 0.97).unwrap_or(r + LEADER_PX * wpp).max(r + MIN_LEADER_PX * wpp);
        let knee = h.start + w * reach;
        let cap = CAP_PX * wpp;
        let lift = n * (0.5 * wpp);
        let at = |o: DVec3, x: f64, y: f64| o + u * x + v * y + lift;
        let f32v = |p: DVec3| [p.x as f32, p.y as f32, p.z as f32];
        let mut mesh = MeshData::default();
        let mut strokes = LineData::default();
        let black = [0u8, 0, 0, 255];
        // A run of text from `x` on the baseline `y`; returns its width.
        let run = |mesh: &mut MeshData, strokes: &mut LineData, text: &str, x: f64, y: f64, height: f64| -> f64 {
            let mut x0 = x;
            // Filled where the sans font has the glyphs, stroked otherwise (↧ ⌴ ⌵).
            let mut chunks: Vec<(bool, String)> = Vec::new();
            for c in text.chars() {
                let fill = font::fill_supported(&c.to_string());
                match chunks.last_mut() {
                    Some((f, s)) if *f == fill => s.push(c),
                    _ => chunks.push((fill, c.to_string())),
                }
            }
            for (fill, s) in chunks {
                let t = Text2 { pos: [x0, y], height, text: s.clone(), angle: 0.0, anchor: Anchor::BottomLeft, pick: 0 };
                if fill {
                    let (pts, idx) = font::text_fill(&t);
                    let base = mesh.positions.len() as u32;
                    for p in &pts {
                        mesh.positions.push(f32v(at(knee, p[0], p[1])));
                        mesh.normals.push(f32v(n));
                        mesh.pick_ids.push(0);
                    }
                    mesh.indices.extend(idx.iter().map(|i| base + i));
                } else {
                    for pl in font::text_strokes(&t) {
                        let pts: Vec<[f32; 3]> = pl.iter().map(|p| f32v(at(knee, p[0], p[1]))).collect();
                        strokes.push_polyline(&pts, black, 0, 1.2, LineSegment::ON_TOP);
                    }
                }
                x0 += font::text_width(&s, height) + height * 0.2;
            }
            x0 - x
        };
        // Lines stack upwards from the shelf, the first line on top.
        let mut width: f64 = 0.0;
        let line_h = cap * 1.6;
        for (i, line) in lines.iter().enumerate() {
            let y = GAP_PX * wpp + line_h * (lines.len() - 1 - i) as f64;
            let x = GAP_PX * wpp;
            let w_line = match split_stacked(line) {
                (a, Some((up, lo)), b) => {
                    let mut x1 = x + run(&mut mesh, &mut strokes, a, x, y, cap);
                    let small = cap * 0.62;
                    let wu = run(&mut mesh, &mut strokes, up, x1, y + small * 0.95, small);
                    let wl = run(&mut mesh, &mut strokes, lo, x1, y - small * 0.25, small);
                    x1 += wu.max(wl) + cap * 0.3;
                    x1 + run(&mut mesh, &mut strokes, b.trim_start(), x1, y, cap) - x
                }
                (a, None, _) => run(&mut mesh, &mut strokes, a, x, y, cap),
            };
            width = width.max(w_line);
        }
        let shelf = knee + u * (width + 2.0 * GAP_PX * wpp);
        let mut lines_data = LineData::default();
        lines_data.push_polyline(&[f32v(tip + lift), f32v(knee + lift), f32v(shelf + lift)], black, 0, 1.0, LineSegment::ON_TOP);
        // Filled arrowhead on the rim, pointing at the hole.
        let side = n.cross(w).normalize_or_zero();
        let back = tip + w * ARROW_PX * wpp;
        let head = [tip + lift, back + side * (ARROW_PX * 0.3 * wpp) + lift, back - side * (ARROW_PX * 0.3 * wpp) + lift];
        let base = mesh.positions.len() as u32;
        for p in head {
            mesh.positions.push(f32v(p));
            mesh.normals.push(f32v(n));
            mesh.pick_ids.push(0);
        }
        mesh.indices.extend([base, base + 1, base + 2]);
        let up = self.gpu.uploader();
        let mut out = vec![Instance::new(Drawable::Lines(up.upload_lines(&lines_data))).per_primitive_color().unpickable().ignore_clip().layer(30)];
        if !strokes.segments.is_empty() {
            out.push(Instance::new(Drawable::Lines(up.upload_lines(&strokes))).per_primitive_color().unpickable().ignore_clip().layer(30));
        }
        if !mesh.indices.is_empty() {
            out.push(Instance::new(Drawable::Mesh(up.upload_mesh(&mesh))).with_color([0.0, 0.0, 0.0, 1.0]).unlit().unpickable().ignore_clip().layer(30));
        }
        out
    }
}
