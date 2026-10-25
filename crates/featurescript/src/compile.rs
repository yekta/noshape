//! Loading modules: imports, symbol tables, and lowering function bodies
//! and constant initializers to [`crate::ir`].
//!
//! A module sees its own top-level symbols plus the exported symbols of the
//! modules it imports (namespaced imports only through `NS::name`).
//! Functions with the same name from different modules overload each
//! other. Standard library modules are compiled once per process and shared.

use crate::ast::{self, BinOp, Body, ExprKind, FnKind, MapKey, Name, QualName, StmtKind, TopKind, UnOp};
use crate::interp::{NativeFn, native};
use crate::ir::*;
use crate::lexer::Pos;
use crate::value::{StdType, Tag, Value, register_type};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, OnceLock};

/// A compile or parse error in a module.
#[derive(Clone, Debug, PartialEq)]
pub struct CompileError {
    pub module: Arc<str>,
    pub pos: Pos,
    pub message: String,
    /// Parse errors are reported as "Parse error" in the notices.
    pub parse: bool,
    /// Errors stop the module from loading; warnings ("Unused
    /// declaration: f") and infos ("Variable m set but not used") don't.
    pub level: Level,
}

/// Severity of a notice (Onshape's red ☒, yellow ⚠ and ⓘ icons).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}: {}", self.module, self.pos, self.message)
    }
}

#[derive(Clone)]
pub enum ValueSym {
    Const(Arc<GlobalCell>),
    Funcs(Arc<OverloadSet>),
    Ambiguous,
}

#[derive(Clone, Default)]
pub struct Symbol {
    pub value: Option<ValueSym>,
    pub ty: Option<TypeRef>,
}

pub type SymbolTable = HashMap<Name, Symbol>;

/// What a module can see.
#[derive(Default)]
pub struct Scope {
    pub symbols: SymbolTable,
    pub namespaces: HashMap<Name, Arc<SymbolTable>>,
}

/// A top-level declaration of a module (for outlines and feature lookup).
#[derive(Clone)]
pub struct Decl {
    pub name: Name,
    pub pos: Pos,
    pub export: bool,
    pub kind: DeclKind,
}

#[derive(Clone)]
pub enum DeclKind {
    Function(Arc<FnDecl>, Arc<ast::FnDef>),
    Const(Arc<GlobalCell>),
    Enum(Tag),
    Type(Tag),
}

pub struct CompiledModule {
    pub path: Arc<str>,
    pub version: Option<u32>,
    pub ast: Arc<ast::Module>,
    pub scope: Scope,
    pub exports: Arc<SymbolTable>,
    pub decls: Vec<Decl>,
    /// Modules this one imports (kept alive with it).
    pub imports: Vec<Arc<CompiledModule>>,
    /// Compile warnings, in Onshape's notice order.
    pub warnings: Vec<CompileError>,
}

impl CompiledModule {
    pub fn decl(&self, name: &str) -> Option<&Decl> {
        self.decls.iter().find(|d| &*d.name == name)
    }
    /// A constant of this module's scope (own or imported).
    pub fn constant(&self, name: &str) -> Option<Arc<GlobalCell>> {
        match self.scope.symbols.get(name)?.value.as_ref()? {
            ValueSym::Const(c) => Some(c.clone()),
            _ => None,
        }
    }
    pub fn functions(&self, name: &str) -> Option<Arc<OverloadSet>> {
        match self.scope.symbols.get(name)?.value.as_ref()? {
            ValueSym::Funcs(f) => Some(f.clone()),
            _ => None,
        }
    }
    /// Compile `function(params) { return expr; }` in this module's scope
    /// (feature dialog visibility conditions, editing logic helpers).
    pub fn compile_lambda(&self, params: &[&str], expr: &ast::Expr) -> Result<Arc<CompiledFn>, Vec<CompileError>> {
        let def = ast::FnDef {
            kind: FnKind::Lambda,
            name: "".into(),
            params: params.iter().map(|p| ast::Param { name: (*p).into(), ty: None, pos: expr.pos }).collect(),
            returns: None,
            precondition: None,
            body: Body::Expr(expr.clone()),
            pos: expr.pos,
            body_pos: expr.pos,
            end: expr.pos,
        };
        let mut errors = Vec::new();
        let mut cx = Cx { scope: &self.scope, module: self.path.clone(), version: self.version, fns: Vec::new(), errors: &mut errors, used: HashSet::new(), lambda_name: None };
        let (f, _) = cx.function(&def, FnKind::Lambda, None, None);
        if errors.iter().any(|e| e.level == Level::Error) { Err(errors) } else { Ok(Arc::new(f)) }
    }

    pub fn type_tag(&self, name: &str) -> Option<Tag> {
        match self.scope.symbols.get(name)?.ty? {
            TypeRef::Tag(t) => Some(t),
            _ => None,
        }
    }
}

/// Finds the source of non-standard-library imports (Feature Studio tabs).
pub trait Resolver {
    /// (canonical module path, source) for an import path.
    fn resolve(&self, path: &str) -> Option<(Arc<str>, Arc<str>)>;
    /// An already compiled module for a canonical path (cache).
    fn cached(&self, _path: &str) -> Option<Arc<CompiledModule>> {
        None
    }
}

/// Resolver for sources with no imports besides the standard library.
pub struct NoImports;

impl Resolver for NoImports {
    fn resolve(&self, _path: &str) -> Option<(Arc<str>, Arc<str>)> {
        None
    }
}

const STD_PREFIX: &str = "onshape/std/";

// ---------------------------------------------------------------- standard library

type StdCache = HashMap<(u32, String), Arc<CompiledModule>>;

static STD_CACHE: LazyLock<Mutex<StdCache>> = LazyLock::new(Default::default);

/// A standard library module (`"geometry.fs"`) of the embedded release,
/// compiled once.
pub fn std_module(name: &str) -> Result<Arc<CompiledModule>, Vec<CompileError>> {
    std_module_at(crate::STD_VERSION, name)
}

