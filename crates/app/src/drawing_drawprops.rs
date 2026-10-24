//! Drawing editor: Onshape's Drawing properties panel (empty-sheet menu ›
//! Drawing properties…), docked at the right: eight tab icons; Units and
//! precision (Primary units, decimal separator, precisions; leading and
//! trailing zeros), Dimensions / Annotations (font, text, color,
//! arrowheads; gaps and extensions), Views (insert view defaults), Tables
//! (cell padding) and Inspection (item parsing, text height, border shape
//! and size, exclude callouts) edit [`DrawingProps`].

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::format::default_digits;
use noshape_model::drawing::sheet::AnnType;
use noshape_model::drawing::*;

/// What a panel field is (live dom/drawing-wt-widgets): a Wt dropdown
/// (text heights, arrowheads, datum size, inspection text) or spinbox
/// (gaps, stack gap, center of mass, table padding, centermark sizes)
/// (`drawing_wt`), or text (label prefixes).
#[derive(Clone, Copy, PartialEq)]
pub enum FieldKind {
    Dropdown,
    /// ↑ / ↓ step in an inch and in a mm drawing, decimals shown (live
    /// dom/drawing-wt-widgets: gaps 4, table padding 3).
    Spin { step_in: f64, step_mm: f64, decimals: usize },
    Text,
}

/// A panel field: its input, kind, the value it shows (mm), whether a
/// spinbox holds no number, and its subscription.
pub struct NumberField {
    pub state: Entity<InputState>,
    pub kind: FieldKind,
    pub value: std::rc::Rc<std::cell::Cell<f64>>,
    pub invalid: std::rc::Rc<std::cell::Cell<bool>>,
    /// Applies a value (mm) / a text to the drawing's properties.
    pub set: Option<fn(&mut DrawingProps, f64)>,
    pub set_text: Option<fn(&mut DrawingProps, String)>,
    _sub: Subscription,
}

/// Drawing properties panel: open, the selected tab and the Inspection
/// tab's number fields (text height, border size).
#[derive(Default)]
pub struct DrawingPropsUi {
    pub open: bool,
    pub tab: usize,
    /// The open tab's number fields: (tab, fields).
    pub inspection_inputs: Option<(usize, Vec<NumberField>)>,
    /// The field that had the focus at the last render.
    pub focused: Option<usize>,
}

/// Onshape's tabs (tooltip, icon).
const TABS: [(&str, &str); 8] = [
    ("Units and precision", "dp-units"),
    ("Dimensions", "dp-dimensions"),
    ("Annotations", "dp-annotations"),
    ("Views", "dp-views"),
    ("Construction geometry", "dp-construction"),
    ("Formats", "dp-formats"),
    ("Tables", "dp-tables"),
    ("Inspection", "dp-inspection"),
];

/// Drawing color choices (None = black, the default ink).
pub const COLORS: [(&str, Option<[u8; 3]>); 9] = [
    ("Black", None),
    ("Red", Some([220, 40, 40])),
    ("Orange", Some([240, 140, 20])),
    ("Yellow", Some([230, 200, 0])),
    ("Green", Some([30, 160, 60])),
    ("Blue", Some([22, 81, 176])),
    ("Purple", Some([130, 60, 180])),
    ("Gray", Some([128, 128, 128])),
    ("White", Some([255, 255, 255])),
];

/// "0", "0.1", "0.12"... for 0..=6 decimals.
fn precision_label(digits: usize) -> String {
    if digits == 0 { "0".into() } else { format!("0.{}", &"123456"[..digits]) }
}

type Act = std::rc::Rc<dyn Fn(&mut DrawingProps)>;

/// Which text a Font row sets.
#[derive(Clone, Copy)]
enum FontOf {
    Dims,
    Inspection,
    Type(AnnType),
}

fn font_of(p: &mut DrawingProps, w: FontOf) -> &mut Option<noshape_model::drawing::sheet::TextFont> {
    match w {
        FontOf::Dims => &mut p.dim_font,
        FontOf::Inspection => &mut p.inspection_font,
        FontOf::Type(t) => &mut p.ann_type_mut(t).font,
    }
}

/// Element ids of the per-type rows (font select, bold, italic, color).
const TYPE_IDS: [[&str; 4]; 9] = [
    ["dp-t0-font", "dp-t0-b", "dp-t0-i", "dp-t0-color"],
    ["dp-t1-font", "dp-t1-b", "dp-t1-i", "dp-t1-color"],
    ["dp-t2-font", "dp-t2-b", "dp-t2-i", "dp-t2-color"],
    ["dp-t3-font", "dp-t3-b", "dp-t3-i", "dp-t3-color"],
    ["dp-t4-font", "dp-t4-b", "dp-t4-i", "dp-t4-color"],
    ["dp-t5-font", "dp-t5-b", "dp-t5-i", "dp-t5-color"],
    ["dp-t6-font", "dp-t6-b", "dp-t6-i", "dp-t6-color"],
    ["dp-t7-font", "dp-t7-b", "dp-t7-i", "dp-t7-color"],
    ["dp-t8-font", "dp-t8-b", "dp-t8-i", "dp-t8-color"],
];

/// Hole/thread callouts › Indicator choices (the count's sign: "4X").
const HOLE_INDICATORS: [&str; 3] = ["X", "x", "-"];


/// What committing a number field needs.
#[derive(Clone)]
struct FieldCommit {
    st: Entity<InputState>,
    kind: FieldKind,
    val: std::rc::Rc<std::cell::Cell<f64>>,
    inv: std::rc::Rc<std::cell::Cell<bool>>,
    set: fn(&mut DrawingProps, f64),
}

/// Commit a number field's text (Enter, or leaving it): a dropdown takes
/// its leading number in the drawing's units or reverts; a spinbox a plain
/// number or stays invalid.
fn commit_number(this: &mut DrawingEditor, f: &FieldCommit, inch: bool, window: &mut Window, cx: &mut Context<DrawingEditor>) {
    use crate::drawing_wt::{leading_number, show, spin_number};
    let FieldCommit { st, kind, val, inv, set } = f.clone();
    let k = if inch { 25.4 } else { 1.0 };
    let text = st.read(cx).value().to_string();
    match kind {
        FieldKind::Dropdown => {
            if let Some(d) = leading_number(&text).filter(|d| *d > 0.0) {
                val.set(d * k);
                this.set_drawing_prop(&move |p| set(p, d * k), cx);
            }
            let shown = show(val.get() / k);
            st.update(cx, |s, cx| s.set_value(shown, window, cx));
        }
        FieldKind::Spin { decimals, .. } => match spin_number(&text).filter(|d| *d >= 0.0) {
            Some(d) => {
                val.set(d * k);
                inv.set(false);
                this.set_drawing_prop(&move |p| set(p, d * k), cx);
                st.update(cx, |s, cx| s.set_value(crate::drawing_wt::show_dec(d, decimals), window, cx));
            }
            None => {
                inv.set(true);
                cx.notify();
            }
        },
        FieldKind::Text => {}
    }
}

/// A border size in characters as the selects show it ("Tight Fit",
/// "1 Character", "2 Characters"…).
fn border_chars_label(n: u8) -> String {
    match n {
        0 => "Tight Fit".into(),
        1 => "1 Character".into(),
        n => format!("{n} Characters"),
    }
}

/// Sets annotation type `I`'s text height.
fn set_type_height<const I: usize>(p: &mut DrawingProps, v: f64) {
    p.ann_type_mut(AnnType::ALL[I]).height = Some(v);
}

impl DrawingEditor {
    pub fn open_drawing_props(&mut self, cx: &mut Context<Self>) {
        self.drawing_props.open = true;
        self.right_panel = None;
        cx.notify();
    }

