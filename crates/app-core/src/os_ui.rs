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
pub const GLYPH_OWNED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><circle cx="10" cy="10" r="8.3" fill="none" stroke="currentColor" stroke-width="1.4"/><circle cx="10" cy="6.8" r="2.6"/><path d="M3.7 14.7c1.1-2.4 3.5-3.9 6.3-3.9s5.2 1.5 6.3 3.9a7.9 7.9 0 0 1-12.6 0z"/></svg>"#;
pub const GLYPH_RECENT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor"><circle cx="10" cy="10" r="7.25" stroke-width="1.5"/><path d="M10 6v4.6l3 3" stroke-width="1.1"/></svg>"#;
pub const GLYPH_FOLDER: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-linejoin="round"><path d="M1.5 3.5h6.6l1.6 2h8.8v11h-17z"/></svg>"#;
/// The folder filled (the dark theme's tab bar).
pub const GLYPH_FOLDER_FILLED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" stroke="currentColor" stroke-linejoin="round"><path d="M1.5 3.5h6.6l1.6 2h8.8v11h-17z"/></svg>"#;
/// Onshape's 10 px section toggles (`svg-icon-expanded` / collapsed).
pub const GLYPH_EXPANDED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8"><path d="M.95 3 5 6.85 9.05 3"/></svg>"#;
pub const GLYPH_COLLAPSED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8"><path d="M3 .95 6.85 5 3 9.05"/></svg>"#;
/// Onshape's overflow "•••" (`svg-icon-overflow`, 20 px).
pub const GLYPH_OVERFLOW: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><circle cx="3" cy="10" r="2"/><circle cx="10" cy="10" r="2"/><circle cx="17" cy="10" r="2"/></svg>"#;
pub const GLYPH_WORKSPACE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.4"><circle cx="7" cy="5.6" r="3.5"/><path d="M7 9v3.6"/></svg>"#;
pub const GLYPH_SORT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M4 3h3v9h1.8l-3.3 4.6L2.2 12H4zM10 3h8v3h-8zm0 5h8v3h-8zm0 5h8v3h-8z"/></svg>"#;
pub const GLYPH_GRIP: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><circle cx="4.4" cy="6.9" r="1.5"/><circle cx="9.4" cy="6.9" r="1.5"/><circle cx="14.4" cy="6.9" r="1.5"/><circle cx="4.4" cy="11.9" r="1.5"/><circle cx="9.4" cy="11.9" r="1.5"/><circle cx="14.4" cy="11.9" r="1.5"/></svg>"#;
pub const GLYPH_PLUS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M8.57 1.43h2.86v17.14H8.57z"/><path d="M1.43 8.57h17.14v2.86H1.43z"/></svg>"#;
pub const GLYPH_CREATED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><path d="M1.7 1.7h12.3l4.3 4.5v12.1H1.7z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/></svg>"#;
pub const GLYPH_TRASH: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><mask id="glyph_trash-m"><rect width="20" height="20" fill="#fff"/><path d="m6 6.8.7 8.2M9.5 6.8V15m3.5-8.2-.7 8.2" stroke="#000" stroke-linecap="round"/></mask><path d="M6 4V3.2C6 2.5 6.5 2 7.2 2h4.6c.7 0 1.2.5 1.2 1.2V4h-1V3H7v1z"/><path d="M3 4h13v1H3zm.8 1h11.4l-.8 12a1 1 0 0 1-1 .9H5.6a1 1 0 0 1-1-.9z" mask="url(#glyph_trash-m)"/></svg>"##;
pub const GLYPH_DOCUMENT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M1.5 1.5h12.3l4.7 4.8v12.2h-17zm12 0v5h5" fill="none" stroke="currentColor"/><path d="M10 7.2 14.5 9.8 10 12.4 5.5 9.8z" fill-opacity=".5"/><path d="M5.5 9.8 10 12.4v4.4l-4.5-2.6z"/><path d="M14.5 9.8 10 12.4v4.4l4.5-2.6z" fill-opacity=".75"/></svg>"#;
pub const GLYPH_OPEN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M10 2.1 15.6 8H12v6H8V8H4.4zM2 16h16v2H2z"/></svg>"#;
pub const GLYPH_HOME: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="m10 1 4 4V2h3v6l2 2v9h-7v-6H8v6H1v-9z"/></svg>"#;
pub const GLYPH_BACK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round"><path d="M11.4 4.4 6.6 10l4.8 5.6"/></svg>"#;
pub const GLYPH_SEARCH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-linecap="round"><circle cx="7.7" cy="7.7" r="5.6" stroke-width="2"/><path d="M12.2 12.2 17.4 17.4" stroke-width="2.6"/></svg>"#;
pub const GLYPH_CARET: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="currentColor"><path d="M0 2h8L4 6z"/></svg>"#;
pub const GLYPH_CARET_UP: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" fill="currentColor"><path d="M0 6h8L4 2z"/></svg>"#;
pub const GLYPH_CHEVRON_DOWN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2.8"><path d="m4.4 7.3 5.6 5.2 5.6-5.2"/></svg>"#;
pub const GLYPH_CHEVRON_RIGHT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2.8"><path d="m7.3 4.4 5.2 5.6-5.2 5.6"/></svg>"#;
/// Label (tag) glyph for the documents page's Labels.
pub const GLYPH_LABEL: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor"><path d="M1.75 4.75h13.4l3.9 5.25-3.9 5.25H1.75z" stroke-width="1.5"/><circle cx="13.1" cy="10" r="1.3" stroke-width="1.1"/></svg>"#;
/// Create-label glyph (tag with a plus).
pub const GLYPH_LABEL_ADD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M1.75 8.6V4.75h13.4l3.9 5.25-3.9 5.25H7.6m-4.2 0H1.75V13" fill="none" stroke="currentColor" stroke-width="1.5"/><circle cx="13.1" cy="10" r="1.3" fill="none" stroke="currentColor" stroke-width="1.1"/></svg>"#;
/// The plus of the "create label" glyph, drawn in blue over the tag.
pub const GLYPH_LABEL_ADD_PLUS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M5 7h2v8H5zm-3 3h8v2H2z"/></svg>"#;
/// Checklist marks (Publish FeatureScript): a heavy ✓ and a heavy ✗.
pub const GLYPH_CHECK_BOLD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="3.8" stroke-linejoin="round"><path d="M1 7.3 6.5 12.6 15 3.3"/></svg>"#;
pub const GLYPH_CROSS_BOLD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="m1.5 1.5 7 7M8.5 1.5l-7 7"/></svg>"#;
pub const GLYPH_CLOSE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="m6.3 6.3 7.4 7.4m0-7.4-7.4 7.4"/></svg>"#;
/// A modal's close ✕: 12 px of 1.5 px strokes in a 16 px box (live 1862, dk-pref-create-profile-dialog).
pub const GLYPH_MODAL_CLOSE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="m2 2 12 12M14 2 2 14"/></svg>"#;
pub const GLYPH_LIST_VIEW: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-opacity=".8"><path d="M.5 2.5h2.7l1.2.9h3.1v4.1h-7zm0 10h2.7l1.2.9h3.1v4.1h-7zM10 3.5h10M10 6.5h8m-8 7h10m-10 3h8"/></svg>"#;
pub const GLYPH_GRID_VIEW: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-opacity=".8"><path d="M1.5 2.5h2.7l1.2.9h3.1v4.1h-7zm10 0h2.7l1.2.9h3.1v4.1h-7zm-10 10h2.7l1.2.9h3.1v4.1h-7zm10 0h2.7l1.2.9h3.1v4.1h-7z"/></svg>"#;