/// A standard library module as an import pinned to `version` gets it
/// (see [`crate::std_version_for`]).
pub fn std_module_at(version: u32, name: &str) -> Result<Arc<CompiledModule>, Vec<CompileError>> {
    let mut cache = STD_CACHE.lock();
    load_std(crate::std_version_for(version), name, &mut cache)
}

/// The version number of an import's `version : "1963.0"`.
fn import_version(version: &str) -> u32 {
    version.split('.').next().and_then(|v| v.parse().ok()).unwrap_or(crate::STD_VERSION)
}

fn load_std(version: u32, name: &str, cache: &mut StdCache) -> Result<Arc<CompiledModule>, Vec<CompileError>> {
    if let Some(m) = cache.get(&(version, name.to_string())) {
        return Ok(m.clone());
    }
    let path: Arc<str> = format!("{STD_PREFIX}{name}").into();
    let Some(src) = crate::std_source_at(version, name) else {
        return Err(vec![CompileError { module: path, pos: Pos::default(), message: format!("Standard library module {name} not found"), parse: false, level: Level::Error }]);
    };
    let ast = Arc::new(crate::parser::parse(src).map_err(|e| vec![CompileError { module: path.clone(), pos: e.pos, message: e.message, parse: true, level: Level::Error }])?);
    let mut imports = Vec::new();
    for item in &ast.items {
        if let TopKind::Import { path: p, version: v, .. } = &item.kind {
            let Some(n) = p.strip_prefix(STD_PREFIX) else {
                return Err(vec![CompileError { module: path.clone(), pos: item.pos, message: format!("Unknown import {p}"), parse: false, level: Level::Error }]);
            };
            // An older library imports its own version's modules.
            let v = if version == crate::STD_VERSION { version } else { crate::std_version_for(import_version(v)) };
            imports.push(Some(load_std(v, n, cache)?));
        } else {
            imports.push(None);
        }
    }
    let m = Arc::new(build_module(path, ast, imports)?);
    cache.insert((version, name.to_string()), m.clone());
    Ok(m)
}

// ---------------------------------------------------------------- user modules

/// Compile a module (a Feature Studio) from source. Standard library
/// imports load the embedded library; other imports go to `resolver`.
pub fn compile_module(path: &str, src: &str, resolver: &dyn Resolver) -> Result<Arc<CompiledModule>, Vec<CompileError>> {
    let mut loading = vec![path.to_string()];
    compile_with(path.into(), src, resolver, &mut loading)
}

