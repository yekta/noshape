//! Onshape-styled building blocks shared by every tab: toolbars, tool
//! buttons, the tool search box, panel rails, tree rows, alerts and
//! tooltips. Measurements and colors follow docs/ONSHAPE_UI_SPEC.md.

use crate::assets;
use crate::theme::{p, with_alpha};
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
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
pub const TOOL_CELL: f32 = 32.;
/// The dropdown caret cell of a split tool (§3.2, measured 18 px live).
pub const TOOL_CARET_W: f32 = 18.;
/// Feature-list icons (17 px at 0.8 opacity, §3.4).
pub const LIST_ICON: f32 = 17.;
/// Left panel default width (§3.4).
pub const LEFT_PANEL_W: f32 = 206.;

/// Onshape's simple tooltip: blue-gray `#d0dbe5`, 12 px / 500, square corners (§4.7).
/// Width in pixels of `text` in the window's font at `size` px (layout
/// decisions Onshape's browser makes by measuring, such as wrapping).
pub fn text_width(window: &Window, text: &str, size: f32) -> f32 {
    let style = window.text_style();
    let run = TextRun { len: text.len(), font: style.font(), color: style.color, background_color: None, underline: None, strikethrough: None };
    f32::from(window.text_system().shape_line(SharedString::from(text.to_string()), px(size), &[run], None).width)
}

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
fn tool_variant(cx: &App, active: bool, grouped: bool) -> ButtonCustomVariant {
    let pal = p();
    let hover = if active {
        pal.tool_active
    } else if grouped {
        pal.tool_hover_group
    } else {
        pal.tool_hover
    };
    ButtonCustomVariant::new(cx)
        .color(gpui_kit::transparent_white())
        .foreground(pal.text.into())
        .hover(hover.into())
        .active(pal.tool_active.into())
        .shadow(false)
}

/// A 32x32 toolbar button holding a 20 px full-color icon (§3.2). Add
/// `.on_click` / `.dropdown_menu` to the result.
pub fn tool_button(id: impl Into<ElementId>, icon: &str, active: bool, cx: &App) -> Button {
    Button::new(id)
        .custom(tool_variant(cx, active, false))
        .child(assets::icon(icon, TOOL_ICON, cx))
        .w(px(TOOL_CELL))
        .h(px(TOOL_CELL))
        .p_0()
        .rounded(px(3.))
        .when(active, |b| b.bg(p().tool_active))
}

/// The icon half of a split tool: like [`tool_button`], but with the
/// group hover (`#d7d7d7`) and only the left corners rounded. Put it in a
/// [`tool_group`] with a [`tool_caret`].
pub fn split_tool_button(id: impl Into<ElementId>, icon: &str, active: bool, cx: &App) -> Button {
    Button::new(id)
        .custom(tool_variant(cx, active, true))
        .child(assets::icon(icon, TOOL_ICON, cx))
        .w(px(TOOL_CELL))
        .h(px(TOOL_CELL))
        .p_0()
        .rounded(px(0.))
        .rounded_l(px(3.))
        .when(active, |b| b.bg(p().tool_active))
}

/// A split tool: hovering either half fills both in `#d7d7d7` (live).
pub fn tool_group(name: impl Into<SharedString>, main: impl IntoElement, caret: impl IntoElement) -> Div {
    let pal = p();
    let name: SharedString = name.into();
    let (n1, n2) = (name.clone(), name.clone());
    h_flex()
        .flex_none()
        .group(name)
        .child(div().rounded_l(px(3.)).group_hover(n1, |d| d.bg(pal.tool_hover_group)).child(main))
        .child(div().rounded_r(px(3.)).group_hover(n2, |d| d.bg(pal.tool_hover_group)).child(caret))
}

/// Onshape's toolbar tooltip: the title ("Extrude (shift+e)") appears first;
/// after 2 s of hovering it expands in place with the tool's description,
/// 400 px wide.
pub struct RichTooltip {
    title: SharedString,
    description: SharedString,
    expanded: bool,
    /// The tool's bounds: the tooltip is centered under it.
    anchor: Bounds<Pixels>,
}

