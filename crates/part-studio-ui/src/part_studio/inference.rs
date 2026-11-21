//! Mate connector inference while hovering (live behaviours 1327, 33725, psui/hole-mc-1851/4): with a field that
//! takes mate connectors active (a Mate connector's Origin entity, a field's mate connector button on), the hovered
//! face or edge shows its inference points as white dots and the one nearest the pointer as a connector preview; a
//! click picks that point. Shift keeps the entity's points while the pointer moves on. The points and the marks are
//! the assembly's (`inference_candidates`, `InferenceMarks`).

use super::PartStudioView;
use crate::scene_builder::{self, PickKind};
use gpui_kit::*;
use noshape_model::assembly::eval::{InferencePoint, inference_candidates};
use noshape_model::partstudio::ConnectorOrigin;
use noshape_model::reference::Reference;
use std::sync::Arc;

/// How near (px) the pointer brings a hovered face's connector to one of its points, in x and in y (a square).
const FACE_REACH_PX: f64 = 47.5;
/// How near (px) the pointer brings a hovered edge's connector to one of its ends.
const EDGE_END_REACH_PX: f64 = 15.0;

/// The hovered entity's inference points (computed off the UI thread) and the one under the pointer.
pub(super) struct InferenceHover {
    on: Reference,
    context: usize,
    points: Option<Vec<InferencePoint>>,
    choice: Option<usize>,
    marks: Option<Arc<scene_builder::InferenceMarks>>,
    /// Shift held: the points stay while the pointer leaves the entity.
    locked: bool,
}

impl PartStudioView {
    /// Whether the active dialog field takes inference points.
    pub(super) fn inference_applies(&self, cx: &App) -> bool {
        self.sketch.is_none() && (self.picking_dialog().is_some_and(|d| d.read(cx).takes_inference()) || self.triad_ball_dragged())
    }

    /// Follows the hovered entity and the pointer; true when what is drawn changed.
    pub(super) fn update_inference(&mut self, shift: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let entity = self.ui.hover_pick.as_ref().filter(|t| matches!(t.kind, PickKind::Face | PickKind::Edge | PickKind::SketchRegion)).map(|t| t.reference.clone());
        let (Some(ctx), true) = (self.context.clone(), self.inference_applies(cx)) else {
            return self.ui.inference.take().is_some_and(|i| i.marks.is_some());
        };
        let locked = shift && self.ui.inference.as_ref().is_some_and(|i| i.points.is_some());
        let address = Arc::as_ptr(&ctx) as usize;
        if !locked {
            let Some(on) = entity else {
                return self.ui.inference.take().is_some_and(|i| i.marks.is_some());
            };
            if self.ui.inference.as_ref().is_none_or(|i| i.on != on || i.context != address) {
                let had = self.ui.inference.take().is_some_and(|i| i.marks.is_some());
                self.ui.inference = Some(InferenceHover { on: on.clone(), context: address, points: None, choice: None, marks: None, locked: false });
                let units = self.doc.read(cx).session.doc.settings.units;
                let task = cx.background_spawn(async move { inference_candidates(&ctx, None, &on, &units) });
                self.ui.inference_task = Some(cx.spawn_in(window, async move |this, cx| {
                    let points = task.await;
                    let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                        let Some(i) = this.ui.inference.as_mut().filter(|i| i.context == address) else { return };
                        i.points = Some(points);
                        if this.choose_inference(cx) {
                            // A ball dragged onto the entity snaps as its points come, not at the pointer's next move.
                            this.triad_ball_snap_now(window, cx);
                            this.refresh_scene(false, cx);
                        }
                    });
                }));
                return had;
            }
        }
        if let Some(i) = self.ui.inference.as_mut() {
            i.locked = locked;
        }
        self.choose_inference(cx)
    }

    /// The point the pointer brings the connector to, as the assembly's (live asm-feel3-1118, live ball-1119): over a
    /// face a point within the 47.5 px square around the pointer, else the face's centroid; over an edge an end within
    /// 15 px, else its midpoint (a circle's centre). Shift (locked points) takes the nearest at any distance.
    fn choose_inference(&mut self, cx: &App) -> bool {
        use noshape_model::reference::Inference;
        let Some(pointer) = self.ui.pointer else { return false };
        let vp = self.vp().read(cx);
        let Some(i) = self.ui.inference.as_mut() else { return false };
        let Some(points) = &i.points else { return false };
        let on_screen: Vec<(usize, glam::DVec2)> = points.iter().enumerate().filter_map(|(k, p)| vp.project(p.frame.origin).map(|s| (k, s - pointer))).collect();
        let nearest = |within: &dyn Fn(glam::DVec2) -> bool| on_screen.iter().filter(|(_, d)| within(*d)).min_by(|a, b| a.1.length().total_cmp(&b.1.length())).map(|(k, _)| *k);
        let primary = points.first().is_some_and(|p| match &p.reference {
            Reference::Implicit { inference: Inference::Centroid, on } => matches!(**on, Reference::Face(_)),
            Reference::Implicit { inference: Inference::Midpoint | Inference::CenterRim, on } => matches!(**on, Reference::Edge(_)),
            _ => false,
        });
        let choice = match &i.on {
            _ if i.locked || !primary => nearest(&|_| true),
            Reference::Face(_) => nearest(&|d| d.x.abs().max(d.y.abs()) <= FACE_REACH_PX).or(on_screen.first().map(|_| 0)),
            _ => nearest(&|d| d.length() <= EDGE_END_REACH_PX).or(on_screen.first().map(|_| 0)),
        };
        if choice == i.choice && i.marks.is_some() {
            return false;
        }
        i.choice = choice;
        let corner = |r: &Reference| matches!(r, Reference::Implicit { inference: noshape_model::reference::Inference::Vertex, .. });
        let marks = points.iter().map(|p| scene_builder::InferenceMark { origin: p.frame.origin, x: p.frame.x(), z: p.frame.z(), corner: corner(&p.reference) });
        i.marks = Some(scene_builder::InferenceMarks::new(marks, choice));
        true
    }

    /// The marks to draw (with the camera's other glyphs).
    pub(super) fn inference_marks(&self) -> Option<Arc<scene_builder::InferenceMarks>> {
        self.ui.inference.as_ref()?.marks.clone()
    }

    /// The point under the pointer.
    pub(super) fn inference_choice(&self) -> Option<&InferencePoint> {
        let i = self.ui.inference.as_ref()?;
        i.points.as_ref()?.get(i.choice?)
    }

    /// The chosen point as the field stores it, when a click on `entity` picks it.
    pub(super) fn inferred_origin(&self, entity: &Reference) -> Option<ConnectorOrigin> {
        self.ui.inference.as_ref().filter(|i| i.on == *entity || i.locked)?;
        let p = self.inference_choice()?;
        Some(ConnectorOrigin::inferred(&p.reference, p.align.as_ref()))
    }
}
