//! noshape-units: dimensioned quantities, units and the Onshape-compatible
//! expression language used by every numeric field.
//!
//! ```
//! use noshape_units::*;
//! let prefs = UnitPrefs::default(); // mm, deg
//! let mut vars = std::collections::HashMap::new();
//! vars.insert("width".to_string(), Quantity::mm(40.0));
//! let q = evaluate_as("#width / 2 + 1/4 in", Dim::LENGTH, &prefs, &vars).unwrap();
//! assert_eq!(format_quantity(q, &prefs), "26.35 mm");
//! // Bare numbers take the field's default unit:
//! let q = evaluate_as("10", Dim::ANGLE, &prefs, &EmptyScope).unwrap();
//! assert_eq!(format_quantity(q, &prefs), "10°");
//! ```
//!
//! Language summary:
//! * numbers `12`, `1.5`, `.5`, `1e-3`; units with or without a space
//!   (`25mm`, `25 mm`), unit powers (`4 mm^2`, `4 mm²`), fractions
//!   (`1/4 in`, `1 1/2 in`), mixed units (`1 ft 3 in`, `5' 3"`), `30°`;
//! * units: `um`/`µm`, `mm`, `cm`, `m`, `in`/`"`, `ft`/`'`, `yd`, `deg`/`°`,
//!   `rad` (plus spelled-out names); a bare unit is a value (`#x * mm`);
//! * operators `+ - * / % ^`, unary `-`, comparisons `< <= > >= == !=`,
//!   logic `&& || !` (also `and`, `or`, `not`), ternary `c ? a : b`;
//! * variables `#name`, constants `PI`/`pi`/`π`, `true`, `false`;
//! * functions `sin cos tan` (angles; bare numbers are radians), `asin acos
//!   atan atan2` (return angles), `sqrt abs floor ceil round` (the last three
//!   take an optional step: `round(#x, 0.5 mm)`), `min max` (variadic), `exp
//!   ln log log10`.
//!
//! Booleans are unitless 1/0; any non-zero value is true.

pub mod complete;
mod dim;
mod error;
mod expr;
mod format;
mod lexer;
mod parser;
mod quantity;
mod scope;
mod unit;
mod value;
mod variables;

#[cfg(test)]
mod tests;

pub use dim::Dim;
pub use error::{EvalError, ExprError, ParseError, ParseErrorKind};
pub use expr::{BinaryOp, Constant, Expr, Func, UnaryOp};
pub use format::{format_number, format_quantity, format_quantity_exact, format_quantity_with};
pub use parser::parse;
pub use quantity::Quantity;
pub use scope::{EmptyScope, FnScope, LayeredScope, VariableScope};
pub use unit::{AngleUnit, LengthUnit, Unit, UnitPrefs, UnknownUnit};
pub use value::ExprValue;
pub use variables::{
    EvalOrder, TableEvaluation, VarError, VariableDef, VariableTable, is_valid_variable_name,
};

/// Parses and evaluates `text` without any unit defaulting.
pub fn evaluate(text: &str, scope: &dyn VariableScope) -> Result<Quantity, ExprError> {
    Ok(parse(text)?.eval(scope)?)
}

/// Parses and evaluates `text` for a field of dimension `expected`.
///
/// Onshape behaviour: if the result is unitless and the field expects a
/// dimensioned value, the document's default unit is applied (`10` in a
/// length field with mm preferences is 10 mm; in an area field 10 mm²; in an
/// angle field 10°). Any other dimension mismatch is an
/// [`EvalError::WrongDimension`] error.
pub fn evaluate_as(
    text: &str,
    expected: Dim,
    prefs: &UnitPrefs,
    scope: &dyn VariableScope,
) -> Result<Quantity, ExprError> {
    let expr = parse(text)?;
    Ok(evaluate_expr_as(&expr, expected, prefs, scope)?)
}

/// [`evaluate_as`] for an already parsed expression.
pub fn evaluate_expr_as(
    expr: &Expr,
    expected: Dim,
    prefs: &UnitPrefs,
    scope: &dyn VariableScope,
) -> Result<Quantity, EvalError> {
    let q = expr.eval(scope)?;
    apply_default_units(q, expected, prefs)
}

/// Applies the default unit to a unitless value for a field of dimension
/// `expected`, or checks that `q` already has that dimension.
pub fn apply_default_units(
    q: Quantity,
    expected: Dim,
    prefs: &UnitPrefs,
) -> Result<Quantity, EvalError> {
    if q.dim == expected {
        Ok(q)
    } else if q.is_unitless() {
        Ok(Quantity::new(q.value * prefs.si_factor(expected), expected))
    } else {
        Err(EvalError::WrongDimension {
            expected,
            found: q.dim,
        })
    }
}

impl Expr {
    /// Evaluates for a field of dimension `expected`; see [`evaluate_as`].
    pub fn eval_as(
        &self,
        expected: Dim,
        prefs: &UnitPrefs,
        scope: &dyn VariableScope,
    ) -> Result<Quantity, EvalError> {
        evaluate_expr_as(self, expected, prefs, scope)
    }
}
