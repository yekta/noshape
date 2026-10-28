//! Part Studio view tools: tool search palette (Alt+C), keyboard shortcut
//! sheet (Shift+?), display modes, section view (Shift+X) and feature rename.

use super::PartStudioView;
use super::feature_icon;
use crate::scene_builder::DisplayMode;
use crate::theme::{p, with_alpha};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use noshape_model::features;
use noshape_model::reference::FeatureId;
use crate::shortcuts::{self, KeyStyle, SheetRow, Tab};

#[derive(Clone)]
pub(super) enum PaletteItem {
    Feature(&'static str),
    /// A sketch tool (only while a sketch is open).
    SketchTool(crate::sketch_mode::Tool),
}

impl PartStudioView {
    pub fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| InputState::new(window, cx));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => {
                this.palette_query = st.read(cx).value().to_string();
                this.ui.palette_sel = 0;
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {
                let items = this.palette_items(cx);
                if let Some(item) = items.get(this.ui.palette_sel).or(items.first()).cloned() {
                    this.run_palette(item, window, cx);
                }
            }
            _ => {}
        });
        st.update(cx, |s, cx| s.focus(window, cx));
        self.palette_query.clear();
        self.ui.palette_sel = 0;
        self.palette = Some((st, sub));
        cx.notify();
    }

    /// The "Search tools" / "Close search" toggle.
    pub fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.is_some() {
            self.close_palette(window, cx);
        } else if !self.ui.palette_closed.is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(400)) {
            self.open_palette(window, cx);
        }
    }

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.take().is_some() {
            self.ui.palette_closed = Some(std::time::Instant::now());
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    /// The feature the tool search has selected, for the yellow toolbar
    /// highlight ("Highlight tool in toolbar").
    pub(super) fn palette_found_tool(&self, cx: &App) -> Option<&'static str> {
        if self.palette.is_none() || self.ui.palette_no_highlight || self.palette_query.trim().is_empty() {
            return None;
        }
        match self.palette_items(cx).get(self.ui.palette_sel) {
            Some(PaletteItem::Feature(id)) => Some(id),
            _ => None,
        }
    }

    /// Onshape's tool search (live, menus/tool-search-*.png): nothing until
    /// something is typed; then the tools whose **names** contain the typed
    /// letters in order ("fil" finds Fillet, Fill, Finish flex PCB model,
    /// Finish sheet metal model, 3D fit spline, Modify fillet), ranked by
    /// where the first letter matches, then how tightly the letters sit,
    /// then catalog order. In a sketch only the sketch tools are searched,
    /// by their plain names (live qa5-ts-sketch-*: "chamfer" lists just the
    /// sketch "Chamfer", no Part Studio feature).
    fn palette_items(&self, cx: &App) -> Vec<PaletteItem> {
        use noshape_ops::tool_search::studio_rank;
        if self.sketch.is_some() {
            let tools: Vec<crate::sketch_mode::Tool> = super::sketch::all_sketch_tools().collect();
            let names: Vec<&str> = tools.iter().map(|t| sketch_search_name(*t)).collect();
            return studio_rank(&names, &self.palette_query).into_iter().map(|i| PaletteItem::SketchTool(tools[i])).collect();
        }
        // Live 10-27: an empty search lists the tools used last, newest first.
        if self.palette_query.trim().is_empty() {
            return self.ui.recent_tools.iter().map(|id| PaletteItem::Feature(id)).collect();
        }
        let custom: Vec<&'static features::FeatureType> = self.doc.read(cx).custom_features().feature_types().into_iter().collect();
        // Only tools: hidden feature types (Rip, Recognize, …) are not in
        // live's tool search either.
        let types: Vec<&'static features::FeatureType> = features::all().filter(|t| t.in_toolbar).chain(custom).collect();
        let names: Vec<&str> = types.iter().map(|t| t.name).collect();
        studio_rank(&names, &self.palette_query).into_iter().map(|i| PaletteItem::Feature(types[i].id)).collect()
    }

    fn run_palette(&mut self, item: PaletteItem, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        match item {
            PaletteItem::Feature(id) => self.start_feature(id, window, cx),
            PaletteItem::SketchTool(t) => self.activate_tool(t, window, cx),
        }
        cx.notify();
    }

    /// Onshape's tool search (live, menus/tool-search-*.png): a 600 px
    /// popover under the right end of the toolbar. A white search input
    /// (focused: 1 px `#3e94ff`), then on `#f5f5f5` the results (198 px,
    /// 34 px rows, selected `#b2ddf6`) beside the selected tool's icon, name
    /// and description, and a `#eeeeee` footer.
    pub(super) fn render_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (st, _) = self.palette.as_ref()?;
        let st = st.clone();
        let pal = p();
        let items = self.palette_items(cx);
        let sel = self.ui.palette_sel.min(items.len().saturating_sub(1));
        let info = |item: &PaletteItem| -> (SharedString, String, Option<&'static str>, String) {
            match item {
                PaletteItem::Feature(id) => {
                    let t = features::get(id);
                    (feature_icon(id), t.map(|t| t.name.to_string()).unwrap_or_default(), t.and_then(|t| t.shortcut), crate::tool_text::description(id).or(t.map(|t| t.description)).unwrap_or_default().to_string())
                }
                PaletteItem::SketchTool(t) => {
                    let name = sketch_search_name(*t);
                    (t.icon().into(), name.to_string(), sketch_tool_shortcut(*t), crate::tool_text::sketch_description(name).unwrap_or_default().to_string())
                }
            }
        };
        let chip = |t: &str| div().px(px(3.)).h(px(18.)).flex().items_center().border_1().border_color(gpui_kit::rgb(0xaaaaaa)).rounded(px(2.)).bg(gpui_kit::rgb(0xf5f5f5)).text_size(px(11.)).text_color(pal.text).child(t.to_string());
        let rows: Vec<AnyElement> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let (icon, label, shortcut, _) = info(item);
                let it = item.clone();
                h_flex()
                    .id(SharedString::from(format!("pal-{i}")))
                    .h(px(34.))
                    .flex_none()
                    .px(px(4.))
                    .gap(px(6.))
                    .rounded(px(2.))
                    .cursor_pointer()
                    .when(i == sel, |d| d.bg(pal.accent_soft))
                    .when(i != sel, |d| d.hover(|d| d.bg(pal.row_hover)))
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        if *h && this.ui.palette_sel != i {
                            this.ui.palette_sel = i;
                            cx.notify();
                        }
                    }))
                    .child(crate::assets::icon(&icon, 20., cx))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(14.)).text_color(pal.text).child(label))
                    .children(shortcut.map(|s| h_flex().children(s.split(['-', '+']).filter(|k| !k.is_empty()).map(|k| chip(&k.to_lowercase())))))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, window, cx| this.run_palette(it.clone(), window, cx)))
                    .into_any_element()
            })
            .collect();
        let detail = items.get(sel).map(|item| {
            let (icon, name, _, desc) = info(item);
            v_flex()
                .flex_1()
                .min_w_0()
                .pl(px(12.))
                .pt(px(4.))
                .gap(px(10.))
                .child(h_flex().gap(px(6.)).child(crate::assets::icon(&icon, 28., cx)).child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(name)))
                .child(div().text_size(px(14.)).line_height(px(21.)).text_color(pal.text).child(crate::ui::tool_description(&desc)))
        });
        // Live (menus/tool-search-export.png, bundle CommandSuggestions):
        // "Did you mean:" + pill buttons for the synonyms of the search (the
        // exact term while there are results, any term holding its letters
        // otherwise); a pill searches for that tool.
        let synonyms = if self.sketch.is_some() { crate::tool_synonyms::SKETCH } else { crate::tool_synonyms::PART_STUDIO };
        let suggested = crate::tool_synonyms::suggestions(synonyms, &self.palette_query, !items.is_empty());
        let suggestions = (!suggested.is_empty()).then(|| {
            h_flex()
                .px(px(14.))
                .pt(px(10.))
                // Live menus/tool-search-export.png: 24 px down to the results.
                .pb(px(24.))
                .gap(px(10.))
                .child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Did you mean:"))
                .children(suggested.into_iter().enumerate().map(|(i, tool)| {
                    let name = tool.trim();
                    div()
                        .id(SharedString::from(format!("pal-suggest-{i}")))
                        .px(px(10.))
                        .h(px(30.))
                        .flex()
                        .items_center()
                        .rounded_full()
                        .border_1()
                        .border_color(gpui_kit::rgb(0x999999))
                        .bg(gpui_kit::rgb(0xffffff))
                        .cursor_pointer()
                        .hover(|d| d.bg(gpui_kit::rgb(0xeaeaea)))
                        .text_size(px(12.))
                        .text_color(pal.text)
                        .child(name.to_string())
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            if let Some((st, _)) = &this.palette {
                                st.clone().update(cx, |s, cx| s.set_value(name, window, cx));
                            }
                            this.palette_query = name.to_string();
                            this.ui.palette_sel = 0;
                            cx.notify();
                        }))
                }))
        });
        let nothing = items.is_empty();
        let recents = self.palette_query.trim().is_empty();
        let highlight = !self.ui.palette_no_highlight;
        // Live (menus/tool-search-*): 10 px under the toolbar at the window's
        // right edge, drawn over the panels.
        let top = self.ui.palette_top.clone();
        let probe = top.clone();
        let anchor = point(window.viewport_size().width, top.get() + px(10.));
        let popover = div()
                .id("tool-search")
                .w(px(600.))
                .occlude()
                .bg(gpui_kit::rgb(0xf5f5f5))
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, window, cx| this.close_palette(window, cx)))
                .capture_key_down(cx.listener(move |this, ev: &KeyDownEvent, window, cx| {
                    let n = this.palette_items(cx).len();
                    match ev.keystroke.key.as_str() {
                        "escape" => this.close_palette(window, cx),
                        "down" if n > 0 => this.ui.palette_sel = (this.ui.palette_sel + 1).min(n - 1),
                        "up" => this.ui.palette_sel = this.ui.palette_sel.saturating_sub(1),
                        _ => return,
                    }
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(
                    div()
                        .bg(gpui_kit::rgb(0xffffff))
                        .p(px(6.))
                        // The placeholder is italic gray (live), drawn over the empty input.
                        .child(
                            div()
                                .relative()
                                .rounded(px(2.))
                                .border_1()
                                .border_color(pal.focus)
                                .child(crate::ui::input_with_placeholder(&st, "Search tools", cx).appearance(false).cleanable(true).text_size(px(14.)).pl(px(5.))),
                        ),
                )
                .when(recents && nothing, |d| {
                    d.child(div().h(px(42.)).flex().items_center().justify_center().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Start typing to search tools"))
                })
                .children(suggestions.filter(|_| !self.palette_query.trim().is_empty()))
                .when(!self.palette_query.trim().is_empty() && nothing, |d| {
                    d.child(div().h(px(42.)).flex().items_center().justify_center().text_size(px(14.)).text_color(pal.text).child("No items match your search"))
                })
                .when(!nothing, |d| d.child(
                    h_flex()
                        .items_start()
                        .px(px(10.))
                        .py(px(14.))
                        .child(
                            v_flex()
                                .w(px(198.))
                                .flex_none()
                                .gap(px(2.))
                                // Live 10-27: a bold "Recently used" centred over the rows.
                                .when(recents, |d| d.child(div().h(px(22.)).flex_none().flex().items_center().justify_center().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Recently used")))
                                .children(rows),
                        )
                        .child(div().w(px(1.)).ml(px(12.)).h(px(142.)).bg(gpui_kit::rgb(0xcccccc)))
                        .children(detail),
                ))
                .child(
                    h_flex()
                        .h(px(34.))
                        .px(px(5.))
                        .gap(px(2.))
                        .bg(gpui_kit::rgb(0xeeeeee))
                        .text_size(px(12.))
                        .text_color(pal.text_muted)
                        .child("Return to this dialog at any time by pressing")
                        .child(div().w(px(3.)))
                        .child(chip("alt/⌥"))
                        .child(chip("c"))
                        .child(div().flex_1())
                        .child(crate::ui::os_checkbox("pal-highlight", "Highlight tool in toolbar", highlight).text_size(px(12.)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.ui.palette_no_highlight = !this.ui.palette_no_highlight;
                            cx.notify();
                        })))
                        .child(div().size(px(14.)).ml(px(6.)).rounded_full().flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).child(crate::ui::glyph("help-dot", 15.6, crate::theme::help_dot(0xaaaaaa)))),
                );
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_0()
                .child(canvas(move |b, _, _| probe.set(b.origin.y), |_, _, _, _| {}).size_0())
                .child(deferred(anchored().anchor(Anchor::TopRight).position(anchor).child(popover)).with_priority(1))
                .into_any_element(),
        )
    }

    pub(super) fn render_shortcut_sheet(&mut self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.show_shortcuts {
            return None;
        }
        // Onshape (live): a white modal over a dimmed page: title +
        // "Customize keyboard shortcuts", a search box and ×; tabs General,
        // Part Studio, Assembly, 3D view, Sketch, Feature Studio, Drawing;
        // rows of right-aligned key chips and the action (🔒 = Onshape
        // controlled); a legend at the bottom. Searching lists matches of
        // every tab.
        let pal = p();
        let query = self.ui.shortcut_query.clone();
        let search = self.ui.shortcut_filter.as_ref().map(|(st, _)| st.clone());
        let tab = self.ui.shortcut_tab.min(Tab::SHEET.len() - 1);
        // Keys as chips (`span.key`); customized blue, disabled struck through.
        let keys_el = |r: &SheetRow| {
            let mut row = h_flex().gap(px(4.)).justify_end();
            if let Some(prefix) = r.prefix {
                row = row.child(div().text_size(px(14.)).text_color(pal.text).child(prefix));
            }
            for (i, group) in r.keys.iter().enumerate() {
                if i > 0 {
                    row = row.child(div().px(px(2.)).text_size(px(16.)).text_color(pal.text).child("/"));
                }
                for k in group {
                    row = row.child(crate::part_studio_hooks::key_chip(k.clone(), r.style));
                }
            }
            row
        };
        let custom = shortcuts::current();
        let rows: Vec<SheetRow> = if query.trim().is_empty() { shortcuts::sheet_rows(&custom, Tab::SHEET[tab]) } else { shortcuts::sheet_search(&custom, &query) };
        // Live (menus/616): the rows fill the tab bar's 599 px column, the
        // chips right-aligned in its first 230 px, the action 14 px after.
        // The rows' area: the sheet less its header, tabs and legend.
        let vp_h = window.viewport_size().height;
        let long = px(rows.len() as f32 * 39. + 21.) > vp_h - px(154. + 55. + 46. + 59.);
        let body = rows.into_iter().map(|r| {
            h_flex()
                .w_full()
                .h(px(39.))
                .flex_none()
                .gap(px(14.))
                .child(div().w(px(230.)).flex_none().child(keys_el(&r)))
                .child(
                    h_flex()
                        .flex_1()
                        .text_size(px(14.))
                        .text_color(pal.text)
                        .child(r.name)
                        // Live: the lock's 20 px box right after the name (4 px blank before the glyph).
                        .when(r.locked, |d| d.child(crate::assets::icon("lock", 20., cx)))
                        .when(r.conflict, |d| d.child(div().ml_1().child(crate::assets::icon("shortcut-conflict", 16., cx)))),
                )
        });
        let tabs = Tab::SHEET.iter().enumerate().map(|(i, t)| {
            let on = i == tab && query.trim().is_empty();
            div()
                .id(SharedString::from(format!("sc-tab-{i}")))
                .px(px(6.))
                .pb(px(6.))
                .cursor_pointer()
                .text_size(px(14.))
                .relative()
                .text_color(if on { pal.emphasis } else { pal.text })
                .when(on, |d| d.font_weight(FontWeight::MEDIUM))
                .child(t.label())
                // Live: the active tab's 2 px underline sits on the bar's 1 px line.
                .when(on, |d| d.child(div().absolute().left_0().right_0().bottom_0().h(px(3.)).bg(pal.emphasis)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.ui.shortcut_tab = i;
                    this.ui.shortcut_scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                }))
        });
        let chip = |style| crate::part_studio_hooks::key_chip("example", style);
        let legend = h_flex()
            .gap(px(5.))
            .px(px(8.))
            .py(px(6.))
            .border_1()
            .border_color(pal.text)
            .rounded(px(3.))
            .text_size(px(14.))
            .text_color(pal.text)
            .child(chip(KeyStyle::Customized))
            .child("Customized")
            .child(div().w(px(4.)))
            .child(chip(KeyStyle::Default))
            .child("Onshape default")
            .child(div().w(px(4.)))
            .child(chip(KeyStyle::Disabled))
            .child("Disabled")
            .child(div().w(px(4.)))
            .child(crate::assets::icon("lock", 16., cx))
            .child("Onshape controlled")
            .child(div().w(px(4.)))
            .child(crate::assets::icon("shortcut-conflict", 18., cx))
            .child("Has conflict")
            .child(div().w(px(4.)))
            .child("* applies in drawing sketches");
        // Live (menus/616): over the whole window (navbar and panels dimmed
        // too), 1000 px wide and centred, 26 px from the top, the window's
        // height less 154 px tall.
        let vp = window.viewport_size();
        let sheet = div()
                .id("shortcut-sheet")
                .occlude()
                .w(vp.width)
                .h(vp.height)
                .flex()
                .justify_center()
                .pt(px(26.))
                .bg(with_alpha(pal.overlay_shadow, 0.5))
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                    if ev.keystroke.key == "escape" {
                        this.toggle_shortcut_sheet(window, cx);
                    }
                }))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    if this.show_shortcuts {
                        this.toggle_shortcut_sheet(window, cx);
                    }
                }))
                .child(
                    v_flex()
                        .id("shortcut-panel")
                        .w(px(1000.))
                        .max_w(relative(0.95))
                        .h((vp.height - px(154.)).max(px(300.)))
                        .bg(gpui_kit::rgb(0xffffff))
                        .shadow_lg()
                        // Clicks inside the sheet (search box, tabs) keep it open.
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(
                            h_flex()
                                .flex_none()
                                .h(px(55.))
                                .px(px(10.))
                                .gap(px(6.))
                                .border_b_1()
                                .border_color(pal.border)
                                // Live (menus/616): #555 (#d7d7d7 in dark, 2446), 12 px in.
                                .child(div().ml(px(2.)).text_size(px(19.)).text_color(crate::theme::themed(0x555555, 0xd7d7d7)).child("Keyboard shortcuts"))
                                .child(
                                    div()
                                        .id("sc-customize")
                                        .text_size(px(12.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        // Live: #094174 (616), #b2ddf6 in dark (2446).
                                        .text_color(crate::theme::themed(0x094174, 0xb2ddf6))
                                        .cursor_pointer()
                                        .child("Customize keyboard shortcuts")
                                        // Onshape opens the account Preferences page (a new tab)
                                        // at Keyboard shortcuts; the sheet stays open here.
                                        .on_click(|_: &ClickEvent, _, cx| crate::part_studio_hooks::show_keyboard_shortcuts(cx)),
                                )
                                .child(div().flex_1())
                                .children(search.map(|st| div().w(px(198.)).h(px(34.)).child(Input::new(&st).h(px(34.)))))
                                // Live (menus/616): ↗ opens the page in its own window.
                                .child(
                                    div()
                                        .id("sc-popout")
                                        // Live menus/616: 21 px after the search box, 6 px above centre.
                                        .ml(px(17.))
                                        .relative()
                                        .top(px(-4.))
                                        .cursor_pointer()
                                        .child(crate::assets::icon("popout-arrow", 14., cx))
                                        .on_click(|_: &ClickEvent, window, cx| crate::part_studio_hooks::show_shortcuts_page(Some(window.bounds().size), cx)),
                                )
                                .child(
                                    div()
                                        .id("sc-close")
                                        .ml(px(-5.))
                                        .relative()
                                        .top(px(-3.))
                                        .cursor_pointer()
                                        // Live (menus/616): the modal's × 10 x 10, darkest 136, 9 px after ↗.
                                        .opacity(0.95)
                                        .child(crate::ui::glyph("close", 19., gpui_kit::rgb(0x333333)))
                                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_shortcut_sheet(window, cx))),
                                ),
                        )
                        .child(h_flex().flex_none().w(px(599.)).mx_auto().border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).gap(px(17.)).pt(px(16.)).children(tabs))
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .w(px(599.))
                                .mx_auto()
                                .mt(px(10.))
                                .relative()
                                .child(div().id("sc-rows").size_full().overflow_y_scroll()
                                    // "Input": the Part Studio's bare-key shortcuts stay quiet while the map has the
                                    // keyboard, as while typing; Shift+/ and Esc close it here.
                                    .when_some(self.ui.shortcut_list_focus.clone(), |d, f| d.track_focus(&f).key_context("ShortcutMap Input"))
                                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                                        let k = &ev.keystroke;
                                        if k.key == "escape" || k.key == "?" || (k.key == "/" && k.modifiers.shift) {
                                            cx.stop_propagation();
                                            this.toggle_shortcut_sheet(window, cx);
                                        }
                                    })).track_scroll(&self.ui.shortcut_scroll).pt(px(3.)).pb(px(8.)).pr(px(gpui_kit::component::menu::NATIVE_SCROLLBAR_WIDTH)).child(v_flex().children(body)))
                                .when(long, |d| d.child(gpui_kit::component::menu::native_scrollbar(&self.ui.shortcut_scroll, cx)))
                                .on_drag_move(cx.listener(|this, e: &DragMoveEvent<gpui_kit::component::menu::NativeThumbDrag>, window, cx| {
                                    gpui_kit::component::menu::native_scrollbar_drag(&this.ui.shortcut_scroll, e.event.position.y);
                                    window.refresh();
                                    cx.notify();
                                })),
                        )
                        .child(div().flex_none().p(px(10.)).border_t_1().border_color(pal.border).flex().justify_center().child(legend)),
                );
        // Drawn over everything in the window, from its top-left corner.
        Some(deferred(anchored().position(point(px(0.), px(0.))).child(sheet)).with_priority(8).into_any_element())
    }

    /// Shift+/ toggles the shortcut map; it opens with a search box.
    pub(super) fn toggle_shortcut_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_shortcuts = !self.show_shortcuts;
        self.ui.shortcut_query.clear();
        if self.show_shortcuts {
            self.ui.shortcut_tab = 0;
            self.ui.shortcut_scroll.set_offset(point(px(0.), px(0.)));
            let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search shortcuts"));
            let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.ui.shortcut_query = st.read(cx).value().to_string();
                    cx.notify();
                }
            });
            // Live (behaviours "qa (2026-10-27) Keyboard shortcuts modal",
            // SHORTCUT-MAP-FOCUS): the list takes the keyboard, not the
            // search box; Shift+/ and Esc close the map.
            let list = cx.focus_handle();
            window.focus(&list, cx);
            self.ui.shortcut_list_focus = Some(list);
            self.ui.shortcut_filter = Some((st, sub));
        } else {
            self.ui.shortcut_filter = None;
            self.ui.shortcut_list_focus = None;
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    pub(super) fn set_display_mode(&mut self, m: DisplayMode, window: &mut Window, cx: &mut Context<Self>) {
        self.opts.display_mode = m;
        let d = self.display.clone();
        self.apply_display(d, window, cx);
    }

    // ------------------------------------------------------------ rename

    pub(super) fn start_rename(&mut self, id: FeatureId, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.studio(cx).and_then(|s| s.feature(id)).map(|f| f.name.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let v = st.read(cx).value().to_string();
                this.renaming = None;
                // Live (qa4 r5 s8 (5)): an empty name goes back to the
                // feature's default name ("Fillet 2"); unchanged, nothing.
                let old = this.studio(cx).and_then(|s| s.feature(id)).map(|f| f.name.clone()).unwrap_or_default();
                if v.trim() != old {
                    this.doc.update(cx, |doc, cx| {
                        doc.edit_studio(this.tab, &noshape_ops::studio::rename_description(&v), cx, |ps| noshape_ops::studio::rename_feature(ps, id, &v));
                    });
                }
                cx.notify();
            }
        });
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.renaming = Some((id, st, sub));
        cx.notify();
    }
}

