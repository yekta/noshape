//! Ordered variable tables (Onshape Variable features and Variable Studios).

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    Dim, EvalError, ExprError, LayeredScope, ParseError, Quantity, UnitPrefs, VariableScope,
    evaluate_expr_as,
};

/// Returns true if `name` is a valid variable name (`[A-Za-z_][A-Za-z0-9_]*`).
pub fn is_valid_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// One variable definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariableDef {
    /// Name without the leading `#`.
    pub name: String,
    /// Expression text.
    pub expr: String,
    /// Expected dimension (Onshape's variable type: length, angle, number).
    /// `None` accepts any value as-is ("Any"). When set, bare numbers take
    /// the default unit and other dimensions are rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<Dim>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

impl VariableDef {
    pub fn new(name: impl Into<String>, expr: impl Into<String>) -> Self {
        VariableDef {
            name: name.into(),
            expr: expr.into(),
            kind: None,
            description: String::new(),
        }
    }

    pub fn with_kind(mut self, kind: Dim) -> Self {
        self.kind = Some(kind);
        self
    }
}

/// How references between table entries are resolved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvalOrder {
    /// Entries are evaluated top to bottom; an entry may only reference
    /// entries above it (or the outer scope). A name may be defined more than
    /// once; later definitions shadow earlier ones and may reference them
    /// (`#x = #x * 2`). This is how Variable features in a Part Studio
    /// feature list behave.
    #[default]
    Sequential,
    /// Entries may reference each other in any order; they are evaluated in
    /// dependency order. Names must be unique and cycles are errors.
    Dependency,
}

/// Error of a single variable.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum VarError {
    #[error("invalid variable name `{0}`")]
    InvalidName(String),
    #[error("variable `#{0}` is defined more than once")]
    DuplicateName(String),
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Eval(#[from] EvalError),
    #[error("undefined variable `#{0}`")]
    Undefined(String),
    #[error("`#{0}` is defined later; variables can only reference variables above them")]
    ForwardReference(String),
    #[error("circular reference: {}", .0.iter().map(|n| format!("#{n}")).collect::<Vec<_>>().join(" -> "))]
    Cycle(Vec<String>),
    #[error("`#{0}` has an error")]
    DependencyFailed(String),
}

impl From<ExprError> for VarError {
    fn from(e: ExprError) -> Self {
        match e {
            ExprError::Parse(p) => VarError::Parse(p),
            ExprError::Eval(e) => VarError::Eval(e),
        }
    }
}

/// An ordered list of variable definitions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VariableTable {
    #[serde(default)]
    pub order: EvalOrder,
    pub entries: Vec<VariableDef>,
}

/// Result of evaluating a [`VariableTable`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableEvaluation {
    /// One result per table entry, in table order.
    pub results: Vec<Result<Quantity, VarError>>,
    /// Final value of every successfully evaluated name (table order of first
    /// definition; for redefined names the last successful value).
    pub values: IndexMap<String, Quantity>,
}

impl TableEvaluation {
    pub fn is_ok(&self) -> bool {
        self.results.iter().all(Result::is_ok)
    }

    /// First error, with the index of the failing entry.
    pub fn first_error(&self) -> Option<(usize, &VarError)> {
        self.results
            .iter()
            .enumerate()
            .find_map(|(i, r)| r.as_ref().err().map(|e| (i, e)))
    }
}

impl VariableScope for TableEvaluation {
    fn get(&self, name: &str) -> Option<Quantity> {
        self.values.get(name).copied()
    }
}

