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
    /// Length: the X / Y distances; Angle: Diameter distance and Angle.
    pub x: Entity<InputState>,
    pub y: Entity<InputState>,
    /// Angle: Instance count.
    pub count: Entity<InputState>,
    /// The direction came from the Y direction field (X is hidden then).
    pub dir_y: bool,
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
    /// Area size's precision ▾ (None: the drawing's; help datum, drawing.md A32).
    pub size_precision: Option<u8>,
    pub rows: Vec<TargetRow>,
    /// The expanded target.
    pub open: Option<usize>,
    /// The target whose Origin field takes the next click.
    pub picking_origin: Option<usize>,
    /// The target whose X (false) / Y (true) direction takes the next click.
    pub picking_dir: Option<(usize, bool)>,
    /// The target whose Reference line (Angle) takes the next click.
    pub picking_ref: Option<usize>,
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
            self.datum_dlg = Some(DatumDlg { tab, label, label_text, area: 0, style: TargetStyle::Inside, area_size: false, size_a, size_b, size_precision: None, rows: Vec::new(), open: None, picking_origin: None, picking_dir: None, picking_ref: None, _subs: vec![sub] });
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
            // Live request 1735: a circle always has its area size.
            2 => DatumTargetArea::Circle { diameter: len(&d.size_a) },
            3 if d.area_size => DatumTargetArea::Rectangle { width: len(&d.size_a), height: len(&d.size_b) },
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
        let custom = area == DatumTargetArea::Custom;
        let style = self.datum_dlg.as_ref().map(|d| d.style).unwrap_or_default();
        if let AnnotationKind::DatumTarget { area: ar, .. } = &mut a.kind {
            *ar = area;
        }
        a.target = (style != TargetStyle::Inside).then(|| Box::new(DatumTargetOpts { style, ..Default::default() }));
        // Custom (live 1699 (e)): the closed region of the view around the
        // pick; the leader ends on its edge.
        if custom
            && let AnnotationKind::DatumTarget { view, anchor, .. } = &mut a.kind
            && let Some(pl) = self.placement(*view)
            && let Some((outer, _)) = self.pk().view_region_at(*view, pl.to_sheet(*anchor))
            && outer.len() > 2
        {
            let n = outer.len();
            let p = *anchor;
            if let Some(q) = (0..n).map(|i| foot(p, outer[i], outer[(i + 1) % n])).min_by(|x, y| dist2(*x, p).total_cmp(&dist2(*y, p))) {
                *anchor = q;
            }
            a.refs.retain(|r| r.slot != 0);
            a.target.get_or_insert_with(Default::default).region = outer;
        }
        // The size's precision (Area size ▾).
        a.format.precision = self.datum_dlg.as_ref().and_then(|d| d.size_precision);
        let label = match &a.kind {
            AnnotationKind::DatumTarget { label, .. } => label.clone(),
            _ => String::new(),
        };
        let lab = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(label));
        let x = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false));
        let y = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false));
        let count = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value("1"));
        let k = self.datum_dlg.as_ref().map(|d| d.rows.len()).unwrap_or(0);
        let mut subs = Vec::new();
        for st in [&lab, &x, &y, &count] {
            subs.push(cx.subscribe(st, move |this: &mut Self, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.datum_row_from_inputs(k, cx);
                }
            }));
        }
        if let Some(d) = &mut self.datum_dlg {
            d.rows.push(TargetRow { ann: a, label: lab, x, y, count, dir_y: false });
            d.open = Some(k);
            // Live: the new target's Origin field is active (blue) at once
            // (a Custom area has none).
            d.picking_origin = (!custom).then_some(k);
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
        // Angle: Diameter distance, Angle (deg), Instance count.
        let angle = row.y.read(cx).value().trim().trim_end_matches("deg").trim_end_matches('°').trim().parse::<f64>().ok();
        if let Ok(n) = row.count.read(cx).value().trim().parse::<u32>() {
            row.ann.target.get_or_insert_with(Default::default).instances = n.clamp(1, 64);
        }
        let opts = row.ann.target.as_deref().cloned().unwrap_or_default();
        if let AnnotationKind::DatumTarget { label: l, anchor, .. } = &mut row.ann.kind {
            *l = label;
            if let (Some(o), true) = (opts.origin, opts.angular) {
                // On the circle of that diameter round the origin, at that
                // angle from the reference (live 1699).
                if let (Some(dd), Some(t)) = (dx, angle) {
                    let t = t.to_radians();
                    let t = if opts.angle_rotate { std::f64::consts::PI - t } else { t };
                    let t = if opts.angle_flip { -t } else { t };
                    let r = opts.reference();
                    let (sn, cs) = t.sin_cos();
                    let moved = [o[0] + dd / 2.0 * (r[0] * cs - r[1] * sn), o[1] + dd / 2.0 * (r[0] * sn + r[1] * cs)];
                    if (moved[0] - anchor[0]).abs() > 1e-12 || (moved[1] - anchor[1]).abs() > 1e-12 {
                        *anchor = moved;
                        row.ann.refs.retain(|r| r.slot != 0);
                    }
                }
            } else if let Some(o) = opts.origin {
                // Along the X direction and across it, each flipped to the
                // other side of the origin by its button (live 1699).
                let (u, n) = opts.axes();
                let sx = if opts.flip_x { -1.0 } else { 1.0 };
                let sy = if opts.flip_y { -1.0 } else { 1.0 };
                let rel = [anchor[0] - o[0], anchor[1] - o[1]];
                let (x, y) = (dx.map(|v| sx * v).unwrap_or(rel[0] * u[0] + rel[1] * u[1]), dy.map(|v| sy * v).unwrap_or(rel[0] * n[0] + rel[1] * n[1]));
                let moved = [o[0] + u[0] * x + n[0] * y, o[1] + u[1] * x + n[1] * y];
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
        let Some(mut pp) = self.any_point(self.cursor) else { return true };
        // On a circle's center: that circle's reference (Angle needs an arc
        // or circle center; the snap may be a centermark's point).
        if !matches!(pp.r, Some(noshape_model::drawing::PointRef::Center { .. }))
            && let Some(r) = self.geoms.get(&pp.view).and_then(|g| g.circles().into_iter().find(|(c, _, r)| r.is_some() && (c[0] - pp.p[0]).hypot(c[1] - pp.p[1]) < 1e-9).and_then(|x| x.2))
        {
            pp.r = Some(r);
        }
        let Some(d) = &mut self.datum_dlg else { return true };
        let Some(row) = d.rows.get_mut(k) else { return true };
        let AnnotationKind::DatumTarget { view, .. } = &row.ann.kind else { return true };
        if *view != pp.view {
            self.status = Some("Select the origin in the target's view".into());
            return true;
        }
        let opts = row.ann.target.get_or_insert_with(Default::default);
        opts.origin = Some(pp.p);
        opts.origin_ref = pp.r.clone();
        row.ann.refs.retain(|r| r.slot != 1);
        if let Some(r) = pp.r {
            row.ann.refs.push(AnnotationRef { slot: 1, point: r });
        }
        d.picking_origin = None;
        self.datum_fill_distances(k, window, cx);
        true
    }

    /// A target's X / Y distance fields (and flips) from where it is
    /// relative to its origin, along its X direction and across it.
    fn datum_fill_distances(&mut self, k: usize, window: &mut Window, cx: &mut Context<Self>) {
        let mm = self.drawing.units_mm;
        let u = noshape_model::drawing::format::length_units(&self.drawing, &Default::default());
        let Some(row) = self.datum_dlg.as_mut().and_then(|d| d.rows.get_mut(k)) else { return };
        let AnnotationKind::DatumTarget { anchor, .. } = &row.ann.kind else { return };
        let anchor = *anchor;
        let opts = row.ann.target.get_or_insert_with(Default::default);
        let Some(o) = opts.origin else { return };
        let rel = [anchor[0] - o[0], anchor[1] - o[1]];
        let fmt = |v: f64| format!("{} {}", noshape_model::drawing::format::format_number(v / u.meters(), Some(if mm { 2 } else { 3 }), 3), u.symbol());
        let (x, y) = if opts.angular {
            // Angle: the diameter through the target and its angle from the
            // reference (live 1699: 50.00 mm, 36.9 deg); below the
            // reference it is measured the other way round (Flip).
            let r = opts.reference();
            opts.angle_flip = r[0] * rel[1] - r[1] * rel[0] < -1e-12;
            (fmt(2.0 * rel[0].hypot(rel[1])), format!("{:.1} deg", opts.measured_angle(anchor, o).to_degrees()))
        } else {
            let (ux, uy) = opts.axes();
            let (a, b) = (rel[0] * ux[0] + rel[1] * ux[1], rel[0] * uy[0] + rel[1] * uy[1]);
            opts.flip_x = a < -1e-12;
            opts.flip_y = b < -1e-12;
            (fmt(a.abs()), fmt(b.abs()))
        };
        let (xs, ys) = (row.x.clone(), row.y.clone());
        xs.update(cx, |s, cx| s.set_value(x, window, cx));
        ys.update(cx, |s, cx| s.set_value(y, window, cx));
        self.datum_preview(cx);
    }

    /// A click while a target's X / Y direction field waits for an edge: a
    /// straight edge of its view sets the X direction (a Y edge: X a
    /// quarter turn clockwise from it, as the hole table's; live 1699).
    /// True when used.
    pub fn datum_dir_click(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(k) = self.datum_dlg.as_ref().and_then(|d| d.picking_ref) {
            // Angle's Reference line: an edge the angle is measured from.
            let view = match self.datum_dlg.as_ref().and_then(|d| d.rows.get(k)).map(|r| &r.ann.kind) {
                Some(AnnotationKind::DatumTarget { view, .. }) => *view,
                _ => return true,
            };
            let Some(dir) = self.straight_edge_dir_near(view) else { return true };
            if let Some(d) = &mut self.datum_dlg
                && let Some(row) = d.rows.get_mut(k)
            {
                row.ann.target.get_or_insert_with(Default::default).ref_dir = Some(dir);
                d.picking_ref = None;
            }
            self.datum_fill_distances(k, window, cx);
            return true;
        }
        let Some((k, is_y)) = self.datum_dlg.as_ref().and_then(|d| d.picking_dir) else { return false };
        let view = match self.datum_dlg.as_ref().and_then(|d| d.rows.get(k)).map(|r| &r.ann.kind) {
            Some(AnnotationKind::DatumTarget { view, .. }) => *view,
            _ => return true,
        };
        let Some(dir) = self.straight_edge_dir_near(view) else { return true };
        if let Some(d) = &mut self.datum_dlg
            && let Some(row) = d.rows.get_mut(k)
        {
            row.ann.target.get_or_insert_with(Default::default).x_dir = Some(if is_y { [dir[1], -dir[0]] } else { dir });
            row.dir_y = is_y;
            d.picking_dir = None;
        }
        self.datum_fill_distances(k, window, cx);
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
            // While editing, its position dimensions show (live 1699);
            // placed, it has none until Show dimensions (A35).
            if let Some(t) = a.target.as_mut() {
                t.hide_dims = false;
            }
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
            // Live (dom drawing-datum*): an emptied label is an error, "This
            // field cannot be empty".
            let empty = d.label_text.is_empty();
            body = body.child(
                h_flex().h(px(30.)).px(px(8.)).gap(px(6.)).child(div().text_size(px(12.)).child("Label")).child(
                    div()
                        .id("dt-label")
                        .flex_1()
                        .border_b_1()
                        .border_color(if empty { gpui_kit::rgb(0xd9534f) } else { blue })
                        .when(empty, |e| e.tooltip(crate::ui::tooltip("This field cannot be empty")))
                        .child(Input::new(&d.label).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall()),
                ),
            );
        } else {
            let areas: Vec<(String, String, bool, Act)> = AREAS.iter().enumerate().map(|(i, (n, g))| (g.to_string(), n.to_string(), d.area == i, std::rc::Rc::new(move |d: &mut DatumDlg| d.area = i) as Act)).collect();
            let styles: Vec<(String, String, bool, Act)> = TargetStyle::ALL
                .iter()
                .map(|&s| (style_glyph(s).to_string(), s.name().to_string(), d.style == s, std::rc::Rc::new(move |d: &mut DatumDlg| d.style = s) as Act))
                .collect();
            // Live request 1735: Circle's "Area size: Ø [25 mm]" has no
            // checkbox (always on); Point's and Rectangle's do.
            let area_size = d.area_size || d.area == 2;
            let sized = matches!(d.area, 2 | 3);
            body = body
                .child(row("Target area:", icon_menu("dt-area", AREAS[d.area].1.to_string(), areas)))
                .child(row("Style:", div().id("dt-style-tip").tooltip(crate::ui::tooltip("Datum style")).child(icon_menu("dt-style", style_glyph(d.style).into(), styles)).into_any_element()))
                .child(
                    h_flex()
                        .h(px(30.))
                        .px(px(8.))
                        .gap(px(6.))
                        .when(d.area == 2, |e| e.child(div().text_size(px(12.)).child("Area size:")))
                        .when(d.area != 2, |e| {
                            e.child(crate::drawing_chrome::DrCheckbox::new("dt-size").label("Area size:").checked(area_size).on_click(cx.listener(|this, v: &bool, _, cx| {
                                let v = *v;
                                this.edit_datum(|d| d.area_size = v, cx)
                            })))
                        })
                        .when(area_size && sized, |e| e.child(div().text_size(px(13.)).child(if d.area == 2 { "Ø" } else { "" })).child(div().w(px(80.)).border_b_1().border_color(gpui_kit::rgb(0xcccccc)).child(Input::new(&d.size_a).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall())))
                        .when(area_size && d.area == 3, |e| e.child(div().w(px(80.)).border_b_1().border_color(gpui_kit::rgb(0xcccccc)).child(Input::new(&d.size_b).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall())))
                        .when(area_size && sized, |e| {
                            // Its precision ▾ (help datum: "Area size … with precision").
                            const PRECISIONS: [(&str, Option<u8>); 8] = [("0", Some(0)), ("0.1", Some(1)), ("0.12", Some(2)), ("0.123", Some(3)), ("(Drawing)", None), ("0.1234", Some(4)), ("0.12345", Some(5)), ("0.123456", Some(6))];
                            let cur = d.size_precision;
                            let items: Vec<(String, String, bool, Act)> = PRECISIONS.iter().map(|&(l, p)| (String::new(), l.to_string(), cur == p, std::rc::Rc::new(move |d: &mut DatumDlg| d.size_precision = p) as Act)).collect();
                            let label = PRECISIONS.iter().find(|x| x.1 == cur).map(|x| x.0).unwrap_or("(Drawing)").to_string();
                            let ed = ed.clone();
                            e.child(Button::new("dt-size-prec").ghost().w(px(96.)).h(px(30.)).border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(4.)).dropdown_caret(true).child(div().text_size(px(12.)).child(label)).os_dropdown_menu("dt-size-prec-dd", true, move |mut m, _, _| {
                                for (_, l, on, f) in &items {
                                    let (e, f) = (ed.clone(), f.clone());
                                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| e.update(cx, |t, cx| t.edit_datum(|d| f(d), cx))));
                                }
                                m
                            }))
                        }),
                );
            // The Targets list.
            let mut list = v_flex()
                .id("dt-targets")
                .when(d.rows.is_empty(), |e| e.tooltip(crate::ui::tooltip("Click in space to add a target")))
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
                // Instances: "A1-A4" (live 1699).
                let n = r.ann.target.as_deref().filter(|o| o.angular).map_or(1, |o| o.instances.max(1)) as usize;
                let name = match &r.ann.kind {
                    AnnotationKind::DatumTarget { label, .. } if n > 1 => format!("{label}-{}", DatumTargetOpts::instance_label(label, n - 1)),
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
                        .child(div().flex_1().text_size(px(13.)).child(crate::ui::user_text(name.clone())))
                        .child(div().id(("dt-row-x", k)).tooltip(crate::ui::tooltip(format!("Delete {name}"))).px(px(4.)).text_color(gpui_kit::rgb(0x9a9a9a)).child("×").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
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
                                m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, window, cx| {
                                    e.update(cx, |t, cx| {
                                        if let Some(d) = &mut t.datum_dlg
                                            && let Some(r) = d.rows.get_mut(k)
                                        {
                                            r.ann.target.get_or_insert_with(Default::default).angular = v;
                                        }
                                        t.datum_fill_distances(k, window, cx);
                                    })
                                }));
                            }
                            m
                        })
                    };
                    let field_box = |id: (&'static str, usize), caption: &'static str, value: Option<String>, active: bool| {
                        // Live dom/drawing-datum-target-after: the fields' tooltips.
                        let tip = match caption {
                            "Origin" => "Select an origin location for datum target",
                            "Target location" => "Select target location for datum target",
                            "X direction" => "Select an edge to define the X direction for this datum target",
                            _ => "Select an edge to define the Y direction for this datum target",
                        };
                        div()
                            .id(id)
                            .tooltip(crate::ui::tooltip(tip))
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
                    let label_row = h_flex().h(px(26.)).gap(px(6.)).child(div().text_size(px(12.)).child("Label")).child(div().flex_1().border_b_1().border_color(blue).child(Input::new(&r.label).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall()));
                    // A Custom area's row is its Label only: no origin, no
                    // positions (live 1699 (e)).
                    if matches!(r.ann.kind, AnnotationKind::DatumTarget { area: DatumTargetArea::Custom, .. }) {
                        list = list.child(v_flex().pl(px(10.)).child(label_row));
                        continue;
                    }
                    list = list.child(
                        v_flex()
                            .pl(px(10.))
                            .child(label_row)
                            .child(div().border_b_1().border_color(gpui_kit::rgb(0xcccccc)).child(mode_btn))
                            .child(field_box(("dt-origin", k), "Origin", opts.origin.map(|_| "Point".to_string()), picking).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(d) = &mut this.datum_dlg {
                                    d.picking_origin = Some(k);
                                    d.picking_dir = None;
                                }
                                cx.notify();
                            })))
                            .child(field_box(("dt-loc", k), "Target location", Some("Point".into()), false))
                            // Length: X / Y positions; Angle: Diameter distance,
                            // Radial lines, Reference line, Angle, Instance count,
                            // Flip and Rotate (live 1699).
                            .when(!opts.angular, |e| {
                                e
                                .child(h_flex().gap(px(4.)).child(crate::ui::caret(true)).child(div().text_size(px(12.)).child("X position")))
                                .when(opts.x_dir.is_none() || !r.dir_y, |e| {
                                    e.child(field_box(("dt-xdir", k), "X direction", opts.x_dir.filter(|_| !r.dir_y).map(|_| "Edge".to_string()), d.picking_dir == Some((k, false))).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        if let Some(d) = &mut this.datum_dlg {
                                            d.picking_dir = Some((k, false));
                                            d.picking_origin = None;
                                        }
                                        cx.notify();
                                    })))
                                })
                                .child(h_flex().h(px(26.)).gap(px(6.)).child(div().text_size(px(12.)).child("Distance:")).child(div().flex_1().border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(2.)).px(px(4.)).child(Input::new(&r.x).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall())))
                                .child(h_flex().gap(px(4.)).child(crate::ui::caret(true)).child(div().text_size(px(12.)).child("Y position")))
                                // Setting one direction hides the other (live 1699, as
                                // the hole table's).
                                .when(opts.x_dir.is_none() || r.dir_y, |e| {
                                    e.child(field_box(("dt-ydir", k), "Y direction", opts.x_dir.filter(|_| r.dir_y).map(|_| "Edge".to_string()), d.picking_dir == Some((k, true))).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        if let Some(d) = &mut this.datum_dlg {
                                            d.picking_dir = Some((k, true));
                                            d.picking_origin = None;
                                        }
                                        cx.notify();
                                    })))
                                })
                                .child(h_flex().h(px(26.)).gap(px(6.)).child(div().text_size(px(12.)).child("Distance:")).child(div().flex_1().border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(2.)).px(px(4.)).child(Input::new(&r.y).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall())))
                                .child(h_flex().gap(px(8.)).pt(px(2.)).child(flip(("dt-flipx", k), "X", opts.flip_x, true, cx)).child(flip(("dt-flipy", k), "Y", opts.flip_y, false, cx)))
                            })
                            .when(opts.angular, |e| {
                                let invalid = opts.origin.is_some() && !matches!(opts.origin_ref, Some(noshape_model::drawing::PointRef::Center { .. }));
                                let input_row = |label: &'static str, st: &Entity<InputState>| h_flex().h(px(26.)).gap(px(6.)).child(div().text_size(px(12.)).child(label)).child(div().flex_1().border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(2.)).px(px(4.)).child(Input::new(st).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall()));
                                let toggle = |id: (&'static str, usize), label: &'static str, on: bool| div().id(id).tooltip(crate::ui::tooltip(if label == "Flip" { "Flip measurement from reference line" } else { "Rotate angle measurement" })).px(px(6.)).h(px(22.)).flex().items_center().rounded(px(2.)).border_1().border_color(gpui_kit::rgb(0xcccccc)).when(on, |e| e.bg(gpui_kit::rgb(0xdfe7ee))).cursor_pointer().text_size(px(12.)).child(label);
                                e.when(invalid, |e| e.child(div().text_size(px(11.)).text_color(gpui_kit::rgb(0xd0021b)).child("Invalid origin selected. Angle datum target origin must be an arc or circle center.")))
                                    .child(input_row("Diameter distance:", &r.x))
                                    .child(h_flex().h(px(26.)).child(crate::drawing_chrome::DrCheckbox::new(("dt-radial", k)).label("Radial lines").checked(opts.radial_lines).on_click(cx.listener(move |this, v: &bool, _, cx| {
                                        let v = *v;
                                        if let Some(r) = this.datum_dlg.as_mut().and_then(|d| d.rows.get_mut(k)) {
                                            r.ann.target.get_or_insert_with(Default::default).radial_lines = v;
                                        }
                                        this.datum_preview(cx);
                                    }))))
                                    .child(field_box(("dt-ref", k), "Reference line", opts.ref_dir.map(|_| "Edge".to_string()), d.picking_ref == Some(k)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        if let Some(d) = &mut this.datum_dlg {
                                            d.picking_ref = Some(k);
                                            (d.picking_origin, d.picking_dir) = (None, None);
                                        }
                                        cx.notify();
                                    })))
                                    .child(input_row("Angle:", &r.y))
                                    .child(input_row("Instance count:", &r.count))
                                    .child(
                                        h_flex()
                                            .gap(px(6.))
                                            .pt(px(2.))
                                            .child(toggle(("dt-aflip", k), "Flip", opts.angle_flip).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                if let Some(r) = this.datum_dlg.as_mut().and_then(|d| d.rows.get_mut(k)) {
                                                    let o = r.ann.target.get_or_insert_with(Default::default);
                                                    o.angle_flip = !o.angle_flip;
                                                }
                                                this.datum_row_from_inputs(k, cx);
                                            })))
                                            .child(toggle(("dt-arot", k), "Rotate", opts.angle_rotate).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                                if let Some(r) = this.datum_dlg.as_mut().and_then(|d| d.rows.get_mut(k)) {
                                                    let o = r.ann.target.get_or_insert_with(Default::default);
                                                    o.angle_rotate = !o.angle_rotate;
                                                }
                                                this.datum_fill_distances(k, window, cx);
                                            }))),
                                    )
                            }),
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

/// The point of segment `a`-`b` nearest `p`.
fn foot(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    [a[0] + dx * t, a[1] + dy * t]
}

fn dist2(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
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

/// The Style list's glyph: the bubble, its pointer, the size outside.
fn style_glyph(s: TargetStyle) -> &'static str {
    match (s.moveable().is_some(), s.outside()) {
        (false, false) => "⊖",
        (false, true) => "⊖↗",
        (true, false) => "⊲",
        (true, true) => "⊲↗",
    }
}
