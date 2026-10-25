//! Expression AST, canonical printing and evaluation.

use std::fmt;

use crate::{Dim, EvalError, Quantity, Unit, VariableScope};

/// Built-in constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Constant {
    Pi,
    True,
    False,
}

impl Constant {
    pub fn value(self) -> Quantity {
        match self {
            Constant::Pi => Quantity::unitless(std::f64::consts::PI),
            Constant::True => Quantity::unitless(1.0),
            Constant::False => Quantity::unitless(0.0),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Constant::Pi => "PI",
            Constant::True => "true",
            Constant::False => "false",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

impl BinaryOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::Pow => "^",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
        }
    }

    pub(crate) fn precedence(self) -> u8 {
        match self {
            BinaryOp::Or => PREC_OR,
            BinaryOp::And => PREC_AND,
            BinaryOp::Eq | BinaryOp::Ne => PREC_EQ,
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => PREC_CMP,
            BinaryOp::Add | BinaryOp::Sub => PREC_ADD,
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => PREC_MUL,
            BinaryOp::Pow => PREC_POW,
        }
    }
}

const PREC_TERNARY: u8 = 1;
const PREC_OR: u8 = 2;
const PREC_AND: u8 = 3;
const PREC_EQ: u8 = 4;
const PREC_CMP: u8 = 5;
const PREC_ADD: u8 = 6;
const PREC_MUL: u8 = 7;
const PREC_UNARY: u8 = 8;
const PREC_POW: u8 = 9;
const PREC_ATOM: u8 = 10;

/// Built-in functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Func {
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Sqrt,
    Abs,
    Floor,
    Ceil,
    Round,
    Min,
    Max,
    Exp,
    Ln,
    Log10,
    Clamp,
    Hypot,
    IsInteger,
    RoundToPrecision,
    TolerantEquals,
}

impl Func {
    pub const ALL: [Func; 22] = [
        Func::Sin,
        Func::Cos,
        Func::Tan,
        Func::Asin,
        Func::Acos,
        Func::Atan,
        Func::Atan2,
        Func::Sqrt,
        Func::Abs,
        Func::Floor,
        Func::Ceil,
        Func::Round,
        Func::Min,
        Func::Max,
        Func::Exp,
        Func::Ln,
        Func::Log10,
        Func::Clamp,
        Func::Hypot,
        Func::IsInteger,
        Func::RoundToPrecision,
        Func::TolerantEquals,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Func::Sin => "sin",
            Func::Cos => "cos",
            Func::Tan => "tan",
            Func::Asin => "asin",
            Func::Acos => "acos",
            Func::Atan => "atan",
            Func::Atan2 => "atan2",
            Func::Sqrt => "sqrt",
            Func::Abs => "abs",
            Func::Floor => "floor",
            Func::Ceil => "ceil",
            Func::Round => "round",
            Func::Min => "min",
            Func::Max => "max",
            Func::Exp => "exp",
            Func::Ln => "ln",
            Func::Log10 => "log10",
            Func::Clamp => "clamp",
            Func::Hypot => "hypot",
            Func::IsInteger => "isInteger",
            Func::RoundToPrecision => "roundToPrecision",
            Func::TolerantEquals => "tolerantEquals",
        }
    }

    /// Looks a function up by name (`log` is accepted as an alias of `ln`).
    pub fn from_name(name: &str) -> Option<Func> {
        if name == "log" {
            return Some(Func::Ln);
        }
        Func::ALL.into_iter().find(|f| f.name() == name)
    }

    /// Allowed argument counts as (min, max); `max == None` is variadic.
    pub fn arity(self) -> (usize, Option<usize>) {
        match self {
            Func::Atan2 | Func::Hypot | Func::RoundToPrecision | Func::TolerantEquals => (2, Some(2)),
            Func::Clamp => (3, Some(3)),
            Func::Floor | Func::Ceil | Func::Round => (1, Some(2)),
            Func::Min | Func::Max => (1, None),
            _ => (1, Some(1)),
        }
    }

    pub(crate) fn arity_text(self) -> String {
        match self.arity() {
            (a, Some(b)) if a == b => a.to_string(),
            (a, Some(b)) => format!("{a} or {b}"),
            (a, None) => format!("at least {a}"),
        }
    }
}

/// Parsed expression.
///
/// Numbers are stored as written (in the unit given by an enclosing
/// [`Expr::WithUnit`]); evaluation converts to SI.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Number(f64),
    /// `expr unit^power`, e.g. `25 mm`, `4 mm^2`, `(1/4) in`.
    WithUnit {
        expr: Box<Expr>,
        unit: Unit,
        power: i8,
    },
    /// A bare unit used as a value (`#x * mm`), equal to one of that unit.
    Unit(Unit),
    Constant(Constant),
    /// `#name`
    Var(String),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(Func, Vec<Expr>),
}