    /// The open tab's number fields (made from render; Enter / blur
    /// applies): Dimensions and Annotations text height and arrowhead,
    /// Inspection text height and border size.
    pub fn sync_drawprops_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab = self.drawing_props.tab;
        if !self.drawing_props.open || ![1, 2, 3, 4, 6, 7].contains(&tab) {
            self.drawing_props.inspection_inputs = None;
            return;
        }
        if self.drawing_props.inspection_inputs.as_ref().is_some_and(|i| i.0 == tab) {
            self.track_field_focus(window, cx);
            return;
        }
        let p = &self.drawing.props;
        // Lengths in the drawing's units ("0.1200 in", "3 mm"); typing a unit
        // overrides them.
        let inch = !self.drawing.units_mm;
        let k = if inch { 25.4 } else { 1.0 };
        let make = move |v: f64, kind: FieldKind, set: fn(&mut DrawingProps, f64), window: &mut Window, cx: &mut Context<Self>| {
            let shown = match kind {
                FieldKind::Spin { decimals, .. } => crate::drawing_wt::show_dec(v / k, decimals),
                _ => crate::drawing_wt::show(v / k),
            };
            let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(shown));
            let (value, invalid) = (std::rc::Rc::new(std::cell::Cell::new(v)), std::rc::Rc::new(std::cell::Cell::new(false)));
            let (val, inv) = (value.clone(), invalid.clone());
            // Enter commits (leaving the field too: `track_field_focus`); a
            // spinbox shows whether it holds a number while typing.
            let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
                InputEvent::PressEnter { .. } => commit_number(this, &FieldCommit { st: st.clone(), kind, val: val.clone(), inv: inv.clone(), set }, inch, window, cx),
                InputEvent::Change if matches!(kind, FieldKind::Spin { .. }) => {
                    let bad = crate::drawing_wt::spin_number(&st.read(cx).value()).filter(|d| *d >= 0.0).is_none();
                    if inv.replace(bad) != bad {
                        cx.notify();
                    }
                }
                _ => {}
            });
            NumberField { state: st, kind, value, invalid, set: Some(set), set_text: None, _sub: sub }
        };
        let dropdown = move |v: f64, set: fn(&mut DrawingProps, f64), window: &mut Window, cx: &mut Context<Self>| make(v, FieldKind::Dropdown, set, window, cx);
        // Steps: live 0.05 in; mm Stack gap 0.05, Text gap 0.1.
        let spin = move |v: f64, set: fn(&mut DrawingProps, f64), window: &mut Window, cx: &mut Context<Self>| make(v, FieldKind::Spin { step_in: 0.05, step_mm: 0.1, decimals: 4 }, set, window, cx);
        let spin_fine = move |v: f64, set: fn(&mut DrawingProps, f64), window: &mut Window, cx: &mut Context<Self>| make(v, FieldKind::Spin { step_in: 0.05, step_mm: 0.05, decimals: 4 }, set, window, cx);
        let spin3 = move |v: f64, set: fn(&mut DrawingProps, f64), window: &mut Window, cx: &mut Context<Self>| make(v, FieldKind::Spin { step_in: 0.05, step_mm: 0.1, decimals: 3 }, set, window, cx);
        // Text fields (label prefixes): applied on Enter / blur.
        let text_field = |v: String, set: fn(&mut DrawingProps, String), window: &mut Window, cx: &mut Context<Self>| {
            let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(v));
            let sub = cx.subscribe(&st, move |this: &mut Self, st, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    let v = st.read(cx).value().to_string();
                    this.set_drawing_prop(&move |p| set(p, v.clone()), cx);
                }
            });
            let none = std::rc::Rc::new(std::cell::Cell::new(0.0));
            NumberField { state: st, kind: FieldKind::Text, value: none, invalid: std::rc::Rc::new(std::cell::Cell::new(false)), set: None, set_text: Some(set), _sub: sub }
        };
        let vt = p.view_tab.clone();
        let fields = match tab {
            3 => vec![
                text_field(vt.section_prefix, |p, v| p.view_tab.section_prefix = v, window, cx),
                text_field(vt.detail_prefix, |p, v| p.view_tab.detail_prefix = v, window, cx),
                text_field(vt.aux_prefix, |p, v| p.view_tab.aux_prefix = v, window, cx),
                text_field(vt.aux_suffix, |p, v| p.view_tab.aux_suffix = v, window, cx),
            ],
            4 => vec![spin(vt.centermark_mark_size, |p, v| p.view_tab.centermark_mark_size = v, window, cx), spin(vt.centermark_gap_size, |p, v| p.view_tab.centermark_gap_size = v, window, cx)],
            1 => {
                let (h, a, tg, gg, ex) = (p.dim_text_height, p.dim_arrow, p.dim_text_gap, p.dim_geometry_gap, p.dim_extension);
                // Live raw dump: Text height / Arrowhead are Wt dropdowns, the
                // gaps spinboxes.
                vec![
                    dropdown(h, |p, v| p.dim_text_height = v, window, cx),
                    dropdown(a, |p, v| p.dim_arrow = v, window, cx),
                    spin(tg, |p, v| p.dim_text_gap = v, window, cx),
                    spin(gg, |p, v| p.dim_geometry_gap = v, window, cx),
                    spin(ex, |p, v| p.dim_extension = v, window, cx),
                ]
            }
            2 => {
                let (h, a) = (p.ann_text_height, p.ann_arrow);
                let mut f = vec![dropdown(h, |p, v| p.ann_text_height = v, window, cx), dropdown(a, |p, v| p.ann_arrow = v, window, cx)];
                // Per type text heights (shown: the type's, else All annotations'),
                // then Datum size, the center of mass diameter, the stack gap.
                let setters: [fn(&mut DrawingProps, f64); 9] = [set_type_height::<0>, set_type_height::<1>, set_type_height::<2>, set_type_height::<3>, set_type_height::<4>, set_type_height::<5>, set_type_height::<6>, set_type_height::<7>, set_type_height::<8>];
                for (t, set) in AnnType::ALL.iter().zip(setters) {
                    f.push(dropdown(p.ann_type(*t).height.unwrap_or(h), set, window, cx));
                }
                let (ds, cd, gap) = (p.datum_size, p.com_diameter, p.callout_stack_gap);
                f.push(dropdown(ds, |p, v| p.datum_size = v, window, cx));
                f.push(spin(cd, |p, v| p.com_diameter = v, window, cx));
                f.push(spin_fine(gap, |p, v| p.callout_stack_gap = v, window, cx));
                f
            }
            6 => {
                let (h, v) = (p.table_h_padding, p.table_v_padding);
                vec![spin3(h, |p, v| p.table_h_padding = v, window, cx), spin3(v, |p, v| p.table_v_padding = v, window, cx)]
            }
            _ => {
                vec![dropdown(p.inspection_text_height, |p, v| p.inspection_text_height = v, window, cx)]
            }
        };
        self.drawing_props.inspection_inputs = Some((tab, fields));
    }

    /// Field `i` of the open tab as live draws it: Wt dropdowns 141 px with
    /// a ▾ list (the current value), spinboxes 169 px with spin arrows (↑ /
    /// ↓ step, pale orange "Must be a number" when invalid), the unit as a
    /// label beside; texts 160 px.
    fn wt_field(&self, i: usize, ed: &Entity<Self>) -> AnyElement {
        use crate::drawing_wt::{INVALID_BG, INVALID_TIP, show, show_dec, spin_step};
        let Some(f) = self.drawing_props.inspection_inputs.as_ref().and_then(|(_, f)| f.get(i)) else { return div().into_any_element() };
        // Show `v` (display units) in the field and apply it.
        let apply = {
            let (ed, st, val, set, inv) = (ed.clone(), f.state.clone(), f.value.clone(), f.set, f.invalid.clone());
            let k = if self.drawing.units_mm { 1.0 } else { 25.4 };
            let decimals = match f.kind {
                FieldKind::Spin { decimals, .. } => decimals,
                _ => 4,
            };
            std::rc::Rc::new(move |v: f64, window: &mut Window, cx: &mut App| {
                st.update(cx, |s, cx| s.set_value(show_dec(v, decimals), window, cx));
                val.set(v * k);
                inv.set(false);
                if let Some(set) = set {
                    ed.update(cx, |this, cx| this.set_drawing_prop(&move |p| set(p, v * k), cx));
                }
            })
        };
        // Invalid spinbox: the pale orange fills the field itself.
        let input = Input::new(&f.state).text_size(px(13.33)).small().text_color(gpui_kit::rgb(0x555555)).pl(px(5.)).when(f.invalid.get(), |i| i.bg(gpui_kit::rgba(INVALID_BG)));
        let unit = div().w(px(24.)).flex_none().text_size(px(13.33)).text_color(gpui_kit::rgb(0x333333)).opacity(0.65).flex().justify_center().child(if self.drawing.units_mm { "mm" } else { "in" });
        let k = if self.drawing.units_mm { 1.0 } else { 25.4 };
        match f.kind {
            FieldKind::Dropdown => {
                // ▾: the preset heights (as the Styles panel's list).
                let val = f.value.clone();
                // Live: the ▾ sits inside the field's right padding (20 px).
                let caret = div().id(SharedString::from(format!("dp-wt-dd-{i}"))).w(px(18.)).h(px(24.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("ui-dropdown-arrow", 10., gpui_kit::rgb(0x555555)));
                let caret = caret.os_dropdown_menu(SharedString::from(format!("dp-wt-ddm-{i}")), false, move |menu, _, _| {
                    // Live: the list holds only the current value.
                    let v = val.get() / k;
                    let apply = apply.clone();
                    menu.item(crate::os_menu::OsMenuItem::new(show(v)).checked(true).on_click(move |_, window, cx| apply(v, window, cx)))
                });
                h_flex().gap(px(6.)).child(div().w(px(141.)).relative().child(input).child(caret.absolute().right(px(1.)).top(px(0.)))).child(unit).into_any_element()
            }
            FieldKind::Spin { step_in, step_mm, .. } => {
                let step_by = if self.drawing.units_mm { step_mm } else { step_in };
                let invalid = f.invalid.get();
                let step = {
                    let (st, val) = (f.state.clone(), f.value.clone());
                    std::rc::Rc::new(move |up: bool, window: &mut Window, cx: &mut App| apply(spin_step(&st.read(cx).value(), val.get() / k, up, step_by), window, cx))
                };
                let arrow = |id: String, up: bool| {
                    let step = step.clone();
                    div().id(SharedString::from(id)).h(px(12.)).w(px(16.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph(if up { "ui-caret-up" } else { "ui-caret-down" }, 8., gpui_kit::rgb(0x555555))).on_click(move |_, window, cx| step(up, window, cx))
                };
                let (step_up, step_dn) = (step.clone(), step.clone());
                let boxed = div()
                    .id(SharedString::from(format!("dp-wt-spin-{i}")))
                    .w(px(169.))
                    .relative()
                    .when(invalid, |d| d.bg(gpui_kit::rgba(INVALID_BG)).tooltip(crate::ui::tooltip(INVALID_TIP)))
                    .capture_action(move |_: &gpui_kit::component::input::MoveUp, window, cx| {
                        step_up(true, window, cx);
                        cx.stop_propagation();
                    })
                    .capture_action(move |_: &gpui_kit::component::input::MoveDown, window, cx| {
                        step_dn(false, window, cx);
                        cx.stop_propagation();
                    })
                    .child(input)
                    .child(v_flex().absolute().right(px(2.)).top(px(1.)).child(arrow(format!("dp-wt-up-{i}"), true)).child(arrow(format!("dp-wt-dn-{i}"), false)));
                h_flex().gap(px(6.)).child(boxed).child(unit).into_any_element()
            }
            FieldKind::Text => div().w(px(160.)).child(input).into_any_element(),
        }
    }

    /// A field that lost the focus (Tab, a click elsewhere) commits like
    /// Enter (live: "Tab commits, away commits"). Checked at each render:
    /// these panel inputs get no blur event.
    fn track_field_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, fields)) = &self.drawing_props.inspection_inputs else { return };
        let now = fields.iter().position(|f| f.state.read(cx).focus_handle(cx).is_focused(window));
        let before = std::mem::replace(&mut self.drawing_props.focused, now);
        let Some(i) = before.filter(|i| Some(*i) != now) else { return };
        let Some(f) = fields.get(i) else { return };
        let (st, kind, val, inv, set, set_text) = (f.state.clone(), f.kind, f.value.clone(), f.invalid.clone(), f.set, f.set_text);
        match (set, set_text) {
            (Some(set), _) => commit_number(self, &FieldCommit { st, kind, val, inv, set }, !self.drawing.units_mm, window, cx),
            (None, Some(set)) => {
                let v = st.read(cx).value().to_string();
                self.set_drawing_prop(&move |p| set(p, v.clone()), cx);
            }
            _ => {}
        }
    }

    fn set_drawing_prop(&mut self, f: &dyn Fn(&mut DrawingProps), cx: &mut Context<Self>) {
        // ☑ Lock drawing properties: only the lock itself changes; the
        // metric flag follows the chosen units.
        self.edit("Drawing properties", cx, |d| noshape_scene::drawing_tools::set_drawing_props(d, f));
    }

    pub fn render_drawing_props(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.drawing_props.open {
            return None;
        }
        let pal = p();
        let ed = cx.entity();
        let props = self.drawing.props.clone();
        let units = props.units.unwrap_or(if self.drawing.units_mm { DimUnits::Millimeter } else { DimUnits::Inch });
        let select = |id: &'static str, current: String, items: Vec<(String, bool, Act)>| {
            let ed = ed.clone();
            Button::new(id)
                .label(current)
                .ghost()
                .small()
                .w(px(160.))
                .dropdown_caret(true)
                .border_b_1()
                .border_color(gpui_kit::rgb(0xdddddd))
                .rounded(px(0.))
                .os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut menu, _, _| {
                    for (label, checked, f) in &items {
                        let (e, f) = (ed.clone(), f.clone());
                        menu = menu.item(crate::os_menu::OsMenuItem::new(label.clone()).checked(*checked).on_click(move |_, _, cx| e.update(cx, |this, cx| this.set_drawing_prop(&*f, cx))));
                    }
                    menu
                })
                .into_any_element()
        };
        // Color swatch: its color and name; a click opens the color dialog.
        let swatch = |id: &'static str, cur: Option<[u8; 3]>, set: crate::drawing_color::ColorSet| -> AnyElement {
            let ed = ed.clone();
            let c = cur.unwrap_or([0, 0, 0]);
            let name = COLORS.iter().find(|x| x.1 == cur).map(|x| x.0.to_string()).unwrap_or_else(|| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]));
            h_flex()
                .id(id)
                .w(px(160.))
                .gap(px(6.))
                .cursor_pointer()
                .child(div().w(px(28.)).h(px(16.)).bg(gpui_kit::rgb(((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32)).border_1().border_color(gpui_kit::rgb(0x999999)))
                .child(div().text_size(px(12.)).child(name))
                .on_click(move |_, w, cx| {
                    let set = set.clone();
                    ed.update(cx, |t, cx| t.open_color_dialog(cur, set, w, cx))
                })
                .into_any_element()
        };
        // Font row: family, Bold, Italic (the default is the drawing sans).
        let font_row = |w: FontOf, cur: Option<noshape_model::drawing::sheet::TextFont>, cx: &mut Context<Self>| -> AnyElement {
            use noshape_model::drawing::sheet::{FONT_FAMILIES, TextFont};
            let label = cur.as_ref().map(|f| f.family.clone()).unwrap_or_else(|| "Default".into());
            let mut items: Vec<(String, bool, Act)> = vec![("Default".into(), cur.is_none(), std::rc::Rc::new(move |p: &mut DrawingProps| *font_of(p, w) = None) as Act)];
            for fam in FONT_FAMILIES {
                let on = cur.as_ref().is_some_and(|f| f.family == fam);
                items.push((fam.to_string(), on, std::rc::Rc::new(move |p: &mut DrawingProps| {
                    let f = font_of(p, w);
                    let (b, i) = f.as_ref().map(|f| (f.bold, f.italic)).unwrap_or((false, false));
                    *f = Some(TextFont { family: fam.into(), bold: b, italic: i });
                }) as Act));
            }
            let toggle = |id: &'static str, glyph: &'static str, on: bool, f: fn(&mut TextFont), cx: &mut Context<Self>| {
                div()
                    .id(id)
                    .size(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(13.))
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .rounded(px(2.))
                    .when(on, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                    .child(glyph)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_drawing_prop(&move |p| {
                            let slot = font_of(p, w);
                            let mut font = slot.clone().unwrap_or_else(|| TextFont { family: FONT_FAMILIES[0].into(), bold: false, italic: false });
                            f(&mut font);
                            *slot = Some(font);
                        }, cx)
                    }))
            };
            let (b, i) = cur.as_ref().map(|f| (f.bold, f.italic)).unwrap_or((false, false));
            let id = match w {
                FontOf::Dims => ("dp-dfont", "dp-dfont-b", "dp-dfont-i"),
                FontOf::Inspection => ("dp-ifont", "dp-ifont-b", "dp-ifont-i"),
                FontOf::Type(t) => {
                    let ids = TYPE_IDS[AnnType::ALL.iter().position(|x| *x == t).unwrap_or(0)];
                    (ids[0], ids[1], ids[2])
                }
            };
            h_flex()
                .gap(px(2.))
                .child(select(id.0, label, items))
                .child(toggle(id.1, "B", b, |f| f.bold = !f.bold, cx))
                .child(toggle(id.2, "I", i, |f| f.italic = !f.italic, cx))
                .into_any_element()
        };
        // Live dom drawing-properties-*: labels 14 px in, 13.33 px #666; group
        // headers 30 px `#efefef` with a `#ddd` rule, 500 #333.
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(33.)).pl(px(14.)).pr(px(12.)).child(div().flex_1().text_size(px(13.33)).text_color(gpui_kit::rgb(0x666666)).child(label)).child(body).into_any_element();
        let section = |title: &'static str| div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(gpui_kit::rgb(0xefefef)).border_t_1().border_color(gpui_kit::rgb(0xdddddd)).text_size(px(13.33)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).mb(px(9.)).child(title).into_any_element();
        let precision_items = |cur: Option<u8>, max: usize, set: fn(&mut DrawingProps, Option<u8>)| -> Vec<(String, bool, Act)> {
            (0..=max).map(|k| (precision_label(k), cur == Some(k as u8), std::rc::Rc::new(move |p: &mut DrawingProps| set(p, Some(k as u8))) as Act)).collect()
        };
        let shown = |cur: Option<u8>, default: usize| precision_label(cur.map(|c| c as usize).unwrap_or(default));
        let mut body = v_flex().id("dr-dprops-body").flex_1().min_h_0().overflow_y_scroll();
        if self.drawing_props.tab == 0 {
            let unit_items: Vec<(String, bool, Act)> = DimUnits::ALL
                .iter()
                .map(|&u| {
                    let label = u.name();
                    (label.to_string(), u == units, std::rc::Rc::new(move |p: &mut DrawingProps| p.units = Some(u)) as Act)
                })
                .collect();
            let unit_label = unit_items.iter().find(|i| i.1).map(|i| i.0.clone()).unwrap_or_default();
            let sep_items: Vec<(String, bool, Act)> = vec![
                ("Period".into(), !props.decimal_comma, std::rc::Rc::new(|p: &mut DrawingProps| p.decimal_comma = false) as Act),
                ("Comma".into(), props.decimal_comma, std::rc::Rc::new(|p: &mut DrawingProps| p.decimal_comma = true) as Act),
            ];
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(37.)).px(px(10.)).flex().items_center().child(Checkbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| f(p, v), cx);
                })))
            };
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).child("Units and precision"))
                .child(section("Primary"))
                .child(row("Units", select("dp-units", unit_label, unit_items)))
                .child(row("Decimal separator", select("dp-sep", if props.decimal_comma { "Comma".into() } else { "Period".into() }, sep_items)))
                .child(row("Precision", select("dp-prec", shown(props.precision, default_digits(units)), precision_items(props.precision, 6, |p, v| p.precision = v))))
                .child(row("Tolerance precision", select("dp-tprec", shown(props.tolerance_precision.or(props.precision), default_digits(units)), precision_items(props.tolerance_precision, 6, |p, v| p.tolerance_precision = v))))
                .child(row("Angular precision", select("dp-aprec", shown(props.angular_precision, 2), precision_items(props.angular_precision, 4, |p, v| p.angular_precision = v))))
                .child(row("Angular tolerance precision", select("dp-atprec", shown(props.angular_tolerance_precision, 2), precision_items(props.angular_tolerance_precision, 4, |p, v| p.angular_tolerance_precision = v))))
                .child(row("Fit class tolerance precision", select("dp-fprec", shown(props.fit_tolerance_precision, 4), precision_items(props.fit_tolerance_precision, 6, |p, v| p.fit_tolerance_precision = v))))
                .child(section("Dual"))
                .child(check("dp-dual", "Show dual dimensions", props.dual.show, |p, v| p.dual.show = v, cx))
                .child(check("dp-dual-unit", "Show dual unit", props.dual.show_unit, |p, v| p.dual.show_unit = v, cx))
                .child(row("Dimension location", select("dp-dual-dloc", props.dual.dim_location.name().into(), noshape_model::drawing::DualLocation::ALL.iter().map(|&l| (l.name().to_string(), l == props.dual.dim_location, std::rc::Rc::new(move |p: &mut DrawingProps| p.dual.dim_location = l) as Act)).collect())))
                .child(row("Hole/thread callout location", select("dp-dual-hloc", props.dual.hole_location.name().into(), noshape_model::drawing::DualLocation::ALL.iter().map(|&l| (l.name().to_string(), l == props.dual.hole_location, std::rc::Rc::new(move |p: &mut DrawingProps| p.dual.hole_location = l) as Act)).collect())))
                .child(row("Units", select("dp-dual-units", props.dual.units_for(self.drawing.units_mm).name().into(), DimUnits::ALL.iter().map(|&u| (u.name().to_string(), u == props.dual.units_for(self.drawing.units_mm), std::rc::Rc::new(move |p: &mut DrawingProps| p.dual.units = Some(u)) as Act)).collect())))
                .child(row("Precision", select("dp-dual-prec", shown(props.dual.precision, 2), precision_items(props.dual.precision, 6, |p, v| p.dual.precision = v))))
                .child(row("Tolerance precision", select("dp-dual-tprec", shown(props.dual.tolerance_precision, 2), precision_items(props.dual.tolerance_precision, 6, |p, v| p.dual.tolerance_precision = v))))
                .child(section("Leading and trailing zeros"))
                .child(check("dp-lz", "Length leading zeros", props.leading_zeros, |p, v| p.leading_zeros = v, cx))
                .child(check("dp-tz", "Length trailing zeros", props.trailing_zeros, |p, v| p.trailing_zeros = v, cx))
                .child(check("dp-tlz", "Length tolerance leading zeros", props.tolerance_leading_zeros, |p, v| p.tolerance_leading_zeros = v, cx))
                .child(check("dp-ttz", "Length tolerance trailing zeros", props.tolerance_trailing_zeros, |p, v| p.tolerance_trailing_zeros = v, cx));
        } else if self.drawing_props.tab == 1 {
            // Dimensions › All dimensions, Gaps and extensions.
            let field = |i: usize| self.wt_field(i, &ed);
            let arrow_items: Vec<(String, bool, Act)> = noshape_model::drawing::sheet::ArrowType::ALL
                .iter()
                .map(|&t| (t.name().to_string(), t == props.dim_arrow_type, std::rc::Rc::new(move |p: &mut DrawingProps| p.dim_arrow_type = t) as Act))
                .collect();
            let horiz = props.dim_text_horizontal;
            // Live order: Align with dimension line | Horizontal.
            let align_items: Vec<(String, bool, Act)> = vec![
                ("Align with dimension line".into(), !horiz, std::rc::Rc::new(|p: &mut DrawingProps| p.dim_text_horizontal = false) as Act),
                ("Horizontal".into(), horiz, std::rc::Rc::new(|p: &mut DrawingProps| p.dim_text_horizontal = true) as Act),
            ];
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(37.)).px(px(10.)).flex().items_center().child(Checkbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| f(p, v), cx);
                })))
            };
            let just_items: Vec<(String, bool, Act)> = noshape_model::drawing::TextJustify::ALL.iter().map(|&j| (j.name().to_string(), j == props.dim_justify, std::rc::Rc::new(move |p: &mut DrawingProps| p.dim_justify = j) as Act)).collect();
            let (fit_ansi, ord_ansi, radial_lines) = (self.drawing.dim_fit_ansi(), self.drawing.ordinate_ansi(), self.drawing.radial_dim_lines());
            // ISO | ANSI (live order).
            let std_items = |ansi: bool, set: fn(&mut DrawingProps, bool)| -> Vec<(String, bool, Act)> {
                [("ISO", false), ("ANSI", true)].iter().map(|&(n, v)| (n.to_string(), ansi == v, std::rc::Rc::new(move |p: &mut DrawingProps| set(p, v)) as Act)).collect()
            };
            let std_name = |ansi: bool| if ansi { "ANSI".to_string() } else { "ISO".to_string() };
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).child("Dimensions"))
                .child(section("All dimensions"))
                .child(row("Font", font_row(FontOf::Dims, props.dim_font.clone(), cx)))
                .child(row("Text height", field(0)))
                .child(row("Color", swatch("dp-dcolor", props.dim_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.dim_color = c))))
                .child(row("Arrowhead", h_flex().gap(px(4.)).child(field(1)).child(select("dp-darrow", props.dim_arrow_type.name().into(), arrow_items)).into_any_element()))
                .child(row("Text justification", select("dp-djust", props.dim_justify.name().into(), just_items)))
                .child(check("dp-dunder", "Underline overridden dimensions", props.underline_overridden, |p, v| p.underline_overridden = v, cx))
                .child(check("dp-dbreak", "Break dimension by default", props.break_dims, |p, v| p.break_dims = v, cx))
                .child(row("Fit standard", select("dp-dfit", std_name(fit_ansi), std_items(fit_ansi, |p, v| p.dim_fit_ansi = Some(v)))))
                .child(section("Gaps and extensions"))
                .child(row("Text alignment", select("dp-dalign", if horiz { "Horizontal".into() } else { "Align with dimension line".into() }, align_items)))
                .child(row("Text gap", field(2)))
                .child(row("Geometry gap", field(3)))
                .child(row("Extension past line", field(4)))
                .child(check("dp-djog45", "Lock the angle of all jogged dimensions to 45 degrees", props.lock_jog_45, |p, v| p.lock_jog_45 = v, cx));
            // Chamfer: 45 degree style / content, the others', precisions.
            let style_items = |cur: bool, set: fn(&mut DrawingProps, bool)| -> Vec<(String, bool, Act)> {
                let opts = [("Note", false), ("Dimensions", true)];
                opts.iter().map(|&(n, v)| (n.to_string(), cur == v, std::rc::Rc::new(move |p: &mut DrawingProps| set(p, v)) as Act)).collect()
            };
            let content_items = |cur: ChamferContent, set: fn(&mut DrawingProps, ChamferContent), with_c: bool| -> Vec<(String, bool, Act)> {
                ChamferContent::ALL.iter().filter(|c| with_c || matches!(c, ChamferContent::LengthXAngle | ChamferContent::LengthXLength)).map(|&c| (c.name().to_string(), cur == c, std::rc::Rc::new(move |p: &mut DrawingProps| set(p, c)) as Act)).collect()
            };
            let style_name = |d: bool| if d { "Dimensions".to_string() } else { "Note".to_string() };
            body = body
                .child(section("Chamfer dimensions"))
                .child(row("45 degree style", select("dp-ch45s", style_name(props.chamfer_45_dimension), style_items(props.chamfer_45_dimension, |p, v| p.chamfer_45_dimension = v))))
                .child(row("45 degree content", select("dp-ch45c", props.chamfer_45_content.name().into(), content_items(props.chamfer_45_content, |p, v| p.chamfer_45_content = v, true))))
                .child(row("Non 45 degree style", select("dp-chs", style_name(props.chamfer_dimension), style_items(props.chamfer_dimension, |p, v| p.chamfer_dimension = v))))
                .child(row("Non 45 degree content", select("dp-chc", props.chamfer_content.name().into(), content_items(props.chamfer_content, |p, v| p.chamfer_content = v, false))))
                .child(row("Length precision", select("dp-chlp", shown(props.chamfer_length_precision, default_digits(units)), precision_items(props.chamfer_length_precision, 6, |p, v| p.chamfer_length_precision = v))))
                .child(row("Angle precision", select("dp-chap", shown(props.chamfer_angle_precision, 0), precision_items(props.chamfer_angle_precision, 4, |p, v| p.chamfer_angle_precision = v))))
                .child(section("Ordinate dimensions"))
                .child(row("Dimension style", select("dp-ordstd", std_name(ord_ansi), std_items(ord_ansi, |p, v| p.ordinate_ansi = Some(v)))))
                .child(section("Diameter and radial dimensions"))
                .child(check("dp-radlines", "Show dimension lines by default", radial_lines, |p, v| p.radial_dim_lines = Some(v), cx));
        } else if self.drawing_props.tab == 2 {
            // Annotations, in Onshape's groups: All annotations (Arrowhead),
            // then each type's font, text height and color with its extras.
            let field = |i: usize| self.wt_field(i, &ed);
            let arrow_items: Vec<(String, bool, Act)> = noshape_model::drawing::sheet::ArrowType::ALL
                .iter()
                .map(|&t| (t.name().to_string(), t == props.ann_arrow_type, std::rc::Rc::new(move |p: &mut DrawingProps| p.ann_arrow_type = t) as Act))
                .collect();
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(37.)).px(px(10.)).flex().items_center().child(Checkbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| f(p, v), cx);
                })))
            };
            // A select of named values.
            fn choices<T: Copy + PartialEq + 'static>(all: &[T], cur: T, name: fn(T) -> &'static str, set: fn(&mut DrawingProps, T)) -> Vec<(String, bool, Act)> {
                all.iter().map(|&v| (name(v).to_string(), v == cur, std::rc::Rc::new(move |p: &mut DrawingProps| set(p, v)) as Act)).collect()
            }
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).child("Annotations"))
                .child(section("All annotations"))
                .child(row("Arrowhead", h_flex().gap(px(4.)).child(field(1)).child(select("dp-aarrow", props.ann_arrow_type.name().into(), arrow_items)).into_any_element()));
            for (i, t) in AnnType::ALL.iter().copied().enumerate() {
                let ts = props.ann_type(t);
                let ids = TYPE_IDS[i];
                let color = row("Color", swatch(ids[3], ts.color, std::rc::Rc::new(move |p: &mut DrawingProps, c| p.ann_type_mut(t).color = c)));
                body = body.child(section(t.name()));
                if t == AnnType::WeldSymbols {
                    // The weld standard (separate from the drawing's; it
                    // follows the drawing's until set).
                    let ansi = self.drawing.weld_ansi();
                    let items: Vec<(String, bool, Act)> = [("ANSI", true), ("ISO", false)].iter().map(|&(n, v)| (n.to_string(), ansi == v, std::rc::Rc::new(move |p: &mut DrawingProps| p.weld_ansi = Some(v)) as Act)).collect();
                    body = body.child(row("Standard", select("dp-weld-std", if ansi { "ANSI".into() } else { "ISO".into() }, items)));
                }
                body = body.child(row("Font", font_row(FontOf::Type(t), ts.font.clone(), cx)));
                if t == AnnType::FlagNotes {
                    let shapes = choices(&noshape_model::drawing::note::FlagShape::ALL, props.flag_shape, |s| s.name(), |p, v| p.flag_shape = v);
                    body = body.child(row("Border shape", select("dp-flag-shape", props.flag_shape.name().into(), shapes))).child(color);
                    continue;
                }
                body = body.child(row("Text height", field(2 + i)));
                if t == AnnType::Datums {
                    body = body.child(row("Datum size", field(2 + AnnType::ALL.len())));
                }
                body = body.child(color);
                match t {
                    AnnType::Notes => {
                        body = body
                            .child(check("dp-hide-ph", "Hide property placeholder", props.hide_placeholder, |p, v| p.hide_placeholder = v, cx))
                            .child(check("dp-hide-pbg", "Hide property background", props.hide_property_background, |p, v| p.hide_property_background = v, cx));
                    }
                    AnnType::Callouts => {
                        let just = choices(&TextJustify::ALL, props.callout_justify, |j| j.name(), |p, v| p.callout_justify = v);
                        let stack = choices(&StackDirection::ALL, props.callout_stack, |d| d.name(), |p, v| p.callout_stack = v);
                        body = body
                            .child(row("Text justification", select("dp-co-just", props.callout_justify.name().into(), just)))
                            .child(row("Stack direction", select("dp-co-stack", props.callout_stack.name().into(), stack)))
                            .child(row("Stack gap size", field(4 + AnnType::ALL.len())))
                            .child(section("Center of mass"))
                            .child(row("Diameter size", field(3 + AnnType::ALL.len())));
                    }
                    AnnType::Datums => {
                        use noshape_model::drawing::hatch_pat::{GROUPS, display_order, pattern};
                        let (std, pat) = props.target_hatch;
                        let stds: Vec<(String, bool, Act)> = GROUPS.iter().enumerate().map(|(k, (n, _))| (n.to_string(), k == std, std::rc::Rc::new(move |p: &mut DrawingProps| p.target_hatch = (k, display_order(k)[0])) as Act)).collect();
                        // ANSI: the eight patterns, without the dialog's "(drawing)" one.
                        let name = |s: usize, k: usize| pattern(s, k).map(|p| p.name).filter(|_| s != 0 || k < 8);
                        let pats: Vec<(String, bool, Act)> = display_order(std).into_iter().filter_map(|k| Some((name(std, k)?.to_string(), k == pat, std::rc::Rc::new(move |p: &mut DrawingProps| p.target_hatch.1 = k) as Act))).collect();
                        let std_name = GROUPS.get(std).map(|s| s.0).unwrap_or("ANSI");
                        let pat_name = name(std, pat).unwrap_or("");
                        body = body
                            .child(row("Target hatch pattern", select("dp-th-std", std_name.into(), stds)))
                            .child(row("", select("dp-th-pat", pat_name.into(), pats)));
                    }
                    AnnType::HoleCallouts => {
                        let ind: Vec<(String, bool, Act)> = HOLE_INDICATORS.iter().map(|&n| (n.to_string(), props.hole_indicator == n, std::rc::Rc::new(move |p: &mut DrawingProps| p.hole_indicator = n.into()) as Act)).collect();
                        let fit: Vec<(String, bool, Act)> = [("ISO", true), ("ANSI", false)].iter().map(|&(n, v)| (n.to_string(), props.fit_iso == v, std::rc::Rc::new(move |p: &mut DrawingProps| p.fit_iso = v) as Act)).collect();
                        body = body
                            .child(row("Indicator", select("dp-hc-ind", props.hole_indicator.clone(), ind)))
                            .child(row("Fit standard", select("dp-hc-fit", if props.fit_iso { "ISO".into() } else { "ANSI".into() }, fit)));
                    }
                    _ => {}
                }
            }
        } else if self.drawing_props.tab == 3 {
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(37.)).px(px(10.)).flex().items_center().child(Checkbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| f(p, v), cx);
                })))
            };
            let vd = props.view_defaults.clone();
            let proj = self.drawing.projection;
            let ed2 = ed.clone();
            let proj_btn = Button::new("dp-proj")
                .label(if proj == Projection::ThirdAngle { "Third angle" } else { "First angle" })
                .ghost()
                .small()
                .w(px(160.))
                .dropdown_caret(true)
                .border_b_1()
                .border_color(gpui_kit::rgb(0xdddddd))
                .rounded(px(0.))
                .os_dropdown_menu("dp-projection-dd", true, move |m, _, _| {
                    let (a, b) = (ed2.clone(), ed2.clone());
                    m.item(crate::os_menu::OsMenuItem::new("Third angle").checked(proj == Projection::ThirdAngle).on_click(move |_, _, cx| a.update(cx, |t, cx| t.edit("Projection angle", cx, |d| d.projection = Projection::ThirdAngle))))
                        .item(crate::os_menu::OsMenuItem::new("First angle").checked(proj == Projection::FirstAngle).on_click(move |_, _, cx| b.update(cx, |t, cx| t.edit("Projection angle", cx, |d| d.projection = Projection::FirstAngle))))
                })
                .into_any_element();
            let simp: [(&str, Simplification); 5] = [("None", Simplification::None), ("Auto", Simplification::Auto), ("Absolute", Simplification::Absolute(0.001)), ("Ratio to studio", Simplification::RatioToStudio(0.01)), ("Ratio to part", Simplification::RatioToPart(0.01))];
            let simp_label = simp.iter().find(|s| std::mem::discriminant(&s.1) == std::mem::discriminant(&vd.simplification)).map(|s| s.0).unwrap_or("Auto");
            let simp_items: Vec<(String, bool, Act)> = simp.iter().map(|&(n, s)| (n.to_string(), n == simp_label, std::rc::Rc::new(move |p: &mut DrawingProps| p.view_defaults.simplification = s) as Act)).collect();
            let hidden_items: Vec<(String, bool, Act)> = vec![
                ("Hidden".into(), !vd.hidden_lines, std::rc::Rc::new(|p: &mut DrawingProps| p.view_defaults.hidden_lines = false) as Act),
                ("Shown".into(), vd.hidden_lines, std::rc::Rc::new(|p: &mut DrawingProps| p.view_defaults.hidden_lines = true) as Act),
            ];
            let tangent_items: Vec<(String, bool, Act)> = ["Hidden", "Solid", "Phantom"].iter().enumerate().map(|(i, n)| (n.to_string(), vd.tangent == i as u8, std::rc::Rc::new(move |p: &mut DrawingProps| p.view_defaults.tangent = i as u8) as Act)).collect();
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).child("Views"))
                .child(section("All views"))
                .child(row("Projection angle", proj_btn))
                .child(section("Insert view defaults"))
                .child(row("View simplification", select("dp-vsimp", simp_label.into(), simp_items)))
                .child(check("dp-vcm", "Auto centermarks (Parts)", vd.auto_centermarks, |p, v| p.view_defaults.auto_centermarks = v, cx))
                .child(check("dp-vcl", "Auto centerlines (Parts)", vd.auto_centerlines, |p, v| p.view_defaults.auto_centerlines = v, cx))
                .child(row("Hidden lines", select("dp-vhidden", if vd.hidden_lines { "Shown".into() } else { "Hidden".into() }, hidden_items)))
                .child(row("Tangent edges", select("dp-vtan", ["Hidden", "Solid", "Phantom"][vd.tangent.min(2) as usize].into(), tangent_items)))
                .child(check("dp-vscale", "Scale label", vd.scale_label, |p, v| p.view_defaults.scale_label = v, cx));
            // Per view type.
            let vt = props.view_tab.clone();
            let field = |i: usize| self.wt_field(i, &ed);
            let detail_lines: Vec<(String, bool, Act)> = ["Phantom", "Dashed", "Solid"].iter().enumerate().map(|(i, n)| (n.to_string(), vt.detail_line == i as u8, std::rc::Rc::new(move |p: &mut DrawingProps| p.view_tab.detail_line = i as u8) as Act)).collect();
            let detail_styles: Vec<(String, bool, Act)> = ["In line", "Connected", "Leader"].iter().enumerate().map(|(i, n)| (n.to_string(), vt.detail_style == i as u8, std::rc::Rc::new(move |p: &mut DrawingProps| p.view_tab.detail_style = i as u8) as Act)).collect();
            let aux_styles: Vec<(String, bool, Act)> = vec![
                ("A".into(), !vt.aux_double_label, std::rc::Rc::new(|p: &mut DrawingProps| p.view_tab.aux_double_label = false) as Act),
                ("A-A".into(), vt.aux_double_label, std::rc::Rc::new(|p: &mut DrawingProps| p.view_tab.aux_double_label = true) as Act),
            ];
            body = body
                .child(section("Hidden edges"))
                .child(row("Color", swatch("dp-hcolor", vt.hidden_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.hidden_color = c))))
                .child(section("Tangent edges"))
                .child(row("Color", swatch("dp-tcolor", vt.tangent_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.tangent_color = c))))
                .child(section("Section views"))
                .child(row("Label prefix", field(0)))
                .child(section("Detail views"))
                .child(row("Profile lines", select("dp-dline", ["Phantom", "Dashed", "Solid"][vt.detail_line.min(2) as usize].into(), detail_lines)))
                .child(row("Label prefix", field(1)))
                .child(row("Style", select("dp-dstyle", ["In line", "Connected", "Leader"][vt.detail_style.min(2) as usize].into(), detail_styles)))
                .child(check("dp-dprofile", "Show detail view profile", vt.detail_show_profile, |p, v| p.view_tab.detail_show_profile = v, cx))
                .child(check("dp-dclip", "Clip border to geometry", vt.detail_clip_border, |p, v| p.view_tab.detail_clip_border = v, cx))
                .child(section("Auxiliary views"))
                .child(check("dp-aind", "Show view reference indicator", vt.aux_indicator, |p, v| p.view_tab.aux_indicator = v, cx))
                .child(row("View reference format", div().into_any_element()))
                .child(self.aux_format_tiles(vt.aux_arrow_format, cx))
                .child(row("View label prefix", field(2)))
                .child(row("View label style", select("dp-alabel", if vt.aux_double_label { "A-A".into() } else { "A".into() }, aux_styles)))
                .child(row("View label suffix", field(3)));
            // Flat pattern views: up / down bend lines (style + color), bend tangent edges.
            const BEND_LINES: [&str; 3] = ["Solid", "Dashed", "Center"];
            let bend = |id: &'static str, cur: u8, set: fn(&mut DrawingProps, u8)| {
                let items: Vec<(String, bool, Act)> = BEND_LINES.iter().enumerate().map(|(i, n)| (n.to_string(), cur == i as u8, std::rc::Rc::new(move |p: &mut DrawingProps| set(p, i as u8)) as Act)).collect();
                select(id, BEND_LINES[cur.min(2) as usize].into(), items)
            };
            body = body
                .child(section("Flat pattern views"))
                .child(row("Up bend lines", bend("dp-bup", vt.bend_up_line, |p, v| p.view_tab.bend_up_line = v)))
                .child(row("", swatch("dp-bupc", vt.bend_up_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.bend_up_color = c))))
                .child(row("Down bend lines", bend("dp-bdown", vt.bend_down_line, |p, v| p.view_tab.bend_down_line = v)))
                .child(row("", swatch("dp-bdownc", vt.bend_down_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.bend_down_color = c))))
                .child(row("Bend tangent edges", swatch("dp-btanc", vt.bend_tangent_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.bend_tangent_color = c))));
        } else if self.drawing_props.tab == 4 {
            let vt = props.view_tab.clone();
            let field = |i: usize| self.wt_field(i, &ed);
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(37.)).px(px(10.)).flex().items_center().child(Checkbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| f(p, v), cx);
                })))
            };
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).child("Construction geometry"))
                // Live order: Mark size, Gap size, Color, Extended lines.
                .child(section("Centermarks"))
                .child(row("Mark size", field(0)))
                .child(row("Gap size", field(1)))
                .child(row("Color", swatch("dp-cmcolor", vt.centermark_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.centermark_color = c))))
                .child(check("dp-cmext", "Extended lines", vt.centermark_extended, |p, v| p.view_tab.centermark_extended = v, cx))
                .child(section("Centerlines"))
                .child(row("Color", swatch("dp-clcolor", vt.centerline_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.centerline_color = c))));
        } else if self.drawing_props.tab == 5 {
            let color_select = |id: &'static str, cur: Option<[u8; 3]>, set: fn(&mut DrawingProps, Option<[u8; 3]>)| swatch(id, cur, std::rc::Rc::new(move |p: &mut DrawingProps, c| set(p, c)));
            let locked = props.formats_locked;
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).child("Formats"))
                .child(div().h(px(37.)).px(px(10.)).flex().items_center().child(Checkbox::new("dp-flock").label("Locked").checked(locked).on_click(cx.listener(|this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| p.formats_locked = v, cx);
                }))))
                .child(section("Border frame"))
                .child(row("Color", color_select("dp-fborder", props.border_color, |p, c| p.border_color = c)))
                .child(section("Border zone"))
                .child(row("Color", color_select("dp-fzone", props.zone_color, |p, c| p.zone_color = c)))
                .child(section("Title block"))
                .child(row("Color", color_select("dp-ftitle", props.title_block_color, |p, c| p.title_block_color = c)));
        } else if self.drawing_props.tab == 6 {
            let field = |i: usize| self.wt_field(i, &ed);
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).child("Tables"))
                .child(section("All tables"))
                .child(row("Horizontal padding", field(0)))
                .child(row("Vertical padding", field(1)));
        } else if self.drawing_props.tab == 7 {
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(37.)).px(px(10.)).flex().items_center().child(Checkbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| f(p, v), cx);
                })))
            };
            let parse_items: Vec<(String, bool, Act)> = vec![
                ("Parameter".into(), !props.inspection_row_parsing, std::rc::Rc::new(|p: &mut DrawingProps| p.inspection_row_parsing = false) as Act),
                ("Row".into(), props.inspection_row_parsing, std::rc::Rc::new(|p: &mut DrawingProps| p.inspection_row_parsing = true) as Act),
            ];
            let shape_items: Vec<(String, bool, Act)> = InspectionShape::ALL.iter().map(|&s| (s.name().to_string(), s == props.inspection_shape, std::rc::Rc::new(move |p: &mut DrawingProps| p.inspection_shape = s) as Act)).collect();
            let border_items: Vec<(String, bool, Act)> = (0..=5u8).map(|n| (border_chars_label(n), n == props.inspection_border_chars, std::rc::Rc::new(move |p: &mut DrawingProps| p.inspection_border_chars = n) as Act)).collect();
            let field = |i: usize| self.wt_field(i, &ed);
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).child("Inspection"))
                // Live: one "Inspection items" group, then "Automatic inspection items".
                .child(section("Inspection items"))
                .child(row("Item parsing", select("dp-iparse", if props.inspection_row_parsing { "Row".into() } else { "Parameter".into() }, parse_items)))
                .child(row("Font", font_row(FontOf::Inspection, props.inspection_font.clone(), cx)))
                .child(row("Text", field(0)))
                .child(row("Border shape", select("dp-ishape", props.inspection_shape.name().into(), shape_items)))
                .child(row("Border size", select("dp-isize", border_chars_label(props.inspection_border_chars), border_items)))
                .child(row(
                    "Border color",
                    swatch("dp-icolor", props.inspection_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.inspection_color = c)),
                ))
                .child(check("dp-isolid", "Solid fill", props.inspection_solid_fill, |p, v| p.inspection_solid_fill = v, cx))
                .child(section("Automatic inspection items"))
                .child(check("dp-iexcl", "Exclude callouts", props.inspection_exclude_callouts, |p, v| p.inspection_exclude_callouts = v, cx));
        } else {
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).child(TABS[self.drawing_props.tab].0))
                .child(div().px(px(12.)).py_2().text_size(px(12.)).text_color(pal.text_muted).child("These properties are set per item for now (select a dimension, note or table)."));
        }
        // Live drawing/341: 38 px cells flush left, `#adadad` rules, `#f4f5f6`;
        // the active one white and open to the white section below.
        let mut tabs = h_flex().h(px(38.)).flex_none().mb(px(1.));
        for (i, (tip, icon)) in TABS.iter().enumerate() {
            let on = i == self.drawing_props.tab;
            tabs = tabs.child(
                div()
                    .id(("dr-dprops-tab", i))
                    .size(px(38.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_t_1()
                    .border_r_1()
                    .border_color(gpui_kit::rgb(0xadadad))
                    .cursor_pointer()
                    .when(on, |d| d.bg(crate::theme::light().panel))
                    .when(!on, |d| d.bg(gpui_kit::rgb(0xf4f5f6)).border_b_1().hover(|d| d.bg(gpui_kit::rgb(0xe8e9ea))))
                    .tooltip(crate::ui::tooltip(*tip))
                    .child(crate::assets::icon(&crate::assets::icon_path(icon), 20., cx))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.drawing_props.tab = i;
                        cx.notify();
                    })),
            );
        }
        Some(
            v_flex()
                .id("dr-drawing-props")
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .w(px(370.))
                .bg(gpui_kit::rgb(0xfafafa))
                .border_l_1()
                .border_color(pal.border)
                .text_color(pal.text)
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                // Live: the title centred, 18.67 px / 500 `#1b5faa` on a 25 px line
                // 5 px down, no close button (the right strip toggles the panel).
                .child(div().h(px(43.)).flex_none().pt(px(5.)).flex().justify_center().child(div().h(px(25.)).line_height(px(25.)).text_size(px(18.67)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x1b5faa)).child("Drawing properties")))
                .child(tabs)
                .child(body)
                .child(self.drawing_props_footer(cx))
                .into_any_element(),
        )
    }
}

