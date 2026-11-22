//! Mate connector inference while hovering (live behaviours 1327, 33725, psui/hole-mc-1851/4): with a field that
//! takes mate connectors active (a Mate connector's Origin entity, a field's mate connector button on), the hovered
//! face or edge shows its inference points as white dots and the one nearest the pointer as a connector preview; a
//! click picks that point. Shift keeps the entity's points while the pointer moves on. The points and the marks are
//! the assembly's (`inference_candidates`, `InferenceMarks`).

use super::PartStudioView;
use crate::scene_builder::{self, PickKind, PickTarget};
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
    /// A vertex reached from a face or an edge: its connector glyph shows ([`noshape_model::assembly::inference::vertex_glyph`]).
    glyph: bool,
}

impl PartStudioView {
    /// Whether the active dialog field takes inference points.
    pub(super) fn inference_applies(&self, cx: &App) -> bool {
        self.sketch.is_none() && (self.picking_dialog().is_some_and(|d| d.read(cx).takes_inference()) || self.triad_ball_dragged())
    }

    /// Follows the hovered entity and the pointer; true when what is drawn changed.
    pub(super) fn update_inference(&mut self, shift: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let entity = self.ui.hover_pick.as_ref().filter(|t| matches!(t.kind, PickKind::Face | PickKind::Edge | PickKind::Vertex | PickKind::SketchRegion)).map(|t| t.reference.clone());
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
                let before = self.ui.inference.take();
                let had = before.as_ref().is_some_and(|i| i.marks.is_some());
                let glyph = noshape_model::assembly::inference::vertex_glyph(&on, before.as_ref().map(|i| (&i.on, i.glyph)));
                self.ui.inference = Some(InferenceHover { on: on.clone(), context: address, points: None, choice: None, marks: None, locked: false, glyph });
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
        // Shift keeps the face lit too, wherever the pointer goes (live mc-infer-1122 shift.png).
        let mut lit = false;
        if locked && let Some(on) = self.ui.inference.as_ref().map(|i| i.on.clone()) && self.ui.hover_pick.as_ref().is_none_or(|t| t.reference != on) {
            let kind = if matches!(on, Reference::Edge(_)) { PickKind::Edge } else { PickKind::Face };
            self.ui.hover_pick = Some(PickTarget { reference: on, kind, anchor: None });
            lit = self.update_hover(cx);
        }
        self.choose_inference(cx) | lit
    }

    /// The point the pointer brings the connector to: the assembly's choice ([`noshape_model::assembly::inference::choose`],
    /// live mc-infer-1122: the same in both). Shift keeps the entity: the same rule on it wherever the pointer is. (Not
    /// the assembly's centroid on entering: the points come after the pointer is on the entity.)
    fn choose_inference(&mut self, cx: &App) -> bool {
        use noshape_model::assembly::inference::{OnScreen, choose};
        let Some(pointer) = self.ui.pointer else { return false };
        let vp = self.vp().read(cx);
        let Some(i) = self.ui.inference.as_mut() else { return false };
        let Some(points) = &i.points else { return false };
        let on_screen = OnScreen { on: &i.on, points: points.iter().map(|p| (&p.reference, vp.project(p.frame.origin))).collect() };
        let choice = choose(&on_screen, pointer, false);
        if choice == i.choice && i.marks.is_some() {
            return false;
        }
        i.choice = choice;
        let marks = points.iter().map(|p| scene_builder::InferenceMark::new(p.frame.origin, p.frame.x(), p.frame.z(), &p.reference));
        // A vertex: its white corner mark; its connector glyph only reached from its face or edge (live mc-infer-1122
        // reach-ps-corner d=6..0, qa FEEL-MC-VERTEX-FROM-FACE).
        let vertex = matches!(i.on, Reference::Vertex(_));
        i.marks = Some(scene_builder::InferenceMarks::new(marks, choice.filter(|_| !vertex || i.glyph)));
        true
    }

    /// The marks to draw (with the camera's other glyphs).
    pub(super) fn inference_marks(&self) -> Option<Arc<scene_builder::InferenceMarks>> {
        self.ui.inference.as_ref()?.marks.clone()
    }

    /// The point under the pointer (none on a vertex: a click or a dragged ball takes the vertex itself).
    pub(super) fn inference_choice(&self) -> Option<&InferencePoint> {
        let i = self.ui.inference.as_ref().filter(|i| !matches!(i.on, Reference::Vertex(_)))?;
        i.points.as_ref()?.get(i.choice?)
    }

    /// Whether the connector glyph shows at the chosen point (a hovered vertex: only reached from its face or edge).
    #[cfg(test)]
    pub(super) fn inference_glyph_shown(&self) -> bool {
        self.ui.inference.as_ref().is_some_and(|i| i.marks.is_some() && i.choice.is_some() && (!matches!(i.on, Reference::Vertex(_)) || i.glyph))
    }

    /// The chosen point as the field stores it, when a click on `entity` picks it.
    pub(super) fn inferred_origin(&self, entity: &Reference) -> Option<ConnectorOrigin> {
        self.ui.inference.as_ref().filter(|i| i.on == *entity || i.locked)?;
        let p = self.inference_choice()?;
        Some(ConnectorOrigin::inferred(&p.reference, p.align.as_ref()))
    }
}
