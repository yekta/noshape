//! Onshape-styled widgets used outside the Part Studio chrome: the buttons
//! and Bootstrap-style modal dialogs of Onshape's documents page and
//! document dialogs, info boxes, and the single-color glyphs of the
//! documents page. Colors and sizes are from the live Onshape captures in
//! /root/onshape-research/live/documents/ and docs/ONSHAPE_UI_SPEC.md §4.

use crate::theme::p;

/// A click callback shared between menu rows (`os_menu::OsMenuItem::Item`).
pub type ClickHandler = std::rc::Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::dialog::{Dialog, DialogClose};
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::Cell;
use std::rc::Rc;

/// Modal outline and header/footer dividers (Bootstrap's `#dee2e6`, as
/// Onshape's modals render).
pub const MODAL_DIVIDER: u32 = 0xdee2e6;
/// Gray "Cancel" button fill and hover (`--os-button-fill-cancel*`).
const CANCEL_FILL: u32 = 0xd7d7d7;
const CANCEL_HOVER: u32 = 0xeeeeee;

// Documents-page glyphs on Onshape's 20-unit grid, drawn as masks.
pub const GLYPH_OWNED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><circle cx="10" cy="10" r="8.25" fill="none" stroke="currentColor" stroke-width="1.5"/><circle cx="10" cy="7.7" r="3.1" fill="currentColor"/><path d="M4.5 15.5c1.2-2.3 3.2-3.5 5.5-3.5s4.3 1.2 5.5 3.5A7.6 7.6 0 0 1 10 17.8a7.6 7.6 0 0 1-5.5-2.3z" fill="currentColor"/></svg>"#;
pub const GLYPH_RECENT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="10" cy="10" r="8.25"/><path d="M10 5.25V10l3.25 2.25"/></svg>"#;
pub const GLYPH_FOLDER: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"><path d="M2.6 4.6h5l1.6 1.8h8.2v9h-14.8z"/></svg>"#;
/// Onshape's 10 px section toggles (`svg-icon-expanded` / collapsed).
pub const GLYPH_EXPANDED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M1.6 3.2 5 6.6l3.4-3.4"/></svg>"#;
pub const GLYPH_COLLAPSED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M3.2 1.6 6.6 5 3.2 8.4"/></svg>"#;
/// Onshape's overflow "•••" (`svg-icon-overflow`, 20 px).
pub const GLYPH_OVERFLOW: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><circle cx="3" cy="10" r="1.8"/><circle cx="10" cy="10" r="1.8"/><circle cx="17" cy="10" r="1.8"/></svg>"#;
pub const GLYPH_WORKSPACE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.3"><circle cx="7" cy="4.6" r="2.9"/><path d="M7 7.5v5.2" stroke-linecap="round"/></svg>"#;
pub const GLYPH_SORT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M4.2 3h1.6v10.6l2-2 1.1 1.1L5 16.6l-3.9-3.9 1.1-1.1 2 2z"/><path d="M10.5 4h8v1.6h-8zM10.5 8h6v1.6h-6zM10.5 12h4v1.6h-4z"/></svg>"#;
pub const GLYPH_GRIP: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><circle cx="7.5" cy="5" r="1.4"/><circle cx="12.5" cy="5" r="1.4"/><circle cx="7.5" cy="10" r="1.4"/><circle cx="12.5" cy="10" r="1.4"/><circle cx="7.5" cy="15" r="1.4"/><circle cx="12.5" cy="15" r="1.4"/></svg>"#;
pub const GLYPH_PLUS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M10 3.5v13M3.5 10h13"/></svg>"#;
pub const GLYPH_CREATED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><path d="M3.7 2.7h12.6v14.6H3.7z"/></svg>"#;
pub const GLYPH_TRASH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M7.6 2.2h4.8l.6 1.4h3.6v1.6H3.4V3.6H7zM4.6 6.2h10.8l-.9 11.6H5.5zm2.6 1.9v7.7h1.3V8.1zm2.2 0v7.7h1.2V8.1zm2.1 0v7.7h1.3V8.1z" fill-rule="evenodd"/></svg>"#;
pub const GLYPH_DOCUMENT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"><path d="M4.6 2.6h7.6l3.2 3.2v11.6H4.6z"/><path d="M7 10.5h6M7 13h6"/></svg>"#;
pub const GLYPH_OPEN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M9.3 3.4 5.2 7.5l1.1 1.1 2.4-2.4V13h1.6V6.2l2.4 2.4 1.1-1.1-4.1-4.1z"/><path d="M3.5 14.5h13V16h-13z"/></svg>"#;
pub const GLYPH_HOME: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M10 2.5 1.8 10h2.4v7h4.3v-4.5h3V17h4.3v-7h2.4z"/></svg>"#;
pub const GLYPH_BACK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M12.5 4.5 7 10l5.5 5.5"/></svg>"#;
pub const GLYPH_SEARCH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="8.2" cy="8.2" r="5.6"/><path d="m12.4 12.4 5 5"/></svg>"#;
pub const GLYPH_CARET: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="currentColor"><path d="M0 2.25h8L4 6.25z"/></svg>"#;
pub const GLYPH_CARET_UP: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="currentColor"><path d="M0 5.75h8L4 1.75z"/></svg>"#;
pub const GLYPH_CHEVRON_DOWN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m5.5 7.75 4.5 4.5 4.5-4.5"/></svg>"#;
pub const GLYPH_CHEVRON_RIGHT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m7.75 5.5 4.5 4.5-4.5 4.5"/></svg>"#;
/// Label (tag) glyph for the documents page's Labels.
pub const GLYPH_LABEL: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><path d="M2.7 4.7h10.6l4 5.3-4 5.3H2.7z"/><circle cx="12.6" cy="10" r="1.2"/></svg>"#;
/// Create-label glyph (tag with a plus).
pub const GLYPH_LABEL_ADD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><path d="M6.7 5.7h7.6l3.4 4.3-3.4 4.3H6.7z"/><path d="M1.5 10h5M4 7.5v5" stroke-linecap="round"/></svg>"#;
/// Checklist marks (Publish FeatureScript): a heavy ✓ and a heavy ✗.
pub const GLYPH_CHECK_BOLD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linejoin="round"><path d="m1.6 8.6 4.1 3.9L14.6 3.6"/></svg>"#;
pub const GLYPH_CROSS_BOLD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="m1.5 1.5 7 7M8.5 1.5l-7 7"/></svg>"#;
pub const GLYPH_CLOSE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="m4.5 4.5 11 11M15.5 4.5l-11 11"/></svg>"#;
pub const GLYPH_LIST_VIEW: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.25"><path d="M2.6 3.1h4.3l1.5 1.5v4.3H2.6zM2.6 11.1h4.3l1.5 1.5v4.3H2.6z" stroke-linejoin="round"/><path d="M10.5 5h7M10.5 7.5h5M10.5 13h7M10.5 15.5h5"/></svg>"#;
pub const GLYPH_GRID_VIEW: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.25"><path d="M2.6 2.6h6.3v6.3H2.6zM11.1 2.6h6.3v6.3h-6.3zM2.6 11.1h6.3v6.3H2.6zM11.1 11.1h6.3v6.3h-6.3z"/></svg>"#;

