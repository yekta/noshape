//! Onshape-styled building blocks shared by every tab: toolbars, tool
//! buttons, the tool search box, panel rails, tree rows, alerts and
//! tooltips. Measurements and colors follow docs/ONSHAPE_UI_SPEC.md.

use crate::assets;
use crate::theme::p;
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
    /// Each tab's list width (live: `left-content-pane<elementId>PanelLength` per user and per tab, feel/chrome.json
    /// "panels"); a tab not resized has the default.
    static LEFT_W: std::cell::RefCell<std::collections::HashMap<u64, f32>> = std::cell::RefCell::new(
        std::fs::read_to_string(left_widths_path())
            .ok()
            .and_then(|t| serde_json::from_str::<std::collections::HashMap<String, f32>>(&t).ok())
            .map(|m| m.into_iter().filter_map(|(k, w)| Some((k.parse().ok()?, w))).filter(|(_, w): &(u64, f32)| w.is_finite()).collect())
            .unwrap_or_default(),
    );
}

fn left_widths_path() -> std::path::PathBuf {
    crate::paths::config_dir().join("left_panel_widths.json")
}

/// Tab `tab`'s Features / instance list width. Onshape's splitter between it and the graphics (`div.os-splitter`, live
/// dom/cursor-rows.json) widens it from the default; it never gets narrower, and each tab keeps its own.
pub fn left_panel_w(tab: noshape_model::document::TabId) -> f32 {
    LEFT_W.with(|m| m.borrow().get(&tab.0).copied()).unwrap_or(LEFT_PANEL_W).max(LEFT_PANEL_W)
}

/// Tab `tab`'s list is `w` px wide from now on (not saved: [`drag_left_panel`] saves).
pub fn remember_left_panel_w(tab: noshape_model::document::TabId, w: f32) {
    LEFT_W.with(|m| m.borrow_mut().insert(tab.0, w));
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
/// starting at the panel's left edge) for tab `tab`: at least the default width, at most
/// all but 160 px of the graphics (live max x 1760 of 1920). True when the
/// width changed.
pub fn drag_left_panel(e: &DragMoveEvent<LeftSplitDrag>, tab: noshape_model::document::TabId) -> bool {
    let max = (f32::from(e.bounds.size.width) - 160.).max(LEFT_PANEL_W);
    let w = f32::from(e.event.position.x - e.bounds.left()).round().clamp(LEFT_PANEL_W, max);
    if w == left_panel_w(tab) {
        return false;
    }
    remember_left_panel_w(tab, w);
    let all: std::collections::HashMap<String, f32> = LEFT_W.with(|m| m.borrow().iter().map(|(k, w)| (k.to_string(), *w)).collect());
    let p = left_widths_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, serde_json::to_string(&all).unwrap_or_default());
    true
}

/// A variable table's columns (Name, Variable type, Value, Description): 140 px each by default; dragging a
/// header's right edge resizes one, at least 40 px (live 11-02: "Na…" at 40). One set for every table in the
/// panel (live: the inserted Variable Studio's table follows the Part Studio's), kept while the app runs (live:
/// across tabs, back to 140 px on a reload).
pub const VAR_COL_W: f32 = 140.;
pub const VAR_COL_MIN: f32 = 40.;

thread_local! {
    static VAR_COLS: std::cell::Cell<[f32; 4]> = const { std::cell::Cell::new([VAR_COL_W; 4]) };
}

pub fn var_col_widths() -> [f32; 4] {
    VAR_COLS.with(|c| c.get())
}

/// Column `col` at width `w` (at least [`VAR_COL_MIN`]).
pub fn set_var_col_width(col: usize, w: f32) {
    let mut ws = var_col_widths();
    let Some(x) = ws.get_mut(col) else { return };
    *x = w.round().max(VAR_COL_MIN);
    VAR_COLS.with(|c| c.set(ws));
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
    move |window, cx| {
        let t = html_spaces(&text);
        styled_tooltip(Tooltip::element(move |w, _| tip_text(w, &t, t.clone())), text.clone(), window, cx)
    }
}

/// A plain tooltip's text: a text longer than live's 400 px `.tooltip-inner` less its 7 px sides gets that width and wraps
/// (the box's `max_w` alone cut the line at its right edge, qa ERR-LIST-TOOLTIP-CHROME; live's mate slider-limits title
/// tip on 2 lines).
fn tip_text(window: &Window, text: &str, el: impl IntoElement) -> Div {
    let max = RICH_TOOLTIP_W - 14.;
    div().debug_selector(|| "tip-text".into()).when(text_width(window, text, 12.) > max, |d| d.w(px(max))).child(el)
}

/// A tip's text as live's HTML draws it: a run of spaces as one ("…properly:  A parameter…", "…500000.  Current…";
/// qa ERR-TIP-DOUBLE-SPACE-RENDER). The text itself keeps them.
fn html_spaces(text: &SharedString) -> SharedString {
    if !text.contains("  ") {
        return text.clone();
    }
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if !(c == ' ' && out.ends_with(' ')) {
            out.push(c);
        }
    }
    out.into()
}

/// [`tooltip`] with some words bold (byte ranges of `text`), as live's "**Submit** requires **Approvers**".
pub fn bold_tooltip(text: &'static str, bold: &'static [std::ops::Range<usize>]) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |window, cx| {
        let words = move |_: &mut Window, _: &mut App| StyledText::new(text).with_highlights(bold.iter().map(|r| (r.clone(), FontWeight::BOLD.into())));
        styled_tooltip(Tooltip::element(move |w, cx| {
            let el = words(w, cx);
            tip_text(w, text, el)
        }), text.into(), window, cx)
    }
}

/// [`tooltip`] for user-authored text (a part's or a tab's name): drawn
/// as is in every UI language (see [`user_text`]).
pub fn user_tooltip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text: SharedString = text.into();
    move |window, cx| {
        let t = text.clone();
        styled_tooltip(Tooltip::element(move |w, _| tip_text(w, &t, user_text(t.clone()))), text.clone(), window, cx)
    }
}

/// Live's tooltip shadow (feel/shadows: `.tooltip`, `0 1px 2px rgba(0,0,0,.2)`).
fn tooltip_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(1.)), blur_radius: px(2.), spread_radius: px(0.), inset: false }]
}

