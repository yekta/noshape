//! Part Studio view tools: tool search palette (Alt+C), keyboard shortcut
//! sheet (Shift+?), display modes, section view (Shift+X) and feature rename.

use super::PartStudioView;
use super::feature_icon;
use crate::scene_builder::DisplayMode;
use crate::theme::{p, with_alpha};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
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
    pub(crate) fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
    pub(crate) fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
    /// then catalog order. In a sketch, its tools are searched too.
    fn palette_items(&self, cx: &App) -> Vec<PaletteItem> {
        let q: Vec<char> = self.palette_query.trim().to_lowercase().chars().collect();
        if q.is_empty() {
            return Vec::new();
        }
        // (first matched index, span of the match) for the in-order match of `q` in `name`.
        let score = |name: &str| -> Option<(usize, usize)> {
            let chars: Vec<char> = name.to_lowercase().chars().collect();
            let mut best: Option<(usize, usize)> = None;
            for start in 0..chars.len() {
                if chars[start] != q[0] {
                    continue;
                }
                let mut k = 1;
                let mut end = start;
                for (i, c) in chars.iter().enumerate().skip(start + 1) {
                    if k == q.len() {
                        break;
                    }
                    if *c == q[k] {
                        k += 1;
                        end = i;
                    }
                }
                if k == q.len() {
                    let cand = (start, end - start);
                    if best.is_none_or(|b| cand < b) {
                        best = Some(cand);
                    }
                }
            }
            best
        };
        let mut found: Vec<((usize, usize, usize), PaletteItem)> = Vec::new();
        let mut order = 0usize;
        if self.sketch.is_some() {
            for t in super::sketch::all_sketch_tools() {
                order += 1;
                if let Some((f, span)) = score(sketch_tool_name(t)) {
                    found.push(((f, span, order), PaletteItem::SketchTool(t)));
                }
            }
        }
        let custom: Vec<&'static features::FeatureType> = self.doc.read(cx).custom_features().feature_types().into_iter().collect();
        // Only tools: hidden feature types (Rip, Recognize, …) are not in
        // live's tool search either.
        for t in features::all().filter(|t| t.in_toolbar).chain(custom) {
            order += 1;
            if let Some((f, span)) = score(t.name) {
                found.push(((f, span, order), PaletteItem::Feature(t.id)));
            }
        }
        found.sort_by_key(|(k, _)| *k);
        found.into_iter().map(|(_, i)| i).collect()
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
                PaletteItem::SketchTool(t) => (t.icon().into(), sketch_tool_name(*t).to_string(), None, String::new()),
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
                .child(div().text_size(px(14.)).line_height(px(21.)).text_color(pal.text).child(desc))
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
                .when(self.palette_query.trim().is_empty(), |d| {
                    d.child(div().h(px(42.)).flex().items_center().justify_center().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Start typing to search tools"))
                })
                .children(suggestions.filter(|_| !self.palette_query.trim().is_empty()))
                .when(!self.palette_query.trim().is_empty() && nothing, |d| {
                    d.child(div().h(px(42.)).flex().items_center().justify_center().text_size(px(14.)).text_color(pal.text).child("No items match your search"))
                })
                .when(!self.palette_query.trim().is_empty() && !nothing, |d| d.child(
                    h_flex()
                        .items_start()
                        .px(px(10.))
                        .py(px(14.))
                        .child(v_flex().w(px(198.)).flex_none().gap(px(2.)).children(rows))
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
                        .child(div().size(px(14.)).ml(px(6.)).rounded_full().bg(gpui_kit::rgb(0xaaaaaa)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0xffffff)).child("?")),
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

    pub(super) fn render_shortcut_sheet(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
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
                    row = row.child(crate::preferences_window::key_chip(k.clone(), r.style));
                }
            }
            row
        };
        let custom = shortcuts::current();
        let rows: Vec<SheetRow> = if query.trim().is_empty() { shortcuts::sheet_rows(&custom, Tab::SHEET[tab]) } else { shortcuts::sheet_search(&custom, &query) };
        let body = rows.into_iter().map(|r| {
            h_flex()
                .w(px(560.))
                .h(px(39.))
                .flex_none()
                .gap(px(14.))
                .child(div().w(px(273.)).flex_none().child(keys_el(&r)))
                .child(
                    h_flex()
                        .flex_1()
                        .gap_1()
                        .text_size(px(14.))
                        .text_color(pal.text)
                        .child(r.name)
                        .when(r.locked, |d| d.child(crate::assets::icon("lock", 14., cx)))
                        .when(r.conflict, |d| d.child(crate::assets::icon("shortcut-conflict", 16., cx))),
                )
        });
        let tabs = Tab::SHEET.iter().enumerate().map(|(i, t)| {
            let on = i == tab && query.trim().is_empty();
            div()
                .id(SharedString::from(format!("sc-tab-{i}")))
                .px(px(6.))
                .pb(px(4.))
                .cursor_pointer()
                .text_size(px(14.))
                .text_color(if on { pal.accent } else { pal.text })
                .when(on, |d| d.border_b_2().border_color(pal.accent))
                .child(t.label())
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.ui.shortcut_tab = i;
                    cx.notify();
                }))
        });
        let chip = |style| crate::preferences_window::key_chip("example", style);
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
            .child(div().w(px(12.)))
            .child(chip(KeyStyle::Default))
            .child("Onshape default")
            .child(div().w(px(12.)))
            .child(chip(KeyStyle::Disabled))
            .child("Disabled")
            .child(div().w(px(12.)))
            .child(crate::assets::icon("lock", 16., cx))
            .child("Onshape controlled")
            .child(div().w(px(12.)))
            .child(crate::assets::icon("shortcut-conflict", 18., cx))
            .child("Has conflict")
            .child(div().w(px(12.)))
            .child("* applies in drawing sketches");
        Some(
            div()
                .id("shortcut-sheet")
                .occlude()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
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
                        .h(relative(0.92))
                        .max_h(px(928.))
                        .bg(gpui_kit::rgb(0xffffff))
                        .shadow_lg()
                        // Clicks inside the sheet (search box, tabs) keep it open.
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(
                            h_flex()
                                .flex_none()
                                .h(px(54.))
                                .px(px(10.))
                                .gap(px(6.))
                                .border_b_1()
                                .border_color(pal.border)
                                .child(div().text_size(px(19.)).text_color(pal.text).child("Keyboard shortcuts"))
                                .child(
                                    div()
                                        .id("sc-customize")
                                        .text_size(px(12.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(pal.accent)
                                        .cursor_pointer()
                                        .child("Customize keyboard shortcuts")
                                        // Onshape opens the account Preferences page (a new tab)
                                        // at Keyboard shortcuts; the sheet stays open here.
                                        .on_click(|_: &ClickEvent, _, cx| crate::preferences_window::show(crate::preferences_window::Page::KeyboardShortcuts, cx)),
                                )
                                .child(div().flex_1())
                                .children(search.map(|st| div().w(px(198.)).child(Input::new(&st).small())))
                                .child(
                                    div()
                                        .id("sc-close")
                                        .px_1()
                                        .cursor_pointer()
                                        // Live (shortcuts/01): the modal's × (10 px, #333 at .5).
                                        .opacity(0.86)
                                        .child(crate::ui::glyph("close", 21., gpui_kit::rgb(0x333333)))
                                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_shortcut_sheet(window, cx))),
                                ),
                        )
                        .child(h_flex().flex_none().justify_center().gap(px(20.)).pt(px(16.)).pb(px(6.)).children(tabs))
                        .child(div().id("sc-rows").flex_1().min_h_0().overflow_y_scroll().py(px(8.)).child(v_flex().items_center().children(body)))
                        .child(div().flex_none().p(px(10.)).border_t_1().border_color(pal.border).flex().justify_center().child(legend)),
                )
                .into_any_element(),
        )
    }

    /// Shift+/ toggles the shortcut map; it opens with a search box.
    pub(super) fn toggle_shortcut_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_shortcuts = !self.show_shortcuts;
        self.ui.shortcut_query.clear();
        if self.show_shortcuts {
            self.ui.shortcut_tab = 0;
            let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search shortcuts"));
            let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.ui.shortcut_query = st.read(cx).value().to_string();
                    cx.notify();
                }
            });
            // Typing goes to the search box right away (and bare-letter
            // shortcuts stay quiet while the map is open).
            st.update(cx, |s, cx| s.focus(window, cx));
            self.ui.shortcut_filter = Some((st, sub));
        } else {
            self.ui.shortcut_filter = None;
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
                if !v.trim().is_empty() {
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
        let pal = p();
        let views = self.studio(cx).map(|s| s.named_views.clone()).unwrap_or_default();
        if self.named_view_picked.as_ref().is_some_and(|n| !views.iter().any(|v| &v.name == n)) {
            self.named_view_picked = None;
        }
        let select = match &self.named_view_select {
            Some((s, _)) => s.clone(),
            None => {
                let s = cx.new(|_| crate::os_select::OsSelect::new("named-view-select"));
                let sub = cx.subscribe(&s, |this: &mut Self, _, ev: &crate::os_select::OsSelectEvent, cx| {
                    let crate::os_select::OsSelectEvent::Picked(name) = ev;
                    this.named_view_picked = (!name.is_empty()).then(|| name.to_string());
                    // Applied on the next render (it needs the window).
                    this.pending_named_view = this.studio(cx).and_then(|s| s.named_views.iter().find(|v| v.name.as_str() == name.as_ref()).cloned());
                    cx.notify();
                });
                self.named_view_select = Some((s.clone(), sub));
                s
            }
        };
        // Live: the list reads "Select a view" until one is picked.
        let items: Vec<(SharedString, SharedString)> = std::iter::once((SharedString::default(), SharedString::from("Select a view"))).chain(views.iter().map(|v| (SharedString::from(v.name.clone()), SharedString::from(v.name.clone())))).collect();
        let current = self.named_view_picked.clone().unwrap_or_default();
        select.update(cx, |s, _| s.set_items(items, current));
        let empty_name = st.read(cx).value().is_empty();
        let square = |id: &'static str, icon: &'static str, color: Rgba| {
            div()
                .id(id)
                .size(px(30.))
                .h(px(31.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui_kit::rgb(0xffffff))
                .border_1()
                .border_color(gpui_kit::rgb(0xaaaaaa))
                .rounded(px(3.5))
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(crate::ui::glyph(icon, 20., color))
        };
        // Live: "+" is pale until a name is typed.
        let add_color = if empty_name { gpui_kit::rgb(0xcccccc) } else { gpui_kit::rgb(0x999999) };
        let st2 = st.clone();
        let shadow = vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(5.), spread_radius: px(0.), inset: false }];
        Some(
            v_flex()
                .id("named-views")
                .occlude()
                .on_action(cx.listener(|this, _: &gpui_kit::component::input::Escape, window, cx| this.close_named_views(window, cx)))
                .child(crate::ui::panel_probe("ps-named-views"))
                .absolute()
                .top(px(3.))
                .left(px(0.))
                .w(px(265.))
                .bg(with_alpha(gpui_kit::rgb(0xffffff), 0.95))
                .shadow(shadow)
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(29.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd7d7d7))
                        .child(crate::ui::panel_title("ps-named-views", div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Named views")))
                        .child(
                            div()
                                .id("nv-close")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .opacity(0.5)
                                .child(crate::ui::glyph("close", 16., gpui_kit::rgb(0x000000)))
                                .on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.toggle_named_views(w, cx))),
                        ),
                )
                .child(
                    h_flex()
                        .h(px(40.))
                        .px(px(5.))
                        .gap(px(5.))
                        // Live: a focused 2 px blue underline (1 px + inset),
                        // italic placeholder, text 6 px down.
                        .child(
                            div()
                                .relative()
                                .w(px(220.))
                                .h(px(27.))
                                .pt(px(3.))
                                .border_b_1()
                                .border_color(pal.accent)
                                .child(crate::ui::input_with_placeholder(&st, "New view name", cx).appearance(false).small().text_size(px(13.33)).pl(px(3.)))
                                .child(div().absolute().left_0().right_0().bottom_0().h(px(1.)).bg(pal.accent)),
                        )
                        .child(square("nv-add", "add-button", add_color).tooltip(crate::ui::tooltip("Add view")).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            let name = st2.read(cx).value().trim().to_string();
                            this.save_named_view(name, cx);
                            st2.update(cx, |s, cx| s.set_value("", window, cx));
                        }))),
                )
                .when(views.is_empty(), |d| d.child(div().px(px(5.)).h(px(22.)).flex().items_center().text_size(px(12.)).child("No views created")))
                .when(!views.is_empty(), |d| {
                    d.child(
                        h_flex()
                            .h(px(40.))
                            .px(px(5.))
                            .gap(px(5.))
                            .child(div().w(px(220.)).child(select.clone()))
                            .child(square("nv-remove", "remove-circle", gpui_kit::rgb(0x999999)).tooltip(crate::ui::tooltip("Remove view")).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                let Some(name) = this.named_view_picked.take() else { return };
                                this.doc.update(cx, |doc, cx| {
                                    doc.edit_studio(this.tab, &format!("Delete view : {name}"), cx, |ps| ps.named_views.retain(|v| v.name != name));
                                });
                                cx.notify();
                            }))),
                    )
                })
                .child(h_flex().h(px(22.)).justify_end().pr(px(4.)).child(div().size(px(14.)).rounded_full().bg(gpui_kit::rgb(0xaaaaaa)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0xffffff)).child("?")))
                .into_any_element(),
        )
    }
}


/// A sketch tool's name without its shortcut ("Line (L)" -> "Line").
fn sketch_tool_name(t: crate::sketch_mode::Tool) -> &'static str {
    let l = t.label();
    l.rsplit_once(" (").map_or(l, |(name, _)| name)
}

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
}
