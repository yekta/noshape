//! Compiling a Feature Studio: parse, run the top level once to collect
//! feature declarations, validate them and build their schemas.

use super::{api, schema};
use crate::document::TabId;
use crate::features::FeatureType;
use parking_lot::Mutex;
use rhai::{AST, Dynamic, Engine, EvalAltResult, FnPtr, Map, Position, Scope};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

/// Sandbox limits.
pub(super) const MAX_OPERATIONS: u64 = 50_000_000;
const MAX_CALL_LEVELS: usize = 48;
const TIME_LIMIT: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// A compile diagnostic. `line`/`column` are 1-based (0 = unknown).
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl Diagnostic {
    fn error(pos: Position, message: impl Into<String>) -> Self {
        Self { severity: Severity::Error, line: pos.line().unwrap_or(0), column: pos.position().unwrap_or(0), message: message.into() }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line > 0 { write!(f, "Line {}: {}", self.line, self.message) } else { f.write_str(&self.message) }
    }
}

/// Value a parameter must have for another one to be visible.
#[derive(Clone, Debug, PartialEq)]
pub enum VisibleValue {
    Bool(bool),
    Option(String),
}

/// Kind of a declared parameter. Numeric defaults and bounds are SI
/// (meters, radians).
#[derive(Clone, Debug, PartialEq)]
pub enum DeclKind {
    Query { filter: u32, max_picks: Option<usize> },
    Length { default: f64, min: f64, max: f64 },
    Angle { default: f64, min: f64, max: f64 },
    Real { default: f64, min: f64, max: f64 },
    Int { default: i64, min: i64, max: i64 },
    Bool { default: bool },
    Enum { options: Vec<(String, String)>, default: String },
    Text { default: String },
}

/// A parameter declared by a script (`length("radius", "Radius", mm(5))`).
#[derive(Clone, Debug, PartialEq)]
pub struct ParamDecl {
    pub id: String,
    pub label: String,
    pub kind: DeclKind,
    pub description: String,
    pub visible_when: Option<(String, VisibleValue)>,
}

/// A custom feature declared by a Feature Studio.
#[derive(Clone)]
pub struct CustomFeatureDef {
    pub name: String,
    /// `custom:<tab>:<name>`.
    pub type_id: String,
    pub description: String,
    pub icon: String,
    pub params: Vec<ParamDecl>,
    /// Registered schema (generic dialog, defaults).
    pub feature_type: &'static FeatureType,
    pub(super) evaluate: FnPtr,
}

impl std::fmt::Debug for CustomFeatureDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomFeatureDef").field("name", &self.name).field("params", &self.params.len()).finish()
    }
}

/// Result of compiling one Feature Studio's source.
pub struct CompiledStudio {
    pub tab: TabId,
    /// Hash of the source (changes whenever the source does).
    pub hash: u64,
    pub features: Vec<CustomFeatureDef>,
    pub diagnostics: Vec<Diagnostic>,
    pub(super) ast: Option<AST>,
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

    /// First error, for feature status messages.
    pub fn first_error(&self) -> Option<&Diagnostic> {
        self.diagnostics.iter().find(|d| d.severity == Severity::Error)
    }
}

// ---------------------------------------------------------------- engine

thread_local! {
    /// Features declared by the top-level run in progress on this thread.
    static DECLARED: RefCell<Option<Vec<(Position, String, Map)>>> = const { RefCell::new(None) };
    /// Deadline of the script run in progress on this thread.
    static DEADLINE: RefCell<Option<Instant>> = const { RefCell::new(None) };
}

/// The shared, sandboxed script engine. Native functions keep no state; a
/// feature evaluation passes its state in the `ctx` argument.
pub(super) static ENGINE: LazyLock<Engine> = LazyLock::new(|| {
    let mut e = Engine::new();
    // Sandbox: no module loading (no filesystem), no eval, bounded work.
    e.set_module_resolver(rhai::module_resolvers::DummyModuleResolver::new());
    e.disable_symbol("eval");
    e.set_max_operations(MAX_OPERATIONS);
    e.set_max_call_levels(MAX_CALL_LEVELS);
    e.set_max_expr_depths(96, 96);
    e.set_max_string_size(1 << 20);
    e.set_max_array_size(200_000);
    e.set_max_map_size(20_000);
    e.set_max_modules(0);
    e.set_max_variables(10_000);
    e.set_max_functions(2_000);
    e.on_print(|s| log::info!("[feature studio] {s}"));
    e.on_debug(|s, _src, pos| log::debug!("[feature studio] {pos}: {s}"));
    e.on_progress(|ops| {
        if ops % 65_536 == 0 && DEADLINE.with(|d| d.borrow().is_some_and(|t| Instant::now() > t)) {
            return Some(Dynamic::from("Script took too long (time limit exceeded)"));
        }
        None
    });
    e.register_fn("feature", declare_feature);
    api::register(&mut e);
    e
});