fn styled_tooltip(tip: Tooltip, text: SharedString, window: &mut Window, cx: &mut App) -> AnyView {
    {
        let pal = p();
        let tip = tip
            .bg(pal.tooltip)
            .text_color(pal.text)
            .border_0()
            .rounded(px(0.))
            .shadow(tooltip_shadow())
            .px(px(7.))
            .pt(px(4.))
            .pb(px(3.))
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            // Live `.tooltip-inner`: at most 400 px wide, a longer text wraps (errors-1123 mate slider-limits: 2 lines).
            .max_w(px(RICH_TOOLTIP_W))
            .build(window, cx);
        cx.new(|_| MenuAwareTip(tip, text)).into()
    }
}

/// A tooltip hidden while a context menu is open: a row's tooltip (shown
/// once the pointer rests on the right-clicked row) was drawn over the
/// menu's first items, e.g. "Open version Start" over "Open".
struct MenuAwareTip(AnyView, SharedString);

impl Render for MenuAwareTip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = !crate::os_menu::is_open(cx);
        if shown {
            crate::ui_script::note_tooltip(&self.1);
        }
        div().when(shown, |d| d.child(div().child(self.0.clone()).with_animation("tooltip-fade", Animation::new(TOOLTIP_FADE_IN), |c, t| c.opacity(t))))
    }
}

/// Note: a custom variant paints only 20% of `color` at rest, so solid
/// fills (the active tool) are set with `.bg()` on the button instead.
fn tool_variant(cx: &App, active: bool, _grouped: bool) -> ButtonCustomVariant {
    tool_variant_hover(cx, active, ACTIVE_HOVER)
}

/// A running tool hovered (live toolbar-states addendum 11-16): its hover colour under a 10 % grey overlay, flat (no
/// top shadow): #eaeaea → #dfdfdf; a sketch tool joined to its ⌄ (#d7d7d7) → #cecece.
const ACTIVE_HOVER: u32 = 0xdfdfdf;
const SKETCH_GROUP_ACTIVE_HOVER: u32 = 0xcecece;

fn tool_variant_hover(cx: &App, active: bool, active_hover: u32) -> ButtonCustomVariant {
    let pal = p();
    let hover: Rgba = if active { gpui_kit::rgb(active_hover) } else { pal.tool_hover };
    ButtonCustomVariant::new(cx)
        .color(gpui_kit::transparent_white())
        .foreground(pal.text.into())
        .hover(hover.into())
        .active(pal.tool_active.into())
        .flat_hover(active)
        .shadow(false)
}

/// A split tool's dropdown arrow (live toolbar-states 11-16): hovered #d7d7d7; open #d7d7d7 with the pressed inset
/// shadow.
fn caret_variant(cx: &App) -> ButtonCustomVariant {
    let pal = p();
    ButtonCustomVariant::new(cx)
        .color(gpui_kit::transparent_white())
        .foreground(pal.text.into())
        .hover(pal.tool_hover_group.into())
        .active(pal.tool_hover_group.into())
        .pressed_inset(true)
        .shadow(false)
}

/// Onshape's pressed toolbar button (a running tool, an open dropdown arrow): `inset 0 1px 1px rgba(0,0,0,0.247)`
/// over #eaeaea (live toolbar-states 11-16).
pub fn pressed_shadow() -> Vec<BoxShadow> {
    vec![gpui_kit::component::button::pressed_inset_shadow()]
}

/// Search tools' "Highlight tool in toolbar" on a toolbar button (live psui/toolbar-highlight 11-17: `.is-highlighted`
/// on the button itself, background #fcda7f and `box-shadow: inset 0 0 0 2px #fab601`, following its corners; no
/// border, so nothing in the toolbar moves).
pub fn search_highlight<E: Styled>(b: E) -> E {
    let ring = BoxShadow { color: gpui_kit::rgb(0xfab601).into(), offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(2.), inset: true };
    b.bg(gpui_kit::rgb(0xfcda7f)).shadow(vec![ring])
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
        .when(active, |b| b.bg(p().tool_active).shadow(pressed_shadow()))
}

/// The icon half of a split tool: like [`tool_button`], but with the
/// group hover (`#d7d7d7`) and only the left corners rounded. Put it in a
/// [`tool_group`] with a [`tool_caret`].
pub fn split_tool_button(id: impl Into<ElementId>, icon: &str, active: bool, cx: &App) -> Button {
    split_tool_button_with(id, assets::icon(icon, TOOL_ICON, cx), active, cx)
}

/// [`split_tool_button`] showing `content` (a custom feature's initials) in place of an icon.
pub fn split_tool_button_with(id: impl Into<ElementId>, content: impl IntoElement, active: bool, cx: &App) -> Button {
    split_tool_button_hover(id, content, active, ACTIVE_HOVER, cx)
}

/// [`split_tool_button`] in the sketch toolbar: running and hovered it reads #cecece (live: Line).
pub fn sketch_split_tool_button(id: impl Into<ElementId>, icon: &str, active: bool, cx: &App) -> Button {
    split_tool_button_hover(id, assets::icon(icon, TOOL_ICON, cx), active, SKETCH_GROUP_ACTIVE_HOVER, cx)
}

fn split_tool_button_hover(id: impl Into<ElementId>, content: impl IntoElement, active: bool, active_hover: u32, cx: &App) -> Button {
    Button::new(id)
        .custom(tool_variant_hover(cx, active, active_hover))
        .child(content)
        .w(px(TOOL_CELL))
        .h(px(TOOL_CELL))
        .p_0()
        .rounded(px(0.))
        .rounded_l(px(3.))
        .when(active, |b| b.bg(p().tool_active).shadow(pressed_shadow()))
}

/// A split tool: hovering either half fills both in `#d7d7d7` (live).
pub fn tool_group(name: impl Into<SharedString>, main: impl IntoElement, caret: impl IntoElement) -> Div {
    let pal = p();
    let name: SharedString = name.into();
    let (n1, n2) = (name.clone(), name.clone());
    h_flex()
        .flex_none()
        .group(name)
        // Live (toolbar-states 11-16): the group lit as one, the tool #eaeaea, its arrow #d7d7d7.
        .child(div().rounded_l(px(3.)).group_hover(n1, |d| d.bg(pal.tool_hover)).child(main))
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
    /// The title-only box's width, measured on its first frame (drawn hidden until then).
    width: std::rc::Rc<std::cell::Cell<Option<Pixels>>>,
    /// The title beside the element, not under it (a text box's: live workflows-1123 06 s2, "Filter inputs" at the
    /// box's right end); the expanded tooltip still goes under it.
    beside: bool,
    /// The expanded body drawn by its owner (rich text), in place of the description's paragraphs.
    body: Option<std::rc::Rc<dyn Fn() -> AnyElement>>,
}

