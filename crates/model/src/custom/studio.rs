//! Compiling a Feature Studio: the FeatureScript module, its features
//! (`annotation { "Feature Type Name" : ... } export const f =
//! defineFeature(...)`), and their dialogs built from the preconditions
//! exactly as Onshape builds them.

use super::host;
use crate::document::TabId;
use crate::features::{Category, FeatureType};
use crate::params::{Hints, NumKind, NumSpec, ParamKind, ParamSpec, ParamValue, ParamValues, QueryFilter};
use noshape_featurescript::annot::{self, Ann};
use noshape_featurescript::ast::{self, ExprKind, Name, StmtKind, TopKind, UnOp};
use noshape_featurescript::compile::{CompiledModule, DeclKind, Resolver};
use noshape_featurescript::interp::Interp;
use noshape_featurescript::ir::{CompiledFn, GlobalCell, TypeRef};
use noshape_featurescript::value::{Map, Tag, Value, type_info};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};

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

// ---------------------------------------------------------------- compile

fn source_hash(source: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut h);
    h.finish()
}

type Cache = HashMap<(u64, u64), Arc<CompiledStudio>>;
static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(Default::default);

/// Module path of a Feature Studio tab (what imports refer to).
pub fn module_path(tab: TabId) -> String {
    format!("tab:{tab}")
}

/// Compile a Feature Studio without imports of other tabs.
pub fn compile(tab: TabId, source: &str) -> Arc<CompiledStudio> {
    compile_with(tab, source, &noshape_featurescript::compile::NoImports, 0)
}

/// Compile a Feature Studio (cached by tab, source and `deps`, a hash of
/// the sources of the tabs it may import).
pub fn compile_with(tab: TabId, source: &str, resolver: &dyn Resolver, deps: u64) -> Arc<CompiledStudio> {
    host::register();
    let hash = source_hash(source) ^ deps.rotate_left(17);
    if let Some(c) = CACHE.lock().get(&(tab.0, hash)) {
        return c.clone();
    }
    let compiled = Arc::new(compile_uncached(tab, source, hash, resolver));
    let mut cache = CACHE.lock();
    if cache.len() > 128 {
        cache.clear();
    }
    cache.insert((tab.0, hash), compiled.clone());
    compiled
}

fn compile_uncached(tab: TabId, source: &str, hash: u64, resolver: &dyn Resolver) -> CompiledStudio {
    let path = module_path(tab);
    let mut out = CompiledStudio { tab, hash, features: Vec::new(), tables: Vec::new(), diagnostics: Vec::new(), module: None };
    let diagnostic = |e: &noshape_featurescript::CompileError| Diagnostic {
        severity: match e.level {
            noshape_featurescript::Level::Error => Severity::Error,
            noshape_featurescript::Level::Warning => Severity::Warning,
            noshape_featurescript::Level::Info => Severity::Info,
        },
        line: e.pos.line as usize,
        column: e.pos.col as usize,
        message: e.message.clone(),
        parse: e.parse,
    };
    let module = match noshape_featurescript::compile_module(&path, source, resolver) {
        Ok(m) => m,
        Err(errs) => {
            out.diagnostics.extend(errs.iter().map(diagnostic));
            return out;
        }
    };
    out.diagnostics.extend(module.warnings.iter().map(diagnostic));
    for d in module.decls.iter() {
        let DeclKind::Const(cell) = &d.kind else { continue };
        if !d.export {
            continue;
        }
        let Some(item) = module.ast.items.iter().find(|it| matches!(&it.kind, TopKind::Const { name, .. } if *name == d.name)) else { continue };
        let Some(ann) = item.annotation.as_ref().map(|a| annot::eval(&module, a)) else { continue };
        let TopKind::Const { init, .. } = &item.kind else { continue };
        if let Some(display) = ann.str("Table Type Name").map(|s| s.to_string()) {
            let lambda = defined_lambda(init, "defineTable");
            // Live: a table function without a precondition is only warned
            // about, and is not offered anywhere.
            let Some(p) = lambda.and_then(|f| f.precondition.as_ref()) else {
                if let Some(f) = lambda {
                    out.diagnostics.push(Diagnostic { severity: Severity::Warning, line: f.pos.line as usize, column: f.pos.col as usize, message: format!("Nonconforming table function '{}': Missing precondition", d.name), parse: false });
                }
                continue;
            };
            let mut params = Vec::new();
            collect_params(&module, p, &mut params);
            out.tables.push(CustomTableDef {
                name: d.name.to_string(),
                display_name: display,
                description: ann.str("Table Type Description").unwrap_or("").to_string(),
                params,
                cell: cell.clone(),
            });
            continue;
        }
        let Some(display) = ann.str("Feature Type Name").map(|s| s.to_string()) else { continue };
        let precondition = feature_lambda(init).and_then(|f| f.precondition.as_ref());
        let mut params = Vec::new();
        if let Some(p) = precondition {
            collect_params(&module, p, &mut params);
        }
        let type_id = super::type_id(tab, &d.name);
        let description = ann.str("Feature Type Description").unwrap_or("").to_string();
        let icon = "custom-feature".to_string();
        let feature_type = build_type(&type_id, &display, &icon, &description, &params, hash);
        out.features.push(CustomFeatureDef {
            name: d.name.to_string(),
            display_name: display,
            type_id,
            description,
            icon,
            params,
            feature_type,
            cell: cell.clone(),
            editing_logic: ann.str("Editing Logic Function").map(|s| s.to_string()),
        });
    }
    // Live: "Importing geometry.fs when defining tables will likely hurt
    // performance; …" at the import, when the studio defines tables.
    let defines_tables = module.ast.items.iter().any(|it| it.annotation.as_ref().is_some_and(|a| annot::eval(&module, a).str("Table Type Name").is_some()));
    if defines_tables {
        for it in &module.ast.items {
            if let TopKind::Import { path, .. } = &it.kind
                && path.as_str() == "onshape/std/geometry.fs"
            {
                // Imports come first in the file: so does their notice.
                out.diagnostics.insert(0, Diagnostic {
                    severity: Severity::Warning,
                    line: it.pos.line as usize,
                    column: it.pos.col as usize,
                    message: "Importing geometry.fs when defining tables will likely hurt performance; consider importing table.fs and specific required modules instead.".into(),
                    parse: false,
                });
            }
        }
    }
    out.module = Some(module);
    out
}