fn compile_with(path: Arc<str>, src: &str, resolver: &dyn Resolver, loading: &mut Vec<String>) -> Result<Arc<CompiledModule>, Vec<CompileError>> {
    let ast = Arc::new(crate::parser::parse(src).map_err(|e| vec![CompileError { module: path.clone(), pos: e.pos, message: e.message, parse: true, level: Level::Error }])?);
    let mut imports = Vec::new();
    for item in &ast.items {
        let TopKind::Import { path: p, version, .. } = &item.kind else {
            imports.push(None);
            continue;
        };
        let err = |m: String| vec![CompileError { module: path.clone(), pos: item.pos, message: m, parse: false, level: Level::Error }];
        if let Some(n) = p.strip_prefix(STD_PREFIX) {
            // The library at the version the import pins (when embedded).
            imports.push(Some(std_module_at(import_version(version), n).map_err(|_| err(format!("Standard library module \"{p}\" does not exist")))?));
            continue;
        }
        let Some((canon, source)) = resolver.resolve(p) else {
            return Err(err(format!("Imported element \"{p}\" not found")));
        };
        if loading.iter().any(|l| **l == *canon) {
            return Err(err(format!("Cyclic import of \"{p}\"")));
        }
        if let Some(m) = resolver.cached(&canon) {
            imports.push(Some(m));
            continue;
        }
        loading.push(canon.to_string());
        let m = compile_with(canon.clone(), &source, resolver, loading).map_err(|errs| {
            let first = errs.first().map(|e| e.message.clone()).unwrap_or_default();
            err(format!("Imported Feature Studio has errors: {first}"))
        })?;
        loading.pop();
        imports.push(Some(m));
    }
    // Version checks (Onshape lists them without a position).
    let own = ast.version.unwrap_or(0);
    let mut errors = Vec::new();
    if ast.version.is_none() {
        errors.push(CompileError { module: path.clone(), pos: Pos::default(), message: "Missing FeatureScript version".into(), parse: false, level: Level::Error });
    }
    for (item, m) in ast.items.iter().zip(&imports) {
        let (TopKind::Import { path: p, version, .. }, Some(m)) = (&item.kind, m) else { continue };
        // The standard library is served at the version the import asks for
        // (we embed one release); other modules have their own header.
        let v = match p.strip_prefix(STD_PREFIX) {
            Some(_) => version.split('.').next().and_then(|v| v.parse().ok()).unwrap_or(0),
            None => m.version.unwrap_or(0),
        };
        if v > own {
            errors.push(CompileError { module: path.clone(), pos: Pos::default(), message: format!("Cannot import a module with FeatureScript version: {v} from module with older version {own}"), parse: false, level: Level::Error });
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    build_module(path, ast, imports).map(Arc::new)
}

// ---------------------------------------------------------------- operators

/// Operator overloads are global: `1 * meter < 1 * inch` works in modules
/// that don't import units.fs. Overloads are kept per module path (a
/// recompiled module replaces its own).
static OPERATORS: LazyLock<parking_lot::RwLock<Operators>> = LazyLock::new(Default::default);

#[derive(Default)]
struct Operators {
    by_module: HashMap<Arc<str>, Vec<Arc<FnDecl>>>,
    sets: HashMap<&'static str, Arc<OverloadSet>>,
}

fn register_operators(module: &Arc<str>, decls: Vec<Arc<FnDecl>>) {
    let mut ops = OPERATORS.write();
    if decls.is_empty() && !ops.by_module.contains_key(module) {
        return;
    }
    ops.by_module.insert(module.clone(), decls);
    let mut sets: HashMap<&'static str, Vec<Arc<FnDecl>>> = HashMap::new();
    for d in ops.by_module.values().flatten() {
        if let Some(name) = operator_name(d.kind) {
            sets.entry(name).or_default().push(d.clone());
        }
    }
    ops.sets = sets.into_iter().map(|(k, fns)| (k, Arc::new(OverloadSet { name: k.into(), fns }))).collect();
}

/// Exported functions are overloads of every same-named function, whichever
/// module calls it: std's `println` (string.fs) prints a Plane with
/// surfaceGeometry.fs's `toString`, which string.fs doesn't import (live FS
/// probe). Kept per module path (a recompiled module replaces its own).
static EXPORTED_FNS: LazyLock<parking_lot::RwLock<ExportedFns>> = LazyLock::new(Default::default);

#[derive(Default)]
struct ExportedFns {
    by_module: HashMap<Arc<str>, Vec<Arc<FnDecl>>>,
    by_name: HashMap<Name, Arc<Vec<Arc<FnDecl>>>>,
    generation: u64,
}

fn register_exports(module: &Arc<str>, decls: Vec<Arc<FnDecl>>) {
    let mut ex = EXPORTED_FNS.write();
    if decls.is_empty() && !ex.by_module.contains_key(module) {
        return;
    }
    ex.by_module.insert(module.clone(), decls);
    let mut by_name: HashMap<Name, Vec<Arc<FnDecl>>> = HashMap::new();
    for d in ex.by_module.values().flatten() {
        by_name.entry(d.name.clone()).or_default().push(d.clone());
    }
    ex.by_name = by_name.into_iter().map(|(k, v)| (k, Arc::new(v))).collect();
    ex.generation += 1;
}

/// Exported functions named `name` of every loaded module, and the
/// registry's generation (changes when modules are (re)compiled).
pub fn exported_overloads(name: &str) -> (u64, Option<Arc<Vec<Arc<FnDecl>>>>) {
    let ex = EXPORTED_FNS.read();
    (ex.generation, ex.by_name.get(name).cloned())
}

/// Every loaded overload of an operator (`"operator+"`).
pub fn operator_overloads(name: &str) -> Option<Arc<OverloadSet>> {
    OPERATORS.read().sets.get(name).cloned()
}

// ---------------------------------------------------------------- symbol tables

fn merge_symbol(table: &mut SymbolTable, name: &Name, sym: &Symbol) {
    match table.get_mut(name) {
        None => {
            table.insert(name.clone(), sym.clone());
        }
        Some(cur) => {
            if cur.ty.is_none() {
                cur.ty = sym.ty;
            }
            cur.value = match (cur.value.take(), &sym.value) {
                (None, v) => v.clone(),
                (v, None) => v,
                (Some(ValueSym::Funcs(a)), Some(ValueSym::Funcs(b))) => {
                    if Arc::ptr_eq(&a, b) {
                        Some(ValueSym::Funcs(a))
                    } else {
                        let mut fns = a.fns.clone();
                        for f in &b.fns {
                            if !fns.iter().any(|g| Arc::ptr_eq(g, f)) {
                                fns.push(f.clone());
                            }
                        }
                        Some(ValueSym::Funcs(Arc::new(OverloadSet { name: name.clone(), fns })))
                    }
                }
                (Some(ValueSym::Const(a)), Some(ValueSym::Const(b))) if Arc::ptr_eq(&a, b) => Some(ValueSym::Const(a)),
                _ => Some(ValueSym::Ambiguous),
            };
        }
    }
}

fn operator_name(kind: FnKind) -> Option<&'static str> {
    Some(match kind {
        FnKind::Operator(BinOp::Add) => "operator+",
        FnKind::Operator(BinOp::Sub) | FnKind::Negate => "operator-",
        FnKind::Operator(BinOp::Mul) => "operator*",
        FnKind::Operator(BinOp::Div) => "operator/",
        FnKind::Operator(BinOp::Mod) => "operator%",
        FnKind::Operator(BinOp::Pow) => "operator^",
        FnKind::Operator(BinOp::Lt) => "operator<",
        _ => return None,
    })
}

fn resolve_type(q: &QualName, symbols: &SymbolTable, namespaces: &HashMap<Name, Arc<SymbolTable>>) -> Option<TypeRef> {
    if let Some(ns) = &q.ns {
        return namespaces.get(ns)?.get(&q.name)?.ty;
    }
    if let Some(t) = symbols.get(&q.name).and_then(|s| s.ty) {
        return Some(t);
    }
    StdType::from_name(&q.name).map(TypeRef::Std)
}

fn build_module(path: Arc<str>, ast: Arc<ast::Module>, imports: Vec<Option<Arc<CompiledModule>>>) -> Result<CompiledModule, Vec<CompileError>> {
    let mut errors = Vec::new();
    let err = |errors: &mut Vec<CompileError>, pos: Pos, m: String| errors.push(CompileError { module: path.clone(), pos, message: m, parse: false, level: Level::Error });
    let mut scope = Scope::default();
    let mut exports = SymbolTable::new();
    let mut kept = Vec::new();
    // Imported symbols.
    for (item, m) in ast.items.iter().zip(&imports) {
        let (TopKind::Import { namespace, .. }, Some(m)) = (&item.kind, m) else { continue };
        kept.push(m.clone());
        match namespace {
            Some(ns) => {
                scope.namespaces.insert(ns.clone(), m.exports.clone());
            }
            None => {
                for (n, s) in m.exports.iter() {
                    // Live (dom/fs-import): two un-namespaced imports exporting
                    // different constants of one name (every file import
                    // exports BLOB_DATA) do not compile.
                    if let (Some(ValueSym::Const(a)), Some(ValueSym::Const(b))) = (scope.symbols.get(n).and_then(|c| c.value.as_ref()), s.value.as_ref())
                        && !Arc::ptr_eq(a, b)
                    {
                        err(&mut errors, item.pos, format!("Duplicate top level symbol name '{n}'"));
                    }
                    merge_symbol(&mut scope.symbols, n, s);
                }
            }
        }
        if item.export {
            for (n, s) in m.exports.iter() {
                merge_symbol(&mut exports, n, s);
            }
        }
    }
    // Own types first (parameter types may use them).
    let mut decls = Vec::new();
    for item in &ast.items {
        match &item.kind {
            TopKind::Enum { name, values } => {
                let members: Vec<Arc<str>> = values.iter().map(|(_, v, _)| v.clone()).collect();
                let tag = register_type(format!("{path}#{name}"), name, true, members.clone());
                let mut map = crate::value::Map::new();
                for (i, m) in members.iter().enumerate() {
                    map.insert(Value::str(m.clone()), Value::Enum(tag, i as u32));
                }
                let cell = Arc::new(GlobalCell::with_value(name.clone(), path.clone(), item.pos, Value::map(map)));
                let sym = Symbol { value: Some(ValueSym::Const(cell)), ty: Some(TypeRef::Tag(tag)) };
                scope.symbols.insert(name.clone(), sym.clone());
                if item.export {
                    merge_symbol(&mut exports, name, &sym);
                }
                decls.push(Decl { name: name.clone(), pos: item.pos, export: item.export, kind: DeclKind::Enum(tag) });
            }
            TopKind::Type { name, .. } => {
                let tag = register_type(format!("{path}#{name}"), name, false, Vec::new());
                let entry = scope.symbols.entry(name.clone()).or_default();
                entry.ty = Some(TypeRef::Tag(tag));
                if item.export {
                    merge_symbol(&mut exports, name, &Symbol { value: None, ty: Some(TypeRef::Tag(tag)) });
                }
                decls.push(Decl { name: name.clone(), pos: item.pos, export: item.export, kind: DeclKind::Type(tag) });
            }
            _ => {}
        }
    }
    // Functions and constants.
    let mut own_fns: Vec<(Arc<FnDecl>, Arc<ast::FnDef>)> = Vec::new();
    let mut own_consts: Vec<(Arc<GlobalCell>, Option<QualName>, ast::Expr)> = Vec::new();
    let mut own_values: SymbolTable = SymbolTable::new();
    for item in &ast.items {
        match &item.kind {
            TopKind::Function(def) => {
                let mut types = Vec::new();
                for p in &def.params {
                    types.push(match &p.ty {
                        None => None,
                        Some(q) => match resolve_type(q, &scope.symbols, &scope.namespaces) {
                            Some(t) => Some(t),
                            None => {
                                err(&mut errors, p.pos, format!("Type {q} not found"));
                                None
                            }
                        },
                    });
                }
                let returns = def.returns.as_ref().and_then(|q| {
                    let t = resolve_type(q, &scope.symbols, &scope.namespaces);
                    if t.is_none() {
                        err(&mut errors, def.pos, format!("Type {q} not found"));
                    }
                    t
                });
                let name: Name = match operator_name(def.kind) {
                    Some(op) => op.into(),
                    None => def.name.clone(),
                };
                let decl = Arc::new(FnDecl { name: name.clone(), kind: def.kind, param_types: types, returns, module: path.clone(), pos: def.pos, compiled: OnceLock::new() });
                let sym = Symbol { value: Some(ValueSym::Funcs(Arc::new(OverloadSet { name: name.clone(), fns: vec![decl.clone()] }))), ty: None };
                merge_symbol(&mut own_values, &name, &sym);
                if item.export {
                    merge_symbol(&mut exports, &name, &sym);
                }
                decls.push(Decl { name: def.name.clone(), pos: item.pos, export: item.export, kind: DeclKind::Function(decl.clone(), def.clone()) });
                own_fns.push((decl, def.clone()));
            }
            TopKind::Const { name, ty, init } => {
                if own_values.contains_key(name) {
                    err(&mut errors, item.pos, format!("Symbol {name} is already defined"));
                    continue;
                }
                let cell = Arc::new(GlobalCell::new(name.clone(), path.clone(), item.pos));
                let sym = Symbol { value: Some(ValueSym::Const(cell.clone())), ty: None };
                own_values.insert(name.clone(), sym.clone());
                if item.export {
                    merge_symbol(&mut exports, name, &sym);
                }
                decls.push(Decl { name: name.clone(), pos: item.pos, export: item.export, kind: DeclKind::Const(cell.clone()) });
                own_consts.push((cell, ty.clone(), init.clone()));
            }
            _ => {}
        }
    }
    // Own declarations: functions overload imported ones, constants shadow.
    for (n, s) in &own_values {
        match &s.value {
            Some(ValueSym::Funcs(_)) => merge_symbol(&mut scope.symbols, n, s),
            _ => {
                let entry = scope.symbols.entry(n.clone()).or_default();
                entry.value = s.value.clone();
            }
        }
    }
    // Bodies and initializers in source order: each declaration's notices,
    // then its "Unused declaration" warning (Onshape's notice order).
    let mut used = HashSet::new();
    let mut segments = Vec::new();
    let (mut next_fn, mut next_const) = (0, 0);
    for item in &ast.items {
        match &item.kind {
            TopKind::Function(_) => {
                let (decl, def) = &own_fns[next_fn];
                next_fn += 1;
                let mut cx = Cx { scope: &scope, module: path.clone(), version: ast.version, fns: Vec::new(), errors: &mut errors, used: HashSet::new(), lambda_name: None };
                let (f, _) = cx.function(def, decl.kind, Some(&decl.param_types), decl.returns);
                used.extend(cx.used);
                let _ = decl.compiled.set(Arc::new(f));
                if operator_name(decl.kind).is_none() {
                    segments.push((errors.len(), item, &def.name));
                }
            }
            TopKind::Const { name, .. } if next_const < own_consts.len() && own_consts[next_const].0.name == *name && own_consts[next_const].0.pos == item.pos => {
                let (cell, ty, init) = &own_consts[next_const];
                next_const += 1;
                let mut cx = Cx { scope: &scope, module: path.clone(), version: ast.version, fns: vec![Fcx::default()], errors: &mut errors, used: HashSet::new(), lambda_name: Some(format!("const {name}").into()) };
                let e = cx.expr(init);
                let frame = cx.fns.pop().map(|f| f.nslots).unwrap_or(0);
                used.extend(cx.used);
                let t = ty.as_ref().and_then(|q| resolve_type(q, &scope.symbols, &scope.namespaces));
                let _ = cell.ty.set(t);
                let _ = cell.init.set((e, frame));
                segments.push((errors.len(), item, name));
            }
            _ => {}
        }
    }
    for (at, item, name) in segments.into_iter().rev() {
        if !item.export && !used.contains(name) {
            errors.insert(at, CompileError { module: path.clone(), pos: item.name_pos, message: format!("Unused declaration: {name}"), parse: false, level: Level::Warning });
        }
    }
    if errors.iter().any(|e| e.level == Level::Error) {
        return Err(errors);
    }
    register_operators(&path, own_fns.iter().filter(|(d, _)| operator_name(d.kind).is_some()).map(|(d, _)| d.clone()).collect());
    let exported: Vec<Arc<FnDecl>> = ast.items.iter().filter(|it| it.export).filter_map(|it| match &it.kind {
        TopKind::Function(def) => own_fns.iter().find(|(_, d)| Arc::ptr_eq(d, def)).map(|(decl, _)| decl.clone()),
        _ => None,
    }).filter(|d| operator_name(d.kind).is_none()).collect();
    register_exports(&path, exported);
    Ok(CompiledModule { path, version: ast.version, ast, scope, exports: Arc::new(exports), decls, imports: kept, warnings: errors })
}

// ---------------------------------------------------------------- bodies

#[derive(Default)]
struct Fcx {
    blocks: Vec<Vec<(Name, u32, bool)>>,
    nslots: u32,
    /// (name, slot in the enclosing frame, slot in this frame)
    captures: Vec<(Name, u32, u32)>,
    /// `var` / `const` locals by slot: (name, set, read), for "Variable x
    /// set but not used".
    uses: Vec<(u32, Name, bool, bool)>,
}

impl Fcx {
    fn use_of(&mut self, slot: u32) -> Option<&mut (u32, Name, bool, bool)> {
        self.uses.iter_mut().find(|u| u.0 == slot)
    }
}

impl Fcx {
    fn find_local(&self, name: &str) -> Option<(u32, bool)> {
        for b in self.blocks.iter().rev() {
            if let Some((_, s, c)) = b.iter().rev().find(|(n, _, _)| &**n == name) {
                return Some((*s, *c));
            }
        }
        self.captures.iter().find(|(n, _, _)| &**n == name).map(|(_, _, s)| (*s, false))
    }
}

struct Cx<'a> {
    scope: &'a Scope,
    module: Arc<str>,
    /// The module's `FeatureScript` version (which form of a native it gets).
    version: Option<u32>,
    fns: Vec<Fcx>,
    errors: &'a mut Vec<CompileError>,
    /// Module-level names referenced (for "Unused declaration").
    used: HashSet<Name>,
    /// Name of anonymous functions in stack traces (`const fsProbe` for
    /// lambdas in a constant's initializer, like Onshape).
    lambda_name: Option<Name>,
}

