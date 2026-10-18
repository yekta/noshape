//! Feature Studio tab: a script editor for the document's custom features
//! (noshape's FeatureScript equivalent). As in Onshape, edits are saved as
//! you type but only Commit (ctrl+s; Commit all: shift+ctrl+s) applies them
//! to the document's Part Studios. Toolbar: undo/redo, the Commit split
//! button, Search tools…; above the code: Module outline ▾, back/forward
//! and the Search Feature Studios button (the search pane is the
//! workspace's, under the tab bar: `fs_search.rs`).

use crate::doc_model::{DocEvent, DocModel};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::input::{Editor, EditorState, HighlightStyleResolver, Input, InputEvent, InputHighlighter, InputState, Position, TabSize};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::custom::{CompiledStudio, Severity};
use noshape_model::document::TabId;
use std::ops::Range;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

actions!(feature_studio, [Commit, CommitAll, SearchStudios]);

/// Onshape's Feature Studio shortcuts (live dom/keyboard-shortcuts.txt).
pub fn bind_keys(cx: &mut App) {
    let c = Some("FeatureStudio");
    cx.bind_keys([
        KeyBinding::new("secondary-s", Commit, c),
        KeyBinding::new("secondary-shift-s", CommitAll, c),
        KeyBinding::new("secondary-shift-f", SearchStudios, c),
    ]);
}

/// Pause after the last keystroke before the text is saved (not committed).
const SAVE_DELAY: Duration = Duration::from_millis(700);

/// Requests for the workspace.
pub enum FeatureStudioEvent {
    /// Open (or focus) the Search Feature Studios pane.
    OpenSearch,
}

/// Search matches to highlight in each Feature Studio (set by the search
/// pane; read by the highlighter).
static MATCHES: Mutex<Vec<(TabId, Vec<Range<usize>>)>> = Mutex::new(Vec::new());

pub fn set_matches(tab: TabId, ranges: Vec<Range<usize>>) {
    let mut m = MATCHES.lock().unwrap_or_else(|e| e.into_inner());
    m.retain(|(t, _)| *t != tab);
    if !ranges.is_empty() {
        m.push((tab, ranges));
    }
}

fn matches_of(tab: TabId) -> Vec<Range<usize>> {
    MATCHES.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(t, _)| *t == tab).map(|(_, r)| r.clone()).unwrap_or_default()
}

/// A function in the Module outline: name, parameter list, 1-based line.
#[derive(Clone)]
struct Symbol {
    name: String,
    params: String,
    line: usize,
}

fn outline(src: &str) -> Vec<Symbol> {
    src.lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let t = l.trim_start();
            let t = t.strip_prefix("private ").unwrap_or(t);
            let rest = t.strip_prefix("fn ")?;
            let open = rest.find('(')?;
            let close = rest[open..].find(')').map(|c| open + c).unwrap_or(rest.len());
            Some(Symbol { name: rest[..open].trim().to_string(), params: rest[open + 1..close].trim().to_string(), line: i + 1 })
        })
        .collect()
}

