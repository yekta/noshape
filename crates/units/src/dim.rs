//! Physical dimensions (exponents of length and angle).

use std::fmt;

use serde::{Deserialize, Serialize};

/// Largest absolute exponent we allow before reporting an overflow.
pub const MAX_EXPONENT: i8 = 64;

/// Dimension of a quantity expressed as integer exponents of the base
/// dimensions length and angle.
///
/// `Dim { length: 2, angle: 0 }` is an area, `Dim { length: 0, angle: 1 }` an
/// angle, and `Dim::UNITLESS` a plain number. Mixed dimensions such as
/// length·angle are representable (they arise as intermediate results, e.g.
/// `r * theta`), but fields only ever accept the plain kinds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Dim {
    pub length: i8,
    pub angle: i8,
}

impl Dim {
    pub const UNITLESS: Dim = Dim::new(0, 0);
    pub const LENGTH: Dim = Dim::new(1, 0);
    pub const AREA: Dim = Dim::new(2, 0);
    pub const VOLUME: Dim = Dim::new(3, 0);
    pub const ANGLE: Dim = Dim::new(0, 1);

    pub const fn new(length: i8, angle: i8) -> Dim {
        Dim { length, angle }
    }

    pub const fn is_unitless(self) -> bool {
        self.length == 0 && self.angle == 0
    }

    /// Dimension of a product. Returns `None` on exponent overflow.
    pub fn checked_mul(self, other: Dim) -> Option<Dim> {
        let d = Dim::new(
            self.length.checked_add(other.length)?,
            self.angle.checked_add(other.angle)?,
        );
        d.in_range().then_some(d)
    }

    /// Dimension of a quotient. Returns `None` on exponent overflow.
    pub fn checked_div(self, other: Dim) -> Option<Dim> {
        let d = Dim::new(
            self.length.checked_sub(other.length)?,
            self.angle.checked_sub(other.angle)?,
        );
        d.in_range().then_some(d)
    }

    /// Dimension of `self ^ exp` for a real exponent. Only succeeds when the
    /// resulting exponents are integers (e.g. `sqrt(area)` is a length but
    /// `sqrt(length)` is rejected).
    pub fn checked_powf(self, exp: f64) -> Option<Dim> {
        fn one(e: i8, exp: f64) -> Option<i8> {
            let r = f64::from(e) * exp;
            let rounded = r.round();
            if (r - rounded).abs() > 1e-9 || rounded.abs() > f64::from(MAX_EXPONENT) {
                return None;
            }
            Some(rounded as i8)
        }
        if self.is_unitless() {
            return Some(self);
        }
        Some(Dim::new(one(self.length, exp)?, one(self.angle, exp)?))
    }

    /// Reciprocal dimension.
    pub fn recip(self) -> Dim {
        Dim::new(-self.length, -self.angle)
    }

    fn in_range(self) -> bool {
        self.length.abs() <= MAX_EXPONENT && self.angle.abs() <= MAX_EXPONENT
    }

    /// Human readable name of the dimension ("length", "area", "angle", ...).
    pub fn name(self) -> String {
        match (self.length, self.angle) {
            (0, 0) => "unitless".into(),
            (1, 0) => "length".into(),
            (2, 0) => "area".into(),
            (3, 0) => "volume".into(),
            (0, 1) => "angle".into(),
            (l, a) => {
                let mut parts = Vec::new();
                if l != 0 {
                    parts.push(if l == 1 {
                        "length".to_string()
                    } else {
                        format!("length^{l}")
                    });
                }
                if a != 0 {
                    parts.push(if a == 1 {
                        "angle".to_string()
                    } else {
                        format!("angle^{a}")
                    });
                }
                parts.join("·")
            }
        }
    }
}

impl fmt::Display for Dim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_div() {
        assert_eq!(Dim::LENGTH.checked_mul(Dim::LENGTH), Some(Dim::AREA));
        assert_eq!(Dim::VOLUME.checked_div(Dim::AREA), Some(Dim::LENGTH));
        assert_eq!(Dim::LENGTH.checked_div(Dim::LENGTH), Some(Dim::UNITLESS));
        assert_eq!(Dim::new(60, 0).checked_mul(Dim::new(10, 0)), None);
        assert_eq!(Dim::LENGTH.recip(), Dim::new(-1, 0));
    }

    #[test]
    fn powf() {
        assert_eq!(Dim::AREA.checked_powf(0.5), Some(Dim::LENGTH));
        assert_eq!(Dim::LENGTH.checked_powf(0.5), None);
        assert_eq!(Dim::LENGTH.checked_powf(3.0), Some(Dim::VOLUME));
        assert_eq!(Dim::VOLUME.checked_powf(1.0 / 3.0), Some(Dim::LENGTH));
        assert_eq!(Dim::UNITLESS.checked_powf(0.37), Some(Dim::UNITLESS));
        assert_eq!(Dim::LENGTH.checked_powf(1000.0), None);
    }

    #[test]
    fn names() {
        assert_eq!(Dim::LENGTH.to_string(), "length");
        assert_eq!(Dim::ANGLE.to_string(), "angle");
        assert_eq!(Dim::UNITLESS.to_string(), "unitless");
        assert_eq!(Dim::new(1, 1).to_string(), "length·angle");
        assert_eq!(Dim::new(-1, 0).to_string(), "length^-1");
    }
}
