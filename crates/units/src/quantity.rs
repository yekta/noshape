//! Dimensioned values.

use std::cmp::Ordering;
use std::fmt;
use std::ops::{Div, Mul, Neg};

use serde::{Deserialize, Serialize};

use crate::{AngleUnit, Dim, EvalError, LengthUnit, Unit};

/// A value with a dimension. `value` is always in SI base units: meters for
/// lengths, radians for angles (so an area is in m², a volume in m³).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    pub value: f64,
    pub dim: Dim,
}

impl Quantity {
    pub const ZERO: Quantity = Quantity::unitless(0.0);

    pub const fn new(value: f64, dim: Dim) -> Quantity {
        Quantity { value, dim }
    }

    pub const fn unitless(value: f64) -> Quantity {
        Quantity {
            value,
            dim: Dim::UNITLESS,
        }
    }

    /// A length given in meters.
    pub const fn meters(value: f64) -> Quantity {
        Quantity {
            value,
            dim: Dim::LENGTH,
        }
    }

    pub fn mm(value: f64) -> Quantity {
        Quantity::length(value, LengthUnit::Millimeter)
    }

    pub fn inches(value: f64) -> Quantity {
        Quantity::length(value, LengthUnit::Inch)
    }

    pub fn length(value: f64, unit: LengthUnit) -> Quantity {
        Quantity::meters(value * unit.meters())
    }

    /// An angle given in radians.
    pub const fn radians(value: f64) -> Quantity {
        Quantity {
            value,
            dim: Dim::ANGLE,
        }
    }

    pub fn degrees(value: f64) -> Quantity {
        Quantity::angle(value, AngleUnit::Degree)
    }

    pub fn angle(value: f64, unit: AngleUnit) -> Quantity {
        Quantity::radians(value * unit.radians())
    }

    /// `value` expressed in `unit`.
    pub fn with_unit(value: f64, unit: Unit) -> Quantity {
        Quantity::new(value * unit.factor(), unit.dim())
    }

    pub fn is_unitless(&self) -> bool {
        self.dim.is_unitless()
    }

    /// Value in the given length unit, or `None` if this is not a length.
    pub fn in_length(&self, unit: LengthUnit) -> Option<f64> {
        (self.dim == Dim::LENGTH).then(|| self.value / unit.meters())
    }

    /// Value in the given angle unit, or `None` if this is not an angle.
    pub fn in_angle(&self, unit: AngleUnit) -> Option<f64> {
        (self.dim == Dim::ANGLE).then(|| self.value / unit.radians())
    }

    /// Value in the given unit (any dimension matching `unit`).
    pub fn in_unit(&self, unit: Unit) -> Option<f64> {
        (self.dim == unit.dim()).then(|| self.value / unit.factor())
    }

    /// Truthiness used by logical operators and the ternary operator.
    pub fn is_truthy(&self) -> bool {
        self.value != 0.0
    }

    pub fn checked_add(self, rhs: Quantity) -> Result<Quantity, EvalError> {
        self.same_dim("add", rhs)?;
        finite(Quantity::new(self.value + rhs.value, self.dim))
    }

    pub fn checked_sub(self, rhs: Quantity) -> Result<Quantity, EvalError> {
        self.same_dim("subtract", rhs)?;
        finite(Quantity::new(self.value - rhs.value, self.dim))
    }

    pub fn checked_mul(self, rhs: Quantity) -> Result<Quantity, EvalError> {
        let dim = self
            .dim
            .checked_mul(rhs.dim)
            .ok_or(EvalError::DimensionOverflow)?;
        finite(Quantity::new(self.value * rhs.value, dim))
    }

    pub fn checked_div(self, rhs: Quantity) -> Result<Quantity, EvalError> {
        if rhs.value == 0.0 {
            return Err(EvalError::DivisionByZero);
        }
        let dim = self
            .dim
            .checked_div(rhs.dim)
            .ok_or(EvalError::DimensionOverflow)?;
        finite(Quantity::new(self.value / rhs.value, dim))
    }

    /// Remainder of `self / rhs` (same dimension required), with the sign of
    /// the dividend like C `fmod`.
    pub fn checked_rem(self, rhs: Quantity) -> Result<Quantity, EvalError> {
        self.same_dim("take the remainder of", rhs)?;
        if rhs.value == 0.0 {
            return Err(EvalError::DivisionByZero);
        }
        finite(Quantity::new(self.value % rhs.value, self.dim))
    }