pub struct FeatureStudioView {
    tab: TabId,
    doc: Entity<DocModel>,
    editor: Entity<EditorState>,
    /// Text as last saved to / loaded from the document.
    synced: String,
    compiled: Option<Arc<CompiledStudio>>,
    compiling: bool,
    save_task: Option<Task<()>>,
    compile_task: Option<Task<()>>,
    /// Module outline popover: its "Search symbols…" field.
    outline: Option<Entity<InputState>>,
    /// Cursor positions visited by jumps (back / forward).
    nav: Vec<Position>,
    nav_at: usize,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl EventEmitter<FeatureStudioEvent> for FeatureStudioView {}

impl Focusable for FeatureStudioView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl FeatureStudioView {
    pub fn new(doc: Entity<DocModel>, tab: TabId, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let source = text_of(&doc, tab, cx).unwrap_or_default();
        let editor = cx.new(|cx| {
            let mut st = EditorState::new(window, cx)
                .language("rhai")
                .line_number(true)
                .soft_wrap(false)
                .tab_size(TabSize { tab_size: 4, hard_tabs: false })
                .default_value(source.clone());
            st.set_highlighter_factory(Rc::new(move |_| Some(Box::new(RhaiHighlighter { tab, ..Default::default() }) as Box<dyn InputHighlighter>)), cx);
            st
        });
        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(&editor, window, |this: &mut Self, _, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::Change) {
                this.schedule_save(window, cx);
            } else if matches!(ev, InputEvent::Blur) {
                this.save(cx);
            }
        }));
        subs.push(cx.subscribe_in(&doc, window, |this: &mut Self, _, ev: &DocEvent, window, cx| match ev {
            DocEvent::TabChanged(t) if *t == this.tab => this.sync_from_doc(window, cx),
            DocEvent::Reloaded | DocEvent::TabsChanged => this.sync_from_doc(window, cx),
            _ => {}
        }));
        let mut this = Self { tab, doc, editor, synced: source.clone(), compiled: None, compiling: false, save_task: None, compile_task: None, outline: None, nav: Vec::new(), nav_at: 0, focus: cx.focus_handle(), _subs: subs };
        this.recompile(source, cx);
        this
    }

    pub fn editor_focus(&self, cx: &App) -> FocusHandle {
        self.editor.read(cx).focus_handle(cx)
    }

    pub fn tab(&self) -> TabId {
        self.tab
    }

    /// Repaint the code (search highlights changed).
    pub fn refresh_highlights(&self, cx: &mut App) {
        self.editor.update(cx, |_, cx| cx.notify());
    }

    fn read_only(&self, cx: &App) -> bool {
        self.doc.read(cx).session.read_only()
    }

    /// Undo/redo or history browsing replaced the text.
    fn sync_from_doc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(src) = text_of(&self.doc, self.tab, cx) else { return };
        if src == self.synced {
            cx.notify();
            return;
        }
        self.save_task = None;
        self.synced = src.clone();
        if self.editor.read(cx).value().as_ref() != src {
            self.editor.update(cx, |e, cx| e.set_value(src.clone(), window, cx));
        }
        self.recompile(src, cx);
    }

    fn schedule_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            let _ = this.update(cx, |this: &mut Self, cx| this.save(cx));
        }));
        cx.notify();
    }

    /// Save the editor's text into the document (Onshape saves as you type;
    /// Part Studios keep using the committed source).
    fn save(&mut self, cx: &mut Context<Self>) {
        self.save_task = None;
        let src = self.editor.read(cx).value().to_string();
        if src == self.synced || self.read_only(cx) {
            return;
        }
        self.synced = src.clone();
        let tab = self.tab;
        let value = src.clone();
        self.doc.update(cx, |doc, cx| {
            doc.edit_tab(tab, "Edit Feature Studio", cx, move |c| {
                if let TabContent::FeatureStudio(f) = c {
                    f.save(value);
                }
            });
        });
        self.recompile(src, cx);
    }

    /// Commit: apply this Feature Studio's text throughout the document.
    pub fn commit(&mut self, cx: &mut Context<Self>) {
        self.save(cx);
        let tab = self.tab;
        if !self.uncommitted(cx) {
            return;
        }
        self.doc.update(cx, |doc, cx| {
            doc.edit_tab(tab, "Commit Feature Studio", cx, |c| {
                if let TabContent::FeatureStudio(f) = c {
                    f.commit();
                }
            });
            let _ = doc.custom_features();
        });
        cx.notify();
    }

    /// Commit all: every Feature Studio with uncommitted changes.
    fn commit_all(&mut self, cx: &mut Context<Self>) {
        self.save(cx);
        commit_all(&self.doc, cx);
        cx.notify();
    }

    fn uncommitted(&self, cx: &App) -> bool {
        match self.doc.read(cx).session.doc.tab(self.tab).map(|t| &t.content) {
            Some(TabContent::FeatureStudio(f)) => f.has_uncommitted() || self.editor.read(cx).value().as_ref() != f.source,
            _ => false,
        }
    }

    /// Feature Studios with uncommitted changes ("Commit all (n)").
    fn uncommitted_count(&self, cx: &App) -> usize {
        let doc = &self.doc.read(cx).session.doc;
        doc.tabs.iter().filter(|t| matches!(&t.content, TabContent::FeatureStudio(f) if f.has_uncommitted() || (t.id == self.tab && self.editor.read(cx).value().as_ref() != f.source))).count()
    }

    fn recompile(&mut self, src: String, cx: &mut Context<Self>) {
        let tab = self.tab;
        self.compiling = true;
        self.compile_task = Some(cx.spawn(async move |this, cx| {
            let compiled = cx.background_spawn(async move { noshape_model::custom::compile(tab, &src) }).await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.compiling = false;
                this.compiled = Some(compiled);
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Jump to a 1-based line and column, remembering where we were.
    pub fn goto_line(&mut self, line: usize, column: usize, window: &mut Window, cx: &mut Context<Self>) {
        if line == 0 {
            return;
        }
        let here = self.editor.read(cx).cursor_position();
        self.nav.truncate(self.nav_at);
        self.nav.push(here);
        let to = Position { line: (line - 1) as u32, character: column.saturating_sub(1) as u32 };
        self.nav.push(to);
        self.nav_at = self.nav.len() - 1;
        self.move_cursor(to, window, cx);
    }

    fn move_cursor(&mut self, to: Position, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| {
            e.set_cursor_position(to, window, cx);
            e.focus(window, cx);
        });
        cx.notify();
    }

    fn navigate(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.nav_at as isize + delta;
        if at < 0 || at as usize >= self.nav.len() {
            return;
        }
        self.nav_at = at as usize;
        self.move_cursor(self.nav[self.nav_at], window, cx);
    }

    fn toggle_outline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.outline.take().is_none() {
            let st = cx.new(|cx| InputState::new(window, cx).placeholder("Search symbols..."));
            st.update(cx, |s, cx| s.focus(window, cx));
            self.outline = Some(st);
        }
        cx.notify();
    }

    /// Commit ▾ (help: "Commit" ctrl s, "Commit all (n)" shift ctrl s).
    fn open_commit_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.uncommitted_count(cx);
        let (v1, v2) = (cx.entity(), cx.entity());
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
            use crate::os_menu::OsMenuItem as I;
            let all = if n > 0 { format!("Commit all ({n})") } else { "Commit all".to_string() };
            m.dropdown()
                .item(I::new("Commit").keys(&["ctrl", "s"]).on_click(move |_, _, cx| v1.update(cx, |t, cx| t.commit(cx))))
                .item(I::new(all).keys(&["shift", "ctrl", "s"]).on_click(move |_, _, cx| v2.update(cx, |t, cx| t.commit_all(cx))))
        });
        crate::os_menu::open_anchored(at, Anchor::TopLeft, menu, window, cx);
    }

    /// The blue Commit split button.
    fn render_commit(&self, cx: &mut Context<Self>) -> AnyElement {
        let dirty = self.uncommitted(cx);
        let blue = ButtonCustomVariant::new(cx).color(rgb(0x1651b0).into()).foreground(rgb(0xffffff).into()).hover(rgb(0x0e3f8c).into()).active(rgb(0x0e3f8c).into()).shadow(false);
        let anchor: Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
        let probe = anchor.clone();
        h_flex()
            .relative()
            .flex_none()
            .mx(px(4.))
            .child(
                Button::new("fs-commit")
                    .custom(blue)
                    .h(px(30.))
                    .px(px(10.5))
                    .rounded_l(px(5.25))
                    .rounded_r(px(0.))
                    .child(div().text_size(px(14.)).child("Commit"))
                    .opacity(if dirty { 1. } else { 0.65 })
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.commit(cx))),
            )
            .child(div().w(px(1.)).h(px(30.)).bg(rgb(0x0e3f8c)))
            .child(
                Button::new("fs-commit-menu")
                    .custom(blue)
                    .h(px(30.))
                    .w(px(24.))
                    .p_0()
                    .rounded_r(px(5.25))
                    .rounded_l(px(0.))
                    .child(os::glyph(os::GLYPH_CHEVRON_DOWN, 14., rgb(0xffffff)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        let b = anchor.get();
                        this.open_commit_menu(point(b.left(), b.bottom() + px(2.)), window, cx);
                    })),
            )
            .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().size_full())
            .into_any_element()
    }

    /// Search tools… (alt+c): the Feature Studio's commands.
    fn open_tools(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let (v1, v2, v3, v4) = (cx.entity(), cx.entity(), cx.entity(), cx.entity());
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
            use crate::os_menu::OsMenuItem as I;
            m.dropdown()
                .item(I::new("Commit").keys(&["ctrl", "s"]).on_click(move |_, _, cx| v1.update(cx, |t, cx| t.commit(cx))))
                .item(I::new("Commit all").keys(&["shift", "ctrl", "s"]).on_click(move |_, _, cx| v2.update(cx, |t, cx| t.commit_all(cx))))
                .item(I::new("Search Feature Studios").keys(&["shift", "ctrl", "f"]).on_click(move |_, _, cx| v3.update(cx, |_, cx| cx.emit(FeatureStudioEvent::OpenSearch))))
                .item(I::new("Module outline").on_click(move |_, w, cx| v4.update(cx, |t, cx| t.toggle_outline(w, cx))))
        });
        crate::os_menu::open_anchored(at, Anchor::TopLeft, menu, window, cx);
    }

    /// Module outline ▾ popover: "Search symbols…", "n symbols", the list.
    fn render_outline(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let field = self.outline.clone()?;
        let pal = p();
        let q = field.read(cx).value().to_lowercase();
        let symbols: Vec<Symbol> = outline(self.editor.read(cx).value().as_ref()).into_iter().filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q)).collect();
        let count = format!("{} symbol{}", symbols.len(), if symbols.len() == 1 { "" } else { "s" });
        let rows: Vec<AnyElement> = symbols
            .into_iter()
            .enumerate()
            .map(|(i, s)| {
                let line = s.line;
                h_flex()
                    .id(SharedString::from(format!("sym-{i}")))
                    .h(px(34.))
                    .px(px(8.))
                    .gap(px(8.))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(0xdef1fb)))
                    .child(os::glyph(os::GLYPH_FUNCTION, 16., pal.text))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(14.)).text_color(pal.text).child(format!("{}({})", s.name, s.params)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.outline = None;
                        this.goto_line(line, 1, window, cx);
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            deferred(
                v_flex()
                    .id("fs-outline")
                    .occlude()
                    .absolute()
                    .top(px(34.))
                    .left(px(4.))
                    .w(px(254.))
                    .max_h(px(420.))
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xd7d7d7))
                    .rounded(px(5.25))
                    .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                    .p(px(8.))
                    .gap(px(6.))
                    .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                        this.outline = None;
                        cx.notify();
                    }))
                    .child(Input::new(&field).h(px(30.)).text_size(px(14.)))
                    .child(div().text_size(px(12.)).text_color(pal.text_muted).child(count))
                    .child(v_flex().id("fs-outline-list").overflow_y_scroll().children(rows)),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    /// FeatureScript notices: the compile problems of the saved text.
    fn render_notices(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let c = self.compiled.clone()?;
        if c.diagnostics.is_empty() {
            return None;
        }
        let rows: Vec<AnyElement> = c
            .diagnostics
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let (line, col) = (d.line, d.column);
                let icon = if d.severity == Severity::Error { "msg-error" } else { "msg-warning" };
                h_flex()
                    .id(SharedString::from(format!("diag-{i}")))
                    .h(px(26.))
                    .px(px(10.))
                    .gap(px(8.))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .hover(|s| s.bg(rgb(0xdef1fb)))
                    .child(crate::assets::icon(icon, 16., cx))
                    .child(div().flex_1().min_w_0().truncate().text_color(pal.text).child(d.message.clone()))
                    .child(div().text_color(pal.text_muted).child(if line > 0 { format!("Line {line}, Column {col}") } else { String::new() }))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.goto_line(line, col, window, cx)))
                    .into_any_element()
            })
            .collect();
        Some(v_flex().id("fs-notices").max_h(px(130.)).flex_none().overflow_y_scroll().border_t_1().border_color(rgb(0xd7d7d7)).bg(rgb(0xffffff)).py(px(2.)).children(rows).into_any_element())
    }
}