// ------------------------------------------------------------ named views

impl PartStudioView {
    /// Esc on a tool panel (live qa5 KB2): Named views, Mass and section
    /// properties and Measure close on the first Esc; the Appearance panel
    /// (a rail panel) stays. False when none is open.
    pub(super) fn close_tool_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.named_views.is_some() {
            self.close_named_views(window, cx);
        } else if self.show_mass {
            self.set_show_mass(false, cx);
        } else if self.ui.measure_pinned {
            self.ui.measure_pinned = false;
        } else {
            return false;
        }
        cx.notify();
        true
    }

    /// Close the Named views panel (Esc, live qa5 KB2); the typed name goes.
    fn close_named_views(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.named_views.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    pub(super) fn toggle_named_views(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.named_views.take().is_some() {
            window.focus(&self.focus, cx);
        } else {
            // Live (qa5 KB2): Esc in "New view name" closes the panel at
            // once, with no revert step (the panel's Escape below).
            let st = cx.new(|cx| InputState::new(window, cx).revert_on_escape(false));
            let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::Change) {
                    cx.notify();
                }
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    let name = st.read(cx).value().trim().to_string();
                    this.save_named_view(name, cx);
                    st.update(cx, |s, cx| s.set_value("", window, cx));
                    window.focus(&this.focus, cx);
                }
            });
            st.update(cx, |s, cx| s.focus(window, cx));
            self.named_views = Some((st, sub));
        }
        cx.notify();
    }

    fn save_named_view(&mut self, name: String, cx: &mut Context<Self>) {
        let cam = self.viewport.read(cx).camera;
        let existing: Vec<String> = self.studio(cx).map(|s| s.named_views.iter().map(|v| v.name.clone()).collect()).unwrap_or_default();
        let name = if name.is_empty() {
            (1..).map(|n| format!("View {n}")).find(|n| !existing.contains(n)).unwrap_or_default()
        } else {
            name
        };
        let view = noshape_model::partstudio::NamedView {
            name: name.clone(),
            target: cam.target.to_array(),
            rotation: cam.rotation.to_array(),
            distance: cam.distance,
            perspective: cam.projection == noshape_render::Projection::Perspective,
            section: super::section::encode(&self.ui.section),
        };
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, &format!("Save view : {name}"), cx, |ps| {
                ps.named_views.retain(|v| v.name != view.name);
                ps.named_views.push(view);
            });
        });
    }

    pub(super) fn apply_named_view(&mut self, v: &noshape_model::partstudio::NamedView, window: &mut Window, cx: &mut Context<Self>) {
        // Named views restore the section state too (Onshape).
        if !v.section.is_empty() || !self.ui.section.is_empty() {
            self.restore_section(&v.section, window, cx);
        }
        self.push_camera_history(cx);
        self.viewport.update(cx, |vp, cx| {
            let mut to = vp.camera;
            to.target = glam::DVec3::from_array(v.target);
            to.rotation = glam::DQuat::from_array(v.rotation).normalize();
            to.distance = v.distance;
            to.projection = if v.perspective { noshape_render::Projection::Perspective } else { noshape_render::Projection::Orthographic };
            vp.animate_to(to, cx);
        });
    }

    /// Onshape's Named views dialog (live part-studio/75-76,
    /// dom/panel-named-views.txt): 265 px at the feature dialog's place; a
    /// 29 px header "Named views" ×; the 220 px "New view name" field and a
    /// 30 x 31 "+" (save the current view); then "No views created", or a
    /// "Select a view" list (picking one goes there) and a 30 x 31 remove
    /// button; the help row.
    pub(super) fn render_named_views(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (st, _) = self.named_views.as_ref()?;
        let st = st.clone();
        let mut views: Vec<String> = self.studio(cx).map(|s| s.named_views.iter().map(|v| v.name.clone()).collect()).unwrap_or_default();
        // Live ("psui (2026-10-28, P1) part-studio 9.14"): the list is sorted by name, whatever the save order.
        views.sort_by_key(|v| v.to_lowercase());
        if self.named_view_picked.as_ref().is_some_and(|n| !views.contains(n)) {
            self.named_view_picked = None;
        }
        let props = crate::named_views_dialog::Props { views, picked: self.named_view_picked.clone() };
        let dialog = crate::named_views_dialog::render(props, &st, "ps-named-views", &cx.entity(), Self::on_named_views_action, cx);
        Some(div().absolute().top(px(3.)).left(px(0.)).child(dialog).into_any_element())
    }

    /// The Named views dialog's buttons and list ([`crate::named_views_dialog`]).
    fn on_named_views_action(&mut self, a: crate::named_views_dialog::Action, window: &mut Window, cx: &mut Context<Self>) {
        use crate::named_views_dialog::Action;
        match a {
            Action::Close => self.close_named_views(window, cx),
            Action::Add => {
                let Some((st, _)) = &self.named_views else { return };
                let st = st.clone();
                let name = st.read(cx).value().trim().to_string();
                self.save_named_view(name, cx);
                st.update(cx, |s, cx| s.set_value("", window, cx));
            }
            Action::Pick(name) => {
                if let Some(v) = self.studio(cx).and_then(|s| s.named_views.iter().find(|v| v.name == name).cloned()) {
                    self.apply_named_view(&v, window, cx);
                }
                self.named_view_picked = Some(name);
                cx.notify();
            }
            Action::Remove => {
                let Some(name) = self.named_view_picked.take() else { return };
                self.doc.update(cx, |doc, cx| {
                    doc.edit_studio(self.tab, &format!("Delete view : {name}"), cx, |ps| ps.named_views.retain(|v| v.name != name));
                });
                cx.notify();
            }
        }
    }
}