impl VariableTable {
    pub fn new(order: EvalOrder) -> Self {
        VariableTable {
            order,
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &VariableDef> {
        self.entries.iter()
    }

    /// Appends a definition.
    pub fn push(&mut self, def: VariableDef) {
        self.entries.push(def);
    }

    /// Inserts a definition at `index`.
    pub fn insert(&mut self, index: usize, def: VariableDef) {
        self.entries.insert(index, def);
    }

    pub fn remove(&mut self, index: usize) -> VariableDef {
        self.entries.remove(index)
    }

    /// Moves an entry (reordering a Variable feature).
    pub fn move_entry(&mut self, from: usize, to: usize) {
        let e = self.entries.remove(from);
        self.entries.insert(to, e);
    }

    /// Last definition of `name`.
    pub fn get(&self, name: &str) -> Option<&VariableDef> {
        self.entries.iter().rev().find(|d| d.name == name)
    }

    /// Sets the expression of the last definition of `name`, or appends a new
    /// definition.
    pub fn set(&mut self, name: &str, expr: impl Into<String>) {
        match self.entries.iter_mut().rev().find(|d| d.name == name) {
            Some(d) => d.expr = expr.into(),
            None => self.entries.push(VariableDef::new(name, expr)),
        }
    }

    /// Evaluates every entry. `outer` supplies variables defined outside the
    /// table (e.g. a referenced Variable Studio); table entries shadow it.
    pub fn evaluate(&self, outer: &dyn VariableScope, prefs: &UnitPrefs) -> TableEvaluation {
        match self.order {
            EvalOrder::Sequential => self.evaluate_sequential(outer, prefs),
            EvalOrder::Dependency => self.evaluate_dependency(outer, prefs),
        }
    }

    fn evaluate_sequential(&self, outer: &dyn VariableScope, prefs: &UnitPrefs) -> TableEvaluation {
        let mut values: IndexMap<String, Quantity> = IndexMap::new();
        let mut failed: HashSet<&str> = HashSet::new();
        let mut results = Vec::with_capacity(self.entries.len());
        for (i, def) in self.entries.iter().enumerate() {
            let r = (|| {
                if !is_valid_variable_name(&def.name) {
                    return Err(VarError::InvalidName(def.name.clone()));
                }
                let expr = crate::parse(&def.expr)?;
                for v in expr.variables() {
                    if values.contains_key(&v) {
                        continue;
                    }
                    if failed.contains(v.as_str()) {
                        return Err(VarError::DependencyFailed(v));
                    }
                    if outer.get(&v).is_some() {
                        continue;
                    }
                    if v == def.name {
                        return Err(VarError::Cycle(vec![v.clone(), v]));
                    }
                    if self.entries[i + 1..].iter().any(|d| d.name == v) {
                        return Err(VarError::ForwardReference(v));
                    }
                    return Err(VarError::Undefined(v));
                }
                let scope = LayeredScope {
                    inner: &values,
                    outer,
                };
                eval_def(def, &expr, prefs, &scope)
            })();
            match &r {
                Ok(q) => {
                    failed.remove(def.name.as_str());
                    values.insert(def.name.clone(), *q);
                }
                Err(_) => {
                    failed.insert(def.name.as_str());
                    values.shift_remove(&def.name);
                }
            }
            results.push(r);
        }
        TableEvaluation { results, values }
    }

    fn evaluate_dependency(&self, outer: &dyn VariableScope, prefs: &UnitPrefs) -> TableEvaluation {
        let n = self.entries.len();
        let mut index: HashMap<&str, usize> = HashMap::new();
        let mut results: Vec<Option<Result<Quantity, VarError>>> = vec![None; n];
        for (i, def) in self.entries.iter().enumerate() {
            if !is_valid_variable_name(&def.name) {
                results[i] = Some(Err(VarError::InvalidName(def.name.clone())));
            } else if index.contains_key(def.name.as_str()) {
                results[i] = Some(Err(VarError::DuplicateName(def.name.clone())));
            } else {
                index.insert(&def.name, i);
            }
        }
        let mut ctx = DepCtx {
            table: self,
            index: &index,
            outer,
            prefs,
            results,
            stack: Vec::new(),
            values: IndexMap::new(),
        };
        for i in 0..n {
            let _ = ctx.visit(i);
        }
        let results: Vec<_> = ctx
            .results
            .into_iter()
            .map(|r| r.expect("all visited"))
            .collect();
        let mut values = IndexMap::new();
        for (def, r) in self.entries.iter().zip(&results) {
            if let Ok(q) = r {
                values.insert(def.name.clone(), *q);
            }
        }
        TableEvaluation { results, values }
    }
}

fn eval_def(
    def: &VariableDef,
    expr: &crate::Expr,
    prefs: &UnitPrefs,
    scope: &dyn VariableScope,
) -> Result<Quantity, VarError> {
    match def.kind {
        Some(kind) => Ok(evaluate_expr_as(expr, kind, prefs, scope)?),
        None => Ok(expr.eval(scope)?),
    }
}

struct DepCtx<'a> {
    table: &'a VariableTable,
    index: &'a HashMap<&'a str, usize>,
    outer: &'a dyn VariableScope,
    prefs: &'a UnitPrefs,
    results: Vec<Option<Result<Quantity, VarError>>>,
    stack: Vec<usize>,
    values: IndexMap<String, Quantity>,
}

impl DepCtx<'_> {
    fn visit(&mut self, i: usize) -> Result<Quantity, VarError> {
        if let Some(r) = &self.results[i] {
            return r.clone();
        }
        if let Some(pos) = self.stack.iter().position(|&s| s == i) {
            let mut path: Vec<String> = self.stack[pos..]
                .iter()
                .map(|&s| self.table.entries[s].name.clone())
                .collect();
            path.push(self.table.entries[i].name.clone());
            return Err(VarError::Cycle(path));
        }
        self.stack.push(i);
        let r = self.compute(i);
        self.stack.pop();
        if let Ok(q) = &r {
            self.values.insert(self.table.entries[i].name.clone(), *q);
        }
        self.results[i] = Some(r.clone());
        r
    }