impl Expr {
    /// Parses `src`. Equivalent to [`crate::parse`].
    pub fn parse(src: &str) -> Result<Expr, crate::ParseError> {
        crate::parse(src)
    }

    /// Variables referenced by the expression, unique, in order of first
    /// appearance (without the `#`).
    pub fn variables(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_vars(&mut out);
        out
    }

    fn collect_vars(&self, out: &mut Vec<String>) {
        match self {
            Expr::Var(name) => {
                if !out.iter().any(|n| n == name) {
                    out.push(name.clone());
                }
            }
            Expr::Number(_) | Expr::Unit(_) | Expr::Constant(_) => {}
            Expr::WithUnit { expr, .. } | Expr::Unary(_, expr) => expr.collect_vars(out),
            Expr::Binary(_, a, b) => {
                a.collect_vars(out);
                b.collect_vars(out);
            }
            Expr::Ternary(a, b, c) => {
                a.collect_vars(out);
                b.collect_vars(out);
                c.collect_vars(out);
            }
            Expr::Call(_, args) => args.iter().for_each(|a| a.collect_vars(out)),
        }
    }

    /// True if the expression contains no variables.
    pub fn is_constant(&self) -> bool {
        self.variables().is_empty()
    }

    /// Evaluates the expression. `&&`, `||` and `?:` short-circuit.
    pub fn eval(&self, scope: &dyn VariableScope) -> Result<Quantity, EvalError> {
        match self {
            Expr::Number(v) => Ok(Quantity::unitless(*v)),
            Expr::WithUnit { expr, unit, power } => {
                let inner = expr.eval(scope)?;
                let u = Quantity::with_unit(1.0, *unit).powi(i32::from(*power))?;
                inner.checked_mul(u)
            }
            Expr::Unit(u) => Ok(Quantity::with_unit(1.0, *u)),
            Expr::Constant(c) => Ok(c.value()),
            Expr::Var(name) => scope
                .get(name)
                .ok_or_else(|| EvalError::UndefinedVariable(name.clone())),
            Expr::Unary(UnaryOp::Neg, e) => Ok(-e.eval(scope)?),
            Expr::Unary(UnaryOp::Not, e) => Ok(bool_q(!e.eval(scope)?.is_truthy())),
            Expr::Binary(op, a, b) => eval_binary(*op, a, b, scope),
            Expr::Ternary(c, t, e) => {
                if c.eval(scope)?.is_truthy() {
                    t.eval(scope)
                } else {
                    e.eval(scope)
                }
            }
            Expr::Call(f, args) => {
                let vals = args
                    .iter()
                    .map(|a| a.eval(scope))
                    .collect::<Result<Vec<_>, _>>()?;
                eval_call(*f, &vals)
            }
        }
    }

    pub(crate) fn precedence(&self) -> u8 {
        match self {
            Expr::Number(v) if v.is_sign_negative() => PREC_UNARY,
            Expr::Number(_) | Expr::Unit(_) | Expr::Constant(_) | Expr::Var(_) => PREC_ATOM,
            Expr::WithUnit { .. } | Expr::Call(..) => PREC_ATOM,
            Expr::Unary(..) => PREC_UNARY,
            Expr::Binary(op, ..) => op.precedence(),
            Expr::Ternary(..) => PREC_TERNARY,
        }
    }
}

fn bool_q(b: bool) -> Quantity {
    Quantity::unitless(if b { 1.0 } else { 0.0 })
}

fn eval_binary(
    op: BinaryOp,
    a: &Expr,
    b: &Expr,
    scope: &dyn VariableScope,
) -> Result<Quantity, EvalError> {
    match op {
        BinaryOp::And => {
            let l = a.eval(scope)?;
            if !l.is_truthy() {
                return Ok(bool_q(false));
            }
            return Ok(bool_q(b.eval(scope)?.is_truthy()));
        }
        BinaryOp::Or => {
            let l = a.eval(scope)?;
            if l.is_truthy() {
                return Ok(bool_q(true));
            }
            return Ok(bool_q(b.eval(scope)?.is_truthy()));
        }
        _ => {}
    }
    let l = a.eval(scope)?;
    let r = b.eval(scope)?;
    use std::cmp::Ordering::*;
    match op {
        BinaryOp::Add => l.checked_add(r),
        BinaryOp::Sub => l.checked_sub(r),
        BinaryOp::Mul => l.checked_mul(r),
        BinaryOp::Div => l.checked_div(r),
        BinaryOp::Rem => l.checked_rem(r),
        BinaryOp::Pow => l.pow(r),
        BinaryOp::Lt => Ok(bool_q(l.checked_cmp(r)? == Less)),
        BinaryOp::Le => Ok(bool_q(l.checked_cmp(r)? != Greater)),
        BinaryOp::Gt => Ok(bool_q(l.checked_cmp(r)? == Greater)),
        BinaryOp::Ge => Ok(bool_q(l.checked_cmp(r)? != Less)),
        BinaryOp::Eq => Ok(bool_q(l.approx_eq(r)?)),
        BinaryOp::Ne => Ok(bool_q(!l.approx_eq(r)?)),
        BinaryOp::And | BinaryOp::Or => unreachable!("handled above"),
    }
}