    /// `self ^ exp` for a real exponent. The resulting unit exponents must be
    /// integers (`(4 mm^2)^0.5` is fine, `(4 mm)^0.5` is not).
    pub fn powf(self, exp: f64) -> Result<Quantity, EvalError> {
        let dim = self
            .dim
            .checked_powf(exp)
            .ok_or(EvalError::FractionalUnits { dim: self.dim, exp })?;
        if self.value < 0.0 && exp.fract() != 0.0 {
            return Err(EvalError::Domain { func: "^" });
        }
        if self.value == 0.0 && exp < 0.0 {
            return Err(EvalError::DivisionByZero);
        }
        finite(Quantity::new(self.value.powf(exp), dim))
    }

    /// `self ^ exp` where `exp` is a quantity (must be unitless).
    pub fn pow(self, exp: Quantity) -> Result<Quantity, EvalError> {
        if !exp.is_unitless() {
            return Err(EvalError::ExponentNotUnitless(exp.dim));
        }
        self.powf(exp.value)
    }

    pub fn powi(self, exp: i32) -> Result<Quantity, EvalError> {
        self.powf(f64::from(exp))
    }

    pub fn sqrt(self) -> Result<Quantity, EvalError> {
        if self.value < 0.0 {
            return Err(EvalError::Domain { func: "sqrt" });
        }
        self.powf(0.5)
    }

    pub fn abs(self) -> Quantity {
        Quantity::new(self.value.abs(), self.dim)
    }

    /// Compare two quantities of the same dimension. A unitless exact zero
    /// compares against anything (so `#len > 0` works).
    pub fn checked_cmp(self, rhs: Quantity) -> Result<Ordering, EvalError> {
        let zero_ok =
            (self.is_unitless() && self.value == 0.0) || (rhs.is_unitless() && rhs.value == 0.0);
        if !zero_ok {
            self.same_dim("compare", rhs)?;
        }
        self.value
            .partial_cmp(&rhs.value)
            .ok_or(EvalError::NotFinite)
    }

    /// Tolerant equality: same dimension and values equal within a relative
    /// tolerance of 1e-12 (absolute 1e-15 near zero).
    pub fn approx_eq(self, rhs: Quantity) -> Result<bool, EvalError> {
        let zero_ok =
            (self.is_unitless() && self.value == 0.0) || (rhs.is_unitless() && rhs.value == 0.0);
        if !zero_ok {
            self.same_dim("compare", rhs)?;
        }
        let diff = (self.value - rhs.value).abs();
        let scale = self.value.abs().max(rhs.value.abs());
        Ok(diff <= 1e-15 || diff <= scale * 1e-12)
    }

    fn same_dim(self, op: &'static str, rhs: Quantity) -> Result<(), EvalError> {
        if self.dim == rhs.dim {
            Ok(())
        } else {
            Err(EvalError::DimensionMismatch {
                op,
                left: self.dim,
                right: rhs.dim,
            })
        }
    }
}

fn finite(q: Quantity) -> Result<Quantity, EvalError> {
    if q.value.is_finite() {
        Ok(q)
    } else {
        Err(EvalError::NotFinite)
    }
}

impl Mul for Quantity {
    type Output = Quantity;
    /// Unchecked product (dimension exponents saturate instead of erroring).
    fn mul(self, rhs: Quantity) -> Quantity {
        Quantity::new(
            self.value * rhs.value,
            Dim::new(
                self.dim.length.saturating_add(rhs.dim.length),
                self.dim.angle.saturating_add(rhs.dim.angle),
            ),
        )
    }
}

impl Mul<f64> for Quantity {
    type Output = Quantity;
    fn mul(self, rhs: f64) -> Quantity {
        Quantity::new(self.value * rhs, self.dim)
    }
}

impl Div for Quantity {
    type Output = Quantity;
    /// Unchecked quotient (IEEE semantics for division by zero).
    fn div(self, rhs: Quantity) -> Quantity {
        Quantity::new(
            self.value / rhs.value,
            Dim::new(
                self.dim.length.saturating_sub(rhs.dim.length),
                self.dim.angle.saturating_sub(rhs.dim.angle),
            ),
        )
    }
}

impl Div<f64> for Quantity {
    type Output = Quantity;
    fn div(self, rhs: f64) -> Quantity {
        Quantity::new(self.value / rhs, self.dim)
    }
}

impl Neg for Quantity {
    type Output = Quantity;
    fn neg(self) -> Quantity {
        Quantity::new(-self.value, self.dim)
    }
}