/// Width of the expanded tooltip (live).
const RICH_TOOLTIP_W: f32 = 400.;

/// Width of a version's tooltip with its description (live2 recheck-1119).
const VERSION_TIP_W: f32 = 300.;

impl Render for RichTooltip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Live's recorder reads the whole tooltip, title then description (fs-co-complete-off, fs-ps-ws-feature-tooltip).
        if crate::ui_script::active() {
            let text = if self.description.is_empty() { self.title.to_string() } else { format!("{} {}", self.title, self.description) };
            crate::ui_script::note_tooltip(&text);
        }
        let pal = p();
        // Live (part-studio/10, 11; dom/hover-states.json): both states sit
        // 3 px under the tool, centered on it; a 26 px title row, 12 px / 500,
        // 4 px padding; the description 12 px on 18 px lines, 4 px padding.
        // Placed in window coordinates (`anchored`), so gpui's pointer-based
        // tooltip origin doesn't matter. Tooltips are drawn outside the
        // window's element tree, so the UI font is set here.
        let shadow = tooltip_shadow();
        let title = div().px(px(4.)).h(px(26.)).flex().items_center().bg(pal.tooltip).font_weight(FontWeight::MEDIUM).whitespace_nowrap().child(self.title.clone());
        let body = if self.expanded {
            v_flex()
                .w(px(RICH_TOOLTIP_W))
                .bg(gpui_kit::rgb(0xffffff))
                .child(title)
                .child(div().p(px(4.)).line_height(px(18.)).map(|d| match &self.body {
                    Some(body) => d.child(body()),
                    None => d.child(tool_description(&self.description)),
                }))
                .into_any_element()
        } else {
            title.into_any_element()
        };
        let measured = self.width.clone();
        let w = if self.expanded { Some(px(RICH_TOOLTIP_W)) } else { self.width.get() };
        let content = div()
            .debug_selector(|| "rich-tooltip".into())
            .relative()
            .flex_none()
            .font_family(cx.theme().font_family.clone())
            .text_size(px(12.))
            .text_color(pal.text)
            .shadow(shadow)
            .child(body)
            .child(canvas(move |b, window, _| {
                if measured.get() != Some(b.size.width) {
                    measured.set(Some(b.size.width));
                    window.request_animation_frame();
                }
            }, |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .when(w.is_none(), |c| c.opacity(0.));
        // Live (tooltips-1118): centred on the tool, its top 3 px under it, kept inside the page only when it would
        // leave it (a fixed 400 px centering strip snapped to the window put a left tool's short tip ~185 px off).
        let a = self.anchor;
        let win = window.viewport_size().width;
        let x = w.map_or(a.left(), |w| (a.center().x - w * 0.5).min(win - w - px(2.)).max(px(2.)));
        let at = if self.beside && !self.expanded { point(a.right() + px(3.), a.center().y - px(13.)) } else { point(x, a.bottom() + px(3.)) };
        // Never drawn over an open context menu (QA3: a stale tooltip covered its items).
        div().when(!crate::os_menu::is_open(cx), |d| d.child(anchored().position(at).snap_to_window().child(content)))
    }
}

/// A text naming stored entities ("Face of Extrude 1", a selection chip):
/// its wording translated by template, the name kept ([`crate::i18n::filled`]).
pub fn entity_text(text: impl Into<SharedString>) -> StyledText {
    let text: SharedString = text.into();
    match crate::i18n::filled(&text) {
        Some(t) => StyledText::new(t),
        None => StyledText::new(text),
    }
}

/// A feature dialog's string (parameter label, enum option) in the UI
/// language as the feature specs have it ([`crate::i18n::spec`]); drawn
/// as-is like [`user_text`].
pub fn spec_text(english: &str) -> StyledText {
    StyledText::new(crate::i18n::spec(english))
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
    // A numbered list as live's `<ol>` (live dom ui2-tip-list-frame-*): no
    // gap above, 14 px below; each `<li>` 20 px in, its "1." marker outside,
    // right-aligned a space before the text.
    let blocks = crate::tool_text::blocks(text);
    let ends_list: Vec<bool> = (0..blocks.len()).map(|i| blocks[i].0.is_some() && blocks.get(i + 1).is_none_or(|b| b.0.is_none())).collect();
    v_flex().children(blocks.into_iter().zip(ends_list).map(|((n, t), end)| match n {
        None => div().child(t),
        Some(n) => h_flex()
            .items_start()
            .when(end, |d| d.mb(px(14.)))
            .child(div().w(px(20.)).flex_none().pr(px(4.)).flex().justify_end().child(format!("{n}.")))
            .child(div().flex_1().min_w_0().child(t)),
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
                    cx.background_executor().timer(TOOLTIP_EXPAND_DELAY).await;
                    let _ = this.update(cx, |t, cx| {
                        t.expanded = true;
                        cx.notify();
                    });
                })
                .detach();
            }
            RichTooltip { title: title.clone(), description: description.clone(), expanded: false, anchor, width: Default::default(), beside: false, body: None }
        });
        view.into()
    }
}

/// A text box's help (live workflows-1123 06 s2-s4, the Configurations panel's "Filter inputs"): the title beside the
/// box at once ([`HELP_TIP_DELAY`]), then [`HELP_EXPAND_DELAY`] later the expanded tooltip under it, centred, `body`
/// under the title band.
/// `text` is the body's words, for the parity recorder. The view shown is kept in `held` for [`held_help`].
pub fn help_tooltip(title: impl Into<SharedString>, text: impl Into<SharedString>, body: impl Fn() -> AnyElement + 'static, anchor: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>, held: HeldHelp) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let (title, text): (SharedString, SharedString) = (title.into(), text.into());
    let body: std::rc::Rc<dyn Fn() -> AnyElement> = std::rc::Rc::new(body);
    move |_, cx| {
        let (anchor, body) = (anchor.get(), body.clone());
        let tip = cx.new(|cx| {
            cx.spawn(async move |this: WeakEntity<RichTooltip>, cx| {
                cx.background_executor().timer(HELP_EXPAND_DELAY).await;
                let _ = this.update(cx, |t, cx| {
                    t.expanded = true;
                    cx.notify();
                });
            })
            .detach();
            RichTooltip { title: title.clone(), description: text.clone(), expanded: false, anchor, width: Default::default(), beside: true, body: Some(body) }
        });
        *held.borrow_mut() = Some(tip.clone());
        tip.into()
    }
}