/// Commit every Feature Studio of the document that has uncommitted changes
/// (each a change of its tab, so the Part Studios using it regenerate).
pub fn commit_all(doc: &Entity<DocModel>, cx: &mut App) {
    let tabs: Vec<TabId> = doc.read(cx).session.doc.tabs.iter().filter(|t| matches!(&t.content, TabContent::FeatureStudio(f) if f.has_uncommitted())).map(|t| t.id).collect();
    doc.update(cx, |doc, cx| {
        for tab in tabs {
            doc.edit_tab(tab, "Commit Feature Studio", cx, |c| {
                if let TabContent::FeatureStudio(f) = c {
                    f.commit();
                }
            });
        }
        let _ = doc.custom_features();
    });
}

fn text_of(doc: &Entity<DocModel>, tab: TabId, cx: &App) -> Option<String> {
    match &doc.read(cx).session.doc.tab(tab)?.content {
        TabContent::FeatureStudio(f) => Some(f.text().to_string()),
        _ => None,
    }
}

impl Render for FeatureStudioView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let read_only = self.read_only(cx);
        let notices = self.render_notices(cx);
        let outline = self.render_outline(cx);
        let tools_anchor: Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
        let tools_probe = tools_anchor.clone();
        let header_btn = |id: &'static str| {
            h_flex().id(id).flex_none().h(px(28.)).px(px(6.)).gap(px(4.)).rounded(px(3.)).cursor_pointer().hover(|d| d.bg(pal.border))
        };
        let (can_back, can_fwd) = (self.nav_at > 0, self.nav_at + 1 < self.nav.len());
        let toolbar = crate::ui::toolbar()
            .child(crate::ui::undo_redo(&self.doc, cx))
            .child(crate::ui::tool_separator())
            .child(self.render_commit(cx))
            .child(
                div()
                    .relative()
                    .child(crate::ui::search_tools_toggle("fs-search-tools", false).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        let b = tools_anchor.get();
                        this.open_tools(point(b.left(), b.bottom() + px(2.)), window, cx);
                    })))
                    .child(canvas(move |b, _, _| tools_probe.set(b), |_, _, _, _| {}).absolute().size_full()),
            );
        let header = h_flex()
            .relative()
            .h(px(36.))
            .flex_none()
            .px(px(4.))
            .gap(px(4.))
            .bg(rgb(0xffffff))
            .border_b_1()
            .border_color(rgb(0xd7d7d7))
            .child(
                header_btn("fs-outline-btn")
                    .when(self.outline.is_some(), |d| d.bg(pal.border))
                    .child(os::glyph(os::GLYPH_OUTLINE, 20., pal.text))
                    .child(div().text_size(px(14.)).text_color(pal.text).child("Module outline"))
                    .child(os::glyph(os::GLYPH_CHEVRON_DOWN, 14., pal.text))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_outline(window, cx))),
            )
            .child(
                header_btn("fs-back")
                    .opacity(if can_back { 1. } else { 0.4 })
                    .child(os::glyph(os::GLYPH_ARROW_LEFT, 20., pal.text_muted))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.navigate(-1, window, cx))),
            )
            .child(
                header_btn("fs-forward")
                    .opacity(if can_fwd { 1. } else { 0.4 })
                    .child(os::glyph(os::GLYPH_ARROW_RIGHT, 20., pal.text_muted))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.navigate(1, window, cx))),
            )
            .child(div().flex_1())
            .child(
                header_btn("fs-search")
                    .tooltip(crate::ui::tooltip("Search Feature Studios"))
                    .child(os::glyph(os::GLYPH_SEARCH, 18., pal.text))
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(FeatureStudioEvent::OpenSearch))),
            )
            .children(outline);
        v_flex()
            .size_full()
            .key_context("FeatureStudio")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Commit, _, cx| this.commit(cx)))
            .on_action(cx.listener(|this, _: &CommitAll, _, cx| this.commit_all(cx)))
            .on_action(cx.listener(|_, _: &SearchStudios, _, cx| cx.emit(FeatureStudioEvent::OpenSearch)))
            .bg(rgb(0xffffff))
            .child(toolbar)
            .child(header)
            .child(div().flex_1().min_h_0().child(Editor::new(&self.editor).readonly(read_only).bordered(false).h_full()))
            .children(notices)
    }
}