impl fmt::Display for Quantity {
    /// Debug-friendly SI rendering (`0.025 m`). Use [`crate::format_quantity`]
    /// for user-facing text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)?;
        let mut parts = Vec::new();
        if self.dim.length != 0 {
            parts.push(if self.dim.length == 1 {
                "m".to_string()
            } else {
                format!("m^{}", self.dim.length)
            });
        }
        if self.dim.angle != 0 {
            parts.push(if self.dim.angle == 1 {
                "rad".to_string()
            } else {
                format!("rad^{}", self.dim.angle)
            });
        }
        if !parts.is_empty() {
            write!(f, " {}", parts.join("*"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.0)
    }

    #[test]
    fn constructors() {
        assert!(close(Quantity::mm(25.4).value, 0.0254));
        assert!(close(
            Quantity::inches(1.0)
                .in_length(LengthUnit::Millimeter)
                .unwrap(),
            25.4
        ));
        assert!(close(Quantity::degrees(180.0).value, std::f64::consts::PI));
        assert_eq!(Quantity::degrees(1.0).in_length(LengthUnit::Meter), None);
        assert!(close(
            Quantity::radians(1.0).in_angle(AngleUnit::Degree).unwrap(),
            57.29577951308232
        ));
    }

    #[test]
    fn add_sub_require_same_dim() {
        let a = Quantity::mm(1.0);
        let b = Quantity::inches(1.0);
        assert!(close(a.checked_add(b).unwrap().value, 0.0264));
        assert!(close(b.checked_sub(a).unwrap().value, 0.0244));
        let err = a.checked_add(Quantity::degrees(1.0)).unwrap_err();
        assert!(matches!(
            err,
            EvalError::DimensionMismatch { op: "add", .. }
        ));
        assert!(a.checked_sub(Quantity::unitless(1.0)).is_err());
    }

    #[test]
    fn mul_div_combine() {
        let a = Quantity::mm(2.0);
        let area = a.checked_mul(a).unwrap();
        assert_eq!(area.dim, Dim::AREA);
        assert!(close(area.value, 4e-6));
        let back = area.checked_div(a).unwrap();
        assert_eq!(back.dim, Dim::LENGTH);
        assert_eq!(
            a.checked_div(Quantity::ZERO),
            Err(EvalError::DivisionByZero)
        );
        assert_eq!((a * a).dim, Dim::AREA);
        assert_eq!((a / a).dim, Dim::UNITLESS);
        assert_eq!((-a).value, -0.002);
        assert_eq!((a * 2.0).value, 0.004);
    }

    #[test]
    fn powers() {
        let a = Quantity::mm(3.0);
        let v = a.powi(3).unwrap();
        assert_eq!(v.dim, Dim::VOLUME);
        assert!(close(v.value, 27e-9));
        assert!(close(v.powf(1.0 / 3.0).unwrap().value, 0.003));
        assert_eq!(a.checked_mul(a).unwrap().sqrt().unwrap().dim, Dim::LENGTH);
        assert!(matches!(a.sqrt(), Err(EvalError::FractionalUnits { .. })));
        assert!(matches!(
            Quantity::unitless(-1.0).sqrt(),
            Err(EvalError::Domain { .. })
        ));
        assert!(matches!(a.pow(a), Err(EvalError::ExponentNotUnitless(_))));
        assert_eq!(a.powi(-1).unwrap().dim, Dim::new(-1, 0));
        assert_eq!(Quantity::ZERO.powi(-1), Err(EvalError::DivisionByZero));
        assert_eq!(
            Quantity::unitless(10.0).powf(1000.0),
            Err(EvalError::NotFinite)
        );
    }

    #[test]
    fn compare() {
        let a = Quantity::mm(1.0);
        assert_eq!(
            a.checked_cmp(Quantity::inches(1.0)).unwrap(),
            Ordering::Less
        );
        assert_eq!(a.checked_cmp(Quantity::ZERO).unwrap(), Ordering::Greater);
        assert!(a.checked_cmp(Quantity::unitless(1.0)).is_err());
        assert!(Quantity::inches(1.0).approx_eq(Quantity::mm(25.4)).unwrap());
        assert!(!a.approx_eq(Quantity::mm(1.001)).unwrap());
    }

    #[test]
    fn display_si() {
        assert_eq!(Quantity::meters(2.0).to_string(), "2 m");
        assert_eq!(Quantity::new(1.0, Dim::new(2, 1)).to_string(), "1 m^2*rad");
        assert_eq!(Quantity::unitless(3.5).to_string(), "3.5");
    }

    #[test]
    fn serde_round_trip() {
        let q = Quantity::mm(25.0);
        let s = serde_json::to_string(&q).unwrap();
        assert_eq!(serde_json::from_str::<Quantity>(&s).unwrap(), q);
    }
}
