//! Drawing editor: section view menu › Edit section… (live drawing/499–504,
//! dom/drawing-edit-section.txt): the Section view dialog reopens (label,
//! Show cut geometry only, Section depth, ✓ ✗) with the points the cutting line passes
//! through as orange dots; hovering another snap point previews the jog to
//! it, clicking adds a stepped segment through it, clicking a dot again
//! removes it. The section view updates live; ✗ / Esc puts it back.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{ViewId, ViewKind};

/// The open Edit section dialog.
pub struct SectionEdit {
    pub view: ViewId,
    pub parent: ViewId,
    pub cuts: Vec<[f64; 2]>,
    pub cut_only: bool,
    /// ☑ Section depth: its field and value (meters).
    pub depth: Option<(Entity<InputState>, Subscription)>,
    pub depth_value: Option<f64>,
}

impl DrawingEditor {
    pub fn open_section_edit(&mut self, view: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let sheet = self.current_sheet();
        let Some(v) = sheet.view(view) else { return };
        let ViewKind::Section { parent, .. } = v.kind else { return };
        let Some(cuts) = sheet.section_cuts(view) else { return };
        let depth = v.section_depth;
        self.section_edit = Some(SectionEdit { view, parent, cuts, cut_only: v.cut_only, depth: None, depth_value: None });
        if let Some(d) = depth {
            self.toggle_edit_depth(Some(d), window, cx);
        }
        self.tool = Tool::Select;
        cx.notify();
    }