/// Width of the expanded tooltip (live).
const RICH_TOOLTIP_W: f32 = 400.;

impl Render for RichTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        // Live (part-studio/10, 11; dom/hover-states.json): both states sit
        // 3 px under the tool, centered on it; a 26 px title row, 12 px / 500,
        // 4 px padding; the description 12 px on 18 px lines, 4 px padding.
        // Placed in window coordinates (`anchored`), so gpui's pointer-based
        // tooltip origin doesn't matter. Tooltips are drawn outside the
        // window's element tree, so the UI font is set here.
        let shadow = vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(2.)), blur_radius: px(3.), spread_radius: px(0.), inset: false }];
        let title = div().px(px(4.)).h(px(26.)).flex().items_center().bg(pal.tooltip).font_weight(FontWeight::MEDIUM).whitespace_nowrap().child(self.title.clone());
        let body = if self.expanded {
            v_flex()
                .w(px(RICH_TOOLTIP_W))
                .bg(gpui_kit::rgb(0xffffff))
                .child(title)
                .child(div().p(px(4.)).line_height(px(18.)).child(self.description.clone()))
                .into_any_element()
        } else {
            title.into_any_element()
        };
        let content = div().font_family(cx.theme().font_family.clone()).text_size(px(12.)).text_color(pal.text).shadow(shadow).child(body);
        let a = self.anchor;
        // The title-only box's width isn't known before layout: center it
        // with a 400 px strip whose content is centered.
        let at = point(a.center().x - px(RICH_TOOLTIP_W * 0.5), a.bottom() + px(3.));
        // Never drawn over an open context menu (QA3: a stale tooltip covered its items).
        div().when(!crate::os_menu::is_open(cx), |d| d.child(anchored().position(at).snap_to_window().child(div().w(px(RICH_TOOLTIP_W)).flex().justify_center().child(content))))
    }
}

/// Tooltip closure for a toolbar tool (see [`RichTooltip`]); `anchor` holds
/// the tool's last painted bounds.
pub fn rich_tooltip(title: impl Into<SharedString>, description: impl Into<SharedString>, anchor: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let title: SharedString = title.into();
    let description: SharedString = description.into();
    move |_, cx| {
        let anchor = anchor.get();
        let view = cx.new(|cx| {
            if !description.is_empty() {
                cx.spawn(async move |this: WeakEntity<RichTooltip>, cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
                    let _ = this.update(cx, |t, cx| {
                        t.expanded = true;
                        cx.notify();
                    });
                })
                .detach();
            }
            RichTooltip { title: title.clone(), description: description.clone(), expanded: false, anchor }
        });
        view.into()
    }
}

/// A square icon button with no fill at rest and `#eaeaea` on hover (§4.1):
/// panel headers, the viewport's quick tools. `on` draws the toggled fill.
pub fn icon_button(id: impl Into<ElementId>, icon: &str, size: f32, on: bool, cx: &App) -> Button {
    let pal = p();
    let variant = ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false);
    Button::new(id).custom(variant).child(assets::icon(icon, TOOL_ICON, cx)).w(px(size)).h(px(size)).p_0().rounded(px(5.)).when(on, |b| b.bg(pal.panel_active))
}

/// A toolbar button with an icon and a label, like Onshape's "Sketch" (§3.2).
pub fn text_tool_button(id: impl Into<ElementId>, icon: &str, label: impl Into<SharedString>, active: bool, cx: &App) -> Button {
    let pal = p();
    let label: SharedString = label.into();
    Button::new(id)
        .custom(tool_variant(cx, active, false))
        .child(h_flex().gap(px(4.)).child(assets::icon(icon, TOOL_ICON, cx)).child(div().text_size(px(14.)).text_color(pal.text).child(label)))
        .h(px(TOOL_CELL))
        .pl(px(6.))
        .pr(px(10.))
        .rounded(px(3.))
        .when(active, |b| b.bg(p().tool_active))
}