/// The lambda passed to `defineFeature(...)`.
fn feature_lambda(e: &ast::Expr) -> Option<&ast::FnDef> {
    defined_lambda(e, "defineFeature")
}

/// The lambda passed to `define(...)` (`defineFeature`, `defineTable`).
pub(crate) fn defined_lambda<'a>(e: &'a ast::Expr, define: &str) -> Option<&'a ast::FnDef> {
    match &*e.kind {
        ExprKind::Call(f, args) if matches!(&*f.kind, ExprKind::Ident(q) if &*q.name == define) => match args.first().map(|a| &*a.kind) {
            Some(ExprKind::Lambda(def)) => Some(def),
            _ => None,
        },
        _ => None,
    }
}

// ---------------------------------------------------------------- preconditions

/// `definition.x` -> "x" (and `item.x` inside array parameter loops).
/// A FeatureScript lookup table (`{ "name", "displayName", "default",
/// "entries" : { key : sub-table or leaf } }`) as a run-time table. `expr`
/// (the table's source, when known) gives the entries' source order, which
/// is Onshape's dropdown order (live L58; the evaluated map sorts its keys)
/// and the default of a level without one.
fn dyn_level(module: &Arc<CompiledModule>, v: &Value, expr: Option<&ast::Expr>) -> Option<crate::features::hole_tables::DynLevel> {
    let entries = v.get("entries").as_map()?;
    let source = expr.and_then(|e| source_entries(module, e));
    let all = v.get("entries");
    // (key, sub-table, its source expression and module)
    type Entry = (Value, Value, Option<(Arc<CompiledModule>, ast::Expr)>);
    let ordered: Vec<Entry> = match &source {
        // The source's keys are exactly the evaluated ones.
        Some((m, list)) if list.len() == entries.len() && list.iter().all(|(k, _)| !all.get(k.as_str()).is_undefined()) => {
            list.iter().map(|(k, x)| (Value::str(k.as_str()), all.get(k.as_str()).clone(), Some((m.clone(), x.clone())))).collect()
        }
        _ => entries.iter().map(|(k, x)| (k.clone(), x.clone(), None)).collect(),
    };
    let mut out = Vec::new();
    for (k, sub, sub_expr) in &ordered {
        let key = k.as_str()?.to_string();
        let child = if sub.get("entries").as_map().is_some() {
            let (m, x) = match sub_expr {
                Some((m, x)) => (m.clone(), Some(x)),
                None => (module.clone(), None),
            };
            dyn_level(&m, sub, x).map(Arc::new)
        } else {
            None
        };
        out.push((key, child));
    }
    let default = v.get("default").as_str().map(|s| s.to_string()).or_else(|| out.first().map(|e| e.0.clone()))?;
    let key = v.get("name").as_str().map(|s| s.to_string()).unwrap_or_default();
    let name = intern(&v.get("displayName").as_str().map(|s| s.to_string()).unwrap_or_else(|| key.clone()));
    Some(crate::features::hole_tables::DynLevel { key, name, default, entries: out })
}

/// The value of `key` in a map literal expression.
fn map_entry<'a>(e: &'a ast::Expr, key: &str) -> Option<&'a ast::Expr> {
    let ExprKind::Map(entries) = &*e.kind else { return None };
    entries.iter().find(|(k, _)| matches!(k, ast::MapKey::Str(s) if &**s == key)).map(|(_, v)| v)
}

/// A constant's definition: the module declaring it and its initializer.
fn const_source(module: &Arc<CompiledModule>, name: &str) -> Option<(Arc<CompiledModule>, ast::Expr)> {
    let cell = module.constant(name)?;
    let mut todo = vec![module.clone()];
    let mut seen = std::collections::HashSet::new();
    while let Some(m) = todo.pop() {
        if !seen.insert(m.path.clone()) {
            continue;
        }
        if m.path == cell.module {
            return m.ast.items.iter().find_map(|it| match &it.kind {
                TopKind::Const { name: n, init, .. } if &**n == name => Some((m.clone(), init.clone())),
                _ => None,
            });
        }
        todo.extend(m.imports.iter().cloned());
    }
    None
}

/// A table expression's "entries" in source order: (key, value expression),
/// with the module the expressions belong to. `None` unless every key is a
/// literal (constants are followed to their definitions).
#[allow(clippy::type_complexity)]
fn source_entries(module: &Arc<CompiledModule>, e: &ast::Expr) -> Option<(Arc<CompiledModule>, Vec<(String, ast::Expr)>)> {
    let resolve = |m: &Arc<CompiledModule>, e: &ast::Expr| -> Option<(Arc<CompiledModule>, ast::Expr)> {
        match &*e.kind {
            ExprKind::Ident(q) if q.ns.is_none() => const_source(m, &q.name),
            ExprKind::Map(_) => Some((m.clone(), e.clone())),
            _ => None,
        }
    };
    let (m, table) = resolve(module, e)?;
    let (m, entries) = resolve(&m, map_entry(&table, "entries")?)?;
    let ExprKind::Map(list) = &*entries.kind else { return None };
    let mut out = Vec::new();
    for (k, x) in list {
        let ast::MapKey::Str(k) = k else { return None };
        out.push((k.to_string(), x.clone()));
    }
    Some((m, out))
}

fn value_hash(v: &Value) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    noshape_featurescript::value::to_display(v).hash(&mut h);
    h.finish()
}

