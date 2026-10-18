//! "Show dimensions" (feature, face and sketch context menus): a sketch's
//! dimensions drawn in 3D on its plane outside the sketch editor, with the
//! parts see-through. Hovering a dimension highlights it; double-clicking
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

/// Label (world anchor, text, constraint id) and line segments (world).
type DimsGeometry = (Vec<(DVec3, String, u32)>, Vec<[DVec3; 2]>);

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

    fn shown_dims_geometry(&self, cx: &App) -> Option<DimsGeometry> {
        let shown = self.ui.shown_dims?;
        let f = self.studio(cx)?.feature(shown.sketch)?;
        let Some(noshape_model::ParamValue::Sketch(data)) = f.params.get("sketch") else { return None };
        let frame: PlaneFrame = self.display.sketches.iter().find(|s| s.feature == shown.sketch)?.frame;
        let vp = self.viewport.read(cx);
        let mut ed = SketchEditor::new(shown.sketch, frame);
        ed.world_per_px = vp.camera.world_per_pixel(vp.size().y.max(1.0));
        let units = self.doc.read(cx).session.doc.settings.units;
        let w = |p: DVec2| frame.to_world(p);
        let labels = ed.dimension_labels(data, &units).into_iter().map(|(cid, p, text, _)| (w(p), text, cid.0)).collect();
        let segments = ed.dimension_graphics(data).into_iter().flat_map(|g| g.segments).map(|[a, b]| [w(a), w(b)]).collect();
        Some((labels, segments))
    }

    /// Dimension and extension lines (thin black, drawn over the parts).
    pub(super) fn shown_dims_overlay(&self, cx: &App) -> Vec<Instance> {
        let Some((_, segments)) = self.shown_dims_geometry(cx) else { return Vec::new() };
        let f = |v: DVec3| -> [f32; 3] { [v.x as f32, v.y as f32, v.z as f32] };
        let mut lines = LineData::default();
        for [a, b] in segments {
            lines.push_polyline(&[f(a), f(b)], [0, 0, 0, 255], 0, 1.0, LineSegment::ON_TOP);
        }
        vec![Instance::new(Drawable::Lines(self.gpu.uploader().upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(30)]
    }

    /// Dimension labels: hover turns them gold on tan; double-click edits the sketch.
    pub(super) fn render_shown_dims(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(shown) = self.ui.shown_dims else { return Vec::new() };
        let Some((labels, _)) = self.shown_dims_geometry(cx) else { return Vec::new() };
        let vp = self.viewport.read(cx);
        let placed: Vec<(DVec2, String, u32)> = labels.into_iter().filter_map(|(p, t, c)| vp.project(p).map(|s| (s, t, c))).collect();
        placed
            .into_iter()
            .map(|(s, text, cid)| {
                let hovered = shown.hover == Some(cid);
                let sketch = shown.sketch;
                div()
                    .id(SharedString::from(format!("shown-dim-{cid}")))
                    .occlude()
                    .absolute()
                    .left(px(s.x as f32))
                    .top(px(s.y as f32))
                    .ml(px(-30.))
                    .mt(px(-10.))
                    .min_w(px(60.))
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .px(px(2.))
                            .text_size(px(14.))
                            .text_color(if hovered { gpui_kit::rgb(0xb08a2a) } else { gpui_kit::rgb(0x000000) })
                            .when(hovered, |d| d.bg(gpui_kit::rgb(0xe9d9b4)))
                            .child(text),
                    )
                    .cursor_pointer()
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        if let Some(sd) = &mut this.ui.shown_dims {
                            let new = if *h { Some(cid) } else { sd.hover.filter(|c| *c != cid) };
                            if sd.hover != new {
                                sd.hover = new;
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

use gpui_kit::prelude::FluentBuilder as _;
