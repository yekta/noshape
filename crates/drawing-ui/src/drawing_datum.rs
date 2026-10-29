//! Drawing editor: the Datum dialog (Datum tool; live drawing/507–518,
//! dom/drawing-datum-target-*): tabs Datum | Datum target.
//!
//! Datum: Label (the next free letter, selected); clicks place datums as
//! before (`Tool::Datum`), the label advancing after each.
//!
//! Datum target: Target area (Point, Point (dot), Circle, Rectangle,
//! Custom), Style (Inside, Moveable horizontal inside, Moveable rotational
//! inside), ☐ Area size (Ø, or width × height), and the Targets list: a
//! click on a model point then a click for the callout adds "A1"; an
//! expanded target shows Label, Length ▾ (Length / Angle), Origin (click
//! the field, then a model point), Target location, X / Y position with
//! their distances (typing moves the target from the origin) and the X / Y
//! flip buttons. ✓ adds the targets (with their position dimensions from
//! the origin), ✗ / Esc discards them.

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{Annotation, AnnotationKind, AnnotationRef, DatumTargetArea, DatumTargetOpts, TargetStyle};

/// The Target area list (live drawing/509): name and glyph.
const AREAS: [(&str, &str); 5] = [("Point", "✕"), ("Point (dot)", "•"), ("Circle", "◍"), ("Rectangle", "▨"), ("Custom", "+")];

/// One target in the Targets list (not yet on the sheet).
pub struct TargetRow {
    pub ann: Annotation,
    pub label: Entity<InputState>,
    pub x: Entity<InputState>,
    pub y: Entity<InputState>,
}

/// The open Datum dialog.
pub struct DatumDlg {
    /// 0 Datum, 1 Datum target.
    pub tab: u8,
    pub label: Entity<InputState>,
    /// The label's text (kept in step with the field).
    pub label_text: String,
    pub area: usize,
    pub style: TargetStyle,
    pub area_size: bool,
    pub size_a: Entity<InputState>,
    pub size_b: Entity<InputState>,
    pub rows: Vec<TargetRow>,
    /// The expanded target.
    pub open: Option<usize>,
    /// The target whose Origin field takes the next click.
    pub picking_origin: Option<usize>,
    _subs: Vec<Subscription>,
}