/// The empty-list illustration: the noshape cube in outline (Onshape shows
/// its "On" mark the same way, light gray).
pub const GLYPH_EMPTY: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" fill="none" stroke="currentColor" stroke-width="4" stroke-linejoin="round" stroke-linecap="round"><mask id="glyph_empty-m"><rect width="100" height="100" fill="#fff" stroke="none"/><path d="M24.5 57 40 66l15.5-9M40 66v18" stroke="#000" stroke-width="2.6"/><ellipse cx="40" cy="57" rx="6.5" ry="3.6" fill="#000" stroke="none"/></mask><path d="M3 36.5Q19 28 38.5 22.5L59 32.5M38 25l6 11.5 11.5-2M60.5 45 80.5 79.5l-7-1.2Q50 84 25.5 93Q6 74 3 36.5M89.5 6.5 98 17 63 45.5l-16.5 5 8-15.5zM54.5 35l8.5 10.5"/><g transform="rotate(-12 40 66)"><path d="M40 48l15.5 9v18L40 84l-15.5-9V57z" fill="currentColor" stroke="none" mask="url(#glyph_empty-m)"/></g></svg>"##;

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

/// "Create label" (live documents/100): the tag in `color` with Onshape's
/// blue `#1651b0` plus on top.
pub fn label_add_glyph(size: f32, color: impl Into<Hsla>) -> Div {
    div()
        .relative()
        .size(px(size))
        .flex_none()
        .child(glyph(GLYPH_LABEL_ADD, size, color))
        .child(div().absolute().top_0().left_0().child(glyph(GLYPH_LABEL_ADD_PLUS, size, rgb(0x1651b0))))
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
        .child(div().text_size(px(14.)).text_color(gpui_kit::rgb_static(0xffffff)).child(label.into()))
        .h(px(34.))
        .px(px(11.))
        .rounded(px(3.))
}