/// Trig input: angles, or unitless numbers interpreted as radians.
fn angle_arg(func: &'static str, q: Quantity) -> Result<f64, EvalError> {
    if q.dim == Dim::ANGLE || q.is_unitless() {
        Ok(q.value)
    } else {
        Err(EvalError::BadArgument {
            func,
            expected: "an angle or a number",
            found: q.dim,
        })
    }
}

/// FeatureScript's `round`: `floor(x + 0.5)` (halves up, also below zero).
fn round_half_up(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn unitless_arg(func: &'static str, q: Quantity) -> Result<f64, EvalError> {
    if q.is_unitless() {
        Ok(q.value)
    } else {
        Err(EvalError::BadArgument {
            func,
            expected: "a unitless number",
            found: q.dim,
        })
    }
}

fn checked(func: &'static str, v: f64, dim: Dim) -> Result<Quantity, EvalError> {
    if v.is_nan() {
        Err(EvalError::Domain { func })
    } else if v.is_infinite() {
        Err(EvalError::NotFinite)
    } else {
        Ok(Quantity::new(v, dim))
    }
}

fn eval_call(f: Func, args: &[Quantity]) -> Result<Quantity, EvalError> {
    let name = f.name();
    let a = args[0];
    match f {
        Func::Sin => checked(name, angle_arg(name, a)?.sin(), Dim::UNITLESS),
        Func::Cos => checked(name, angle_arg(name, a)?.cos(), Dim::UNITLESS),
        Func::Tan => {
            let x = angle_arg(name, a)?;
            if x.cos().abs() < 1e-15 {
                return Err(EvalError::Domain { func: name });
            }
            checked(name, x.tan(), Dim::UNITLESS)
        }
        Func::Asin => checked(name, unitless_arg(name, a)?.asin(), Dim::ANGLE),
        Func::Acos => checked(name, unitless_arg(name, a)?.acos(), Dim::ANGLE),
        Func::Atan => checked(name, unitless_arg(name, a)?.atan(), Dim::ANGLE),
        Func::Atan2 => {
            let b = args[1];
            if a.dim != b.dim {
                return Err(EvalError::DimensionMismatch {
                    op: "atan2",
                    left: a.dim,
                    right: b.dim,
                });
            }
            checked(name, a.value.atan2(b.value), Dim::ANGLE)
        }
        Func::Sqrt => a.sqrt(),
        Func::Abs => Ok(a.abs()),
        Func::Floor | Func::Ceil | Func::Round => {
            // Live (FeatureScript): halves round up, `round(-2.5)` is -2.
            let op = |x: f64| match f {
                Func::Floor => x.floor(),
                Func::Ceil => x.ceil(),
                _ => round_half_up(x),
            };
            match args.get(1) {
                None => Ok(Quantity::unitless(op(unitless_arg(name, a)?))),
                Some(step) => {
                    if step.dim != a.dim {
                        return Err(EvalError::DimensionMismatch {
                            op: name,
                            left: a.dim,
                            right: step.dim,
                        });
                    }
                    if step.value == 0.0 {
                        return Err(EvalError::DivisionByZero);
                    }
                    checked(name, op(a.value / step.value) * step.value, a.dim)
                }
            }
        }
        Func::Min | Func::Max => {
            let mut best = a;
            for &x in &args[1..] {
                if x.dim != best.dim {
                    return Err(EvalError::DimensionMismatch {
                        op: name,
                        left: best.dim,
                        right: x.dim,
                    });
                }
                let better = match f {
                    Func::Min => x.value < best.value,
                    _ => x.value > best.value,
                };
                if better {
                    best = x;
                }
            }
            Ok(best)
        }
        Func::Exp => checked(name, unitless_arg(name, a)?.exp(), Dim::UNITLESS),
        // Live's FeatureScript semantics (behaviours "skb (10-25) expression
        // functions"): arguments of one kind; results keep it.
        Func::Clamp => {
            let (lo, hi) = (args[1], args[2]);
            for b in [lo, hi] {
                if b.dim != a.dim {
                    return Err(EvalError::DimensionMismatch { op: name, left: a.dim, right: b.dim });
                }
            }
            if lo.value > hi.value {
                return Err(EvalError::Domain { func: name });
            }
            Ok(if a.value < lo.value {
                lo
            } else if a.value > hi.value {
                hi
            } else {
                a
            })
        }
        Func::Hypot => {
            let b = args[1];
            if b.dim != a.dim {
                return Err(EvalError::DimensionMismatch { op: name, left: a.dim, right: b.dim });
            }
            checked(name, a.value.hypot(b.value), a.dim)
        }
        // A number with units is never an integer (live: `isInteger(3 mm)`
        // is false).
        Func::IsInteger => Ok(Quantity::unitless(f64::from(a.is_unitless() && a.value.is_finite() && a.value.fract() == 0.0))),
        Func::RoundToPrecision => {
            let x = unitless_arg(name, a)?;
            let p = unitless_arg(name, args[1])?;
            if p.fract() != 0.0 {
                return Err(EvalError::Domain { func: name });
            }
            let m = 10f64.powf(p);
            checked(name, round_half_up(x * m) / m, Dim::UNITLESS)
        }
        Func::TolerantEquals => {
            let b = args[1];
            if b.dim != a.dim {
                return Err(EvalError::DimensionMismatch { op: name, left: a.dim, right: b.dim });
            }
            // Live's tolerances: numbers 1e-13 (inclusive), lengths 1e-8 m,
            // angles 1e-11 rad, anything else 1e-8.
            let diff = (a.value - b.value).abs();
            let equal = if a.is_unitless() {
                diff <= 1e-13
            } else if a.dim == Dim::ANGLE {
                diff < 1e-11
            } else {
                diff < 1e-8
            };
            Ok(Quantity::unitless(f64::from(equal)))
        }
        Func::Ln => {
            let x = unitless_arg(name, a)?;
            if x <= 0.0 {
                return Err(EvalError::Domain { func: name });
            }
            checked(name, x.ln(), Dim::UNITLESS)
        }
        Func::Log10 => {
            let x = unitless_arg(name, a)?;
            if x <= 0.0 {
                return Err(EvalError::Domain { func: name });
            }
            checked(name, x.log10(), Dim::UNITLESS)
        }
    }
}

// ---------------------------------------------------------------------------
// Canonical printing
// ---------------------------------------------------------------------------

struct Paren<'a>(&'a Expr, bool);

