//! Onshape's "Search Feature Studios" pane (help: Feature Studios ›
//! Searching, replacing, and renaming): a document panel under the tab bar.
//! Row 1: Toggle Replace ›/⌄, the Search mode button (T text, 𝑥 symbol,
//! {·} standard library import), the find field, Match case, Regex, the
//! Feature Studio name filter, Find, ×. Row 2 (replace): Replace with /
//! Set version to, Replace all / Rename. Then expand/collapse all, the
//! summary, and the results by Feature Studio tab; a click jumps there.

use crate::doc_model::DocModel;
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::document::TabId;
use std::collections::HashSet;
use std::ops::Range;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Text,
    Symbol,
    Import,
}

pub enum FsSearchEvent {
    /// Show this Feature Studio at a 1-based line and column.
    Goto(TabId, usize, usize),
    Close,
}

struct Hit {
    line: usize,
    column: usize,
    text: String,
    /// Byte ranges of the match within `text`.
    marks: Vec<Range<usize>>,
}

struct Group {
    tab: TabId,
    name: String,
    hits: Vec<Hit>,
}

pub struct FsSearchPane {
    doc: Entity<DocModel>,
    mode: Mode,
    find: Entity<InputState>,
    replace: Entity<InputState>,
    filter: Entity<InputState>,
    exclude: Entity<InputState>,
    match_case: bool,
    regex: bool,
    replace_open: bool,
    /// Symbol mode: the symbol and where it was taken from.
    symbol: Option<(String, String)>,
    groups: Vec<Group>,
    collapsed: HashSet<TabId>,
    summary: String,
    _subs: Vec<Subscription>,
}

impl EventEmitter<FsSearchEvent> for FsSearchPane {}