fn lit(pos: Pos, v: Value) -> Ex {
    Ex { pos, k: ExK::Lit(v) }
}

impl Cx<'_> {
    fn error(&mut self, pos: Pos, m: impl Into<String>) {
        self.errors.push(CompileError { module: self.module.clone(), pos, message: m.into(), parse: false, level: Level::Error });
    }

    /// A "Parse warning" (shown in the Feature Studio, not in Part Studios).
    fn parse_warning(&mut self, pos: Pos, m: impl Into<String>) {
        self.errors.push(CompileError { module: self.module.clone(), pos, message: m.into(), parse: true, level: Level::Warning });
    }

    /// Live: "Non-numeric operand" at a string / array / map / boolean
    /// literal operand of `+ - * / % ^` whose other operand is a number
    /// literal (`[1] + 1`, `1 - "a"`; `id + "cube"` is fine).
    fn check_numeric_operand(&mut self, op: BinOp, e: &ast::Expr, other: &ast::Expr) {
        let arithmetic = matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::Pow);
        if arithmetic && matches!(&*other.kind, ExprKind::Number(_)) && matches!(&*e.kind, ExprKind::Str(_) | ExprKind::Array(_) | ExprKind::Map(_) | ExprKind::Bool(_)) {
            self.parse_warning(e.pos, "Non-numeric operand");
        }
    }

    /// Live: "Non-boolean operand" at a non-boolean literal condition.
    fn check_boolean_condition(&mut self, e: &ast::Expr) {
        if matches!(&*e.kind, ExprKind::Number(_) | ExprKind::Str(_) | ExprKind::Array(_) | ExprKind::Map(_)) {
            self.parse_warning(e.pos, "Non-boolean operand");
        }
    }

    fn warning(&mut self, pos: Pos, m: impl Into<String>) {
        self.errors.push(CompileError { module: self.module.clone(), pos, message: m.into(), parse: false, level: Level::Warning });
    }

    fn cur(&mut self) -> &mut Fcx {
        self.fns.last_mut().expect("function scope")
    }

    fn cur_use(&mut self, slot: u32) -> Option<&mut (u32, Name, bool, bool)> {
        self.cur().use_of(slot)
    }

    fn declare(&mut self, name: &Name, constant: bool, pos: Pos) -> u32 {
        let f = self.cur();
        if f.blocks.last().is_some_and(|b| b.iter().any(|(n, _, _)| n == name)) {
            self.error(pos, format!("Variable {name} is already defined"));
        }
        let f = self.cur();
        let slot = f.nslots;
        f.nslots += 1;
        if f.blocks.is_empty() {
            f.blocks.push(Vec::new());
        }
        f.blocks.last_mut().expect("block").push((name.clone(), slot, constant));
        slot
    }

    /// Slot of a local read (capturing it from enclosing functions if
    /// needed).
    fn local(&mut self, name: &str) -> Option<(u32, bool)> {
        self.local_use(name, true)
    }

    /// Slot of a local that is read (`read`) or assigned.
    fn local_use(&mut self, name: &str, read: bool) -> Option<(u32, bool)> {
        let depth = self.fns.len();
        let found = (0..depth).rev().find_map(|i| self.fns[i].find_local(name).map(|s| (i, s)))?;
        let (level, (mut slot, constant)) = found;
        if let Some(u) = self.fns[level].use_of(slot) {
            if read {
                u.3 = true;
            } else {
                u.2 = true;
            }
        }
        for i in level + 1..depth {
            let f = &mut self.fns[i];
            if let Some((_, _, inner)) = f.captures.iter().find(|(n, _, _)| &**n == name) {
                slot = *inner;
                continue;
            }
            let inner = f.nslots;
            f.nslots += 1;
            f.captures.push((name.into(), slot, inner));
            slot = inner;
        }
        Some((slot, constant && level + 1 == depth))
    }

    fn symbol(&mut self, q: &QualName) -> Option<&Symbol> {
        if q.ns.is_none() {
            self.used.insert(q.name.clone());
        }
        match &q.ns {
            Some(ns) => self.scope.namespaces.get(ns)?.get(&q.name),
            None => self.scope.symbols.get(&q.name),
        }
    }

    fn type_ref(&mut self, q: &QualName, pos: Pos) -> TypeRef {
        match resolve_type(q, &self.scope.symbols, &self.scope.namespaces) {
            Some(t) => t,
            None => {
                self.error(pos, format!("Type {q} not found"));
                TypeRef::Std(StdType::Undefined)
            }
        }
    }

    fn op_set(&self, name: &str) -> Option<Arc<OverloadSet>> {
        match self.scope.symbols.get(name)?.value.as_ref()? {
            ValueSym::Funcs(f) => Some(f.clone()),
            _ => None,
        }
    }

    // ------------------------------------------------------------ functions

    /// Compile a function; also returns, for lambdas, the enclosing frame
    /// slots of its captured variables (in `capture_slots` order).
    fn function(&mut self, def: &ast::FnDef, kind: FnKind, types: Option<&[Option<TypeRef>]>, returns: Option<TypeRef>) -> (CompiledFn, Vec<u32>) {
        let mut f = Fcx::default();
        f.blocks.push(Vec::new());
        self.fns.push(f);
        let first_notice = self.errors.len();
        let mut param_types = Vec::new();
        for (i, p) in def.params.iter().enumerate() {
            self.declare(&p.name, false, p.pos);
            let t = match types {
                Some(t) => t[i],
                None => p.ty.as_ref().map(|q| self.type_ref(q, p.pos)),
            };
            param_types.push(t);
        }
        let returns = match types {
            Some(_) => returns,
            None => def.returns.as_ref().map(|q| self.type_ref(q, def.pos)),
        };
        // Anonymous functions inside a named one show its name in stacks.
        let outer_name = if def.name.is_empty() { None } else { self.lambda_name.replace(def.name.clone()) };
        let precondition = def.precondition.as_ref().map(|s| self.stmt(s, true));
        let predicate = kind == FnKind::Predicate;
        let body = match &def.body {
            Body::Block(stmts) => FnBody::Block(stmts.iter().map(|s| self.stmt(s, predicate)).collect()),
            Body::Expr(e) => FnBody::Expr(self.expr(e)),
        };
        if !def.name.is_empty() {
            self.lambda_name = outer_name;
        }
        let f = self.fns.pop().expect("function scope");
        // Listed before the notices of the function's body, like Onshape.
        let unused = f.uses.iter().filter(|u| u.2 && !u.3).map(|u| CompileError { module: self.module.clone(), pos: def.body_pos, message: format!("Variable {} set but not used", u.1), parse: false, level: Level::Info });
        self.errors.splice(first_notice..first_notice, unused);
        let outer = f.captures.iter().map(|c| c.1).collect();
        let compiled = CompiledFn {
            name: match (operator_name(kind), &self.lambda_name, def.name.is_empty()) {
                (Some(op), _, _) => op.into(),
                (None, Some(n), true) => n.clone(),
                _ => def.name.clone(),
            },
            kind,
            param_types,
            param_names: def.params.iter().map(|p| p.name.clone()).collect(),
            returns,
            precondition,
            body,
            frame_size: f.nslots,
            capture_slots: f.captures.iter().map(|c| c.2).collect(),
            module: self.module.clone(),
            pos: def.pos,
        };
        (compiled, outer)
    }

    // ------------------------------------------------------------ statements

    fn block(&mut self, stmts: &[ast::Stmt], pred: bool) -> Vec<St> {
        self.cur().blocks.push(Vec::new());
        let out = stmts.iter().map(|s| self.stmt(s, pred)).collect();
        self.cur().blocks.pop();
        out
    }

    /// A statement that is its own scope (loop and if bodies).
    fn scoped(&mut self, s: &ast::Stmt, pred: bool) -> St {
        self.cur().blocks.push(Vec::new());
        let out = self.stmt(s, pred);
        self.cur().blocks.pop();
        out
    }

    fn stmt(&mut self, s: &ast::Stmt, pred: bool) -> St {
        let pos = s.pos;
        let k = match &s.kind {
            StmtKind::Var { constant, name, ty, init } => {
                let ty = ty.as_ref().map(|q| self.type_ref(q, pos));
                let init = init.as_ref().map(|e| self.expr(e));
                let slot = self.declare(name, *constant, pos);
                let set = init.is_some();
                self.cur().uses.push((slot, name.clone(), set, false));
                StK::Let { slot, ty, init }
            }
            StmtKind::Assign { target, op, value } => {
                let value = self.expr(value);
                let mut path = Vec::new();
                let mut root = target;
                loop {
                    match &*root.kind {
                        ExprKind::Member(b, k, _) => {
                            path.push(Acc::Member(Value::str(k.clone())));
                            root = b;
                        }
                        ExprKind::Index(b, i, _) => {
                            path.push(Acc::Index(self.expr(i)));
                            root = b;
                        }
                        ExprKind::BoxGet(b, _) => {
                            path.push(Acc::Box);
                            root = b;
                        }
                        _ => break,
                    }
                }
                path.reverse();
                let ExprKind::Ident(q) = &*root.kind else {
                    self.error(pos, "Invalid assignment target");
                    return St { pos, k: StK::Nop };
                };
                let Some((slot, constant)) = self.local_use(&q.name, op.is_some()) else {
                    if self.symbol(q).is_some() {
                        self.error(root.pos, format!("Cannot assign to constant {}", q.name));
                    } else {
                        self.warning(root.pos, format!("Variable {} not found.", q.name));
                    }
                    return St { pos, k: StK::Nop };
                };
                if let Some(u) = self.cur_use(slot) {
                    u.2 = true;
                }
                if constant && !path.iter().any(|a| matches!(a, Acc::Box)) {
                    self.error(root.pos, format!("Cannot assign to constant {}", q.name));
                }
                let op = op.map(|o| (o, self.bin_overloads(o)));
                StK::Assign { slot, path, op, value }
            }
            StmtKind::Expr(e) => {
                let e = self.expr(e);
                if pred { StK::Check(e, s.text.clone().unwrap_or_default()) } else { StK::Expr(e) }
            }
            StmtKind::Block(b) => StK::Block(self.block(b, pred)),
            StmtKind::If { cond, then, otherwise } => {
                self.check_boolean_condition(cond);
                let c = self.expr(cond);
                let t = Box::new(self.scoped(then, pred));
                let o = otherwise.as_ref().map(|o| Box::new(self.scoped(o, pred)));
                StK::If(c, t, o)
            }
            StmtKind::While { cond, body } => {
                self.check_boolean_condition(cond);
                let c = self.expr(cond);
                StK::While(c, Box::new(self.scoped(body, pred)))
            }
            StmtKind::For { init, cond, step, body } => {
                self.cur().blocks.push(Vec::new());
                let init = init.as_ref().map(|s| Box::new(self.stmt(s, false)));
                let cond = cond.as_ref().map(|e| self.expr(e));
                let step = step.as_ref().map(|s| Box::new(self.stmt(s, false)));
                let body = Box::new(self.scoped(body, pred));
                self.cur().blocks.pop();
                StK::For { init, cond, step, body }
            }
            StmtKind::ForIn { declare, key, value, iter, body } => {
                let iter = self.expr(iter);
                self.cur().blocks.push(Vec::new());
                let bind = |cx: &mut Self, n: &Name| -> u32 {
                    if *declare {
                        return cx.declare(n, false, pos);
                    }
                    match cx.local_use(n, false) {
                        Some((s, false)) => s,
                        Some((s, true)) => {
                            cx.error(pos, format!("Cannot assign to constant {n}"));
                            s
                        }
                        None => {
                            cx.warning(pos, format!("Variable {n} not found."));
                            0
                        }
                    }
                };
                let key = key.as_ref().map(|k| bind(self, k));
                let val = bind(self, value);
                let body = Box::new(self.scoped(body, pred));
                self.cur().blocks.pop();
                StK::ForIn { key, val, iter, body }
            }
            StmtKind::Return(e) => StK::Return(e.as_ref().map(|e| self.expr(e))),
            StmtKind::Break => StK::Break,
            StmtKind::Continue => StK::Continue,
            StmtKind::Throw(e) => StK::Throw(e.as_ref().map(|e| self.expr(e))),
            StmtKind::Try { silent, body, catch } => {
                let body = self.block(body, pred);
                let catch = catch.as_ref().map(|(name, stmts)| {
                    self.cur().blocks.push(Vec::new());
                    let slot = name.as_ref().map(|n| self.declare(n, false, pos));
                    let b = stmts.iter().map(|s| self.stmt(s, pred)).collect();
                    self.cur().blocks.pop();
                    (slot, b)
                });
                StK::Try { silent: *silent, body, catch }
            }
            // A predicate statement's "Message" annotation replaces its text
            // in "Precondition of f failed (...)"; located at the annotation.
            StmtKind::Annotated { annotation, stmt } => {
                let message = match &*annotation.kind {
                    ExprKind::Map(entries) => entries.iter().find_map(|(k, v)| match (k, &*v.kind) {
                        (MapKey::Str(k), ExprKind::Str(m)) if &**k == "Message" => Some(m.clone()),
                        _ => None,
                    }),
                    _ => None,
                };
                let inner = self.stmt(stmt, pred);
                return match (inner.k, message) {
                    (StK::Check(e, _), Some(m)) => St { pos, k: StK::Check(e, m) },
                    (k, _) => St { pos: inner.pos, k },
                };
            }
            StmtKind::Empty => StK::Nop,
        };
        St { pos, k }
    }

    // ------------------------------------------------------------ expressions

    fn bin_overloads(&self, op: BinOp) -> Option<Arc<OverloadSet>> {
        let name = match op {
            BinOp::Add => "operator+",
            BinOp::Sub => "operator-",
            BinOp::Mul => "operator*",
            BinOp::Div => "operator/",
            BinOp::Mod => "operator%",
            BinOp::Pow => "operator^",
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => "operator<",
            _ => return None,
        };
        self.op_set(name)
    }

    fn exprs(&mut self, es: &[ast::Expr]) -> Vec<Ex> {
        es.iter().map(|e| self.expr(e)).collect()
    }

    /// The callee of `name(...)` / `x->name(...)`.
    fn callee(&mut self, q: &QualName, pos: Pos, nargs: usize) -> Callee {
        if q.ns.is_none()
            && let Some((slot, _)) = self.local(&q.name)
        {
            return Callee::Value(Box::new(Ex { pos, k: ExK::Local(slot) }));
        }
        let sym = match &q.ns {
            Some(ns) => self.scope.namespaces.get(ns).and_then(|t| t.get(&q.name)),
            None => self.scope.symbols.get(&q.name),
        };
        let callee = match sym.and_then(|s| s.value.clone()) {
            // Live: a call no overload takes by argument count is "Function two
            // with 1 argument(s) not found" (and `two` stays unused).
            Some(ValueSym::Funcs(f))
                if !f.fns.iter().any(|d| d.arity() == nargs) && !exported_overloads(&q.name).1.is_some_and(|g| g.iter().any(|d| d.arity() == nargs)) =>
            {
                None
            }
            Some(ValueSym::Funcs(f)) => Some(Callee::Overloads(f)),
            Some(ValueSym::Const(c)) => Some(Callee::Value(Box::new(Ex { pos, k: ExK::Global(c) }))),
            Some(ValueSym::Ambiguous) => {
                self.error(pos, format!("Symbol {q} is ambiguous"));
                return Callee::MissingNative(q.name.clone());
            }
            None => None,
        };
        match callee {
            Some(c) => {
                if q.ns.is_none() {
                    self.used.insert(q.name.clone());
                }
                c
            }
            None => {
                self.warning(pos, format!("Function {q} with {nargs} argument(s) not found"));
                Callee::Missing(q.name.clone(), nargs)
            }
        }
    }

    fn map_key(&mut self, k: &MapKey) -> Ex {
        match k {
            MapKey::Str(s) => lit(Pos::default(), Value::str(s.clone())),
            MapKey::Expr(e) => self.expr(e),
        }
    }

    fn expr(&mut self, e: &ast::Expr) -> Ex {
        let pos = e.pos;
        let k = match &*e.kind {
            ExprKind::Number(n) => ExK::Lit(Value::num(*n)),
            ExprKind::Str(s) => ExK::Lit(Value::str(s.clone())),
            ExprKind::Bool(b) => ExK::Lit(Value::bool(*b)),
            ExprKind::Undefined => ExK::Lit(Value::Undefined),
            ExprKind::Ident(q) => {
                if q.ns.is_none()
                    && let Some((slot, _)) = self.local(&q.name)
                {
                    return Ex { pos, k: ExK::Local(slot) };
                }
                match self.symbol(q).and_then(|s| s.value.clone()) {
                    Some(ValueSym::Const(c)) => ExK::Global(c),
                    Some(ValueSym::Funcs(f)) => ExK::FnValue(f),
                    Some(ValueSym::Ambiguous) => {
                        self.error(pos, format!("Symbol {q} is ambiguous"));
                        ExK::Lit(Value::Undefined)
                    }
                    None => {
                        self.error(pos, format!("Variable {q} not found."));
                        ExK::Lit(Value::Undefined)
                    }
                }
            }
            ExprKind::Builtin(name) => {
                self.error(pos, format!("@{name} can only be called"));
                ExK::Lit(Value::Undefined)
            }
            ExprKind::Array(items) => ExK::Array(self.exprs(items)),
            ExprKind::Map(entries) => {
                let mut out = Vec::new();
                for (k, v) in entries {
                    let kx = self.map_key(k);
                    out.push((kx, self.expr(v)));
                }
                ExK::Map(out)
            }
            ExprKind::Unary(UnOp::Neg, x) => ExK::Neg(Box::new(self.expr(x)), self.op_set("operator-")),
            ExprKind::Unary(UnOp::Not, x) => ExK::Not(Box::new(self.expr(x))),
            ExprKind::Binary(op, a, b) => {
                self.check_numeric_operand(*op, a, b);
                let ax = Box::new(self.expr(a));
                self.check_numeric_operand(*op, b, a);
                let (a, b) = (ax, Box::new(self.expr(b)));
                match op {
                    BinOp::And => ExK::And(a, b),
                    BinOp::Or => ExK::Or(a, b),
                    BinOp::Coalesce => ExK::Coalesce(a, b),
                    _ => ExK::Bin(*op, a, b, self.bin_overloads(*op)),
                }
            }
            ExprKind::Cond(c, a, b) => ExK::Cond(Box::new(self.expr(c)), Box::new(self.expr(a)), Box::new(self.expr(b))),
            ExprKind::Is(x, q) => {
                let t = self.type_ref(q, pos);
                ExK::Is(Box::new(self.expr(x)), t)
            }
            ExprKind::As(x, q) => {
                let t = self.type_ref(q, pos);
                ExK::As(Box::new(self.expr(x)), t)
            }
            ExprKind::Call(f, args) => {
                let callee = match &*f.kind {
                    ExprKind::Ident(q) => self.callee(q, f.pos, args.len()),
                    ExprKind::Builtin(name) => match crate::interp::native_at(name, self.version) {
                        Some(n) => Callee::Native(n, name.clone()),
                        None => Callee::MissingNative(name.clone()),
                    },
                    _ => Callee::Value(Box::new(self.expr(f))),
                };
                ExK::Call(callee, self.exprs(args))
            }
            ExprKind::Arrow(x, q, args) => {
                let first = self.expr(x);
                let callee = self.callee(q, pos, args.len() + 1);
                let mut a = vec![first];
                a.extend(self.exprs(args));
                ExK::Call(callee, a)
            }
            ExprKind::Index(b, i, safe) => ExK::Index(Box::new(self.expr(b)), Box::new(self.expr(i)), *safe),
            ExprKind::Member(b, k, safe) => ExK::Member(Box::new(self.expr(b)), Value::str(k.clone()), *safe),
            ExprKind::BoxGet(b, safe) => ExK::BoxGet(Box::new(self.expr(b)), *safe),
            ExprKind::Lambda(def) => {
                let (f, outer) = self.function(def, FnKind::Lambda, None, None);
                ExK::Lambda(Arc::new(f), outer)
            }
            ExprKind::NewBox(x) => ExK::NewBox(Box::new(self.expr(x))),
            ExprKind::Try(silent, x) => ExK::Try(*silent, Box::new(self.expr(x))),
            ExprKind::Switch(subject, entries) => {
                let s = Box::new(self.expr(subject));
                let mut out = Vec::new();
                for (k, v) in entries {
                    let kx = self.map_key(k);
                    out.push((kx, self.expr(v)));
                }
                ExK::Switch(s, out)
            }
        };
        Ex { pos, k }
    }

}

/// Look up a native by name (for tests and the host).
pub fn native_fn(name: &str) -> Option<NativeFn> {
    native(name)
}
