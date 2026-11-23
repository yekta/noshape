//! A mate row's flyout (live2 asm-feel 11-17 (1), errors-1119 lo): the row's tooltip, right of the list and centred on
//! the row, 228 px wide or as wide as its title: the mate's name and state bold over `#D0DBE5`, then its parameters
//! read-only as in its dialog, a value grey on its field's line (only the type while they are all at their defaults). It fades in from about 110 ms after the pointer arrives; the next row's replaces it
//! at once (a warm tooltip).

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// How long after the pointer arrives the flyout starts to show, and how long its fade takes (live: ~110 ms, fully
/// shown by ~270 ms).
const SHOW_DELAY: std::time::Duration = std::time::Duration::from_millis(110);
const FADE: std::time::Duration = std::time::Duration::from_millis(150);
const WIDTH: f32 = 228.;
const HEAD_H: f32 = 26.;
const ROW_H: f32 = 26.;
const PAD: f32 = 4.;
/// The flyout's widest (live errors-1123 mates: a longer title wraps, its bar 400 px wide and 44 px high on two lines).
const MAX_W: f32 = 400.;
const HEAD_LINE: f32 = 18.;
/// The body's left edge and a read-only field's width (live2 errors-1119 lo: "Revolute" 11 px in, its line 212 px).
const BODY_X: f32 = 11.;
const FIELD_W: f32 = 212.;

/// One read-only row of the flyout's body.
#[derive(Clone, Debug, PartialEq)]
pub enum FlyRow {
    Text(String),
    Check(String, bool),
}

pub struct MateFlyout {
    head: SharedString,
    rows: Vec<FlyRow>,
    /// The row's bounds.
    anchor: Bounds<Pixels>,
    /// What live's recorder reads of it.
    note: String,
}

impl MateFlyout {
    /// The flyout's height: the header (`head_lines` lines) and the body's rows.
    fn height(&self, head_lines: usize) -> f32 {
        HEAD_H + HEAD_LINE * head_lines.saturating_sub(1) as f32 + if self.rows.is_empty() { 0. } else { self.rows.len() as f32 * ROW_H + 2. * PAD }
    }
}

impl Render for MateFlyout {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !crate::os_menu::is_open(cx) {
            crate::ui_script::note_tooltip(&self.note);
        }
        let ink = gpui_kit::rgb(0x333333);
        let check = |on: bool| {
            div().size(px(13.)).flex_none().border_1().border_color(gpui_kit::rgb(0x999999)).rounded(px(2.)).bg(gpui_kit::rgb(0xffffff)).flex().items_center().justify_center().when(on, |d| d.child(div().size(px(7.)).bg(gpui_kit::rgb(0x555555))))
        };
        let body = (!self.rows.is_empty()).then(|| {
            div().py(px(PAD)).pl(px(BODY_X)).pr(px(PAD)).bg(gpui_kit::rgb(0xffffff)).flex().flex_col().children(self.rows.iter().map(|r| match r {
                FlyRow::Text(t) => div()
                    .h(px(ROW_H))
                    .w(px(FIELD_W))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xdddddd))
                    .text_size(px(13.))
                    .text_color(gpui_kit::rgb(0x999999))
                    .child(crate::ui::user_text(t.clone()))
                    .into_any_element(),
                FlyRow::Check(t, on) => div().h(px(ROW_H)).flex().items_center().gap(px(6.)).text_size(px(14.)).child(check(*on)).child(t.clone()).into_any_element(),
            }))
        });
        // A name-only tip (an instance row's) is as wide as its text (live2 13:05: "Part 1 <1>" 63 wide).
        // The theme's font, as the app's other tooltips: a tooltip view doesn't inherit it, and the fallback has no
        // bold face (live2 recheck-1119 (7): the title read regular).
        let content = div()
            .font_family(cx.theme().font_family.clone())
            .when(!self.rows.is_empty(), |d| d.min_w(px(WIDTH)))
            .max_w(px(MAX_W))
            .whitespace_nowrap()
            .text_size(px(12.))
            .text_color(ink)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(1.)), blur_radius: px(2.), spread_radius: px(0.), inset: false }])
            .child(div().min_h(px(HEAD_H)).py(px((HEAD_H - HEAD_LINE) / 2.)).px(px(PAD)).flex().items_center().whitespace_normal().bg(gpui_kit::rgb(0xd0dbe5)).font_weight(FontWeight::BOLD).line_height(px(HEAD_LINE)).child(crate::ui::user_text(self.head.clone())))
            .children(body)
            .with_animation("mate-flyout-fade", Animation::new(FADE), |c, t| c.opacity(t));
        // (Bold runs ~6 % wider than the regular face text_width measures.)
        let head_lines = (crate::ui::text_width(window, &self.head, 12.) * 1.06 / (MAX_W - 2. * PAD)).ceil().max(1.) as usize;
        let a = self.anchor;
        let at = point(a.right() + px(3.), a.center().y - px(self.height(head_lines) / 2.));
        div().child(anchored().position(at).snap_to_window().child(content))
    }
}

/// An instance row's tooltip (live2 asm-feel2 (a)): its name alone, in the flyout's place and look, cut off or not.
pub fn with_row_tip(row: Stateful<Div>, text: String) -> Stateful<Div> {
    with_mate_flyout(row, text.clone(), Vec::new(), text)
}

/// Gives a mate row its flyout: `head` ("Revolute 1 (Hidden)"), the body's `rows`, and `note`, what the recorder
/// reads.
pub fn with_mate_flyout(row: Stateful<Div>, head: String, rows: Vec<FlyRow>, note: String) -> Stateful<Div> {
    let anchor: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
    let a2 = anchor.clone();
    row.relative()
        .child(canvas(move |b, _, _| a2.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
        .tooltip(move |_, cx| {
            let (head, rows, note, anchor) = (head.clone().into(), rows.clone(), note.clone(), anchor.get());
            cx.new(|_| MateFlyout { head, rows, anchor, note }).into()
        })
        .tooltip_show_delay(SHOW_DELAY)
}