fn param_path(module: &CompiledModule, e: &ast::Expr, roots: &[&str]) -> Option<String> {
    let is_root = |b: &ast::Expr| matches!(&*b.kind, ExprKind::Ident(q) if q.ns.is_none() && (roots.contains(&&*q.name) || &*q.name == "definition"));
    match &*e.kind {
        ExprKind::Member(b, name, _) if is_root(b) => Some(name.to_string()),
        // `definition[FILLET_WIDTH]`, `profile[uOrV ~ CONDITION]` (after
        // predicate arguments are substituted): a constant key.
        ExprKind::Index(b, key, _) if is_root(b) => eval_value(module, key).and_then(|v| v.as_str()).map(|s| s.to_string()),
        _ => None,
    }
}

/// `e` with identifiers bound in `env` replaced (inlined predicate arguments).
fn subst_expr(e: &ast::Expr, env: &[(Name, ast::Expr)]) -> ast::Expr {
    if env.is_empty() {
        return e.clone();
    }
    let r = |x: &ast::Expr| subst_expr(x, env);
    let k = match &*e.kind {
        ExprKind::Ident(q) if q.ns.is_none() => match env.iter().find(|(n, _)| *n == q.name) {
            Some((_, v)) => return v.clone(),
            None => return e.clone(),
        },
        ExprKind::Array(v) => ExprKind::Array(v.iter().map(r).collect()),
        ExprKind::Map(v) => ExprKind::Map(v.iter().map(|(k, x)| (k.clone(), r(x))).collect()),
        ExprKind::Unary(o, x) => ExprKind::Unary(*o, r(x)),
        ExprKind::Binary(o, a, b) => ExprKind::Binary(*o, r(a), r(b)),
        ExprKind::Cond(c, a, b) => ExprKind::Cond(r(c), r(a), r(b)),
        ExprKind::Is(x, q) => ExprKind::Is(r(x), q.clone()),
        ExprKind::As(x, q) => ExprKind::As(r(x), q.clone()),
        ExprKind::Call(f, a) => ExprKind::Call(r(f), a.iter().map(r).collect()),
        ExprKind::Arrow(x, q, a) => ExprKind::Arrow(r(x), q.clone(), a.iter().map(r).collect()),
        ExprKind::Index(x, i, s) => ExprKind::Index(r(x), r(i), *s),
        ExprKind::Member(x, n, s) => ExprKind::Member(r(x), n.clone(), *s),
        ExprKind::BoxGet(x, s) => ExprKind::BoxGet(r(x), *s),
        ExprKind::Try(s, x) => ExprKind::Try(*s, r(x)),
        _ => return e.clone(),
    };
    ast::Expr::new(e.pos, k)
}

/// A precondition statement with `env` substituted (see [`subst_expr`]).
fn subst_stmt(s: &ast::Stmt, env: &[(Name, ast::Expr)]) -> ast::Stmt {
    if env.is_empty() {
        return s.clone();
    }
    let kind = match &s.kind {
        StmtKind::Block(v) => StmtKind::Block(v.iter().map(|x| subst_stmt(x, env)).collect()),
        StmtKind::Expr(e) => StmtKind::Expr(subst_expr(e, env)),
        StmtKind::Annotated { annotation, stmt } => StmtKind::Annotated { annotation: subst_expr(annotation, env), stmt: Box::new(subst_stmt(stmt, env)) },
        StmtKind::If { cond, then, otherwise } => StmtKind::If { cond: subst_expr(cond, env), then: Box::new(subst_stmt(then, env)), otherwise: otherwise.as_ref().map(|o| Box::new(subst_stmt(o, env))) },
        StmtKind::ForIn { declare, key, value, iter, body } => StmtKind::ForIn { declare: *declare, key: key.clone(), value: value.clone(), iter: subst_expr(iter, env), body: Box::new(subst_stmt(body, env)) },
        other => other.clone(),
    };
    ast::Stmt { pos: s.pos, kind, text: s.text.clone() }
}

fn compile_condition(module: &CompiledModule, cond: &ast::Expr, root: &str) -> Option<Arc<CompiledFn>> {
    module.compile_lambda(&[root], cond).ok()
}

pub(crate) fn collect_params(module: &Arc<CompiledModule>, s: &ast::Stmt, out: &mut Vec<ParamDecl>) {
    collect_in(module, s, &[], None, &[], out)
}

