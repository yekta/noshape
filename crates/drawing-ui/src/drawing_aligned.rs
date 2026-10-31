//! Drawing editor: Onshape's Aligned section view panel (live asb 10-24
//! "aligned section dialog"): Section id (the next free letter), Center,
//! Viewing plane location, Revolved plane location (each "Point ×" once
//! clicked, the next one active), ☐ Show cut geometry only. ✓ is enabled
//! once the three points are picked; a click on the sheet places the view.
//! Edit aligned section... reopens it prefilled (✓ applies the Section id
//! and Show cut geometry only).

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{ViewId, ViewKind};

/// Aligned section view panel state.
pub struct AlignedDlg {
    pub label: Entity<InputState>,
    pub cut_only: bool,
    /// The Viewing plane location's flip button: the view looks the other way.
    pub flip: bool,
    /// Edit aligned section...: the view edited (the tool stays Select).
    pub edit: Option<ViewId>,
    /// Editing: its center and the two legs' points (parent view
    /// coordinates), and which of them a click on the parent re-picks (a
    /// query row clicked).
    pub points: Option<([f64; 2], [f64; 2], [f64; 2])>,
    pub repick: Option<usize>,
}

impl DrawingEditor {
    /// Create / drop the panel with the Aligned section view tool (called
    /// from render); an edit request opens it prefilled.
    pub fn sync_aligned_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = self.aligned_edit_request.take()
            && let Some(v) = self.current_sheet().view(view)
            && let ViewKind::AlignedSection { label, center, a, b, .. } = &v.kind
        {
            let (label, cut_only, flip, points) = (label.clone(), v.cut_only, v.flip, (*center, *a, *b));
            let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Section id").default_value(label));
            self.tool = Tool::Select;
            self.aligned_dlg = Some(AlignedDlg { label: st, cut_only, flip, edit: Some(view), points: Some(points), repick: None });
            return;
        }
        let active = matches!(self.tool, Tool::AlignedSection { .. });
        let editing = self.aligned_dlg.as_ref().is_some_and(|d| d.edit.is_some());
        match (&self.aligned_dlg, active) {
            (None, true) => {
                let next = self.current_sheet().next_label();
                let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Section id").default_value(next));
                self.aligned_dlg = Some(AlignedDlg { label: st, cut_only: false, flip: false, edit: None, points: None, repick: None });
            }
            (Some(_), false) if !editing => self.aligned_dlg = None,
            _ => {}
        }
    }

    /// Editing, a query row clicked: the next click on the parent view
    /// re-picks that point (the legs run on past the part as when placed).
    /// True when used.
    pub fn aligned_repick_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(d) = &self.aligned_dlg else { return false };
        let (Some(k), Some(view), Some((c, a, b))) = (d.repick, d.edit, d.points) else { return false };
        let Some(parent) = self.current_sheet().parent_of(view) else { return false };
        let Some(pp) = self.view_point(Some(parent)) else { return true };
        let far = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) > 1e-7;
        let leg = |this: &Self, p: [f64; 2]| this.pk().extend_line_ends(parent, &[p, c])[0];
        let points = match k {
            0 => (pp.p, a, b),
            1 if far(pp.p, c) => (c, leg(self, pp.p), b),
            2 if far(pp.p, c) => (c, a, leg(self, pp.p)),
            _ => return true,
        };
        if let Some(d) = &mut self.aligned_dlg {
            d.points = Some(points);
            d.repick = None;
        }
        // The edited view as it will be.
        let mut dr = self.drawing.clone();
        if let Some(ViewKind::AlignedSection { center, a, b, .. }) = dr.sheets[self.sheet].view_mut(view).map(|v| &mut v.kind) {
            (*center, *a, *b) = points;
        }
        self.set_preview(Some(dr), cx);
        cx.notify();
        true
    }

    /// Edit aligned section... (the cutting line's and the view's menus).
    pub fn open_aligned_edit(&mut self, view: ViewId, cx: &mut Context<Self>) {
        self.aligned_edit_request = Some(view);
        cx.notify();
    }

    /// The panel's Section id, Show cut geometry only and flip.
    pub fn aligned_values(&self, cx: &App) -> Option<(String, bool, bool)> {
        let d = self.aligned_dlg.as_ref()?;
        let t = d.label.read(cx).value().trim().to_string();
        Some((t, d.cut_only, d.flip))
    }

    /// ✓: an edit applies; a new view is placed where its preview is.
    fn aligned_ok(&mut self, cx: &mut Context<Self>) {
        let Some((label, cut_only, flip)) = self.aligned_values(cx) else { return };
        if let Some(view) = self.aligned_dlg.as_ref().and_then(|d| d.edit) {
            let points = self.aligned_dlg.as_ref().and_then(|d| d.points);
            self.aligned_dlg = None;
            self.set_preview(None, cx);
            self.edit_view("Edit aligned section", view, cx, |v| {
                if let (ViewKind::AlignedSection { center, a, b, .. }, Some(p)) = (&mut v.kind, points) {
                    (*center, *a, *b) = p;
                }
                if let ViewKind::AlignedSection { label: l, .. } = &mut v.kind
                    && !label.is_empty()
                {
                    *l = label.clone();
                }
                v.cut_only = cut_only;
                v.flip = flip;
            });
            return;
        }
        if matches!(self.tool, Tool::AlignedSection { b: Some(_), .. }) {
            self.place_aligned_section(cx);
        }
    }

    /// Place the pending aligned section view with the panel's values.
    pub fn place_aligned_section(&mut self, cx: &mut Context<Self>) {
        let Some(mut v) = self.pending_view() else { return };
        if let Some((label, cut_only, flip)) = self.aligned_values(cx) {
            if let ViewKind::AlignedSection { label: l, .. } = &mut v.kind
                && !label.is_empty()
            {
                *l = label;
            }
            v.cut_only = cut_only;
            v.flip = flip;
        }
        let id = v.id;
        self.edit_sheet("Aligned section view", cx, |s| s.views.push(v));
        self.tool = Tool::Select;
        self.aligned_dlg = None;
        self.selection = vec![Sel::View(id)];
    }

    pub fn render_aligned_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.aligned_dlg.as_ref()?;
        let pal = p();
        let editing = d.edit.is_some();
        // Picked so far: 0 none, 1 center, 2 + viewing, 3 + revolved.
        let picked = match &self.tool {
            _ if editing => 3,
            Tool::AlignedSection { center: None, .. } => 0,
            Tool::AlignedSection { a: None, .. } => 1,
            Tool::AlignedSection { b: None, .. } => 2,
            _ => 3,
        };
        let ink = gpui_kit::rgb(0x333333);
        let caption = gpui_kit::rgb(0x616b73);
        // A query row: its caption, then "Point ×" once picked; the next one
        // to pick is active (blue underline), later ones disabled.
        let repick = d.repick;
        let query = |id: &'static str, title: &'static str, k: usize, cx: &mut Context<Self>| {
            // Editing: a click re-picks the point (the row turns active).
            let (done, active) = (picked > k, if editing { repick == Some(k) } else { picked == k });
            v_flex()
                .id(id)
                .when(editing, |e| {
                    e.cursor_pointer().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.aligned_dlg {
                            d.repick = Some(k);
                        }
                        cx.notify();
                    }))
                })
                .mt(px(4.))
                .px(px(4.))
                .pb(px(3.))
                .border_b_1()
                .border_color(if active { pal.accent } else { gpui_kit::rgb(0xdddddd) })
                .when(!done && !active, |e| e.opacity(0.45))
                .child(div().text_size(px(11.)).text_color(caption).child(title))
                .when(done, |e| e.child(h_flex().gap(px(4.)).child(div().px(px(5.)).rounded(px(2.)).bg(gpui_kit::rgb(0xe5ebf0)).text_size(px(12.)).text_color(ink).child("Point ×"))))
                .into_any_element()
        };
        let (check_on, flipped) = (d.cut_only, d.flip);
        Some(
            v_flex()
                .id("dr-aligned-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(229.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Aligned section view"))
                        // ✓ once the three points are picked.
                        .child(
                            div()
                                .id("da-ok")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(gpui_kit::rgb(0x009500))
                                .when(picked < 3, |e| e.bg(gpui_kit::rgb(crate::drawing_chrome::OK_DISABLED)))
                                .when(picked == 3, |e| e.cursor_pointer().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.aligned_ok(cx))))
                                .child(crate::ui::glyph("check", 18., gpui_kit::white())),
                        )
                        .child(
                            div()
                                .id("da-cancel")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(crate::drawing_chrome::cancel_x(cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.aligned_dlg = None;
                                    this.cancel(cx);
                                })),
                        ),
                )
                .child(
                    v_flex()
                        .p(px(3.))
                        .child(div().mt(px(1.)).h(px(25.)).flex().items_end().border_b_1().border_color(gpui_kit::rgb(0xdddddd)).child(Input::new(&d.label).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).small()))
                        .child(query("da-center", "Center", 0, cx))
                        .child(
                            h_flex()
                                .child(div().flex_1().child(query("da-viewing", "Viewing plane location", 1, cx)))
                                // Flip: the view looks the other way (help
                                // aligned_section_view).
                                .child(
                                    div()
                                        .id("da-flip")
                                        .size(px(24.))
                                        .mt(px(6.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .border_1()
                                        .border_color(gpui_kit::rgb(0xd3d7d9))
                                        .when(flipped, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                                        .cursor_pointer()
                                        .tooltip(crate::ui::tooltip("Flip direction"))
                                        .child(crate::assets::icon(&crate::assets::icon_path("flip"), 16., cx))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            if let Some(d) = &mut this.aligned_dlg {
                                                d.flip = !d.flip;
                                            }
                                            this.update_preview(cx);
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(query("da-revolved", "Revolved plane location", 2, cx))
                        .child(
                            div().h(px(30.)).px(px(4.)).flex().items_center().child(
                                crate::drawing_chrome::DrCheckbox::new("da-cut-only").native().label("Show cut geometry only").checked(check_on).on_click(cx.listener(|this, v: &bool, _, cx| {
                                    if let Some(d) = &mut this.aligned_dlg {
                                        d.cut_only = *v;
                                    }
                                    this.update_preview(cx);
                                })),
                            ),
                        ),
                )
                .into_any_element(),
        )
    }
}