// ---------------------------------------------------------------- syntax highlighting

#[derive(Clone, Copy, PartialEq)]
enum Tok {
    Comment,
    String,
    Number,
    Keyword,
    Function,
    Constant,
    Plain,
}

/// A small lexer-based highlighter for Rhai.
struct RhaiHighlighter {
    tab: TabId,
    tokens: Vec<(Range<usize>, Tok)>,
    len: usize,
}

impl Default for RhaiHighlighter {
    fn default() -> Self {
        Self { tab: TabId::new_random(), tokens: Vec::new(), len: 0 }
    }
}

const KEYWORDS: &[&str] = &[
    "fn", "let", "const", "if", "else", "for", "in", "while", "loop", "do", "until", "break", "continue", "return", "throw", "try", "catch", "switch", "true", "false",
    "private", "this", "global", "is", "as",
];

fn lex(src: &str) -> Vec<(Range<usize>, Tok)> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let start = i;
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            out.push((start..i, Tok::Comment));
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(b.len());
            out.push((start..i, Tok::Comment));
        } else if c == b'"' || c == b'`' || c == b'\'' {
            i += 1;
            while i < b.len() && b[i] != c {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(b.len());
            out.push((start..i, Tok::String));
        } else if c.is_ascii_digit() {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || b[i] == b'_') {
                i += 1;
            }
            out.push((start..i, Tok::Number));
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = &src[start..i];
            let mut j = i;
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            let tok = if KEYWORDS.contains(&word) {
                Tok::Keyword
            } else if b.get(j) == Some(&b'(') {
                Tok::Function
            } else if word.len() > 1 && word.chars().all(|ch| ch.is_ascii_uppercase() || ch == '_') {
                Tok::Constant
            } else {
                Tok::Plain
            };
            if tok != Tok::Plain {
                out.push((start..i, tok));
            }
        } else {
            // Advance by one UTF-8 character.
            i += src[i..].chars().next().map(|ch| ch.len_utf8()).unwrap_or(1);
        }
    }
    out
}

