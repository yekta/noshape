//! Onshape-styled building blocks shared by every tab: toolbars, tool
//! buttons, the tool search box, panel rails, tree rows, alerts and
//! tooltips. Measurements and colors follow docs/ONSHAPE_UI_SPEC.md.

use crate::assets;
use crate::theme::{p, with_alpha};
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// Document top bar (§3.1).
pub const NAVBAR_H: f32 = 40.;
/// Feature / sketch / assembly / drawing toolbar (§3.2).
pub const TOOLBAR_H: f32 = 36.;
/// Left document-panel rail (§3.3).
pub const RAIL_W: f32 = 40.;
/// Bottom tab bar (§3.5).
pub const TAB_BAR_H: f32 = 30.;
/// Tree and list rows (§3.4).
pub const ROW_H: f32 = 26.;
/// List row text: 10pt (§2.2).
pub const ROW_TEXT: f32 = 13.33;
/// Toolbar icons and their button cell (§3.2).
pub const TOOL_ICON: f32 = 20.;
pub const TOOL_CELL: f32 = 30.;
/// Feature-list icons (17 px at 0.8 opacity, §3.4).
pub const LIST_ICON: f32 = 17.;
/// Left panel default width (§3.4).
pub const LEFT_PANEL_W: f32 = 210.;

/// Onshape's simple tooltip: blue-gray `#d0dbe5`, 12 px / 500, square corners (§4.7).
pub fn tooltip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text: SharedString = text.into();
    move |window, cx| {
        let pal = p();
        Tooltip::new(text.clone())
            .bg(pal.tooltip)
            .text_color(pal.text)
            .border_0()
            .rounded(px(0.))
            .shadow_sm()
            .px(px(7.))
            .pt(px(4.))
            .pb(px(3.))
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .build(window, cx)
    }
}

/// Note: a custom variant paints only 20% of `color` at rest, so solid
/// fills (the active tool) are set with `.bg()` on the button instead.
fn tool_variant(cx: &App, active: bool) -> ButtonCustomVariant {
    let pal = p();
    ButtonCustomVariant::new(cx)
        .color(gpui_kit::transparent_white())
        .foreground(pal.text.into())
        .hover(if active { pal.tool_active.into() } else { pal.tool_hover.into() })
        .active(pal.tool_active.into())
        .shadow(false)
}

/// A 30x30 toolbar button holding a 20 px full-color icon (§3.2). Add
/// `.on_click` / `.dropdown_menu` to the result.
pub fn tool_button(id: impl Into<ElementId>, icon: &str, active: bool, cx: &App) -> Button {
    Button::new(id)
        .custom(tool_variant(cx, active))
        .child(assets::icon(icon, TOOL_ICON, cx))
        .w(px(TOOL_CELL))
        .h(px(TOOL_CELL))
        .p_0()
        .rounded(px(3.))
        .when(active, |b| b.bg(p().tool_active))
}

/// A toolbar button with an icon and a label, like Onshape's "Sketch" (§3.2).
pub fn text_tool_button(id: impl Into<ElementId>, icon: &str, label: impl Into<SharedString>, active: bool, cx: &App) -> Button {
    let pal = p();
    let label: SharedString = label.into();
    Button::new(id)
        .custom(tool_variant(cx, active))
        .child(h_flex().gap(px(5.)).child(assets::icon(icon, TOOL_ICON, cx)).child(div().text_size(px(14.)).text_color(pal.text).child(label)))
        .h(px(TOOL_CELL))
        .pl(px(5.))
        .pr(px(8.))
        .rounded(px(3.))
        .when(active, |b| b.bg(p().tool_active))
}

/// The 15 px dropdown caret cell next to a split tool (§3.2).
pub fn tool_caret(id: impl Into<ElementId>, cx: &App) -> Button {
    let pal = p();
    Button::new(id)
        .custom(tool_variant(cx, false))
        .child(glyph("ui-chevron-down", 9., pal.text))
        .w(px(15.))
        .h(px(TOOL_CELL))
        .p_0()
        .rounded(px(3.))
}

/// Wrap a toolbar element so it shows an Onshape tooltip.
pub fn with_tooltip(id: impl Into<ElementId>, text: impl Into<SharedString>, el: impl IntoElement) -> AnyElement {
    div().id(id).flex_none().tooltip(tooltip(text)).child(el).into_any_element()
}

/// The toolbar row: white, 36 px, no bottom border (§3.2).
pub fn toolbar() -> Div {
    h_flex().h(px(TOOLBAR_H)).flex_none().px(px(4.)).gap(px(2.)).bg(p().panel).overflow_hidden()
}

/// Toolbar group separator: 2 px wide, 26 px tall, `#d7d7d7` (§3.2).
pub fn tool_separator() -> Div {
    div().flex_none().w(px(2.)).h(px(26.)).mx(px(4.)).bg(p().border)
}

/// The "Search tools…" box at the right end of a toolbar (§3.2).
pub fn search_tools_box(id: impl Into<ElementId>) -> Stateful<Div> {
    let pal = p();
    let chip = |t: &'static str| div().px(px(3.)).h(px(15.)).flex().items_center().rounded(px(2.)).border_1().border_color(pal.field_outline).bg(pal.panel).text_size(px(10.)).text_color(pal.text).child(t);
    h_flex()
        .id(id)
        .flex_none()
        .h(px(30.))
        .px(px(10.))
        .gap(px(6.))
        .rounded(px(3.))
        .bg(pal.search_fill)
        .hover(|d| d.bg(pal.border))
        .cursor_pointer()
        .child(div().text_size(px(14.)).text_color(pal.text).child("Search tools…"))
        .child(h_flex().gap(px(2.)).child(chip("alt/⌥")).child(chip("c")))
}