impl FsSearchPane {
    pub fn new(doc: Entity<DocModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let find = cx.new(|cx| InputState::new(window, cx).placeholder("Find text"));
        let replace = cx.new(|cx| InputState::new(window, cx).placeholder("Replace with"));
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Feature Studio name filter"));
        let exclude = cx.new(|cx| InputState::new(window, cx).placeholder("Exclude"));
        let mut subs = Vec::new();
        for st in [&find, &filter, &exclude] {
            subs.push(cx.subscribe_in(st, window, |this: &mut Self, _, ev: &InputEvent, _, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    this.run(cx);
                }
            }));
        }
        find.update(cx, |s, cx| s.focus(window, cx));
        Self { doc, mode: Mode::Text, find, replace, filter, exclude, match_case: false, regex: false, replace_open: false, symbol: None, groups: Vec::new(), collapsed: HashSet::new(), summary: String::new(), _subs: subs }
    }

    pub fn find_field(&self) -> Entity<InputState> {
        self.find.clone()
    }

    /// Symbol search for `name` (a Feature Studio's right-click › Find
    /// usages / Rename in workspace; Rename opens the replace row).
    pub fn search_symbol(&mut self, name: String, at: String, rename: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.set_mode(Mode::Symbol, window, cx);
        self.find.update(cx, |s, cx| s.set_value("", window, cx));
        self.symbol = Some((name, at));
        if rename {
            self.replace_open = true;
            self.replace.update(cx, |s, cx| s.focus(window, cx));
        }
        self.run(cx);
    }

    fn set_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = mode;
        let (find_ph, replace_ph, filter_ph) = match mode {
            Mode::Text => ("Find text", "Replace with", "Feature Studio name filter"),
            Mode::Symbol => ("Find symbol", "Replace with", "Feature Studio name filter"),
            Mode::Import => ("Include", "Set version to", "Feature Studio name filter"),
        };
        self.find.update(cx, |s, cx| s.set_placeholder(find_ph, window, cx));
        self.replace.update(cx, |s, cx| s.set_placeholder(replace_ph, window, cx));
        self.filter.update(cx, |s, cx| s.set_placeholder(filter_ph, window, cx));
        self.clear(cx);
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        self.groups.clear();
        self.summary.clear();
        self.publish(cx);
        cx.notify();
    }

    /// The pattern to search for (None: nothing to search).
    fn pattern(&self, cx: &App) -> Option<Result<regex::Regex, String>> {
        let (src, literal, word) = match self.mode {
            Mode::Text => (self.find.read(cx).value().to_string(), !self.regex, false),
            Mode::Symbol => {
                let typed = self.find.read(cx).value().trim().to_string();
                let name = if typed.is_empty() { self.symbol.as_ref()?.0.clone() } else { typed };
                (name, true, true)
            }
            // Standard library imports: `import(path : "onshape/std/…", version : "…")`.
            Mode::Import => (r#"import\s*\(\s*path\s*:\s*"onshape/std/[^"]*"\s*,\s*version\s*:\s*"[^"]*"\s*\)"#.to_string(), false, false),
        };
        if src.is_empty() {
            return None;
        }
        let mut pat = if literal { regex::escape(&src) } else { src };
        if word {
            pat = format!(r"\b{pat}\b");
        }
        let case = self.match_case || self.mode == Mode::Symbol;
        Some(regex::RegexBuilder::new(&pat).case_insensitive(!case).build().map_err(|e| e.to_string()))
    }

    /// The symbol searched for in Symbol mode (typed, else the one picked).
    fn symbol_name(&self, cx: &App) -> Option<String> {
        let typed = self.find.read(cx).value().trim().to_string();
        if typed.is_empty() { self.symbol.as_ref().map(|s| s.0.clone()) } else { Some(typed) }
    }

    /// Feature Studio name filter: `*` matches anything.
    fn name_matches(filter: &str, name: &str) -> bool {
        if filter.trim().is_empty() {
            return true;
        }
        let pat = format!("^{}$", regex::escape(filter.trim()).replace(r"\*", ".*"));
        regex::RegexBuilder::new(&pat).case_insensitive(true).build().map(|r| r.is_match(name)).unwrap_or(true)
    }

    fn studios(&self, cx: &App) -> Vec<(TabId, String, String)> {
        let filter = self.filter.read(cx).value().to_string();
        let doc = &self.doc.read(cx).session.doc;
        doc.ordered_tabs()
            .iter()
            .filter_map(|t| match &t.content {
                TabContent::FeatureStudio(f) if Self::name_matches(&filter, &t.name) => Some((t.id, t.name.clone(), f.text().to_string())),
                _ => None,
            })
            .collect()
    }

    /// Find: search every Feature Studio matching the name filter.
    fn run(&mut self, cx: &mut Context<Self>) {
        self.groups.clear();
        let Some(pat) = self.pattern(cx) else {
            self.summary.clear();
            self.publish(cx);
            cx.notify();
            return;
        };
        let re = match pat {
            Ok(r) => r,
            Err(e) => {
                self.summary = format!("Invalid regular expression: {}", e.lines().last().unwrap_or_default());
                self.publish(cx);
                cx.notify();
                return;
            }
        };
        // Symbol mode finds references (identifier tokens), not text in
        // strings, comments or field names.
        let symbol = (self.mode == Mode::Symbol).then(|| self.symbol_name(cx)).flatten();
        let mut total = 0;
        for (tab, name, text) in self.studios(cx) {
            let mut hits = Vec::new();
            let refs = symbol.as_ref().map(|s| noshape_model::custom::symbol_occurrences(&text, s));
            let mut line_start = 0;
            for (i, line) in text.split('\n').enumerate() {
                let marks: Vec<Range<usize>> = match &refs {
                    Some(refs) => refs.iter().filter(|r| r.start >= line_start && r.end <= line_start + line.len()).map(|r| r.start - line_start..r.end - line_start).collect(),
                    None => re.find_iter(line).filter(|m| !m.range().is_empty()).map(|m| m.range()).collect(),
                };
                line_start += line.len() + 1;
                if let Some(first) = marks.first() {
                    total += marks.len();
                    hits.push(Hit { line: i + 1, column: line[..first.start].chars().count() + 1, text: line.to_string(), marks });
                }
            }
            if !hits.is_empty() {
                self.groups.push(Group { tab, name, hits });
            }
        }
        let what = match self.mode {
            Mode::Text => format!("'{}'", self.find.read(cx).value()),
            Mode::Symbol => format!("'{}'", self.symbol.as_ref().map(|s| s.0.clone()).filter(|_| self.find.read(cx).value().trim().is_empty()).unwrap_or_else(|| self.find.read(cx).value().trim().to_string())),
            Mode::Import => "standard library imports".to_string(),
        };
        let tabs = self.groups.len();
        let filter = self.filter.read(cx).value().trim().to_string();
        // Onshape writes "tabs" even for one ("found in all 1 tabs").
        let scope = if filter.is_empty() { format!("in all {tabs} tabs") } else { format!("in {tabs} tabs matching '{filter}'") };
        self.summary = if total == 0 { format!("No occurrences of {what} found") } else { format!("All {total} occurrence{} of {what} found {scope}", if total == 1 { "" } else { "s" }) };
        self.publish(cx);
        cx.notify();
    }

    /// Highlight the matches in the Feature Studios' code.
    fn publish(&self, cx: &mut App) {
        let re = self.pattern(cx).and_then(|r| r.ok());
        let symbol = (self.mode == Mode::Symbol).then(|| self.symbol_name(cx)).flatten();
        for (tab, _, text) in self.studios(cx) {
            let ranges = match &symbol {
                Some(s) => noshape_model::custom::symbol_occurrences(&text, s),
                None => re.as_ref().map(|re| re.find_iter(&text).filter(|m| !m.range().is_empty()).map(|m| m.range()).collect()).unwrap_or_default(),
            };
            crate::feature_studio::set_matches(tab, ranges);
        }
        cx.refresh_windows();
    }

    /// Replace all / Rename: rewrite every match in the listed Feature Studios.
    fn replace_all(&mut self, cx: &mut Context<Self>) {
        let Some(Ok(re)) = self.pattern(cx) else { return };
        let with = self.replace.read(cx).value().to_string();
        let symbol = (self.mode == Mode::Symbol).then(|| self.symbol_name(cx)).flatten();
        if self.mode == Mode::Import && with.trim().is_empty() {
            return;
        }
        let edits: Vec<(TabId, String)> = self
            .groups
            .iter()
            .filter_map(|g| {
                let doc = &self.doc.read(cx).session.doc;
                let TabContent::FeatureStudio(f) = &doc.tab(g.tab)?.content else { return None };
                let text = f.text();
                let new = match &symbol {
                    // Rename: replace the references only, back to front.
                    Some(sym) => {
                        let mut t = text.to_string();
                        for r in noshape_model::custom::symbol_occurrences(text, sym).into_iter().rev() {
                            t.replace_range(r, with.trim());
                        }
                        t
                    }
                    None if self.mode == Mode::Text && self.regex => re.replace_all(text, with.as_str()).into_owned(),
                    None => re.replace_all(text, regex::NoExpand(&with)).into_owned(),
                };
                (new != text).then_some((g.tab, new))
            })
            .collect();
        let label = if self.mode == Mode::Symbol { "Rename in workspace" } else { "Replace all" };
        self.doc.update(cx, |doc, cx| {
            for (tab, text) in edits {
                doc.edit_tab(tab, label, cx, |c| {
                    if let TabContent::FeatureStudio(f) = c {
                        f.save(text);
                    }
                });
            }
        });
        if self.mode == Mode::Symbol {
            let new = self.replace.read(cx).value().trim().to_string();
            if let Some(s) = self.symbol.as_mut() {
                s.0 = new;
            }
        }
        self.run(cx);
    }

    fn open_mode_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let mode = self.mode;
        let (v1, v2, v3) = (cx.entity(), cx.entity(), cx.entity());
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
            use crate::os_menu::OsMenuItem as I;
            let icon = |g: &'static [u8]| os::glyph(g, 16., p().text);
            m.select_list(260.)
                .item(I::new("Search for text").icon(icon(os::GLYPH_TEXT_MODE)).checked(mode == Mode::Text).on_click(move |_, w, cx| v1.update(cx, |t, cx| t.set_mode(Mode::Text, w, cx))))
                .item(I::new("Search for symbol").icon(icon(os::GLYPH_SYMBOL_MODE)).checked(mode == Mode::Symbol).on_click(move |_, w, cx| v2.update(cx, |t, cx| t.set_mode(Mode::Symbol, w, cx))))
                .item(I::new("Search for standard library import").icon(icon(os::GLYPH_IMPORT_MODE)).checked(mode == Mode::Import).on_click(move |_, w, cx| v3.update(cx, |t, cx| t.set_mode(Mode::Import, w, cx))))
        });
        crate::os_menu::open_anchored(at, Anchor::TopLeft, menu, window, cx);
    }

    fn render_results(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let mut out = Vec::new();
        for (gi, g) in self.groups.iter().enumerate() {
            let tab = g.tab;
            let open = !self.collapsed.contains(&tab);
            out.push(
                h_flex()
                    .id(SharedString::from(format!("fss-g{gi}")))
                    .h(px(28.))
                    .px(px(8.))
                    .gap(px(6.))
                    .flex_none()
                    .cursor_pointer()
                    .border_t_1()
                    .border_color(rgb(0xeeeeee))
                    .child(os::glyph(if open { os::GLYPH_CHEVRON_DOWN } else { os::GLYPH_CHEVRON_RIGHT }, 14., pal.text))
                    .child(crate::assets::icon("feature-studio", 18., cx))
                    .child(div().text_size(px(14.)).text_color(pal.text).child(crate::ui::user_text(g.name.clone())))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !this.collapsed.remove(&tab) {
                            this.collapsed.insert(tab);
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
            if !open {
                continue;
            }
            for (hi, h) in g.hits.iter().enumerate() {
                let (line, column) = (h.line, h.column);
                // The line text, left-trimmed, with the matches in bold.
                let trim = h.text.len() - h.text.trim_start().len();
                let mut parts = h_flex().min_w_0().overflow_hidden().whitespace_nowrap();
                let mut at = trim;
                for m in &h.marks {
                    let s = m.start.max(trim);
                    if s > at {
                        parts = parts.child(h.text[at..s].to_string());
                    }
                    if m.end > s {
                        parts = parts.child(div().font_weight(FontWeight::BOLD).child(h.text[s..m.end].to_string()));
                    }
                    at = m.end.max(at);
                }
                if at < h.text.len() {
                    parts = parts.child(h.text[at..].to_string());
                }
                out.push(
                    h_flex()
                        .id(SharedString::from(format!("fss-{gi}-{hi}")))
                        .h(px(24.))
                        .flex_none()
                        .cursor_pointer()
                        .text_size(px(14.))
                        .text_color(pal.text)
                        .hover(|d| d.bg(rgb(0xdef1fb)))
                        .child(div().w(px(80.)).flex_none().pr(px(16.)).flex().justify_end().text_color(pal.text_muted).child(line.to_string()))
                        .child(parts)
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(FsSearchEvent::Goto(tab, line, column))))
                        .into_any_element(),
                );
            }
        }
        out
    }
}

