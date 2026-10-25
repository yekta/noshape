//! Executable form of FeatureScript: the syntax tree with every name
//! resolved (locals to frame slots, module symbols to their cells and
//! overload sets, `@builtins` to native functions).

use crate::ast::{BinOp, FnKind, Name};
use crate::interp::NativeFn;
use crate::lexer::Pos;
use crate::value::{StdType, Tag, Value};
use std::sync::{Arc, OnceLock};

/// A type constraint (`is T`, `as T`, parameter and return types).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeRef {
    Std(StdType),
    Tag(Tag),
}

impl TypeRef {
    pub fn matches(self, v: &Value) -> bool {
        match self {
            TypeRef::Std(t) => v.std_type() == t,
            // Only enums get a tag per library version (other types share one).
            TypeRef::Tag(t) => v.tag() == t || (matches!(v, Value::Enum(..)) && crate::value::same_declaration(v.tag(), t)),
        }
    }
    /// Overload specificity: tag > standard type > unconstrained (0).
    pub fn specificity(self) -> u8 {
        match self {
            TypeRef::Std(_) => 1,
            TypeRef::Tag(_) => 2,
        }
    }
    pub fn name(self) -> String {
        match self {
            TypeRef::Std(t) => t.name().to_string(),
            TypeRef::Tag(t) => crate::value::tag_name(t).to_string(),
        }
    }
}

pub struct Ex {
    pub pos: Pos,
    pub k: ExK,
}

pub enum Callee {
    Overloads(Arc<OverloadSet>),
    Value(Box<Ex>),
    Native(NativeFn, Name),
    /// A runtime builtin we do not provide (raises when called).
    MissingNative(Name),
    /// A function that is not defined (a compile warning; raises when
    /// called): name and argument count.
    Missing(Name, usize),
}

pub enum ExK {
    Lit(Value),
    Local(u32),
    Global(Arc<GlobalCell>),
    /// A top-level function used as a value.
    FnValue(Arc<OverloadSet>),
    Array(Vec<Ex>),
    Map(Vec<(Ex, Ex)>),
    Neg(Box<Ex>, Option<Arc<OverloadSet>>),
    Not(Box<Ex>),
    /// Arithmetic, `~` and comparisons; the overload set of the operator
    /// visible in the module (`<` for all comparisons).
    Bin(BinOp, Box<Ex>, Box<Ex>, Option<Arc<OverloadSet>>),
    And(Box<Ex>, Box<Ex>),
    Or(Box<Ex>, Box<Ex>),
    Coalesce(Box<Ex>, Box<Ex>),
    Cond(Box<Ex>, Box<Ex>, Box<Ex>),
    Is(Box<Ex>, TypeRef),
    As(Box<Ex>, TypeRef),
    Call(Callee, Vec<Ex>),
    Index(Box<Ex>, Box<Ex>, bool),
    Member(Box<Ex>, Value, bool),
    BoxGet(Box<Ex>, bool),
    /// A lambda and the enclosing frame's slots it captures.
    Lambda(Arc<CompiledFn>, Vec<u32>),
    NewBox(Box<Ex>),
    Try(bool, Box<Ex>),
    Switch(Box<Ex>, Vec<(Ex, Ex)>),
}

pub enum Acc {
    Member(Value),
    Index(Ex),
    Box,
}

pub struct St {
    pub pos: Pos,
    pub k: StK,
}

pub enum StK {
    Let { slot: u32, ty: Option<TypeRef>, init: Option<Ex> },
    Assign { slot: u32, path: Vec<Acc>, op: Option<(BinOp, Option<Arc<OverloadSet>>)>, value: Ex },
    Expr(Ex),
    /// Expression statement in a predicate or precondition: must be true.
    /// A predicate / precondition statement and its text for messages.
    Check(Ex, Arc<str>),
    Block(Vec<St>),
    If(Ex, Box<St>, Option<Box<St>>),
    While(Ex, Box<St>),
    For { init: Option<Box<St>>, cond: Option<Ex>, step: Option<Box<St>>, body: Box<St> },
    ForIn { key: Option<u32>, val: u32, iter: Ex, body: Box<St> },
    Return(Option<Ex>),
    Break,
    Continue,
    Throw(Option<Ex>),
    Try { silent: bool, body: Vec<St>, catch: Option<(Option<u32>, Vec<St>)> },
    Nop,
}

pub enum FnBody {
    Block(Vec<St>),
    Expr(Ex),
}

pub struct CompiledFn {
    pub name: Name,
    pub kind: FnKind,
    pub param_types: Vec<Option<TypeRef>>,
    pub param_names: Vec<Name>,
    pub returns: Option<TypeRef>,
    pub precondition: Option<St>,
    pub body: FnBody,
    pub frame_size: u32,
    /// Frame slots receiving the captured values (lambdas).
    pub capture_slots: Vec<u32>,
    pub module: Arc<str>,
    pub pos: Pos,
}

/// A top-level function, predicate or operator declaration.
pub struct FnDecl {
    pub name: Name,
    pub kind: FnKind,
    pub param_types: Vec<Option<TypeRef>>,
    pub returns: Option<TypeRef>,
    pub module: Arc<str>,
    pub pos: Pos,
    pub compiled: OnceLock<Arc<CompiledFn>>,
}

impl FnDecl {
    pub fn arity(&self) -> usize {
        self.param_types.len()
    }
}

/// Every visible declaration of one function name.
pub struct OverloadSet {
    pub name: Name,
    pub fns: Vec<Arc<FnDecl>>,
}

/// A top-level constant, initialized on first use.
pub struct GlobalCell {
    pub name: Name,
    pub module: Arc<str>,
    pub pos: Pos,
    pub ty: OnceLock<Option<TypeRef>>,
    pub init: OnceLock<(Ex, u32)>,
    pub value: OnceLock<Result<Value, Value>>,
}

impl GlobalCell {
    pub fn new(name: Name, module: Arc<str>, pos: Pos) -> Self {
        GlobalCell { name, module, pos, ty: OnceLock::new(), init: OnceLock::new(), value: OnceLock::new() }
    }
    /// A constant with a known value (enums).
    pub fn with_value(name: Name, module: Arc<str>, pos: Pos, v: Value) -> Self {
        let c = Self::new(name, module, pos);
        let _ = c.value.set(Ok(v));
        c
    }
}
