//! Drawing editor: section view menu › Edit section… (live drawing/499–504,
//! dom/drawing-edit-section.txt): the Section view dialog reopens (label,
//! Show cut geometry only, ✓ ✗) with the points the cutting line passes
//! through as orange dots; hovering another snap point previews the jog to
//! it, clicking adds a stepped segment through it, clicking a dot again
//! removes it. The section view updates live; ✗ / Esc puts it back.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use noshape_model::drawing::{ViewId, ViewKind};

/// The open Edit section dialog.
pub struct SectionEdit {
    pub view: ViewId,
    pub parent: ViewId,
    pub cuts: Vec<[f64; 2]>,
    pub cut_only: bool,
}

impl DrawingEditor {
    pub fn open_section_edit(&mut self, view: ViewId, cx: &mut Context<Self>) {
        let sheet = self.current_sheet();
        let Some(v) = sheet.view(view) else { return };
        let ViewKind::Section { parent, .. } = v.kind else { return };
        let Some(cuts) = sheet.section_cuts(view) else { return };
        self.section_edit = Some(SectionEdit { view, parent, cuts, cut_only: v.cut_only });
        self.tool = Tool::Select;
        cx.notify();
    }

    /// The drawing with the edited cut points.
    fn section_edited(&self) -> Option<noshape_model::drawing::Drawing> {
        let e = self.section_edit.as_ref()?;
        let mut dr = self.drawing.clone();
        let s = dr.sheets.get_mut(self.sheet)?;
        s.set_section_cuts(e.view, e.cuts.clone());
        if let Some(v) = s.view_mut(e.view) {
            v.cut_only = e.cut_only;
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
                overlay.lines.push((line.iter().map(|p| pl.to_sheet(*p)).collect(), true));
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
                .child(div().px(px(10.)).pt(px(6.)).child(crate::drawing_chrome::DrCheckbox::new("se-cut-only").label("Show cut geometry only").checked(cut_only).on_click(cx.listener(|this, v: &bool, _, cx| {
                    if let Some(e) = &mut this.section_edit {
                        e.cut_only = *v;
                    }
                    this.preview_section_edit(cx);
                }))))
                .into_any_element(),
        )
    }
}