impl fmt::Display for Paren<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.1 {
            write!(f, "({})", self.0)
        } else {
            write!(f, "{}", self.0)
        }
    }
}

fn is_number_with_unit(e: &Expr) -> bool {
    matches!(e, Expr::WithUnit { expr, .. } if matches!(**expr, Expr::Number(_)))
}

impl fmt::Display for Expr {
    /// Canonical form: single spaces around binary operators, canonical unit
    /// symbols, minimal parentheses. `parse(&e.to_string()) == Ok(e)` for
    /// every parsed expression.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Number(v) => {
                if v.is_sign_negative() {
                    write!(f, "-{}", -v)
                } else {
                    write!(f, "{v}")
                }
            }
            Expr::WithUnit { expr, unit, power } => {
                let simple = matches!(**expr, Expr::Number(v) if !v.is_sign_negative());
                write!(f, "{} {unit}", Paren(expr, !simple))?;
                if *power != 1 {
                    write!(f, "^{power}")?;
                }
                Ok(())
            }
            Expr::Unit(u) => write!(f, "{u}"),
            Expr::Constant(c) => f.write_str(c.name()),
            Expr::Var(n) => write!(f, "#{n}"),
            Expr::Unary(op, e) => {
                f.write_str(match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                })?;
                write!(f, "{}", Paren(e, e.precedence() < PREC_UNARY))
            }
            Expr::Binary(BinaryOp::Pow, a, b) => {
                // A number-with-unit base must be parenthesised, otherwise
                // `^n` would be read back as a unit power.
                let pa = a.precedence() <= PREC_POW || matches!(**a, Expr::WithUnit { .. });
                let pb = b.precedence() < PREC_UNARY;
                write!(f, "{} ^ {}", Paren(a, pa), Paren(b, pb))
            }
            Expr::Binary(op, a, b) => {
                let p = op.precedence();
                let pa = a.precedence() < p;
                // `1 / 4 in` would be read back as the fraction `(1/4) in`.
                let pb = b.precedence() <= p || (*op == BinaryOp::Div && is_number_with_unit(b));
                write!(f, "{} {} {}", Paren(a, pa), op.symbol(), Paren(b, pb))
            }
            Expr::Ternary(c, t, e) => {
                write!(
                    f,
                    "{} ? {} : {}",
                    Paren(c, c.precedence() <= PREC_TERNARY),
                    t,
                    Paren(e, e.precedence() < PREC_TERNARY)
                )
            }
            Expr::Call(func, args) => {
                write!(f, "{}(", func.name())?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{a}")?;
                }
                f.write_str(")")
            }
        }
    }
}
