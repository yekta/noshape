//! Feature Studio tab: a script editor for the document's custom features
//! (noshape's FeatureScript equivalent), with compile diagnostics and the
//! list of features the source declares. Edits are committed to the
//! document after a short pause; Part Studios using the features
//! regenerate.

use crate::doc_model::{DocEvent, DocModel};
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Editor, EditorState, HighlightStyleResolver, InputEvent, InputHighlighter, Position, TabSize};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::custom::{CompiledStudio, DeclKind, Severity};
use noshape_model::document::TabId;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// Pause after the last keystroke before the source is committed.
const COMMIT_DELAY: Duration = Duration::from_millis(700);

pub struct FeatureStudioView {
    tab: TabId,
    doc: Entity<DocModel>,
    editor: Entity<EditorState>,
    /// Source as last committed to / loaded from the document.
    synced: String,
    compiled: Option<Arc<CompiledStudio>>,
    compiling: bool,
    commit_task: Option<Task<()>>,
    compile_task: Option<Task<()>>,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl Focusable for FeatureStudioView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl FeatureStudioView {
    pub fn new(doc: Entity<DocModel>, tab: TabId, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let source = source_of(&doc, tab, cx).unwrap_or_default();
        let editor = cx.new(|cx| {
            let mut st = EditorState::new(window, cx)
                .language("rhai")
                .line_number(true)
                .soft_wrap(false)
                .tab_size(TabSize { tab_size: 4, hard_tabs: false })
                .default_value(source.clone());
            st.set_highlighter_factory(Rc::new(|_| Some(Box::new(RhaiHighlighter::default()) as Box<dyn InputHighlighter>)), cx);
            st
        });
        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(&editor, window, |this: &mut Self, _, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::Change) {
                this.schedule_commit(window, cx);
            } else if matches!(ev, InputEvent::Blur) {
                this.commit(cx);
            }
        }));
        subs.push(cx.subscribe_in(&doc, window, |this: &mut Self, _, ev: &DocEvent, window, cx| match ev {
            DocEvent::TabChanged(t) if *t == this.tab => this.sync_from_doc(window, cx),
            DocEvent::Reloaded => this.sync_from_doc(window, cx),
            _ => {}
        }));
        let mut this = Self { tab, doc, editor, synced: source.clone(), compiled: None, compiling: false, commit_task: None, compile_task: None, focus: cx.focus_handle(), _subs: subs };
        this.recompile(source, cx);
        this
    }

    pub fn editor_focus(&self, cx: &App) -> FocusHandle {
        self.editor.read(cx).focus_handle(cx)
    }

    fn read_only(&self, cx: &App) -> bool {
        self.doc.read(cx).session.read_only()
    }

    /// Undo/redo or history browsing replaced the source.
    fn sync_from_doc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(src) = source_of(&self.doc, self.tab, cx) else { return };
        if src == self.synced {
            return;
        }
        self.commit_task = None;
        self.synced = src.clone();
        if self.editor.read(cx).value().as_ref() != src {
            self.editor.update(cx, |e, cx| e.set_value(src.clone(), window, cx));
        }
        self.recompile(src, cx);
    }

    fn schedule_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(COMMIT_DELAY).await;
            let _ = this.update(cx, |this: &mut Self, cx| this.commit(cx));
        }));
    }

    /// Write the editor's source into the document (one microversion).
    fn commit(&mut self, cx: &mut Context<Self>) {
        self.commit_task = None;
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
                    f.source = value;
                }
            });
        });
        self.recompile(src, cx);
    }

    fn recompile(&mut self, src: String, cx: &mut Context<Self>) {
        let tab = self.tab;
        self.compiling = true;
        self.compile_task = Some(cx.spawn(async move |this, cx| {
            let compiled = cx.background_spawn(async move { noshape_model::custom::compile(tab, &src) }).await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.compiling = false;
                this.compiled = Some(compiled);
                // Register the new schemas (toolbars of open Part Studios).
                this.doc.update(cx, |d, cx| {
                    let _ = d.custom_features();
                    cx.notify();
                });
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn goto_line(&mut self, line: usize, column: usize, window: &mut Window, cx: &mut Context<Self>) {
        if line == 0 {
            return;
        }
        self.editor.update(cx, |e, cx| {
            e.set_cursor_position(Position { line: (line - 1) as u32, character: column.saturating_sub(1) as u32 }, window, cx);
            e.focus(window, cx);
        });
    }

    fn render_diagnostics(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let mut rows: Vec<AnyElement> = Vec::new();
        let status: (SharedString, Rgba) = match &self.compiled {
            _ if self.compiling => ("Compiling…".into(), pal.text_faint),
            None => ("".into(), pal.text_faint),
            Some(c) if c.has_errors() => (format!("{} problem(s)", c.diagnostics.iter().filter(|d| d.severity == Severity::Error).count()).into(), pal.error),
            Some(c) => (format!("Compiled: {} feature(s)", c.features.len()).into(), pal.success),
        };
        if let Some(c) = &self.compiled {
            for (i, d) in c.diagnostics.iter().enumerate() {
                let (line, col) = (d.line, d.column);
                let color = if d.severity == Severity::Error { pal.error } else { pal.warning };
                rows.push(
                    h_flex()
                        .id(SharedString::from(format!("diag-{i}")))
                        .gap_2()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .text_xs()
                        .cursor_pointer()
                        .hover(|s| s.bg(pal.panel_hover))
                        .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(color))
                        .child(div().w(px(64.)).text_color(pal.text_faint).child(if line > 0 { format!("Line {line}") } else { String::new() }))
                        .child(div().flex_1().text_color(pal.text).child(d.message.clone()))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.goto_line(line, col, window, cx)))
                        .into_any_element(),
                );
            }
        }
        v_flex()
            .h(px(120.))
            .flex_none()
            .border_t_1()
            .border_color(pal.border)
            .bg(pal.panel)
            .child(
                h_flex()
                    .h(px(26.))
                    .px_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(pal.border)
                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child("PROBLEMS"))
                    .child(div().text_xs().text_color(status.1).child(status.0)),
            )
            .child(v_flex().id("diagnostics").flex_1().overflow_y_scroll().p_1().children(rows))
            .into_any_element()
    }

    fn render_features(&mut self) -> AnyElement {
        let pal = p();
        let mut list: Vec<AnyElement> = Vec::new();
        if let Some(c) = &self.compiled {
            for f in &c.features {
                let params: Vec<String> = f
                    .params
                    .iter()
                    .map(|d| {
                        let kind = match &d.kind {
                            DeclKind::Query { .. } => "selection",
                            DeclKind::Length { .. } => "length",
                            DeclKind::Angle { .. } => "angle",
                            DeclKind::Real { .. } => "number",
                            DeclKind::Int { .. } => "integer",
                            DeclKind::Bool { .. } => "check box",
                            DeclKind::Enum { .. } => "choice",
                            DeclKind::Text { .. } => "text",
                        };
                        format!("{} ({kind})", d.label)
                    })
                    .collect();
                list.push(
                    v_flex()
                        .gap_1()
                        .p_2()
                        .rounded_md()
                        .bg(pal.panel_raised)
                        .border_1()
                        .border_color(pal.border)
                        .child(
                            h_flex()
                                .gap_2()
                                .child(svg().path(crate::assets::icon_path(&f.icon)).size_4().text_color(pal.accent))
                                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(f.name.clone())),
                        )
                        .when(!f.description.is_empty(), |d| d.child(div().text_xs().text_color(pal.text_muted).child(f.description.clone())))
                        .children(params.into_iter().map(|t| div().pl_6().text_xs().text_color(pal.text_faint).child(t)))
                        .into_any_element(),
                );
            }
        }
        if list.is_empty() {
            list.push(div().p_2().text_xs().text_color(pal.text_faint).child("No features declared yet.").into_any_element());
        }
        v_flex()
            .w(px(280.))
            .flex_none()
            .h_full()
            .border_l_1()
            .border_color(pal.border)
            .bg(pal.panel)
            .child(
                h_flex()
                    .h(px(26.))
                    .px_2()
                    .border_b_1()
                    .border_color(pal.border)
                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child("FEATURES")),
            )
            .child(v_flex().id("fs-features").flex_1().overflow_y_scroll().p_2().gap_2().children(list))
            .child(
                div()
                    .p_2()
                    .border_t_1()
                    .border_color(pal.border)
                    .text_xs()
                    .text_color(pal.text_faint)
                    .child("Features appear in every Part Studio's toolbar (Custom features) and in tool search (Alt+C). Script API: docs/custom-features.md"),
            )
            .into_any_element()
    }
}

