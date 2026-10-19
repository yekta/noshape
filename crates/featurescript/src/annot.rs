//! Annotations (`annotation { "Name" : "Depth", "UIHint" : [...] }`).
//!
//! Annotations are read by the system, not run by programs: their
//! "Filter" values combine enum members with `&&`, `||` and `!` (query
//! filters), which is not FeatureScript boolean logic. [`eval`] keeps such
//! expressions symbolic and evaluates everything else normally.

use crate::ast::{self, BinOp, ExprKind, MapKey, UnOp};
use crate::compile::{CompiledModule, DeclKind};
use crate::interp::Interp;
use crate::ir::TypeRef;
use crate::value::{Tag, Value, type_info};
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum Ann {
    Str(String),
    Num(f64),
    Bool(bool),
    /// An enum member: (enum type name, member).
    Enum(String, String),
    List(Vec<Ann>),
    Map(Vec<(String, Ann)>),
    And(Box<Ann>, Box<Ann>),
    Or(Box<Ann>, Box<Ann>),
    Not(Box<Ann>),
    /// Any other value (evaluated).
    Value(Value),
    Undefined,
}

impl Ann {
    /// `annotation["key"]`.
    pub fn get(&self, key: &str) -> Option<&Ann> {
        match self {
            Ann::Map(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Ann::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Ann::Bool(b) => Some(*b),
            Ann::Value(Value::Bool(b, _)) => Some(*b),
            _ => None,
        }
    }
    pub fn as_num(&self) -> Option<f64> {
        match self {
            Ann::Num(n) => Some(*n),
            Ann::Value(Value::Number(n, _)) => Some(*n),
            _ => None,
        }
    }
    /// Enum members named in a value or list (`UIHint.X` or `[UIHint.X, ...]`).
    pub fn members(&self) -> Vec<String> {
        match self {
            Ann::Enum(_, m) => vec![m.clone()],
            Ann::Str(s) => vec![s.clone()],
            Ann::List(v) => v.iter().flat_map(|a| a.members()).collect(),
            _ => Vec::new(),
        }
    }
}

/// Evaluate an annotation expression in a module's scope.
pub fn eval(module: &CompiledModule, e: &ast::Expr) -> Ann {
    match &*e.kind {
        ExprKind::Str(s) => Ann::Str(s.to_string()),
        ExprKind::Number(n) => Ann::Num(*n),
        ExprKind::Bool(b) => Ann::Bool(*b),
        ExprKind::Undefined => Ann::Undefined,
        ExprKind::Array(items) => Ann::List(items.iter().map(|x| eval(module, x)).collect()),
        ExprKind::Map(entries) => Ann::Map(
            entries
                .iter()
                .map(|(k, v)| {
                    let key = match k {
                        MapKey::Str(s) => s.to_string(),
                        MapKey::Expr(x) => match eval(module, x) {
                            Ann::Str(s) => s,
                            other => format!("{other:?}"),
                        },
                    };
                    (key, eval(module, v))
                })
                .collect(),
        ),
        ExprKind::Member(base, member, _) => {
            if let ExprKind::Ident(q) = &*base.kind
                && let Some(sym) = match &q.ns {
                    Some(ns) => module.scope.namespaces.get(ns).and_then(|t| t.get(&q.name)),
                    None => module.scope.symbols.get(&q.name),
                }
                && let Some(TypeRef::Tag(t)) = sym.ty
                && type_info(t).is_enum
            {
                return Ann::Enum(q.name.to_string(), member.to_string());
            }
            value(module, e)
        }
        ExprKind::Binary(BinOp::And, a, b) => Ann::And(Box::new(eval(module, a)), Box::new(eval(module, b))),
        ExprKind::Binary(BinOp::Or, a, b) => Ann::Or(Box::new(eval(module, a)), Box::new(eval(module, b))),
        ExprKind::Unary(UnOp::Not, a) => Ann::Not(Box::new(eval(module, a))),
        _ => value(module, e),
    }
}

fn value(module: &CompiledModule, e: &ast::Expr) -> Ann {
    let Ok(f) = module.compile_lambda(&[], e) else { return Ann::Undefined };
    let mut i = Interp::new();
    match i.invoke(&f, Vec::new(), &[], false) {
        Ok(Value::Str(s, _)) => Ann::Str(s.to_string()),
        Ok(Value::Number(n, Tag::NONE)) => Ann::Num(n),
        Ok(Value::Bool(b, _)) => Ann::Bool(b),
        Ok(v) => Ann::Value(v),
        Err(_) => Ann::Undefined,
    }
}

/// (declaring module, members with their annotations).
pub type EnumMembers = (Arc<CompiledModule>, Vec<(Arc<str>, Option<ast::Expr>)>);

/// Members of an enum with their annotations, and the module declaring
/// it (whose scope evaluates those annotations). Searches the module and
/// everything it imports.
pub fn enum_members(module: &Arc<CompiledModule>, tag: Tag) -> Option<EnumMembers> {
    fn find(m: &Arc<CompiledModule>, tag: Tag, name: &str, seen: &mut HashSet<usize>) -> Option<EnumMembers> {
        if m.decls.iter().any(|d| matches!(d.kind, DeclKind::Enum(t) if t == tag)) {
            for item in &m.ast.items {
                if let ast::TopKind::Enum { name: n, values } = &item.kind
                    && &**n == name
                {
                    return Some((m.clone(), values.iter().map(|(a, n, _)| (n.clone(), a.clone())).collect()));
                }
            }
        }
        for imp in &m.imports {
            if seen.insert(Arc::as_ptr(imp) as usize)
                && let Some(r) = find(imp, tag, name, seen)
            {
                return Some(r);
            }
        }
        None
    }
    let name = type_info(tag).name.clone();
    find(module, tag, &name, &mut HashSet::new())
}