impl DrawingEditor {
    /// Create / drop the dialog with the Datum tools (from render).
    pub fn sync_datum_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Datum { .. } | Tool::DatumTarget { .. });
        if active && self.datum_dlg.is_none() {
            let label = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(self.next_datum_label()));
            // Live 507: the label is focused with its letter selected.
            label.update(cx, |s, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
            let mm = self.drawing.units_mm;
            let size_a = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(if mm { "25 mm" } else { "1.000 in" }));
            let size_b = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(if mm { "25 mm" } else { "1.000 in" }));
            let tab = u8::from(matches!(self.tool, Tool::DatumTarget { .. }));
            let label_text = self.next_datum_label();
            let sub = cx.subscribe(&label, |this: &mut Self, st, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change)
                    && let Some(d) = &mut this.datum_dlg
                {
                    d.label_text = st.read(cx).value().trim().to_string();
                }
            });
            self.datum_dlg = Some(DatumDlg { tab, label, label_text, area: 0, style: TargetStyle::Inside, area_size: false, size_a, size_b, rows: Vec::new(), open: None, picking_origin: None, _subs: vec![sub] });
        } else if !active {
            self.datum_dlg = None;
        }
    }

    /// The label for a new datum (the dialog's, else the next free letter).
    pub fn datum_dialog_label(&self) -> String {
        self.datum_dlg.as_ref().map(|d| d.label_text.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| self.next_datum_label())
    }

    /// After a datum is placed: the label field moves to the next letter.
    pub fn datum_placed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next = self.next_datum_label();
        if let Some(d) = &mut self.datum_dlg {
            d.label_text = next.clone();
            d.label.update(cx, |s, cx| s.set_value(next, window, cx));
        }
    }

    /// Target area / size as the dialog sets them (meters).
    fn dialog_area(&self, cx: &App) -> DatumTargetArea {
        let Some(d) = &self.datum_dlg else { return DatumTargetArea::Point };
        let mm = self.drawing.units_mm;
        let len = |st: &Entity<InputState>| crate::drawing_section::parse_depth(&st.read(cx).value(), mm).unwrap_or(0.025).max(1e-4);
        match d.area {
            1 => DatumTargetArea::Dot,
            2 if d.area_size => DatumTargetArea::Circle { diameter: len(&d.size_a) },
            3 if d.area_size => DatumTargetArea::Rectangle { width: len(&d.size_a), height: len(&d.size_b) },
            2 => DatumTargetArea::Circle { diameter: 0.01 },
            3 => DatumTargetArea::Rectangle { width: 0.01, height: 0.01 },
            4 => DatumTargetArea::Custom,
            _ => DatumTargetArea::Point,
        }
    }

    /// A target picked and its callout placed (Datum target tab): add it to
    /// the list, expanded. True when the dialog took it.
    pub fn datum_target_add(&mut self, mut a: Annotation, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.datum_dlg.is_none() {
            return false;
        }
        let area = self.dialog_area(cx);
        let style = self.datum_dlg.as_ref().map(|d| d.style).unwrap_or_default();
        if let AnnotationKind::DatumTarget { area: ar, .. } = &mut a.kind {
            *ar = area;
        }
        a.target = (style != TargetStyle::Inside).then(|| Box::new(DatumTargetOpts { style, ..Default::default() }));
        let label = match &a.kind {
            AnnotationKind::DatumTarget { label, .. } => label.clone(),
            _ => String::new(),
        };
        let lab = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(label));
        let x = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false));
        let y = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false));
        let k = self.datum_dlg.as_ref().map(|d| d.rows.len()).unwrap_or(0);
        let mut subs = Vec::new();
        for st in [&lab, &x, &y] {
            subs.push(cx.subscribe(st, move |this: &mut Self, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.datum_row_from_inputs(k, cx);
                }
            }));
        }
        if let Some(d) = &mut self.datum_dlg {
            d.rows.push(TargetRow { ann: a, label: lab, x, y });
            d.open = Some(k);
            // Live: the new target's Origin field is active (blue) at once.
            d.picking_origin = Some(k);
            d._subs.extend(subs);
        }
        self.datum_preview(cx);
        true
    }

    /// Read a row's fields: the label, and the X / Y distances from the
    /// origin (which move the target).
    fn datum_row_from_inputs(&mut self, k: usize, cx: &mut Context<Self>) {
        let mm = self.drawing.units_mm;
        let Some(d) = &mut self.datum_dlg else { return };
        let Some(row) = d.rows.get_mut(k) else { return };
        let label = row.label.read(cx).value().trim().to_string();
        let dist = |st: &Entity<InputState>| crate::drawing_section::parse_depth(&st.read(cx).value(), mm);
        let (dx, dy) = (dist(&row.x), dist(&row.y));
        let opts = row.ann.target.as_deref().cloned().unwrap_or_default();
        if let AnnotationKind::DatumTarget { label: l, anchor, .. } = &mut row.ann.kind {
            *l = label;
            if let Some(o) = opts.origin {
                let sx = if opts.flip_x { -1.0 } else { 1.0 };
                let sy = if opts.flip_y { -1.0 } else { 1.0 };
                let moved = [dx.map(|v| o[0] + sx * v).unwrap_or(anchor[0]), dy.map(|v| o[1] + sy * v).unwrap_or(anchor[1])];
                if (moved[0] - anchor[0]).abs() > 1e-12 || (moved[1] - anchor[1]).abs() > 1e-12 {
                    *anchor = moved;
                    // Placed by its distances now, not on the picked point.
                    row.ann.refs.retain(|r| r.slot != 0);
                }
            }
        }
        self.datum_preview(cx);
    }

    /// A click while the dialog waits for a target's origin. True when used.
    pub fn datum_origin_click(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(k) = self.datum_dlg.as_ref().and_then(|d| d.picking_origin) else { return false };
        let Some(pp) = self.any_point(self.cursor) else { return true };
        let mm = self.drawing.units_mm;
        let u = noshape_model::drawing::format::length_units(&self.drawing, &Default::default());
        let Some(d) = &mut self.datum_dlg else { return true };
        let Some(row) = d.rows.get_mut(k) else { return true };
        let AnnotationKind::DatumTarget { anchor, view, .. } = &row.ann.kind else { return true };
        if *view != pp.view {
            self.status = Some("Select the origin in the target's view".into());
            return true;
        }
        let (ax, ay) = (anchor[0], anchor[1]);
        let opts = row.ann.target.get_or_insert_with(Default::default);
        opts.origin = Some(pp.p);
        opts.origin_ref = pp.r.clone();
        opts.flip_x = ax < pp.p[0];
        opts.flip_y = ay < pp.p[1];
        row.ann.refs.retain(|r| r.slot != 1);
        if let Some(r) = pp.r {
            row.ann.refs.push(AnnotationRef { slot: 1, point: r });
        }
        let fmt = |v: f64| format!("{} {}", noshape_model::drawing::format::format_number(v / u.meters(), Some(if mm { 2 } else { 3 }), 3), u.symbol());
        let (x, y) = (fmt((ax - pp.p[0]).abs()), fmt((ay - pp.p[1]).abs()));
        let (xs, ys) = (row.x.clone(), row.y.clone());
        d.picking_origin = None;
        xs.update(cx, |s, cx| s.set_value(x, window, cx));
        ys.update(cx, |s, cx| s.set_value(y, window, cx));
        self.datum_preview(cx);
        true
    }

    /// The drawing with the dialog's pending targets (None: no targets).
    pub fn datum_rows_drawing(&self) -> Option<noshape_model::drawing::Drawing> {
        let d = self.datum_dlg.as_ref().filter(|d| !d.rows.is_empty())?;
        let mut dr = self.drawing.clone();
        let first = dr.sheets[self.sheet].next_annotation_id();
        for (r, id) in d.rows.iter().zip(first..) {
            let mut a = r.ann.clone();
            a.id = id;
            dr.sheets[self.sheet].annotations.push(a);
        }
        Some(dr)
    }

    fn datum_preview(&mut self, cx: &mut Context<Self>) {
        let dr = self.datum_rows_drawing();
        self.set_preview(dr, cx);
        cx.notify();
    }

    pub fn close_datum_dlg(&mut self, accept: bool, cx: &mut Context<Self>) {
        let rows: Vec<Annotation> = self.datum_dlg.as_mut().map(|d| d.rows.drain(..).map(|r| r.ann).collect()).unwrap_or_default();
        self.set_preview(None, cx);
        if accept && !rows.is_empty() {
            self.edit_sheet("Datum target", cx, |s| {
                for mut a in rows {
                    a.id = s.next_annotation_id();
                    s.annotations.push(a);
                }
            });
        }
        self.tool = Tool::Select;
        self.datum_dlg = None;
        cx.notify();
    }

    fn edit_datum(&mut self, f: impl FnOnce(&mut DatumDlg), cx: &mut Context<Self>) {
        if let Some(d) = &mut self.datum_dlg {
            f(d);
        }
        // Area / style apply to the listed targets too.
        let area = self.dialog_area(cx);
        if let Some(d) = &mut self.datum_dlg {
            let style = d.style;
            for r in &mut d.rows {
                if let AnnotationKind::DatumTarget { area: a, .. } = &mut r.ann.kind {
                    *a = area.clone();
                }
                r.ann.target.get_or_insert_with(Default::default).style = style;
            }
        }
        self.datum_preview(cx);
    }

    pub fn render_datum_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.datum_dlg.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let ed = cx.entity();
        let blue = gpui_kit::rgb(0x1651b0);
        let tab = d.tab;
        let tabs = h_flex().border_b_1().border_color(gpui_kit::rgb(0xe0e0e0)).children([(0u8, "Datum"), (1, "Datum target")].into_iter().map(|(t, name)| {
            let on = t == tab;
            div()
                .id(("dt-tab", t as usize))
                .flex_1()
                .h(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.))
                .cursor_pointer()
                .when(on, |e| e.text_color(blue).border_b_2().border_color(blue))
                .child(name)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.datum_dlg {
                        d.tab = t;
                    }
                    this.tool = if t == 0 { Tool::Datum { anchor: None } } else { Tool::DatumTarget { anchor: None } };
                    cx.notify();
                }))
                .into_any_element()
        }));
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(36.)).px(px(8.)).gap(px(6.)).child(div().w(px(72.)).text_size(px(12.)).child(label)).child(body).into_any_element();
        type Act = std::rc::Rc<dyn Fn(&mut DatumDlg)>;
        let icon_menu = |id: &'static str, glyph: String, items: Vec<(String, String, bool, Act)>| -> AnyElement {
            let ed = ed.clone();
            Button::new(id).ghost().w(px(50.)).h(px(30.)).border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(4.)).dropdown_caret(true).child(div().text_size(px(16.)).child(glyph)).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                for (g, l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(format!("{g}  {l}")).checked(*on).on_click(move |_, _, cx| e.update(cx, |t, cx| t.edit_datum(|d| f(d), cx))));
                }
                m
            })
            .into_any_element()
        };
        let mut body = v_flex();
        if tab == 0 {
            body = body.child(h_flex().h(px(30.)).px(px(8.)).gap(px(6.)).child(div().text_size(px(12.)).child("Label")).child(div().flex_1().border_b_1().border_color(blue).child(Input::new(&d.label).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall())));
        } else {
            let areas: Vec<(String, String, bool, Act)> = AREAS.iter().enumerate().map(|(i, (n, g))| (g.to_string(), n.to_string(), d.area == i, std::rc::Rc::new(move |d: &mut DatumDlg| d.area = i) as Act)).collect();
            let styles: Vec<(String, String, bool, Act)> = TargetStyle::ALL
                .iter()
                .map(|&s| ((if s == TargetStyle::Inside { "⊖" } else { "⊲" }).to_string(), s.name().to_string(), d.style == s, std::rc::Rc::new(move |d: &mut DatumDlg| d.style = s) as Act))
                .collect();
            let area_size = d.area_size;
            let sized = matches!(d.area, 2 | 3);
            body = body
                .child(row("Target area:", icon_menu("dt-area", AREAS[d.area].1.to_string(), areas)))
                .child(row("Style:", icon_menu("dt-style", if d.style == TargetStyle::Inside { "⊖".into() } else { "⊲".into() }, styles)))
                .child(
                    h_flex()
                        .h(px(30.))
                        .px(px(8.))
                        .gap(px(6.))
                        .child(crate::drawing_chrome::DrCheckbox::new("dt-size").label("Area size:").checked(area_size).on_click(cx.listener(|this, v: &bool, _, cx| {
                            let v = *v;
                            this.edit_datum(|d| d.area_size = v, cx)
                        })))
                        .when(area_size && sized, |e| e.child(div().text_size(px(13.)).child(if d.area == 2 { "Ø" } else { "" })).child(div().w(px(80.)).border_b_1().border_color(gpui_kit::rgb(0xcccccc)).child(Input::new(&d.size_a).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall())))
                        .when(area_size && d.area == 3, |e| e.child(div().w(px(80.)).border_b_1().border_color(gpui_kit::rgb(0xcccccc)).child(Input::new(&d.size_b).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall()))),
                );
            // The Targets list.
            let mut list = v_flex()
                .mx(px(4.))
                .my(px(3.))
                .px(px(6.))
                .py(px(4.))
                .rounded(px(2.))
                .border_1()
                .border_color(gpui_kit::rgb(0x5da7dd))
                .when(d.rows.is_empty(), |e| e.bg(gpui_kit::rgb(0xd9eef9)))
                .child(div().text_size(px(if d.rows.is_empty() { 12. } else { 10. })).text_color(gpui_kit::rgb(0x5c6670)).child("Targets"));
            for (k, r) in d.rows.iter().enumerate() {
                let open = d.open == Some(k);
                let name = match &r.ann.kind {
                    AnnotationKind::DatumTarget { label, .. } => label.clone(),
                    _ => String::new(),
                };
                list = list.child(
                    h_flex()
                        .id(("dt-row", k))
                        .h(px(22.))
                        .gap(px(4.))
                        .cursor_pointer()
                        .child(crate::ui::caret(open))
                        .child(div().flex_1().text_size(px(13.)).child(crate::ui::user_text(name)))
                        .child(div().id(("dt-row-x", k)).px(px(4.)).text_color(gpui_kit::rgb(0x9a9a9a)).child("×").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.edit_datum(
                                |d| {
                                    if k < d.rows.len() {
                                        d.rows.remove(k);
                                        d.open = None;
                                    }
                                },
                                cx,
                            )
                        })))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(d) = &mut this.datum_dlg {
                                d.open = if d.open == Some(k) { None } else { Some(k) };
                            }
                            cx.notify();
                        })),
                );
                if open {
                    let opts = r.ann.target.as_deref().cloned().unwrap_or_default();
                    let picking = d.picking_origin == Some(k);
                    let modes: Vec<(String, bool, bool)> = vec![("Length".into(), !opts.angular, false), ("Angle".into(), opts.angular, true)];
                    let mode_btn = {
                        let ed = ed.clone();
                        Button::new(("dt-mode", k)).ghost().xsmall().w_full().dropdown_caret(true).label(if opts.angular { "Angle" } else { "Length" }).os_dropdown_menu(SharedString::from(format!("dt-mode-dd-{k}")), true, move |mut m, _, _| {
                            for (l, on, v) in &modes {
                                let (e, v) = (ed.clone(), *v);
                                m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| {
                                    e.update(cx, |t, cx| {
                                        if let Some(d) = &mut t.datum_dlg
                                            && let Some(r) = d.rows.get_mut(k)
                                        {
                                            r.ann.target.get_or_insert_with(Default::default).angular = v;
                                        }
                                        t.datum_preview(cx);
                                    })
                                }));
                            }
                            m
                        })
                    };
                    let field_box = |id: (&'static str, usize), caption: &'static str, value: Option<String>, active: bool| {
                        div()
                            .id(id)
                            .my(px(2.))
                            .px(px(6.))
                            .py(px(3.))
                            .min_h(px(24.))
                            .rounded(px(2.))
                            .border_1()
                            .border_color(gpui_kit::rgb(if active { 0x5da7dd } else { 0xdddddd }))
                            .when(active, |e| e.bg(gpui_kit::rgb(0xd9eef9)))
                            .cursor_pointer()
                            .child(div().text_size(px(if value.is_some() { 10. } else { 11. })).text_color(gpui_kit::rgb(0x8a8a8a)).child(caption))
                            .children(value.map(|v| div().text_size(px(13.)).child(v)))
                    };
                    let flip = |id: (&'static str, usize), label: &'static str, on: bool, x: bool, cx: &mut Context<Self>| {
                        v_flex().items_center().child(div().text_size(px(10.)).child(label)).child(
                            div().id(id).size(px(20.)).flex().items_center().justify_center().rounded(px(2.)).border_1().border_color(gpui_kit::rgb(0xcccccc)).when(on, |e| e.bg(gpui_kit::rgb(0xdfe7ee))).cursor_pointer().child(div().text_size(px(11.)).child("⤢")).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(d) = &mut this.datum_dlg
                                    && let Some(r) = d.rows.get_mut(k)
                                {
                                    let o = r.ann.target.get_or_insert_with(Default::default);
                                    if x {
                                        o.flip_x = !o.flip_x;
                                    } else {
                                        o.flip_y = !o.flip_y;
                                    }
                                }
                                this.datum_row_from_inputs(k, cx);
                            })),
                        )
                    };
                    list = list.child(
                        v_flex()
                            .pl(px(10.))
                            .child(h_flex().h(px(26.)).gap(px(6.)).child(div().text_size(px(12.)).child("Label")).child(div().flex_1().border_b_1().border_color(blue).child(Input::new(&r.label).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall())))
                            .child(div().border_b_1().border_color(gpui_kit::rgb(0xcccccc)).child(mode_btn))
                            .child(field_box(("dt-origin", k), "Origin", opts.origin.map(|_| "Point".to_string()), picking).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(d) = &mut this.datum_dlg {
                                    d.picking_origin = Some(k);
                                }
                                cx.notify();
                            })))
                            .child(field_box(("dt-loc", k), "Target location", Some("Point".into()), false))
                            .child(h_flex().gap(px(4.)).child(crate::ui::caret(true)).child(div().text_size(px(12.)).child("X position")))
                            .child(field_box(("dt-xdir", k), "X direction", None, false))
                            .child(h_flex().h(px(26.)).gap(px(6.)).child(div().text_size(px(12.)).child("Distance:")).child(div().flex_1().border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(2.)).px(px(4.)).child(Input::new(&r.x).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall())))
                            .child(h_flex().gap(px(4.)).child(crate::ui::caret(true)).child(div().text_size(px(12.)).child("Y position")))
                            .child(field_box(("dt-ydir", k), "Y direction", None, false))
                            .child(h_flex().h(px(26.)).gap(px(6.)).child(div().text_size(px(12.)).child("Distance:")).child(div().flex_1().border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(2.)).px(px(4.)).child(Input::new(&r.y).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall())))
                            .child(h_flex().gap(px(8.)).pt(px(2.)).child(flip(("dt-flipx", k), "X", opts.flip_x, true, cx)).child(flip(("dt-flipy", k), "Y", opts.flip_y, false, cx))),
                    );
                }
            }
            body = body.child(list);
        }
        let can_accept = tab == 1 && !d.rows.is_empty();
        Some(
            v_flex()
                .id("dr-datum-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(265.))
                .pb(px(6.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(ink)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(30.))
                        .pl(px(11.))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Datum"))
                        .child(div().id("dt-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(if can_accept { pal.accept } else { gpui_kit::rgb(crate::drawing_chrome::OK_DISABLED) }).cursor_pointer().child(crate::ui::glyph("check", 18., gpui_kit::white())).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if can_accept {
                                this.close_datum_dlg(true, cx)
                            }
                        })))
                        .child(div().id("dt-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_datum_dlg(false, cx)))),
                )
                .child(tabs)
                .child(div().pt(px(4.)).child(body))
                .into_any_element(),
        )
    }
}

