//! Onshape-styled widgets used outside the Part Studio chrome: the buttons
//! and Bootstrap-style modal dialogs of Onshape's documents page and
//! document dialogs, info boxes, and the single-color glyphs of the
//! documents page. Colors and sizes are from the live Onshape captures in
//! /root/onshape-research/live/documents/ and docs/ONSHAPE_UI_SPEC.md §4.

use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::dialog::{Dialog, DialogClose};
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

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
pub const GLYPH_DOCUMENT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"><path d="M4.6 2.6h7.6l3.2 3.2v11.6H4.6z"/><path d="M7 10.5h6M7 13h6"/></svg>"#;
pub const GLYPH_OPEN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M9.3 3.4 5.2 7.5l1.1 1.1 2.4-2.4V13h1.6V6.2l2.4 2.4 1.1-1.1-4.1-4.1z"/><path d="M3.5 14.5h13V16h-13z"/></svg>"#;
pub const GLYPH_HOME: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M10 2.5 1.8 10h2.4v7h4.3v-4.5h3V17h4.3v-7h2.4z"/></svg>"#;
pub const GLYPH_BACK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M12.5 4.5 7 10l5.5 5.5"/></svg>"#;
pub const GLYPH_SEARCH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="8.2" cy="8.2" r="5.6"/><path d="m12.4 12.4 5 5"/></svg>"#;
pub const GLYPH_CARET: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="currentColor"><path d="M0 2.25h8L4 6.25z"/></svg>"#;
pub const GLYPH_CHEVRON_DOWN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m5.5 7.75 4.5 4.5 4.5-4.5"/></svg>"#;
pub const GLYPH_CHEVRON_RIGHT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m7.75 5.5 4.5 4.5-4.5 4.5"/></svg>"#;
pub const GLYPH_CLOSE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="m4.5 4.5 11 11M15.5 4.5l-11 11"/></svg>"#;
pub const GLYPH_LIST_VIEW: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.25"><path d="M2.6 3.1h4.3l1.5 1.5v4.3H2.6zM2.6 11.1h4.3l1.5 1.5v4.3H2.6z" stroke-linejoin="round"/><path d="M10.5 5h7M10.5 7.5h5M10.5 13h7M10.5 15.5h5"/></svg>"#;
pub const GLYPH_GRID_VIEW: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.25"><path d="M2.6 2.6h6.3v6.3H2.6zM11.1 2.6h6.3v6.3h-6.3zM2.6 11.1h6.3v6.3H2.6zM11.1 11.1h6.3v6.3h-6.3z"/></svg>"#;

/// The empty-list illustration: the noshape cube in outline (Onshape shows
/// its "On" mark the same way, light gray).
pub const GLYPH_EMPTY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" fill="none" stroke="currentColor" stroke-width="3.5" stroke-linejoin="round"><path d="M50 8 88 29v42L50 92 12 71V29z"/><path d="M12 29l38 21 38-21M50 50v42"/><ellipse cx="50" cy="29" rx="12" ry="6.5"/></svg>"#;

/// A documents-page glyph as a tinted mask.
pub fn glyph(data: &'static [u8], size: f32, color: impl Into<Hsla>) -> Icon {
    Icon::empty().data(data).size(px(size)).text_color(color.into())
}

/// Onshape's primary button: `#1651b0`, hover `#769fcc`, 34 px, radius 3,
/// 14 px white text.
pub fn primary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).primary().child(div().text_size(px(14.)).child(label.into())).h(px(34.)).px(px(11.)).rounded(px(3.))
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
                .child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).whitespace_nowrap().child(title))
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
pub fn modal_ok(button: Button) -> Div {
    div().flex_none().child(gpui_kit::component::dialog::DialogAction::new().child(button))
}

/// A footer button that closes the dialog.
pub fn modal_cancel(button: Button) -> Div {
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

/// A documents-page menu row (Create ▾): 30 px, 18 px glyph, 13.33 px
/// text, like Onshape's "Document…" / "Folder…" items.
pub fn menu_item(glyph_data: &'static [u8], label: &'static str) -> gpui_kit::component::menu::PopupMenuItem {
    gpui_kit::component::menu::PopupMenuItem::element(move |_, _| {
        let pal = p();
        h_flex()
            .h(px(30.))
            .w_full()
            .gap(px(8.))
            .pl(px(8.))
            .child(glyph(glyph_data, 18., pal.text))
            .child(div().text_size(px(13.33)).text_color(pal.text).child(label))
    })
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
    h_flex()
        .h(px(30.))
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
                .when(empty, |d| d.py(px(5.)).text_size(px(12.)))
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


/// Onshape's message bubble (`.osx-message-bubble`, live DOM): 14 px text
/// in a rounded (5 px) box, padding 4/25/4/10, an 18 px icon 10 px before
/// the text and a bold 21 px "×" (`#467bb6`) at the right. Info is
/// `#def1fb` / `#b2ddf6` / `#094174`; warnings and errors use the theme's
/// warning and error fills. Position it yourself (Onshape: top center).
/// Icons: msg-info / msg-warning / msg-error.svg (our own artwork).
pub fn message_bubble(id: impl Into<ElementId>, kind: crate::ui::AlertKind, message: impl Into<SharedString>, on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static, cx: &App) -> Stateful<Div> {
    use crate::ui::AlertKind;
    let (fill, outline, text) = crate::ui::alert_colors(kind);
    let icon = match kind {
        AlertKind::Info => "msg-info",
        AlertKind::Warning => "msg-warning",
        AlertKind::Error => "msg-error",
    };
    h_flex()
        .id(id)
        .occlude()
        .relative()
        .max_w(px(640.))
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
        .child(div().min_w_0().child(message.into()))
        .child(
            div()
                .id("bubble-x")
                .absolute()
                .right(px(11.))
                .top(px(0.))
                .cursor_pointer()
                .text_size(px(21.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0x467bb6))
                .child("×")
                .on_click(on_close),
        )
}