/// Onshape's gray modal "Cancel" button: `#d7d7d7`, hover `#eeeeee`,
/// `#333` text. It stays light in the dark theme (live dark/dk-9bb Export
/// Drawing, dk-999 Properties: `btn-secondary` rgb(215,215,215)).
pub fn cancel_button(id: impl Into<ElementId>, label: impl Into<SharedString>, cx: &App) -> Button {
    let (fill, hover, text) = (gpui_kit::rgb_static(CANCEL_FILL), gpui_kit::rgb_static(CANCEL_HOVER), gpui_kit::rgb_static(0x333333));
    Button::new(id)
        .custom(ButtonCustomVariant::new(cx).color(fill.into()).foreground(text.into()).hover(hover.into()).active(fill.into()).shadow(false))
        .bg(fill)
        .child(div().text_size(px(14.)).text_color(text).child(label.into()))
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
        // Dark: the frame is a faint #3c3c3c (live dk-999), the rules #495057.
        .border_color(if noshape_scene::palette::is_dark() { gpui_kit::rgb_static(0x3c3c3c) } else { rgb(MODAL_DIVIDER) })
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
                        .child(Icon::empty().data(GLYPH_MODAL_CLOSE).size(px(16.))),
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
    checkbox_with(id, label.into().into_any_element(), on)
}

/// [`checkbox`] labelled with user-authored text (a label's name).
pub fn user_checkbox(id: impl Into<ElementId>, label: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    checkbox_with(id, crate::ui::user_text(label).into_any_element(), on)
}

fn checkbox_with(id: impl Into<ElementId>, label: AnyElement, on: bool) -> Stateful<Div> {
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
                .when(on, |d| d.child(Icon::new(gpui_kit::component::IconName::Check).size(px(10.)).text_color(gpui_kit::rgb_static(0xffffff)))),
        )
        .child(label)
}

