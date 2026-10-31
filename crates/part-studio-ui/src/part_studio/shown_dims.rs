//! "Show dimensions" (feature, face and sketch context menus): a sketch's
//! dimensions drawn in 3D on its plane outside the sketch editor, over the
//! parts. Hovering a dimension highlights it; double-clicking
//! one opens the sketch for editing; clicking elsewhere hides them (live
//! capture, part-studio/40–42: there is no inline value box).

use super::PartStudioView;
use crate::sketch_mode::SketchEditor;
use glam::{DVec2, DVec3};
use gpui_kit::*;
use noshape_model::display::PlaneFrame;
use noshape_model::reference::FeatureId;
use noshape_render::{Drawable, Instance, LineData, LineSegment};

/// Dimensions on display: the sketch and the hovered dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShownDims {
    pub sketch: FeatureId,
    pub hover: Option<u32>,
}

/// A label laid in the sketch plane (live part-studio/40: 16 px text along
/// its dimension line, in the plane's perspective).
pub(super) struct ShownLabel {
    pub cid: u32,
    pub text: String,
    /// Baseline start, reading direction and up, in world space.
    pub origin: DVec3,
    pub x: DVec3,
    pub y: DVec3,
    /// Ascender height and advance width (world).
    pub asc: f64,
    pub width: f64,
}

/// Labels and line segments (world) with their constraint ids.
type DimsGeometry = (Vec<ShownLabel>, Vec<(u32, [DVec3; 2])>);

/// Live's 16 px labels: Noto Sans' ascender is 1.069 em.
const LABEL_ASC_PX: f64 = 16.0 * 1.069;
/// Digits' height over the ascender (cap height 0.714 em).
const DIGIT_H: f64 = 0.714 / 1.069;
/// The dimension line's gap on each side of its label.
const LABEL_GAP_PX: f64 = 4.0;
/// Hovered: the lines gold, the label on a tan box (live part-studio/41:
/// lines (148,128,55), box (170,156,133), the text stays black).
const HOVER_INK: u32 = 0x94803a;
const HOVER_BOX: u32 = 0xaa9c85;

impl PartStudioView {
    /// Show the dimensions of `sketch` (a sketch feature).
    pub(super) fn show_dimensions(&mut self, sketch: FeatureId, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.shown_dims = Some(ShownDims { sketch, hover: None });
        let d = self.display.clone();
        self.apply_display(d, window, cx);
    }

    /// Hide shown dimensions (a click elsewhere, Esc). Returns whether any were shown.
    pub(super) fn hide_dimensions(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.ui.shown_dims.take().is_none() {
            return false;
        }
        let d = self.display.clone();
        self.apply_display(d, window, cx);
        true
    }

    /// The sketch whose dimensions a feature's "Show dimensions" shows: the
    /// sketch itself, or the sketch the feature was built from.
    pub(super) fn dimensions_sketch(&self, id: FeatureId, cx: &App) -> Option<FeatureId> {
        let s = self.studio(cx)?;
        let f = s.feature(id)?;
        if f.type_id == "sketch" {
            return Some(id);
        }
        noshape_model::interaction::source_sketch(&s.features, f)
    }

    pub(super) fn shown_dims_geometry(&self, cx: &App) -> Option<DimsGeometry> {
        let shown = self.ui.shown_dims?;
        let f = self.studio(cx)?.feature(shown.sketch)?;
        let Some(noshape_model::ParamValue::Sketch(data)) = f.params.get("sketch") else { return None };
        let frame: PlaneFrame = self.display.sketches.iter().find(|s| s.feature == shown.sketch)?.frame;
        let vp = self.viewport.read(cx);
        let mut ed = SketchEditor::new(shown.sketch, frame);
        ed.world_per_px = vp.camera.world_per_pixel(vp.size().y.max(1.0));
        let units = self.doc.read(cx).session.doc.settings.units;
        let w = |p: DVec2| frame.to_world(p);
        let v = |d: DVec2| frame.x * d.x + frame.y * d.y;
        let asc = LABEL_ASC_PX * ed.world_per_px;
        let mut graphics = ed.dimension_graphics(data);
        let mut labels = Vec::new();
        for (cid, p, text, _) in ed.dimension_labels(data, &units) {
            let layout = super::plane_text(&text);
            let width = layout.width * asc;
            // Along the dimension line through the label (a radius's
            // leader); reading toward the sketch's +x (+y when vertical).
            let g = graphics.iter_mut().find(|g| g.constraint == cid);
            let line = g.as_ref().and_then(|g| {
                g.segments.iter().enumerate().filter(|(_, [a, b])| (*b - *a).length() > 1e-12).min_by(|(_, s), (_, t)| line_dist(p, s).total_cmp(&line_dist(p, t))).map(|(i, s)| (i, *s))
            });
            let mut d = line.map_or(DVec2::X, |(_, [a, b])| (b - a).normalize());
            if d.x < -1e-9 || (d.x.abs() <= 1e-9 && d.y < 0.0) {
                d = -d;
            }
            // The line breaks around the label (on the label's own line).
            if let (Some(g), Some((i, [a, b]))) = (g, line)
                && line_dist(p, &[a, b]) < 0.5 * asc
            {
                let dir = (b - a).normalize();
                let t = (p - a).dot(dir);
                let half = 0.5 * width + LABEL_GAP_PX * ed.world_per_px;
                let len = (b - a).length();
                g.segments.remove(i);
                if t - half > 0.0 {
                    g.segments.push([a, a + dir * (t - half).min(len)]);
                }
                if t + half < len {
                    g.segments.push([a + dir * (t + half).max(0.0), b]);
                }
            }
            let (x, y) = (v(d), v(DVec2::new(-d.y, d.x)));
            let origin = w(p) - x * (0.5 * width) - y * (0.5 * DIGIT_H * asc);
            labels.push(ShownLabel { cid: cid.0, text, origin, x, y, asc, width });
        }
        let segments = graphics.into_iter().flat_map(|g| g.segments.into_iter().map(move |[a, b]| (g.constraint.0, [w(a), w(b)]))).collect();
        Some((labels, segments))
    }