/// A selected datum's fill (live drawing/1073: rgb 204,146,92).
const DATUM_SEL: [u8; 4] = [0xcc, 0x92, 0x5c, 255];
/// Its attached edge and anchor dot (live drawing/1073: rgb 144,206,241).
const DATUM_EDGE: [u8; 4] = [0x90, 0xce, 0xf1, 255];

impl DrawingEditor {
    /// The selected datums: annotation id, its pick ids and its anchor.
    fn selected_datums(&self) -> Vec<(u64, Vec<u32>, [f64; 2])> {
        let Some(b) = &self.built else { return vec![] };
        self.selection
            .iter()
            .filter_map(|s| {
                let Sel::Annotation(id) = s else { return None };
                let a = b.resolved.annotations.iter().find(|a| a.id == *id)?;
                let AnnotationKind::Datum { anchor, .. } = &a.kind else { return None };
                Some((*id, self.picks_of(*s), *anchor))
            })
            .collect()
    }

    /// Pick ids drawn by [`Self::push_datum_selection`] instead of the
    /// plain selection tint.
    pub fn datum_selection_picks(&self) -> Vec<u32> {
        self.selected_datums().into_iter().flat_map(|(_, p, _)| p).collect()
    }

    /// A selected datum's look (live drawing/1073): the box, stem and
    /// triangle filled orange with the letter black on it and a thin white
    /// bar inside each side of the box; the edge it sits on light blue with
    /// a light blue dot (white rim) at the anchor. Hovering changes nothing.
    pub fn push_datum_selection(&self, scene: &mut noshape_render::scene::Scene) {
        use noshape_model::drawing::views::dist_to_segment;
        use noshape_model::drawing::build::PickTarget;
        use noshape_render::{Drawable, Instance, LineData, LineSegment, MeshData, PointData, PointSprite};
        let Some(b) = &self.built else { return };
        let datums = self.selected_datums();
        if datums.is_empty() {
            return;
        }
        let g = &b.geometry;
        let px = 1.0 / self.zoom.max(1e-6);
        let mut fill = MeshData::default();
        let quad = |m: &mut MeshData, c: [[f64; 2]; 4], z: f32| {
            let base = m.positions.len() as u32;
            for p in c {
                m.positions.push([p[0] as f32, p[1] as f32, z]);
                m.normals.push([0.0, 0.0, 1.0]);
                m.pick_ids.push(0);
            }
            m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        };
        let mut lines = LineData::default();
        let mut white = LineData::default();
        let mut edges = LineData::default();
        let mut dots = PointData::default();
        for (_, picks, anchor) in &datums {
            let own: Vec<_> = g.lines.iter().filter(|l| picks.contains(&l.pick)).collect();
            for l in &own {
                let width = crate::drawing_scene::px_width(l.style, self.zoom).max(1.5);
                for w in l.points.windows(2) {
                    lines.segments.push(LineSegment { a: [w[0][0] as f32, w[0][1] as f32, 0.4], b: [w[1][0] as f32, w[1][1] as f32, 0.4], color: DATUM_SEL, pick_id: 0, width, style: LineSegment::ON_TOP });
                }
            }
            // The box: the datum's closed outline.
            let frame = own.iter().filter(|l| l.closed || (l.points.len() >= 5 && l.points.first() == l.points.last())).max_by(|a, c| bbox_area(&a.points).total_cmp(&bbox_area(&c.points)));
            let mut reach = 4.0;
            if let Some(f) = frame {
                let [x0, y0, x1, y1] = bbox(&f.points);
                quad(&mut fill, [[x0, y0], [x1, y0], [x1, y1], [x0, y1]], -0.5);
                let inset = 3.0 * px;
                for x in [x0 + inset, x1 - inset] {
                    white.segments.push(LineSegment { a: [x as f32, (y0 + inset) as f32, 0.45], b: [x as f32, (y1 - inset) as f32, 0.45], color: [255, 255, 255, 255], pick_id: 0, width: 1.0, style: LineSegment::ON_TOP });
                }
                reach = (x1 - x0).max(y1 - y0);
            }
            // The triangle: the filled triangle touching the anchor.
            for f in g.fills.iter().filter(|f| f.points.len() == 3 && f.points.iter().any(|p| (p[0] - anchor[0]).hypot(p[1] - anchor[1]) < reach)) {
                let tri = &f.points;
                if tri.iter().any(|p| (p[0] - anchor[0]).hypot(p[1] - anchor[1]) < reach * 0.6) {
                    quad(&mut fill, [tri[0], tri[1], tri[2], tri[2]], 0.35);
                }
            }
            // The view edge through the anchor.
            for l in g.lines.iter().filter(|l| matches!(b.picks.get(l.pick as usize), Some(PickTarget::Edge { .. }))) {
                if l.points.windows(2).any(|w| dist_to_segment(*anchor, w[0], w[1]) < 0.05) {
                    let width = crate::drawing_scene::px_width(l.style, self.zoom).max(3.0);
                    for w in l.points.windows(2) {
                        edges.segments.push(LineSegment { a: [w[0][0] as f32, w[0][1] as f32, 0.3], b: [w[1][0] as f32, w[1][1] as f32, 0.3], color: DATUM_EDGE, pick_id: 0, width, style: LineSegment::ON_TOP });
                    }
                }
            }
            let at = [anchor[0] as f32, anchor[1] as f32, 0.7];
            dots.points.push(PointSprite { position: at, color: [255, 255, 255, 255], pick_id: 0, size: 13.0, style: LineSegment::ON_TOP, _pad: 0 });
            dots.points.push(PointSprite { position: [at[0], at[1], 0.71], color: DATUM_EDGE, pick_id: 0, size: 11.0, style: LineSegment::ON_TOP, _pad: 0 });
        }
        let up = self.gpu.uploader();
        let color = crate::theme::lin(0xcc925c);
        scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&edges))).per_primitive_color().unpickable().layer(17));
        if !fill.indices.is_empty() {
            scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(&fill))).with_color(color).unlit().unpickable().layer(-4));
        }
        scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(18));
        scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&white))).per_primitive_color().unpickable().layer(19));
        scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&dots))).per_primitive_color().unpickable().layer(22));
    }
}

fn bbox(p: &[[f64; 2]]) -> [f64; 4] {
    p.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])])
}

fn bbox_area(p: &[[f64; 2]]) -> f64 {
    let b = bbox(p);
    (b[2] - b[0]) * (b[3] - b[1])
}