/// Chrome's native checkbox box (13 px, `#0075ff` when checked): the Preferences pages' checkboxes and the drawing
/// dialogs' native ones (moved from `PreferencesView::checkbox_box`).
pub fn checkbox_box(on: bool) -> Div {
    div()
        .size(px(13.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(2.))
        .border_1()
        .border_color(if on { rgb(0x0075ff) } else { rgb(0x767676) })
        .bg(if on { rgb(0x0075ff) } else { rgb(0xffffff) })
        .when(on, |d| d.child(Icon::new(gpui_kit::component::IconName::Check).size(px(11.)).text_color(gpui_kit::rgb_static(0xffffff))))
}

/// A dialog row with a 12 px `#666` label at the left and a value field
/// (underlined, right-aligned number) at the right, 26 px tall (§3.9).
pub fn param_row(label: impl Into<SharedString>, field: impl IntoElement) -> Div {
    let pal = p();
    // Live (dom dialog-animate): 26 px rows, the label 5 px in. A 220 px
    // dialog's rows fill a 194 px first column (x 246–440), the 26 px right
    // column left for a row's icon (dom/dialog-reflection-analysis.txt: the
    // flip icon at 443).
    h_flex()
        .flex_none()
        .w(px(194.))
        .h(px(26.))
        .pl(px(5.))
        .pr(px(3.))
        .gap(px(6.))
        .child(div().flex_none().text_size(px(12.)).text_color(pal.text_muted).child(label.into()))
        .child(div().flex_1().min_w_0().h(px(26.)).border_b_1().border_color(pal.border).hover(|d| d.border_color(pal.accent)).child(field))
}

/// [`param_row`] for a field that draws its own underline (a
/// `quantity_field::QuantityInput`).
pub fn param_row_plain(label: impl Into<SharedString>, field: impl IntoElement) -> Div {
    let pal = p();
    h_flex()
        .flex_none()
        .w(px(194.))
        .h(px(26.))
        .pl(px(5.))
        .pr(px(3.))
        .gap(px(6.))
        .child(div().flex_none().text_size(px(12.)).text_color(pal.text_muted).child(label.into()))
        .child(div().flex_1().min_w_0().h(px(26.)).child(field))
}

/// A selection (query) box: white with a 1 px `#d7d7d7` outline, or
/// `#def1fb` with a `#1651b0` outline while it takes the picks. Empty it
/// shows `label` (12 px `#555`); with entries the label shrinks to the top.
pub fn query_box(id: impl Into<ElementId>, label: impl Into<SharedString>, active: bool, rows: Vec<AnyElement>) -> Stateful<Div> {
    query_box_tip(id, label, None, active, rows)
}

/// Live's query field carrying the dialog's error (dom/dialog-asm-replicate-*):
/// filled pink `#f9ebec` with a 1 px `#bd3039` border; the label stays grey.
pub fn query_box_error(el: Stateful<Div>) -> Stateful<Div> {
    let pal = p();
    el.bg(pal.error_fill).border_color(pal.error)
}

/// [`query_box`] whose label's tooltip is `tip` instead of the label (live:
/// a field in error shows the error, "Select one or more instances.").
pub fn query_box_tip(id: impl Into<ElementId>, label: impl Into<SharedString>, tip: Option<SharedString>, active: bool, rows: Vec<AnyElement>) -> Stateful<Div> {
    let pal = p();
    let empty = rows.is_empty();
    let label: SharedString = label.into();
    let tip = tip.unwrap_or_else(|| label.clone());
    gpui_kit::component::v_flex()
        .id(id)
        .min_h(px(26.))
        .rounded(px(2.))
        .border_1()
        .border_color(if active { pal.accent } else { pal.border })
        .bg(if active { pal.query_active } else { pal.panel })
        .cursor_pointer()
        .child(
            div()
                .id("query-label")
                .px(px(6.))
                // Live (dom dialog-mate-connector): `pad 6px 6px 5px` on a
                // 13.33 px line, 26 px with the border.
                .when(empty, |d| d.pt(px(6.)).pb(px(5.)).line_height(px(13.33)).text_size(px(12.)))
                .when(!empty, |d| d.pt(px(2.)).text_size(px(10.)))
                .text_color(rgb(0x555555))
                // Live: the label's tooltip is the label ("Mate connectors", "Instances").
                .tooltip(crate::ui::tooltip(tip))
                .child(label),
        )
        .children(rows)
}

/// One entry of a [`query_box`]: 24 px, 13.33 px `#555`, a ✕ on hover.
/// Its text names entities (instances, mates, "Face of Extrude 1"): user
/// text, never translated.
pub fn query_row(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    h_flex().id(id).h(px(24.)).pl(px(6.)).pr(px(4.)).gap(px(4.)).text_size(px(13.33)).text_color(rgb(0x555555)).child(div().flex_1().min_w_0().truncate().child(crate::ui::entity_text(text)))
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
    select_field_with(id, options, Vec::new(), current, on_select)
}

/// [`select_field`] with `disabled` options (greyed, not pickable: an
/// `<option disabled>`).
pub fn select_field_with(id: impl Into<ElementId>, options: Vec<String>, disabled: Vec<usize>, current: usize, on_select: impl Fn(usize, &mut Window, &mut App) + 'static) -> Stateful<Div> {
    select_field_inner(id, options, disabled, current, FieldOpts::default(), usize::MAX, on_select)
}

/// A plain `<select>` styled as an Onshape list field (`ns-enum-value`,
/// live Named views, psb/nv-911): 26 px, a 1 px `#d7d7d7` underline only,
/// black 14 px text 7 px in, Chrome's `#777` arrow 6 px from the end; it
/// opens Chrome's native list like [`select_field`]. Options from
/// `user_from` on are user content.
pub fn select_field_flat(id: impl Into<ElementId>, options: Vec<String>, disabled: Vec<usize>, user_from: usize, current: usize, on_select: impl Fn(usize, &mut Window, &mut App) + 'static) -> Stateful<Div> {
    select_field_inner(id, options, disabled, current, FieldOpts { focus: None, flat: true }, user_from, on_select)
}

/// [`select_field`] of user content (configuration options, parts, named
/// views…): the options are never translated.
pub fn user_select_field(id: impl Into<ElementId>, options: Vec<String>, current: usize, on_select: impl Fn(usize, &mut Window, &mut App) + 'static) -> Stateful<Div> {
    select_field_inner(id, options, Vec::new(), current, FieldOpts::default(), 0, on_select)
}

/// [`select_field`] whose options from `user_from` on are user content
/// (after fixed choices such as "None").
pub fn select_field_user_from(id: impl Into<ElementId>, options: Vec<String>, user_from: usize, current: usize, on_select: impl Fn(usize, &mut Window, &mut App) + 'static) -> Stateful<Div> {
    select_field_inner(id, options, Vec::new(), current, FieldOpts::default(), user_from, on_select)
}

/// [`user_select_field`] as a Tab stop (`focus`): a focused select opens
/// with Space, Enter or ↓, as a `<select>` (live Change configuration: Tab
/// walks Configuration, List input 1, d, ?).
pub fn user_select_field_focusable(id: impl Into<ElementId>, options: Vec<String>, current: usize, focus: &FocusHandle, on_select: impl Fn(usize, &mut Window, &mut App) + 'static) -> Stateful<Div> {
    select_field_inner(id, options, Vec::new(), current, FieldOpts { focus: Some(focus.clone()), flat: false }, 0, on_select)
}

/// How a [`select_field`] looks: a Tab stop (`focus`), Onshape's flat list field.
#[derive(Default)]
struct FieldOpts {
    focus: Option<FocusHandle>,
    flat: bool,
}

fn select_field_inner(id: impl Into<ElementId>, options: Vec<String>, disabled: Vec<usize>, current: usize, opts: FieldOpts, user_from: usize, on_select: impl Fn(usize, &mut Window, &mut App) + 'static) -> Stateful<Div> {
    let pal = p();
    let bounds: Rc<Cell<Bounds<Pixels>>> = Default::default();
    let on_select = Rc::new(on_select);
    let label = options.get(current).cloned().unwrap_or_default();
    let probe = bounds.clone();
    let open = Rc::new(move |window: &mut Window, cx: &mut App| {
        let (options, disabled, on_select, b) = (options.clone(), disabled.clone(), on_select.clone(), bounds.get());
        let width = f32::from(b.size.width);
        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
            m = m.native_select(width).focus_ring(b);
            for (i, o) in options.iter().enumerate() {
                let f = on_select.clone();
                let item = crate::os_menu::OsMenuItem::new(o.clone());
                let item = if i >= user_from { item.user() } else { item };
                m = m.item(item.checked(i == current).disabled(disabled.contains(&i)).on_click(move |_, window, cx| {
                    f(i, window, cx);
                    window.refresh();
                }));
            }
            m
        });
        crate::os_menu::open_anchored(point(b.origin.x, b.origin.y + b.size.height), Anchor::TopLeft, menu, window, cx);
    });
    let open_key = open.clone();
    div()
        .id(id)
        .when_some(opts.focus, |d, f| {
            d.track_focus(&f)
                // Chrome's focus ring on a focused `<select>` (2 px #3e94ff).
                // Dark: the ring outside the border is #90cef1 (live dk-pref-select-focused).
                .focus(|s| {
                    let outer = if noshape_scene::palette::is_dark() { 0x90cef1 } else { 0x3e94ff };
                    s.border_color(gpui_kit::rgb_static(0x3e94ff)).shadow(vec![BoxShadow { color: gpui_kit::rgb_static(outer).into(), offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(1.), inset: false }])
                })
                .on_key_down(move |ev: &KeyDownEvent, window, cx| {
                    if matches!(ev.keystroke.key.as_str(), "space" | "enter" | "down") {
                        cx.stop_propagation();
                        open_key(window, cx);
                    }
                })
        })
        .relative()
        .w_full()
        .flex()
        .items_center()
        .when(!opts.flat, |d| d.h(px(34.)).pl(px(11.)).pr(px(10.)).rounded(px(5.25)).border_1().border_color(rgb(0xaaaaaa)).text_color(pal.text))
        .when(opts.flat, |d| d.h(px(26.)).pl(px(6.)).pr(px(6.)).border_b_1().border_color(rgb(0xd7d7d7)).text_color(rgb(0x000000)))
        .bg(rgb(0xffffff))
        .cursor_pointer()
        .text_size(px(14.))
        .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
        .child(div().flex_1().truncate().map(|d| if current >= user_from { d.child(crate::ui::user_text(label)) } else { d.child(label) }))
        .child(glyph(GLYPH_CARET, 8., gpui_kit::rgb_static(0x777777))) // live assembly/181-182: `#777` (also in dark, prefs/dk-904)
        .on_click(move |_, window, cx| open(window, cx))
}