/// The 18 px dropdown caret cell next to a split tool (§3.2).
pub fn tool_caret(id: impl Into<ElementId>, cx: &App) -> Button {
    let pal = p();
    Button::new(id)
        .custom(tool_variant(cx, false, true))
        // Live: a 10 x 5 px caret, about 2 px thick.
        .child(glyph("tool-caret", 12., pal.text))
        .w(px(TOOL_CARET_W))
        .h(px(TOOL_CELL))
        .p_0()
        .rounded(px(0.))
        .rounded_r(px(3.))
}

/// A keyboard shortcut chip (`shift`, `f`): 11 px text in a 1 px `#aaa`
/// box with 2 px radius, as in Onshape's menus and the tool search box.
pub fn shortcut_chip(text: impl Into<SharedString>) -> Div {
    let pal = p();
    div().px(px(4.)).h(px(18.)).flex().items_center().rounded(px(2.)).border_1().border_color(pal.field_outline).bg(pal.panel).text_size(px(11.)).text_color(pal.text).child(text.into())
}

/// Shortcut chips for an Onshape shortcut string such as "shift-f" or "ctrl+m".
pub fn shortcut_chips(shortcut: &str) -> Div {
    let keys: Vec<String> = shortcut.split(['-', '+']).filter(|k| !k.is_empty()).map(|k| k.to_lowercase()).collect();
    h_flex().gap(px(2.)).children(keys.into_iter().map(shortcut_chip))
}

/// A toolbar dropdown row (§3.2, measured live): 32 px tall, a 20 px
/// full-color icon in a 30 px cell, the name at 14 px and the shortcut
/// chips at the right. Add `.on_click` to the result.
pub fn menu_tool_item(name: &'static str, icon: &str, shortcut: Option<&'static str>) -> gpui_kit::component::menu::PopupMenuItem {
    let icon = icon.to_string();
    gpui_kit::component::menu::PopupMenuItem::element(move |_, cx| {
        let pal = p();
        h_flex()
            .h(px(32.))
            .w_full()
            .gap(px(4.))
            .child(div().w(px(30.)).flex_none().flex().items_center().justify_center().child(assets::icon(&icon, TOOL_ICON, cx)))
            .child(div().flex_1().text_size(px(14.)).text_color(pal.text).child(name))
            .children(shortcut.map(shortcut_chips))
    })
}

/// Wrap a toolbar element so it shows an Onshape tooltip.
pub fn with_tooltip(id: impl Into<ElementId>, text: impl Into<SharedString>, el: impl IntoElement) -> AnyElement {
    div().id(id).flex_none().tooltip(tooltip(text)).child(el).into_any_element()
}

/// A tool tooltip title as Onshape writes it (live dom/toolbar-sketch.txt,
/// dropdowns-sketch.json): Onshape's tool names and lowercase shortcuts,
/// "Line (l)", "Sketch fillet (shift+f)", "Use (Project/Convert) (u)".
pub fn onshape_title(title: &str) -> String {
    let (name, shortcut) = match title.rfind(" (") {
        Some(i) if title.ends_with(')') => (&title[..i], Some(title[i + 2..title.len() - 1].to_lowercase())),
        _ => (title, None),
    };
    let name = match name {
        "Center rectangle" => "Center point rectangle",
        "Use / project edges" => "Use (Project/Convert)",
        n => n,
    };
    match shortcut {
        Some(k) => format!("{name} ({k})"),
        None => name.to_string(),
    }
}

/// A sketch toolbar tool's tooltip: the title, expanding with the tool's
/// description when there is one (see `tool_text`).
pub fn with_tool_tooltip(id: impl Into<ElementId>, title: impl Into<SharedString>, el: impl IntoElement) -> AnyElement {
    let title: SharedString = onshape_title(&title.into()).into();
    let name = title.split(" (").next().unwrap_or_default().to_string();
    match crate::tool_text::sketch_description(&name) {
        Some(d) => with_rich_tooltip(id, title, d, el),
        None => with_tooltip(id, title, el),
    }
}

