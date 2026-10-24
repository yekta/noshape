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

thread_local! {
    static LEFT_W: std::cell::Cell<f32> = std::cell::Cell::new(
        std::fs::read_to_string(crate::files::config_dir().join("left_panel_width.txt")).ok().and_then(|t| t.trim().parse::<f32>().ok()).filter(|w| w.is_finite()).unwrap_or(LEFT_PANEL_W).max(LEFT_PANEL_W),
    );
}

/// The Features / instance list's width. Onshape's splitter between it and
/// the graphics (`div.os-splitter`, live dom/cursor-rows.json) widens it
/// from the default; it never gets narrower, and the width persists.
pub fn left_panel_w() -> f32 {
    LEFT_W.with(|w| w.get())
}

/// Dragging the left panel's splitter.
#[derive(Clone)]
pub struct LeftSplitDrag;

impl Render for LeftSplitDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// The splitter's hit zone: 9 px into the graphics from the panel's edge
/// (live x 241-249 for a 241 px panel), `col-resize`. Place it at the left of
/// the graphics area.
pub fn left_splitter() -> Stateful<Div> {
    div()
        .id("left-splitter")
        .absolute()
        .top_0()
        .bottom_0()
        .left_0()
        .w(px(9.))
        .occlude()
        .cursor(CursorStyle::ResizeColumn)
        .on_drag(LeftSplitDrag, |_, _, _, cx| cx.new(|_| LeftSplitDrag))
}

/// Follows a splitter drag over `e.bounds` (the panel and the graphics,
/// starting at the panel's left edge): at least the default width, at most
/// all but 160 px of the graphics (live max x 1760 of 1920). True when the
/// width changed.
pub fn drag_left_panel(e: &DragMoveEvent<LeftSplitDrag>) -> bool {
    let max = (f32::from(e.bounds.size.width) - 160.).max(LEFT_PANEL_W);
    let w = f32::from(e.event.position.x - e.bounds.left()).round().clamp(LEFT_PANEL_W, max);
    if w == left_panel_w() {
        return false;
    }
    LEFT_W.with(|c| c.set(w));
    let p = crate::files::config_dir().join("left_panel_width.txt");
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, w.to_string());
    true
}

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
    move |window, cx| styled_tooltip(Tooltip::new(text.clone()), window, cx)
}

/// [`tooltip`] for user-authored text (a part's or a tab's name): drawn
/// as is in every UI language (see [`user_text`]).
pub fn user_tooltip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text: SharedString = text.into();
    move |window, cx| {
        let t = text.clone();
        styled_tooltip(Tooltip::element(move |_, _| user_text(t.clone())), window, cx)
    }
}

fn styled_tooltip(tip: Tooltip, window: &mut Window, cx: &mut App) -> AnyView {
    {
        let pal = p();
        let tip = tip
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
            .build(window, cx);
        cx.new(|_| MenuAwareTip(tip)).into()
    }
}

/// A tooltip hidden while a context menu is open: a row's tooltip (shown
/// once the pointer rests on the right-clicked row) was drawn over the
/// menu's first items, e.g. "Open version Start" over "Open".
struct MenuAwareTip(AnyView);

impl Render for MenuAwareTip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().when(!crate::os_menu::is_open(cx), |d| d.child(self.0.clone()))
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
                .child(div().p(px(4.)).line_height(px(18.)).child(tool_description(&self.description)))
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

/// User-authored text (names of documents, tabs, parts, features, sketches,
/// variables, configurations, mates, instances, folders, views, sheets,
/// layers; notes, properties, comments): drawn as is in every UI language
/// (the string-table lookup only applies to plain UI text, see `i18n`).
pub fn user_text(text: impl Into<SharedString>) -> StyledText {
    StyledText::new(text)
}