/// Toolbar undo / redo pair (Onshape puts them first in every toolbar).
pub fn undo_redo(doc: &Entity<crate::doc_model::DocModel>, cx: &App) -> AnyElement {
    let d = doc.read(cx);
    let (can_undo, can_redo) = (d.session.can_undo(), d.session.can_redo());
    let undo_tip: SharedString = d.session.undo_description().map(|t| format!("Undo {t} (Ctrl+Z)")).unwrap_or_else(|| "Undo (Ctrl+Z)".into()).into();
    let redo_tip: SharedString = d.session.redo_description().map(|t| format!("Redo {t} (Ctrl+Y)")).unwrap_or_else(|| "Redo (Ctrl+Y)".into()).into();
    let (d1, d2) = (doc.clone(), doc.clone());
    h_flex()
        .flex_none()
        .child(with_tooltip(
            "tt-undo",
            undo_tip,
            tool_button("tb-undo", "undo", false, cx).opacity(if can_undo { 1.0 } else { 0.4 }).on_click(move |_, _, cx| {
                if can_undo {
                    d1.update(cx, |d, cx| d.undo(cx));
                }
            }),
        ))
        .child(with_tooltip(
            "tt-redo",
            redo_tip,
            tool_button("tb-redo", "redo", false, cx).opacity(if can_redo { 1.0 } else { 0.4 }).on_click(move |_, _, cx| {
                if can_redo {
                    d2.update(cx, |d, cx| d.redo(cx));
                }
            }),
        ))
        .into_any_element()
}

/// Panel header: "Features (11)" style, 14 px / 600 (§3.4).
pub fn panel_title(text: impl Into<SharedString>) -> Div {
    div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(p().text).child(text.into())
}

/// A 26 px tree row with the standard hover / selected fills (§3.4).
pub fn tree_row(id: impl Into<ElementId>, selected: bool) -> Stateful<Div> {
    let pal = p();
    h_flex()
        .id(id)
        .h(px(ROW_H))
        .flex_none()
        .pl(px(5.))
        .pr(px(3.))
        .gap(px(3.))
        .text_size(px(ROW_TEXT))
        .text_color(pal.text)
        .when(selected, |d| d.bg(pal.accent_soft))
        .when(!selected, |d| d.hover(|d| d.bg(pal.row_hover)))
}

/// A tree icon: 17 px at 0.8 opacity in a 20 px cell (§3.4).
pub fn tree_icon(name: &str, dim: bool, cx: &App) -> Div {
    div().w(px(20.)).flex_none().flex().items_center().justify_center().opacity(if dim { 0.4 } else { 0.8 }).child(assets::icon(name, LIST_ICON, cx))
}

/// Collapsible caret (▾/▸) in a 16 px column (§4.11).
pub fn caret(open: bool) -> Div {
    div()
        .w(px(16.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(glyph(if open { "ui-chevron-down" } else { "ui-chevron-right" }, 11., p().text))
}

/// A single-color glyph (chevrons, carets): an app icon drawn as a mask
/// tinted with `color`.
pub fn glyph(name: &str, size: f32, color: impl Into<Hsla>) -> Svg {
    svg().path(assets::icon_path(name)).size(px(size)).flex_none().text_color(color.into())
}

/// Colors of an alert (§4.9): fill, outline, text.
pub fn alert_colors(kind: AlertKind) -> (Rgba, Rgba, Rgba) {
    let pal = p();
    match kind {
        AlertKind::Info => (pal.info_fill, pal.info_outline, pal.info_text),
        AlertKind::Warning => (pal.warn_fill, pal.warn_outline, pal.text),
        AlertKind::Error => (pal.error_fill, pal.error_outline, pal.text),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    Info,
    Warning,
    Error,
}

/// An alert box (top-center toasts), without positioning (§4.9).
pub fn alert(kind: AlertKind) -> Div {
    let (fill, outline, text) = alert_colors(kind);
    h_flex().gap(px(7.)).px(px(10.)).py(px(6.)).rounded(px(4.)).bg(fill).border_1().border_color(outline).text_size(px(13.)).text_color(text)
}

/// A button on a side rail: full width x 31 px, 20 px icon (§3.3, §3.6).
/// `active` draws the 4 px blue indicator on the given edge.
pub fn rail_button(id: impl Into<ElementId>, icon: &str, active: bool, indicator_left: bool, cx: &App) -> Stateful<Div> {
    let pal = p();
    div()
        .id(id)
        .relative()
        .w_full()
        .h(px(31.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|d| d.bg(pal.panel_hover))
        .child(assets::icon(icon, TOOL_ICON, cx))
        .when(active, |d| {
            let bar = div().absolute().top_0().bottom_0().w(px(4.)).bg(pal.accent);
            d.bg(with_alpha(pal.accent, 0.08)).child(if indicator_left { bar.left_0() } else { bar.right_0() })
        })
}
