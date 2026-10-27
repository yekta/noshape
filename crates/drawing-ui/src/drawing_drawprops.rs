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
    /// dom/drawing-wt-widgets: gaps 4, table padding 3); `unitless`: a
    /// plain number (Dashes), no unit.
    Spin { step_in: f64, step_mm: f64, decimals: usize, unitless: bool },
    Text,
}

/// Millimetres per shown unit of a field of `kind` (lengths in the
/// drawing's units, unitless spinboxes as typed).
fn unit_k(kind: FieldKind, inch: bool) -> f64 {
    if inch && !matches!(kind, FieldKind::Spin { unitless: true, .. }) { 25.4 } else { 1.0 }
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
pub struct DrawingPropsUi {
    pub open: bool,
    pub tab: usize,
    /// The open tab's number fields: (tab, fields).
    pub inspection_inputs: Option<(usize, Vec<NumberField>)>,
    /// The field that had the focus at the last render.
    pub focused: Option<usize>,
    /// The body's scroll and whether it overflows (Chrome's scrollbar).
    pub scroll: ScrollHandle,
    pub overflow: std::rc::Rc<std::cell::Cell<bool>>,
    /// The number field whose ▾ list is open.
    pub wt_list: Option<usize>,
    /// The arrowhead picker whose list is open.
    pub arrow_menu: Option<&'static str>,
}

impl Default for DrawingPropsUi {
    fn default() -> Self {
        Self { open: false, tab: 0, inspection_inputs: None, focused: None, scroll: ScrollHandle::new(), overflow: Default::default(), wt_list: None, arrow_menu: None }
    }
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
/// Sets a line style (0 Solid, 1 Dashed, 2 Center).
type StyleSet = std::rc::Rc<dyn Fn(&mut DrawingProps, u8)>;

/// The panel's select caret (live drawing/1710): 8 px wide, about 5.5 px
/// down to its point, steeper than the menus' caret.
const SELECT_CARET: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="currentColor"><path d="M0 2h8L4 7.5z"/></svg>"#;
/// Its mirror, a spinbox's ▲.
const SELECT_CARET_UP: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="currentColor"><path d="M0 6h8L4 0.5z"/></svg>"#;

/// A select's field (live drawing/1710): white, 25 px high on a 1 px #ddd
/// line (164 px wide on Units and precision; the other tabs' widths follow
/// their table columns), the #555 value's ink 7 px in, an 8 px #777 caret
/// 6 px from the right.
fn select_field(id: &'static str, current: String, w: f32) -> Stateful<Div> {
    h_flex()
        .id(id)
        .w(px(w))
        .h(px(26.))
        .pl(px(6.))
        .pr(px(6.))
        .bg(gpui_kit::rgb(0xffffff))
        .border_b_1()
        .border_color(gpui_kit::rgb(0xdddddd))
        .text_size(px(13.33))
        .text_color(gpui_kit::rgb(0x555555))
        .cursor_pointer()
        .child(div().flex_1().min_w_0().truncate().child(current))
        .child(crate::os_ui::glyph(SELECT_CARET, 8., gpui_kit::rgb_static(0x777777)))
}

/// An arrowhead style as the Arrowhead control shows it (live drawing/1710):
/// an 18 × 9 px #333 glyph, a shaft from the left to the tip at the right
/// with the head 12 px long and 7 px wide.
fn arrowhead_glyph(t: noshape_model::drawing::sheet::ArrowType) -> impl IntoElement {
    use noshape_model::drawing::sheet::ArrowType;
    canvas(|_, _, _| {}, move |b, _, window, _| {
        let ink = gpui_kit::rgb_static(0x333333);
        let (l, y) = (b.left(), b.top() + px(4.5));
        let at = |x: f32, dy: f32| point(l + px(x), y + px(dy));
        let stroke = |window: &mut Window, pts: &[Point<Pixels>]| {
            let mut pb = PathBuilder::stroke(px(1.));
            pb.move_to(pts[0]);
            for q in &pts[1..] {
                pb.line_to(*q);
            }
            if let Ok(path) = pb.build() {
                window.paint_path(path, ink);
            }
        };
        let shaft_end = match t {
            ArrowType::Filled | ArrowType::Unfilled => 6.,
            ArrowType::FilledDot => 11.,
            _ => 17.,
        };
        stroke(window, &[at(0., 0.), at(shaft_end, 0.)]);
        match t {
            ArrowType::Filled => {
                let mut pb = PathBuilder::fill();
                pb.move_to(at(6., -3.5));
                pb.line_to(at(18., 0.));
                pb.line_to(at(6., 3.5));
                pb.close();
                if let Ok(path) = pb.build() {
                    window.paint_path(path, ink);
                }
            }
            ArrowType::Unfilled => stroke(window, &[at(6.5, -3.), at(17., 0.), at(6.5, 3.), at(6.5, -3.)]),
            ArrowType::Open => stroke(window, &[at(6., -3.5), at(17., 0.), at(6., 3.5)]),
            ArrowType::Oblique => stroke(window, &[at(13.5, 3.5), at(17.5, -3.5)]),
            ArrowType::FilledDot => {
                let mut pb = PathBuilder::fill();
                let c = at(14., 0.);
                pb.move_to(point(c.x + px(3.), c.y));
                pb.arc_to(point(px(3.), px(3.)), px(0.), false, true, point(c.x - px(3.), c.y));
                pb.arc_to(point(px(3.), px(3.)), px(0.), false, true, point(c.x + px(3.), c.y));
                pb.close();
                if let Ok(path) = pb.build() {
                    window.paint_path(path, ink);
                }
            }
            ArrowType::None => {}
        }
    })
    .w(px(18.))
    .h(px(9.))
}

/// Drawing properties › Views › Cosmetic threads: the Thread style's top
/// and side view previews, two 96 × 96 tiles 14 px apart (live 10-26 d:
/// pictures, not buttons; a 100-unit tile, white with a #333 0.5 frame).
fn thread_tiles(style: ThreadStyle) -> AnyElement {
    let tile = move |side: bool| {
        canvas(|_, _, _| {}, move |b, _, window, _| {
            let k = 0.96;
            // Tile units from the frame's outer corner (the canvas sits inside it).
            let at = |p: [f32; 2]| point(b.left() - px(1.) + px(p[0] * k), b.top() - px(1.) + px(p[1] * k));
            // A polyline (tile units) with an on/off dash pattern.
            let stroke = |window: &mut Window, pts: &[[f32; 2]], w: f32, dash: &[f32]| {
                let mut pieces: Vec<Vec<[f32; 2]>> = vec![vec![pts[0]]];
                let (mut k_dash, mut left, mut on) = (0, dash.first().copied().unwrap_or(f32::MAX), true);
                for s in pts.windows(2) {
                    let len = ((s[1][0] - s[0][0]).powi(2) + (s[1][1] - s[0][1]).powi(2)).sqrt();
                    let mut t = 0.0;
                    while len - t > left {
                        t += left;
                        let q = [s[0][0] + (s[1][0] - s[0][0]) * t / len, s[0][1] + (s[1][1] - s[0][1]) * t / len];
                        if on {
                            pieces.last_mut().expect("started").push(q);
                        } else {
                            pieces.push(vec![q]);
                        }
                        on = !on;
                        k_dash = (k_dash + 1) % dash.len().max(1);
                        left = dash.get(k_dash).copied().unwrap_or(f32::MAX);
                    }
                    left -= len - t;
                    if on {
                        pieces.last_mut().expect("started").push(s[1]);
                    }
                }
                // Live's hairlines: 1 px, grey by their coverage (0.5 units
                // ≈ rgb 155); straight ones on pixel centres.
                let ink = gpui_kit::rgba(((w * 0.8).clamp(0.0, 1.0) * 255.0) as u32);
                let snap = |v: Pixels| px(f32::from(v).floor() + 0.5);
                for piece in pieces.iter().filter(|p| p.len() >= 2) {
                    let mut pts: Vec<Point<Pixels>> = piece.iter().map(|q| at(*q)).collect();
                    if pts.len() == 2 && pts[0].y == pts[1].y {
                        let y = snap(pts[0].y);
                        pts.iter_mut().for_each(|q| q.y = y);
                    } else if pts.len() == 2 && pts[0].x == pts[1].x {
                        let x = snap(pts[0].x);
                        pts.iter_mut().for_each(|q| q.x = x);
                    }
                    let mut pb = PathBuilder::stroke(px(1.));
                    pb.move_to(pts[0]);
                    for q in &pts[1..] {
                        pb.line_to(*q);
                    }
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, ink);
                    }
                }
            };
            // Arc from `a0` to `a1` degrees, clockwise on screen from 3 o'clock.
            let arc = |r: f32, a0: f32, a1: f32| (0..=96).map(|i| (a0 + (a1 - a0) * i as f32 / 96.0).to_radians()).map(|t| [50.0 + r * t.cos(), 50.0 + r * t.sin()]).collect::<Vec<_>>();
            let ansi = style == ThreadStyle::Ansi;
            if side {
                for (y, w, dash) in [(39.75, 0.5, false), (36.75, 0.5, true), (61.75, 0.5, true), (58.75, 0.5, false)] {
                    let (y, w) = if dash && !ansi { (y + 0.125, 0.25) } else { (y, w) };
                    let pattern: &[f32] = if dash && ansi { &[2.0, 2.0] } else { &[] };
                    stroke(window, &[[0.0, y], [100.0, y]], w, pattern);
                }
            } else {
                match style {
                    ThreadStyle::Ansi => stroke(window, &arc(12.75, 0.0, 360.0), 0.5, &[2.0, 2.0]),
                    ThreadStyle::Iso => stroke(window, &arc(12.75, 175.5, 454.5), 0.5, &[]),
                    ThreadStyle::Jis => stroke(window, &arc(12.75, -4.5, 274.5), 0.5, &[]),
                }
                stroke(window, &arc(9.75, 0.0, 360.0), 0.5, &[]);
                stroke(window, &[[68.0, 50.25], [32.0, 50.25]], 0.5, &[6.0, 1.0, 2.0, 1.0]);
                stroke(window, &[[49.745, 67.995], [49.745, 31.995]], 0.5, &[6.0, 1.0, 2.0, 1.0]);
            }
        })
        .size_full()
    };
    let frame = |side: bool| div().w(px(96.)).h(px(96.)).flex_none().bg(gpui_kit::rgb(0xffffff)).border_1().border_color(gpui_kit::rgb(0x9b9b9b)).child(tile(side));
    h_flex().items_start().gap(px(14.)).child(frame(false)).child(frame(true)).into_any_element()
}

/// Which text a Font row sets.
#[derive(Clone, Copy)]
enum FontOf {
    Dims,
    Inspection,
    Type(AnnType),
    /// Views › All views › View labels.
    ViewLabel,
    /// Views › Section / Detail / Auxiliary views' labels.
    SectionLabel,
    DetailLabel,
    AuxLabel,
    /// Tables › kind `k`'s font, Hole tables' tags.
    Table(usize),
    HoleTags,
    /// Tables › Revision tables › Revision callouts.
    RevisionCallout,
}