fn tok_style(t: Tok) -> HighlightStyle {
    let color = |hex: u32| -> Hsla { rgb(hex).into() };
    let mut s = HighlightStyle::default();
    match t {
        Tok::Comment => {
            // Light editor palette (Onshape's FeatureScript editor is light).
            s.color = Some(color(0x6a737d));
            s.font_style = Some(FontStyle::Italic);
        }
        Tok::String => s.color = Some(color(0xa31515)),
        Tok::Number => s.color = Some(color(0x098658)),
        Tok::Keyword => {
            s.color = Some(color(0x0000c0));
            s.font_weight = Some(FontWeight::SEMIBOLD);
        }
        Tok::Function => s.color = Some(color(0x795e26)),
        Tok::Constant => s.color = Some(color(0x1651b0)),
        Tok::Plain => {}
    }
    s
}

impl InputHighlighter for RhaiHighlighter {
    fn language(&self) -> SharedString {
        "rhai".into()
    }

    fn update(&mut self, _edit: Option<gpui_kit::component::input::InputEdit>, text: &gpui_kit::component::input::Rope, _folding: bool, _window: &mut Window, _cx: &mut Context<EditorState>) {
        let src = text.to_string();
        self.len = src.len();
        self.tokens = lex(&src);
    }

    fn styles(&self, range: &Range<usize>, _resolver: &dyn HighlightStyleResolver) -> Vec<(Range<usize>, HighlightStyle)> {
        let mut out = Vec::new();
        let mut at = range.start;
        let first = self.tokens.partition_point(|(r, _)| r.end <= range.start);
        for (r, t) in &self.tokens[first..] {
            if r.start >= range.end {
                break;
            }
            let s = r.start.max(range.start);
            let e = r.end.min(range.end);
            if s > at {
                out.push((at..s, HighlightStyle::default()));
            }
            if e > s {
                out.push((s..e, tok_style(*t)));
                at = e;
            }
        }
        if at < range.end {
            out.push((at..range.end, HighlightStyle::default()));
        }
        // Search pane matches: Onshape's yellow mark over the token colors.
        let marks = matches_of(self.tab);
        if marks.is_empty() {
            return out;
        }
        let mut split = Vec::with_capacity(out.len());
        for (r, st) in out {
            let mut at = r.start;
            for m in marks.iter().filter(|m| m.start < r.end && m.end > r.start) {
                let (s, e) = (m.start.max(r.start), m.end.min(r.end));
                if s > at {
                    split.push((at..s, st));
                }
                let mut hl = st;
                hl.background_color = Some(rgb(0xfff1a8).into());
                split.push((s..e, hl));
                at = e;
            }
            if at < r.end {
                split.push((at..r.end, st));
            }
        }
        split
    }

    fn fold_ranges(&self, _text: &gpui_kit::component::input::Rope) -> Vec<gpui_kit::component::input::FoldRange> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Tok, lex};

    // Plain std test attribute (`gpui_kit::*` brings in gpui's `test` macro).
    #[::core::prelude::v1::test]
    fn lexes_rhai() {
        let src = "// hi\nfn f(ctx, p) { let x = mm(5); \"s\" FACE }";
        let toks = lex(src);
        let kinds: Vec<Tok> = toks.iter().map(|t| t.1).collect();
        assert!(kinds.contains(&Tok::Comment) && kinds.contains(&Tok::Keyword) && kinds.contains(&Tok::Function) && kinds.contains(&Tok::String) && kinds.contains(&Tok::Constant) && kinds.contains(&Tok::Number));
    }
}