/// A text box's help title shows as the pointer arrives (live asm config-filter-help-1123: "at once")…
pub const HELP_TIP_DELAY: std::time::Duration = std::time::Duration::ZERO;
/// …and moves under the box, expanded, 1.97 s after it (the same capture).
const HELP_EXPAND_DELAY: std::time::Duration = std::time::Duration::from_millis(1970);

/// A [`help_tooltip`]'s view, kept by its owner: gpui hides a tooltip once a key is pressed, live keeps this help
/// while the pointer rests on the box and the user types (workflows-1123 06 s3-s4: a new input's name typed, Enter).
pub type HeldHelp = std::rc::Rc<std::cell::RefCell<Option<Entity<RichTooltip>>>>;

/// The held help to draw while the last input was a key and the pointer is still on `anchor` (none once gpui shows
/// its own again). The owner empties `held` when the pointer leaves or presses the box.
pub fn held_help(held: &HeldHelp, anchor: Bounds<Pixels>, window: &Window) -> Option<AnyElement> {
    let tip = held.borrow().clone()?;
    (window.last_input_was_keyboard() && anchor.contains(&window.mouse_position())).then(|| deferred(tip).with_priority(1).into_any_element())
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
    /// Fades in over this long once shown, from this opacity (a feature row's, live feel/list-1118).
    fade: Option<(std::time::Duration, f32)>,
    /// The expanded block upright, not italic (a version's description).
    upright: bool,
}

/// A feature list row's tooltip: shown 80 ms after the pointer arrives, fully by ~200 ms (live feel/list-1118, three runs:
/// fading in from 65–100 ms, opaque at 166–209; 11-13's "from 160, full by 390" was a slower lane). Its fade starts a
/// fifth in: from nothing, its first frame drew nothing, and on a slow lane the tip showed 150–167 ms after the row
/// lit (live recheck 11-18).
/// A menu row's side tooltip unfolds sooner (live fs-ps-ws-feature-tooltip: unfolded within a 1.5 s hover).
const SIDE_TIP_EXPAND_DELAY: std::time::Duration = std::time::Duration::from_millis(1500);
/// A toolbar tooltip's longer help unfolds this long after it shows (live drift-1120: 1.77-1.90 s after the pointer
/// arrives; 11-13's 2000 ms after showing was the old 400 ms tooltip delay).
pub const TOOLTIP_EXPAND_DELAY: std::time::Duration = std::time::Duration::from_millis(1800);
/// A toolbar tooltip shows as the pointer arrives, cold or warm (live drift-1120: 15-45 ms, was 400 ms on 11-13).
pub const TOOLBAR_TIP_DELAY: std::time::Duration = std::time::Duration::from_millis(20);
pub const ROW_TIP_DELAY: std::time::Duration = std::time::Duration::from_millis(80);
const ROW_TIP_FADE: (std::time::Duration, f32) = (std::time::Duration::from_millis(110), 0.2);
/// Tooltips fade in (live feel/chrome.json "tooltips": fully shown 470-545 ms after the pointer arrives, the 400 ms
/// delay and ~150 ms of Bootstrap's fade).
pub const TOOLTIP_FADE_IN: std::time::Duration = std::time::Duration::from_millis(150);

impl Render for PlacedTooltip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Noted only when drawn: not under an open right-click menu (a menu row's own, with its expanded text,
        // is), nor while pressed; the expanded text after the title once shown (live fs-ps-ws-feature-tooltip).
        if (!crate::os_menu::is_open(cx) || self.expanded.is_some()) && !self.pressed.get() {
            match self.expanded.as_ref().filter(|(_, shown)| *shown) {
                Some((extra, _)) => crate::ui_script::note_tooltip(&format!("{} {extra}", self.text)),
                None => crate::ui_script::note_tooltip(&self.text),
            }
        }
        let pal = p();
        let shadow = tooltip_shadow();
        let a = self.anchor;
        let h = px(26.);
        let measured = self.width.clone();
        // Live (errors-1118 tip-chamfer-full): at most 400 px wide, a longer text wraps (18 px lines, 4 px padding:
        // two lines 44 px high).
        let title = div()
            .font_weight(FontWeight::MEDIUM)
            .min_h(h)
            .max_w(px(RICH_TOOLTIP_W))
            .px(px(4.))
            .py(px(4.))
            .line_height(px(18.))
            .flex_none()
            .bg(pal.tooltip)
            .map(|d| if self.user { d.child(user_text(self.text.clone())) } else { d.child(html_spaces(&self.text)) });
        // Live `.tooltip-expand`: the expanded fill, 4 px padding, at most
        // 400 px wide, under the title.
        let extra = self.expanded.as_ref().filter(|(_, shown)| *shown).map(|(text, _)| div().max_w(px(RICH_TOOLTIP_W)).p(px(4.)).line_height(px(18.)).bg(gpui_kit::rgb(0xffffff)).when(!self.upright, |d| d.italic()).child(text.clone()));
        let content = div()
            .debug_selector(|| "placed-tooltip".into())
            .font_family(cx.theme().font_family.clone())
            .text_size(px(12.))
            .text_color(pal.text)
            .flex()
            .flex_col()
            .flex_none()
            .shadow(shadow)
            // A version's described tooltip is 300 px wide, title band and description alike (live2 recheck-1119).
            .when(self.upright && extra.is_some(), |d| d.w(px(VERSION_TIP_W)))
            .child(title)
            .children(extra)
            .child(canvas(move |b, window, _| {
                if measured.get() != Some(b.size.width) {
                    measured.set(Some(b.size.width));
                    // (`refresh` is ignored while drawing.)
                    window.request_animation_frame();
                }
            }, |_, _, _, _| {}).absolute().top_0().left_0().size_full());
        // A version's tip grown into its description stays centred on the row (live2 versions-1119: its top 13 px up).
        let grown = self.upright && self.expanded.as_ref().is_some_and(|(_, shown)| *shown);
        let mid_y = a.center().y - h * if grown { 1.0 } else { 0.5 };
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
            // (Live cov-1120 R8: 3 px under the rollback bar's 7 px line, centred in its 26 px row.)
            (TipSide::Below, Some(w)) => point(clamp_x(a.center().x - w * 0.5, w), a.center().y + px(6.5)),
            (_, None) => point(px(0.), mid_y),
        };
        let hidden = self.side != TipSide::Right && w.is_none();
        let content = content.when(hidden, |c| c.opacity(0.));
        let content = match self.fade {
            Some((fade, from)) => content.with_animation("tooltip-fade", Animation::new(fade), move |c, t| c.opacity(from + (1. - from) * t)).into_any_element(),
            None => content.into_any_element(),
        };
        div().when(!crate::os_menu::is_open(cx) && !self.pressed.get(), |d| d.child(anchored().position(at).snap_to_window().child(content)))
    }
}