/// `ann`: the annotation on `s` (evaluated, and its expression).
fn collect_in(module: &Arc<CompiledModule>, s: &ast::Stmt, conds: &[Arc<CompiledFn>], ann: Option<(&Ann, &ast::Expr)>, roots: &[&str], out: &mut Vec<ParamDecl>) {
    let root = "definition";
    match &s.kind {
        StmtKind::Block(stmts) => {
            let mut k = 0;
            while k < stmts.len() {
                // An array parameter: `definition.items is array;` followed by
                // `for (var item in definition.items) { ... }`.
                if let StmtKind::Annotated { annotation, stmt } = &stmts[k].kind
                    && let StmtKind::Expr(e) = &stmt.kind
                    && let ExprKind::Is(x, q) = &*e.kind
                    && &*q.name == "array"
                    && let Some(id) = param_path(module, x, roots)
                {
                    let a = annot::eval(module, annotation);
                    let mut items = Vec::new();
                    if let Some(next) = stmts.get(k + 1)
                        && let StmtKind::ForIn { value, body, .. } = &next.kind
                    {
                        let r = value.to_string();
                        collect_in(module, body, &[], None, &[r.as_str()], &mut items);
                        k += 1;
                    }
                    out.push(ParamDecl {
                        label: a.str("Name").unwrap_or(&id).to_string(),
                        id,
                        kind: ParamKindFs::Array { item_label: a.str("Item name").unwrap_or("").to_string(), template: a.str("Item label template").map(str::to_string), items },
                        hints: hints(&a),
                        description: a.str("Description").unwrap_or("").to_string(),
                        conditions: conds.to_vec(),
                    });
                    k += 1;
                    continue;
                }
                collect_in(module, &stmts[k], conds, None, roots, out);
                k += 1;
            }
        }
        StmtKind::Annotated { annotation, stmt } => {
            let a = annot::eval(module, annotation);
            // A group titled on its own (a "Driving Parameter" group's title
            // is that boolean, already a parameter).
            if let Some(title) = a.str("Group Name")
                && a.get("Driving Parameter").is_none()
                && matches!(stmt.kind, StmtKind::Block(_))
            {
                // Its id as the built-in schemas name groups ("generalGroup",
                // "edgeProjectionOptionsGroup").
                let mut id = String::new();
                for (k, w) in title.split_whitespace().enumerate() {
                    let mut c = w.chars();
                    if let Some(f) = c.next() {
                        id.extend(if k == 0 { f.to_lowercase().collect::<Vec<_>>() } else { f.to_uppercase().collect() });
                        id.push_str(c.as_str());
                    }
                }
                id.push_str("Group");
                let open = !a.get("Collapsed By Default").and_then(|v| v.as_bool()).unwrap_or(false);
                out.push(ParamDecl {
                    id: id.clone(),
                    label: title.to_string(),
                    kind: ParamKindFs::Group { open },
                    hints: Hints::GROUP_HEADER | Hints::GROUP_LABEL | Hints::UNCONFIGURABLE,
                    description: String::new(),
                    conditions: conds.to_vec(),
                });
                let r = roots.first().copied().unwrap_or(root);
                let gate = ast::Expr::new(
                    stmt.pos,
                    ExprKind::Member(ast::Expr::new(stmt.pos, ExprKind::Ident(ast::QualName { ns: None, name: r.into() })), id.as_str().into(), false),
                );
                let mut with = conds.to_vec();
                with.extend(compile_condition(module, &gate, r));
                collect_in(module, stmt, &with, None, roots, out);
                return;
            }
            // A "Driving Parameter" group: its boolean heads the group (a
            // checkbox title over the group's fields, as built-in dialogs).
            if a.str("Group Name").is_some()
                && let Some(driver) = a.str("Driving Parameter")
                && let Some(p) = out.iter_mut().rev().find(|p| p.id == driver)
            {
                p.hints = p.hints | Hints::GROUP_HEADER;
            }
            collect_in(module, stmt, conds, Some((&a, annotation)), roots, out);
        }
        StmtKind::If { cond, then, otherwise } => {
            let r = roots.first().copied().unwrap_or(root);
            let c = compile_condition(module, cond, r);
            let mut with = conds.to_vec();
            with.extend(c);
            collect_in(module, then, &with, None, roots, out);
            if let Some(o) = otherwise {
                let neg = ast::Expr::new(cond.pos, ExprKind::Unary(UnOp::Not, cond.clone()));
                let mut with = conds.to_vec();
                with.extend(compile_condition(module, &neg, r));
                collect_in(module, o, &with, None, roots, out);
            }
        }
        StmtKind::Expr(e) => {
            let Some((a, ann_expr)) = ann else {
                // A predicate holding parameters: `myParameters(definition);`.
                if let ExprKind::Call(f, args) = &*e.kind
                    && let ExprKind::Ident(q) = &*f.kind
                    && let Some(ExprKind::Ident(arg)) = args.first().map(|x| &*x.kind)
                    && (roots.contains(&&*arg.name) || &*arg.name == "definition")
                    && let Some((owner, def)) = find_predicate(module, &q.name, args.len())
                    && let ast::Body::Block(stmts) = &def.body
                    && let Some(p0) = def.params.first()
                {
                    // Further arguments (`isProfile(profile, "u")`) are
                    // substituted into the predicate's body.
                    let env: Vec<(Name, ast::Expr)> = def.params.iter().zip(args.iter()).skip(1).map(|(p, a)| (p.name.clone(), a.clone())).collect();
                    let r = p0.name.to_string();
                    let block = subst_stmt(&ast::Stmt { pos: def.pos, kind: StmtKind::Block(stmts.clone()), text: None }, &env);
                    collect_in(&owner, &block, conds, None, &[r.as_str()], out);
                }
                return;
            };
            if let Some(p) = param_decl(module, e, a, ann_expr, roots) {
                out.push(ParamDecl { conditions: conds.to_vec(), ..p });
            }
        }
        _ => {}
    }
}

/// A predicate declaration visible in `module` (own or imported).
fn find_predicate(module: &Arc<CompiledModule>, name: &str, arity: usize) -> Option<(Arc<CompiledModule>, Arc<ast::FnDef>)> {
    fn find(m: &Arc<CompiledModule>, name: &str, arity: usize, seen: &mut HashSet<usize>) -> Option<(Arc<CompiledModule>, Arc<ast::FnDef>)> {
        for d in &m.decls {
            if let DeclKind::Function(_, def) = &d.kind
                && &*def.name == name
                && def.kind == ast::FnKind::Predicate
                && def.params.len() == arity
            {
                return Some((m.clone(), def.clone()));
            }
        }
        for imp in &m.imports {
            if seen.insert(Arc::as_ptr(imp) as usize)
                && let Some(r) = find(imp, name, arity, seen)
            {
                return Some(r);
            }
        }
        None
    }
    find(module, name, arity, &mut HashSet::new())
}