    /// ☐ Section depth (live: as when the section was made): the field,
    /// starting at `value` (meters) or 10 mm / 0.5 in.
    fn toggle_edit_depth(&mut self, value: Option<f64>, window: &mut Window, cx: &mut Context<Self>) {
        let mm = self.drawing.units_mm;
        let on = self.section_edit.as_ref().is_some_and(|e| e.depth.is_none());
        let Some(e) = &mut self.section_edit else { return };
        if !on {
            (e.depth, e.depth_value) = (None, None);
        } else {
            let text = match value {
                Some(m) if mm => format!("{} mm", noshape_model::drawing::format::format_number(m * 1000.0, None, 3)),
                Some(m) => format!("{} in", noshape_model::drawing::format::format_number(m / 0.0254, None, 3)),
                None if mm => "10 mm".into(),
                None => "0.5 in".into(),
            };
            e.depth_value = crate::drawing_section::parse_depth(&text, mm);
            let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(text));
            let sub = cx.subscribe(&st, move |this: &mut Self, st, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    let v = crate::drawing_section::parse_depth(&st.read(cx).value(), mm);
                    if let Some(e) = &mut this.section_edit {
                        e.depth_value = v;
                    }
                    this.preview_section_edit(cx);
                }
            });
            e.depth = Some((st, sub));
        }
        self.preview_section_edit(cx);
    }

    /// The drawing with the edited cut points.
    pub fn section_edited(&self) -> Option<noshape_model::drawing::Drawing> {
        let e = self.section_edit.as_ref()?;
        let mut dr = self.drawing.clone();
        let s = dr.sheets.get_mut(self.sheet)?;
        s.set_section_cuts(e.view, e.cuts.clone());
        if let Some(v) = s.view_mut(e.view) {
            v.cut_only = e.cut_only;
            if e.depth.is_none() {
                (v.section_depth, v.section_depth_ref) = (None, None);
            } else if e.depth_value != v.section_depth {
                (v.section_depth, v.section_depth_ref) = (e.depth_value, None);
            }
        }
        Some(dr)
    }

    fn preview_section_edit(&mut self, cx: &mut Context<Self>) {
        let dr = self.section_edited();
        self.set_preview(dr, cx);
    }

    /// A click while editing: a dot removes that point, another snap point
    /// of the parent view adds it. True when used.
    pub fn section_edit_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(e) = &self.section_edit else { return false };
        let Some(pl) = self.placement(e.parent) else { return true };
        let tol = self.tol_mm(4.0);
        let c = self.cursor;
        let hit = e.cuts.iter().position(|p| {
            let s = pl.to_sheet(*p);
            ((s[0] - c[0]).powi(2) + (s[1] - c[1]).powi(2)).sqrt() < tol
        });
        let snap = self.snap_at(c).filter(|s| s.view == e.parent).map(|s| s.p);
        let Some(e) = &mut self.section_edit else { return true };
        match hit {
            Some(i) if e.cuts.len() > 1 => {
                e.cuts.remove(i);
            }
            Some(_) => {}
            None => {
                if let Some(p) = snap {
                    e.cuts.push(p);
                }
            }
        }
        self.preview_section_edit(cx);
        true
    }

    /// Overlay while editing: the dots, and the line the hovered snap point
    /// would give.
    pub fn section_edit_overlay(&self, overlay: &mut crate::drawing_scene::Overlay) {
        let Some(e) = &self.section_edit else { return };
        let Some(pl) = self.placement(e.parent) else { return };
        let r = self.tol_mm(9.0);
        for p in &e.cuts {
            overlay.marks.push((pl.to_sheet(*p), crate::drawing_scene::SnapMark::Circle, r));
        }
        if let Some(s) = self.snap_at(self.cursor).filter(|s| s.view == e.parent) {
            let mut sheet = self.current_sheet().clone();
            let mut cuts = e.cuts.clone();
            cuts.push(s.p);
            if sheet.set_section_cuts(e.view, cuts)
                && let Some(line) = sheet.section_points(e.view)
            {
                overlay.tinted.push((line.iter().map(|p| pl.to_sheet(*p)).collect(), [255, 140, 0, 255], 1.5));
            }
        }
    }

    pub fn close_section_edit(&mut self, accept: bool, cx: &mut Context<Self>) {
        let dr = if accept { self.section_edited() } else { None };
        self.section_edit = None;
        self.set_preview(None, cx);
        if let Some(dr) = dr {
            self.commit("Edit section", dr, cx);
        }
        cx.notify();
    }

    pub fn render_section_edit(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let e = self.section_edit.as_ref()?;
        let pal = p();
        let label = match self.current_sheet().view(e.view).map(|v| &v.kind) {
            Some(ViewKind::Section { label, .. }) => label.clone(),
            _ => String::new(),
        };
        let cut_only = e.cut_only;
        let depth = e.depth.as_ref().map(|(st, _)| st.clone());
        Some(
            v_flex()
                .id("dr-section-edit")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(200.))
                .pb(px(6.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(gpui_kit::rgb(0x333333))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(10.))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Section view"))
                        .child(div().id("se-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(pal.accept).cursor_pointer().child(crate::ui::glyph("check", 16., gpui_kit::white())).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_section_edit(true, cx))))
                        .child(div().id("se-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 14., pal.cancel)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_section_edit(false, cx)))),
                )
                .child(div().px(px(10.)).h(px(26.)).flex().items_center().text_size(px(12.)).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).child(label))
                // The tool button (live 499–504, as in the creating dialog): the
                // cutting line's points are being picked, so it shows active.
                .child(
                    h_flex().px(px(10.)).pt(px(6.)).child(
                        div()
                            .id("se-line")
                            .size(px(30.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(2.))
                            .border_1()
                            .border_color(gpui_kit::rgb(0x9aa5af))
                            .bg(gpui_kit::rgb(0xcfdbe5))
                            .tooltip(crate::ui::tooltip("Cutting line"))
                            .child(crate::assets::icon("section-cut-line", 20., cx)),
                    ),
                )
                .child(div().px(px(10.)).pt(px(6.)).child(crate::drawing_chrome::DrCheckbox::new("se-cut-only").label("Show cut geometry only").checked(cut_only).on_click(cx.listener(|this, v: &bool, _, cx| {
                    if let Some(e) = &mut this.section_edit {
                        e.cut_only = *v;
                    }
                    this.preview_section_edit(cx);
                }))))
                .child(div().px(px(10.)).pt(px(4.)).child(crate::drawing_chrome::DrCheckbox::new("se-depth").label("Section depth").checked(depth.is_some()).on_click(cx.listener(|this, _: &bool, w, cx| this.toggle_edit_depth(None, w, cx)))))
                .when_some(depth, |d, st| d.child(div().px(px(10.)).pt(px(4.)).child(Input::new(&st).xsmall())))
                .into_any_element(),
        )
    }
}