/// A solid circled × (Named views "Remove view", live psb/nv-911): the disc
/// in `currentColor` with the × cut out (our own drawing).
pub const GLYPH_REMOVE_CIRCLE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><mask id="rc-m" maskUnits="userSpaceOnUse" x="0" y="0" width="20" height="20"><rect width="20" height="20" fill="#fff"/><path d="M6.5 6.5L13.5 13.5M13.5 6.5L6.5 13.5" stroke="#000" stroke-width="2.1"/></mask><circle cx="10" cy="10" r="7" fill="currentColor" mask="url(#rc-m)"/></svg>"##;

// Feature Studio toolbar and search pane glyphs (our own drawings).
pub const GLYPH_ARROW_LEFT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><path d="M17 10H2.5M7.4 4.6 2.5 10l4.9 5.4"/></svg>"#;
pub const GLYPH_ARROW_RIGHT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><path d="M3 10h14.5M12.6 4.6l4.9 5.4-4.9 5.4"/></svg>"#;
pub const GLYPH_OUTLINE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M2 2h2v2h5v2H4v7h5v2H2z"/><g transform="matrix(.47 0 0 .47 9.9 .1)" fill="none" stroke="currentColor"><path d="M4.2 5.8C6.3 4.5 8.3 4.6 9 7.2l1.6 6.1c.5 2.1 2.5 2.3 4.7 1.4" stroke-width="2.8"/><path d="M14.6 4.6c-2.6 0-4.2 3.5-5.2 6-1 2.4-2.4 3.6-4.2 4" stroke-width="2.1"/><circle cx="14.7" cy="5.4" r="1.9" fill="currentColor" stroke="none"/><circle cx="4.6" cy="14.4" r="2.2" fill="currentColor" stroke="none"/></g><ellipse cx="14.5" cy="13.7" rx="2.85" ry="2.55" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M14 10h1v7.8h-1z"/></svg>"#;
pub const GLYPH_TEXT_MODE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M3.6 4a1 1 0 0 1 1-1h10.8a1 1 0 0 1 1 1v2h-1.8V5H11v10h2v2H7v-2h2V5H5.4v1H3.6z"/></svg>"#;
pub const GLYPH_SYMBOL_MODE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor"><path d="M4.2 5.8C6.3 4.5 8.3 4.6 9 7.2l1.6 6.1c.5 2.1 2.5 2.3 4.7 1.4" stroke-width="2.8"/><path d="M14.6 4.6c-2.6 0-4.2 3.5-5.2 6-1 2.4-2.4 3.6-4.2 4" stroke-width="2.1"/><circle cx="14.7" cy="5.4" r="1.9" fill="currentColor" stroke="none"/><circle cx="4.6" cy="14.4" r="2.2" fill="currentColor" stroke="none"/></svg>"#;
pub const GLYPH_IMPORT_MODE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M6.4 3.9H4.8c-.8 0-1.3.4-1.3 1.2v3.3c0 .9-.6 1.6-1.9 1.6 1.3 0 1.9.7 1.9 1.6v3.3c0 .8.5 1.2 1.3 1.2h1.6m7.2-12.2h1.6c.8 0 1.3.4 1.3 1.2v3.3c0 .9.6 1.6 1.9 1.6-1.3 0-1.9.7-1.9 1.6v3.3c0 .8-.5 1.2-1.3 1.2h-1.6" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M10 2v16" stroke="currentColor" stroke-width="1.6"/><circle cx="10" cy="10" r="3.8"/></svg>"#;
pub const GLYPH_FUNCTION: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M14.4 2.8c-1.2-.5-2.5 0-3.1 1.1-.6 1.2-.9 3-1.3 5.2l-1.2 6.2c-.4 2.1-1.6 3.5-3.5 3.2" fill="none" stroke="currentColor" stroke-width="2.1"/><path d="M8.4 8.5h4.8" stroke="currentColor" stroke-width=".9"/><circle cx="15.1" cy="3.8" r="1.3"/><circle cx="5.1" cy="17.4" r="1.3"/></svg>"#;
pub const GLYPH_EXPAND_ALL: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M6 5h8l-4-4zM6 16h8l-4 4zM2 7h16v1H2zm0 3h16v1H2zm0 3h16v1H2z"/></svg>"#;
pub const GLYPH_COLLAPSE_ALL: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M6 4h8l-4 3.9zM6 17h8l-4-3.9zM2 10h16v1H2z"/></svg>"#;
pub const GLYPH_NEW_FEATURE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M6 6.6l5.5 3v6l-5.5 3-5.5-3v-6zm-5.5 3 5.5 3 5.5-3M6 12.6v6M13 3.5h6m-6 3h6m-6 3h6m-6 3h6m-6 3h6" fill="none" stroke="currentColor"/><path d="M8 1v5l3.5-2.5z"/></svg>"#;
pub const GLYPH_FS_IMPORT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M7.8 8H15.6L11.5 13.8Z" fill-opacity=".45"/><g fill="none" stroke="currentColor" stroke-width="1.01"><path d="M8.5 4.5H3.5V15L6 17.5H16.5V7.5L13.5 4.5H12"/><path d="M3.5 4.5L6.5 7.5V17M6.5 7.5H16" stroke-opacity=".5"/><path d="M6.2 2.5H8.3C9.2 2.6 9.8 3.2 10.3 4C10.8 4.8 11.3 5.6 11.3 6.8V9.4" stroke-width="1.3"/></g><path d="M8.5 9H14.5L11.5 12.2Z"/></svg>"#;
pub const GLYPH_BRUSH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M17.2 3.3 12.9 8" stroke="currentColor" stroke-width="2.9" stroke-linecap="round"/><path d="M6.05 5.45 7.15 4.35 15.9 13.1 14.8 14.2z"/><path d="M0.95 10.25 5.2 6 14.35 15.15 10.1 19.4z"/></svg>"#;
pub const GLYPH_PULSE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor"><rect x="1.5" y="1.5" width="17" height="17" rx="1"/><path d="M2 11.5h3.1l1.1-3.5.9 4.6 1.5-9.6 1.6 13.8 2-7 .9 2 1.5-1.9.8 1.6H18" stroke-width="1.1" stroke-miterlimit="5"/></svg>"#;