/// The empty-list illustration: the noshape cube in outline (Onshape shows
/// its "On" mark the same way, light gray).
pub const GLYPH_EMPTY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" fill="none" stroke="currentColor" stroke-width="3.5" stroke-linejoin="round"><path d="M50 8 88 29v42L50 92 12 71V29z"/><path d="M12 29l38 21 38-21M50 50v42"/><ellipse cx="50" cy="29" rx="12" ry="6.5"/></svg>"#;

/// A documents-page glyph as a tinted mask.
/// A single-line input `h` px tall. The kit's own `Input::h` sizes only
/// multi-line inputs (single-line ones stay at its 28 px); the styled
/// height set here applies after the kit's.
pub fn input_h(input: gpui_kit::component::input::Input, h: f32) -> gpui_kit::component::input::Input {
    Styled::h(input, px(h))
}

pub fn glyph(data: &'static [u8], size: f32, color: impl Into<Hsla>) -> Icon {
    Icon::empty().data(data).size(px(size)).text_color(color.into())
}

/// Onshape's primary button: `#1651b0`, hover `#769fcc`, 34 px, radius 3,
/// 14 px white text.
pub fn primary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).primary().child(div().text_size(px(14.)).child(label.into())).h(px(34.)).px(px(11.)).rounded(px(3.))
}