    fn compute(&mut self, i: usize) -> Result<Quantity, VarError> {
        let def = &self.table.entries[i];
        let expr = crate::parse(&def.expr)?;
        for v in expr.variables() {
            match self.index.get(v.as_str()) {
                Some(&j) => match self.visit(j) {
                    Ok(_) => {}
                    Err(VarError::Cycle(path)) if path.contains(&def.name) => {
                        return Err(VarError::Cycle(path));
                    }
                    Err(_) => return Err(VarError::DependencyFailed(v)),
                },
                None if self.outer.get(&v).is_some() => {}
                None => return Err(VarError::Undefined(v)),
            }
        }
        let scope = LayeredScope {
            inner: &self.values,
            outer: self.outer,
        };
        eval_def(def, &expr, self.prefs, &scope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EmptyScope;

    fn prefs() -> UnitPrefs {
        UnitPrefs::default()
    }

    fn mm(q: &Result<Quantity, VarError>) -> f64 {
        q.as_ref().unwrap().value * 1000.0
    }

    #[test]
    fn names() {
        assert!(is_valid_variable_name("width_2"));
        assert!(is_valid_variable_name("_x"));
        assert!(!is_valid_variable_name("2x"));
        assert!(!is_valid_variable_name(""));
        assert!(!is_valid_variable_name("a-b"));
    }

    #[test]
    fn sequential_in_order() {
        let mut t = VariableTable::default();
        t.push(VariableDef::new("w", "20 mm"));
        t.push(VariableDef::new("h", "#w / 2"));
        t.push(VariableDef::new("area", "#w * #h"));
        let r = t.evaluate(&EmptyScope, &prefs());
        assert!(r.is_ok());
        assert!((mm(&r.results[1]) - 10.0).abs() < 1e-9);
        assert_eq!(r.values["area"].dim, Dim::AREA);
        assert_eq!(r.values.keys().collect::<Vec<_>>(), vec!["w", "h", "area"]);
        assert!((r.get("area").unwrap().value - 2e-4).abs() < 1e-15);
    }

    #[test]
    fn sequential_forward_reference_and_undefined() {
        let mut t = VariableTable::default();
        t.push(VariableDef::new("a", "#b + 1 mm"));
        t.push(VariableDef::new("b", "1 mm"));
        t.push(VariableDef::new("c", "#zzz"));
        t.push(VariableDef::new("d", "#a * 2"));
        let r = t.evaluate(&EmptyScope, &prefs());
        assert_eq!(r.results[0], Err(VarError::ForwardReference("b".into())));
        assert!(r.results[1].is_ok());
        assert_eq!(r.results[2], Err(VarError::Undefined("zzz".into())));
        assert_eq!(r.results[3], Err(VarError::DependencyFailed("a".into())));
        assert_eq!(r.first_error().unwrap().0, 0);
        assert!(!r.values.contains_key("a"));
    }

    #[test]
    fn sequential_redefinition_and_self_reference() {
        let mut t = VariableTable::default();
        t.push(VariableDef::new("x", "1 mm"));
        t.push(VariableDef::new("y", "#x"));
        t.push(VariableDef::new("x", "#x * 3"));
        t.push(VariableDef::new("z", "#x"));
        t.push(VariableDef::new("s", "#s + 1"));
        let r = t.evaluate(&EmptyScope, &prefs());
        assert!((mm(&r.results[1]) - 1.0).abs() < 1e-9);
        assert!((mm(&r.results[3]) - 3.0).abs() < 1e-9);
        assert!((r.values["x"].value - 0.003).abs() < 1e-12);
        assert_eq!(
            r.results[4],
            Err(VarError::Cycle(vec!["s".into(), "s".into()]))
        );
    }

    #[test]
    fn outer_scope_and_shadowing() {
        let mut outer = IndexMap::new();
        outer.insert("t".to_string(), Quantity::mm(2.0));
        outer.insert("w".to_string(), Quantity::mm(100.0));
        let mut t = VariableTable::default();
        t.push(VariableDef::new("w", "#t * 5"));
        t.push(VariableDef::new("t", "#t + 1 mm"));
        let r = t.evaluate(&outer, &prefs());
        assert!((mm(&r.results[0]) - 10.0).abs() < 1e-9);
        assert!((mm(&r.results[1]) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn kinds_apply_default_units() {
        let mut t = VariableTable::default();
        t.push(VariableDef::new("len", "10").with_kind(Dim::LENGTH));
        t.push(VariableDef::new("ang", "45").with_kind(Dim::ANGLE));
        t.push(VariableDef::new("n", "3").with_kind(Dim::UNITLESS));
        t.push(VariableDef::new("bad", "3 deg").with_kind(Dim::LENGTH));
        t.push(VariableDef::new("any", "3"));
        let r = t.evaluate(&EmptyScope, &prefs());
        assert!((mm(&r.results[0]) - 10.0).abs() < 1e-9);
        assert!((r.values["ang"].value - std::f64::consts::FRAC_PI_4).abs() < 1e-12);
        assert_eq!(r.values["n"], Quantity::unitless(3.0));
        assert!(matches!(
            r.results[3],
            Err(VarError::Eval(EvalError::WrongDimension {
                expected: Dim::LENGTH,
                found: Dim::ANGLE
            }))
        ));
        assert_eq!(r.values["any"], Quantity::unitless(3.0));
    }

    #[test]
    fn errors_reported_per_entry() {
        let mut t = VariableTable::default();
        t.push(VariableDef::new("2bad", "1"));
        t.push(VariableDef::new("p", "1 +"));
        t.push(VariableDef::new("q", "1 mm + 1 deg"));
        let r = t.evaluate(&EmptyScope, &prefs());
        assert!(matches!(r.results[0], Err(VarError::InvalidName(_))));
        assert!(matches!(r.results[1], Err(VarError::Parse(_))));
        assert!(matches!(
            r.results[2],
            Err(VarError::Eval(EvalError::DimensionMismatch { .. }))
        ));
        // Errors render as messages.
        assert!(
            r.results[2]
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("cannot add")
        );
    }

    #[test]
    fn dependency_order_any_order() {
        let mut t = VariableTable::new(EvalOrder::Dependency);
        t.push(VariableDef::new("area", "#w * #h"));
        t.push(VariableDef::new("h", "#w / 2"));
        t.push(VariableDef::new("w", "20 mm"));
        let r = t.evaluate(&EmptyScope, &prefs());
        assert!(r.is_ok(), "{:?}", r.results);
        assert!((r.values["area"].value - 2e-4).abs() < 1e-15);
        // values keep table order
        assert_eq!(r.values.keys().collect::<Vec<_>>(), vec!["area", "h", "w"]);
    }

    #[test]
    fn dependency_cycles() {
        let mut t = VariableTable::new(EvalOrder::Dependency);
        t.push(VariableDef::new("a", "#b + 1"));
        t.push(VariableDef::new("b", "#c + 1"));
        t.push(VariableDef::new("c", "#a + 1"));
        t.push(VariableDef::new("d", "#a"));
        t.push(VariableDef::new("e", "5"));
        t.push(VariableDef::new("f", "#f"));
        t.push(VariableDef::new("e", "6"));
        t.push(VariableDef::new("g", "#nope"));
        let r = t.evaluate(&EmptyScope, &prefs());
        for i in 0..3 {
            match &r.results[i] {
                Err(VarError::Cycle(path)) => {
                    assert_eq!(path.len(), 4);
                    assert_eq!(path.first(), path.last());
                }
                other => panic!("expected cycle, got {other:?}"),
            }
        }
        assert_eq!(r.results[3], Err(VarError::DependencyFailed("a".into())));
        assert_eq!(r.results[4], Ok(Quantity::unitless(5.0)));
        assert_eq!(
            r.results[5],
            Err(VarError::Cycle(vec!["f".into(), "f".into()]))
        );
        assert_eq!(r.results[6], Err(VarError::DuplicateName("e".into())));
        assert_eq!(r.results[7], Err(VarError::Undefined("nope".into())));
        assert!(
            r.results[0]
                .as_ref()
                .unwrap_err()
                .to_string()
                .starts_with("circular reference: #a -> #b")
        );
    }

    #[test]
    fn edit_operations_and_serde() {
        let mut t = VariableTable::default();
        t.set("a", "1");
        t.set("b", "#a + 1");
        t.set("a", "5");
        assert_eq!(t.len(), 2);
        assert_eq!(t.get("a").unwrap().expr, "5");
        t.move_entry(1, 0);
        let r = t.evaluate(&EmptyScope, &prefs());
        assert_eq!(r.results[0], Err(VarError::ForwardReference("a".into())));
        t.insert(0, VariableDef::new("c", "2"));
        assert_eq!(t.remove(0).name, "c");
        let json = serde_json::to_string(&t).unwrap();
        let back: VariableTable = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
        assert!(!t.is_empty());
        assert_eq!(t.iter().count(), 2);
    }
}
