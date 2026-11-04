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
        self.sketch.is_none() && self.picking_dialog().is_some_and(|d| d.read(cx).takes_inference())
    }

    /// Follows the hovered entity and the pointer; true when what is drawn changed.
    pub(super) fn update_inference(&mut self, shift: bool, cx: &mut Context<Self>) -> bool {
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
                self.ui.inference_task = Some(cx.spawn(async move |this, cx| {
                    let points = task.await;
                    let _ = this.update(cx, |this: &mut Self, cx| {
                        let Some(i) = this.ui.inference.as_mut().filter(|i| i.context == address) else { return };
                        i.points = Some(points);
                        if this.choose_inference(cx) {
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

    /// The point nearest the pointer on screen.
    fn choose_inference(&mut self, cx: &App) -> bool {
        let Some(pointer) = self.ui.pointer else { return false };
        let vp = self.viewport.read(cx);
        let Some(i) = self.ui.inference.as_mut() else { return false };
        let Some(points) = &i.points else { return false };
        let choice = points
            .iter()
            .enumerate()
            .filter_map(|(k, p)| vp.project(p.frame.origin).map(|s| (k, s.distance(pointer))))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(k, _)| k);
        if choice == i.choice && i.marks.is_some() {
            return false;
        }
        i.choice = choice;
        i.marks = Some(scene_builder::InferenceMarks::new(points.iter().map(|p| (p.frame.origin, p.frame.x(), p.frame.z())), choice));
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