/// Onshape's danger button (Bootstrap `btn-danger`, live documents/70
/// "Delete forever"): `#bd3039`, white 14 px text.
pub fn danger_button(id: impl Into<ElementId>, label: impl Into<SharedString>, cx: &App) -> Button {
    Button::new(id)
        .custom(ButtonCustomVariant::new(cx).color(rgb(0xbd3039).into()).foreground(rgb(0xffffff).into()).hover(rgb(0xd27177).into()).active(rgb(0xa3272f).into()).shadow(false))
        .bg(rgb(0xbd3039))
        .child(div().text_size(px(14.)).text_color(rgb(0xffffff)).child(label.into()))
        .h(px(34.))
        .px(px(11.))
        .rounded(px(3.))
}

/// Onshape's gray modal "Cancel" button: `#d7d7d7`, hover `#eeeeee`.
pub fn cancel_button(id: impl Into<ElementId>, label: impl Into<SharedString>, cx: &App) -> Button {
    let pal = p();
    Button::new(id)
        .custom(
            ButtonCustomVariant::new(cx)
                .color(rgb(CANCEL_FILL).into())
                .foreground(pal.text.into())
                .hover(rgb(CANCEL_HOVER).into())
                .active(rgb(CANCEL_FILL).into())
                .shadow(false),
        )
        .bg(rgb(CANCEL_FILL))
        .child(div().text_size(px(14.)).child(label.into()))
        .h(px(34.))
        .px(px(11.))
        .rounded(px(3.))
}

/// Onshape's secondary button: white with a 1 px `#aaaaaa` outline, hover
/// `#eaeaea`.
pub fn secondary_button(id: impl Into<ElementId>, label: impl Into<SharedString>, cx: &App) -> Button {
    let pal = p();
    Button::new(id)
        .custom(
            ButtonCustomVariant::new(cx)
                .color(pal.panel.into())
                .foreground(pal.text.into())
                .hover(pal.panel_hover.into())
                .active(pal.panel_active.into())
                .shadow(false),
        )
        .bg(pal.panel)
        .border_1()
        .border_color(pal.border_strong)
        .child(div().text_size(px(14.)).child(label.into()))
        .h(px(34.))
        .px(px(11.))
        .rounded(px(3.))
}

/// Restyle a kit dialog as an Onshape modal: white, square-ish corners,
/// a 47 px header with an 18 px title and a gray ×, and dividers above and
/// below the body. Put the body in `.child(..)` with [`modal_body`] and
/// the buttons in `.footer(modal_footer()..)`.
pub fn modal(dialog: Dialog, title: impl Into<SharedString>, width: f32) -> Dialog {
    modal_with(dialog, title, width, false)
}

/// [`modal`] as the documents page draws it: the title `#333`, weight 500
/// (live documents/10, 43, 47), where the document window's is `#666`.
pub fn modal_doc_page(dialog: Dialog, title: impl Into<SharedString>, width: f32) -> Dialog {
    modal_with(dialog, title, width, true)
}