fn hints(a: &Ann) -> Hints {
    let mut h = Hints::NONE;
    for m in a.get("UIHint").map(|x| x.members()).unwrap_or_default() {
        h = h | match m.as_str() {
            "HORIZONTAL_ENUM" => Hints::HORIZONTAL_ENUM,
            "SHOW_LABEL" => Hints::SHOW_LABEL,
            "REMEMBER_PREVIOUS_VALUE" => Hints::REMEMBER_PREVIOUS_VALUE,
            "OPPOSITE_DIRECTION" => Hints::OPPOSITE_DIRECTION,
            "OPPOSITE_DIRECTION_CIRCULAR" => Hints::OPPOSITE_DIRECTION_CIRCULAR,
            "DISPLAY_SHORT" => Hints::DISPLAY_SHORT,
            "FIRST_IN_ROW" => Hints::FIRST_IN_ROW,
            "READ_ONLY" => Hints::READ_ONLY,
            "UNCONFIGURABLE" => Hints::UNCONFIGURABLE,
            "ALLOW_QUERY_ORDER" => Hints::ALLOW_QUERY_ORDER,
            "INITIAL_FOCUS" => Hints::INITIAL_FOCUS,
            "INITIAL_FOCUS_ON_EDIT" => Hints::INITIAL_FOCUS_ON_EDIT,
            "FOCUS_ON_VISIBLE" => Hints::FOCUS_ON_VISIBLE,
            "ALLOW_ARRAY_FOCUS" => Hints::ALLOW_ARRAY_FOCUS,
            "PREVENT_ARRAY_REORDER" => Hints::PREVENT_ARRAY_REORDER,
            "COLLAPSE_ARRAY_ITEMS" => Hints::COLLAPSE_ARRAY_ITEMS,
            "MATCH_LAST_ARRAY_ITEM" => Hints::MATCH_LAST_ARRAY_ITEM,
            "CAN_BE_TOLERANT" => Hints::CAN_BE_TOLERANT,
            "CAN_BE_TOLERANT_DIAMETER" => Hints::CAN_BE_TOLERANT_DIAMETER,
            "ALWAYS_HIDDEN" => Hints::ALWAYS_HIDDEN,
            "SHOW_CREATE_SELECTION" => Hints::SHOW_CREATE_SELECTION,
            "PRIMARY_AXIS" => Hints::PRIMARY_AXIS,
            "MATE_CONNECTOR_AXIS_TYPE" => Hints::MATE_CONNECTOR_AXIS_TYPE,
            "FOCUS_INNER_QUERY" | "PREVENT_CREATING_NEW_MATE_CONNECTORS" => Hints::NONE,
            _ => Hints::NONE,
        };
    }
    h
}

/// Evaluate a bound spec expression (`LENGTH_BOUNDS`, `{ (millimeter) : [..] } as LengthBoundSpec`).
fn eval_value(module: &CompiledModule, e: &ast::Expr) -> Option<Value> {
    let f = module.compile_lambda(&[], e).ok()?;
    Interp::new().invoke(&f, Vec::new(), &[], false).ok()
}

/// (min, default metric, default imperial, max) in SI from a bound spec.
fn bounds(spec: &Value, kind: NumKind) -> (f64, f64, f64, f64) {
    let mut min = f64::NEG_INFINITY;
    let mut max = f64::INFINITY;
    let mut metric: Option<f64> = None;
    let mut imperial: Option<f64> = None;
    let mut any: Option<f64> = None;
    if let Some(m) = spec.as_map() {
        for (unit, v) in m.iter() {
            // Key: a ValueWithUnits (unit scale) or `unitless` (1).
            let scale = host::magnitude_of(unit).unwrap_or(1.0);
            let unit_name = unit_name(unit);
            let def = match v {
                Value::Array(a, _) if a.len() == 3 => {
                    min = a[0].as_number().unwrap_or(min) * scale;
                    max = a[2].as_number().unwrap_or(max) * scale;
                    a[1].as_number()
                }
                other => other.as_number(),
            };
            let Some(d) = def.map(|d| d * scale) else { continue };
            any.get_or_insert(d);
            match unit_name.as_deref() {
                Some("millimeter") => metric = Some(d),
                Some("inch") => imperial = Some(d),
                Some("meter" | "centimeter") if metric.is_none() => metric = Some(d),
                Some("degree") => {
                    metric = Some(d);
                    imperial = Some(d)
                }
                _ => {}
            }
        }
    }
    let _ = kind;
    let d = any.unwrap_or(0.0);
    (min, metric.unwrap_or(d), imperial.or(metric).unwrap_or(d), max)
}

/// "millimeter" for `millimeter`-valued keys (matched by scale).
fn unit_name(v: &Value) -> Option<String> {
    let x = host::magnitude_of(v)?;
    let units = v.get("unit").as_map().map(|m| m.keys().filter_map(|k| k.as_str()).map(|s| s.to_string()).collect::<Vec<_>>()).unwrap_or_default();
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * b.abs().max(1e-12);
    if units == ["meter"] {
        return Some(
            if close(x, 1.0) {
                "meter"
            } else if close(x, 0.01) {
                "centimeter"
            } else if close(x, 0.001) {
                "millimeter"
            } else if close(x, 0.0254) {
                "inch"
            } else if close(x, 0.3048) {
                "foot"
            } else if close(x, 0.9144) {
                "yard"
            } else {
                "?"
            }
            .to_string(),
        );
    }
    if units == ["radian"] {
        return Some(if close(x, 1.0) { "radian" } else { "degree" }.to_string());
    }
    None
}

fn default_bounds(kind: NumKind) -> &'static str {
    match kind {
        NumKind::Length => "LENGTH_BOUNDS",
        NumKind::Angle => "ANGLE_360_BOUNDS",
        NumKind::Integer => "POSITIVE_COUNT_BOUNDS",
        NumKind::Real => "POSITIVE_REAL_BOUNDS",
    }
}

