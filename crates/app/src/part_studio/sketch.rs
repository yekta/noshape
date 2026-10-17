//! Sketch mode integration for the Part Studio view: routes viewport input
//! to the [`SketchEditor`], renders the sketch toolbar and dimension labels,
//! and regenerates synchronously so dragging and drawing feel immediate.

use super::PartStudioView;
use crate::actions::*;
use crate::assets::icon_path;
use crate::sketch_mode::{ConstraintTool, EditOutcome, SketchEditor, Tool};
use crate::theme::{p, with_alpha};
use crate::viewport::{Viewport, ViewportEvent};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, Selectable as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use glam::DVec2;
use noshape_model::display::{FeatureStatus, PlaneFrame};
use noshape_model::params::ParamValue;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::RegenOptions;
use noshape_model::sketch_feature::SketchData;
use noshape_render::{Drawable, Instance};
use std::sync::Arc;

const DRAW_TOOLS: &[&[Tool]] = &[
    &[Tool::Line],
    &[Tool::CornerRect, Tool::CenterRect],
    &[Tool::Circle, Tool::ThreePointCircle],
    &[Tool::Arc3, Tool::TangentArc, Tool::CenterArc],
    &[Tool::Spline],
    &[Tool::Point],
    &[Tool::Slot],
    &[Tool::Polygon],
    &[Tool::Ellipse],
];

const EDIT_TOOLS: &[Tool] = &[Tool::Fillet, Tool::Trim, Tool::Extend, Tool::Split, Tool::Offset, Tool::Mirror];

impl PartStudioView {
    /// Id of the sketch feature being edited (if the dialog is a sketch).
    pub(super) fn editing_sketch_id(&self, cx: &App) -> Option<FeatureId> {
        let e = self.edit.as_ref()?;
        let d = e.dialog.read(cx);
        (d.feature.type_id == "sketch").then_some(d.feature.id)
    }

    /// Create/update/drop the sketch editor to match the edit session.
    pub(super) fn sync_sketch_editor(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.editing_sketch_id(cx) else {
            if self.sketch.take().is_some() {
                self.opts.editing_sketch = None;
                self.viewport.update(cx, |vp, _| vp.tool_captures_left = false);
            }
            return;
        };
        let plane = self.edit.as_ref().and_then(|e| e.dialog.read(cx).feature.params.query("sketchPlane").first().cloned());
        let frame = self.display.sketches.iter().find(|s| s.feature == id).map(|s| s.frame).or_else(|| match plane {
            Some(Reference::Plane(DatumPlane::Top)) => Some(PlaneFrame::TOP),
            Some(Reference::Plane(DatumPlane::Front)) => Some(PlaneFrame::FRONT),
            Some(Reference::Plane(DatumPlane::Right)) => Some(PlaneFrame::RIGHT),
            _ => None,
        });
        let Some(frame) = frame else { return };
        match &mut self.sketch {
            Some(ed) if ed.feature == id => ed.frame = frame,
            _ => {
                self.sketch = Some(SketchEditor::new(id, frame));
                // A fresh sketch starts with the line tool, like Onshape.
                let empty = self.edit.as_ref().and_then(|e| e.dialog.read(cx).sketch_data().map(|d| d.sketch.entity_count() == 0)).unwrap_or(false);
                if empty && let Some(ed) = &mut self.sketch {
                    ed.set_tool(Tool::Line);
                }
            }
        }
        self.opts.editing_sketch = Some(id);
        self.viewport.update(cx, |vp, _| vp.tool_captures_left = true);
    }

    pub(super) fn leave_sketch(&mut self, cx: &mut Context<Self>) {
        if self.sketch.take().is_some() {
            self.dim_input = None;
            self.opts.editing_sketch = None;
            self.viewport.update(cx, |vp, _| vp.tool_captures_left = false);
        }
    }