/// Onshape menus on dropdown buttons (instead of the kit's `dropdown_menu`):
/// wraps the button, and a click opens `f`'s menu under it, as a Bootstrap
/// select list as wide as the button (`select`) or a dropdown.
pub trait OsDropdownExt: IntoElement + Sized {
    fn os_dropdown_menu(self, id: impl Into<ElementId>, select: bool, f: impl Fn(crate::os_menu::OsMenu, &mut Window, &mut Context<crate::os_menu::OsMenu>) -> crate::os_menu::OsMenu + 'static) -> Stateful<Div> {
        let bounds: Rc<Cell<Bounds<Pixels>>> = Default::default();
        let probe = bounds.clone();
        let f = Rc::new(f);
        let id: ElementId = id.into();
        let key = id.to_string();
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
                OPEN_DROPDOWNS.with(|o| o.borrow_mut().insert(key.clone(), menu.downgrade()));
                // Near the window's right edge the menu aligns to the button's right.
                if b.right() + px(260.) > window.viewport_size().width {
                    crate::os_menu::open_anchored(point(b.right(), b.bottom()), Anchor::TopRight, menu, window, cx);
                } else {
                    crate::os_menu::open_anchored(point(b.origin.x, b.bottom()), Anchor::TopLeft, menu, window, cx);
                }
            })
    }
}