fn param_decl(module: &Arc<CompiledModule>, e: &ast::Expr, a: &Ann, ann_expr: &ast::Expr, roots: &[&str]) -> Option<ParamDecl> {
    let (id, kind): (String, ParamKindFs) = match &*e.kind {
        // definition.x is Query / boolean / string / SomeEnum
        ExprKind::Is(x, q) => {
            let id = param_path(module, x, roots)?;
            let kind = match &*q.name {
                "Query" => ParamKindFs::Query { filter: filter_of(a.get("Filter")), max_picks: a.get("MaxNumberOfPicks").and_then(|v| v.as_num()).map(|n| n as usize) },
                // Feature pattern's "Features to pattern".
                "FeatureList" => ParamKindFs::FeatureList { max_picks: a.get("MaxNumberOfPicks").and_then(|v| v.as_num()).map(|n| n as usize) },
                "TableData" | "ImageData" | "JSONData" | "TextData" | "PartStudioData" | "VariableStudioData" | "CADImportData" => {
                    use crate::params::ReferenceKind as K;
                    let kind = match &*q.name {
                        "TableData" => K::Csv,
                        "ImageData" => K::Image,
                        "JSONData" => K::Json,
                        "TextData" => K::Text,
                        "PartStudioData" => K::PartStudio,
                        "VariableStudioData" => K::VariableStudio,
                        _ => K::CadImport,
                    };
                    ParamKindFs::Reference { kind }
                }
                "LookupTablePath" => {
                    let Some(annot::Ann::Value(table)) = a.get("Lookup Table") else { return None };
                    let level = dyn_level(module, table, map_entry(ann_expr, "Lookup Table"))?;
                    let name = intern(&format!("fs:{}:{id}:{:x}", module.path, value_hash(table)));
                    crate::features::hole_tables::register_dynamic_table(name, level);
                    ParamKindFs::Lookup { table: name }
                }
                "boolean" => ParamKindFs::Bool { default: a.get("Default").and_then(|v| v.as_bool()).unwrap_or(false) },
                "string" => ParamKindFs::Text { default: a.str("Default").unwrap_or("").to_string() },
                _ => {
                    let tag = match module.scope.symbols.get(&q.name).and_then(|s| s.ty) {
                        Some(TypeRef::Tag(t)) if type_info(t).is_enum => t,
                        _ => return None,
                    };
                    let (owner, members) = annot::enum_members(module, tag)?;
                    let mut options = Vec::new();
                    for (m, ann) in &members {
                        let ma = ann.as_ref().map(|x| annot::eval(&owner, x));
                        if ma.as_ref().and_then(|x| x.get("Hidden")).and_then(|h| h.as_bool()).unwrap_or(false) {
                            continue;
                        }
                        let name = ma.as_ref().and_then(|x| x.str("Name")).map(|s| s.to_string()).unwrap_or_else(|| m.to_string());
                        options.push((m.to_string(), name));
                    }
                    let default = a.get("Default").map(|d| d.members()).and_then(|m| m.first().cloned()).or_else(|| options.first().map(|o| o.0.clone()))?;
                    ParamKindFs::Enum { tag, options, default }
                }
            };
            (id, kind)
        }
        // isLength(definition.x, BOUNDS), isAngle, isInteger, isReal, isAnything
        ExprKind::Call(f, args) => {
            let ExprKind::Ident(q) = &*f.kind else { return None };
            let id = param_path(module, args.first()?, roots)?;
            let nk = match &*q.name {
                "isLength" => NumKind::Length,
                "isAngle" => NumKind::Angle,
                "isInteger" => NumKind::Integer,
                "isReal" => NumKind::Real,
                "isAnything" => return Some(ParamDecl { label: a.str("Name").unwrap_or(&id).to_string(), id, kind: ParamKindFs::Anything, hints: hints(a), description: String::new(), conditions: Vec::new() }),
                _ => return None,
            };
            let spec = match args.get(1) {
                Some(b) => eval_value(module, b),
                None => module.constant(default_bounds(nk)).and_then(|c| Interp::new().global(&c).ok()),
            }
            .unwrap_or_default();
            let (min, dm, di, max) = bounds(&spec, nk);
            let spec = NumSpec { kind: nk, default_metric: dm, default_imperial: di, min, max };
            (id, ParamKindFs::Num { kind: nk, spec })
        }
        _ => return None,
    };
    Some(ParamDecl { label: a.str("Name").unwrap_or(&id).to_string(), id, kind, hints: hints(a), description: a.str("Description").unwrap_or("").to_string(), conditions: Vec::new() })
}

/// Our selection filter for an Onshape "Filter" expression.
fn filter_of(f: Option<&Ann>) -> QueryFilter {
    let Some(f) = f else { return QueryFilter(QueryFilter::ANY_FACE.0 | QueryFilter::ANY_EDGE.0 | QueryFilter::POINT_LIKE.0 | QueryFilter::BODY.0 | QueryFilter::SKETCH_REGION.0 | QueryFilter::PLANE.0) };
    match f {
        // A condition we have no bit for (BodyType.COMPOSITE …) adds nothing.
        Ann::Or(a, b) if matches!(&**b, Ann::Enum(t, m) if enum_filter(t, m).is_none()) => filter_of(Some(a)),
        Ann::Or(a, b) if matches!(&**a, Ann::Enum(t, m) if enum_filter(t, m).is_none()) => filter_of(Some(b)),
        Ann::Or(a, b) => QueryFilter(filter_of(Some(a)).0 | filter_of(Some(b)).0),
        // `… && SketchObject.NO`: the other side without its sketch entities
        // (fillet.fs, chamfer.fs: model edges only, qa5 SEL1).
        Ann::And(a, b) if [a, b].iter().any(|s| matches!(&***s, Ann::Enum(t, m) if t == "SketchObject" && m == "NO")) => {
            let other = if matches!(&**a, Ann::Enum(t, _) if t == "SketchObject") { b } else { a };
            QueryFilter(filter_of(Some(other)).0 & !(QueryFilter::SKETCH_REGION.0 | QueryFilter::SKETCH_EDGE.0 | QueryFilter::SKETCH_POINT.0 | QueryFilter::SKETCH.0))
        }
        // A condition we have no bit for (EdgeTopology.TWO_SIDED,
        // ConstructionObject.NO, AllowMeshGeometry.YES …) narrows nothing.
        Ann::And(a, b) if matches!(&**b, Ann::Enum(t, m) if enum_filter(t, m).is_none()) => filter_of(Some(a)),
        Ann::And(a, b) if matches!(&**a, Ann::Enum(t, m) if enum_filter(t, m).is_none()) => filter_of(Some(b)),
        Ann::And(a, b) => {
            let (x, y) = (filter_of(Some(a)), filter_of(Some(b)));
            // Refinements: EDGE && LINE -> linear edges, FACE && PLANE -> planar faces...
            let i = QueryFilter(x.0 & y.0);
            if i.0 != 0 { i } else { if x.0.count_ones() <= y.0.count_ones() { x } else { y } }
        }
        Ann::Not(_) => QueryFilter(QueryFilter::ANY_FACE.0 | QueryFilter::ANY_EDGE.0 | QueryFilter::POINT_LIKE.0 | QueryFilter::BODY.0),
        Ann::Enum(ty, m) => enum_filter(ty, m).unwrap_or(QueryFilter(QueryFilter::ANY_FACE.0 | QueryFilter::ANY_EDGE.0 | QueryFilter::POINT_LIKE.0 | QueryFilter::BODY.0 | QueryFilter::SKETCH_REGION.0 | QueryFilter::PLANE.0)),
        _ => QueryFilter(QueryFilter::ANY_FACE.0 | QueryFilter::ANY_EDGE.0 | QueryFilter::POINT_LIKE.0 | QueryFilter::BODY.0),
    }
}

