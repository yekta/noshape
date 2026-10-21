//! Serializable expression-carrying field value.

use serde::{Deserialize, Serialize};

use crate::{
    Dim, Expr, ExprError, ParseError, Quantity, UnitPrefs, VariableScope, evaluate_expr_as,
};

/// The value of a numeric feature/sketch field: the expression text the user
/// typed plus the last successfully evaluated SI value.
///
/// Only the text is authoritative; `cached` lets documents load and display
/// without re-evaluating, and is refreshed by [`ExprValue::evaluate`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExprValue {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached: Option<Quantity>,
}

impl ExprValue {
    /// A value from expression text (not yet evaluated).
    /// Typed text in the comma form (Preferences › Decimal format) is
    /// stored canonically.
    pub fn new(text: impl Into<String>) -> Self {
        ExprValue {
            text: crate::delocalize(&text.into()),
            cached: None,
        }
    }

    /// A literal value formatted in the preferred units, with the cache set.
    pub fn from_quantity(q: Quantity, prefs: &UnitPrefs) -> Self {
        ExprValue {
            text: crate::delocalize(&crate::format_quantity_exact(q, prefs)),
            cached: Some(q),
        }
    }

    pub fn parse(&self) -> Result<Expr, ParseError> {
        crate::parse(&self.text)
    }

    /// Variables referenced by the text (empty if it does not parse).
    pub fn variables(&self) -> Vec<String> {
        self.parse().map(|e| e.variables()).unwrap_or_default()
    }

    /// Evaluates the text as a field of dimension `expected` (bare numbers
    /// take the default unit, see [`crate::evaluate_as`]) and updates the
    /// cache on success. On failure the previous cache is kept.
    pub fn evaluate(
        &mut self,
        expected: Dim,
        prefs: &UnitPrefs,
        scope: &dyn VariableScope,
    ) -> Result<Quantity, ExprError> {
        let expr = self.parse()?;
        let q = evaluate_expr_as(&expr, expected, prefs, scope)?;
        self.cached = Some(q);
        Ok(q)
    }

    /// Replaces the text and clears the cache.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cached = None;
    }
}

impl From<&str> for ExprValue {
    fn from(s: &str) -> Self {
        ExprValue::new(s)
    }
}

impl From<String> for ExprValue {
    fn from(s: String) -> Self {
        ExprValue::new(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EmptyScope;

    #[test]
    fn evaluate_updates_cache() {
        let prefs = UnitPrefs::default();
        let mut v = ExprValue::new("10");
        let q = v.evaluate(Dim::LENGTH, &prefs, &EmptyScope).unwrap();
        assert!((q.value - 0.01).abs() < 1e-15);
        assert_eq!(v.cached, Some(q));
        v.set_text("#nope");
        assert_eq!(v.cached, None);
        assert!(v.evaluate(Dim::LENGTH, &prefs, &EmptyScope).is_err());
        assert_eq!(v.variables(), vec!["nope".to_string()]);
    }

    #[test]
    fn serde_round_trip() {
        let prefs = UnitPrefs::default();
        let v = ExprValue::from_quantity(Quantity::mm(25.0), &prefs);
        assert_eq!(v.text, "25 mm");
        let json = serde_json::to_string(&v).unwrap();
        let back: ExprValue = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v);
        let bare: ExprValue = serde_json::from_str(r##"{"text":"#a + 1 mm"}"##).unwrap();
        assert_eq!(bare.cached, None);
        assert_eq!(
            serde_json::to_string(&ExprValue::new("1")).unwrap(),
            r#"{"text":"1"}"#
        );
    }
}