/// A tool description ([`crate::tool_text::blocks`]): its paragraphs, and
/// list items numbered at the right of the browser's 40 px list indent.
pub fn tool_description(text: &str) -> Div {
    v_flex().children(crate::tool_text::blocks(text).into_iter().map(|(n, t)| match n {
        None => div().child(t),
        Some(n) => h_flex().items_start().child(div().w(px(40.)).flex_none().pr(px(4.)).flex().justify_end().child(format!("{n}."))).child(div().flex_1().min_w_0().child(t)),
    }))
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

/// A measured number (Measure panel, the bottom-right readout, mass
/// properties): Onshape's `.measurement-value` is Inconsolata 14 px / 600
/// (the Bold face; Onshape serves only 400 and 700), on a 1 px `#d7d7d7`
/// underline (`--os-outline-tertiary`), 3 px from the label.
pub fn measure_value(num: impl Into<SharedString>) -> Div {
    div()
        .ml(px(3.))
        .mb(px(2.))
        .font_family("Inconsolata")
        .text_size(px(14.))
        .font_weight(FontWeight::BOLD)
        .border_b_1()
        .border_color(gpui_kit::rgb(0xd7d7d7))
        .child(num.into())
}

/// Where an Onshape tooltip sits relative to its element (live
/// part-studio/16-22, 95): beside list rows and left-rail buttons (Right),
/// left of the right strip and the camera menu button (Left), over the
/// viewport's bottom-right tools (Above), under the rollback bar (Below).
#[derive(Clone, Copy, PartialEq)]
pub enum TipSide {
    Right,
    Left,
    Above,
    Below,
}

/// An Onshape tooltip placed against its element: a 26 px `#d0dbe5` box with
/// 12 px / 500 text and 4 px padding, 2-3 px from the element (centered on
/// it across that side), kept inside the window.
pub struct PlacedTooltip {
    text: SharedString,
    side: TipSide,
    /// The element's bounds.
    anchor: Bounds<Pixels>,
    /// The box's width, measured on the first frame (drawn hidden): a
    /// centered or left-side box can only be placed once it is known.
    width: std::rc::Rc<std::cell::Cell<Option<Pixels>>>,
    /// Hidden after a press on the element, until the pointer leaves it.
    pressed: std::rc::Rc<std::cell::Cell<bool>>,
    /// Onshape's `data-bs-expanded-content`: a second, italic block shown
    /// under the title after the hover delay (and whether it shows yet).
    expanded: Option<(SharedString, bool)>,
    /// The text is user-authored (a list row's name): no translation.
    user: bool,
}

impl Render for PlacedTooltip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let shadow = vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(2.)), blur_radius: px(3.), spread_radius: px(0.), inset: false }];
        let a = self.anchor;
        let h = px(26.);
        let measured = self.width.clone();
        let title = div()
            .font_weight(FontWeight::MEDIUM)
            .h(h)
            .px(px(4.))
            .flex()
            .flex_none()
            .items_center()
            .bg(pal.tooltip)
            .whitespace_nowrap()
            .map(|d| if self.user { d.child(user_text(self.text.clone())) } else { d.child(self.text.clone()) });
        // Live `.tooltip-expand`: the expanded fill, 4 px padding, at most
        // 400 px wide, under the title.
        let extra = self.expanded.as_ref().filter(|(_, shown)| *shown).map(|(text, _)| div().max_w(px(RICH_TOOLTIP_W)).p(px(4.)).line_height(px(18.)).bg(gpui_kit::rgb(0xffffff)).italic().child(text.clone()));
        let content = div()
            .font_family(cx.theme().font_family.clone())
            .text_size(px(12.))
            .text_color(pal.text)
            .flex()
            .flex_col()
            .flex_none()
            .shadow(shadow)
            .child(title)
            .children(extra)
            .child(canvas(move |b, window, _| {
                if measured.get() != Some(b.size.width) {
                    measured.set(Some(b.size.width));
                    // (`refresh` is ignored while drawing.)
                    window.request_animation_frame();
                }
            }, |_, _, _, _| {}).absolute().top_0().left_0().size_full());
        let mid_y = a.center().y - h * 0.5;
        // Placed from the measured width (a fixed-width centering strip got
        // flipped to the far side by `anchored` near the window's edge: the
        // bottom-right tools' tooltips showed 600 px to their left).
        let win = window.viewport_size().width;
        let w = self.width.get();
        let clamp_x = |x: Pixels, w: Pixels| x.min(win - w - px(2.)).max(px(2.));
        let at = match (self.side, w) {
            (TipSide::Right, _) => point(a.right() + px(3.), mid_y),
            (TipSide::Left, Some(w)) => point(clamp_x(a.left() - px(2.) - w, w), mid_y),
            (TipSide::Above, Some(w)) => point(clamp_x(a.center().x - w * 0.5, w), a.top() - px(4.) - h),
            (TipSide::Below, Some(w)) => point(clamp_x(a.center().x - w * 0.5, w), a.bottom() + px(8.)),
            (_, None) => point(px(0.), mid_y),
        };
        let hidden = self.side != TipSide::Right && w.is_none();
        div().when(!crate::os_menu::is_open(cx) && !self.pressed.get(), |d| d.child(anchored().position(at).snap_to_window().child(content.when(hidden, |c| c.opacity(0.)))))
    }
}

/// Gives `el` an Onshape tooltip showing `text` on `side` (see [`PlacedTooltip`]).
/// Like a browser's, the tooltip hides when the element is pressed (a strip
/// button opening its panel) and shows again only after the pointer has left
/// it (qa4 F22).
pub fn with_tip(el: Stateful<Div>, text: impl Into<SharedString>, side: TipSide) -> Stateful<Div> {
    tip_with(el, text.into(), side, false)
}

/// A list row's tooltip, beside the row (live part-studio/16, 95), showing
/// its stored name (user text, never translated).
pub fn with_user_side_tooltip(row: Stateful<Div>, text: impl Into<SharedString>) -> Stateful<Div> {
    tip_with(row, text.into(), TipSide::Right, true)
}

