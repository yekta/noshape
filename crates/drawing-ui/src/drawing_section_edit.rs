//! Drawing editor: section view menu › Edit section… (live drawing/499–504,
//! dom/drawing-edit-section.txt): the Section view dialog reopens (label, Edit
//! cutting line, Opposite direction, Show cut geometry only, Section depth with
//! its End type, Depth of section cut and Offset as when the section is made,
//! live2 drw-section-edit-depth-direction; ✓ ✗) with the points the cutting line passes
//! through as orange dots; hovering another snap point previews the jog to
//! it, clicking adds a stepped segment through it, clicking a dot again
//! removes it. The section view updates live; ✗ / Esc puts it back.

use crate::drawing_view::*;
use crate::theme::p;
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
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
    /// The section's label field (live: an input, as when creating it) and its text.
    pub label: (Entity<InputState>, Subscription),
    pub label_text: String,
    /// Opposite direction: look the other way across the cutting line.
    pub flip: bool,
    /// Section depth's End type Up to entity (else Blind), its picked point, whether the field takes clicks, and its
    /// Offset (field, value in meters, Opposite direction).
    pub up_to: bool,
    pub depth_ref: Option<noshape_model::drawing::PointRef>,
    pub picking: bool,
    pub offset: Option<(Entity<InputState>, Subscription)>,
    pub offset_value: Option<f64>,
    pub offset_flip: bool,
    /// ☑ Exclude from cut (assembly views) and its Excluded parts; the field takes the clicks while `exclude_picking`
    /// (live2 drw-section-exclude-from-cut s10-s21).
    pub exclude_on: bool,
    pub exclude: Vec<noshape_model::BodyId>,
    pub exclude_picking: bool,
}

