//! Text input: the shared editing engine and the three states built on it.
//!
//! Nothing here should be `pub` unless it is reachable from outside the crate.
//! A `pub` on an item behind a private module says something the module path
//! contradicts, and it hides the real API surface from anyone reading it.
#![warn(unreachable_pub)]

use gpui::App;

/// Character used by masked editor modes.
pub(crate) const MASK_CHAR: char = '•';

/// NOSHAPE PATCH: the browser's text selection for plain (single-line,
/// unmasked, non-code) inputs: when an app sets this global, their focused
/// selection is filled with `background` and the selected text drawn in
/// `foreground` (Chrome's `#3367d1` / white, as in Onshape). Code editors
/// keep the theme's selection color.
#[derive(Clone, Copy, Debug)]
pub struct PlainSelectionColors {
    pub background: gpui::Hsla,
    pub foreground: gpui::Hsla,
}

impl gpui::Global for PlainSelectionColors {}

/// NOSHAPE PATCH: how inputs draw their placeholder when an app sets this
/// global (Onshape: `input::placeholder { font-style: italic; font-weight:
/// 300 }` everywhere). Only the placeholder takes it; typed text keeps the
/// input's own style. Drawn by the input itself, the placeholder starts
/// exactly where typed text and the caret do.
#[derive(Clone, Copy, Debug)]
pub struct PlaceholderStyle {
    pub italic: bool,
    pub weight: Option<gpui::FontWeight>,
    pub color: Option<gpui::Hsla>,
}

impl gpui::Global for PlaceholderStyle {}

/// NOSHAPE PATCH: code editor gutter metrics. When an app sets this global,
/// code editors lay out their gutter with it instead of the crate's own
/// constants (3-digit minimum, 6 px margin, 18 px fold column), e.g. to match
/// Ace's 42 px gutter in Onshape's Feature Studio.
#[derive(Clone, Copy, Debug)]
pub struct CodeGutterMetrics {
    /// Digits reserved for line numbers before the line count needs more.
    pub min_digits: usize,
    /// Space between the gutter's left padding and the line numbers.
    pub number_inset: gpui::Pixels,
    /// Space between the line numbers and the fold column.
    pub right_margin: gpui::Pixels,
    /// Width of the fold icon column at the gutter's right edge.
    pub fold_width: gpui::Pixels,
    /// Space between the gutter and the text (editor background).
    pub text_gap: gpui::Pixels,
    /// The current line's gutter cell (else the active line color).
    pub active_background: Option<gpui::Hsla>,
}

impl gpui::Global for CodeGutterMetrics {}

/// NOSHAPE PATCH: a gutter marker (Ace's error / warning icon, 2 px into the
/// gutter cell, centred on the line), given its index, the most severe
/// severity and every (severity, message) it stands for, for its tooltip.
/// Code editors draw none unless the app sets this global.
#[derive(Clone)]
pub struct GutterMarkerRenderer(pub std::rc::Rc<GutterMarkerFn>);

/// NOSHAPE PATCH: see [`GutterMarkerRenderer`].
pub type GutterMarkerFn = dyn Fn(usize, DiagnosticSeverity, &[(DiagnosticSeverity, gpui::SharedString)], &gpui::App) -> gpui::AnyElement;

impl gpui::Global for GutterMarkerRenderer {}