/// Gives `el` an Onshape tooltip showing `text` on `side` (see [`PlacedTooltip`]).
/// Like a browser's, the tooltip hides when the element is pressed (a strip
/// button opening its panel) and shows again only after the pointer has left
/// it (qa4 F22).
pub fn with_tip(el: Stateful<Div>, text: impl Into<SharedString>, side: TipSide) -> Stateful<Div> {
    tip_with(el, text.into(), side, false, Pixels::ZERO, false)
}

/// [`with_tip`] whose tooltip stays when the element is clicked (live vsm-1: a Variable Studio value's chip over its
/// column header).
pub fn with_tip_kept_on_click(el: Stateful<Div>, text: impl Into<SharedString>, side: TipSide) -> Stateful<Div> {
    tip_with(el, text.into(), side, false, Pixels::ZERO, true)
}

/// [`with_tip`] whose text holds a user's name (never translated): the documents page's "Click to open <name>." above a
/// name (live docs-page-gaps-1122).
pub fn with_user_tip(el: Stateful<Div>, text: impl Into<SharedString>, side: TipSide) -> Stateful<Div> {
    tip_with(el, text.into(), side, true, Pixels::ZERO, false)
}

/// A dialog field's tooltip right of the dialog, beside the field (live errors-1123: a selection box `inset` in from
/// the dialog's right edge).
pub fn with_dialog_field_tooltip(el: Stateful<Div>, text: impl Into<SharedString>, inset: Pixels) -> Stateful<Div> {
    tip_with(el, text.into(), TipSide::Right, false, inset, false)
}

/// A feature dialog's red title's tooltip, fixed where live puts it whatever the pointer does (live
/// err-tip-placement-1123: the title at 246, 79, 164 wide, its tip at 385, 80: 25 px in from the title's right, 1 px down;
/// live's title box is 28 px high, ours 22 with the text on the same rows, so the tip is 2 px above ours).
pub fn with_dialog_title_tooltip(el: Stateful<Div>, text: impl Into<SharedString>) -> Stateful<Div> {
    let text: SharedString = text.into();
    let anchor: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
    let a2 = anchor.clone();
    el.relative()
        .child(canvas(move |b, _, _| a2.set(b), |_, _, _, _| {}).absolute().top(px(-2.)).left_0().right(px(28.)).h(px(26.)))
        .tooltip(move |_, cx| {
            let (text, anchor) = (text.clone(), anchor.get());
            cx.new(|_| PlacedTooltip { text, side: TipSide::Right, anchor, width: Default::default(), pressed: Default::default(), expanded: None, user: false, fade: Some((TOOLTIP_FADE_IN, 0.)), upright: false }).into()
        })
}

/// A list row's tooltip, beside the row (live part-studio/16, 95), showing
/// its stored name (user text, never translated).
pub fn with_user_side_tooltip(row: Stateful<Div>, text: impl Into<SharedString>) -> Stateful<Div> {
    tip_with(row, text.into(), TipSide::Right, true, Pixels::ZERO, false)
}

/// [`with_user_side_tooltip`] for a list row drawn [`LIST_ROW_INSET`] short of the list's right edge: the tooltip
/// beside the list, as a feature row's (live list-1119: the Parts row's). A click on the row, even before its tooltip
/// shows, leaves it to show by the still pointer (qa wf23 17 s11: "Part 1" clicked into Boolean's field).
pub fn with_list_row_tooltip(row: Stateful<Div>, text: impl Into<SharedString>) -> Stateful<Div> {
    tip_with(row, text.into(), TipSide::Right, true, LIST_ROW_INSET, true).tooltip_after_press()
}

/// How far the default geometry, Parts and curve rows stop short of the list's right edge.
pub const LIST_ROW_INSET: Pixels = px(5.);

fn tip_with(el: Stateful<Div>, text: SharedString, side: TipSide, user: bool, reach: Pixels, stays: bool) -> Stateful<Div> {
    // The parity suite clicks a button by its tooltip.
    let el = crate::ui_script::mark(el, || format!("button:{text}"));
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
            .bottom_0()
            .right(-reach),
        )
        .on_any_mouse_down(move |_, _, _| p3.set(!stays))
        .when(stays, |d| d.tooltip_stays_on_click())
        .tooltip(move |_, cx| {
            let (text, anchor, pressed) = (text.clone(), anchor.get(), pressed.clone());
            cx.new(|_| PlacedTooltip { text, side, anchor, width: Default::default(), pressed, expanded: None, user, fade: Some((TOOLTIP_FADE_IN, 0.)), upright: false }).into()
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
                    cx.background_executor().timer(SIDE_TIP_EXPAND_DELAY).await;
                    let _ = this.update(cx, |t, cx| {
                        if let Some((_, shown)) = &mut t.expanded {
                            *shown = true;
                        }
                        cx.notify();
                    });
                })
                .detach();
                PlacedTooltip { text, side: TipSide::Right, anchor, width: Default::default(), pressed: Default::default(), expanded: Some((extra, false)), user: false, fade: Some((TOOLTIP_FADE_IN, 0.)), upright: false }
            })
            .into()
        })
}

/// A version row's tooltip, in two stages (live2 versions-1119): "Open version V1" on one line beside the row 130 ms
/// after the pointer arrives, fading in until ~495 ms; at ~2.3 s it grows into the version's description (upright, on
/// white) under the title. Without a description it stays one line.
pub const VERSION_TIP_DELAY: std::time::Duration = std::time::Duration::from_millis(130);
/// When the version tooltip's description unfolds, after it shows (live2 versions-1119: ~2.3 s after the hover).
pub const VERSION_TIP_EXPAND: std::time::Duration = std::time::Duration::from_millis(2170);
const VERSION_TIP_FADE: std::time::Duration = std::time::Duration::from_millis(365);