impl DrawingEditor {
    pub fn open_section_edit(&mut self, view: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let sheet = self.current_sheet();
        let Some(v) = sheet.view(view) else { return };
        let ViewKind::Section { parent, label: text, .. } = &v.kind else { return };
        let (parent, text, cut_only, depth, flip) = (*parent, text.clone(), v.cut_only, v.section_depth, v.flip);
        let (depth_ref, offset, exclude) = (v.section_depth_ref.clone(), v.section_depth_offset, v.exclude_from_cut.clone());
        let Some(cuts) = sheet.section_cuts(view) else { return };
        let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(text.clone()));
        let sub = cx.subscribe(&st, |this: &mut Self, st, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                let v = st.read(cx).value().trim().to_string();
                if let Some(e) = &mut this.section_edit {
                    e.label_text = v;
                }
                this.preview_section_edit(cx);
            }
        });
        let up_to = depth_ref.is_some();
        self.section_edit = Some(SectionEdit { view, parent, cuts, cut_only, depth: None, depth_value: None, label: (st, sub), label_text: text, flip, up_to, depth_ref, picking: false, offset: None, offset_value: None, offset_flip: offset.is_some_and(|o| o < 0.0), exclude_on: !exclude.is_empty(), exclude, exclude_picking: false });
        if depth.is_some() || up_to {
            self.toggle_edit_depth(depth, window, cx);
            if let Some(e) = &mut self.section_edit {
                e.up_to = up_to;
            }
        }
        if let Some(o) = offset {
            self.toggle_edit_offset(Some(o.abs()), window, cx);
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
            e.picking = false;
        } else {
            // As when the section is made: Up to entity first, its field taking the pick.
            e.up_to = true;
            e.picking = e.depth_ref.is_none();
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

    /// Up to entity › ☑ Offset: its field (`value` meters, or the default length).
    fn toggle_edit_offset(&mut self, value: Option<f64>, window: &mut Window, cx: &mut Context<Self>) {
        let mm = self.drawing.units_mm;
        let Some(e) = &mut self.section_edit else { return };
        if e.offset.is_some() {
            (e.offset, e.offset_value) = (None, None);
        } else {
            let text = match value {
                Some(m) if mm => format!("{} mm", noshape_model::drawing::format::format_number(m * 1000.0, None, 3)),
                Some(m) => format!("{} in", noshape_model::drawing::format::format_number(m / 0.0254, None, 3)),
                None if mm => "25 mm".into(),
                None => "1 in".into(),
            };
            e.offset_value = crate::drawing_section::parse_offset(&text, mm);
            let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(text));
            let sub = cx.subscribe(&st, move |this: &mut Self, st, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    let v = crate::drawing_section::parse_offset(&st.read(cx).value(), mm);
                    if let Some(e) = &mut this.section_edit {
                        e.offset_value = v;
                    }
                    this.preview_section_edit(cx);
                }
            });
            e.offset = Some((st, sub));
        }
        self.preview_section_edit(cx);
        cx.notify();
    }

    /// The drawing with the edited cut points.
    pub fn section_edited(&self) -> Option<noshape_model::drawing::Drawing> {
        let e = self.section_edit.as_ref()?;
        let mut dr = self.drawing.clone();
        let s = dr.sheets.get_mut(self.sheet)?;
        s.set_section_cuts(e.view, e.cuts.clone());
        if let Some(v) = s.view_mut(e.view) {
            v.cut_only = e.cut_only && e.depth.is_none();
            v.flip = e.flip;
            if let ViewKind::Section { label, .. } = &mut v.kind
                && !e.label_text.is_empty()
            {
                label.clone_from(&e.label_text);
            }
            v.exclude_from_cut = if e.exclude_on { e.exclude.clone() } else { Vec::new() };
            (v.section_depth, v.section_depth_ref, v.section_depth_offset) = match (&e.depth, e.up_to) {
                (None, _) => (None, None, None),
                (Some(_), false) => (e.depth_value, None, None),
                (Some(_), true) => {
                    let offset = e.offset_value.filter(|_| e.offset.is_some() && e.depth_ref.is_some()).map(|o| if e.offset_flip { -o } else { o });
                    (None, e.depth_ref.clone(), offset)
                }
            };
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
        // Excluded parts takes the click: the part whose edge is under it, in or out.
        if e.exclude_picking {
            if let Some(body) = self.edge_body_at(self.cursor)
                && let Some(e) = &mut self.section_edit
            {
                match e.exclude.iter().position(|b| *b == body) {
                    Some(i) => {
                        e.exclude.remove(i);
                    }
                    None => e.exclude.push(body),
                }
                self.preview_section_edit(cx);
            }
            cx.notify();
            return true;
        }
        // Up to entity's field takes the click: the point the depth runs to.
        if e.picking {
            let r = self.snap_at(self.cursor).and_then(|p| p.r);
            if let (Some(r), Some(e)) = (r, &mut self.section_edit) {
                (e.depth_ref, e.picking) = (Some(r), false);
                self.preview_section_edit(cx);
            } else {
                self.status = Some("Select a vertex or circle center for the section depth".into());
            }
            cx.notify();
            return true;
        }
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

    /// A part's name in `view`'s source ("Part 2"), for the Excluded parts chips.
    fn part_name_in(&self, view: ViewId, body: &noshape_model::BodyId) -> String {
        noshape_model::drawing::views::view_source(self.current_sheet(), view)
            .and_then(|(k, _)| self.sources.get(&k))
            .and_then(|s| s.parts.iter().find(|p| p.body.as_ref() == Some(body)).map(|p| p.name.clone()))
            .unwrap_or_else(|| "Part".into())
    }

    pub fn render_section_edit(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let e = self.section_edit.as_ref()?;
        let pal = p();
        let assembly = noshape_model::drawing::views::view_source(self.current_sheet(), e.parent).and_then(|(k, _)| self.sources.get(&k)).is_some_and(|s| s.assembly);
        let (exclude_on, exclude_picking) = (e.exclude_on, e.exclude_picking);
        let excluded: Vec<(usize, String)> = e.exclude.iter().enumerate().map(|(i, b)| (i, self.part_name_in(e.parent, b))).collect();
        let label = e.label.0.clone();
        let (cut_only, flip, up_to, picking, has_ref, offset_flip) = (e.cut_only, e.flip, e.up_to, e.picking, e.depth_ref.is_some(), e.offset_flip);
        let depth = e.depth.as_ref().map(|(st, _)| st.clone());
        let offset = e.offset.as_ref().map(|(st, _)| st.clone());
        crate::ui_script::note_dialog(|| {
            // Live (drw-section-edit-depth-direction): Show cut geometry only goes while Section depth is on; End type is
            // a select, its options left out by live's recorder since 2026-11-17.3.
            let mut t = crate::drawing_panels::PanelText::default();
            t.text("Section view");
            let mut fields = Vec::new();
            if depth.is_none() {
                t.text("Show cut geometry only");
                fields.push(serde_json::json!({ "label": "Show cut geometry only", "kind": "checkbox", "value": cut_only }));
            }
            t.text("Section depth");
            fields.push(serde_json::json!({ "label": "Section depth", "kind": "checkbox", "value": depth.is_some() }));
            if depth.is_some() {
                t.options(["Up to entity", "Blind"]);
                fields.push(serde_json::json!({ "label": "End type", "kind": "select", "value": if up_to { "Up to entity" } else { "Blind" } }));
                if up_to {
                    t.texts([if has_ref { "Point" } else { "Depth of section cut" }, "Offset"]);
                    fields.push(serde_json::json!({ "label": "Offset", "kind": "checkbox", "value": offset.is_some() }));
                } else {
                    t.text("Depth");
                }
            }
            if assembly {
                t.text("Exclude from cut");
                fields.push(serde_json::json!({ "label": "Exclude from cut", "kind": "checkbox", "value": exclude_on }));
                if exclude_on {
                    t.text("Excluded parts");
                    for (_, name) in &excluded {
                        t.texts([name.clone(), "×".into()]);
                    }
                }
            }
            crate::drawing_panels::panel_text(serde_json::json!({ "kind": "drawing-panel", "title": "Section view", "fields": fields }), t.0)
        });
        let ed = cx.entity();
        // (Marked by their tooltips: live's runner aims at them by pixel, ours by name: `ours_on`.)
        let square = |id: &'static str, on: bool, tip: &'static str| {
            div().id(id).map(move |d| crate::ui_script::mark(d, move || format!("button:{tip}"))).size(px(26.)).flex().items_center().justify_center().border_1().border_color(gpui_kit::rgb(0xd3d7d9)).when(on, |d| d.bg(gpui_kit::rgb(0xcfdbe5))).cursor_pointer().tooltip(crate::ui::tooltip(tip))
        };
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
                        .child(div().id("se-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(pal.accept).cursor_pointer().child(crate::drawing_chrome::ok_check(16.)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_section_edit(true, cx))))
                        .child(div().id("se-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 14., pal.cancel)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_section_edit(false, cx)))),
                )
                .child(div().px(px(10.)).pt(px(4.)).child(Input::new(&label).xsmall()))
                // The tool button (live 499–504, as in the creating dialog): the cutting line's points are being picked,
                // so it shows active; Opposite direction at the row's right end (live2 drw-section-edit-depth-direction).
                .child(
                    h_flex()
                        .px(px(10.))
                        .pt(px(6.))
                        .child(
                            div()
                                .id("se-line")
                                .map(|d| crate::ui_script::mark(d, || "button:Edit cutting line".into()))
                                .size(px(30.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(2.))
                                .border_1()
                                .border_color(gpui_kit::rgb(0x9aa5af))
                                .bg(gpui_kit::rgb(0xcfdbe5))
                                .tooltip(crate::ui::tooltip("Edit cutting line"))
                                .child(crate::assets::icon("section-cut-line", 20., cx)),
                        )
                        .child(div().flex_1())
                        .child(square("se-flip", flip, "Opposite direction").child(crate::assets::icon("section-flip", 20., cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(e) = &mut this.section_edit {
                                e.flip = !e.flip;
                            }
                            this.preview_section_edit(cx);
                            cx.notify();
                        }))),
                )
                .when(depth.is_none(), |d| d.child(div().px(px(10.)).pt(px(6.)).child(crate::drawing_chrome::DrCheckbox::new("se-cut-only").label("Show cut geometry only").checked(cut_only).on_click(cx.listener(|this, v: &bool, _, cx| {
                    if let Some(e) = &mut this.section_edit {
                        e.cut_only = *v;
                    }
                    this.preview_section_edit(cx);
                })))))
                .child(div().px(px(10.)).pt(px(4.)).child(crate::drawing_chrome::DrCheckbox::new("se-depth").label("Section depth").checked(depth.is_some()).on_click(cx.listener(|this, _: &bool, w, cx| this.toggle_edit_depth(None, w, cx)))))
                .when(depth.is_some(), |d| {
                    d.child(
                        div().px(px(8.)).pt(px(4.)).child(
                            Button::new("se-end-type")
                                .ghost()
                                .w_full()
                                .h(px(26.))
                                .border_1()
                                .border_color(gpui_kit::rgb(0xcccccc))
                                .rounded(px(4.))
                                .dropdown_caret(true)
                                .tooltip("End type")
                                .child(div().flex_1().text_size(px(12.)).child(if up_to { "Up to entity" } else { "Blind" }))
                                .os_dropdown_menu("se-end-type-dd", true, move |mut m, _, _| {
                                    for (label, v) in [("Up to entity", true), ("Blind", false)] {
                                        let ed = ed.clone();
                                        m = m.item(crate::os_menu::OsMenuItem::new(label).checked(up_to == v).on_click(move |_, _, cx| {
                                            ed.update(cx, |this, cx| {
                                                if let Some(e) = &mut this.section_edit {
                                                    e.up_to = v;
                                                    e.picking = v && e.depth_ref.is_none();
                                                }
                                                this.preview_section_edit(cx);
                                                cx.notify();
                                            })
                                        }));
                                    }
                                    m
                                }),
                        ),
                    )
                })
                .when_some(depth.clone().filter(|_| !up_to), |d, st| d.child(h_flex().px(px(10.)).pt(px(4.)).gap(px(6.)).child(div().text_size(px(12.)).text_color(pal.text_muted).child("Depth")).child(div().flex_1().child(Input::new(&st).xsmall()))))
                .when(depth.is_some() && up_to, |d| {
                    d.child(
                        div()
                            .id("se-upto-field")
                            .map(|d| crate::ui_script::mark(d, || "button:Up to geometry".into()))
                            .tooltip(crate::ui::tooltip("Up to geometry"))
                            .mx(px(10.))
                            .mt(px(4.))
                            .h(px(26.))
                            .px(px(6.))
                            .flex()
                            .items_center()
                            .text_size(px(12.))
                            .border_1()
                            .border_color(if picking { pal.accent } else { gpui_kit::rgb(0xd3d7d9) })
                            .when(picking, |d| d.bg(gpui_kit::rgb(0xe8f1fb)))
                            .cursor_pointer()
                            .child(if has_ref { "Point" } else { "Depth of section cut" })
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(e) = &mut this.section_edit {
                                    e.picking = !e.picking;
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        h_flex()
                            .px(px(8.))
                            .pt(px(4.))
                            .gap(px(6.))
                            .child(crate::drawing_chrome::DrCheckbox::new("se-offset").label("Offset").checked(offset.is_some()).on_click(cx.listener(|this, _: &bool, w, cx| this.toggle_edit_offset(None, w, cx))))
                            .when_some(offset.clone(), |r, st| {
                                r.child(div().flex_1().child(Input::new(&st).xsmall())).child(square("se-offset-flip", offset_flip, "Opposite direction").child(crate::assets::icon(&crate::assets::icon_path("flip"), 16., cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if let Some(e) = &mut this.section_edit {
                                        e.offset_flip = !e.offset_flip;
                                    }
                                    this.preview_section_edit(cx);
                                    cx.notify();
                                })))
                            }),
                    )
                })
                .when(assembly, |p| {
                    p.child(div().px(px(10.)).pt(px(4.)).child(crate::drawing_chrome::DrCheckbox::new("se-exclude-on").label("Exclude from cut").checked(exclude_on).on_click(cx.listener(|this, v: &bool, _, cx| {
                        if let Some(e) = &mut this.section_edit {
                            // Ticked, its Excluded parts field takes the clicks; unticked, nothing is excluded.
                            (e.exclude_on, e.exclude_picking) = (*v, *v);
                            if !*v {
                                e.exclude.clear();
                            }
                        }
                        this.preview_section_edit(cx);
                        cx.notify();
                    }))))
                })
                .when(assembly && exclude_on, |p| {
                    let chips: Vec<AnyElement> = excluded
                        .iter()
                        .map(|(i, name)| {
                            let i = *i;
                            h_flex()
                                .px(px(4.))
                                .gap(px(4.))
                                .bg(gpui_kit::rgb(0xe6e6e6))
                                .text_size(px(12.))
                                .child(name.clone())
                                .child(
                                    div()
                                        .id(("se-exclude-x", i))
                                        .cursor_pointer()
                                        .tooltip(crate::ui::tooltip(&format!("Delete {name}")))
                                        .child("×")
                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            if let Some(e) = &mut this.section_edit
                                                && i < e.exclude.len()
                                            {
                                                e.exclude.remove(i);
                                            }
                                            this.preview_section_edit(cx);
                                            cx.notify();
                                        })),
                                )
                                .into_any_element()
                        })
                        .collect();
                    p.child(div().px(px(10.)).pt(px(4.)).text_size(px(12.)).text_color(pal.text_muted).child("Excluded parts")).child(
                        div()
                            .id("se-exclude")
                            .mx(px(10.))
                            .my(px(3.))
                            .min_h(px(26.))
                            .p(px(3.))
                            .flex()
                            .flex_wrap()
                            .gap(px(4.))
                            .border_1()
                            .border_color(if exclude_picking { pal.accent } else { gpui_kit::rgb(0xd3d7d9) })
                            .when(exclude_picking, |d| d.bg(gpui_kit::rgb(0xe8f1fb)))
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip("Select parts to exclude from cut"))
                            .children(chips)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(e) = &mut this.section_edit {
                                    e.exclude_picking = !e.exclude_picking;
                                }
                                cx.notify();
                            })),
                    )
                })
                .into_any_element(),
        )
    }
}