/// Our selection filter for one Onshape filter enum member; `None` for a
/// condition we have no bit for (it narrows nothing in an `&&`).
fn enum_filter(ty: &str, m: &str) -> Option<QueryFilter> {
    // A body's edges: any model edge (GeometryType.LINE && BodyType.SOLID is a part's linear edge).
    const MODEL_EDGES: u32 = QueryFilter::EDGE.0 | QueryFilter::LINEAR_EDGE.0 | QueryFilter::CIRCULAR_EDGE.0;
    let any = |items: &[(&str, &str)]| items.iter().filter_map(|(t, m)| enum_filter(t, m)).fold(QueryFilter(0), |a, f| QueryFilter(a.0 | f.0));
    Some(match (ty, m) {
        ("EntityType", "FACE") => QueryFilter(QueryFilter::ANY_FACE.0 | QueryFilter::SKETCH_REGION.0 | QueryFilter::PLANE.0),
        ("EntityType", "EDGE") => QueryFilter::ANY_EDGE,
        ("EntityType", "VERTEX") => QueryFilter(QueryFilter::VERTEX.0 | QueryFilter::SKETCH_POINT.0 | QueryFilter::MATE_CONNECTOR.0),
        ("EntityType", "BODY") => QueryFilter(QueryFilter::BODY.0 | QueryFilter::WIRE_BODY.0 | QueryFilter::POINT_BODY.0 | QueryFilter::MATE_CONNECTOR.0),
        ("GeometryType", "PLANE") => QueryFilter(QueryFilter::PLANAR_FACE.0 | QueryFilter::PLANE.0 | QueryFilter::SKETCH_REGION.0),
        ("GeometryType", "CYLINDER") => QueryFilter::CYLINDRICAL_FACE,
        ("GeometryType", "LINE") => QueryFilter(QueryFilter::LINEAR_EDGE.0 | QueryFilter::SKETCH_EDGE.0),
        ("GeometryType", "CIRCLE" | "ARC") => QueryFilter(QueryFilter::CIRCULAR_EDGE.0 | QueryFilter::SKETCH_EDGE.0),
        ("BodyType", "SOLID") => QueryFilter(QueryFilter::SOLID_BODY.0 | QueryFilter::ANY_FACE.0 | MODEL_EDGES | QueryFilter::VERTEX.0),
        ("BodyType", "SHEET") => QueryFilter(QueryFilter::SHEET_BODY.0 | QueryFilter::ANY_FACE.0 | MODEL_EDGES),
        ("BodyType", "WIRE") => QueryFilter(QueryFilter::WIRE_BODY.0 | MODEL_EDGES),
        ("BodyType", "MATE_CONNECTOR") => QueryFilter::MATE_CONNECTOR,
        ("BodyType", "POINT") => QueryFilter::POINT_BODY,
        ("SketchObject", "YES") => QueryFilter(QueryFilter::SKETCH_REGION.0 | QueryFilter::SKETCH_EDGE.0 | QueryFilter::SKETCH_POINT.0 | QueryFilter::SKETCH.0),
        ("ConstructionObject", "YES") => QueryFilter(QueryFilter::PLANE.0 | QueryFilter::MATE_CONNECTOR.0),
        // query.fs's expansions of the compound filters.
        ("QueryFilterCompound", "ALLOWS_AXIS") => any(&[("GeometryType", "LINE"), ("GeometryType", "CIRCLE"), ("GeometryType", "CYLINDER"), ("BodyType", "MATE_CONNECTOR")]),
        ("QueryFilterCompound", "ALLOWS_DIRECTION") => any(&[("GeometryType", "LINE"), ("GeometryType", "CIRCLE"), ("GeometryType", "CYLINDER"), ("GeometryType", "PLANE")]),
        ("QueryFilterCompound", "ALLOWS_PLANE") => any(&[("GeometryType", "PLANE"), ("BodyType", "MATE_CONNECTOR")]),
        ("QueryFilterCompound", "ALLOWS_VERTEX") => any(&[("EntityType", "VERTEX"), ("BodyType", "MATE_CONNECTOR")]),
        _ => return None,
    })
}

// ---------------------------------------------------------------- schemas

static STRINGS: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Default::default);
static SCHEMAS: LazyLock<Mutex<HashMap<(String, u64), &'static FeatureType>>> = LazyLock::new(Default::default);

pub(super) fn intern(s: &str) -> &'static str {
    let mut set = STRINGS.lock();
    if let Some(x) = set.get(s) {
        return x;
    }
    let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
    set.insert(leaked);
    leaked
}

fn intern_options(opts: &[(String, String)]) -> &'static [(&'static str, &'static str)] {
    let v: Vec<(&'static str, &'static str)> = opts.iter().map(|(a, b)| (intern(a), intern(b))).collect();
    Box::leak(v.into_boxed_slice())
}