fn source_of(doc: &Entity<DocModel>, tab: TabId, cx: &App) -> Option<String> {
    match &doc.read(cx).session.doc.tab(tab)?.content {
        TabContent::FeatureStudio(f) => Some(f.source.clone()),
        _ => None,
    }
}

impl Render for FeatureStudioView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let read_only = self.read_only(cx);
        let dirty = self.commit_task.is_some();
        let diagnostics = self.render_diagnostics(cx);
        let features = self.render_features();
        v_flex()
            .size_full()
            .track_focus(&self.focus)
            .bg(pal.app_bg)
            .child(
                h_flex()
                    .h(px(40.))
                    .px_3()
                    .gap_2()
                    .bg(pal.panel)
                    .border_b_1()
                    .border_color(pal.border)
                    .child(svg().path(crate::assets::icon_path("feature-studio")).size_4().text_color(pal.accent))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Feature Studio"))
                    .child(div().text_xs().text_color(pal.text_faint).child("Custom features in Rhai script. Lengths in meters (mm(5)), angles in radians (deg(30))."))
                    .child(div().flex_1())
                    .when(dirty, |d| d.child(div().text_xs().text_color(pal.text_faint).child("Unsaved edits…")))
                    .child(
                        Button::new("fs-commit")
                            .label("Commit")
                            .primary()
                            .small()
                            .tooltip("Save the source now (otherwise saved automatically after a pause)")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.commit(cx))),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(v_flex().flex_1().min_w_0().h_full().child(div().flex_1().min_h_0().child(Editor::new(&self.editor).readonly(read_only).bordered(false).h_full())).child(diagnostics))
                    .child(features),
            )
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
#[derive(Default)]
struct RhaiHighlighter {
    tokens: Vec<(Range<usize>, Tok)>,
    len: usize,
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
        out
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
