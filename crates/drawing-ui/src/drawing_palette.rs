//! Drawing editor: Onshape's dimension palette and the hole callout panel
//! (double-click a dimension or a hole / thread callout, or the dimension's
//! Edit…; live drawing/15, 460–467, dom/drawing-dimension-edit-dialog.txt,
//! drawing-hole-callout-*): a floating editor beside the annotation.
//!
//! Rows: text above; [prefix] [value + precision ▾ (dimension) | count ▾ +
//! the callout, read only (hole)] [tolerance ▾ (dimension)] [units ▾]
//! [suffix]; text below; [Ø ▾ symbols] and the toggles (dimension:
//! inspection oval, dual ▾, reset text position, parentheses, radius /
//! diameter; above them, with a dual shown, the dual row: its precision ▾,
//! units ▾ and Show/hide units; hole: (XX), dual ▾, ☐ Override precision/tolerance);
//! then the tolerance's values (Symmetrical / Deviation / Limits with ±
//! buttons) or Fit options (fit type, hole class, shaft class, standard,
//! fit precision, stacking).
//!
//! Model: everything edits `Annotation::format` (`DimFormat`: above,
//! below, prefix, suffix, precision, units, show_units, tolerance, fit,
//! reference, inspection_frame, dual, count) plus `Annotation::hole`
//! (`HoleCalloutOpts`) and, for linear dimensions, `override_text`.
//! Changes preview at once; Enter or a click on the sheet keeps them, Esc
//! puts them back (one undo step).

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{Annotation, AnnotationKind, DimUnits, FitMode, FitStacking, FitType, HoleCount, Tolerance};

/// The symbols list (live drawing/461; D32).
pub const SYMBOLS: [&str; 14] = ["°", "Ø", "℄", "⌵", "↧", "⌴", "▢", "⌒", "±", "Ⓤ", "Ⓘ", "✟", "✝", "▷"];
/// Precision list: label and decimals (None: the drawing's).
pub use noshape_ops::drawing::PRECISIONS;
/// Tolerance lists (D35; hole callouts have no Basic).
pub use noshape_ops::drawing::{DIM_TOLERANCES, HOLE_TOLERANCES};

impl PaletteDlg {
    /// Open it at `at` (editor px; the launcher puts it over itself).
    pub fn set_at(&mut self, at: [f32; 2]) {
        self.at = Some(at);
    }
}

/// The open palette.
pub struct PaletteDlg {
    pub id: u64,
    /// A hole callout (else a dimension).
    pub hole: bool,
    pub above: Entity<InputState>,
    pub prefix: Entity<InputState>,
    /// The value (dimensions; typing overrides a linear dimension's text).
    pub value: Entity<InputState>,
    pub suffix: Entity<InputState>,
    pub below: Entity<InputState>,
    /// Tolerance values: Symmetrical `tol_a`; Deviation / Limits upper,
    /// lower (the callout's units).
    pub tol_a: Entity<InputState>,
    pub tol_b: Entity<InputState>,
    /// The field symbols go into (0 above, 1 prefix, 2 suffix, 3 below).
    pub target: u8,
    /// The tolerance list entry.
    pub tol: &'static str,
    /// The value shown when the palette opened (unchanged: no override).
    value_text: String,
    /// Where it opened (editor px); it stays there while the text changes.
    at: Option<[f32; 2]>,
    pub orig: Annotation,
    pub cur: Annotation,
    _subs: Vec<Subscription>,
}

use noshape_ops::drawing::tolerance_name as tol_name;

impl DrawingEditor {
    /// Double-click / Edit… on a dimension.
    pub fn open_dim_palette(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette(id, false, window, cx);
    }

    /// Double-click on a hole callout.
    pub fn open_hole_callout(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette(id, true, window, cx);
    }