fn font_of(p: &mut DrawingProps, w: FontOf) -> &mut Option<noshape_model::drawing::sheet::TextFont> {
    match w {
        FontOf::Dims => &mut p.dim_font,
        FontOf::Inspection => &mut p.inspection_font,
        FontOf::Type(t) => &mut p.ann_type_mut(t).font,
        FontOf::ViewLabel => &mut p.view_tab.view_label_font,
        FontOf::SectionLabel => &mut p.view_tab.section_label.font,
        FontOf::DetailLabel => &mut p.view_tab.detail_label.font,
        FontOf::AuxLabel => &mut p.view_tab.aux_label.font,
        FontOf::Table(k) => &mut table_style_mut(p, k).font,
        FontOf::HoleTags => &mut p.table_styles.hole_tags_font,
        FontOf::RevisionCallout => &mut p.table_styles.revision_callout_font,
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
    let k = unit_k(kind, inch);
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

/// Flat pattern views' stored line row `k` (style, colour), filled in with
/// live's defaults up to it.
fn flat_slot(p: &mut DrawingProps, k: usize) -> &mut (u8, Option<[u8; 3]>) {
    let rows = &mut p.view_tab.flat_lines;
    while rows.len() <= k {
        rows.push((FLAT_LINE_ROWS[rows.len()].1, None));
    }
    &mut rows[k]
}

/// Drawing properties › Tables' style of kind `k` (live's order: BOM,
/// Custom, Cut list, Hole, Revision tables, Tables).
fn table_style_mut(p: &mut DrawingProps, k: usize) -> &mut TableStyle {
    let s = &mut p.table_styles;
    match k {
        0 => &mut s.bom,
        1 => &mut s.custom,
        2 => &mut s.cut_list,
        3 => &mut s.hole,
        4 => &mut s.revision,
        _ => &mut s.general,
    }
}

/// A Tables row text: kind `k`'s header / content, 6 Hole tables' Tags,
/// 7 Revision callouts, 8 / 9 Revision tables' / Tables' title row.
fn row_text_mut(p: &mut DrawingProps, k: usize, header: bool) -> &mut RowText {
    match k {
        6 => &mut p.table_styles.hole_tags,
        7 => &mut p.table_styles.revision_callout_text,
        8 => &mut p.table_styles.revision.title_text,
        9 => &mut p.table_styles.general.title_text,
        _ => {
            let s = table_style_mut(p, k);
            if header { &mut s.header } else { &mut s.content }
        }
    }
}

fn set_table_header<const K: usize>(p: &mut DrawingProps, v: f64) {
    table_style_mut(p, K).header.height = Some(v);
}

fn set_table_content<const K: usize>(p: &mut DrawingProps, v: f64) {
    table_style_mut(p, K).content.height = Some(v);
}

fn set_table_title<const K: usize>(p: &mut DrawingProps, v: f64) {
    table_style_mut(p, K).title = Some(v);
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
        let make = move |v: f64, kind: FieldKind, set: fn(&mut DrawingProps, f64), window: &mut Window, cx: &mut Context<Self>| {
            let k = unit_k(kind, inch);
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
        let spin = move |v: f64, set: fn(&mut DrawingProps, f64), window: &mut Window, cx: &mut Context<Self>| make(v, FieldKind::Spin { step_in: 0.05, step_mm: 0.1, decimals: 4, unitless: false }, set, window, cx);
        let spin_fine = move |v: f64, set: fn(&mut DrawingProps, f64), window: &mut Window, cx: &mut Context<Self>| make(v, FieldKind::Spin { step_in: 0.05, step_mm: 0.05, decimals: 4, unitless: false }, set, window, cx);
        let spin3 = move |v: f64, set: fn(&mut DrawingProps, f64), window: &mut Window, cx: &mut Context<Self>| make(v, FieldKind::Spin { step_in: 0.05, step_mm: 0.1, decimals: 3, unitless: false }, set, window, cx);
        // Dash scales (live "1.00").
        let spin_scale = move |v: f64, set: fn(&mut DrawingProps, f64), window: &mut Window, cx: &mut Context<Self>| make(v, FieldKind::Spin { step_in: 0.1, step_mm: 0.1, decimals: 2, unitless: true }, set, window, cx);
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
                // 4: View labels' height; 5–7: Hidden / Tangent edges' and
                // Exploded lines' Dashes.
                dropdown(vt.view_label_height.unwrap_or(p.ann_text_height), |p, v| p.view_tab.view_label_height = Some(v), window, cx),
                spin_scale(vt.hidden_dashes, |p, v| p.view_tab.hidden_dashes = v, window, cx),
                spin_scale(vt.tangent_dashes, |p, v| p.view_tab.tangent_dashes = v, window, cx),
                spin_scale(vt.exploded_dashes, |p, v| p.view_tab.exploded_dashes = v, window, cx),
                // 8–13: Section / Detail / Auxiliary views' arrowhead size
                // and label height.
                dropdown(vt.section_arrow.size.unwrap_or(p.ann_arrow), |p, v| p.view_tab.section_arrow.size = Some(v), window, cx),
                dropdown(vt.section_label.height.unwrap_or(p.ann_text_height), |p, v| p.view_tab.section_label.height = Some(v), window, cx),
                dropdown(vt.detail_arrow.size.unwrap_or(p.ann_arrow), |p, v| p.view_tab.detail_arrow.size = Some(v), window, cx),
                dropdown(vt.detail_label.height.unwrap_or(p.ann_text_height), |p, v| p.view_tab.detail_label.height = Some(v), window, cx),
                dropdown(vt.aux_arrow.size.unwrap_or(p.ann_arrow), |p, v| p.view_tab.aux_arrow.size = Some(v), window, cx),
                dropdown(vt.aux_label.height.unwrap_or(noshape_model::drawing::build::AUX_LETTER), |p, v| p.view_tab.aux_label.height = Some(v), window, cx),
                // 14: Cosmetic threads' Dashes.
                spin_scale(vt.thread_dashes, |p, v| p.view_tab.thread_dashes = v, window, cx),
            ],
            4 => vec![
                spin(vt.centermark_mark_size, |p, v| p.view_tab.centermark_mark_size = v, window, cx),
                spin(vt.centermark_gap_size, |p, v| p.view_tab.centermark_gap_size = v, window, cx),
                // 2: Centerlines' Dashes.
                spin_scale(vt.centerline_dashes, |p, v| p.view_tab.centerline_dashes = v, window, cx),
            ],
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
                let ts = &p.table_styles;
                let styles = [&ts.bom, &ts.custom, &ts.cut_list, &ts.hole, &ts.revision, &ts.general];
                let header = |k: usize| styles[k].header.height.unwrap_or(p.ann_text_height);
                let content = |k: usize| styles[k].content.height.unwrap_or(p.ann_text_height);
                let title = |k: usize| styles[k].title.unwrap_or(content(k) * 1.3);
                // 2 + 2k / 3 + 2k: kind k's header / content row text; 14 /
                // 15: Revision tables' and Tables' title row text.
                vec![
                    spin3(h, |p, v| p.table_h_padding = v, window, cx),
                    spin3(v, |p, v| p.table_v_padding = v, window, cx),
                    dropdown(header(0), set_table_header::<0>, window, cx),
                    dropdown(content(0), set_table_content::<0>, window, cx),
                    dropdown(header(1), set_table_header::<1>, window, cx),
                    dropdown(content(1), set_table_content::<1>, window, cx),
                    dropdown(header(2), set_table_header::<2>, window, cx),
                    dropdown(content(2), set_table_content::<2>, window, cx),
                    dropdown(header(3), set_table_header::<3>, window, cx),
                    dropdown(content(3), set_table_content::<3>, window, cx),
                    dropdown(header(4), set_table_header::<4>, window, cx),
                    dropdown(content(4), set_table_content::<4>, window, cx),
                    dropdown(header(5), set_table_header::<5>, window, cx),
                    dropdown(content(5), set_table_content::<5>, window, cx),
                    dropdown(title(4), set_table_title::<4>, window, cx),
                    dropdown(title(5), set_table_title::<5>, window, cx),
                    // 16: Hole tables' Indicator arrowhead; 17 / 18: Tags' and
                    // Revision callouts' text height.
                    dropdown(ts.hole_indicator_arrow.unwrap_or(p.ann_arrow * 2.0 / 3.0), |p, v| p.table_styles.hole_indicator_arrow = Some(v), window, cx),
                    dropdown(ts.hole_tags.height.unwrap_or(p.ann_text_height), |p, v| p.table_styles.hole_tags.height = Some(v), window, cx),
                    dropdown(ts.revision_callout_text.height.unwrap_or(p.ann_text_height), |p, v| p.table_styles.revision_callout_text.height = Some(v), window, cx),
                ]
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
        self.wt_field_w(i, ed, None)
    }

    /// [`Self::wt_field`] `w` px wide (live's fields fill their table
    /// column; default 141 with a ▾, 169 as a spinbox).
    fn wt_field_w(&self, i: usize, ed: &Entity<Self>, w: Option<f32>) -> AnyElement {
        use crate::drawing_wt::{INVALID_BG, INVALID_TIP, show, show_dec, spin_step};
        let Some(f) = self.drawing_props.inspection_inputs.as_ref().and_then(|(_, f)| f.get(i)) else { return div().into_any_element() };
        // Show `v` (display units) in the field and apply it.
        let apply = {
            let (ed, st, val, set, inv) = (ed.clone(), f.state.clone(), f.value.clone(), f.set, f.invalid.clone());
            let k = unit_k(f.kind, !self.drawing.units_mm);
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
        // Live 1710: white like the selects, a 1 px #ddd line under it, the
        // #555 value's ink 7 px in (24 px high with a ▾, 26 as a spinbox).
        // Invalid spinbox: the pale orange fills the field itself.
        let field_h = if matches!(f.kind, FieldKind::Spin { .. }) { 26. } else { 24. };
        let input = Input::new(&f.state)
            .appearance(false)
            .small()
            .h(px(field_h))
            .pl(px(6.))
            .pr(px(if matches!(f.kind, FieldKind::Text) { 6. } else { 20. }))
            .bg(gpui_kit::rgb(0xffffff))
            .border_b_1()
            .border_color(gpui_kit::rgb(0xdddddd))
            // Focused (live 1731): a 2 px #1b5ea9 line instead.
            .when(self.drawing_props.focused == Some(i) || self.drawing_props.wt_list == Some(i), |d| d.border_b_2().border_color(gpui_kit::rgb(0x1b5ea9)))
            .text_size(px(13.33))
            .text_color(gpui_kit::rgb(0x555555))
            .when(f.invalid.get(), |i| i.bg(gpui_kit::rgba(INVALID_BG)));
        let unitless = matches!(f.kind, FieldKind::Spin { unitless: true, .. });
        let unit = (!unitless).then(|| div().w(px(24.)).flex_none().text_size(px(13.33)).text_color(gpui_kit::rgb(0x333333)).opacity(0.65).flex().justify_center().child(if self.drawing.units_mm { "mm" } else { "in" }));
        let k = unit_k(f.kind, !self.drawing.units_mm);
        match f.kind {
            FieldKind::Dropdown => {
                // ▾: the preset heights (as the Styles panel's list).
                let val = f.value.clone();
                // Live: the selects' ▾ 6 px from the field's right, inside
                // its right padding (20 px).
                // Live 1731: a click opens the field's list under it, 1 px
                // down and as wide as the field (white, a #ccc line and a
                // soft shadow), holding only the current value: a 21 px row,
                // #333 14 px text 7 px in. The field keeps a 2 px #1b5ea9 line
                // while it is open.
                let open = self.drawing_props.wt_list == Some(i);
                let caret = {
                    let ed = ed.clone();
                    div()
                        .id(SharedString::from(format!("dp-wt-dd-{i}")))
                        .w(px(20.))
                        .h(px(23.))
                        .pr(px(6.))
                        .flex()
                        .items_center()
                        .justify_end()
                        .cursor_pointer()
                        .child(crate::os_ui::glyph(SELECT_CARET, 8., gpui_kit::rgb_static(0x777777)))
                        .on_click(move |_, _, cx| {
                            ed.update(cx, |this, cx| {
                                this.drawing_props.wt_list = if open { None } else { Some(i) };
                                cx.notify();
                            })
                        })
                };
                let list = open.then(|| {
                    let v = val.get() / k;
                    let (ed, close) = (ed.clone(), ed.clone());
                    deferred(
                        div()
                            .id(SharedString::from(format!("dp-wt-list-{i}")))
                            .absolute()
                            .top(px(25.))
                            .left_0()
                            .w(px(w.unwrap_or(141.)))
                            .bg(gpui_kit::rgb(0xffffff))
                            .border_1()
                            .border_color(gpui_kit::rgb(0xcccccc))
                            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.18), offset: point(px(0.), px(3.)), blur_radius: px(6.), spread_radius: px(0.), inset: false }])
                            .occlude()
                            .on_mouse_down_out(move |_, _, cx| {
                                close.update(cx, |this, cx| {
                                    this.drawing_props.wt_list = None;
                                    cx.notify();
                                })
                            })
                            .child(
                                div()
                                    .id(SharedString::from(format!("dp-wt-item-{i}")))
                                    .h(px(21.))
                                    .pl(px(5.))
                                    .flex()
                                    .items_center()
                                    .text_size(px(14.))
                                    .text_color(gpui_kit::rgb(0x333333))
                                    .cursor_pointer()
                                    .child(show(v))
                                    .on_click(move |_, window, cx| {
                                        apply(v, window, cx);
                                        ed.update(cx, |this, cx| {
                                            this.drawing_props.wt_list = None;
                                            cx.notify();
                                        })
                                    }),
                            ),
                    )
                });
                h_flex()
                    .gap(px(6.))
                    .child(div().w(px(w.unwrap_or(141.))).relative().child(input).child(caret.absolute().right_0().top_0()).children(list))
                    .children(unit)
                    .into_any_element()
            }
            FieldKind::Spin { step_in, step_mm, .. } => {
                let step_by = if self.drawing.units_mm { step_mm } else { step_in };
                let invalid = f.invalid.get();
                let step = {
                    let (st, val) = (f.state.clone(), f.value.clone());
                    std::rc::Rc::new(move |up: bool, window: &mut Window, cx: &mut App| apply(spin_step(&st.read(cx).value(), val.get() / k, up, step_by), window, cx))
                };
                // Live: the select's ▾ and its mirror, 6 px from the right,
                // their bases 1 px apart at the field's middle.
                let arrow = |id: String, up: bool| {
                    let step = step.clone();
                    let glyph = crate::os_ui::glyph(if up { SELECT_CARET_UP } else { SELECT_CARET }, 8., gpui_kit::rgb_static(0x777777));
                    div().id(SharedString::from(id)).absolute().right_0().top(px(if up { 0. } else { 12.5 })).h(px(12.5)).w(px(20.)).cursor_pointer().child(div().absolute().right(px(6.)).top(px(if up { 5.5 } else { -1.5 })).child(glyph)).on_click(move |_, window, cx| step(up, window, cx))
                };
                let (step_up, step_dn) = (step.clone(), step.clone());
                let boxed = div()
                    .id(SharedString::from(format!("dp-wt-spin-{i}")))
                    .w(px(w.unwrap_or(169.)))
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
                    .child(arrow(format!("dp-wt-up-{i}"), true))
                    .child(arrow(format!("dp-wt-dn-{i}"), false));
                h_flex().gap(px(6.)).child(boxed).children(unit).into_any_element()
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
        let sel = |id: &'static str, current: String, items: Vec<(String, bool, Act)>, w: f32| {
            let ed = ed.clone();
            select_field(id, current, w)
                .os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut menu, _, _| {
                    for (label, checked, f) in &items {
                        let (e, f) = (ed.clone(), f.clone());
                        menu = menu.item(crate::os_menu::OsMenuItem::new(label.clone()).checked(*checked).on_click(move |_, _, cx| e.update(cx, |this, cx| this.set_drawing_prop(&*f, cx))));
                    }
                    menu
                })
                .into_any_element()
        };
        let select = |id: &'static str, current: String, items: Vec<(String, bool, Act)>| sel(id, current, items, 164.);
        // A pen select (live: 0.13 mm … 2.00 mm).
        let thick_sel = |id: &'static str, cur: f64, set: fn(&mut DrawingProps, f64), w: f32| {
            let items: Vec<(String, bool, Act)> = noshape_model::drawing::LINE_THICKNESSES.iter().map(|&t| (format!("{t:.2} mm"), (t - cur).abs() < 1e-6, std::rc::Rc::new(move |p: &mut DrawingProps| set(p, t)) as Act)).collect();
            sel(id, format!("{cur:.2} mm"), items, w)
        };
        // Color swatch (live drawing/1710): a 26 px square of the colour; a
        // click opens the color dialog.
        let swatch = |id: &'static str, cur: Option<[u8; 3]>, set: crate::drawing_color::ColorSet| -> AnyElement {
            let ed = ed.clone();
            let c = cur.unwrap_or([0, 0, 0]);
            div()
                .id(id)
                .size(px(26.))
                .cursor_pointer()
                .bg(gpui_kit::rgb_static(((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32))
                .on_click(move |_, w, cx| {
                    let set = set.clone();
                    ed.update(cx, |t, cx| t.open_color_dialog(cur, set, w, cx))
                })
                .into_any_element()
        };
        // Font select (the default is the drawing sans; live has no Bold /
        // Italic here, only in the note editor and the Views tab's labels).
        let font_row = |w: FontOf, cur: Option<noshape_model::drawing::sheet::TextFont>, width: f32| -> AnyElement {
            use noshape_model::drawing::sheet::{DRAWING_DEFAULT_FONT, FONT_FAMILIES, TextFont};
            // Live (dom/drawing-properties-annotations): "Noto Sans" of the 22
            // families; no separate Default entry.
            let label = cur.as_ref().map(|f| f.family.clone()).unwrap_or_else(|| DRAWING_DEFAULT_FONT.into());
            let mut items: Vec<(String, bool, Act)> = Vec::new();
            for fam in FONT_FAMILIES {
                let on = cur.as_ref().map_or(fam == DRAWING_DEFAULT_FONT, |f| f.family == fam);
                items.push((fam.to_string(), on, std::rc::Rc::new(move |p: &mut DrawingProps| {
                    let f = font_of(p, w);
                    let (b, i) = f.as_ref().map(|f| (f.bold, f.italic)).filter(|_| !noshape_model::drawing::sheet::is_shx(fam)).unwrap_or((false, false));
                    *f = Some(TextFont { family: fam.into(), bold: b, italic: i });
                }) as Act));
            }
            let id = match w {
                FontOf::Dims => "dp-dfont",
                FontOf::Inspection => "dp-ifont",
                FontOf::Type(t) => TYPE_IDS[AnnType::ALL.iter().position(|x| *x == t).unwrap_or(0)][0],
                FontOf::ViewLabel => "dp-vlfont",
                FontOf::SectionLabel => "dp-slfont",
                FontOf::DetailLabel => "dp-dlfont",
                FontOf::AuxLabel => "dp-alfont",
                FontOf::Table(k) => ["dp-tf0", "dp-tf1", "dp-tf2", "dp-tf3", "dp-tf4", "dp-tf5"][k.min(5)],
                FontOf::HoleTags => "dp-htags",
                FontOf::RevisionCallout => "dp-rcfont",
            };
            sel(id, label, items, width)
        };
        // Live dom drawing-properties-*: labels 13.33 px #666, their ink 15 px
        // in (1710); the field 2 px below the row's top, above the label's
        // centre (3 px of margin under it); group headers 30 px `#efefef`
        // with a `#ddd` rule, 500 #333, 8 px above their first row and 4 px
        // below the previous group's last one.
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(33.)).flex_none().pl(px(13.)).pr(px(10.)).child(div().flex_1().text_size(px(13.33)).text_color(gpui_kit::rgb(0x666666)).child(label)).child(div().mb(px(3.)).child(body)).into_any_element();
        // A table row (live 1710's Dimensions tab): each group lays its
        // controls out from its own column `col` (px from the panel's
        // border), the row `h` high, the control `top` px down, the label
        // centred.
        let trow = |label: &'static str, col: f32, h: f32, top: f32, body: AnyElement| div().h(px(h)).flex_none().flex().pl(px(13.)).child(div().w(px(col - 14.)).flex_none().h_full().flex().items_center().text_size(px(13.33)).text_color(gpui_kit::rgb(0x666666)).child(label)).child(div().pt(px(top)).child(body)).into_any_element();
        // The arrowhead control: the style's glyph and a caret, 28 px.
        // Live "ui2 (10-26) arrowhead picker": a 28 x 158 list right under
        // the control, six 26 px cells of glyphs only (None, Filled,
        // Closed, Open, Tick, Dot), hover rgba(144,206,241,0.7), no check.
        let arrow_open = self.drawing_props.arrow_menu;
        let asel = |id: &'static str, cur: noshape_model::drawing::sheet::ArrowType, items: Vec<(String, bool, Act)>| {
            let open = arrow_open == Some(id);
            let toggle = {
                let ed = ed.clone();
                move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    ed.update(cx, |this, cx| {
                        this.drawing_props.arrow_menu = if open { None } else { Some(id) };
                        cx.notify();
                    })
                }
            };
            let menu = open.then(|| {
                let close = ed.clone();
                deferred(
                    div()
                        .id(SharedString::from(format!("{id}-menu")))
                        .absolute()
                        .top(px(26.))
                        .left_0()
                        .w(px(28.))
                        .bg(gpui_kit::rgb(0xffffff))
                        .border_1()
                        .border_color(gpui_kit::rgb(0xcccccc))
                        .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.18), offset: point(px(0.), px(3.)), blur_radius: px(6.), spread_radius: px(0.), inset: false }])
                        .occlude()
                        .on_mouse_down_out(move |_, _, cx| {
                            close.update(cx, |this, cx| {
                                this.drawing_props.arrow_menu = None;
                                cx.notify();
                            })
                        })
                        .children(items.into_iter().zip(noshape_model::drawing::sheet::ArrowType::ALL).enumerate().map(|(i, ((_, _, f), t))| {
                            let e = ed.clone();
                            div()
                                .id(SharedString::from(format!("{id}-{i}")))
                                .size(px(26.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|d| d.bg(gpui_kit::rgba(0x90cef1b3)))
                                .child(arrowhead_glyph(t))
                                .on_click(move |_, _, cx| {
                                    e.update(cx, |this, cx| {
                                        this.drawing_props.arrow_menu = None;
                                        this.set_drawing_prop(&*f, cx);
                                    })
                                })
                        })),
                )
            });
            div()
                .relative()
                .child(h_flex().id(id).h(px(26.)).gap(px(2.)).cursor_pointer().child(arrowhead_glyph(cur)).child(crate::os_ui::glyph(SELECT_CARET, 8., gpui_kit::rgb_static(0x333333))).on_click(toggle))
                .children(menu)
                .into_any_element()
        };
        let section = |title: &'static str| div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(gpui_kit::rgb(0xefefef)).border_t_1().border_color(gpui_kit::rgb(0xdddddd)).text_size(px(13.33)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).mt(px(4.)).mb(px(8.)).child(title).into_any_element();
        let precision_items = |cur: Option<u8>, max: usize, set: fn(&mut DrawingProps, Option<u8>)| -> Vec<(String, bool, Act)> {
            (0..=max).map(|k| (precision_label(k), cur == Some(k as u8), std::rc::Rc::new(move |p: &mut DrawingProps| set(p, Some(k as u8))) as Act)).collect()
        };
        let shown = |cur: Option<u8>, default: usize| precision_label(cur.map(|c| c as usize).unwrap_or(default));
        // Live 1710: a tab longer than the panel scrolls with Chrome's 15 px
        // scrollbar at the right, the groups ending before it.
        let overflow = self.drawing_props.overflow.get();
        let mut body = v_flex().id("dr-dprops-body").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.drawing_props.scroll).when(overflow, |d| d.pr(px(gpui_kit::component::menu::NATIVE_SCROLLBAR_WIDTH)));
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
            // Live (1710): the box 9 px in and 11 px down a 37 px row, a 13.33 px #666 label.
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(37.)).flex_none().pl(px(8.)).pb(px(2.)).flex().items_center().child(crate::drawing_chrome::DrCheckbox::new(id).native().label_style(13.33, 0x666666).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| f(p, v), cx);
                })))
            };
            body = body
                // The first group sits right under the title (no 4 px gap).
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).mb(px(-4.)).child("Units and precision"))
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
                .child(row("Precision", select("dp-dual-prec", shown(props.dual.precision, 3), precision_items(props.dual.precision, 6, |p, v| p.dual.precision = v))))
                .child(row("Tolerance precision", select("dp-dual-tprec", shown(props.dual.tolerance_precision, 3), precision_items(props.dual.tolerance_precision, 6, |p, v| p.dual.tolerance_precision = v))))
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
                div().h(px(37.)).flex_none().pl(px(8.)).pb(px(2.)).flex().items_center().child(crate::drawing_chrome::DrCheckbox::new(id).native().label_style(13.33, 0x666666).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
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
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).mb(px(-4.)).child("Dimensions"))
                // Live 1710: the group's column at 123 px (selects 1 px
                // further, 220 px wide), rows 32 / 32 / 30 / 33 / 33 px.
                .child(section("All dimensions"))
                .child(trow("Font", 124., 32., 2., font_row(FontOf::Dims, props.dim_font.clone(), 220.)))
                .child(trow("Text height", 123., 32., 2., field(0)))
                .child(trow("Color", 123., 30., 0., swatch("dp-dcolor", props.dim_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.dim_color = c))))
                .child(trow("Arrowhead", 123., 33., 2., h_flex().items_start().child(field(1)).child(div().ml(px(10.)).mt(px(1.)).child(asel("dp-darrow", props.dim_arrow_type, arrow_items))).into_any_element()))
                .child(trow("Text justification", 124., 33., 2., sel("dp-djust", props.dim_justify.name().into(), just_items, 220.)))
                .child(check("dp-dunder", "Underline overridden dimensions", props.underline_overridden, |p, v| p.underline_overridden = v, cx))
                .child(check("dp-dbreak", "Break dimension by default", props.break_dims, |p, v| p.break_dims = v, cx))
                .child(trow("Fit standard", 124., 33., 2., sel("dp-dfit", std_name(fit_ansi), std_items(fit_ansi, |p, v| p.dim_fit_ansi = Some(v)), 220.)))
                // Column 153 (selects 154, 197 px), rows 32 / 32 / 32 / 33 px.
                .child(section("Gaps and extensions"))
                .child(trow("Text alignment", 154., 32., 2., sel("dp-dalign", if horiz { "Horizontal".into() } else { "Align with dimension line".into() }, align_items, 197.)))
                .child(trow("Text gap", 153., 32., 2., field(2)))
                .child(trow("Geometry gap", 153., 32., 2., field(3)))
                .child(trow("Extension past line", 153., 33., 2., field(4)))
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
                // Column 165, 186 px selects, 33 px rows.
                .child(section("Chamfer dimensions"))
                .child(trow("45 degree style", 165., 33., 2., sel("dp-ch45s", style_name(props.chamfer_45_dimension), style_items(props.chamfer_45_dimension, |p, v| p.chamfer_45_dimension = v), 186.)))
                .child(trow("45 degree content", 165., 33., 2., sel("dp-ch45c", props.chamfer_45_content.name().into(), content_items(props.chamfer_45_content, |p, v| p.chamfer_45_content = v, true), 186.)))
                .child(trow("Non 45 degree style", 165., 33., 2., sel("dp-chs", style_name(props.chamfer_dimension), style_items(props.chamfer_dimension, |p, v| p.chamfer_dimension = v), 186.)))
                .child(trow("Non 45 degree content", 165., 33., 2., sel("dp-chc", props.chamfer_content.name().into(), content_items(props.chamfer_content, |p, v| p.chamfer_content = v, false), 186.)))
                .child(trow("Length precision", 165., 33., 2., sel("dp-chlp", shown(props.chamfer_length_precision, default_digits(units)), precision_items(props.chamfer_length_precision, 6, |p, v| p.chamfer_length_precision = v), 186.)))
                .child(trow("Angle precision", 165., 33., 2., sel("dp-chap", shown(props.chamfer_angle_precision, 0), precision_items(props.chamfer_angle_precision, 4, |p, v| p.chamfer_angle_precision = v), 186.)))
                .child(section("Ordinate dimensions"))
                // Live drawing/ui2-ah-tab-Dimensions-end: column 146, 205 px.
                .child(trow("Dimension style", 146., 33., 2., sel("dp-ordstd", std_name(ord_ansi), std_items(ord_ansi, |p, v| p.ordinate_ansi = Some(v)), 205.)))
                .child(section("Diameter and radial dimensions"))
                .child(check("dp-radlines", "Show dimension lines by default", radial_lines, |p, v| p.radial_dim_lines = Some(v), cx));
        } else if self.drawing_props.tab == 2 {
            // Annotations, in Onshape's groups: All annotations (Arrowhead),
            // then each type's font, text height and color with its extras.
            let field = |i: usize| self.wt_field(i, &ed);
            let field_w = |i: usize, w: f32| self.wt_field_w(i, &ed, Some(w));
            let arrow_items: Vec<(String, bool, Act)> = noshape_model::drawing::sheet::ArrowType::ALL
                .iter()
                .map(|&t| (t.name().to_string(), t == props.ann_arrow_type, std::rc::Rc::new(move |p: &mut DrawingProps| p.ann_arrow_type = t) as Act))
                .collect();
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(37.)).flex_none().pl(px(8.)).pb(px(2.)).flex().items_center().child(crate::drawing_chrome::DrCheckbox::new(id).native().label_style(13.33, 0x666666).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| f(p, v), cx);
                })))
            };
            // A select of named values.
            fn choices<T: Copy + PartialEq + 'static>(all: &[T], cur: T, name: fn(T) -> &'static str, set: fn(&mut DrawingProps, T)) -> Vec<(String, bool, Act)> {
                all.iter().map(|&v| (name(v).to_string(), v == cur, std::rc::Rc::new(move |p: &mut DrawingProps| set(p, v)) as Act)).collect()
            }
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).mb(px(-4.)).child("Annotations"))
                // Live 1710: column 111, a 32 px row with the field 1 px down.
                .child(section("All annotations"))
                .child(trow("Arrowhead", 111., 32., 1., h_flex().items_start().child(field(1)).child(div().ml(px(10.)).mt(px(1.)).child(asel("dp-aarrow", props.ann_arrow_type, arrow_items))).into_any_element()));
            for (i, t) in AnnType::ALL.iter().copied().enumerate() {
                let ts = props.ann_type(t);
                let ids = TYPE_IDS[i];
                // Live 1710: Notes, Flag notes and Callouts as tables from
                // their columns (108 / 114 / 123 px; selects 1 px further, up
                // to 343); the groups below live's capture keep plain rows.
                // Live drawing/ui2-ah-tab-Annotations-p1, -end: every group a
                // table from its own column.
                let col = match t {
                    AnnType::Notes | AnnType::GeometricTolerances | AnnType::SurfaceFinish | AnnType::WeldSymbols | AnnType::BendNotes => Some(108.),
                    AnnType::FlagNotes => Some(114.),
                    AnnType::Callouts | AnnType::Datums => Some(123.),
                    AnnType::HoleCallouts => Some(110.),
                };
                // Notes, Flag notes and Callouts: a 31 px Color row with the
                // square at the top; the others 32 px, 1 px down; Text height
                // 32 / 31 px (Datums 30).
                let early = matches!(t, AnnType::Notes | AnnType::FlagNotes | AnnType::Callouts);
                let lay = |label: &'static str, select: bool, h: f32, top: f32, body: AnyElement| match col {
                    Some(c) => trow(label, if select { c + 1. } else { c }, h, top, body),
                    None => row(label, body),
                };
                let sel_w = col.map_or(164., |c| 343. - c);
                let color = lay("Color", false, if early { 31. } else { 32. }, if early { 0. } else { 1. }, swatch(ids[3], ts.color, std::rc::Rc::new(move |p: &mut DrawingProps, c| p.ann_type_mut(t).color = c)));
                body = body.child(section(t.name()));
                if t == AnnType::WeldSymbols {
                    // The weld standard (separate from the drawing's; it
                    // follows the drawing's until set).
                    let ansi = self.drawing.weld_ansi();
                    let items: Vec<(String, bool, Act)> = [("ANSI", true), ("ISO", false)].iter().map(|&(n, v)| (n.to_string(), ansi == v, std::rc::Rc::new(move |p: &mut DrawingProps| p.weld_ansi = Some(v)) as Act)).collect();
                    body = body.child(lay("Standard", true, 33., 2., sel("dp-weld-std", if ansi { "ANSI".into() } else { "ISO".into() }, items, sel_w)));
                }
                body = body.child(lay("Font", true, if t == AnnType::FlagNotes { 33. } else { 32. }, 2., font_row(FontOf::Type(t), ts.font.clone(), sel_w)));
                if t == AnnType::FlagNotes {
                    let shapes = choices(&noshape_model::drawing::note::FlagShape::ALL, props.flag_shape, |s| s.name(), |p, v| p.flag_shape = v);
                    body = body.child(lay("Border shape", true, 34., 2., sel("dp-flag-shape", props.flag_shape.name().into(), shapes, sel_w))).child(color);
                    continue;
                }
                body = body.child(lay("Text height", false, if early { 32. } else if t == AnnType::Datums { 30. } else { 31. }, 2., field(2 + i)));
                if t == AnnType::Datums {
                    body = body.child(lay("Datum size", false, 31., 2., field(2 + AnnType::ALL.len())));
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
                            .child(lay("Text justification", true, 33., 2., sel("dp-co-just", props.callout_justify.name().into(), just, sel_w)))
                            .child(lay("Stack direction", true, 32., 2., sel("dp-co-stack", props.callout_stack.name().into(), stack, sel_w)))
                            .child(lay("Stack gap size", false, 33., 2., field_w(4 + AnnType::ALL.len(), 192.)))
                            // Live: column 141, a 174 px spinbox 1 px down.
                            .child(section("Center of mass"))
                            .child(trow("Diameter size", 141., 32., 1., field_w(3 + AnnType::ALL.len(), 174.)));
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
                            // Live: its own table from column 147 / 146.
                            .child(trow("Target hatch pattern", 147., 33., 2., sel("dp-th-std", std_name.into(), stds, 197.)))
                            .child(trow("", 146., 33., 2., sel("dp-th-pat", pat_name.into(), pats, 199.)));
                    }
                    AnnType::HoleCallouts => {
                        let ind: Vec<(String, bool, Act)> = HOLE_INDICATORS.iter().map(|&n| (n.to_string(), props.hole_indicator == n, std::rc::Rc::new(move |p: &mut DrawingProps| p.hole_indicator = n.into()) as Act)).collect();
                        let fit: Vec<(String, bool, Act)> = [("ISO", true), ("ANSI", false)].iter().map(|&(n, v)| (n.to_string(), props.fit_iso == v, std::rc::Rc::new(move |p: &mut DrawingProps| p.fit_iso = v) as Act)).collect();
                        body = body
                            .child(lay("Indicator", true, 33., 2., sel("dp-hc-ind", props.hole_indicator.clone(), ind, sel_w)))
                            .child(lay("Fit standard", true, 33., 2., sel("dp-hc-fit", if props.fit_iso { "ISO".into() } else { "ANSI".into() }, fit, sel_w)));
                    }
                    _ => {}
                }
            }
        } else if self.drawing_props.tab == 3 {
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(37.)).flex_none().pl(px(8.)).pb(px(2.)).flex().items_center().child(crate::drawing_chrome::DrCheckbox::new(id).native().label_style(13.33, 0x666666).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| f(p, v), cx);
                })))
            };
            let vd = props.view_defaults.clone();
            let vt = props.view_tab.clone();
            let proj = self.drawing.projection;
            let ed2 = ed.clone();
            // Live 1710: 223 px from the group's column (121 px).
            let proj_btn = select_field("dp-proj", if proj == Projection::ThirdAngle { "Third angle".into() } else { "First angle".into() }, 223.)
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
            let on_off = |on: bool| if on { "On" } else { "Off" };
            let on_off_items = |on: bool, set: fn(&mut DrawingProps, bool)| -> Vec<(String, bool, Act)> {
                [("On", true), ("Off", false)].iter().map(|&(n, v)| (n.to_string(), on == v, std::rc::Rc::new(move |p: &mut DrawingProps| set(p, v)) as Act)).collect()
            };
            let tangent_items: Vec<(String, bool, Act)> = ["Hidden", "Solid", "Phantom"].iter().enumerate().map(|(i, n)| (n.to_string(), vd.tangent == i as u8, std::rc::Rc::new(move |p: &mut DrawingProps| p.view_defaults.tangent = i as u8) as Act)).collect();
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).mb(px(-4.)).child("Views"))
                // Live 1710: Visible edges a 193 px pen select and its colour
                // 6 px on; View labels a font select, then a row of the
                // 100 px height, Bold / Italic and the colour.
                .child(section("All views"))
                .child(trow("Projection angle", 121., 33., 2., proj_btn))
                .child(trow("Visible edges", 120., 33., 2., h_flex().items_start().child(thick_sel("dp-vthick", vt.visible_thickness, |p, v| p.view_tab.visible_thickness = v, 193.)).child(div().ml(px(6.)).child(swatch("dp-vcolor", vt.visible_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.visible_color = c)))).into_any_element()))
                .child(trow("View labels", 121., 32., 2., font_row(FontOf::ViewLabel, vt.view_label_font.clone(), 223.)))
                .child(trow(
                    "",
                    120.,
                    33.,
                    2.,
                    h_flex()
                        .items_start()
                        .child(self.wt_field_w(4, &ed, Some(100.)))
                        .child(div().ml(px(7.)).child(Self::font_style_toggles(FontOf::ViewLabel, vt.view_label_font.clone(), ("dp-vlfont-b", "dp-vlfont-i"), 6., cx)))
                        .child(div().ml(px(5.)).child(swatch("dp-vlcolor", vt.view_label_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.view_label_color = c))))
                        .into_any_element(),
                ))
                // Live 1710: the column at 165 px, selects up to 343 px, in
                // live's order; Scale label and the auto centermarks /
                // centerlines are On / Off selects.
                .child(section("Insert view defaults"))
                .child(trow("View simplification", 165., 39., 2., sel("dp-vsimp", simp_label.into(), simp_items, 179.)))
                .child(trow("Hidden lines", 165., 33., 2., sel("dp-vhidden", if vd.hidden_lines { "Shown".into() } else { "Hidden".into() }, hidden_items, 179.)))
                .child(trow("Tangent edges", 165., 33., 2., sel("dp-vtan", ["Hidden", "Solid", "Phantom"][vd.tangent.min(2) as usize].into(), tangent_items, 179.)))
                .child(trow("Scale label", 165., 33., 2., sel("dp-vscale", on_off(vd.scale_label).into(), on_off_items(vd.scale_label, |p, v| p.view_defaults.scale_label = v), 179.)))
                .child(trow("Auto centermarks (Parts)", 165., 33., 2., sel("dp-vcm", on_off(vd.auto_centermarks).into(), on_off_items(vd.auto_centermarks, |p, v| p.view_defaults.auto_centermarks = v), 179.)))
                .child(trow("Auto centerlines (Parts)", 165., 33., 2., sel("dp-vcl", on_off(vd.auto_centerlines).into(), on_off_items(vd.auto_centerlines, |p, v| p.view_defaults.auto_centerlines = v), 179.)))
                .child(trow("Threads (Parts)", 165., 33., 2., sel("dp-vthreads", if vd.threads { "Shown".into() } else { "Hidden".into() }, vec![
                    ("Hidden".into(), !vd.threads, std::rc::Rc::new(|p: &mut DrawingProps| p.view_defaults.threads = false) as Act),
                    ("Shown".into(), vd.threads, std::rc::Rc::new(|p: &mut DrawingProps| p.view_defaults.threads = true) as Act),
                ], 179.)))
                .child(trow("Bend lines (Parts)", 165., 33., 2., sel("dp-vbend", on_off(vd.bend_lines).into(), on_off_items(vd.bend_lines, |p, v| p.view_defaults.bend_lines = v), 179.)))
                .child(trow("Bend notes (Parts)", 165., 33., 2., sel("dp-vbendn", on_off(vd.bend_notes).into(), on_off_items(vd.bend_notes, |p, v| p.view_defaults.bend_notes = v), 179.)));
            // Per view type.
            // Live drawing/ui2-ah-tab-Views-p1…p3 (the tab scrolled): each
            // row from its own column (`col`, px from the panel's border),
            // `h` high, the control `top` px down; a pen select up to `end`
            // and its colour at `sw`.
            let pen_at = |label: &'static str, col: f32, end: f32, sw: f32, h: f32, top: f32, ids: (&'static str, &'static str), pen: &noshape_model::drawing::PenProps, set: fn(&mut DrawingProps, f64), set_color: crate::drawing_color::ColorSet| {
                trow(label, col, h, top, h_flex().items_start().child(thick_sel(ids.0, pen.thickness, set, end - col + 1.)).child(div().ml(px(sw - end - 1.)).child(swatch(ids.1, pen.color, set_color))).into_any_element())
            };
            let arrow_at = |col: f32, h: f32, id: &'static str, field_i: usize, kind: noshape_model::drawing::sheet::ArrowType, set: fn(&mut DrawingProps, noshape_model::drawing::sheet::ArrowType)| {
                let items: Vec<(String, bool, Act)> = noshape_model::drawing::sheet::ArrowType::ALL.iter().map(|&t| (t.name().to_string(), t == kind, std::rc::Rc::new(move |p: &mut DrawingProps| set(p, t)) as Act)).collect();
                trow("Arrowhead", col, h, 2., h_flex().items_start().child(self.wt_field(field_i, &ed)).child(div().ml(px(10.)).mt(px(1.)).child(asel(id, kind, items))).into_any_element())
            };
            // A label's second row: its 100 px height, Bold / Italic 7 px
            // after the unit (6 apart), the colour 6 px on.
            let label_at = |col: f32, field_i: usize, w: FontOf, font: Option<noshape_model::drawing::sheet::TextFont>, ids: (&'static str, &'static str, &'static str), color: Option<[u8; 3]>, set_color: crate::drawing_color::ColorSet, cx: &mut Context<Self>| {
                trow(
                    "",
                    col,
                    33.,
                    2.,
                    h_flex()
                        .items_start()
                        .child(self.wt_field_w(field_i, &ed, Some(100.)))
                        .child(div().ml(px(7.)).child(Self::font_style_toggles(w, font, (ids.0, ids.1), 6., cx)))
                        .child(div().ml(px(6.)).child(swatch(ids.2, color, set_color)))
                        .into_any_element(),
                )
            };
            let pick = |id: &'static str, names: &[&'static str], cur: u8, set: fn(&mut DrawingProps, u8), w: f32| {
                let items: Vec<(String, bool, Act)> = names.iter().enumerate().map(|(i, n)| (n.to_string(), cur == i as u8, std::rc::Rc::new(move |p: &mut DrawingProps| set(p, i as u8)) as Act)).collect();
                sel(id, names[(cur as usize).min(names.len() - 1)].into(), items, w)
            };
            use noshape_model::drawing::hatch_pat;
            let (hstd, hpat) = vt.hatch_pattern;
            let hatch_std_items: Vec<(String, bool, Act)> = hatch_pat::GROUPS.iter().enumerate().map(|(i, g)| (g.0.to_string(), i == hstd, std::rc::Rc::new(move |p: &mut DrawingProps| p.view_tab.hatch_pattern = (i, hatch_pat::default_pattern(i))) as Act)).collect();
            let hatch_pat_items: Vec<(String, bool, Act)> = hatch_pat::display_order(hstd)
                .into_iter()
                .filter_map(|k| hatch_pat::pattern(hstd, k).map(|p| (p.name.to_string(), k == hpat, std::rc::Rc::new(move |pr: &mut DrawingProps| pr.view_tab.hatch_pattern = (hstd, k)) as Act)))
                .collect();
            let aux_styles: Vec<(String, bool, Act)> = vec![
                ("A".into(), !vt.aux_double_label, std::rc::Rc::new(|p: &mut DrawingProps| p.view_tab.aux_double_label = false) as Act),
                ("A-A".into(), vt.aux_double_label, std::rc::Rc::new(|p: &mut DrawingProps| p.view_tab.aux_double_label = true) as Act),
            ];
            body = body
                // Live 1710: column 124, a 189 px pen select and its colour,
                // Dashes a 200 px spinbox 1 px further.
                .child(section("Hidden edges"))
                .child(trow("Thickness", 124., 33., 1., h_flex().items_start().child(thick_sel("dp-hthick", vt.hidden_thickness, |p, v| p.view_tab.hidden_thickness = v, 189.)).child(div().ml(px(6.)).child(swatch("dp-hcolor", vt.hidden_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.hidden_color = c)))).into_any_element()))
                .child(trow("Dashes", 125., 32., 1., self.wt_field_w(5, &ed, Some(200.))))
                .child(section("Tangent edges"))
                .child(trow("Thickness", 124., 33., 1., h_flex().items_start().child(thick_sel("dp-tthick", vt.tangent_thickness, |p, v| p.view_tab.tangent_thickness = v, 189.)).child(div().ml(px(6.)).child(swatch("dp-tcolor", vt.tangent_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.tangent_color = c)))).into_any_element()))
                .child(trow("Dashes", 125., 32., 1., self.wt_field_w(6, &ed, Some(200.))))
                .child(section("Exploded lines"))
                .child(trow("Thickness", 124., 33., 1., h_flex().items_start().child(thick_sel("dp-ethick", vt.exploded_thickness, |p, v| p.view_tab.exploded_thickness = v, 189.)).child(div().ml(px(6.)).child(swatch("dp-ecolor", vt.exploded_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.exploded_color = c)))).into_any_element()))
                .child(trow("Dashes", 125., 32., 1., self.wt_field_w(7, &ed, Some(200.))))
                .child(section("Section views"))
                .child(pen_at("Cutting lines", 123., 312., 319., 32., 2., ("dp-scut", "dp-scutc"), &vt.section_line, |p, v| p.view_tab.section_line.thickness = v, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.section_line.color = c)))
                .child(arrow_at(123., 37., "dp-sarrow", 8, vt.section_arrow.kind, |p, t| p.view_tab.section_arrow.kind = t))
                .child(trow("Cutting line label", 124., 32., 2., font_row(FontOf::SectionLabel, vt.section_label.font.clone(), 220.)))
                .child(label_at(123., 9, FontOf::SectionLabel, vt.section_label.font.clone(), ("dp-slb", "dp-sli", "dp-slcolor"), vt.section_label.color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.section_label.color = c), cx))
                .child(section("View and region hatches"))
                .child(trow("Hatch pattern", 116., 34., 2., sel("dp-hstd", noshape_model::drawing::hatch_pat::GROUPS[hstd.min(2)].0.into(), hatch_std_items, 228.)))
                .child(trow("", 115., 32., 2., sel("dp-hpat", noshape_model::drawing::hatch_pat::pattern(hstd, hpat).map_or("", |p| p.name).into(), hatch_pat_items, 230.)))
                .child(pen_at("", 115., 312., 319., 33., 2., ("dp-hatch", "dp-hatchc"), &vt.hatch_line, |p, v| p.view_tab.hatch_line.thickness = v, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.hatch_line.color = c)))
                .child(section("Detail views"))
                .child(pen_at("Detail profile lines", 121., 312., 319., 32., 1., ("dp-dpen", "dp-dpenc"), &vt.detail_pen, |p, v| p.view_tab.detail_pen.thickness = v, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.detail_pen.color = c)))
                .child(trow("", 122., 32., 2., pick("dp-dline", &["Phantom", "Dashed", "Solid"], vt.detail_line, |p, v| p.view_tab.detail_line = v, 222.)))
                .child(arrow_at(121., 37., "dp-darrowh", 10, vt.detail_arrow.kind, |p, t| p.view_tab.detail_arrow.kind = t))
                .child(trow("Detail profile label", 122., 32., 2., font_row(FontOf::DetailLabel, vt.detail_label.font.clone(), 222.)))
                .child(label_at(121., 11, FontOf::DetailLabel, vt.detail_label.font.clone(), ("dp-dlb", "dp-dli", "dp-dlcolor"), vt.detail_label.color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.detail_label.color = c), cx))
                .child(trow("Detail profile style", 122., 33., 2., pick("dp-dstyle", &["In line", "Connected", "Leader"], vt.detail_style, |p, v| p.view_tab.detail_style = v, 222.)))
                .child(check("dp-dprofile", "Show detail view profile", vt.detail_show_profile, |p, v| p.view_tab.detail_show_profile = v, cx))
                .child(check("dp-dclip", "Clip border to geometry", vt.detail_clip_border, |p, v| p.view_tab.detail_clip_border = v, cx))
                .child(section("Auxiliary views"))
                .child(check("dp-aind", "Show view reference indicator", vt.aux_indicator, |p, v| p.view_tab.aux_indicator = v, cx))
                .child(row("View reference format", div().into_any_element()))
                .child(div().pt(px(10.)).pb(px(5.)).child(self.aux_format_tiles(vt.aux_arrow_format, cx)))
                .child(pen_at("Viewing lines", 125., 314., 318., 29., 1., ("dp-aline", "dp-alinec"), &vt.aux_line, |p, v| p.view_tab.aux_line.thickness = v, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.aux_line.color = c)))
                .child(arrow_at(125., 37., "dp-aarrowh", 12, vt.aux_arrow.kind, |p, t| p.view_tab.aux_arrow.kind = t))
                .child(trow("Viewing line label", 126., 32., 2., font_row(FontOf::AuxLabel, vt.aux_label.font.clone(), 218.)))
                .child(label_at(125., 13, FontOf::AuxLabel, vt.aux_label.font.clone(), ("dp-alb", "dp-ali", "dp-alcolor"), vt.aux_label.color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.aux_label.color = c), cx))
                .child(trow("View label prefix", 126., 33., 2., self.wt_field_w(2, &ed, Some(200.))))
                .child(trow("View label style", 126., 33., 2., sel("dp-alabel", if vt.aux_double_label { "A-A".into() } else { "A".into() }, aux_styles, 218.)))
                .child(trow("View label suffix", 126., 33., 2., self.wt_field_w(3, &ed, Some(200.))))
                .child(section("Break views"))
                .child(pen_at("Break lines", 135., 312., 319., 32., 1., ("dp-break", "dp-breakc"), &vt.break_line, |p, v| p.view_tab.break_line.thickness = v, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.break_line.color = c)));
            // Cosmetic threads and Flat pattern views (live dom/drawing-
            // properties-views): line styles as a select and the colour.
            const BEND_LINES: [&str; 3] = ["Solid", "Dashed", "Center"];
            // Live drawing/ui2-ah-tab-Views-p3: Flat pattern views' rows from
            // column 131 (bend lines), 201 (Counterbore / Countersink) and 126
            // (forms), selects up to 312 and the colour at 319, 32 px rows.
            let style_row = |label: &'static str, col: f32, ids: (&'static str, &'static str), cur: u8, set: StyleSet, color: Option<[u8; 3]>, set_color: crate::drawing_color::ColorSet| {
                let items: Vec<(String, bool, Act)> = BEND_LINES
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        let set = set.clone();
                        (n.to_string(), cur == i as u8, std::rc::Rc::new(move |p: &mut DrawingProps| set(p, i as u8)) as Act)
                    })
                    .collect();
                trow(label, col, 32., 1., h_flex().items_start().child(sel(ids.0, BEND_LINES[cur.min(2) as usize].into(), items, 313. - col)).child(div().ml(px(6.)).child(swatch(ids.1, color, set_color))).into_any_element())
            };
            let thread_style = vt.thread_style.unwrap_or(if self.current_sheet().size.is_ansi() { ThreadStyle::Ansi } else { ThreadStyle::Iso });
            let thread_items: Vec<(String, bool, Act)> = ThreadStyle::ALL.iter().map(|&t| (t.name().to_string(), t == thread_style, std::rc::Rc::new(move |p: &mut DrawingProps| p.view_tab.thread_style = Some(t)) as Act)).collect();
            body = body
                // Live p3: column 105, Thread style 239 px, Thickness 202 px
                // and its colour at 314, Dashes 200 px.
                .child(section("Cosmetic threads"))
                .child(trow("Thread style", 105., 34., 2., sel("dp-thstyle", thread_style.name().into(), thread_items, 239.)))
                .child(pen_at("Thickness", 105., 306., 314., 33., 1., ("dp-thpen", "dp-thpenc"), &vt.thread_line, |p, v| p.view_tab.thread_line.thickness = v, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.thread_line.color = c)))
                // Live 10-26 d: Dashes only for ANSI; the style's two
                // previews under it (the tiles 33 px higher without it).
                .when(thread_style == ThreadStyle::Ansi, |d| d.child(trow("Dashes", 105., 32., 1., self.wt_field_w(14, &ed, Some(200.)))))
                .child(div().pl(px(77.)).pt(px(if thread_style == ThreadStyle::Ansi { 2. } else { 1. })).pb(px(5.)).child(thread_tiles(thread_style)))
                .child(section("Flat pattern views"))
                .child(trow("All bend lines", 131., 32., 1., thick_sel("dp-bendall", vt.bend_lines_thickness, |p, v| p.view_tab.bend_lines_thickness = v, 214.)))
                .child(style_row("Up bend lines", 131., ("dp-bup", "dp-bupc"), vt.bend_up_line, std::rc::Rc::new(|p: &mut DrawingProps, v| p.view_tab.bend_up_line = v), vt.bend_up_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.bend_up_color = c)))
                .child(style_row("Down bend lines", 131., ("dp-bdown", "dp-bdownc"), vt.bend_down_line, std::rc::Rc::new(|p: &mut DrawingProps, v| p.view_tab.bend_down_line = v), vt.bend_down_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.bend_down_color = c)))
                .child(pen_at(
                    "Bend tangent edges",
                    131.,
                    312.,
                    319.,
                    32.,
                    1.,
                    ("dp-btan", "dp-btanc"),
                    &noshape_model::drawing::PenProps { thickness: vt.bend_tangent_thickness, color: vt.bend_tangent_color },
                    |p, v| p.view_tab.bend_tangent_thickness = v,
                    std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.bend_tangent_color = c),
                ));
            const FLAT_IDS: [(&str, &str); 5] = [("dp-fl0", "dp-fl0c"), ("dp-fl1", "dp-fl1c"), ("dp-fl2", "dp-fl2c"), ("dp-fl3", "dp-fl3c"), ("dp-fl4", "dp-fl4c")];
            for (k, (label, default)) in noshape_model::drawing::FLAT_LINE_ROWS.iter().enumerate() {
                let (cur, color) = vt.flat_lines.get(k).copied().unwrap_or((*default, None));
                body = body.child(style_row(label, if k < 2 { 201. } else { 126. }, FLAT_IDS[k], cur, std::rc::Rc::new(move |p: &mut DrawingProps, v| flat_slot(p, k).0 = v), color, std::rc::Rc::new(move |p: &mut DrawingProps, c| flat_slot(p, k).1 = c)));
            }
            body = body.child(style_row(
                "Form centermarks",
                126.,
                ("dp-fcm", "dp-fcmc"),
                vt.form_centermark_line,
                std::rc::Rc::new(|p: &mut DrawingProps, v| p.view_tab.form_centermark_line = v),
                vt.form_centermark_color,
                std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.form_centermark_color = c),
            ));
        } else if self.drawing_props.tab == 4 {
            let vt = props.view_tab.clone();
            let point_shapes: Vec<(String, bool, Act)> = noshape_model::drawing::SketchPointShape::ALL
                .iter()
                .map(|&s| (s.name().to_string(), vt.sketch_point_shape == s, std::rc::Rc::new(move |p: &mut DrawingProps| p.view_tab.sketch_point_shape = s) as Act))
                .collect();
            // Live: ".25 mm", ".5 mm", "1 mm" … (no leading zero).
            let point_size_label = |s: f64| format!("{} mm", format!("{s}").trim_start_matches('0'));
            let point_sizes: Vec<(String, bool, Act)> = noshape_model::drawing::SKETCH_POINT_SIZES
                .iter()
                .map(|&s| (point_size_label(s), (vt.sketch_point_size - s).abs() < 1e-9, std::rc::Rc::new(move |p: &mut DrawingProps| p.view_tab.sketch_point_size = s) as Act))
                .collect();
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(44.)).flex_none().pl(px(8.)).pb(px(3.)).flex().items_center().child(crate::drawing_chrome::DrCheckbox::new(id).native().label_style(13.33, 0x666666).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| f(p, v), cx);
                })))
            };
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).mb(px(-4.)).child("Construction geometry"))
                // Live 1710: Extended lines first (a 44 px row), then column
                // 149 with 181 px spinboxes; Centerlines from column 130.
                // Live 1710: Created in drawing: Virtual sharps' style select
                // (column 155) over its pen and colour, Sketch geometry's pen
                // and colour (column 154).
                .child(section("Created in drawing"))
                // (Its label centred on the select, 13 px under the header.)
                .child(div().pt(px(13.)).child(trow("Virtual sharps", 155., 31., 1., sel("dp-vsharp", if vt.virtual_sharp_extension { "Edge extension".into() } else { "Centermark".into() }, vec![
                    ("Centermark".into(), !vt.virtual_sharp_extension, std::rc::Rc::new(|p: &mut DrawingProps| p.view_tab.virtual_sharp_extension = false) as Act),
                    ("Edge extension".into(), vt.virtual_sharp_extension, std::rc::Rc::new(|p: &mut DrawingProps| p.view_tab.virtual_sharp_extension = true) as Act),
                ], 204.))))
                .child(trow("", 154., 32., 2., h_flex().items_start().child(thick_sel("dp-vsharpw", vt.virtual_sharp_line.thickness, |p, v| p.view_tab.virtual_sharp_line.thickness = v, 174.)).child(div().ml(px(6.)).child(swatch("dp-vsharpc", vt.virtual_sharp_line.color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.virtual_sharp_line.color = c)))).into_any_element()))
                .child(trow("Sketch geometry", 154., 33., 2., h_flex().items_start().child(thick_sel("dp-skw", vt.sketch_line.thickness, |p, v| p.view_tab.sketch_line.thickness = v, 174.)).child(div().ml(px(6.)).child(swatch("dp-skc", vt.sketch_line.color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.sketch_line.color = c)))).into_any_element()))
                .child(trow("Sketch driven dimensions color", 214., 32., 1., swatch("dp-sddc", Some(vt.sketch_driven_dim_color), std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.sketch_driven_dim_color = c.unwrap_or([0, 0, 0])))))
                // Live 1710: Part Studio sketches from column 138: 190 px pens
                // (Sketch points: 104 px shape and 80 px size), swatches at 334.
                .child(section("Part Studio sketches"))
                .child(trow("Solid lines", 138., 32., 1., h_flex().items_start().child(thick_sel("dp-pssw", vt.sketch_solid.thickness, |p, v| p.view_tab.sketch_solid.thickness = v, 190.)).child(div().ml(px(6.)).child(swatch("dp-pssc", vt.sketch_solid.color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.sketch_solid.color = c)))).into_any_element()))
                .child(trow("Construction lines", 138., 32., 1., h_flex().items_start().child(thick_sel("dp-pscw", vt.sketch_construction.thickness, |p, v| p.view_tab.sketch_construction.thickness = v, 190.)).child(div().ml(px(6.)).child(swatch("dp-pscc", vt.sketch_construction.color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.sketch_construction.color = c)))).into_any_element()))
                .child(trow(
                    "Sketch points",
                    138.,
                    32.,
                    1.,
                    h_flex()
                        .items_start()
                        .child(sel("dp-psptshape", vt.sketch_point_shape.name().into(), point_shapes, 104.))
                        .child(div().ml(px(6.)).child(sel("dp-psptsize", point_size_label(vt.sketch_point_size), point_sizes, 80.)))
                        .child(div().ml(px(6.)).child(swatch("dp-psptc", vt.sketch_point_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.sketch_point_color = c))))
                        .into_any_element(),
                ))
                .child(section("Centermarks"))
                .child(check("dp-cmext", "Extended lines", vt.centermark_extended, |p, v| p.view_tab.centermark_extended = v, cx))
                .child(trow("Thickness", 150., 33., 2., thick_sel("dp-cmthick", vt.centermark_thickness, |p, v| p.view_tab.centermark_thickness = v, 209.)))
                .child(trow("Mark size", 149., 32., 2., self.wt_field_w(0, &ed, Some(181.))))
                .child(trow("Gap size", 149., 32., 2., self.wt_field_w(1, &ed, Some(181.))))
                .child(trow("Color", 149., 33., 2., swatch("dp-cmcolor", vt.centermark_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.centermark_color = c))))
                .child(section("Centerlines"))
                .child(trow("Thickness", 130., 33., 2., h_flex().items_start().child(thick_sel("dp-clthick", vt.centerline_thickness, |p, v| p.view_tab.centerline_thickness = v, 193.)).child(div().ml(px(6.)).child(swatch("dp-clcolor", vt.centerline_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.centerline_color = c)))).into_any_element()))
                .child(trow("Dashes", 130., 32., 2., self.wt_field_w(2, &ed, Some(193.))));
        } else if self.drawing_props.tab == 5 {
            let color_select = |id: &'static str, cur: Option<[u8; 3]>, set: fn(&mut DrawingProps, Option<[u8; 3]>)| swatch(id, cur, std::rc::Rc::new(move |p: &mut DrawingProps, c| set(p, c)));
            let locked = props.formats_locked;
            let lock_items: Vec<(String, bool, Act)> = vec![
                ("Unlocked".into(), !locked, std::rc::Rc::new(|p: &mut DrawingProps| p.formats_locked = false) as Act),
                ("Locked".into(), locked, std::rc::Rc::new(|p: &mut DrawingProps| p.formats_locked = true) as Act),
            ];
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).mb(px(-4.)).child("Formats"))
                // Live 1710: a Lock group with an Unlocked / Locked select
                // (column 112, 147 px); the colours at column 111.
                .child(section("Lock"))
                .child(trow("Lock", 112., 33., 2., sel("dp-flock", if locked { "Locked".into() } else { "Unlocked".into() }, lock_items, 147.)))
                // Live 1710: each group's Color, then a 147 px Thickness.
                .child(section("Border frame"))
                .child(trow("Color", 111., 32., 1., color_select("dp-fborder", props.border_color, |p, c| p.border_color = c)))
                .child(trow("Thickness", 112., 33., 2., thick_sel("dp-fborderw", props.border_thickness, |p, v| p.border_thickness = v, 147.)))
                .child(section("Border zone"))
                .child(trow("Color", 111., 32., 1., color_select("dp-fzone", props.zone_color, |p, c| p.zone_color = c)))
                .child(trow("Thickness", 112., 33., 2., thick_sel("dp-fzonew", props.zone_thickness, |p, v| p.zone_thickness = v, 147.)))
                .child(section("Title block"))
                .child(trow("Color", 111., 32., 1., color_select("dp-ftitle", props.title_block_color, |p, c| p.title_block_color = c)))
                .child(trow("Thickness", 112., 33., 2., thick_sel("dp-ftitlew", props.title_block_thickness, |p, v| p.title_block_thickness = v, 147.)));
        } else if self.drawing_props.tab == 6 {
            body = body
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).mb(px(-4.)).child("Tables"))
                // Live 1710: column 181, 100 px spinboxes 1 px down 32 px rows.
                .child(section("All tables"))
                .child(trow("Horizontal padding", 181., 32., 1., self.wt_field_w(0, &ed, Some(100.))))
                .child(trow("Vertical padding", 181., 32., 1., self.wt_field_w(1, &ed, Some(100.))));
            // Live 1710 / dom/drawing-wt-widgets: each kind's group from
            // column 144: Line thickness and Font (199 px), the row texts an
            // 85 px height, Bold / Italic and the colour.
            const KINDS: [&str; 6] = ["BOM tables", "Custom tables", "Cut list tables", "Hole tables", "Revision tables", "Tables"];
            const IDS: [[&str; 7]; 6] = [
                ["dp-t0l", "dp-t0hb", "dp-t0hi", "dp-t0hc", "dp-t0cb", "dp-t0ci", "dp-t0cc"],
                ["dp-t1l", "dp-t1hb", "dp-t1hi", "dp-t1hc", "dp-t1cb", "dp-t1ci", "dp-t1cc"],
                ["dp-t2l", "dp-t2hb", "dp-t2hi", "dp-t2hc", "dp-t2cb", "dp-t2ci", "dp-t2cc"],
                ["dp-t3l", "dp-t3hb", "dp-t3hi", "dp-t3hc", "dp-t3cb", "dp-t3ci", "dp-t3cc"],
                ["dp-t4l", "dp-t4hb", "dp-t4hi", "dp-t4hc", "dp-t4cb", "dp-t4ci", "dp-t4cc"],
                ["dp-t5l", "dp-t5hb", "dp-t5hi", "dp-t5hc", "dp-t5cb", "dp-t5ci", "dp-t5cc"],
            ];
            let ts = props.table_styles.clone();
            let styles = [&ts.bom, &ts.custom, &ts.cut_list, &ts.hole, &ts.revision, &ts.general];
            // A text row of its own: field `field` (85 px), Bold / Italic and
            // the colour of row text `k` (see `row_text_mut`).
            let text_row = |field: usize, k: usize, rt: &RowText, ids: (&'static str, &'static str, &'static str), cx: &mut Context<Self>| {
                h_flex()
                    .items_start()
                    .child(self.wt_field_w(field, &ed, Some(85.)))
                    .child(Self::row_text_toggles(k, false, rt, (ids.0, ids.1), cx))
                    .child(div().ml(px(3.)).child(swatch(ids.2, rt.color, std::rc::Rc::new(move |p: &mut DrawingProps, c| row_text_mut(p, k, false).color = c))))
                    .into_any_element()
            };
            for (k, name) in KINDS.iter().enumerate() {
                let st = styles[k];
                let ids = IDS[k];
                let set_line: fn(&mut DrawingProps, f64) = match k {
                    0 => |p, v| table_style_mut(p, 0).line = v,
                    1 => |p, v| table_style_mut(p, 1).line = v,
                    2 => |p, v| table_style_mut(p, 2).line = v,
                    3 => |p, v| table_style_mut(p, 3).line = v,
                    4 => |p, v| table_style_mut(p, 4).line = v,
                    _ => |p, v| table_style_mut(p, 5).line = v,
                };
                body = body
                    .child(section(name))
                    .child(trow("Line thickness", 145., 33., 2., thick_sel(ids[0], st.line, set_line, 199.)))
                    .child(trow("Font", 145., 32., 2., font_row(FontOf::Table(k), st.font.clone(), 199.)));
                if k >= 4 {
                    const TITLE_IDS: [(&str, &str, &str); 2] = [("dp-t4tb", "dp-t4ti", "dp-t4tc"), ("dp-t5tb", "dp-t5ti", "dp-t5tc")];
                    body = body.child(trow("Title row text", 144., 32., 2., text_row(14 + k - 4, k + 4, &st.title_text, TITLE_IDS[k - 4], cx)));
                }
                for (header, label) in [(true, "Header row text"), (false, "Content row text")] {
                    let rt = if header { &st.header } else { &st.content };
                    let field = 2 + 2 * k + (!header) as usize;
                    let (bi, ci) = if header { (1, 3) } else { (4, 6) };
                    body = body.child(trow(
                        label,
                        144.,
                        if header { 32. } else { 33. },
                        2.,
                        h_flex()
                            .items_start()
                            .child(self.wt_field_w(field, &ed, Some(85.)))
                            .child(Self::row_text_toggles(k, header, rt, (ids[bi], ids[bi + 1]), cx))
                            .child(div().ml(px(3.)).child(swatch(ids[ci], rt.color, std::rc::Rc::new(move |p: &mut DrawingProps, c| {
                                let s = table_style_mut(p, k);
                                if header { s.header.color = c } else { s.content.color = c }
                            }))))
                            .into_any_element(),
                    ));
                }
                if k == 3 {
                    // Hole tables' indicator and tags (live dom/drawing-wt-widgets).
                    body = body
                        .child(trow("Indicator style", 145., 33., 2., sel("dp-hind", if ts.hole_indicator_iso { "ISO".into() } else { "ANSI".into() }, vec![
                            ("ANSI".into(), !ts.hole_indicator_iso, std::rc::Rc::new(|p: &mut DrawingProps| p.table_styles.hole_indicator_iso = false) as Act),
                            ("ISO".into(), ts.hole_indicator_iso, std::rc::Rc::new(|p: &mut DrawingProps| p.table_styles.hole_indicator_iso = true) as Act),
                        ], 199.)))
                        .child(trow("Indicator arrowhead", 144., 32., 2., self.wt_field_w(16, &ed, Some(85.))))
                        // Live 1710: Indicator lines' colour at 319; the Tags'
                        // height, Bold / Italic and colour under their font.
                        .child(trow("Indicator lines", 145., 33., 2., h_flex().items_start().child(thick_sel("dp-hindl", ts.hole_indicator_line, |p, v| p.table_styles.hole_indicator_line = v, 169.)).child(div().ml(px(6.)).child(swatch("dp-hindc", ts.hole_indicator_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.table_styles.hole_indicator_color = c)))).into_any_element()))
                        .child(trow("Tags", 145., 32., 2., font_row(FontOf::HoleTags, ts.hole_tags_font.clone(), 199.)))
                        .child(trow("", 144., 33., 2., text_row(17, 6, &ts.hole_tags, ("dp-htb", "dp-hti", "dp-htc"), cx)));
                }
                if k == 4 {
                    // Live 1710: Revision callouts' border (98 px) and fit
                    // (97 px), then its font and text rows.
                    let shapes: Vec<(String, bool, Act)> = CalloutBorder::ALL
                        .iter()
                        .filter(|&&b| b != CalloutBorder::SplitCircle)
                        .map(|&b| (b.name().to_string(), b == ts.revision_callout_shape, std::rc::Rc::new(move |p: &mut DrawingProps| p.table_styles.revision_callout_shape = b) as Act))
                        .collect();
                    let fits: Vec<(String, bool, Act)> = (0..=5u8).map(|n| (border_chars_label(n), n == ts.revision_callout_chars, std::rc::Rc::new(move |p: &mut DrawingProps| p.table_styles.revision_callout_chars = n) as Act)).collect();
                    body = body
                        .child(trow(
                            "Revision callouts",
                            144.,
                            32.,
                            1.,
                            h_flex().items_start().child(sel("dp-rcshape", ts.revision_callout_shape.name().into(), shapes, 98.)).child(div().ml(px(6.)).child(sel("dp-rcfit", border_chars_label(ts.revision_callout_chars), fits, 97.))).into_any_element(),
                        ))
                        .child(trow("", 145., 32., 2., font_row(FontOf::RevisionCallout, ts.revision_callout_font.clone(), 199.)))
                        .child(trow("", 144., 33., 2., text_row(18, 7, &ts.revision_callout_text, ("dp-rcb", "dp-rci", "dp-rcc"), cx)));
                }
            }
        } else if self.drawing_props.tab == 7 {
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(37.)).flex_none().pl(px(8.)).pb(px(2.)).flex().items_center().child(crate::drawing_chrome::DrCheckbox::new(id).native().label_style(13.33, 0x666666).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
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
                .child(div().h(px(30.)).flex_none().pl(px(14.)).flex().items_center().bg(crate::theme::light().panel).text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x333333)).mb(px(-4.)).child("Inspection"))
                // Live: one "Inspection items" group, then "Automatic inspection items".
                // Live 1710: column 105 (selects 106, 253 px), the Text row's
                // field followed by Bold and Italic.
                .child(section("Inspection items"))
                .child(trow("Item parsing", 106., 33., 2., sel("dp-iparse", if props.inspection_row_parsing { "Row".into() } else { "Parameter".into() }, parse_items, 253.)))
                .child(trow("Font", 106., 32., 2., font_row(FontOf::Inspection, props.inspection_font.clone(), 253.)))
                .child(trow(
                    "Text",
                    105.,
                    33.,
                    2.,
                    h_flex()
                        .items_start()
                        .child(field(0))
                        .child(Self::font_style_toggles(FontOf::Inspection, props.inspection_font.clone(), ("dp-ifont-b", "dp-ifont-i"), 3., cx))
                        .child(div().ml(px(3.)).child(swatch("dp-itcolor", props.inspection_text_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.inspection_text_color = c))))
                        .into_any_element(),
                ))
                .child(trow("Border shape", 106., 33., 2., sel("dp-ishape", props.inspection_shape.name().into(), shape_items, 253.)))
                .child(trow("Border size", 106., 32., 2., sel("dp-isize", border_chars_label(props.inspection_border_chars), border_items, 253.)))
                .child(trow("Border color", 105., 33., 2., swatch("dp-icolor", props.inspection_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.inspection_color = c))))
                // Live 1710: Solid fill's colour on the checkbox's row, at
                // the group's column.
                .child(
                    div()
                        .relative()
                        .child(check("dp-isolid", "Solid fill", props.inspection_solid_fill, |p, v| p.inspection_solid_fill = v, cx))
                        .child(div().absolute().left(px(104.)).top(px(1.)).child(swatch(
                            "dp-ifcolor",
                            Some(props.inspection_fill_color.unwrap_or(noshape_model::drawing::sheet::INSPECTION_FILL)),
                            std::rc::Rc::new(|p: &mut DrawingProps, c| p.inspection_fill_color = c),
                        ))),
                )
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
                        if this.drawing_props.tab != i {
                            // Each tab opens at its top.
                            this.drawing_props.scroll.set_offset(point(px(0.), px(0.)));
                        }
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
                // Live 1710: a #d1d1d1 line at the left, #dbdbdb at the right
                // (drawn in the 1 px right padding).
                .border_l_1()
                .border_color(gpui_kit::rgb(0xd1d1d1))
                .pr(px(1.))
                .child(div().absolute().top_0().bottom_0().right_0().w(px(1.)).bg(gpui_kit::rgb(0xdbdbdb)))
                .text_color(pal.text)
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                // Live: the title centred, 18.67 px / 500 `#1b5faa` on a 25 px line
                // 5 px down, no close button (the right strip toggles the panel);
                // the tabs 42 px down (1710).
                .child(div().h(px(42.)).flex_none().pt(px(5.)).flex().justify_center().child(div().h(px(25.)).line_height(px(25.)).text_size(px(18.67)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x1b5faa)).child("Drawing properties")))
                .child(tabs)
                .child({
                    let (scroll, flag) = (self.drawing_props.scroll.clone(), self.drawing_props.overflow.clone());
                    div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .child(body)
                        .when(overflow, |d| d.child(gpui_kit::component::menu::native_scrollbar(&self.drawing_props.scroll, cx)))
                        .on_drag_move(cx.listener(|this, e: &DragMoveEvent<gpui_kit::component::menu::NativeThumbDrag>, window, cx| {
                            gpui_kit::component::menu::native_scrollbar_drag(&this.drawing_props.scroll, e.event.position.y);
                            window.refresh();
                            cx.notify();
                        }))
                        // Known once laid out: draw again with the scrollbar.
                        .child(
                            canvas(
                                move |_, window, _| {
                                    let o = scroll.max_offset().y > px(0.5);
                                    if o != flag.get() {
                                        flag.set(o);
                                        window.request_animation_frame();
                                    }
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .size_0(),
                        )
                })
                .child(self.drawing_props_footer(cx))
                .into_any_element(),
        )
    }
}

