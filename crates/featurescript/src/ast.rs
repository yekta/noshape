//! FeatureScript syntax tree.

use super::lexer::Pos;
use std::sync::Arc;

pub type Name = Arc<str>;

/// A possibly namespaced name: `Vector`, `MyLib::Thing`.
#[derive(Clone, Debug, PartialEq)]
pub struct QualName {
    pub ns: Option<Name>,
    pub name: Name,
}

impl std::fmt::Display for QualName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.ns {
            Some(ns) => write!(f, "{ns}::{}", self.name),
            None => f.write_str(&self.name),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Module {
    /// `FeatureScript 3095;`
    pub version: Option<u32>,
    pub items: Vec<TopLevel>,
}

#[derive(Clone, Debug)]
pub struct TopLevel {
    pub pos: Pos,
    /// Position of the declared name (`f` in `function f`), else `pos`.
    pub name_pos: Pos,
    pub export: bool,
    pub annotation: Option<Expr>,
    pub kind: TopKind,
}

#[derive(Clone, Debug)]
pub enum TopKind {
    Import { namespace: Option<Name>, path: String, version: String },
    Function(Arc<FnDef>),
    Const { name: Name, ty: Option<QualName>, init: Expr },
    Enum { name: Name, values: Vec<(Option<Expr>, Name, Pos)> },
    Type { name: Name, typecheck: QualName },
    TypeConvert(Arc<FnDef>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FnKind {
    Function,
    Predicate,
    Operator(BinOp),
    /// Unary minus overload (`operator-(x is T)`).
    Negate,
    Lambda,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: Name,
    pub ty: Option<QualName>,
    pub pos: Pos,
}

#[derive(Debug)]
pub struct FnDef {
    pub kind: FnKind,
    /// Empty for lambdas.
    pub name: Name,
    pub params: Vec<Param>,
    pub returns: Option<QualName>,
    pub precondition: Option<Stmt>,
    pub body: Body,
    pub pos: Pos,
    /// Position of the body (its `{`, or the expression of `x => e`).
    pub body_pos: Pos,
    /// Position of the closing brace (end of the function).
    pub end: Pos,
}

#[derive(Debug)]
pub enum Body {
    Block(Vec<Stmt>),
    /// `x => x + 1`
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub pos: Pos,
    pub kind: StmtKind,
    /// Source text of an expression statement ("x > 0"), quoted by
    /// "Precondition of f failed (x > 0)".
    pub text: Option<Arc<str>>,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    Var { constant: bool, name: Name, ty: Option<QualName>, init: Option<Expr> },
    Assign { target: Expr, op: Option<BinOp>, value: Expr },
    Expr(Expr),
    Block(Vec<Stmt>),
    If { cond: Expr, then: Box<Stmt>, otherwise: Option<Box<Stmt>> },
    While { cond: Expr, body: Box<Stmt> },
    For { init: Option<Box<Stmt>>, cond: Option<Expr>, step: Option<Box<Stmt>>, body: Box<Stmt> },
    /// `for (var k, v in x)`; `key` is set for the two-variable form.
    ForIn { declare: bool, key: Option<Name>, value: Name, iter: Expr, body: Box<Stmt> },
    Return(Option<Expr>),
    Break,
    Continue,
    Throw(Option<Expr>),
    Try { silent: bool, body: Vec<Stmt>, catch: Option<(Option<Name>, Vec<Stmt>)> },
    /// `annotation { ... } stmt` (feature parameters in preconditions).
    Annotated { annotation: Expr, stmt: Box<Stmt> },
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Cat,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    And,
    Or,
    Coalesce,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Pow => "^",
            BinOp::Cat => "~",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::Coalesce => "??",
        }
    }

    /// The verb of "Can not add ValueWithUnits (map) and number." (only
    /// "add" is confirmed live; the others follow its pattern).
    pub fn verb(self) -> &'static str {
        match self {
            BinOp::Add => "add",
            BinOp::Sub => "subtract",
            BinOp::Mul => "multiply",
            BinOp::Div => "divide",
            BinOp::Mod => "take the remainder of",
            BinOp::Pow => "raise",
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => "compare",
            _ => self.symbol(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Clone, Debug)]
pub enum MapKey {
    /// Unquoted identifier or string literal.
    Str(Name),
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub pos: Pos,
    pub kind: Box<ExprKind>,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Number(f64),
    Str(Name),
    Bool(bool),
    Undefined,
    Ident(QualName),
    /// `@name` (only valid as a callee).
    Builtin(Name),
    Array(Vec<Expr>),
    Map(Vec<(MapKey, Expr)>),
    Unary(UnOp, Expr),
    Binary(BinOp, Expr, Expr),
    Cond(Expr, Expr, Expr),
    Is(Expr, QualName),
    As(Expr, QualName),
    Call(Expr, Vec<Expr>),
    /// `x->f(a, b)` = `f(x, a, b)`
    Arrow(Expr, QualName, Vec<Expr>),
    /// `x[i]` / `x?[i]`
    Index(Expr, Expr, bool),
    /// `x.k` / `x?.k`
    Member(Expr, Name, bool),
    /// `x[]` / `x?[]`
    BoxGet(Expr, bool),
    Lambda(Arc<FnDef>),
    NewBox(Expr),
    Try(bool, Expr),
    /// `switch (x) { k : v, ... }`
    Switch(Expr, Vec<(MapKey, Expr)>),
}

impl Expr {
    pub fn new(pos: Pos, kind: ExprKind) -> Self {
        Expr { pos, kind: Box::new(kind) }
    }
}