impl DrawingEditor {
    /// The panel's footer (live drawing/18): "Update properties from a
    /// template…" with the DWT button (the standard templates), and
    /// ☐ Lock drawing properties.
    fn drawing_props_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let ed = cx.entity();
        let locked = self.drawing.props.locked;
        let dwt = Button::new("dp-dwt").ghost().small().w(px(25.)).h(px(25.)).p_0().border_1().border_color(gpui_kit::rgb(0xd3d7d9)).rounded(px(0.)).child(crate::assets::icon(&crate::assets::icon_path("dwt-template"), 20., cx)).tooltip("Select a DWT").os_dropdown_menu("dp-dwt-dd", true, move |mut m, _, _| {
            for row in noshape_ops::drawing::template_rows() {
                let e = ed.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(row.file).on_click(move |_, _, cx| {
                    e.update(cx, |t, cx| {
                        let props = noshape_model::drawing::templates::template_props(&row.template, row.file);
                        t.set_drawing_prop(&move |p| *p = props.clone(), cx)
                    })
                }));
            }
            m
        });
        v_flex()
            .flex_none()
            .border_t_1()
            .border_color(gpui_kit::rgb(0xdddddd))
            // Live drawing/160: an input-like row (13.33 px italic, 20 px in,
            // a #ddd underline up to the 25 px DWT button 9 px from the edge),
            // then the 12 px Lock checkbox 9 px in with a 12 px label and a
            // grey help icon at the right.
            .child(
                h_flex()
                    .h(px(34.))
                    .pt(px(6.))
                    .pr(px(9.))
                    .gap(px(19.))
                    .child(div().flex_1().h(px(26.)).pl(px(20.)).flex().items_center().border_b_1().border_color(gpui_kit::rgb(0xdddddd)).text_size(px(13.33)).italic().text_color(gpui_kit::rgb(0x666666)).child("Update properties from a template…"))
                    .child(dwt),
            )
            .child(
                h_flex()
                    .h(px(34.))
                    .pl(px(5.))
                    .pr(px(9.))
                    .child(div().flex_1().text_size(px(12.)).child(Checkbox::new("dp-lock").label("Lock drawing properties").small().checked(locked).on_click(cx.listener(|this, v: &bool, _, cx| {
                        let v = *v;
                        this.set_drawing_prop(&move |p| p.locked = v, cx)
                    }))))
                    .child(div().size(px(14.)).child(crate::assets::icon("help-filled", 14., cx))),
            )
            .into_any_element()
    }
}