fn tip_with(el: Stateful<Div>, text: SharedString, side: TipSide, user: bool) -> Stateful<Div> {
    let anchor: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
    let pressed: std::rc::Rc<std::cell::Cell<bool>> = Default::default();
    let (a2, p2, p3) = (anchor.clone(), pressed.clone(), pressed.clone());
    el.relative()
        .child(
            canvas(
                move |b, window, _| {
                    a2.set(b);
                    if p2.get() && !b.contains(&window.mouse_position()) {
                        p2.set(false);
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .on_any_mouse_down(move |_, _, _| p3.set(true))
        .tooltip(move |_, cx| {
            let (text, anchor, pressed) = (text.clone(), anchor.get(), pressed.clone());
            cx.new(|_| PlacedTooltip { text, side, anchor, width: Default::default(), pressed, expanded: None, user }).into()
        })
}

/// `.tip(text, side)`: [`with_tip`] in builder chains.
pub trait TipExt {
    fn tip(self, text: impl Into<SharedString>, side: TipSide) -> Self;
}

impl TipExt for Stateful<Div> {
    fn tip(self, text: impl Into<SharedString>, side: TipSide) -> Self {
        with_tip(self, text, side)
    }
}

/// A list row's side tooltip with Onshape's expanded content: `extra` (italic)
/// joins the title after the same delay as the toolbar's [`RichTooltip`]
/// (a custom feature's "Feature from current workspace", live FS4).
pub fn with_side_tooltip_expanded(row: Stateful<Div>, text: impl Into<SharedString>, extra: impl Into<SharedString>) -> Stateful<Div> {
    let (text, extra): (SharedString, SharedString) = (text.into(), extra.into());
    let anchor: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
    let a2 = anchor.clone();
    row.relative()
        .child(canvas(move |b, _, _| a2.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
        .tooltip(move |_, cx| {
            let (text, extra, anchor) = (text.clone(), extra.clone(), anchor.get());
            cx.new(|cx| {
                cx.spawn(async move |this: WeakEntity<PlacedTooltip>, cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
                    let _ = this.update(cx, |t, cx| {
                        if let Some((_, shown)) = &mut t.expanded {
                            *shown = true;
                        }
                        cx.notify();
                    });
                })
                .detach();
                PlacedTooltip { text, side: TipSide::Right, anchor, width: Default::default(), pressed: Default::default(), expanded: Some((extra, false)), user: false }
            })
            .into()
        })
}

/// [`with_user_side_tooltip`] whose text and place are worked out when the
/// tooltip opens, not each time the row renders (a feature row's "Extrude 1
/// <- Sketch 1"): `anchor` gives the row's bounds then. Its text is the row's
/// name and references: user-authored, never translated.
pub fn with_lazy_side_tooltip(row: Stateful<Div>, anchor: impl Fn() -> Bounds<Pixels> + 'static, text: impl Fn(&App) -> SharedString + 'static) -> Stateful<Div> {
    row.tooltip(move |_, cx| {
        let (text, anchor) = (text(cx), anchor());
        cx.new(|_| PlacedTooltip { text, side: TipSide::Right, anchor, width: Default::default(), pressed: Default::default(), expanded: None, user: true }).into()
    })
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

/// A toolbar dropdown kept inside the window: it scrolls when its tools don't
/// fit under the toolbar (the collapsed Plane group has 26 tools; on a
/// 1280x800 screen the last ones were off the window, unreachable).
/// Live (ui/tb-912, dom/ui-toolbar-1440-groups.json): a toolbar dropdown is
/// capped at 75 % of the window height (`max-height: 675px` at 900) and
/// scrolls inside it with Chrome's native scrollbar; it never flips up.
pub fn fit_tool_menu(menu: gpui_kit::component::menu::PopupMenu, window: &Window) -> gpui_kit::component::menu::PopupMenu {
    menu.scrollable(true).native_scrollbar(true).tool_list(true).max_h(window.viewport_size().height * 0.75)
}

/// The drawing toolbar's caret cell: live drawing/08 shows a filled 8 x 4 px blue
/// triangle (`#2767ae`) there instead of the Part Studio's dark chevron.
pub fn tool_caret_blue(id: impl Into<ElementId>, cx: &App) -> Button {
    Button::new(id)
        .custom(tool_variant(cx, false, true))
        .child(glyph("drawing-tool-caret", 12., gpui_kit::rgb(0x2767ae)))
        .w(px(TOOL_CARET_W))
        .h(px(TOOL_CELL))
        .p_0()
        .rounded(px(0.))
        .rounded_r(px(3.))
}

/// The 18 px dropdown caret cell next to a split tool (§3.2).
pub fn tool_caret(id: impl Into<ElementId>, cx: &App) -> Button {
    let pal = p();
    Button::new(id)
        .custom(tool_variant(cx, false, true))
        // Live: the 10 x 6 `expanded` sprite at 1:1, about 2 px thick.
        .child(glyph("tool-caret", 10., pal.text))
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
    // Onshape's titles (live dom/toolbar-partstudio.txt).
    undo_redo_with(doc, TOOL_CELL, ("Undo (cmd+z/ctrl+z)", "Redo (ctrl+y/shift+cmd+z)"), cx)
}

/// [`undo_redo`] with `cell` px buttons and the (undo, redo) titles (the
/// drawing toolbar: 30 px, "Undo (ctrl+z)", live drawing-toolbar-widths).
pub fn undo_redo_with(doc: &Entity<crate::doc_model::DocModel>, cell: f32, tips: (&'static str, &'static str), cx: &App) -> AnyElement {
    let d = doc.read(cx);
    let (can_undo, can_redo) = (d.can_undo(), d.can_redo());
    let undo_tip: SharedString = tips.0.into();
    let redo_tip: SharedString = tips.1.into();
    let (d1, d2) = (doc.clone(), doc.clone());
    // Live: Undo @42, Redo @76 (dom/toolbar-partstudio.txt), 2 px apart like every tool.
    h_flex()
        .flex_none()
        .gap(px(2.))
        .child(with_tooltip(
            "tt-undo",
            undo_tip,
            tool_button("tb-undo", "undo", false, cx).w(px(cell)).h(px(cell)).opacity(if can_undo { 1.0 } else { 0.4 }).when(!can_undo, |b| b.cursor(CursorStyle::OperationNotAllowed)).on_click(move |_, _, cx| {
                if can_undo {
                    d1.update(cx, |d, cx| d.undo(cx));
                }
            }),
        ))
        .child(with_tooltip(
            "tt-redo",
            redo_tip,
            tool_button("tb-redo", "redo", false, cx).w(px(cell)).h(px(cell)).opacity(if can_redo { 1.0 } else { 0.4 }).when(!can_redo, |b| b.cursor(CursorStyle::OperationNotAllowed)).on_click(move |_, _, cx| {
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
    os_checkbox_focused(id, label, on, false)
}

/// [`os_checkbox`] with keyboard focus (feature dialogs, bundle CSS
/// `input.ns-parameter-checkbox:focus + label.os-checkbox`): the box gets a
/// `#1651b0` border and a 3 px `#769fcc` glow.
pub fn os_checkbox_focused(id: impl Into<ElementId>, label: impl Into<SharedString>, on: bool, focused: bool) -> Stateful<Div> {
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
                // Live (dom/dialog-fillet): the box is 8 px from the row's
                // top (centering alone puts it at 7.5, drawn at 7).
                .mt(px(1.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(if on || focused { pal.accent } else { gpui_kit::rgb(0x888888) })
                .when(focused, |d| d.shadow(vec![BoxShadow { color: gpui_kit::rgb(0x769fcc).into(), offset: point(px(0.), px(0.)), blur_radius: px(3.), spread_radius: px(0.), inset: false }]))
                .bg(if on { pal.accent } else { gpui_kit::rgb(0xffffff) })
                .when(on, |d| d.child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).with_size(px(10.)).text_color(gpui_kit::rgb_static(0xffffff)))),
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
    // Live (part-studio/495): dimmed (hidden) rows' icons at 0.5.
    div().w(px(20.)).flex_none().flex().items_center().justify_center().opacity(if dim { 0.5 } else { 0.8 }).child(assets::icon(name, LIST_ICON, cx))
}

/// Collapsible caret (▾/▸) in a 16 px column (§4.11).
/// Gives an input its placeholder (set once). The input draws it itself,
/// in gpui-base's app-wide `PlaceholderStyle` (Onshape's italic), so it
/// starts exactly where typed text and the caret do; never draw a
/// placeholder over an input yourself.
pub fn ensure_placeholder(st: &Entity<gpui_kit::component::input::InputState>, text: &str, cx: &mut App) {
    if st.read(cx).placeholder_text().as_ref() != text {
        // Onshape's search and filter boxes don't select their text on a
        // click and keep it on Esc (live behaviours.md "Input behaviour").
        let search = text.starts_with("Search") || text.starts_with("Filter");
        let text = SharedString::from(text.to_string());
        // Only switch search boxes over: the other inputs keep what they were
        // built with (KB2: "New view name" opts out of the revert, so Esc closes
        // Named views).
        st.update(cx, |s, cx| {
            s.put_placeholder(text, cx);
            if search {
                s.set_search_box(true);
            }
        });
    }
}

/// `Input::new(st)` with its placeholder set (see [`ensure_placeholder`]).
pub fn input_with_placeholder(st: &Entity<gpui_kit::component::input::InputState>, text: impl AsRef<str>, cx: &mut App) -> gpui_kit::component::input::Input {
    ensure_placeholder(st, text.as_ref(), cx);
    gpui_kit::component::input::Input::new(st)
}

pub fn caret(open: bool) -> Div {
    // Live (Parts / Default geometry headers): a thin 12 x 6 chevron,
    // `--os-icon-fill-tertiary` (#666, #999 in dark: live dark/dk-940).
    div()
        .w(px(16.))
        .ml(px(-3.))
        .mr(px(3.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        // Live: the expander (`.node-expander`) shows the pointer.
        .cursor_pointer()
        .child(glyph(if open { "expander-down" } else { "expander-right" }, 14., p().icon_tertiary))
}

/// A dialog's disabled green ✓ button (after `.bg(accept)`). Live: the button at
/// 20% opacity: #cce9cc with a white ✓ in light, (41,70,41) with a grey ✓ in dark
/// (dialogs/*-01-open, dark/dk-978). GPUI applies opacity per primitive, which
/// would tint the light ✓, so light uses the blended colour instead.
pub fn disabled_accept<E: Styled>(e: E) -> E {
    if gpui_kit::hex_theme_active() { e.opacity(0.2) } else { e.bg(gpui_kit::rgb(0xcce9cc)) }
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

/// A control revealed while its row (`group`) is hovered: Onshape's eye on
/// feature, part and instance rows. It stays painted at opacity 0 rather
/// than `invisible()`: gpui registers no mouse listeners for an invisible
/// element, so a quick click (or a trackpad tap, before a hover frame was
/// drawn) went to the row instead.
pub fn hover_reveal<E: InteractiveElement + Styled>(el: E, group: impl Into<SharedString>) -> E {
    el.opacity(0.).group_hover(group, |s| s.opacity(1.))
}

/// Onshape's panel dialogs (Mass properties, Isolate, Named views, Section
/// view, Appearance, Material) move by the drag area around their title
/// (`.ns-drag-area`: `grab`, also while moving; live dom/cursor-rows.json).
/// The drag carries the panel's key.
#[derive(Clone)]
pub struct PanelMove {
    key: &'static str,
}

/// A panel title press: where the drag starts from.
#[derive(Clone, Copy)]
struct PanelPress {
    pointer: Point<Pixels>,
    offset: Point<Pixels>,
    /// The panel's rect (see [`panel_probe`]).
    rect: Option<Bounds<Pixels>>,
    generation: u64,
}

thread_local! {
    /// The last title press. One press at a time; kept out of the drag
    /// value, which is built anew on every render (a render between the
    /// press and the drag start would lose it).
    static PANEL_PRESS: std::cell::Cell<Option<PanelPress>> = const { std::cell::Cell::new(None) };
    /// Each panel's rect when last drawn and the drag move it was drawn
    /// after (see [`PanelOffsets::drag`]).
    static PANEL_RECTS: std::cell::RefCell<std::collections::HashMap<&'static str, (Bounds<Pixels>, u64)>> = std::cell::RefCell::new(std::collections::HashMap::new());
    /// Counts panel drag moves.
    static PANEL_GEN: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Records a moving panel's rect for the drag's snapping: a child of the
/// panel's root, which is the panel's size (`key` as in [`panel_title`]).
pub fn panel_probe(key: &'static str) -> impl IntoElement {
    canvas(
        move |b, _, _| {
            PANEL_RECTS.with(|m| m.borrow_mut().insert(PanelOffsets::panel(key), (b, PANEL_GEN.with(|g| g.get()))));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// Live's `panelLayoutManager.snapIfNearBounds` (tolerance 10 px): an edge
/// within 10 px of the graphics area's edge sticks to it (left before
/// right, top before bottom); else an edge within 10 px of another open
/// panel's edge (side by side or aligned, the panels overlapping along it)
/// sticks to that; then the rect is kept inside the area.
fn snap_panel(r: Bounds<Pixels>, area: Bounds<Pixels>, others: &[Bounds<Pixels>]) -> Point<Pixels> {
    const TOL: f32 = 10.;
    let near = |a: Pixels, b: Pixels| f32::from(a - b).abs() <= TOL;
    let overlaps = |a0: Pixels, a1: Pixels, b0: Pixels, b1: Pixels| a0 < b1 + px(TOL) && b0 < a1 + px(TOL);
    // `gap`: live leaves 5 px under a panel snapped to the bottom
    // (`bounds.bottom - t.bottom - 5`), none at the other edges.
    let axis = |lo: Pixels, len: Pixels, area_lo: Pixels, area_hi: Pixels, gap: Pixels, others: &mut dyn Iterator<Item = (Pixels, Pixels)>| -> Pixels {
        let hi = lo + len;
        if lo < area_lo + px(TOL) {
            return area_lo;
        }
        if hi > area_hi - px(TOL) {
            return area_hi - gap - len;
        }
        for (o0, o1) in others {
            for (edge, at) in [(hi, o0), (lo, o1), (lo, o0), (hi, o1)] {
                if near(edge, at) {
                    return lo + (at - edge);
                }
            }
        }
        lo
    };
    let x = axis(r.left(), r.size.width, area.left(), area.right(), px(0.), &mut others.iter().filter(|o| overlaps(r.top(), r.bottom(), o.top(), o.bottom())).map(|o| (o.left(), o.right())));
    let y = axis(r.top(), r.size.height, area.top(), area.bottom(), px(5.), &mut others.iter().filter(|o| overlaps(r.left(), r.right(), o.left(), o.right())).map(|o| (o.top(), o.bottom())));
    // keepPanelWithinBounds
    point(x.min(area.right() - r.size.width).max(area.left()), y.min(area.bottom() - r.size.height).max(area.top()))
}

struct PanelMoveGhost;

impl Render for PanelMoveGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// A panel dialog's title as its drag area.
pub fn panel_title(key: &'static str, title: Div) -> Stateful<Div> {
    title
        .id(SharedString::from(format!("panel-title-{key}")))
        .cursor(CursorStyle::OpenHand)
        .on_mouse_down(MouseButton::Left, move |e, _, _| {
            let rect = PANEL_RECTS.with(|m| m.borrow().get(PanelOffsets::panel(key)).map(|r| r.0));
            let press = PanelPress { pointer: e.position, offset: PanelOffsets.get(key), rect, generation: PANEL_GEN.with(|g| g.get()) };
            PANEL_PRESS.with(|p| p.set(Some(press)));
        })
        .on_drag(PanelMove { key }, |_, _, _, cx| cx.new(|_| PanelMoveGhost))
}

thread_local! {
    /// Where each panel dialog was moved, by panel (shared by the Part
    /// Studio and the assembly), kept across sessions.
    static PANEL_OFFSETS: std::cell::RefCell<std::collections::HashMap<String, (f32, f32)>> = std::cell::RefCell::new(
        std::fs::read_to_string(crate::files::config_dir().join("panel_positions.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default(),
    );
}

/// How far each panel dialog was moved from its place. Live remembers a
/// panel's rect per panel id, for the user: the same in the Part Studio and
/// the assembly, after closing and in later sessions (behaviours.md,
/// Interference detection reopening at the remembered rect). Keys may carry
/// a `ps-` / `asm-` prefix; the panel is the rest.
#[derive(Default)]
pub struct PanelOffsets;

impl PanelOffsets {
    fn panel(key: &'static str) -> &'static str {
        key.strip_prefix("ps-").or_else(|| key.strip_prefix("asm-")).unwrap_or(key)
    }

    pub fn get(&self, key: &'static str) -> Point<Pixels> {
        PANEL_OFFSETS.with(|m| m.borrow().get(Self::panel(key)).map(|(x, y)| point(px(*x), px(*y))).unwrap_or_default())
    }

    /// Follows a panel drag over `e.bounds` (the graphics area): the
    /// panel's rect at the press moved by the pointer, snapped and kept
    /// inside the area ([`snap_panel`]). A panel without a probe keeps the
    /// pointer inside the area instead.
    pub fn drag(&mut self, e: &DragMoveEvent<PanelMove>, cx: &App) {
        let key = Self::panel(e.drag(cx).key);
        let Some(press) = PANEL_PRESS.with(|p| p.get()) else { return };
        let generation = PANEL_GEN.with(|g| {
            g.set(g.get() + 1);
            g.get()
        });
        // Live's panel container: right of the 40 px rail (over the feature
        // list too), its top 3 px under the toolbar where feature dialogs
        // open ("Panels vs feature dialogs": snapped top = y 79).
        let b = Bounds::from_corners(point(px(RAIL_W), e.bounds.top() + px(3.)), e.bounds.bottom_right());
        let offset = match press.rect {
            Some(r0) => {
                let r = Bounds { origin: r0.origin + (e.event.position - press.pointer), size: r0.size };
                // Open panels were drawn after the last move (none is known
                // yet at the first).
                let others: Vec<Bounds<Pixels>> = if generation > press.generation + 1 {
                    PANEL_RECTS.with(|m| m.borrow().iter().filter(|(k, (_, g))| **k != key && *g + 1 >= generation).map(|(_, (r, _))| *r).collect())
                } else {
                    Vec::new()
                };
                press.offset + (snap_panel(r, b, &others) - r0.origin)
            }
            None => {
                let at = point(e.event.position.x.clamp(b.left(), b.right()), e.event.position.y.clamp(b.top(), b.bottom()));
                press.offset + (at - press.pointer)
            }
        };
        let offset = (f32::from(offset.x).round(), f32::from(offset.y).round());
        PANEL_OFFSETS.with(|m| {
            let mut m = m.borrow_mut();
            if m.get(key) == Some(&offset) {
                return;
            }
            m.insert(key.to_string(), offset);
            let p = crate::files::config_dir().join("panel_positions.json");
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(t) = serde_json::to_string(&*m) {
                let _ = std::fs::write(p, t);
            }
        });
    }
}

/// `panel` moved by `offset` (its own placement stays as is).
pub fn moved_panel(panel: impl IntoElement, offset: Point<Pixels>) -> AnyElement {
    if offset == Point::default() {
        return panel.into_any_element();
    }
    div().absolute().left(offset.x).top(offset.y).size_full().child(panel).into_any_element()
}

/// [`moved_panel`] for a panel laid out in a column (offset relatively,
/// leaving its place in the column).
pub fn nudged_panel(panel: impl IntoElement, offset: Point<Pixels>) -> AnyElement {
    if offset == Point::default() {
        return panel.into_any_element();
    }
    div().relative().left(offset.x).top(offset.y).child(panel).into_any_element()
}

/// The right panels' width (Configurations, BOM, Variables, ...: live 712 px).
/// The splitter at their left edge (`div.os-splitter`, `col-resize`, a 4 px
/// hit zone) narrows them to 235 px or widens them; the width persists.
pub const RIGHT_PANEL_W: f32 = 712.;
const RIGHT_PANEL_MIN: f32 = 235.;

thread_local! {
    static RIGHT_W: std::cell::Cell<f32> = std::cell::Cell::new(
        std::fs::read_to_string(crate::files::config_dir().join("right_panel_width.txt")).ok().and_then(|t| t.trim().parse::<f32>().ok()).filter(|w| w.is_finite()).unwrap_or(RIGHT_PANEL_W).max(RIGHT_PANEL_MIN),
    );
}

pub fn right_panel_w() -> f32 {
    RIGHT_W.with(|w| w.get())
}

/// Dragging a right panel's splitter.
#[derive(Clone)]
pub struct RightSplitDrag;

impl Render for RightSplitDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// `panel` with its splitter: a 4 px hit zone at its left edge (`panel`
/// must be positioned). `inset` is what the panel draws left of its content
/// width (a gutter, a border).
pub fn right_resizable<E: InteractiveElement + ParentElement>(panel: E, inset: f32) -> E {
    panel
        .child(
            div()
                .id("right-splitter")
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .w(px(4.))
                .occlude()
                .cursor(CursorStyle::ResizeColumn)
                .on_drag(RightSplitDrag, |_, _, _, cx| cx.new(|_| RightSplitDrag)),
        )
        .on_drag_move(move |e: &DragMoveEvent<RightSplitDrag>, window, _| {
            // The panel keeps its right edge; its left edge follows the pointer.
            let max = (f32::from(window.viewport_size().width) - 400.).max(RIGHT_PANEL_MIN);
            let w = (f32::from(e.bounds.right() - e.event.position.x) - inset).round().clamp(RIGHT_PANEL_MIN, max);
            if w != right_panel_w() {
                RIGHT_W.with(|c| c.set(w));
                let p = crate::files::config_dir().join("right_panel_width.txt");
                if let Some(dir) = p.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(p, w.to_string());
                window.refresh();
            }
        })
}

thread_local! {
    /// Table column widths the user dragged, by table and column; kept
    /// across sessions as live keeps them (reopen and reload).
    static COL_WIDTHS: std::cell::RefCell<std::collections::HashMap<String, f32>> = std::cell::RefCell::new(
        std::fs::read_to_string(crate::files::config_dir().join("table_columns.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default(),
    );
    /// The column width and the pointer x when a grip was pressed.
    static COL_PRESS: std::cell::Cell<(f32, Pixels)> = const { std::cell::Cell::new((0., Pixels::ZERO)) };
}

/// The width of column `col` of table `table` (`default` until dragged).
pub fn col_w(table: &str, col: &str, default: f32) -> f32 {
    COL_WIDTHS.with(|m| m.borrow().get(&format!("{table}/{col}")).copied().unwrap_or(default))
}

/// A column grip being dragged.
#[derive(Clone)]
pub struct ColResize {
    key: SharedString,
}

impl Render for ColResize {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Live's table column grip (`.table-cell--header .drag-handle`, behaviours.md
/// "Table column resize"): 5 px inside the header cell's right edge,
/// `col-resize`. Dragging changes only that column (the ones to its right
/// shift), at least 40 px, no maximum; double-click does nothing. The header
/// cell must be positioned and sized with [`col_w`].
pub fn col_grip(table: &str, col: &str, default: f32) -> Stateful<Div> {
    let key = SharedString::from(format!("{table}/{col}"));
    let w0 = col_w(table, col, default);
    div()
        .id(SharedString::from(format!("col-grip-{key}")))
        .absolute()
        .top_0()
        .bottom_0()
        .right_0()
        .w(px(5.))
        .cursor(CursorStyle::ResizeColumn)
        .on_mouse_down(MouseButton::Left, move |e, _, _| COL_PRESS.with(|p| p.set((w0, e.position.x))))
        .on_drag(ColResize { key }, |d, _, _, cx| cx.new(|_| d.clone()))
        .on_drag_move(|e: &DragMoveEvent<ColResize>, window, cx| {
            let key = e.drag(cx).key.to_string();
            let (w0, x0) = COL_PRESS.with(|p| p.get());
            let w = (w0 + f32::from(e.event.position.x - x0)).round().max(40.);
            let changed = COL_WIDTHS.with(|m| {
                let mut m = m.borrow_mut();
                if m.get(&key) == Some(&w) {
                    return false;
                }
                m.insert(key, w);
                let p = crate::files::config_dir().join("table_columns.json");
                if let Some(dir) = p.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Ok(t) = serde_json::to_string(&*m) {
                    let _ = std::fs::write(p, t);
                }
                true
            });
            if changed {
                window.refresh();
            }
        })
}

/// Live's drag image for reordering (Configurations headers, selection lists,
/// cursors/670, ui/966): a `#1651b0` pill with the name in white bold at the
/// pointer; the pointer shows `grabbing` meanwhile. `offset` is where the
/// dragged element was grabbed (GPUI draws the drag view at the pointer less
/// it, at its least size: pad it back).
pub fn name_pill(name: SharedString, offset: Point<Pixels>) -> Div {
    div()
        .pl(offset.x)
        .pt(offset.y)
        .child(
            div()
                .mt(px(-15.))
                .h(px(31.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(15.5))
                .bg(gpui_kit::rgb(0x1651b0))
                .text_size(px(14.))
                .font_weight(FontWeight::BOLD)
                .text_color(gpui_kit::rgb_static(0xffffff))
                .child(name),
        )
        .child(canvas(|_, _, _| {}, |_, _, window, _| window.set_window_cursor_style(CursorStyle::ClosedHand)).absolute().size_0())
}

/// Live's reorder drop line (cursors/670, ui/966): 2 px `#1651b0` from a 6 px
/// open circle, across the list; 6 px tall, place it centred on the gap.
pub fn drop_line() -> Div {
    let accent = gpui_kit::rgb(0x1651b0);
    div()
        .absolute()
        .left_0()
        .right_0()
        .h(px(6.))
        .child(div().absolute().left(px(5.)).right_0().top(px(2.)).h(px(2.)).bg(accent))
        .child(div().absolute().left_0().top_0().size(px(6.)).rounded_full().border_2().border_color(accent).bg(gpui_kit::rgb(0xcad8ed)))
}

/// The two letters of a custom feature's initials icon: the first letter
/// of its first two words ("QA cube" → "Qc", "FS pack probe" → "Fp"), or
/// the first two letters of a single word.
pub fn feature_initials(name: &str) -> String {
    let mut words = name.split_whitespace();
    let first = words.next().unwrap_or_default();
    let mut out: String = first.chars().take(1).flat_map(char::to_uppercase).collect();
    match words.next().and_then(|w| w.chars().next()) {
        Some(c) => out.extend(c.to_lowercase()),
        None => out.extend(first.chars().skip(1).take(1).flat_map(char::to_lowercase)),
    }
    out
}

/// Onshape's custom-feature icon (`.tool-initials-icon`, live
/// part-studio/1252): the initials in a 20 px box with a 1 px `#999`
/// border, in the custom features dropdown and the feature list.
pub fn initials_icon(name: &str) -> Div {
    initials_icon_status(name, false)
}

/// The initials box's border: `#999`, or the error red while the feature
/// fails to regenerate (live qa5 featurescript (7): "Qc" outlined red).
pub fn initials_border(failing: bool) -> u32 {
    if failing { 0xbd3039 } else { 0x999999 }
}

/// [`initials_icon`] for a feature-list row, red-outlined when it fails.
pub fn initials_icon_status(name: &str, failing: bool) -> Div {
    div()
        .flex_none()
        .size(px(20.))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(gpui_kit::rgb(initials_border(failing)))
        .rounded(px(2.))
        .bg(gpui_kit::rgb(0xffffff))
        .text_size(px(12.))
        .text_color(p().text)
        .child(feature_initials(name))
}

/// A regeneration time as Onshape shows it: "617ms", "1.02s" from 1 s on
/// (live behaviours "qa5 big plate").
pub fn regen_time_text(ms: f32) -> String {
    if ms >= 1000.0 { format!("{}s", noshape_units::to_fixed(f64::from(ms) / 1000.0, 2)) } else { format!("{}ms", noshape_units::to_fixed(f64::from(ms), 0)) }
}

#[cfg(test)]
mod tests {
    // `super::*` brings gpui's `test` attribute (via gpui_kit), which would
    // shadow the built-in one and expand forever.
    use super::*;

    #[::core::prelude::v1::test]
    fn failing_custom_feature_icon_is_outlined_red() {
        // Live (qa5 featurescript (7)): a failing custom feature's "Qc" box
        // is outlined red; otherwise #999.
        assert_eq!(initials_border(true), 0xbd3039);
        assert_eq!(initials_border(false), 0x999999);
        assert_eq!(feature_initials("QA cube"), "Qc");
    }

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds { origin: point(px(x), px(y)), size: size(px(w), px(h)) }
    }

    /// CRASH1: `tree_row` sets the hover fill of unselected rows, and GPUI
    /// panics ("hover style already set") on a second `.hover`. No row chain
    /// may add its own before its first child.
    #[::core::prelude::v1::test]
    fn tree_rows_never_set_a_second_hover() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let mut files = Vec::new();
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut bad = Vec::new();
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            let lines: Vec<&str> = text.lines().collect();
            for (i, l) in lines.iter().enumerate() {
                if !l.contains("tree_row(") || l.contains("fn tree_row") || l.trim_start().starts_with("//") {
                    continue;
                }
                for (j, m) in lines.iter().enumerate().skip(i + 1).take(14) {
                    let m = m.trim_start();
                    if m.starts_with(".child(") || m.starts_with(".children(") {
                        break;
                    }
                    if m.contains(".hover(") {
                        bad.push(format!("{}:{}", f.display(), j + 1));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "a second hover on a tree_row: {bad:?}");
    }

    #[::core::prelude::v1::test]
    fn custom_feature_initials_and_regen_times_read_as_onshape() {
        assert_eq!(feature_initials("QA cube"), "Qc");
        assert_eq!(feature_initials("FS pack probe"), "Fp");
        assert_eq!(feature_initials("Cube"), "Cu");
        assert_eq!(regen_time_text(617.4), "617ms");
        assert_eq!(regen_time_text(1024.0), "1.02s");
    }

    #[::core::prelude::v1::test]
    fn panels_snap_like_onshape() {
        let area = rect(0., 0., 1000., 600.);
        // Within 10 px of the area's edges: stuck to them.
        assert_eq!(snap_panel(rect(8., 9., 200., 100.), area, &[]), point(px(0.), px(0.)));
        assert_eq!(snap_panel(rect(791., 495., 200., 100.), area, &[]), point(px(800.), px(495.)));
        // Farther: left as is.
        assert_eq!(snap_panel(rect(11., 50., 200., 100.), area, &[]), point(px(11.), px(50.)));
        // Beside another panel, overlapping it vertically; the tops, 10 px
        // apart, align too.
        assert_eq!(snap_panel(rect(306., 50., 200., 100.), area, &[rect(100., 40., 200., 300.)]), point(px(300.), px(40.)));
        assert_eq!(snap_panel(rect(306., 70., 200., 100.), area, &[rect(100., 40., 200., 300.)]), point(px(300.), px(70.)));
        // Not beside it (no vertical overlap): no snap.
        assert_eq!(snap_panel(rect(306., 400., 200., 100.), area, &[rect(100., 40., 200., 100.)]), point(px(306.), px(400.)));
        // Dragged out: kept inside.
        assert_eq!(snap_panel(rect(900., -50., 200., 100.), area, &[]), point(px(800.), px(0.)));
    }
}