pub fn with_described_side_tooltip(row: Stateful<Div>, text: impl Into<SharedString>, description: Option<SharedString>, delay: std::time::Duration) -> Stateful<Div> {
    let text: SharedString = text.into();
    let anchor: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
    let a2 = anchor.clone();
    row.relative()
        .child(canvas(move |b, _, _| a2.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
        .tooltip(move |_, cx| {
            let (text, expanded, anchor) = (text.clone(), description.clone().map(|d| (d, false)), anchor.get());
            cx.new(|cx| {
                if expanded.is_some() {
                    cx.spawn(async move |this: WeakEntity<PlacedTooltip>, cx| {
                        cx.background_executor().timer(VERSION_TIP_EXPAND).await;
                        let _ = this.update(cx, |t, cx| {
                            if let Some((_, shown)) = &mut t.expanded {
                                *shown = true;
                            }
                            cx.notify();
                        });
                    })
                    .detach();
                }
                PlacedTooltip { text, side: TipSide::Right, anchor, width: Default::default(), pressed: Default::default(), expanded, user: false, fade: Some((VERSION_TIP_FADE, 0.)), upright: true }
            })
            .into()
        })
        .tooltip_show_delay(delay)
}

/// [`with_user_side_tooltip`] whose text and place are worked out when the
/// tooltip opens, not each time the row renders (a feature row's "Extrude 1
/// <- Sketch 1"): `anchor` gives the row's bounds then. Its text is the row's
/// name and references: user-authored, never translated.
pub fn with_lazy_side_tooltip(row: Stateful<Div>, anchor: impl Fn() -> Bounds<Pixels> + 'static, text: impl Fn(&App) -> SharedString + 'static) -> Stateful<Div> {
    row.tooltip(move |_, cx| {
        let (text, anchor) = (text(cx), anchor());
        cx.new(|_| PlacedTooltip { text, side: TipSide::Right, anchor, width: Default::default(), pressed: Default::default(), expanded: None, user: true, fade: Some(ROW_TIP_FADE), upright: false }).into()
    })
    .tooltip_show_delay(ROW_TIP_DELAY)
    // (A press doesn't hold it back: live workflow 10, a double-click opening Extrude 1's dialog leaves its name shown.)
    .tooltip_after_press()
}

/// A feature row's name-only tip ("Block [Extrude]"), beside the row where its tooltip goes: a quick double-click's,
/// shown once the dialog opens (live row-tip-dblclick-1124: fully shown 0.3-1.2 s later). The caller holds it.
pub fn row_name_tip(text: SharedString, anchor: Bounds<Pixels>, cx: &mut App) -> AnyView {
    cx.new(|_| PlacedTooltip { text, side: TipSide::Right, anchor, width: Default::default(), pressed: Default::default(), expanded: None, user: true, fade: Some((std::time::Duration::from_millis(300), 0.)), upright: false }).into()
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
        .when(active, |b| b.bg(p().tool_active).shadow(pressed_shadow()))
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
        .custom(caret_variant(cx))
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
        .custom(caret_variant(cx))
        // Live: the 10 x 6 `expanded` sprite at 1:1, about 2 px thick.
        .child(glyph("tool-caret", 10., pal.text))
        .w(px(TOOL_CARET_W))
        .h(px(TOOL_CELL))
        .p_0()
        .rounded(px(0.))
        .rounded_r(px(3.))
}

/// A toolbar dropdown row's shortcut chips for an Onshape shortcut string such as "shift-f" or "ctrl+m" (live2
/// feel/shadows "Toolbar dropdown list box model": `.shortcut-key` min-width 17, padding 0 2px, 1 px #AAA border,
/// radius 2, 11 px on a 16.5 px line, no background, touching). They sit on the foot of their 22.5 px line, as live's
/// inline blocks on its baseline.
pub fn shortcut_chips(shortcut: &str) -> Div {
    let keys: Vec<String> = shortcut.split(['-', '+']).filter(|k| !k.is_empty()).map(|k| k.to_lowercase()).collect();
    let pal = p();
    h_flex().h(px(22.5)).items_end().children(keys.into_iter().map(move |k| {
        div().min_w(px(17.)).px(px(2.)).h(px(19.5)).flex().items_center().justify_center().rounded(px(2.)).border_1().border_color(gpui::rgb(0xaaaaaa)).text_size(px(11.)).line_height(px(16.5)).text_color(pal.text).child(k)
    }))
}

/// A toolbar dropdown row (live2 feel/shadows "Toolbar dropdown list box model"): 32 px tall (34.5 with chips), the
/// 20 px full-color icon at the list's left + 8, the name at 14 px from + 32, the shortcut chips 8 px in from the
/// right. Add `.on_click` to the result.
pub fn menu_tool_item(name: &'static str, icon: &str, shortcut: Option<&'static str>) -> gpui_kit::component::menu::PopupMenuItem {
    let icon = icon.to_string();
    gpui_kit::component::menu::PopupMenuItem::element(move |_, cx| {
        let pal = p();
        let row = h_flex()
            .relative()
            // (The row's own 6 px padding: 5 here and the menu item's 1.)
            .h(px(if shortcut.is_some() { 34.5 } else { 32. }))
            .w_full()
            .child(div().pl(px(5.)).flex_none().flex().items_center().child(assets::icon(&icon, TOOL_ICON, cx)))
            .child(div().flex_1().mx(px(4.)).text_size(px(14.)).text_color(pal.text).child(name))
            .children(shortcut.map(shortcut_chips));
        crate::ui_script::mark(row, || format!("menu:{name}"))
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
        None => div().id(id).flex_none().tooltip(tooltip(title)).tooltip_show_delay(TOOLBAR_TIP_DELAY).child(el).into_any_element(),
    }
}

/// Wrap a toolbar tool so it shows the title at once and the description after 1.8 s.
pub fn with_rich_tooltip(id: impl Into<ElementId>, title: impl Into<SharedString>, description: impl Into<SharedString>, el: impl IntoElement) -> AnyElement {
    let anchor: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
    let a2 = anchor.clone();
    let title: SharedString = title.into();
    let name = title.clone();
    let id: ElementId = id.into();
    let selector = id.to_string();
    div()
        .id(id)
        .debug_selector(move || selector)
        .map(|d| crate::ui_script::mark(d, || format!("button:{name}")))
        .flex_none()
        .relative()
        .tooltip(rich_tooltip(title, description, anchor))
        .tooltip_show_delay(TOOLBAR_TIP_DELAY)
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
    // Live (part-studio/00): 19 px keys on the box's fill, 1 px #aaa with a
    // 2 px bottom edge, side by side ("alt/⌥" 35 px, "c" 17 px), 3 px after
    // the label, 2 px under the box's middle (text 1 px lower still).
    let chip = |t: &'static str, w: Option<f32>| {
        div()
            .when_some(w, |d, w| d.w(px(w)))
            .px(px(2.))
            .pt(px(2.))
            .min_w(px(17.))
            .h(px(19.))
            .relative()
            .top(px(2.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(2.))
            .border_1()
            .border_b_2()
            .border_color(pal.field_outline)
            .text_size(px(11.))
            .text_color(pal.text)
            .child(t)
    };
    h_flex()
        .id(id)
        .flex_none()
        .h(px(32.))
        .px(px(10.))
        .gap(px(3.))
        .rounded(px(3.))
        .bg(pal.search_fill)
        .hover(|d| d.bg(pal.border))
        .cursor_pointer()
        .when(open, |d| d.bg(pal.border))
        .child(div().text_size(px(14.)).text_color(pal.text).child(if open { "Close search" } else { "Search tools…" }))
        .when(open, |d| d.child(chip("escape", None)))
        .when(!open, |d| d.child(h_flex().child(chip("alt/⌥", Some(35.))).child(chip("c", Some(17.)))))
}

/// Toolbar undo / redo pair (Onshape puts them first in every toolbar).
pub fn undo_redo(doc: &Entity<crate::doc_model::DocModel>, cx: &App) -> AnyElement {
    // Onshape's titles (live dom/toolbar-partstudio.txt).
    undo_redo_with(doc, TOOL_CELL, ("Undo (cmd+z/ctrl+z)", "Redo (ctrl+y/shift+cmd+z)"), cx)
}

/// [`undo_redo`] for a view that takes Undo / Redo itself while a dialog is
/// open (the assembly's mate dialog): the buttons dispatch the actions and
/// show `state` (can undo, can redo).
pub fn undo_redo_actions(state: (bool, bool), cx: &App) -> AnyElement {
    let (can_undo, can_redo) = state;
    let button = |id: &'static str, icon: &'static str, tip: &'static str, enabled: bool, action: Box<dyn gpui_kit::Action>, cx: &App| {
        with_tooltip(
            SharedString::from(format!("tt-{id}")),
            SharedString::from(tip),
            tool_button(SharedString::from(format!("tb-{id}")), icon, false, cx).w(px(TOOL_CELL)).h(px(TOOL_CELL)).opacity(if enabled { 1.0 } else { 0.4 }).when(!enabled, |b| b.cursor(CursorStyle::OperationNotAllowed)).on_click(move |_, window, cx| {
                if enabled {
                    window.dispatch_action(action.boxed_clone(), cx);
                }
            }),
        )
    };
    h_flex()
        .flex_none()
        .gap(px(2.))
        .child(button("undo", "undo", "Undo (cmd+z/ctrl+z)", can_undo, Box::new(crate::actions::Undo), cx))
        .child(button("redo", "redo", "Redo (ctrl+y/shift+cmd+z)", can_redo, Box::new(crate::actions::Redo), cx))
        .into_any_element()
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
            crate::ui_script::mark(tool_button("tb-undo", "undo", false, cx), || "button:Undo".into()).w(px(cell)).h(px(cell)).opacity(if can_undo { 1.0 } else { 0.4 }).when(!can_undo, |b| b.cursor(CursorStyle::OperationNotAllowed)).on_click(move |_, _, cx| {
                if can_undo {
                    d1.update(cx, |d, cx| d.undo(cx));
                }
            }),
        ))
        .child(with_tooltip(
            "tt-redo",
            redo_tip,
            crate::ui_script::mark(tool_button("tb-redo", "redo", false, cx), || "button:Redo".into()).w(px(cell)).h(px(cell)).opacity(if can_redo { 1.0 } else { 0.4 }).when(!can_redo, |b| b.cursor(CursorStyle::OperationNotAllowed)).on_click(move |_, _, cx| {
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

/// [`os_checkbox`] with keyboard focus (feature dialogs; live feel/mirror 35, Merge with all just clicked): a 2 px
/// `#3e94ff` ring over the box's edge, 14 px across, no glow.
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
                .relative()
                .rounded(px(3.))
                .border_1()
                .border_color(if on || focused { pal.accent } else { gpui_kit::rgb(0x888888) })
                .bg(if on { pal.accent } else { gpui_kit::rgb(0xffffff) })
                .when(on, |d| d.child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).with_size(px(10.)).text_color(gpui_kit::rgb_static(0xffffff))))
                .when(focused, |d| d.child(div().absolute().top(px(-2.)).left(px(-2.)).size(px(14.)).rounded(px(4.)).border_2().border_color(gpui_kit::rgb(0x3e94ff)))),
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

/// The box a popup list hangs from: its parent's bottom edge, out of the flow, so opening the list moves nothing.
/// (A zero-height box in the flow still takes its column's gap: Depth's `#` list pushed the rows below 2 px down.)
pub fn popup_anchor() -> Div {
    div().absolute().left_0().right_0().top_full().h_0()
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
        // Live (document/rail-panel-*, dk-996): the active one's icon and bar
        // #1651b0 on #eaeaea; dark #90cef1 on the rail's own #2c2c2c.
        .map(|d| {
            let dark = noshape_scene::palette::is_dark();
            if !active {
                return d.child(assets::icon(icon, TOOL_ICON, cx));
            }
            let ink = if dark { 0x90cef1 } else { 0x1651b0 };
            let bar = div().absolute().top_0().bottom_0().w(px(4.)).bg(gpui_kit::rgb_static(ink));
            d.when(!dark, |d| d.bg(gpui_kit::rgb_static(0xeaeaea))).child(assets::icon_inked(icon, TOOL_ICON, ink, cx)).child(if indicator_left { bar.left_0() } else { bar.right_0() })
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
        std::fs::read_to_string(crate::paths::config_dir().join("panel_positions.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default(),
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
            let p = crate::paths::config_dir().join("panel_positions.json");
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
        std::fs::read_to_string(crate::paths::config_dir().join("right_panel_width.txt")).ok().and_then(|t| t.trim().parse::<f32>().ok()).filter(|w| w.is_finite()).unwrap_or(RIGHT_PANEL_W).max(RIGHT_PANEL_MIN),
    );
}

pub fn right_panel_w() -> f32 {
    RIGHT_W.with(|w| w.get())
}

/// The right panels' width for this run (the splitter also saves it).
pub fn set_right_panel_w(w: f32) {
    RIGHT_W.with(|c| c.set(w.max(RIGHT_PANEL_MIN)));
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
                set_right_panel_w(w);
                let p = crate::paths::config_dir().join("right_panel_width.txt");
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
        std::fs::read_to_string(crate::paths::config_dir().join("table_columns.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default(),
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
                let p = crate::paths::config_dir().join("table_columns.json");
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
    name_pill_on(name, offset, gpui_kit::rgb(0x1651b0))
}

/// [`name_pill`] on the colour `bg` (live cursors/672: an inserted Variable
/// Studio table's pill is `#1651b0` at 60 %, its text still white).
pub fn name_pill_on(name: SharedString, offset: Point<Pixels>, bg: Rgba) -> Div {
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
                .bg(bg)
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
/// of its first two words as written ("QA cube" → "Qc", "FS pack probe" →
/// "Fp", "uip Feature" → "uF", live 2361), or the first two letters of a
/// single word.
pub fn feature_initials(name: &str) -> String {
    let mut words = name.split_whitespace();
    let first = words.next().unwrap_or_default();
    let mut out: String = first.chars().take(1).collect();
    match words.next().and_then(|w| w.chars().next()) {
        Some(c) => out.push(c),
        None => out.extend(first.chars().skip(1).take(1)),
    }
    out
}

/// Onshape's custom-feature icon (`div.tool-initials-icon`, live part-studio/1252, custom-feature-row-icon-1123): the
/// initials 11 px in a 20 px box, 1 px `#999` border, radius 3, in the custom features dropdown.
pub fn initials_icon(name: &str) -> Div {
    div()
        .flex_none()
        .size(px(20.))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(gpui_kit::rgb(0x999999))
        .rounded(px(3.))
        .bg(gpui_kit::rgb(0xffffff))
        .text_size(px(11.))
        .text_color(gpui_kit::rgb(0x333333))
        .child(feature_initials(name))
}

/// A custom feature's feature-list row icon (`span.os-tool-initials-icon`, live custom-feature-row-icon-1123): 17 × 15
/// centred in the 20 px icon slot, 1 px `#999` border (never red), radius 2, see-through; the initials 10 px `#333`,
/// the row's red `#AA252D` while it fails, semibold while the row is selected.
pub fn initials_row_icon(name: &str, failing: bool, selected: bool) -> Div {
    div().flex_none().size(px(20.)).flex().items_center().justify_center().child(
        div()
            .w(px(17.))
            .h(px(15.))
            .flex()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(gpui_kit::rgb(0x999999))
            .rounded(px(2.))
            .text_size(px(10.))
            .line_height(px(15.))
            .font_weight(if selected { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
            .text_color(gpui_kit::rgb(if failing { 0xaa252d } else { 0x333333 }))
            .child(feature_initials(name)),
    )
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

    /// Live's toolbar tooltips write the key in lower case (live2 p2b2 toolbar0: "Coincident (i)", "Concentric
    /// (shift+o)"; behaviours: "Extrude (shift+e)") with live's names; the labels keep their own case elsewhere.
    #[::core::prelude::v1::test]
    fn toolbar_tooltips_write_keys_as_live() {
        for (label, tooltip) in [
            ("Coincident (I)", "Coincident (i)"),
            ("Concentric (Shift+O)", "Concentric (shift+o)"),
            ("Perpendicular (Shift+L)", "Perpendicular (shift+l)"),
            ("Extrude (Shift+E)", "Extrude (shift+e)"),
            ("Center rectangle (R)", "Center point rectangle (r)"),
            ("Flow", "Flow"),
        ] {
            assert_eq!(onshape_title(label), tooltip);
        }
    }

    #[::core::prelude::v1::test]
    fn box_shadow_blur_is_css() {
        // CSS blurs a shadow with a Gaussian of half its blur radius; vendored gpui's shaders take the sigma
        // (vendor/gpui-pre/NOSHAPE.md). Live's values are written as they are: no halving at the call sites.
        let dialog = BoxShadow::new(px(0.), px(5.), hsla(0., 0., 0., 0.2)).blur_radius(px(10.));
        assert_eq!(dialog.blur_sigma(), px(5.));
        let pressed = &pressed_shadow()[0];
        assert!(pressed.inset);
        assert_eq!((pressed.offset, pressed.blur_radius, pressed.blur_sigma()), (point(px(0.), px(1.)), px(1.), px(0.5)));
    }

    #[::core::prelude::v1::test]
    fn failing_custom_feature_icon_is_outlined_red() {
        // Live (qa5 featurescript (7)): a failing custom feature's "Qc" box
        // is outlined red; otherwise #999.
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
        // Every app crate's sources (crates/app-core, crates/app, the studio UIs crates/*-ui).
        for e in std::fs::read_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")).unwrap().flatten() {
            let src = e.path().join("src");
            let name = e.file_name().to_string_lossy().to_string();
            if (name.starts_with("app") || name.ends_with("-ui")) && src.is_dir() {
                walk(&src, &mut files);
            }
        }
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
        assert_eq!(feature_initials("uip Feature"), "uF");
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

/// A Variable Studio table being dragged by its header (live psb 10-20 t,
/// cursors/672): the ghost is the reorder pill with the table name, lighter
/// than the Configurations panel's (`#1651b0` at 60 %, the table showing
/// through), and the pointer shows `grabbing` meanwhile.
#[derive(Clone)]
pub struct VarTableDrag {
    pub studio: noshape_model::document::TabId,
    pub name: SharedString,
    /// Where the header was grabbed: the pill sits at the pointer instead.
    pub offset: Point<Pixels>,
}

impl Render for VarTableDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        name_pill_on(self.name.clone(), self.offset, gpui_kit::rgba(0x1651b099))
    }
}

/// Make `el` (an inserted table's header, its empty part) the drag handle for reordering the tables.
pub fn var_table_drag_source<E: StatefulInteractiveElement>(el: E, studio: noshape_model::document::TabId, name: SharedString) -> E {
    el.on_drag(VarTableDrag { studio, name, offset: Point::default() }, |d, offset, _, cx| cx.new(|_| VarTableDrag { offset, ..d.clone() }))
}