impl DrawingEditor {
    /// Drawing properties › Auxiliary views › View reference format: two
    /// 96×70 tiles, "Line style" and "Arrow style" (live u-930/931; the
    /// pressed one rgb(207,219,229), the other rgb(245,245,245)).
    fn aux_format_tiles(&self, arrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let tile = |id: &'static str, tip: &'static str, icon: &'static str, on: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .w(px(96.))
                .h(px(70.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(gpui_kit::rgb(if on { 0xb9c6d1 } else { 0xd7d7d7 }))
                .bg(gpui_kit::rgb(if on { 0xcfdbe5 } else { 0xf5f5f5 }))
                .cursor_pointer()
                .tooltip(crate::ui::tooltip(tip))
                // Live (asb/u-930): the part + reference art on a 48 px canvas.
                .child(crate::assets::icon(icon, 48., cx))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let arrow = id == "dp-aformat-arrow";
                    this.set_drawing_prop(&move |p| p.view_tab.aux_arrow_format = arrow, cx);
                }))
        };
        h_flex()
            .pl(px(80.))
            .pb(px(8.))
            .gap(px(16.))
            .child(tile("dp-aformat-line", "Line style", "aux-format-line", !arrow, cx))
            .child(tile("dp-aformat-arrow", "Arrow style", "aux-format-arrow", arrow, cx))
            .into_any_element()
    }
}