impl Render for FsSearchPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let primary = |id: &'static str, label: &'static str| Button::new(id).primary().h(px(30.)).px(px(10.5)).rounded(px(5.25)).child(div().text_size(px(14.)).child(label));
        let field = |st: &Entity<InputState>| crate::os_ui::input_h(Input::new(st), 30.).text_size(px(14.));
        let check = |id: &'static str, label: &'static str, on: bool| os::checkbox(id, label, on).flex_none().cursor_pointer();
        let mode_anchor: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
        let probe = mode_anchor.clone();
        let mode_glyph = match self.mode {
            Mode::Text => os::GLYPH_TEXT_MODE,
            Mode::Symbol => os::GLYPH_SYMBOL_MODE,
            Mode::Import => os::GLYPH_IMPORT_MODE,
        };
        let mode_btn = h_flex()
            .id("fss-mode")
            .relative()
            .flex_none()
            .h(px(30.))
            .px(px(8.))
            .gap(px(4.))
            .rounded(px(5.25))
            .border_1()
            .border_color(rgb(0xaaaaaa))
            .cursor_pointer()
            .child(os::glyph(mode_glyph, 20., pal.text))
            .child(os::glyph(os::GLYPH_CARET, 8., pal.text))
            .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                let b = mode_anchor.get();
                this.open_mode_menu(point(b.left(), b.bottom() + px(2.)), window, cx);
            }));
        let find_cell: AnyElement = match (&self.symbol, self.mode) {
            (Some((name, at)), Mode::Symbol) if self.find.read(cx).value().is_empty() => div()
                .flex_1()
                .h(px(30.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(5.25))
                .bg(rgb(0xeeeeee))
                .text_size(px(14.))
                .text_color(pal.text)
                .child(format!("Search for symbol \"{name}\" at {at}"))
                .into_any_element(),
            _ => div().flex_1().child(field(&self.find)).into_any_element(),
        };
        let row1 = h_flex()
            .h(px(42.))
            .px(px(8.))
            .gap(px(8.))
            .child(
                div()
                    .id("fss-toggle")
                    .w(px(24.))
                    .flex_none()
                    .cursor_pointer()
                    .child(os::glyph(if self.replace_open { os::GLYPH_CHEVRON_DOWN } else { os::GLYPH_CHEVRON_RIGHT }, 18., pal.text))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.replace_open = !this.replace_open;
                        cx.notify();
                    })),
            )
            .child(mode_btn)
            .child(find_cell)
            .when(self.mode == Mode::Text, |d| {
                d.child(check("fss-case", "Match case", self.match_case).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.match_case = !this.match_case;
                    this.run(cx);
                })))
                .child(check("fss-regex", "Regex", self.regex).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.regex = !this.regex;
                    this.run(cx);
                })))
            })
            .when(self.mode == Mode::Import, |d| d.child(div().w(px(200.)).flex_none().child(field(&self.exclude))))
            .child(div().w(px(340.)).flex_none().child(field(&self.filter)))
            .child(primary("fss-find", "Find").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(cx))))
            .child(
                div()
                    .id("fss-close")
                    .flex_none()
                    .cursor_pointer()
                    .child(os::glyph(os::GLYPH_CLOSE, 20., pal.text_muted))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.groups.clear();
                        this.publish(cx);
                        cx.emit(FsSearchEvent::Close);
                    })),
            );
        let row2 = self.replace_open.then(|| {
            let label = match self.mode {
                Mode::Symbol => "Rename",
                _ => "Replace all",
            };
            let empty = self.groups.is_empty();
            h_flex()
                .h(px(40.))
                .pl(px(40.))
                .pr(px(8.))
                .gap(px(8.))
                .child(div().flex_1().child(field(&self.replace)))
                .child(primary("fss-replace", label).opacity(if empty { 0.4 } else { 1. }).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if !empty {
                        this.replace_all(cx);
                    }
                })))
        });
        let summary = (!self.summary.is_empty()).then(|| {
            h_flex()
                .h(px(28.))
                .px(px(8.))
                .gap(px(6.))
                .child(div().id("fss-expand").cursor_pointer().child(os::glyph(os::GLYPH_EXPAND_ALL, 18., pal.text)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.collapsed.clear();
                    cx.notify();
                })))
                .child(div().id("fss-collapse").cursor_pointer().child(os::glyph(os::GLYPH_COLLAPSE_ALL, 18., pal.text)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.collapsed = this.groups.iter().map(|g| g.tab).collect();
                    cx.notify();
                })))
                .child(div().text_size(px(14.)).text_color(pal.text).child(self.summary.clone()))
        });
        let results = self.render_results(cx);
        v_flex()
            .h(px(300.))
            .flex_none()
            .bg(rgb(0xffffff))
            .border_t_1()
            .border_color(rgb(0xd7d7d7))
            .child(row1)
            .children(row2)
            .children(summary)
            .child(v_flex().id("fss-results").flex_1().min_h_0().overflow_y_scroll().children(results))
    }
}