/// Run `f` with the per-run deadline set.
pub(super) fn with_deadline<R>(f: impl FnOnce() -> R) -> R {
    let prev = DEADLINE.with(|d| d.replace(Some(Instant::now() + TIME_LIMIT)));
    let r = f();
    DEADLINE.with(|d| *d.borrow_mut() = prev);
    r
}

fn declare_feature(ctx: rhai::NativeCallContext, name: &str, def: Map) -> Result<(), Box<EvalAltResult>> {
    let pos = ctx.call_position();
    DECLARED.with(|d| match d.borrow_mut().as_mut() {
        Some(list) => {
            list.push((pos, name.to_string(), def));
            Ok(())
        }
        None => Err("feature() can only be called at the top level of a Feature Studio".into()),
    })
}

/// Readable message for a script error, with its line when known.
pub(super) fn error_message(e: &EvalAltResult) -> String {
    use EvalAltResult as E;
    let (msg, pos) = match e {
        E::ErrorInFunctionCall(_, _, inner, _) => return error_message(inner),
        E::ErrorRuntime(v, pos) => (v.to_string(), *pos),
        E::ErrorTooManyOperations(pos) => ("Script exceeded the operation limit (infinite loop?)".to_string(), *pos),
        E::ErrorStackOverflow(pos) => ("Script recursion is too deep".to_string(), *pos),
        E::ErrorTerminated(v, pos) => (v.to_string(), *pos),
        E::ErrorDataTooLarge(what, pos) => (format!("{what} is too large"), *pos),
        other => (other.to_string(), other.position()),
    };
    // Rhai appends " (line x, position y)" to some messages already.
    match pos.line() {
        Some(l) if !msg.contains("(line ") => format!("Line {l}: {msg}"),
        _ => msg,
    }
}

// ---------------------------------------------------------------- compile

fn source_hash(source: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut h);
    h.finish()
}

type Cache = HashMap<(u64, u64), Arc<CompiledStudio>>;
static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(Default::default);

/// Compile a Feature Studio's source (cached by tab and source hash).
pub fn compile(tab: TabId, source: &str) -> Arc<CompiledStudio> {
    let hash = source_hash(source);
    if let Some(c) = CACHE.lock().get(&(tab.0, hash)) {
        return c.clone();
    }
    let compiled = Arc::new(compile_uncached(tab, source, hash));
    let mut cache = CACHE.lock();
    if cache.len() > 128 {
        cache.clear();
    }
    cache.insert((tab.0, hash), compiled.clone());
    compiled
}

fn compile_uncached(tab: TabId, source: &str, hash: u64) -> CompiledStudio {
    let mut out = CompiledStudio { tab, hash, features: Vec::new(), diagnostics: Vec::new(), ast: None };
    let engine = &*ENGINE;
    let ast = match engine.compile(source) {
        Ok(a) => a,
        Err(e) => {
            out.diagnostics.push(Diagnostic::error(e.1, e.0.to_string()));
            return out;
        }
    };
    // Top level: declares features. Filter names are available as constants.
    let mut scope = Scope::new();
    for (name, _) in api::FILTERS {
        scope.push_constant(*name, name.to_string());
    }
    DECLARED.with(|d| *d.borrow_mut() = Some(Vec::new()));
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| with_deadline(|| engine.run_ast_with_scope(&mut scope, &ast))));
    let declared = DECLARED.with(|d| d.borrow_mut().take()).unwrap_or_default();
    match run {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            out.diagnostics.push(Diagnostic::error(e.position(), error_message(&e)));
            return out;
        }
        Err(_) => {
            out.diagnostics.push(Diagnostic::error(Position::NONE, "Internal error while running the Feature Studio"));
            return out;
        }
    }
    let functions: Vec<(String, usize)> = ast.iter_functions().map(|f| (f.name.to_string(), f.params.len())).collect();
    for (pos, name, def) in declared {
        match feature_def(tab, &name, &def, &functions) {
            Ok(f) => {
                if out.features.iter().any(|g| g.name == f.name) {
                    out.diagnostics.push(Diagnostic::error(pos, format!("Feature \"{name}\" is declared twice")));
                } else {
                    out.features.push(f);
                }
            }
            Err(m) => out.diagnostics.push(Diagnostic::error(pos, format!("Feature \"{name}\": {m}"))),
        }
    }
    if out.features.is_empty() && out.diagnostics.is_empty() {
        out.diagnostics.push(Diagnostic { severity: Severity::Warning, line: 0, column: 0, message: "No features declared. Declare one with feature(\"Name\", #{ params: [...], evaluate: \"fn_name\" })".into() });
    }
    out.ast = Some(ast);
    out
}

/// `"Bolt pattern"` -> `"bolt_pattern"`.
fn snake_case(name: &str) -> String {
    let mut s = String::new();
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            s.extend(ch.to_lowercase());
        } else if !s.ends_with('_') && !s.is_empty() {
            s.push('_');
        }
    }
    s.trim_end_matches('_').to_string()
}