    /// The labels as glyph outlines in the sketch plane (drawn with the
    /// plane names), the hovered one on a tan box.
    pub(super) fn shown_dims_fills(&self, cx: &App) -> Vec<noshape_render::overlay::WorldFill> {
        let Some(shown) = self.ui.shown_dims else { return Vec::new() };
        let Some((labels, _)) = self.shown_dims_geometry(cx) else { return Vec::new() };
        let rgba = |hex: u32| {
            let c = gpui_kit::rgb(hex);
            [c.r, c.g, c.b, c.a]
        };
        let mut out = Vec::new();
        for l in labels {
            let hovered = shown.hover == Some(l.cid);
            if hovered {
                // Live: about 0.45 of the ascender past the text along the
                // line, 0.3 across.
                let (px, py) = (0.45 * l.asc, 0.3 * l.asc);
                let (lo, hi) = (-py, DIGIT_H * l.asc + py);
                let quad = vec![l.origin + l.x * -px + l.y * lo, l.origin + l.x * (l.width + px) + l.y * lo, l.origin + l.x * (l.width + px) + l.y * hi, l.origin + l.x * -px + l.y * hi];
                out.push(noshape_render::overlay::WorldFill { contours: std::sync::Arc::new(vec![quad]), color: rgba(HOVER_BOX) });
            }
            let layout = super::plane_text(&l.text);
            let contours: Vec<Vec<DVec3>> = layout.polylines.iter().filter(|c| c.len() >= 3).map(|c| c.iter().map(|q| l.origin + l.x * (q.x * l.asc) + l.y * (q.y * l.asc)).collect()).collect();
            out.push(noshape_render::overlay::WorldFill { contours: std::sync::Arc::new(contours), color: rgba(0x000000) });
        }
        out
    }

    /// Dimension and extension lines (thin black, drawn over the parts).
    pub(super) fn shown_dims_overlay(&self, cx: &App) -> Vec<Instance> {
        let Some((_, segments)) = self.shown_dims_geometry(cx) else { return Vec::new() };
        let hover = self.ui.shown_dims.and_then(|s| s.hover);
        let f = |v: DVec3| -> [f32; 3] { [v.x as f32, v.y as f32, v.z as f32] };
        let gold = [(HOVER_INK >> 16) as u8, (HOVER_INK >> 8) as u8, HOVER_INK as u8, 255];
        let mut lines = LineData::default();
        for (cid, [a, b]) in segments {
            // Live: ~1.1 px of ink, as in the sketch (width − 0.4 px with our AA).
            lines.push_polyline(&[f(a), f(b)], if hover == Some(cid) { gold } else { [0, 0, 0, 255] }, 0, 1.5, LineSegment::ON_TOP);
        }
        vec![Instance::new(Drawable::Lines(self.gpu.uploader().upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(30)]
    }

    /// Dimension labels: hover turns them gold on tan; double-click edits the sketch.
    pub(super) fn render_shown_dims(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(shown) = self.ui.shown_dims else { return Vec::new() };
        let Some((labels, _)) = self.shown_dims_geometry(cx) else { return Vec::new() };
        let vp = self.viewport.read(cx);
        // Hit areas over each label's screen box (the text itself is drawn
        // in the plane, `shown_dims_fills`).
        let placed: Vec<(DVec2, DVec2, u32)> = labels
            .into_iter()
            .filter_map(|l| {
                let (lo, hi) = (-0.25 * l.asc, DIGIT_H * l.asc);
                let corners = [l.origin + l.y * lo, l.origin + l.x * l.width + l.y * lo, l.origin + l.x * l.width + l.y * hi, l.origin + l.y * hi];
                let pts: Vec<DVec2> = corners.iter().filter_map(|&c| vp.project(c)).collect();
                (pts.len() == 4).then(|| (pts.iter().fold(DVec2::INFINITY, |m, p| m.min(*p)), pts.iter().fold(DVec2::NEG_INFINITY, |m, p| m.max(*p)), l.cid))
            })
            .collect();
        placed
            .into_iter()
            .map(|(lo, hi, cid)| {
                let sketch = shown.sketch;
                div()
                    .id(SharedString::from(format!("shown-dim-{cid}")))
                    .occlude()
                    .absolute()
                    .left(px(lo.x as f32 - 2.))
                    .top(px(lo.y as f32 - 2.))
                    .w(px((hi.x - lo.x) as f32 + 4.))
                    .h(px((hi.y - lo.y) as f32 + 4.))
                    .cursor_pointer()
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        if let Some(sd) = &mut this.ui.shown_dims {
                            let new = if *h { Some(cid) } else { sd.hover.filter(|c| *c != cid) };
                            if sd.hover != new {
                                sd.hover = new;
                                // Its lines turn gold too (the scene's overlay).
                                this.refresh_scene(false, cx);
                                cx.notify();
                            }
                        }
                    }))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        if ev.click_count() >= 2 {
                            this.hide_dimensions(window, cx);
                            this.edit_feature(sketch, window, cx);
                        }
                    }))
                    .into_any_element()
            })
            .collect()
    }
}

/// Distance from `p` to the line through `s`.
fn line_dist(p: DVec2, s: &[DVec2; 2]) -> f64 {
    let d = (s[1] - s[0]).normalize_or_zero();
    let r = p - s[0];
    (r - d * r.dot(d)).length()
}