    fn open_palette(&mut self, id: u64, hole: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(a) = self.current_sheet().annotations.iter().find(|x| x.id == id).cloned() else { return };
        let u = noshape_model::drawing::format::length_units(&self.drawing, &a.format);
        let num = |m: f64| noshape_model::drawing::format::format_number(m / u.meters(), None, 4);
        let (ta, tb) = match &a.format.tolerance {
            Tolerance::Symmetric { value } => (num(*value), String::new()),
            Tolerance::Deviation { upper, lower } | Tolerance::Limits { upper, lower } => (num(*upper), num(*lower)),
            _ => (String::new(), String::new()),
        };
        let rotation = a.kind.view().and_then(|v| self.placement(v)).map(|p| p.rotation).unwrap_or(0.0);
        let value_text = match &a.kind {
            AnnotationKind::LinearDimension { override_text: Some(t), .. } => t.clone(),
            k => k.measured(rotation).map(|m| noshape_model::drawing::format::format_len(&self.drawing, &noshape_model::drawing::DimFormat { show_units: false, ..a.format.clone() }, m)).unwrap_or_default(),
        };
        let above = self.input(&a.format.above, window, cx);
        let prefix = self.input(&a.format.prefix, window, cx);
        let value = self.input(&value_text, window, cx);
        let suffix = self.input(&a.format.suffix, window, cx);
        let below = self.input(&a.format.below, window, cx);
        let tol_a = self.input(&ta, window, cx);
        let tol_b = self.input(&tb, window, cx);
        // Live: the prefix box has the focus.
        // Live 15: the prefix box has the focus. Set it after the double
        // click's own focus handling.
        let first = prefix.clone();
        window.defer(cx, move |window, cx| first.update(cx, |s, cx| s.focus(window, cx)));
        let mut subs = Vec::new();
        for (k, st) in [(0u8, &above), (1, &prefix), (2, &suffix), (3, &below), (4, &tol_a), (5, &tol_b), (6, &value)] {
            subs.push(cx.subscribe(st, move |this: &mut Self, _, ev: &InputEvent, cx| match ev {
                InputEvent::Change => {
                    if let Some(d) = &mut this.palette
                        && k < 4
                    {
                        d.target = k;
                    }
                    this.palette_from_inputs(cx);
                }
                InputEvent::PressEnter { .. } => this.close_palette(true, cx),
                _ => {}
            }));
        }
        let tol = tol_name(&a);
        self.palette = Some(PaletteDlg { id, hole, above, prefix, value, suffix, below, tol_a, tol_b, target: 1, tol, value_text, at: None, orig: a.clone(), cur: a, _subs: subs });
        self.tool = Tool::Select;
        cx.notify();
    }

    /// Read the text fields into the edited annotation and preview it.
    fn palette_from_inputs(&mut self, cx: &mut Context<Self>) {
        let u = self.palette.as_ref().map(|d| noshape_model::drawing::format::length_units(&self.drawing, &d.cur.format));
        let Some(d) = &mut self.palette else { return };
        let Some(u) = u else { return };
        let val = |st: &Entity<InputState>| st.read(cx).value().to_string();
        let m = |st: &Entity<InputState>| val(st).trim().replace('−', "-").parse::<f64>().ok().map(|v| v * u.meters());
        let f = &mut d.cur.format;
        f.above = val(&d.above);
        f.below = val(&d.below);
        f.prefix = val(&d.prefix);
        f.suffix = val(&d.suffix);
        noshape_ops::drawing::set_tolerance(f, d.tol, m(&d.tol_a).unwrap_or(0.0), m(&d.tol_b).unwrap_or(0.0));
        // A typed value overrides a linear dimension's text.
        let typed = val(&d.value);
        if let AnnotationKind::LinearDimension { override_text, .. } = &mut d.cur.kind {
            *override_text = (typed.trim() != d.value_text.trim() && !typed.trim().is_empty()).then(|| typed.clone());
        }
        if d.cur.hole.as_deref().is_some_and(|o| *o == Default::default()) {
            d.cur.hole = None;
        }
        self.palette_preview(cx);
    }

    /// Change the edited annotation (dropdowns, toggles) and preview it.
    fn edit_palette(&mut self, f: impl FnOnce(&mut PaletteDlg), cx: &mut Context<Self>) {
        if let Some(d) = &mut self.palette {
            f(d);
        }
        self.palette_from_inputs(cx);
    }

    fn palette_preview(&mut self, cx: &mut Context<Self>) {
        let Some(d) = &self.palette else { return };
        let mut dr = self.drawing.clone();
        if let Some(a) = dr.sheets[self.sheet].annotations.iter_mut().find(|a| a.id == d.id) {
            *a = d.cur.clone();
        }
        self.set_preview(Some(dr), cx);
        cx.notify();
    }

    pub fn close_palette(&mut self, accept: bool, cx: &mut Context<Self>) {
        self.launcher.opened = false;
        let Some(d) = self.palette.take() else { return };
        self.set_preview(None, cx);
        if accept && d.cur != d.orig {
            let (id, cur) = (d.id, d.cur);
            self.edit_sheet(if d.hole { "Edit hole callout" } else { "Edit dimension" }, cx, |s| {
                if let Some(a) = s.annotations.iter_mut().find(|a| a.id == id) {
                    *a = cur;
                }
            });
        }
        cx.notify();
    }