thread_local! {
    /// The menu each dropdown (by its element id) opened last.
    static OPEN_DROPDOWNS: std::cell::RefCell<std::collections::HashMap<String, WeakEntity<crate::os_menu::OsMenu>>> = Default::default();
}

/// Whether the dropdown `id` ([`OsDropdownExt::os_dropdown_menu`]) has its menu open: its button keeps the open look
/// while the pointer is in the menu (live dk-913b: the documents page's Create stays #769fcc).
pub fn dropdown_open(id: &str, cx: &App) -> bool {
    OPEN_DROPDOWNS.with(|o| o.borrow().get(id).is_some_and(|m| crate::os_menu::is_open_menu(m, cx)))
}

/// A context-menu-style list opening upward from a button at the bottom of
/// the window, its bottom-right corner on the button's top-right (live
/// dom/menu-analysis-tools.txt: the Show analysis tools menu).
pub trait OsDropupExt: IntoElement + Sized {
    fn os_dropup_menu(self, id: impl Into<ElementId>, f: impl Fn(crate::os_menu::OsMenu, &mut Window, &mut Context<crate::os_menu::OsMenu>) -> crate::os_menu::OsMenu + 'static) -> Stateful<Div> {
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
                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| f(m, window, cx));
                crate::os_menu::open_anchored(point(b.right(), b.top()), Anchor::BottomRight, menu, window, cx);
            })
    }
}

impl OsDropupExt for Button {}

impl OsDropdownExt for Button {}
impl OsDropdownExt for Stateful<Div> {}