/// Wrap a toolbar tool so it shows the title now and the description after 2 s.
pub fn with_rich_tooltip(id: impl Into<ElementId>, title: impl Into<SharedString>, description: impl Into<SharedString>, el: impl IntoElement) -> AnyElement {
    let anchor: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
    let a2 = anchor.clone();
    div()
        .id(id)
        .flex_none()
        .relative()
        .tooltip(rich_tooltip(title, description, anchor))
        .child(el)
        .child(canvas(move |b, _, _| a2.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
        .into_any_element()
}

/// The toolbar row: white, 36 px, no bottom border (§3.2).
pub fn toolbar() -> Div {
    h_flex().h(px(TOOLBAR_H)).flex_none().px(px(2.)).gap(px(2.)).bg(p().panel).overflow_hidden()
}

/// Toolbar group separator: 2 px wide, 26 px tall, `#d7d7d7`, 2 px from
/// the tools on each side (§3.2, measured live).
pub fn tool_separator() -> Div {
    div().flex_none().w(px(2.)).h(px(26.)).bg(p().border)
}

/// The "Search tools…" box at the right end of a toolbar (§3.2); while the
/// search popover is open it reads "Close search [escape]" (Onshape).
pub fn search_tools_toggle(id: impl Into<ElementId>, open: bool) -> Stateful<Div> {
    let pal = p();
    let chip = |t: &'static str| div().px(px(3.)).h(px(16.)).flex().items_center().rounded(px(2.)).border_1().border_color(pal.field_outline).bg(pal.panel).text_size(px(11.)).text_color(pal.text).child(t);
    h_flex()
        .id(id)
        .flex_none()
        .h(px(32.))
        .px(px(10.))
        .gap(px(6.))
        .rounded(px(3.))
        .bg(pal.search_fill)
        .hover(|d| d.bg(pal.border))
        .cursor_pointer()
        .when(open, |d| d.bg(pal.border))
        .child(div().text_size(px(14.)).text_color(pal.text).child(if open { "Close search" } else { "Search tools…" }))
        .when(open, |d| d.child(chip("escape")))
        .when(!open, |d| d.child(h_flex().gap(px(2.)).child(chip("alt/⌥")).child(chip("c"))))
}

/// Toolbar undo / redo pair (Onshape puts them first in every toolbar).
pub fn undo_redo(doc: &Entity<crate::doc_model::DocModel>, cx: &App) -> AnyElement {
    let d = doc.read(cx);
    let (can_undo, can_redo) = (d.session.can_undo(), d.session.can_redo());
    // Onshape's titles (live dom/toolbar-partstudio.txt).
    let undo_tip: SharedString = "Undo (cmd+z/ctrl+z)".into();
    let redo_tip: SharedString = "Redo (ctrl+y/shift+cmd+z)".into();
    let (d1, d2) = (doc.clone(), doc.clone());
    // Live: Undo @42, Redo @76 (dom/toolbar-partstudio.txt), 2 px apart like every tool.
    h_flex()
        .flex_none()
        .gap(px(2.))
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

/// Onshape's dialog checkbox (live DOM): a 27 px row, a 12 px box 4 px
/// from the left (1 px `#888`, radius 3; checked `#1651b0` with a white
/// check), then the 13.33 px `#666` label 4 px after it. Add `.on_click`.
pub fn os_checkbox(id: impl Into<ElementId>, label: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    let pal = p();
    h_flex()
        .id(id)
        .h(px(27.))
        .pl(px(4.))
        .pr(px(4.))
        .gap(px(4.))
        .flex_none()
        .cursor_pointer()
        .text_size(px(13.33))
        .text_color(pal.text_muted)
        .child(
            div()
                .size(px(12.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(if on { pal.accent } else { gpui_kit::rgb(0x888888) })
                .bg(if on { pal.accent } else { gpui_kit::rgb(0xffffff) })
                .when(on, |d| d.child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).with_size(px(10.)).text_color(gpui_kit::rgb(0xffffff)))),
        )
        .child(label.into())
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
    // Live (Parts / Default geometry headers): a thin 12 x 6 `#666` chevron.
    div()
        .w(px(16.))
        .ml(px(-3.))
        .mr(px(3.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(glyph(if open { "expander-down" } else { "expander-right" }, 14., gpui_kit::rgb(0x666666)))
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