    /// Where the palette sits: beside the annotation text's upper right
    /// (editor px).
    fn palette_at(&self, id: u64) -> [f32; 2] {
        let Some(b) = &self.built else { return [200., 120.] };
        let picks = self.picks_of(Sel::Annotation(id));
        let (mut x, mut y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for t in b.geometry.texts.iter().filter(|t| picks.contains(&t.pick)) {
            x = x.max(t.pos[0] + noshape_model::drawing::font::text_width(&t.text, t.height));
            y = y.max(t.pos[1] + t.height);
        }
        if !x.is_finite() {
            return [200., 120.];
        }
        let s = self.size_px();
        let px_x = (x - self.center[0]) * self.zoom + s[0] / 2.0;
        let px_y = (self.center[1] - y) * self.zoom + s[1] / 2.0;
        [(px_x as f32).clamp(0., (s[0] as f32 - 500.).max(0.)), (px_y as f32 - 30.).clamp(0., (s[1] as f32 - 200.).max(0.))]
    }

    pub fn render_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.palette.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let ed = cx.entity();
        let at = d.at.unwrap_or_else(|| self.palette_at(d.id));
        if d.at.is_none()
            && let Some(p) = &mut self.palette
        {
            p.at = Some(at);
        }
        let d = self.palette.as_ref()?;
        let hole = d.hole;
        let opts = d.cur.hole.as_deref().cloned().unwrap_or_default();
        let fmt = d.cur.format.clone();
        let sheet = self.current_sheet();
        let units = fmt.units;
        let unit_label = match units {
            None => format!("({})", noshape_model::drawing::format::length_units(&self.drawing, &fmt).symbol()),
            Some(u) if u.symbol().is_empty() => "(ft-in)".to_string(),
            Some(u) => format!("({})", u.symbol()),
        };
        // A focused box: live's 1 px rgba(82,168,236,.8) border with an 8 px
        // rgba(82,168,236,.6) glow (dom/drawing-dimension-edit-dialog.txt).
        let focus_id = [&d.above, &d.prefix, &d.value, &d.suffix, &d.below, &d.tol_a, &d.tol_b].into_iter().find(|st| st.read(cx).focus_handle(cx).is_focused(window)).map(|st| st.entity_id());
        let field = |st: &Entity<InputState>, w: Option<f32>, h: f32| {
            let on = focus_id == Some(st.entity_id());
            let b = div()
                .h(px(h))
                .border_1()
                .border_color(if on { gpui_kit::rgba(0x52a8eccc) } else { gpui_kit::rgba(0xccccccff) })
                .when(on, |e| e.shadow(vec![BoxShadow { color: hsla(206. / 360., 0.81, 0.62, 0.6), offset: point(px(0.), px(0.)), blur_radius: px(8.), spread_radius: px(0.), inset: false }]))
                .rounded(px(4.))
                .bg(crate::theme::light().panel)
                .flex()
                .items_center()
                .px(px(3.))
                // Live: the text fields are 14 px `#555` (dom/drawing-dimension-edit-dialog.txt).
                .text_color(gpui_kit::rgb(0x555555))
                .child(Input::new(st).text_size(px(14.)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall());
            match w {
                Some(w) => b.w(px(w)),
                None => b.w_full(),
            }
        };
        type Act = std::rc::Rc<dyn Fn(&mut PaletteDlg)>;
        // Live 463, 466: a white combo, 1 px `#ccc`, radius 4: the value and a
        // 20 px caret cell with its own left rule (`h` 28, or the hole
        // callout's 54 px count / units).
        let combo = |id: &'static str, label: AnyElement, w: f32, h: f32, items: Vec<(String, bool, Act)>| -> AnyElement {
            let ed = ed.clone();
            Button::new(id)
                .ghost()
                .w(px(w))
                .h(px(h))
                .p_0()
                .bg(crate::theme::light().panel)
                .border_1()
                .border_color(gpui_kit::rgb(0xcccccc))
                .rounded(px(4.))
                .child(
                    h_flex()
                        .size_full()
                        .child(div().flex_1().min_w_0().flex().justify_center().child(label))
                        .child(div().w(px(20.)).h_full().flex_none().flex().items_center().justify_center().border_l_1().border_color(gpui_kit::rgb(0xcccccc)).child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., gpui_kit::rgb(0x000000)))),
                )
                .os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                // As wide as the button, or as the longest row (a browser
                // select; "Fit with tolerance" ran out of a 45 px list).
                m = m.min_width(widest(&items));
                for (l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| e.update(cx, |t, cx| t.edit_palette(|d| f(d), cx))));
                }
                m
            })
            .into_any_element()
        };
        let menu = |id: &'static str, label: AnyElement, w: f32, items: Vec<(String, bool, Act)>| combo(id, label, w, 28., items);
        let menu_tall = |id: &'static str, label: AnyElement, w: f32, items: Vec<(String, bool, Act)>| combo(id, label, w, 54., items);
        let toggle = |id: &'static str, label: &'static str, tip: &'static str, on: bool, f: fn(&mut PaletteDlg), cx: &mut Context<Self>| {
            div()
                .id(id)
                .h(px(26.))
                .px(px(4.))
                .flex()
                .items_center()
                .rounded(px(2.))
                .border_1()
                .border_color(gpui_kit::rgb(0xcccccc))
                .when(on, |e| e.bg(gpui_kit::rgb(0xdfe7ee)))
                .cursor_pointer()
                .text_size(px(12.))
                .child(label)
                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(w, cx))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.edit_palette(f, cx)))
                .into_any_element()
        };
        let small = |s: String| div().text_size(px(12.)).text_color(ink).child(s).into_any_element();
        // Units (D34): the drawing's entry "… (Drawing)" in the list.
        let mut unit_items: Vec<(String, bool, Act)> = DimUnits::CALLOUT.iter().map(|&u| (u.name().to_string(), units == Some(u), std::rc::Rc::new(move |d: &mut PaletteDlg| d.cur.format.units = Some(u)) as Act)).collect();
        let drawing_units = if self.drawing.units_mm { "Millimeters (Drawing)" } else { "Inches (Drawing)" };
        unit_items.insert(if self.drawing.units_mm { 1 } else { 4 }, (drawing_units.to_string(), units.is_none(), std::rc::Rc::new(|d: &mut PaletteDlg| d.cur.format.units = None) as Act));
        // Live: the drawing's own entry follows its Dual setting (asb 10-24:
        // Single unit | Dual unit | Dual unit (Drawing); drawing/461 with dual
        // off: Single unit | Single unit (Drawing) | Dual unit).
        let entries: [(&str, Option<bool>); 3] = if self.drawing.props.dual.show {
            [("Single unit", Some(false)), ("Dual unit", Some(true)), ("Dual unit (Drawing)", None)]
        } else {
            [("Single unit", Some(false)), ("Single unit (Drawing)", None), ("Dual unit", Some(true))]
        };
        let dual_items: Vec<(String, bool, Act)> = entries
            .iter()
            .map(|&(l, v)| (l.to_string(), fmt.dual == v, std::rc::Rc::new(move |d: &mut PaletteDlg| d.cur.format.dual = v) as Act))
            .collect();
        let precision = fmt.precision;
        let prec_items: Vec<(String, bool, Act)> = PRECISIONS.iter().map(|&(l, v)| (l.to_string(), precision == v, std::rc::Rc::new(move |d: &mut PaletteDlg| d.cur.format.precision = v) as Act)).collect();
        let prec_label = PRECISIONS.iter().find(|x| x.1 == precision).map(|x| x.0).unwrap_or("(Drawing)");
        let tol = d.tol;
        let tol_list: &'static [&'static str] = if hole { &HOLE_TOLERANCES } else { &DIM_TOLERANCES };
        let tol_items: Vec<(String, bool, Act)> = tol_list.iter().map(|&l| (l.to_string(), tol == l, std::rc::Rc::new(move |d: &mut PaletteDlg| d.tol = l) as Act)).collect();
        let symbols = {
            let ed = ed.clone();
            Button::new("dp-sym").ghost().w(px(45.)).h(px(30.)).p_0().border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(4.)).child(h_flex().gap(px(5.)).child(div().text_size(px(20.)).text_color(ink).child("Ø")).child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., gpui_kit::rgb(0x000000)))).os_dropdown_menu("dp-sym-dd", true, move |mut m, _, _| {
                for s in SYMBOLS {
                    let e = ed.clone();
                    m = m.item(crate::os_menu::OsMenuItem::new(s).on_click(move |_, window, cx| {
                        e.update(cx, |t, cx| {
                            let Some(d) = &t.palette else { return };
                            let st = match d.target {
                                0 => d.above.clone(),
                                2 => d.suffix.clone(),
                                3 => d.below.clone(),
                                _ => d.prefix.clone(),
                            };
                            st.update(cx, |st, cx| {
                                let v = format!("{}{s}", st.value());
                                st.set_value(v, window, cx)
                            });
                            t.palette_from_inputs(cx);
                        })
                    }));
                }
                m
            })
            .into_any_element()
        };
        // Live palette buttons: 30 px tall, 1 px #ccc border, 4 px radius;
        // a ▾ list when `items` isn't empty (`right` rounds only the right
        // side, the value combo's caret).
        let pbutton = |id: &'static str, content: AnyElement, w: f32, items: Vec<(String, bool, Act)>, right: bool| -> AnyElement {
            let ed = ed.clone();
            Button::new(id)
                .ghost()
                .w(px(w))
                .h(px(30.))
                .p_0()
                .border_1()
                .border_color(gpui_kit::rgb(0xcccccc))
                .when(right, |b| b.rounded_r(px(4.)).rounded_l(px(0.)))
                .when(!right, |b| b.rounded(px(4.)))
                // Live: the content and an 8 x 4 black caret.
                .child(h_flex().gap(px(3.)).child(content).child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., gpui_kit::rgb(0x000000))))
                .os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                    m = m.min_width(widest(&items));
                    for (l, on, f) in &items {
                        let (e, f) = (ed.clone(), f.clone());
                        m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| e.update(cx, |t, cx| t.edit_palette(|d| f(d), cx))));
                    }
                    m
                })
                .into_any_element()
        };
        let icon_toggle = |id: &'static str, content: AnyElement, tip: &'static str, on: bool, f: fn(&mut PaletteDlg), cx: &mut Context<Self>| -> AnyElement {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .border_1()
                .border_color(gpui_kit::rgb(0xcccccc))
                .when(on, |e| e.bg(gpui_kit::rgb(0xdfe7ee)))
                .cursor_pointer()
                .child(content)
                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(w, cx))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.edit_palette(f, cx)))
                .into_any_element()
        };
        // The middle row.
        let middle = if hole {
            let n = noshape_model::drawing::build::hole_callout_count(sheet, &self.sources, &d.cur).unwrap_or(1);
            let core = Annotation { format: noshape_model::drawing::DimFormat { prefix: String::new(), suffix: String::new(), above: String::new(), below: String::new(), reference: false, ..fmt.clone() }, ..d.cur.clone() };
            let preview = noshape_model::drawing::build::hole_callout_lines(&self.drawing, sheet, &self.sources, &core).unwrap_or_default();
            let counts: Vec<(String, bool, Act)> = HoleCount::ALL.iter().map(|&c| (c.name().to_string(), opts.count == c, std::rc::Rc::new(move |d: &mut PaletteDlg| d.cur.hole.get_or_insert_with(Default::default).count = c) as Act)).collect();
            h_flex()
                .gap(px(4.))
                .h(px(54.))
                .child(field(&d.prefix, Some(80.), 54.))
                .child(menu_tall("dp-count", small(format!("{n}{}", self.drawing.props.hole_indicator)), 104., counts))
                .child(v_flex().w(px(145.)).h(px(54.)).px(px(10.)).justify_center().rounded(px(2.)).bg(gpui_kit::rgb(0xe8e8e8)).text_size(px(12.)).text_color(gpui_kit::rgb(0x8a8a8a)).children(preview.into_iter().map(|l| div().child(l))))
                .child(menu_tall("dp-units", small(unit_label), 58., unit_items))
                .child(field(&d.suffix, Some(80.), 54.))
        } else {
            // Live drawing/15 (dom/drawing-dimension-edit-dialog.txt): prefix
            // 80, the value 85 + ▾ 20 (precision), the tolerance icon 45, the
            // units 60, suffix 80; 30 px tall, 2 px apart.
            let value_combo = h_flex()
                .h(px(30.))
                .ml(px(10.))
                .child(div().w(px(85.)).h(px(30.)).border_1().border_color(gpui_kit::rgb(0xdddddd)).rounded_l(px(4.)).bg(crate::theme::light().panel).flex().items_center().px(px(6.)).text_color(gpui_kit::rgb(0x555555)).child(Input::new(&d.value).appearance(false).xsmall().text_size(px(14.)).text_center()))
                .child(pbutton("dp-prec", div().into_any_element(), 20., prec_items.clone(), true));
            let tol_icon = v_flex()
                .items_center()
                .line_height(px(8.))
                .child(div().text_size(px(7.)).text_color(gpui_kit::rgb(0x1b72c4)).child("+0.01"))
                .child(div().text_size(px(10.)).text_color(ink).child("1.500"))
                .child(div().text_size(px(7.)).text_color(gpui_kit::rgb(0x1b72c4)).child("−0.01"))
                .into_any_element();
            h_flex()
                .gap(px(2.))
                .h(px(32.))
                .child(field(&d.prefix, Some(80.), 30.))
                .child(value_combo)
                .child(pbutton("dp-tol", tol_icon, 45., tol_items.clone(), false))
                .child(pbutton("dp-units", div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(ink).child(unit_label).into_any_element(), 60., unit_items, false))
                .child(field(&d.suffix, Some(80.), 30.))
        };
        // The dual row (live asbdd-9, a dimension showing its dual): "[ 0.39 ▾
        // (in) ▾ 👁 ]" on grey, the value read only; precision, units and
        // Show/hide units are this dimension's ("(Drawing)": Drawing
        // properties › Dual).
        let dual_row = if !hole && fmt.dual.unwrap_or(self.drawing.props.dual.show) && !matches!(d.cur.kind, AnnotationKind::AngularDimension { .. }) {
            use noshape_model::drawing::format as nf;
            let rotation = d.cur.kind.view().and_then(|v| self.placement(v)).map(|p| p.rotation).unwrap_or(0.0);
            let value = d.cur.kind.measured(rotation).map(|m| nf::dual_value(&self.drawing, &fmt, m)).unwrap_or_default();
            let du = nf::dual_units(&self.drawing, &fmt);
            let drawing_du = self.drawing.props.dual.units_for(!nf::length_units(&self.drawing, &fmt).imperial());
            let dprec = fmt.dual_fmt.precision;
            let prec_items: Vec<(String, bool, Act)> = PRECISIONS.iter().map(|&(l, v)| (l.to_string(), dprec == v, std::rc::Rc::new(move |d: &mut PaletteDlg| d.cur.format.dual_fmt.precision = v) as Act)).collect();
            let own = fmt.dual_fmt.units;
            let mut unit_items: Vec<(String, bool, Act)> = DimUnits::CALLOUT.iter().map(|&u| (u.name().to_string(), own == Some(u), std::rc::Rc::new(move |d: &mut PaletteDlg| d.cur.format.dual_fmt.units = Some(u)) as Act)).collect();
            let at = DimUnits::CALLOUT.iter().position(|&u| u == drawing_du).map_or(0, |i| i + 1);
            unit_items.insert(at, (format!("{} (Drawing)", drawing_du.name()), own.is_none(), std::rc::Rc::new(|d: &mut PaletteDlg| d.cur.format.dual_fmt.units = None) as Act));
            let unit_label = if du.symbol().is_empty() { "(ft-in)".to_string() } else { format!("({})", du.symbol()) };
            let shown = nf::dual_shows_unit(&self.drawing, &fmt);
            let bracket = |s: &'static str| div().w(px(14.)).flex().justify_center().text_size(px(28.)).font_weight(FontWeight::LIGHT).text_color(gpui_kit::rgb(0x777777)).child(s);
            let eye = div()
                .id("dp-dual-units")
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .border_1()
                .border_color(gpui_kit::rgb(0xcccccc))
                .when(shown, |e| e.bg(gpui_kit::rgb(0xdfe7ee)))
                .cursor_pointer()
                .child(crate::ui::glyph("eye", 16., ink))
                .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new("Show/hide units").build(w, cx))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.edit_palette(move |d| d.cur.format.dual_fmt.show_unit = Some(!shown), cx)));
            Some(
                h_flex()
                    .h(px(34.))
                    .pl(px(70.))
                    .gap(px(2.))
                    .bg(gpui_kit::rgb(0xf0f0f0))
                    .child(bracket("["))
                    .child(
                        h_flex()
                            .h(px(30.))
                            .ml(px(4.))
                            .child(div().w(px(85.)).h(px(30.)).border_1().border_color(gpui_kit::rgb(0xdddddd)).rounded_l(px(4.)).bg(gpui_kit::rgb(0xeeeeee)).flex().items_center().justify_center().text_size(px(14.)).text_color(gpui_kit::rgb(0x999999)).child(value))
                            .child(pbutton("dp-dual-prec", div().into_any_element(), 20., prec_items, true)),
                    )
                    .child(div().w(px(50.)))
                    .child(pbutton("dp-dual-unit", div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(ink).child(unit_label).into_any_element(), 60., unit_items, false))
                    .child(eye)
                    .child(bracket("]"))
                    .into_any_element(),
            )
        } else {
            None
        };
        // The toggles row.
        let radial = matches!(d.cur.kind, AnnotationKind::RadialDimension { .. });
        let mut toggles = h_flex().gap(px(if hole { 4. } else { 2. })).child(symbols);
        if hole {
            toggles = toggles.child(toggle("dp-ref", "(XX)", "Parentheses", fmt.reference, |d| d.cur.format.reference = !d.cur.format.reference, cx)).child(menu("dp-dual", small("[XX] x.xx".into()), 80., dual_items)).child(
                div().pl(px(8.)).child(crate::drawing_chrome::DrCheckbox::new("dp-override").label("Override precision/tolerance").checked(opts.override_tol).tooltip("Click to override the precision and tolerance from the hole feature").on_click(cx.listener(|this, v: &bool, _, cx| {
                    let v = *v;
                    this.edit_palette(|d| d.cur.hole.get_or_insert_with(Default::default).override_tol = v, cx)
                }))),
            );
        } else {
            // Live drawing/15: [Ø ▾] 45, reset text position, parentheses,
            // inspection dimension (30 each), [XX] x.xx ▾ dual 45; plus the
            // radius / diameter toggle on radial dimensions.
            let blue = gpui_kit::rgb(0x1b72c4);
            let reset = h_flex().gap(px(1.)).child(div().text_size(px(9.)).text_color(blue).child("←")).child(div().text_size(px(13.)).text_color(ink).child("X")).child(div().text_size(px(9.)).text_color(blue).child("→")).into_any_element();
            let parens = h_flex().text_size(px(13.)).child(div().text_color(blue).child("(")).child(div().text_color(ink).child("XX")).child(div().text_color(blue).child(")")).into_any_element();
            let oval = div().px(px(3.)).rounded_full().border_1().border_color(blue).text_size(px(11.)).text_color(ink).child("XX").into_any_element();
            let dual = v_flex().items_center().line_height(px(11.)).child(div().text_size(px(12.)).text_color(blue).child("[XX]")).child(div().text_size(px(8.)).text_color(ink).child("x.xx")).into_any_element();
            toggles = toggles
                .child(icon_toggle("dp-reset", reset, "Reset text position", false, |d| {
                    if let AnnotationKind::RadialDimension { angle, .. } = &mut d.cur.kind {
                        *angle = std::f64::consts::FRAC_PI_4;
                    }
                }, cx))
                .child(icon_toggle("dp-paren", parens, "Parentheses", fmt.reference, |d| d.cur.format.reference = !d.cur.format.reference, cx))
                .child(icon_toggle("dp-insp", oval, "Inspection dimension", fmt.inspection_frame, |d| d.cur.format.inspection_frame = !d.cur.format.inspection_frame, cx))
                .child(pbutton("dp-dual", dual, 45., dual_items, false))
                .when(radial, |e| {
                    e.child(icon_toggle("dp-rd", div().text_size(px(12.)).text_color(ink).child("R/Ø").into_any_element(), "Toggle Radius/Diameter dimension", false, |d| {
                        if let AnnotationKind::RadialDimension { diameter, .. } = &mut d.cur.kind {
                            *diameter = !*diameter;
                        }
                    }, cx))
                });
        }
        // Tolerance values / Fit options (hole callouts: under Override).
        let show_tol = !hole || opts.override_tol;
        let flip = |id: &'static str, upper: bool, cx: &mut Context<Self>| {
            div().id(id).w(px(20.)).h(px(26.)).flex().items_center().justify_center().border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(2.)).cursor_pointer().text_size(px(12.)).child("±").on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                // "+" / "-": flip one value's sign (+/+ or -/- tolerances).
                let Some(d) = &this.palette else { return };
                let st = if upper { d.tol_a.clone() } else { d.tol_b.clone() };
                st.update(cx, |st, cx| {
                    let v = st.value().to_string();
                    let t = v.trim();
                    let flipped = match t.strip_prefix('-').or_else(|| t.strip_prefix('−')) {
                        Some(r) => r.to_string(),
                        None => format!("-{t}"),
                    };
                    st.set_value(flipped, window, cx)
                });
                this.palette_from_inputs(cx);
            }))
        };
        let fit = matches!(tol, "Fit" | "Fit with tolerance" | "Fit (Tolerance only)");
        let fo = fmt.fit.clone();
        let ansi_default = self.drawing.dim_fit_ansi();
        let standard = match fo.iso {
            Some(true) => "ISO",
            Some(false) => "ANSI",
            None if ansi_default => "ANSI",
            None => "ISO",
        };
        let (hole_class, shaft_class) = match &fmt.tolerance {
            Tolerance::Fit { hole, shaft } => (hole.clone(), shaft.clone()),
            _ => (String::new(), String::new()),
        };
        let fit_row = || -> AnyElement {
            let type_items: Vec<(String, bool, Act)> = FitType::ALL.iter().map(|&t| (t.name().to_string(), fo.fit_type == t, std::rc::Rc::new(move |d: &mut PaletteDlg| d.cur.format.fit.fit_type = t) as Act)).collect();
            let hole_items: Vec<(String, bool, Act)> = std::iter::once("None")
                .chain(noshape_model::fits::hole_classes(standard, fo.fit_type.name()))
                .map(|c| {
                    let v = if c == "None" { String::new() } else { c.to_string() };
                    (c.to_string(), hole_class == v, std::rc::Rc::new(move |d: &mut PaletteDlg| if let Tolerance::Fit { hole, .. } = &mut d.cur.format.tolerance { *hole = v.clone() }) as Act)
                })
                .collect();
            let shaft_items: Vec<(String, bool, Act)> = noshape_model::fits::shaft_classes(standard, fo.fit_type.name(), &hole_class)
                .into_iter()
                .map(|c| {
                    let v = if c == "None" { String::new() } else { c.to_string() };
                    (c.to_string(), shaft_class == v, std::rc::Rc::new(move |d: &mut PaletteDlg| if let Tolerance::Fit { shaft, .. } = &mut d.cur.format.tolerance { *shaft = v.clone() }) as Act)
                })
                .collect();
            let std_items: Vec<(String, bool, Act)> = [(if ansi_default { "ANSI (Drawing)" } else { "ISO (Drawing)" }, None), ("ISO", Some(true)), ("ANSI", Some(false))]
                .iter()
                .map(|&(l, v)| (l.to_string(), fo.iso == v, std::rc::Rc::new(move |d: &mut PaletteDlg| d.cur.format.fit.iso = v) as Act))
                .collect();
            let fprec_items: Vec<(String, bool, Act)> = PRECISIONS.iter().map(|&(l, v)| (l.to_string(), fo.precision == v, std::rc::Rc::new(move |d: &mut PaletteDlg| d.cur.format.fit.precision = v) as Act)).collect();
            let stack_items: Vec<(String, bool, Act)> = FitStacking::ALL.iter().map(|&s| (s.name().to_string(), fo.stacking == s, std::rc::Rc::new(move |d: &mut PaletteDlg| d.cur.format.fit.stacking = s) as Act)).collect();
            let or_none = |s: &str| if s.is_empty() { "None".to_string() } else { s.to_string() };
            h_flex()
                .gap(px(3.))
                .pt(px(3.))
                .child(menu("dp-fit-type", small(fo.fit_type.name().into()), 98., type_items))
                .child(menu("dp-fit-hole", small(or_none(&hole_class)), 58., hole_items))
                .child(menu("dp-fit-shaft", small(or_none(&shaft_class)), 58., shaft_items))
                .child(menu("dp-fit-std", small(match fo.iso {
                    None => format!("{standard} (Drawing)"),
                    Some(_) => standard.to_string(),
                }), 110., std_items))
                .when(fo.mode != FitMode::Fit, |e| e.child(menu("dp-fit-prec", small(PRECISIONS.iter().find(|x| x.1 == fo.precision).map(|x| x.0).unwrap_or("(Drawing)").to_string()), 80., fprec_items)))
                .child(menu("dp-fit-stack", small("H7/g6".into()), 58., stack_items))
                .into_any_element()
        };
        let tol_row = h_flex()
            .gap(px(4.))
            .pt(px(3.))
            .when(hole, |e| e.child(menu("dp-hprec", small(prec_label.to_string()), 110., prec_items)).child(menu("dp-htol", small(tol.to_string()), 150., tol_items)))
            .when(matches!(tol, "Symmetrical" | "Deviation" | "Limits"), |e| e.child(field(&d.tol_a, Some(70.), 26.)))
            .when(matches!(tol, "Deviation" | "Limits"), |e| e.child(flip("dp-flip-a", true, cx)).child(field(&d.tol_b, Some(70.), 26.)).child(flip("dp-flip-b", false, cx)));
        Some(
            v_flex()
                .id("dr-dim-palette")
                // Esc in any of its fields puts the annotation back.
                .capture_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                    if e.keystroke.key == "escape" {
                        this.close_palette(false, cx);
                        cx.stop_propagation();
                    }
                }))
                // Opened from the launcher: leaving it closes it (live).
                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                    if !*hovered {
                        this.launcher_palette_left(cx);
                    }
                }))
                .absolute()
                .left(px(at[0]))
                .top(px(at[1]))
                .w(px(if hole { 497. } else { 405. }))
                // Live: 1 px body padding, rows 2 px apart (hole callouts 3).
                .p(px(if hole { 3. } else { 1. }))
                .gap(px(if hole { 3. } else { 2. }))
                .bg(pal.dialog_bg)
                // Live 15: 6 px corners, 1 px grey border.
                .rounded(px(6.))
                .border_1()
                .border_color(gpui_kit::rgb(0xbbbbbb))
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(ink)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                // The drag handle.
                .child(h_flex().h(px(13.)).justify_center().border_b_1().border_color(gpui_kit::rgb(0xeeeeee)).child(div().text_size(px(8.)).text_color(gpui_kit::rgb(0x9a9a9a)).child("▫▫▫")))
                .child(field(&d.above, None, 22.))
                .children(dual_row)
                .child(middle)
                .child(field(&d.below, None, 22.))
                .child(toggles)
                .when(show_tol && (hole || tol != "None"), |e| e.child(tol_row))
                .when(show_tol && fit, |e| e.child(fit_row()))
                .into_any_element(),
        )
    }
}

/// A select list's width for its longest row (13 px text, the row's padding).
fn widest<T, U>(items: &[(String, T, U)]) -> f32 {
    items.iter().map(|i| i.0.chars().count()).max().unwrap_or(0) as f32 * 7.5 + 24.0
}