    /// Run `f` with the editor and the draft sketch data.
    fn with_sketch<R>(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut SketchEditor, &mut SketchData) -> R) -> Option<R> {
        let dialog = self.edit.as_ref()?.dialog.clone();
        let mut editor = self.sketch.take()?;
        let r = dialog.update(cx, |d, _| d.sketch_data_mut().map(|data| f(&mut editor, data)));
        self.sketch = Some(editor);
        r
    }

    /// Regenerate immediately on the UI thread (sketch edits are cheap: only
    /// the sketch is evaluated, upstream results come from the cache).
    pub(super) fn regen_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((studio, upto)) = self.effective_studio(cx) else { return };
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let config = studio.configuration.defaults();
        let editing_sketch = self.sketch.as_ref().map(|s| s.feature);
        self.regen_seq += 1;
        let result = {
            let mut regen = self.regen.lock();
            regen.regenerate(&studio, &config, &units, &RegenOptions { upto, editing_sketch, ..Default::default() })
        };
        self.regenerating = false;
        self.regen_task = None;
        self.apply_display(Arc::new(result.display), window, cx);
    }

    fn apply_outcome(&mut self, out: EditOutcome, window: &mut Window, cx: &mut Context<Self>) {
        if out.changed {
            self.regen_sync(window, cx);
        } else if out.redraw {
            self.refresh_scene(false, cx);
        }
        cx.notify();
    }

    fn hovered_sketch_entity(&self) -> Option<noshape_sketch::EntityId> {
        let ed = self.sketch.as_ref()?;
        match &self.hover {
            Some(Reference::SketchEntity { sketch, entity }) if *sketch == ed.feature => Some(noshape_sketch::EntityId(*entity)),
            _ => None,
        }
    }

    /// Handle viewport input in sketch mode. Returns true when consumed.
    pub(super) fn sketch_viewport_event(&mut self, vp: &Entity<Viewport>, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let (wpp, ray_of) = {
            let v = vp.read(cx);
            let wpp = v.camera.world_per_pixel(v.size().y);
            let rays = |pos: DVec2| v.ray(pos);
            let _ = &rays;
            (wpp, ())
        };
        let _ = ray_of;
        let to_plane = |this: &Self, pos: DVec2, cx: &App| -> Option<DVec2> {
            let (o, d) = vp.read(cx).ray(pos);
            this.sketch.as_ref()?.plane_point(o, d)
        };
        if let Some(ed) = &mut self.sketch {
            ed.world_per_px = wpp;
        }
        match ev {
            ViewportEvent::PointerMove { position, modifiers } => {
                let Some(p) = to_plane(self, *position, cx) else { return false };
                let shift = modifiers.shift;
                if let Some(out) = self.with_sketch(cx, |ed, data| ed.pointer_move(data, p, shift)) {
                    self.apply_outcome(out, window, cx);
                }
                // Let hover highlighting run too.
                false
            }
            ViewportEvent::ToolDown { position, modifiers, click_count } => {
                let Some(p) = to_plane(self, *position, cx) else { return true };
                let hovered = self.hovered_sketch_entity();
                let (shift, dbl) = (modifiers.shift, *click_count >= 2);
                if let Some(out) = self.with_sketch(cx, |ed, data| ed.tool_down(data, p, hovered, shift, dbl)) {
                    self.apply_outcome(out, window, cx);
                }
                self.sync_dim_edit(window, cx);
                true
            }
            ViewportEvent::ToolDrag { position, .. } => {
                let Some(p) = to_plane(self, *position, cx) else { return true };
                if let Some(out) = self.with_sketch(cx, |ed, data| ed.tool_drag(data, p)) {
                    self.apply_outcome(out, window, cx);
                }
                true
            }
            ViewportEvent::ToolUp { position, .. } => {
                let Some(p) = to_plane(self, *position, cx) else { return true };
                if let Some(out) = self.with_sketch(cx, |ed, data| ed.tool_up(data, p)) {
                    self.apply_outcome(out, window, cx);
                }
                true
            }
            ViewportEvent::Click { .. } => true,
            _ => false,
        }
    }

    /// Overlay instances and selection/accent refs for the sketch editor.
    pub(super) fn sketch_overlay(&self, selection: &mut Vec<Reference>, accent: &mut Vec<Reference>, cx: &App) -> Vec<Instance> {
        let Some(ed) = &self.sketch else { return Vec::new() };
        let f = ed.feature;
        selection.clear();
        selection.extend(ed.selection.iter().map(|e| Reference::SketchEntity { sketch: f, entity: e.0 }));
        accent.clear();
        accent.extend(ed.picked().iter().map(|e| Reference::SketchEntity { sketch: f, entity: e.0 }));
        let Some(data) = self.edit.as_ref().and_then(|e| e.dialog.read(cx).sketch_data().cloned()) else { return Vec::new() };
        let (lines, points) = ed.overlay(&data);
        let up = self.gpu.uploader();
        let mut out = Vec::new();
        if !lines.segments.is_empty() {
            out.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(30));
        }
        if !points.points.is_empty() {
            out.push(Instance::new(Drawable::Points(up.upload_points(&points))).per_primitive_color().unpickable().ignore_clip().layer(31));
        }
        out
    }

    pub(super) fn sketch_cancel(&mut self, cx: &mut Context<Self>) {
        let cancelled = self.sketch.as_mut().map(|s| s.cancel()).unwrap_or(false);
        if cancelled {
            self.dim_input = None;
            self.refresh_scene(false, cx);
            cx.notify();
        }
    }

    pub(super) fn sketch_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.with_sketch(cx, |ed, data| ed.delete_selection(data)) == Some(true) {
            self.regen_sync(window, cx);
        }
    }

    fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        if let Some(ed) = &mut self.sketch {
            ed.set_tool(tool);
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    fn constraint(&mut self, c: ConstraintTool, window: &mut Window, cx: &mut Context<Self>) {
        if self.with_sketch(cx, |ed, data| ed.constraint_command(data, c)) == Some(true) {
            self.regen_sync(window, cx);
        }
        cx.notify();
    }

    fn toggle_construction(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.with_sketch(cx, |ed, data| ed.toggle_construction(data)) == Some(true) {
            self.regen_sync(window, cx);
        }
        cx.notify();
    }

    /// Accept the sketch and return references to all its regions.
    pub(super) fn finish_sketch(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<Reference> {
        let Some(id) = self.sketch.as_ref().map(|s| s.feature) else { return Vec::new() };
        let regions: Vec<Reference> = self
            .display
            .sketches
            .iter()
            .find(|s| s.feature == id)
            .map(|s| s.regions.iter().map(|r| Reference::SketchRegion { sketch: id, key: r.key.clone() }).collect())
            .unwrap_or_default();
        self.accept_edit(false, window, cx);
        regions
    }

    /// Keep the inline dimension input in sync with the editor's pending edit.
    fn sync_dim_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pending = self.sketch.as_ref().and_then(|s| s.dim_edit.clone());
        match pending {
            Some(edit) => {
                if self.dim_input.as_ref().is_some_and(|(c, _, _)| *c == edit.constraint) {
                    return;
                }
                let state = cx.new(|cx| InputState::new(window, cx).default_value(edit.text.clone()));
                let cid = edit.constraint;
                let sub = cx.subscribe_in(&state, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
                    InputEvent::PressEnter { .. } => {
                        let text = st.read(cx).value().to_string();
                        this.commit_dimension(cid, &text, window, cx);
                    }
                    InputEvent::Blur => {
                        if let Some(ed) = &mut this.sketch {
                            ed.dim_edit = None;
                        }
                        this.dim_input = None;
                        cx.notify();
                    }
                    _ => {}
                });
                state.update(cx, |s, cx| {
                    s.focus(window, cx);
                    s.select_all(window, cx);
                });
                self.dim_input = Some((cid, state, sub));
            }
            None => self.dim_input = None,
        }
    }

    fn commit_dimension(&mut self, cid: noshape_sketch::ConstraintId, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let vars = self.display.variables.clone();
        let res = self.with_sketch(cx, |ed, data| {
            let r = ed.apply_dimension(data, cid, text, &units, &vars);
            if r.is_ok() {
                ed.dim_edit = None;
            }
            r
        });
        match res {
            Some(Ok(())) => {
                self.dim_input = None;
                self.regen_sync(window, cx);
                window.focus(&self.focus, cx);
            }
            Some(Err(e)) => self.message = Some((FeatureStatus::Error, e)),
            None => {}
        }
        cx.notify();
    }

    fn open_dim_edit(&mut self, cid: noshape_sketch::ConstraintId, window: &mut Window, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let text = self
            .edit
            .as_ref()
            .and_then(|e| e.dialog.read(cx).sketch_data().and_then(|d| d.dimension_exprs.get(&cid.0).map(|x| x.text.clone()).or_else(|| {
                let c = d.sketch.constraint(cid)?;
                let v = c.kind.value()?;
                let ang = matches!(c.kind, noshape_sketch::ConstraintKind::Angle { .. } | noshape_sketch::ConstraintKind::ArcAngle { .. });
                Some(noshape_units::format_quantity(noshape_units::Quantity::new(v, if ang { noshape_units::Dim::ANGLE } else { noshape_units::Dim::LENGTH }), &units))
            })))
            .unwrap_or_default();
        if let Some(ed) = &mut self.sketch {
            ed.dim_edit = Some(crate::sketch_mode::DimEdit { constraint: cid, text });
        }
        self.sync_dim_edit(window, cx);
        cx.notify();
    }

    /// Dimension labels and inline value editor (viewport overlay).
    pub(super) fn render_sketch_labels(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let Some(ed) = &self.sketch else { return Vec::new() };
        let Some(data) = self.edit.as_ref().and_then(|e| e.dialog.read(cx).sketch_data().cloned()) else { return Vec::new() };
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let labels = ed.dimension_labels(&data, &units);
        let frame = ed.frame;
        let editing = self.dim_input.as_ref().map(|(c, st, _)| (*c, st.clone()));
        let msg = ed.message.clone();
        let vp = self.viewport.read(cx);
        let mut out = Vec::new();
        for (cid, pos, text, driving) in labels {
            let Some(s) = vp.project(frame.to_world(pos)) else { continue };
            let (x, y) = (s.x as f32, s.y as f32);
            if let Some((ec, st)) = &editing
                && *ec == cid
            {
                out.push(div().absolute().left(px(x - 50.0)).top(px(y - 13.0)).w(px(110.)).child(Input::new(st).xsmall()).into_any_element());
                continue;
            }
            out.push(
                div()
                    .id(SharedString::from(format!("dim-{}", cid.0)))
                    .absolute()
                    .left(px(x))
                    .top(px(y - 9.0))
                    .px_1()
                    .rounded_sm()
                    .bg(with_alpha(pal.panel_raised, 0.85))
                    .border_1()
                    .border_color(pal.border)
                    .text_xs()
                    .text_color(if driving { pal.text } else { pal.text_faint })
                    .cursor_pointer()
                    .child(text)
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        if ev.click_count >= 2 && driving {
                            this.open_dim_edit(cid, window, cx);
                        } else if let Some(ed) = &mut this.sketch {
                            if let Some(pos) = ed.selected_constraints.iter().position(|c| *c == cid) {
                                ed.selected_constraints.remove(pos);
                            } else {
                                ed.selected_constraints.push(cid);
                            }
                            cx.notify();
                        }
                    }))
                    .into_any_element(),
            );
        }
        if let Some(m) = msg {
            out.push(div().absolute().bottom_3().left(px(340.)).px_2().py_1().rounded_md().bg(with_alpha(pal.panel_raised, 0.9)).text_xs().text_color(pal.text_muted).child(m).into_any_element());
        }
        out
    }

    pub(super) fn render_sketch_toolbar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(ed) = &self.sketch else { return div().into_any_element() };
        let active = ed.tool;
        let construction = ed.construction;
        let tool_btn = |tool: Tool, cx: &mut Context<Self>| {
            Button::new(SharedString::from(format!("sk-{tool:?}")))
                .icon(Icon::empty().path(icon_path(tool.icon())).text_color(pal.text).with_size(px(18.)))
                .ghost()
                .with_size(px(30.))
                .selected(active == tool)
                .tooltip(tool.label())
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_tool(tool, cx)))
                .into_any_element()
        };
        let sep = || div().w(px(1.)).h(px(22.)).mx_1().bg(pal.border).into_any_element();
        let mut bar = h_flex().h(px(40.)).px_2().gap_0p5().bg(pal.panel).border_b_1().border_color(pal.accent);
        bar = bar.child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.accent).mr_2().child("SKETCH"));
        bar = bar.child(tool_btn(Tool::Select, cx)).child(sep());
        for group in DRAW_TOOLS {
            // Show the active member of a group as its face.
            let face = group.iter().copied().find(|t| *t == active).unwrap_or(group[0]);
            bar = bar.child(tool_btn(face, cx));
            if group.len() > 1 {
                let view = cx.entity();
                let members: Vec<Tool> = group.to_vec();
                bar = bar.child(
                    Button::new(SharedString::from(format!("skm-{face:?}")))
                        .icon(Icon::new(gpui_kit::component::IconName::ChevronDown).xsmall())
                        .ghost()
                        .xsmall()
                        .w(px(14.))
                        .dropdown_menu(move |mut menu, _, _| {
                            for t in &members {
                                let v = view.clone();
                                let t = *t;
                                menu = menu.item(PopupMenuItem::new(t.label()).icon(Icon::empty().path(icon_path(t.icon()))).on_click(move |_, _, cx| v.update(cx, |this, cx| this.set_tool(t, cx))));
                            }
                            menu
                        }),
                );
            }
        }
        bar = bar.child(
            Button::new("sk-construction")
                .icon(Icon::empty().path(icon_path("construction")).text_color(pal.text))
                .ghost()
                .small()
                .selected(construction)
                .tooltip("Construction (Q)")
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_construction(window, cx))),
        );
        bar = bar.child(sep());
        for t in EDIT_TOOLS {
            bar = bar.child(tool_btn(*t, cx));
        }
        bar = bar.child(sep()).child(tool_btn(Tool::Dimension, cx)).child(sep());
        for c in ConstraintTool::ALL {
            bar = bar.child(
                Button::new(SharedString::from(format!("skc-{c:?}")))
                    .icon(Icon::empty().path(icon_path(c.icon())).text_color(pal.text).with_size(px(18.)))
                    .ghost()
                    .with_size(px(30.))
                    .selected(active == Tool::Constraint(c))
                    .tooltip(c.label())
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.constraint(c, window, cx))),
            );
        }
        bar = bar.child(div().flex_1()).child(
            Button::new("sk-done")
                .label("Finish sketch")
                .icon(Icon::empty().path(icon_path("check")))
                .primary()
                .small()
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.accept_edit(false, window, cx))),
        );
        bar.into_any_element()
    }

    /// Sketch keyboard actions on the view root.
    pub(super) fn with_sketch_actions(&mut self, root: Div, cx: &mut Context<Self>) -> AnyElement {
        if self.sketch.is_none() {
            return root.into_any_element();
        }
        let t = |tool: Tool| move |this: &mut Self, _: &dyn std::any::Any, _: &mut Window, cx: &mut Context<Self>| this.set_tool(tool, cx);
        let _ = t;
        root.on_action(cx.listener(|this, _: &SketchLine, _, cx| this.set_tool(Tool::Line, cx)))
            .on_action(cx.listener(|this, _: &SketchArc3, _, cx| this.set_tool(Tool::Arc3, cx)))
            .on_action(cx.listener(|this, _: &SketchCircle, _, cx| this.set_tool(Tool::Circle, cx)))
            .on_action(cx.listener(|this, _: &SketchCenterRectangle, _, cx| this.set_tool(Tool::CenterRect, cx)))
            .on_action(cx.listener(|this, _: &SketchCornerRectangle, _, cx| this.set_tool(Tool::CornerRect, cx)))
            .on_action(cx.listener(|this, _: &SketchPoint, _, cx| this.set_tool(Tool::Point, cx)))
            .on_action(cx.listener(|this, _: &SketchFillet, _, cx| this.set_tool(Tool::Fillet, cx)))
            .on_action(cx.listener(|this, _: &SketchOffset, _, cx| this.set_tool(Tool::Offset, cx)))
            .on_action(cx.listener(|this, _: &SketchTrim, _, cx| this.set_tool(Tool::Trim, cx)))
            .on_action(cx.listener(|this, _: &SketchExtend, _, cx| this.set_tool(Tool::Extend, cx)))
            .on_action(cx.listener(|this, _: &SketchDimension, _, cx| this.set_tool(Tool::Dimension, cx)))
            .on_action(cx.listener(|this, _: &SketchConstruction, window, cx| this.toggle_construction(window, cx)))
            .on_action(cx.listener(|this, _: &SketchTangentArcToggle, _, cx| {
                let next = if this.sketch.as_ref().is_some_and(|s| s.tool == Tool::TangentArc) { Tool::Line } else { Tool::TangentArc };
                this.set_tool(next, cx)
            }))
            .on_action(cx.listener(|this, _: &ConstraintCoincident, w, cx| this.constraint(ConstraintTool::Coincident, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintConcentric, w, cx| this.constraint(ConstraintTool::Concentric, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintCurvature, w, cx| this.constraint(ConstraintTool::Curvature, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintEqual, w, cx| this.constraint(ConstraintTool::Equal, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintFix, w, cx| this.constraint(ConstraintTool::Fix, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintHorizontal, w, cx| this.constraint(ConstraintTool::Horizontal, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintVertical, w, cx| this.constraint(ConstraintTool::Vertical, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintMidpoint, w, cx| this.constraint(ConstraintTool::Midpoint, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintNormal, w, cx| this.constraint(ConstraintTool::Normal, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintParallel, w, cx| this.constraint(ConstraintTool::Parallel, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintPerpendicular, w, cx| this.constraint(ConstraintTool::Perpendicular, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintPierce, w, cx| this.constraint(ConstraintTool::Pierce, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintSymmetric, w, cx| this.constraint(ConstraintTool::Symmetric, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintTangent, w, cx| this.constraint(ConstraintTool::Tangent, w, cx)))
            .into_any_element()
    }
}


#[allow(dead_code)]
fn unused(_: ParamValue) {}
