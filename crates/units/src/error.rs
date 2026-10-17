//! Error types.

use std::fmt;
use std::ops::Range;

use crate::Dim;

/// What went wrong while parsing an expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// The input was empty or only whitespace.
    Empty,
    UnexpectedChar(char),
    /// Found a token where something else was required.
    UnexpectedToken {
        found: String,
        expected: String,
    },
    UnexpectedEnd {
        expected: String,
    },
    InvalidNumber(String),
    /// `#` not followed by a valid variable name.
    InvalidVariable,
    UnknownIdentifier(String),
    UnknownFunction(String),
    WrongArgCount {
        func: String,
        expected: String,
        found: usize,
    },
}

/// Parse failure with the byte span of the offending input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind} at {}..{}", span.start, span.end)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    /// Byte range into the source string.
    pub span: Range<usize>,
}

impl ParseError {
    pub(crate) fn new(kind: ParseErrorKind, span: Range<usize>) -> Self {
        ParseError { kind, span }
    }
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseErrorKind::Empty => write!(f, "empty expression"),
            ParseErrorKind::UnexpectedChar(c) => write!(f, "unexpected character `{c}`"),
            ParseErrorKind::UnexpectedToken { found, expected } => {
                write!(f, "expected {expected}, found `{found}`")
            }
            ParseErrorKind::UnexpectedEnd { expected } => {
                write!(f, "unexpected end of expression, expected {expected}")
            }
            ParseErrorKind::InvalidNumber(s) => write!(f, "invalid number `{s}`"),
            ParseErrorKind::InvalidVariable => {
                write!(f, "`#` must be followed by a variable name")
            }
            ParseErrorKind::UnknownIdentifier(s) => {
                write!(f, "unknown identifier `{s}` (variables are written `#{s}`)")
            }
            ParseErrorKind::UnknownFunction(s) => write!(f, "unknown function `{s}`"),
            ParseErrorKind::WrongArgCount {
                func,
                expected,
                found,
            } => {
                write!(f, "`{func}` takes {expected} argument(s), got {found}")
            }
        }
    }
}

/// Evaluation failure.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EvalError {
    #[error("undefined variable `#{0}`")]
    UndefinedVariable(String),
    #[error("cannot {op} {left} and {right}")]
    DimensionMismatch {
        op: &'static str,
        left: Dim,
        right: Dim,
    },
    #[error("expected {expected}, got {found}")]
    WrongDimension { expected: Dim, found: Dim },
    #[error("`{func}` expects {expected}, got {found}")]
    BadArgument {
        func: &'static str,
        expected: &'static str,
        found: Dim,
    },
    #[error("exponent must be unitless, got {0}")]
    ExponentNotUnitless(Dim),
    #[error("cannot raise {dim} to the power {exp}: the result would have fractional units")]
    FractionalUnits { dim: Dim, exp: f64 },
    #[error("unit exponent overflow")]
    DimensionOverflow,
    #[error("division by zero")]
    DivisionByZero,
    #[error("`{func}`: argument out of domain")]
    Domain { func: &'static str },
    #[error("result is not a finite number")]
    NotFinite,
}

/// Either a parse or an evaluation error.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ExprError {
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Eval(#[from] EvalError),
}