// A sketch tool's search name and key badge (shared with the agent API's
// ui_search_tools).
use noshape_ops::tool_search::{sketch_search_name, sketch_tool_shortcut};

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    #[test]
    fn shortcut_sheet_lists_every_part_studio_key() {
        let s = shortcuts::Shortcuts::default();
        let keys: Vec<String> = Tab::SHEET.iter().flat_map(|t| shortcuts::sheet_rows(&s, *t)).flat_map(|r| r.keys.into_iter().map(|g| g.join(" "))).collect();
        for k in ["y", "shift y", "shift i", "shift t", "w", "s", "`", "[", "shift p"] {
            assert!(keys.iter().any(|x| x == k), "{k} missing from the shortcut sheet");
        }
        assert!(shortcuts::sheet_search(&s, "isol").iter().any(|r| r.name == "Make isolate"));
        assert!(shortcuts::sheet_search(&s, "shift+i").iter().any(|r| r.name == "Make isolate"));
        assert!(!shortcuts::sheet_search(&s, "fillet").iter().any(|r| r.name == "Zoom to window"));
    }

    /// Live qa5-ts-sketch-*: a sketch tool is searched and listed under its
    /// live title (which has a live description), with its key badge.
    #[test]
    fn sketch_tools_search_by_live_titles() {
        use crate::sketch_mode::Tool;
        for t in super::super::sketch::all_sketch_tools() {
            let name = sketch_search_name(t);
            assert!(crate::tool_text::sketch_description(name).is_some(), "{name} ({}) is not a live sketch tool title", t.label());
        }
        assert_eq!((sketch_search_name(Tool::Fillet), sketch_tool_shortcut(Tool::Fillet)), ("Fillet", Some("Shift+F")));
        assert_eq!((sketch_search_name(Tool::Chamfer), sketch_tool_shortcut(Tool::Chamfer)), ("Chamfer", None));
        assert_eq!((sketch_search_name(Tool::Offset), sketch_tool_shortcut(Tool::Offset)), ("Offset", Some("O")));
        assert_eq!(sketch_search_name(Tool::LinearPattern), "Linear pattern");
        assert_eq!(sketch_search_name(Tool::Mirror), "Mirror");
    }
}