fn modal_with(dialog: Dialog, title: impl Into<SharedString>, width: f32, dark: bool) -> Dialog {
    let pal = p();
    let title: SharedString = title.into();
    dialog
        .w(px(width))
        .margin_top(px(26.))
        .close_button(false)
        .p_0()
        .gap_0()
        .rounded(px(4.))
        .border_color(rgb(MODAL_DIVIDER))
        .title(
            h_flex()
                .w(px(width - 2.))
                .h(px(47.))
                .pl(px(10.))
                .pr(px(12.))
                .border_b_1()
                .border_color(rgb(MODAL_DIVIDER))
                // Live (document/80, drawing/90, document/23): 18 px `#666`, regular.
                .child(div().text_size(px(18.)).when(dark, |d| d.text_color(rgb(0x333333)).font_weight(FontWeight::MEDIUM)).when(!dark, |d| d.text_color(pal.text_muted)).whitespace_nowrap().child(title))
                .child(div().flex_1())
                .child(div().flex_none().child(DialogClose::new().child(
                    div()
                        .id("modal-close")
                        .size(px(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(pal.text_faint)
                        .hover(|d| d.text_color(pal.text_muted))
                        .child(Icon::empty().data(GLYPH_CLOSE).size(px(16.))),
                ))),
        )
}

/// Modal body: 10 px padding, 14 px text.
pub fn modal_body() -> Div {
    gpui_kit::component::v_flex().px(px(10.)).pt(px(8.)).pb(px(10.)).gap(px(6.)).text_size(px(14.)).text_color(p().text)
}

/// Modal footer: right-aligned buttons, 8 px apart, below a divider.
pub fn modal_footer() -> Div {
    h_flex().w_full().justify_end().gap(px(8.)).px(px(14.)).py(px(13.)).border_t_1().border_color(rgb(MODAL_DIVIDER))
}

/// A footer button that confirms the dialog (runs its `on_ok`).
pub fn modal_ok(button: impl IntoElement) -> Div {
    div().flex_none().child(gpui_kit::component::dialog::DialogAction::new().child(button))
}

/// A footer button that closes the dialog.
pub fn modal_cancel(button: impl IntoElement) -> Div {
    div().flex_none().child(DialogClose::new().child(button))
}

/// A bold 14 px field label ("Document name").
pub fn field_label(text: impl Into<SharedString>) -> Div {
    div().pt(px(4.)).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(p().text).child(text.into())
}

/// An info box ("No additional folders"): `#def1fb`, outline `#b2ddf6`,
/// text `#094174`.
pub fn info_box(text: impl Into<SharedString>) -> Div {
    let pal = p();
    div().px(px(14.)).py(px(15.)).rounded(px(3.)).bg(pal.info_fill).border_1().border_color(pal.info_outline).text_size(px(14.)).text_color(pal.info_text).child(text.into())
}

/// Onshape's dialog checkbox row: 27 px, a 12 px box (1 px `#888`, radius
/// 3; checked `#1651b0` with a white check) and a 13.33 px `#666` label
/// (`#333` on hover). Add `.on_click`.
pub fn checkbox(id: impl Into<ElementId>, label: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    let pal = p();
    h_flex()
        .id(id)
        .h(px(27.))
        .pl(px(4.))
        .gap(px(4.))
        .cursor_pointer()
        .text_size(px(13.33))
        .text_color(pal.text_muted)
        .hover(|d| d.text_color(pal.text))
        .child(
            div()
                .size(px(12.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(if on { pal.accent } else { rgb(0x888888) })
                .bg(if on { pal.accent } else { rgb(0xffffff) })
                .when(on, |d| d.child(Icon::new(gpui_kit::component::IconName::Check).size(px(10.)).text_color(rgb(0xffffff)))),
        )
        .child(label.into())
}

/// A dialog row with a 13.33 px `#666` label at the left and a value field
/// (underlined, right-aligned number) at the right, 26 px tall (§3.9).
pub fn param_row(label: impl Into<SharedString>, field: impl IntoElement) -> Div {
    let pal = p();
    // Live (dom dialog-animate): 26 px rows, the label 5 px in.
    h_flex()
        .h(px(26.))
        .pl(px(5.))
        .pr(px(3.))
        .gap(px(6.))
        .child(div().flex_none().text_size(px(13.33)).text_color(pal.text_muted).child(label.into()))
        .child(div().flex_1().min_w_0().h(px(26.)).border_b_1().border_color(pal.border).hover(|d| d.border_color(pal.accent)).child(field))
}

/// A selection (query) box: white with a 1 px `#d7d7d7` outline, or
/// `#def1fb` with a `#1651b0` outline while it takes the picks. Empty it
/// shows `label` (12 px `#555`); with entries the label shrinks to the top.
pub fn query_box(id: impl Into<ElementId>, label: impl Into<SharedString>, active: bool, rows: Vec<AnyElement>) -> Stateful<Div> {
    let pal = p();
    let empty = rows.is_empty();
    gpui_kit::component::v_flex()
        .id(id)
        .min_h(px(26.))
        .rounded(px(2.))
        .border_1()
        .border_color(if active { pal.accent } else { pal.border })
        .bg(if active { pal.info_fill } else { pal.panel })
        .cursor_pointer()
        .child(
            div()
                .px(px(6.))
                // Live (dom dialog-mate-connector): `pad 6px 6px 5px` on a
                // 13.33 px line, 26 px with the border.
                .when(empty, |d| d.pt(px(6.)).pb(px(5.)).line_height(px(13.33)).text_size(px(12.)))
                .when(!empty, |d| d.pt(px(2.)).text_size(px(10.)))
                .text_color(rgb(0x555555))
                .child(label.into()),
        )
        .children(rows)
}

/// One entry of a [`query_box`]: 24 px, 13.33 px `#555`, a ✕ on hover.
pub fn query_row(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    h_flex().id(id).h(px(24.)).pl(px(6.)).pr(px(4.)).gap(px(4.)).text_size(px(13.33)).text_color(rgb(0x555555)).child(div().flex_1().min_w_0().truncate().child(text.into()))
}

/// Hover/selected styling of a plain clickable row.
pub fn row_states(d: Stateful<Div>, selected: bool) -> Stateful<Div> {
    let pal = p();
    d.when(selected, |d| d.bg(pal.accent_soft)).when(!selected, |d| d.hover(|d| d.bg(pal.row_hover)))
}


/// The look of a message bubble: the three alert kinds plus Onshape's green
/// success bubble ("✓ Folder created. ×").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BubbleKind {
    Info,
    Warning,
    Error,
    Success,
}

impl From<crate::ui::AlertKind> for BubbleKind {
    fn from(k: crate::ui::AlertKind) -> Self {
        match k {
            crate::ui::AlertKind::Info => Self::Info,
            crate::ui::AlertKind::Warning => Self::Warning,
            crate::ui::AlertKind::Error => Self::Error,
        }
    }
}

/// Onshape's message bubble (`.osx-message-bubble`, live dom/toast-*.txt,
/// assembly-bom-copy-toast.txt): 14 px text in a rounded (5.25 px) box,
/// padding 4/25/4/10 plus the 12 px close cell, an 18 px icon 10 px before
/// the text and a bold 21 px "×" at the right. Info `#def1fb` / `#b2ddf6` /
/// `#094174` (× `#467bb6`), warning (× `#987107`), error (× `#bd3039`),
/// success `#ddf6cc` / `#a2db7d` / `#333` (× `#028502`). Position it
/// yourself (Onshape: top center, 8–9 px down). Icons: toast-*.svg.
pub fn message_bubble(id: impl Into<ElementId>, kind: impl Into<BubbleKind>, message: impl IntoElement, on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static, cx: &App) -> Stateful<Div> {
    use crate::ui::AlertKind;
    let kind = kind.into();
    let (fill, outline, text) = match kind {
        BubbleKind::Info => crate::ui::alert_colors(AlertKind::Info),
        BubbleKind::Warning => crate::ui::alert_colors(AlertKind::Warning),
        BubbleKind::Error => crate::ui::alert_colors(AlertKind::Error),
        BubbleKind::Success => (rgb(0xddf6cc), rgb(0xa2db7d), rgb(0x333333)),
    };
    let (icon, close) = match kind {
        BubbleKind::Info => ("toast-info", 0x467bb6),
        BubbleKind::Warning => ("toast-warning", 0x987107),
        BubbleKind::Error => ("toast-error", 0xbd3039),
        BubbleKind::Success => ("toast-success", 0x028502),
    };
    h_flex()
        .id(id)
        .occlude()
        .relative()
        .max_w(px(1000.))
        .min_h(px(29.))
        .pl(px(10.))
        .pr(px(37.))
        .py(px(4.))
        .gap(px(10.))
        .rounded(px(5.25))
        .bg(fill)
        .border_1()
        .border_color(outline)
        .text_size(px(14.))
        .text_color(text)
        .child(crate::assets::icon(icon, 18., cx))
        .child(div().min_w_0().child(message))
        .child(
            div()
                .id("bubble-x")
                .absolute()
                .right(px(5.))
                .top(px(0.))
                .bottom(px(0.))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_size(px(21.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(close))
                .child("×")
                .on_click(on_close),
        )
}

/// A message bubble with Onshape's action link after the message
/// (`.osx-message-bubble-link`: 14 px, weight 600, underlined, the text
/// color), e.g. "Sketch 2 has been canceled. **Restore**".
pub fn message_bubble_with_link(
    id: impl Into<ElementId>,
    kind: impl Into<BubbleKind>,
    message: impl IntoElement,
    link: impl Into<SharedString>,
    on_link: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    message_bubble(id, kind, message, on_close, cx).child(div().id("bubble-link").ml(px(-6.)).cursor_pointer().font_weight(FontWeight::SEMIBOLD).underline().child(link.into()).on_click(on_link))
}

/// Onshape's form select (Bootstrap `form-control` select, live DOM): full
/// width, 34 px, 1 px `#aaa`, radius 5.25, 14 px text 11 px in, a caret at
/// the right. A `<select>`, so Chrome's native list opens under it
/// (`#1967d2` highlight, live assembly/182) and the field shows the 2 px
/// `#3e94ff` focus ring while it is open.
pub fn select_field(id: impl Into<ElementId>, options: Vec<String>, current: usize, on_select: impl Fn(usize, &mut Window, &mut App) + 'static) -> Stateful<Div> {
    let pal = p();
    let bounds: Rc<Cell<Bounds<Pixels>>> = Default::default();
    let on_select = Rc::new(on_select);
    let label = options.get(current).cloned().unwrap_or_default();
    let probe = bounds.clone();
    div()
        .id(id)
        .relative()
        .h(px(34.))
        .w_full()
        .flex()
        .items_center()
        .pl(px(11.))
        .pr(px(10.))
        .rounded(px(5.25))
        .border_1()
        .border_color(rgb(0xaaaaaa))
        .bg(rgb(0xffffff))
        .cursor_pointer()
        .text_size(px(14.))
        .text_color(pal.text)
        .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
        .child(div().flex_1().truncate().child(label))
        .child(glyph(GLYPH_CARET, 8., rgb(0x777777))) // live assembly/181-182: `#777`
        .on_click(move |_, window, cx| {
            let (options, on_select, b) = (options.clone(), on_select.clone(), bounds.get());
            let width = f32::from(b.size.width);
            let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                m = m.native_select(width).focus_ring(b);
                for (i, o) in options.iter().enumerate() {
                    let f = on_select.clone();
                    m = m.item(crate::os_menu::OsMenuItem::new(o.clone()).checked(i == current).on_click(move |_, window, cx| {
                        f(i, window, cx);
                        window.refresh();
                    }));
                }
                m
            });
            crate::os_menu::open_anchored(point(b.origin.x, b.origin.y + b.size.height), Anchor::TopLeft, menu, window, cx);
        })
}

// Feature Studio toolbar and search pane glyphs (our own drawings).
pub const GLYPH_ARROW_LEFT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M16.5 10h-13M8.5 5 3.5 10l5 5"/></svg>"#;
pub const GLYPH_ARROW_RIGHT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 10h13M11.5 5l5 5-5 5"/></svg>"#;
pub const GLYPH_OUTLINE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.3"><rect x="2.6" y="2.6" width="14.8" height="14.8" rx="1.5"/><path d="M5.5 6h4M7.5 6v6M5.5 12h4M12 8.5l3 3M15 8.5l-3 3" stroke-linecap="round"/></svg>"#;
pub const GLYPH_TEXT_MODE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M3.5 3.5h13v3.2h-1.6V5.1h-4.1v10.3h1.8V17H7.4v-1.6h1.8V5.1H5.1v1.6H3.5z"/></svg>"#;
pub const GLYPH_SYMBOL_MODE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M6.2 5.2c1.4 0 2.2.8 2.8 2.2l.7 1.6 2.6-3.6h2.1l-3.9 5.3 1.3 3c.3.7.6 1 1.1 1 .3 0 .6-.1.9-.3l.4 1.3c-.5.3-1 .5-1.7.5-1.3 0-2-.8-2.6-2.1l-.7-1.6-2.8 3.8H4.4l4.1-5.5-1.2-2.8c-.3-.7-.6-1-1.1-1-.3 0-.6.1-.9.3l-.4-1.3c.5-.4 1-.6 1.3-.6z"/></svg>"#;
pub const GLYPH_IMPORT_MODE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3.5c-1.6 0-2 .8-2 2v2.3c0 1.1-.6 2.2-1.6 2.2 1 0 1.6 1.1 1.6 2.2v2.3c0 1.2.4 2 2 2M14 3.5c1.6 0 2 .8 2 2v2.3c0 1.1.6 2.2 1.6 2.2-1 0-1.6 1.1-1.6 2.2v2.3c0 1.2-.4 2-2 2"/><circle cx="10" cy="10" r="2" fill="currentColor" stroke="none"/></svg>"#;
pub const GLYPH_FUNCTION: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="2" y="2" width="16" height="16" rx="3"/><path d="M10 5v10"/><ellipse cx="10" cy="10" rx="3.6" ry="3"/></svg>"#;
pub const GLYPH_EXPAND_ALL: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 10h14M3 7h14M3 13h14M7 4l3-2.5L13 4M7 16l3 2.5 3-2.5"/></svg>"#;
pub const GLYPH_COLLAPSE_ALL: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 10h14M7 2.5l3 3 3-3M7 17.5l3-3 3 3"/></svg>"#;
pub const GLYPH_NEW_FEATURE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"><path d="M3 7.5 9 4.5l6 3v6l-6 3-6-3z"/><path d="M3 7.5l6 3 6-3M9 10.5v6"/><path d="M15.5 2v5M13 4.5h5" stroke-linecap="round"/></svg>"#;
pub const GLYPH_FS_IMPORT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"><path d="M5 2.6h7l3 3v11.8H5z"/><path d="M10 7v7M7.2 11.2 10 14l2.8-2.8" stroke-linecap="round"/></svg>"#;
pub const GLYPH_BRUSH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M16.6 2.2c.5.4.5 1.1.1 1.6L10.4 11l-1.9-1.7 6.6-6.9c.4-.5 1.1-.5 1.5-.2z"/><path d="M7.6 10.1l2 1.8c-.3 2.6-2.4 4.8-6.6 5.1.9-1 1.1-2 1.3-3 .3-2.2 1.6-3.6 3.3-3.9z"/></svg>"#;
pub const GLYPH_PULSE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"><rect x="2.6" y="2.6" width="14.8" height="14.8" rx="1.5"/><path d="M4.5 11h2.5l1.5-4 2.5 7 1.5-3h3" stroke-linecap="round"/></svg>"#;

/// Onshape menus on dropdown buttons (instead of the kit's `dropdown_menu`):
/// wraps the button, and a click opens `f`'s menu under it, as a Bootstrap
/// select list as wide as the button (`select`) or a dropdown.
pub trait OsDropdownExt: IntoElement + Sized {
    fn os_dropdown_menu(self, id: impl Into<ElementId>, select: bool, f: impl Fn(crate::os_menu::OsMenu, &mut Window, &mut Context<crate::os_menu::OsMenu>) -> crate::os_menu::OsMenu + 'static) -> Stateful<Div> {
        let bounds: Rc<Cell<Bounds<Pixels>>> = Default::default();
        let probe = bounds.clone();
        let f = Rc::new(f);
        div()
            .id(id)
            .relative()
            .flex_none()
            .child(self)
            .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .on_click(move |_, window, cx| {
                let (f, b) = (f.clone(), bounds.get());
                let width = f32::from(b.size.width);
                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| {
                    let m = if select { m.select_list(width) } else { m.dropdown() };
                    f(m, window, cx)
                });
                // Near the window's right edge the menu aligns to the button's right.
                if b.right() + px(260.) > window.viewport_size().width {
                    crate::os_menu::open_anchored(point(b.right(), b.bottom()), Anchor::TopRight, menu, window, cx);
                } else {
                    crate::os_menu::open_anchored(point(b.origin.x, b.bottom()), Anchor::TopLeft, menu, window, cx);
                }
            })
    }
}

impl OsDropdownExt for Button {}
impl OsDropdownExt for Stateful<Div> {}
