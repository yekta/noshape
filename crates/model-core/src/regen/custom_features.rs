//! What regeneration needs of the FeatureScript host (noshape-custom), which sits above the model core: the
//! compiled Feature Studio types and [`CustomFeatureProvider`] (passed in `RegenOptions::custom_features`), and
//! the [`CustomHost`] hook through which regeneration evaluates custom features and the features std runs. The host
//! registers it (`noshape_custom::register`, called by every compile, `DocumentScripts::from_document`, std's
//! prewarm, …); a regeneration that needs it before then fails the feature with [`NO_HOST`].

use crate::document::TabId;
use crate::features::FeatureType;
use crate::params::{Hints, NumKind, NumSpec, QueryFilter};
use crate::regen::eval::{EvalCtx, FResult, err};
use crate::regen::features::EvalFn;
use noshape_featurescript::compile::CompiledModule;
use noshape_featurescript::ir::{CompiledFn, GlobalCell};
use noshape_featurescript::value::Tag;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A compile notice. `line`/`column` are 1-based (0 = unknown).
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub line: usize,
    pub column: usize,
    pub message: String,
    /// "Parse error" notices (syntax) vs other compile errors.
    pub parse: bool,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line > 0 { write!(f, "Line {}: {}", self.line, self.message) } else { f.write_str(&self.message) }
    }
}

/// Kind of a feature parameter (from its precondition statement).
#[derive(Clone, Debug)]
pub enum ParamKindFs {
    Query { filter: QueryFilter, max_picks: Option<usize> },
    /// `definition.x is FeatureList`: picked features, passed as a map from
    /// feature Id to feature function.
    FeatureList { max_picks: Option<usize> },
    Num { kind: NumKind, spec: NumSpec },
    Bool { default: bool },
    Enum { tag: Tag, options: Vec<(String, String)>, default: String },
    Text { default: String },
    /// `template`: the "Item label template" (`#param` placeholders).
    Array { item_label: String, template: Option<String>, items: Vec<ParamDecl> },
    /// `definition.x is LookupTablePath` with a "Lookup Table" annotation:
    /// the table registered under `table` (see `hole_tables::DynLevel`).
    Lookup { table: &'static str },
    /// `definition.x is TableData` (and the other reference types).
    Reference { kind: crate::params::ReferenceKind },
    /// `isAnything(definition.x)` and other parameters without a UI.
    Anything,
    /// A parameter group (`annotation { "Group Name" : "Position" } { ... }`):
    /// its title, unfolded unless "Collapsed By Default". Not in the
    /// definition; the group's parameters show only while it is unfolded.
    Group { open: bool },
}

/// A feature parameter.
#[derive(Clone)]
pub struct ParamDecl {
    pub id: String,
    pub label: String,
    pub kind: ParamKindFs,
    pub hints: Hints,
    pub description: String,
    /// Visibility conditions (`if (definition.x) { ... }`): all must hold.
    pub conditions: Vec<Arc<CompiledFn>>,
}

impl std::fmt::Debug for ParamDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{:?}", self.id, self.kind)
    }
}

/// A custom feature declared by a Feature Studio.
#[derive(Clone)]
pub struct CustomFeatureDef {
    /// The exported constant (`myFeature`).
    pub name: String,
    /// "Feature Type Name" (`My Feature`).
    pub display_name: String,
    /// `custom:<tab>:<name>`.
    pub type_id: String,
    pub description: String,
    pub icon: String,
    pub params: Vec<ParamDecl>,
    pub feature_type: &'static FeatureType,
    pub cell: Arc<GlobalCell>,
    pub editing_logic: Option<String>,
}

/// A custom table declared by a Feature Studio.
#[derive(Clone)]
pub struct CustomTableDef {
    /// The exported constant (`myTable`).
    pub name: String,
    /// "Table Type Name" (`My table`).
    pub display_name: String,
    /// "Table Type Description", if any.
    pub description: String,
    /// Parameters of its precondition (Onshape shows them above the table).
    pub params: Vec<ParamDecl>,
    pub cell: Arc<GlobalCell>,
}

impl std::fmt::Debug for CustomTableDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomTableDef").field("name", &self.name).field("display_name", &self.display_name).finish()
    }
}

impl std::fmt::Debug for CustomFeatureDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomFeatureDef").field("name", &self.name).field("params", &self.params).finish()
    }
}

/// Result of compiling one Feature Studio's source.
pub struct CompiledStudio {
    pub tab: TabId,
    pub hash: u64,
    pub features: Vec<CustomFeatureDef>,
    /// Custom tables (`"Table Type Name"` + `defineTable`).
    pub tables: Vec<CustomTableDef>,
    pub diagnostics: Vec<Diagnostic>,
    pub module: Option<Arc<CompiledModule>>,
}

impl std::fmt::Debug for CompiledStudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledStudio").field("tab", &self.tab).field("features", &self.features).field("diagnostics", &self.diagnostics).finish()
    }
}

impl CompiledStudio {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }
    pub fn feature(&self, name: &str) -> Option<&CustomFeatureDef> {
        self.features.iter().find(|f| f.name == name)
    }
    pub fn table(&self, name: &str) -> Option<&CustomTableDef> {
        self.tables.iter().find(|t| t.name == name)
    }
    pub fn first_error(&self) -> Option<&Diagnostic> {
        self.diagnostics.iter().find(|d| d.severity == Severity::Error)
    }
}