impl DrawingEditor {
    /// The panel's footer (live drawing/18): "Update properties from a
    /// template…" with the DWT button (the standard templates), and
    /// ☐ Lock drawing properties.
    /// Bold and Italic for a text style (live drawing/1710, Inspection ›
    /// Text, Views › View labels): 26 px buttons `gap` px apart, pressed `#cfdbe5` with a 1 px
    /// `#1b5faa` line 1 px in, else `#f5f5f5`; SHX fonts disable them.
    fn font_style_toggles(w: FontOf, cur: Option<noshape_model::drawing::sheet::TextFont>, ids: (&'static str, &'static str), gap: f32, cx: &mut Context<Self>) -> AnyElement {
        use noshape_model::drawing::sheet::{DRAWING_DEFAULT_FONT, TextFont, is_shx};
        let (b, i) = cur.as_ref().map(|f| (f.bold, f.italic)).unwrap_or((false, false));
        let shx = cur.as_ref().is_some_and(|f| is_shx(&f.family));
        let toggle = |id: &'static str, glyph: &'static str, italic: bool, on: bool, f: fn(&mut TextFont), cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(26.))
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui_kit::rgb(if on { 0xcfdbe5 } else { 0xf5f5f5 }))
                .when(on, |d| d.child(div().absolute().top(px(1.)).left(px(1.)).right(px(1.)).bottom(px(1.)).border_1().border_color(gpui_kit::rgb(0x1b5faa))))
                .child(div().text_size(px(22.)).font_weight(FontWeight::BOLD).when(italic, |d| d.italic()).text_color(gpui_kit::rgb(0x333333)).child(glyph))
                .when(shx, |d| d.opacity(0.4))
                .when(!shx, |d| {
                    d.cursor_pointer().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_drawing_prop(&move |p| {
                            let slot = font_of(p, w);
                            let mut font = slot.clone().unwrap_or_else(|| TextFont { family: DRAWING_DEFAULT_FONT.into(), bold: false, italic: false });
                            f(&mut font);
                            *slot = Some(font);
                        }, cx)
                    }))
                })
        };
        h_flex()
            .gap(px(gap))
            .child(toggle(ids.0, "B", false, b, |f| f.bold = !f.bold, cx))
            .child(toggle(ids.1, "I", true, i, |f| f.italic = !f.italic, cx))
            .into_any_element()
    }

    /// A table row text's Bold and Italic (live drawing/1710 Tables: 26 px,
    /// 3 px apart, as Inspection's).
    fn row_text_toggles(k: usize, header: bool, rt: &RowText, ids: (&'static str, &'static str), cx: &mut Context<Self>) -> AnyElement {
        let toggle = |id: &'static str, glyph: &'static str, italic: bool, on: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(26.))
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .bg(gpui_kit::rgb(if on { 0xcfdbe5 } else { 0xf5f5f5 }))
                .when(on, |d| d.child(div().absolute().top(px(1.)).left(px(1.)).right(px(1.)).bottom(px(1.)).border_1().border_color(gpui_kit::rgb(0x1b5faa))))
                .child(div().text_size(px(22.)).font_weight(FontWeight::BOLD).when(italic, |d| d.italic()).text_color(gpui_kit::rgb(0x333333)).child(glyph))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.set_drawing_prop(&move |p| {
                        let r = row_text_mut(p, k, header);
                        if italic { r.italic = !r.italic } else { r.bold = !r.bold }
                    }, cx)
                }))
        };
        h_flex().gap(px(3.)).child(toggle(ids.0, "B", false, rt.bold, cx)).child(toggle(ids.1, "I", true, rt.italic, cx)).into_any_element()
    }

    fn drawing_props_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let ed = cx.entity();
        let locked = self.drawing.props.locked;
        let template = self.drawing.props.template.clone();
        let dwt = Button::new("dp-dwt").ghost().small().w(px(26.)).h(px(26.)).p_0().border_1().border_color(gpui_kit::rgb(0xd3d7d9)).rounded(px(0.)).child(crate::assets::icon(&crate::assets::icon_path("dwt-template"), 20., cx)).tooltip("Select a DWT").os_dropdown_menu("dp-dwt-dd", true, move |mut m, _, _| {
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
            // Live drawing/160, 1710: an input-like row 4 px under the rule
            // (13.33 px italic, its ink 15 px in, a #ddd underline up to the
            // 26 px DWT button 8 px from the panel's edge), then a 41 px row with the
            // 12 px Lock checkbox 9 px in, a 12 px label and a grey help icon
            // at the right.
            // Live "ui2 (10-26 b)": once the properties were updated from a
            // DWT, its name in a grey read-only box (301 x 27, #eee, #ccc,
            // 14 px #555 9 px in) and Lock drawing properties enabled;
            // before, the Lock checkbox is greyed (no tooltip).
            .child(
                h_flex()
                    .h(px(if template.is_some() { 33. } else { 30. }))
                    .pt(px(4.))
                    .pr(px(7.))
                    .gap(px(19.))
                    .child(match &template {
                        Some(name) => h_flex()
                            .flex_1()
                            .child(div().ml(px(8.)).w(px(301.)).h(px(27.)).px(px(8.)).flex().items_center().rounded(px(4.)).bg(gpui_kit::rgb(0xeeeeee)).border_1().border_color(gpui_kit::rgb(0xcccccc)).text_size(px(14.)).text_color(gpui_kit::rgb(0x555555)).child(div().truncate().child(name.clone())))
                            .into_any_element(),
                        None => div().flex_1().h(px(26.)).pl(px(14.)).flex().items_center().border_b_1().border_color(gpui_kit::rgb(0xdddddd)).text_size(px(13.33)).italic().text_color(gpui_kit::rgb(0x666666)).child("Update properties from a template…").into_any_element(),
                    })
                    .child(dwt),
            )
            .child(
                h_flex()
                    .h(px(41.))
                    .pb(px(4.))
                    .pl(px(9.))
                    .pr(px(9.))
                    .child(div().flex_1().text_size(px(12.)).child(crate::drawing_chrome::DrCheckbox::new("dp-lock").native().label("Lock drawing properties").checked(locked).disabled(template.is_none()).on_click(cx.listener(|this, v: &bool, _, cx| {
                        let v = *v;
                        this.set_drawing_prop(&move |p| p.locked = v, cx)
                    }))))
                    .child(div().size(px(14.)).mt(px(4.)).child(crate::assets::icon("help-filled", 14., cx))),
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