/// The definition map visibility conditions see: booleans, enums, strings
/// and numbers (plain numeric expressions) of the current values.
fn condition_definition(decls: &[ParamDecl], v: &ParamValues) -> Value {
    let mut m = Map::new();
    for d in decls {
        let val = match (&d.kind, v.get(&d.id)) {
            (ParamKindFs::Bool { default } | ParamKindFs::Group { open: default }, p) => Value::bool(match p {
                Some(ParamValue::Bool(b)) => *b,
                _ => *default,
            }),
            (ParamKindFs::Enum { tag, default, .. }, p) => {
                let s = match p {
                    Some(ParamValue::Enum(s)) => s.clone(),
                    _ => default.clone(),
                };
                type_info(*tag).ordinal(&s).map(|o| Value::Enum(*tag, o)).unwrap_or_default()
            }
            (ParamKindFs::Text { default }, p) => Value::str(match p {
                Some(ParamValue::Text(s)) => s.clone(),
                _ => default.clone(),
            }),
            (ParamKindFs::Num { kind, spec }, p) => {
                let x = match p {
                    Some(ParamValue::Expr(e)) => e.cached.map(|q| q.value).unwrap_or(spec.default_metric),
                    _ => spec.default_metric,
                };
                match kind {
                    NumKind::Length => host::length_value(x),
                    NumKind::Angle => host::angle_value(x),
                    _ => Value::num(x),
                }
            }
            _ => Value::Undefined,
        };
        if !val.is_undefined() {
            m.insert(Value::str(d.id.as_str()), val);
        }
    }
    Value::map(m)
}

/// Dialog specs of a precondition's parameters (as a custom feature's).
pub(super) fn param_specs(params: &[ParamDecl]) -> Vec<ParamSpec> {
    let shared = Arc::new(params.to_vec());
    params.iter().map(|p| param_spec(p, shared.clone())).collect()
}

fn param_spec(d: &ParamDecl, siblings: Arc<Vec<ParamDecl>>) -> ParamSpec {
    let id = intern(&d.id);
    let label = intern(&d.label);
    let mut spec = match &d.kind {
        ParamKindFs::Query { filter, max_picks } => {
            let mut s = ParamSpec::query(id, label, *filter);
            if let ParamKind::Query { max_picks: m, .. } = &mut s.kind {
                *m = *max_picks;
            }
            s
        }
        ParamKindFs::FeatureList { max_picks } => {
            let mut s = ParamSpec::query(id, label, QueryFilter::FEATURE);
            if let ParamKind::Query { max_picks: m, .. } = &mut s.kind {
                *m = *max_picks;
            }
            s
        }
        ParamKindFs::Num { spec, .. } => ParamSpec::num(id, label, *spec),
        ParamKindFs::Bool { default } => ParamSpec::bool(id, label, *default),
        ParamKindFs::Enum { options, default, .. } => ParamSpec::enum_(id, label, intern_options(options)).default_enum(intern(default)),
        ParamKindFs::Text { default } => ParamSpec::text(id, label, intern(default)),
        ParamKindFs::Array { item_label, template, items } => {
            let shared = Arc::new(items.clone());
            let spec = ParamSpec::array(id, label, intern(item_label), items.iter().map(|x| param_spec(x, shared.clone())).collect());
            match template {
                Some(t) => spec.item_template(intern(t)),
                None => spec,
            }
        }
        ParamKindFs::Lookup { table } => ParamSpec::lookup(id, label, table),
        ParamKindFs::Reference { kind } => ParamSpec::reference(id, label, *kind),
        ParamKindFs::Anything => ParamSpec::text(id, label, "").hints(Hints::ALWAYS_HIDDEN),
        ParamKindFs::Group { open } => ParamSpec::bool(id, label, *open),
    };
    let h = spec.hints | d.hints;
    spec = spec.hints(h);
    if !d.description.is_empty() {
        spec = spec.describe(intern(&d.description));
    }
    if !d.conditions.is_empty() {
        let conds = d.conditions.clone();
        spec = spec.visible(move |v| {
            let def = condition_definition(&siblings, v);
            let mut i = Interp::new();
            conds.iter().all(|c| matches!(i.invoke(c, vec![def.clone()], &[], false), Ok(Value::Bool(true, _))))
        });
    }
    spec
}

fn build_type(type_id: &str, name: &str, icon: &str, description: &str, params: &[ParamDecl], hash: u64) -> &'static FeatureType {
    let key = (type_id.to_string(), hash);
    if let Some(t) = SCHEMAS.lock().get(&key) {
        return t;
    }
    let shared = Arc::new(params.to_vec());
    let mut t = FeatureType::new(intern(type_id), intern(name), Category::Custom, params.iter().map(|p| param_spec(p, shared.clone())).collect()).icon(intern(icon));
    if !description.is_empty() {
        t = t.describe(intern(description));
    }
    let leaked: &'static FeatureType = Box::leak(Box::new(t));
    SCHEMAS.lock().insert(key, leaked);
    leaked
}

// ---------------------------------------------------------------- active schemas

static ACTIVE: LazyLock<parking_lot::RwLock<HashMap<String, &'static FeatureType>>> = LazyLock::new(Default::default);

/// Make `types` the active custom features of a Feature Studio tab
/// (replacing what it declared before).
pub(super) fn activate(tab: TabId, types: &[&'static FeatureType]) {
    let prefix = super::type_id(tab, "");
    let mut active = ACTIVE.write();
    let unchanged = active.keys().filter(|k| k.starts_with(&prefix)).count() == types.len() && types.iter().all(|t| active.get(t.id).is_some_and(|x| std::ptr::eq(*x, *t)));
    if unchanged {
        return;
    }
    active.retain(|k, _| !k.starts_with(&prefix));
    for t in types {
        active.insert(t.id.to_string(), t);
    }
}

/// Schema of an active custom feature type.
pub fn lookup(type_id: &str) -> Option<&'static FeatureType> {
    if !super::is_custom(type_id) {
        return None;
    }
    ACTIVE.read().get(type_id).copied()
}