/// Supplies compiled Feature Studios by tab id string (the `<tab>` part of
/// `custom:<tab>:<name>`). Passed to regeneration in
/// `RegenOptions::custom_features`, like the Derived provider.
pub trait CustomFeatureProvider: Send + Sync + std::fmt::Debug {
    fn studio(&self, tab: &str) -> Option<Arc<CompiledStudio>>;
    /// A file tab of the document, by tab id string (reference parameters).
    fn file(&self, _tab: &str) -> Option<crate::file_tab::FileTab> {
        None
    }
    /// Changes whenever the studio's compiled result may change.
    fn version(&self, tab: &str) -> u64 {
        self.studio(tab).map(|s| s.hash).unwrap_or(0)
    }
}

/// The FeatureScript host's entry points, registered by noshape-custom ([`set_host`]).
#[derive(Clone, Copy, Debug)]
pub struct CustomHost {
    /// The provider for regenerations without one: the most recently compiled Feature Studios.
    pub fallback_provider: fn() -> Arc<dyn CustomFeatureProvider>,
    /// Evaluates the current `custom:<tab>:<name>` feature.
    pub evaluate_custom: EvalFn,
    /// Evaluates the current feature with std's feature `name` of `module` (e.g. "sheetMetalFlange.fs",
    /// "sheetMetalFlange").
    pub evaluate_std: fn(&mut EvalCtx, &'static str, &'static str) -> FResult<()>,
}

static HOST: OnceLock<CustomHost> = OnceLock::new();

/// Register the FeatureScript host (the first registration wins; noshape-custom registers the same one each time).
pub fn set_host(host: CustomHost) {
    let _ = HOST.set(host);
}

/// The registered FeatureScript host, if noshape-custom has registered it.
pub fn host() -> Option<&'static CustomHost> {
    HOST.get()
}

/// A feature's error when it needs the FeatureScript host and nothing has registered it.
pub const NO_HOST: &str = "The FeatureScript host is not loaded (noshape_custom::register)";

/// No Feature Studios: the fallback provider before the host is registered.
#[derive(Debug)]
struct NoStudios;

impl CustomFeatureProvider for NoStudios {
    fn studio(&self, _tab: &str) -> Option<Arc<CompiledStudio>> {
        None
    }
}

/// Provider for regenerations without an explicit one: the most recently compiled Feature Studios (registered by
/// `DocumentScripts::from_document`).
pub fn fallback_provider() -> Arc<dyn CustomFeatureProvider> {
    match host() {
        Some(h) => (h.fallback_provider)(),
        None => Arc::new(NoStudios),
    }
}

/// Evaluator for every `custom:<tab>:<name>` feature type (the host's).
pub fn evaluate_custom(ev: &mut EvalCtx) -> FResult<()> {
    match host() {
        Some(h) => (h.evaluate_custom)(ev),
        None => err(NO_HOST),
    }
}

/// Evaluate the current feature with std's feature `name` of `module` (the host's).
pub fn evaluate_std(ev: &mut EvalCtx, module: &'static str, name: &'static str) -> FResult<()> {
    match host() {
        Some(h) => (h.evaluate_std)(ev, module, name),
        None => err(NO_HOST),
    }
}

/// `ErrorStringEnum` members' texts (the comments above them in
/// errorstringenum.gen.fs) and, back, each text's first member.
#[allow(clippy::type_complexity)]
fn error_texts() -> &'static (HashMap<String, String>, HashMap<String, String>) {
    static TEXTS: std::sync::LazyLock<(HashMap<String, String>, HashMap<String, String>)> = std::sync::LazyLock::new(|| {
        let (mut m, mut back) = (HashMap::new(), HashMap::new());
        let src = noshape_featurescript::std_source("errorstringenum.gen.fs").unwrap_or("");
        let mut last: Option<String> = None;
        for line in src.lines() {
            let t = line.trim();
            if let Some(c) = t.strip_prefix("/*").and_then(|c| c.strip_suffix("*/")) {
                last = Some(c.trim().to_string());
                continue;
            }
            let name = t.trim_end_matches(',');
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
                if let Some(text) = last.take() {
                    back.entry(text.clone()).or_insert_with(|| name.to_string());
                    m.insert(name.to_string(), text);
                }
            } else if !t.starts_with('*') && !t.starts_with("/**") {
                last = None;
            }
        }
        (m, back)
    });
    &TEXTS
}

/// The `ErrorStringEnum` member whose text is `text` (a built-in
/// evaluator's Onshape message, reported by an operation as its member).
pub fn error_code(text: &str) -> Option<&'static str> {
    error_texts().1.get(text).map(String::as_str)
}

/// The user-facing text of an `ErrorStringEnum` member: the comment above
/// it in errorstringenum.gen.fs (what Onshape shows in the feature list).
pub fn error_text(member: &str) -> String {
    error_texts().0.get(member).cloned().unwrap_or_else(|| match member {
        "REGEN_ERROR" => "Error regenerating.".to_string(),
        other => other.to_string(),
    })
}

/// (width, height, media type) from PNG / GIF / JPEG headers.
pub fn image_size(b: &[u8]) -> (Option<u32>, Option<u32>, &'static str) {
    let be32 = |i: usize| b.get(i..i + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]));
    let be16 = |i: usize| b.get(i..i + 2).map(|s| u16::from_be_bytes([s[0], s[1]]) as u32);
    let le16 = |i: usize| b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as u32);
    if b.starts_with(b"\x89PNG") {
        return (be32(16), be32(20), "image/png");
    }
    if b.starts_with(b"GIF8") {
        return (le16(6), le16(8), "image/gif");
    }
    if b.starts_with(&[0xFF, 0xD8]) {
        // Walk the segments to a start-of-frame marker.
        let mut i = 2;
        while i + 9 < b.len() {
            if b[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = b[i + 1];
            let len = be16(i + 2).unwrap_or(0) as usize;
            if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
                return (be16(i + 7), be16(i + 5), "image/jpeg");
            }
            i += 2 + len.max(1);
        }
        return (None, None, "image/jpeg");
    }
    (None, None, "application/octet-stream")
}