fn feature_def(tab: TabId, name: &str, def: &Map, functions: &[(String, usize)]) -> Result<CustomFeatureDef, String> {
    let name = name.trim();
    if name.is_empty() || name.contains(':') {
        return Err("feature names must be non-empty and must not contain ':'".into());
    }
    for key in def.keys() {
        if !matches!(key.as_str(), "params" | "evaluate" | "icon" | "description") {
            return Err(format!("unknown key \"{key}\" (expected params, evaluate, icon, description)"));
        }
    }
    let text = |k: &str| -> Result<String, String> {
        match def.get(k) {
            None => Ok(String::new()),
            Some(v) => v.clone().into_string().map_err(|_| format!("\"{k}\" must be a string")),
        }
    };
    let icon = text("icon")?;
    let description = text("description")?;
    let mut params: Vec<ParamDecl> = Vec::new();
    if let Some(p) = def.get("params") {
        let arr = p.clone().into_array().map_err(|_| "\"params\" must be an array of parameters".to_string())?;
        for item in arr {
            let Some(d) = item.try_cast::<ParamDecl>() else {
                return Err("every entry of \"params\" must be a parameter, e.g. length(\"depth\", \"Depth\", mm(10))".into());
            };
            if d.id.is_empty() || !d.id.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(format!("invalid parameter id \"{}\" (letters, digits and _ only)", d.id));
            }
            if params.iter().any(|q| q.id == d.id) {
                return Err(format!("parameter id \"{}\" is used twice", d.id));
            }
            params.push(d);
        }
    }
    for d in &params {
        if let Some((other, _)) = &d.visible_when
            && !params.iter().any(|q| &q.id == other)
        {
            return Err(format!("parameter \"{}\" depends on unknown parameter \"{other}\"", d.id));
        }
    }
    let has_fn = |n: &str| functions.iter().any(|(f, arity)| f == n && *arity == 2);
    let evaluate = match def.get("evaluate") {
        Some(v) if v.is_fnptr() => {
            let f = v.clone().cast::<FnPtr>();
            if !f.is_anonymous() && !has_fn(f.fn_name()) {
                return Err(format!("evaluate function {}(ctx, params) is not defined", f.fn_name()));
            }
            f
        }
        Some(v) if v.is_string() => {
            let n = v.clone().into_string().unwrap_or_default();
            if !has_fn(&n) {
                return Err(format!("evaluate function {n}(ctx, params) is not defined"));
            }
            FnPtr::new(n).map_err(|e| e.to_string())?
        }
        Some(_) => return Err("\"evaluate\" must be a function name or a function pointer".into()),
        None => {
            let guess = snake_case(name);
            if has_fn(&guess) {
                FnPtr::new(guess).map_err(|e| e.to_string())?
            } else if has_fn("evaluate") {
                FnPtr::new("evaluate").map_err(|e| e.to_string())?
            } else {
                return Err(format!("no evaluate function: define fn {guess}(ctx, params) or set evaluate: \"fn_name\""));
            }
        }
    };
    let type_id = super::type_id(tab, name);
    let icon = if icon.is_empty() { "custom-feature".to_string() } else { icon };
    let feature_type = schema::build_type(&type_id, name, &icon, &description, &params);
    Ok(CustomFeatureDef { name: name.to_string(), type_id, description, icon, params, feature_type, evaluate })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake() {
        assert_eq!(snake_case("Bolt pattern"), "bolt_pattern");
        assert_eq!(snake_case("  My-Feature 2 "), "my_feature_2");
    }

    #[test]
    fn parse_error_has_line() {
        let c = compile(TabId(1), "let x = 1;\nlet y = ;\n");
        assert!(c.has_errors());
        assert_eq!(c.diagnostics[0].line, 2);
    }

    #[test]
    fn declares_features() {
        let src = r#"
            feature("Thing", #{ params: [ length("d", "Depth", mm(5)), int("n", "Count", 3), enum_("mode", "Mode", ["A", "B"]), query("f", "Faces", FACE, 1) ] });
            fn thing(ctx, p) { }
        "#;
        let c = compile(TabId(2), src);
        assert!(!c.has_errors(), "{:?}", c.diagnostics);
        assert_eq!(c.features.len(), 1);
        let t = c.features[0].feature_type;
        assert_eq!(t.params.len(), 4);
        assert_eq!(t.id, format!("custom:{}:Thing", TabId(2)));
    }

    #[test]
    fn missing_evaluate_is_error() {
        let c = compile(TabId(3), r#"feature("Nope", #{ params: [] });"#);
        assert!(c.has_errors());
        assert!(c.diagnostics[0].message.contains("no evaluate function"), "{:?}", c.diagnostics);
    }

    #[test]
    fn infinite_loop_is_stopped() {
        let c = compile(TabId(4), "loop { }");
        assert!(c.has_errors());
        assert!(c.diagnostics[0].message.contains("operation limit"), "{:?}", c.diagnostics);
    }

    #[test]
    fn no_modules() {
        let c = compile(TabId(5), r#"import "/etc/passwd" as x;"#);
        assert!(c.has_errors());
    }
}