/// NOSHAPE PATCH: when on, inputs open no diagnostic popover; the app reads
/// `hovered_diagnostic()` and draws its own tooltip (Onshape's Ace tooltip).
static APP_DIAGNOSTIC_HOVER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// NOSHAPE PATCH: see [`APP_DIAGNOSTIC_HOVER`].
pub fn set_app_diagnostic_hover(on: bool) {
    APP_DIAGNOSTIC_HOVER.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn app_diagnostic_hover() -> bool {
    APP_DIAGNOSTIC_HOVER.load(std::sync::atomic::Ordering::Relaxed)
}

#[path = "editor/auto_close.rs"]
mod auto_close;
mod base;
#[path = "base/blink_cursor.rs"]
pub(crate) mod blink_cursor;
#[path = "base/change.rs"]
mod change;
#[path = "base/cursor.rs"]
mod cursor;
#[path = "editor/decorations.rs"]
mod decorations;
#[path = "editor/diagnostics.rs"]
mod diagnostics;
#[path = "editor/display_map/mod.rs"]
mod display_map;
mod editor;
#[path = "base/element.rs"]
mod element;
// NOSHAPE PATCH: the whole input box is the hit target.
#[path = "base/frame_hit.rs"]
mod frame_hit;
pub use frame_hit::{InputFrame, InputFrameHit, input_frame};
#[path = "editor/highlighting.rs"]
mod highlighting;
#[path = "editor/indent.rs"]
mod indent;
#[path = "base/inline_tokens.rs"]
mod inline_tokens;
mod input;
#[path = "base/token_presentation.rs"]
mod token_presentation;
pub use inline_tokens::{InlineToken, InlineTokenError, InlineTokenSpan, InputContent};
pub(crate) use token_presentation::InlineTokenPresentation;
pub use token_presentation::{
    InlineTokenClickEvent, InlineTokenClickListener, InlineTokenContext, InlineTokenHoverEvent,
    InlineTokenHoverListener, InlineTokenRenderer,
};
#[path = "base/kind.rs"]
mod kind;
#[path = "editor/language.rs"]
mod language;
#[path = "editor/language_config.rs"]
pub mod language_config;
#[path = "base/layout.rs"]
mod layout;
#[path = "editor/lsp/mod.rs"]
mod lsp;
#[path = "base/mask_pattern.rs"]
mod mask_pattern;
#[path = "base/mode.rs"]
mod mode;
#[path = "base/movement.rs"]
mod movement;
#[path = "base/native.rs"]
mod native;
#[path = "base/rope_ext.rs"]
mod rope_ext;
#[path = "editor/search.rs"]
mod search;
#[path = "base/selection.rs"]
mod selection;
#[path = "base/state.rs"]
mod state;
// NOSHAPE PATCH (agent qa4): a multi-line input's text as a cached view.
#[path = "base/text_layer.rs"]
mod text_layer;
mod textarea;
#[path = "base/touch.rs"]
mod touch;
#[path = "base/undo_manager.rs"]
mod undo_manager;

pub(crate) fn init(cx: &mut App) {
    state::init(cx);
}

pub use crate::number_input::{NumberInputEvent, NumberStep};
pub use base::{InputBase, InputContextMenuCapabilities, InputStyles};
pub use cursor::Selection;
pub use decorations::{
    RangeDecoration, RangeDecorationCollection, RangeDecorationStyle, TextDecoration,
    TextDecorationCollection,
};
pub use diagnostics::{
    Diagnostic, DiagnosticEntry, DiagnosticRelatedInformation, DiagnosticSet, DiagnosticSeverity,
    DiagnosticSummary, DiagnosticTag, RelatedInformation,
};
pub use display_map::{BufferPoint, DisplayMap, DisplayPoint, FoldRange, WrappingIndent};
pub use editor::{Editor, EditorState};
pub use highlighting::{
    DiagnosticColors, FoldIconRenderer, HighlightStyleResolver, InputEditorStyle, InputHighlighter,
    InputHighlighterFactory, SharedHighlightStyleResolver, SyntaxContext, SyntaxContextProvider,
};
pub use indent::TabSize;
pub use input::{Input, InputState};
pub use kind::{
    EditorExtras, EditorMode, InputExtras, InputMode, InputModeKind, MultiLineMode, TextareaMode,
};
pub(crate) use language::EditorLanguage;
pub use language::{LanguageProvider, set_language_config, set_language_provider};
pub(crate) use language_config::LanguageConfig;
pub use language_config::{AutoClosingPair, BracketPair, IndentationRules};
pub use lsp::{
    CodeActionItem, CodeActionMenuState, CodeActionProvider, CompletionMenuOptions,
    CompletionMenuState, CompletionProvider, DefinitionProvider, DocumentColorProvider,
    DocumentRangeSemanticTokensProvider, HoverPopoverState, HoverProvider, InputOverlayKind, Lsp,
    ShowDocumentHandler,
};
pub use lsp_types::Position;
pub use mask_pattern::MaskPattern;
#[cfg(target_os = "macos")]
#[doc(hidden)]
pub use native::set_text_content_type;
pub use native::{NativeMenu, NativeMenuItem};
pub use rope_ext::{InputEdit, Point, RopeExt, RopeLines};
pub use ropey::Rope;
pub use search::{SearchMatcher, SearchSession};
pub use state::*;
pub use textarea::{Textarea, TextareaState};
