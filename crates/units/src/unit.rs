//! Length and angle units and per-document unit preferences.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::Dim;

/// A unit of length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LengthUnit {
    Micrometer,
    Millimeter,
    Centimeter,
    Meter,
    Inch,
    Foot,
    Yard,
}

impl LengthUnit {
    pub const ALL: [LengthUnit; 7] = [
        LengthUnit::Micrometer,
        LengthUnit::Millimeter,
        LengthUnit::Centimeter,
        LengthUnit::Meter,
        LengthUnit::Inch,
        LengthUnit::Foot,
        LengthUnit::Yard,
    ];

    /// Size of one unit in meters.
    pub const fn meters(self) -> f64 {
        match self {
            LengthUnit::Micrometer => 1e-6,
            LengthUnit::Millimeter => 1e-3,
            LengthUnit::Centimeter => 1e-2,
            LengthUnit::Meter => 1.0,
            LengthUnit::Inch => 0.0254,
            LengthUnit::Foot => 0.3048,
            LengthUnit::Yard => 0.9144,
        }
    }

    /// Canonical symbol used in expressions and display.
    pub const fn symbol(self) -> &'static str {
        match self {
            LengthUnit::Micrometer => "um",
            LengthUnit::Millimeter => "mm",
            LengthUnit::Centimeter => "cm",
            LengthUnit::Meter => "m",
            LengthUnit::Inch => "in",
            LengthUnit::Foot => "ft",
            LengthUnit::Yard => "yd",
        }
    }

    /// Display name ("Millimeter").
    pub const fn name(self) -> &'static str {
        match self {
            LengthUnit::Micrometer => "Micrometer",
            LengthUnit::Millimeter => "Millimeter",
            LengthUnit::Centimeter => "Centimeter",
            LengthUnit::Meter => "Meter",
            LengthUnit::Inch => "Inch",
            LengthUnit::Foot => "Foot",
            LengthUnit::Yard => "Yard",
        }
    }
}

/// A unit of angle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AngleUnit {
    Degree,
    Radian,
}

impl AngleUnit {
    pub const ALL: [AngleUnit; 2] = [AngleUnit::Degree, AngleUnit::Radian];

    /// Size of one unit in radians.
    pub const fn radians(self) -> f64 {
        match self {
            AngleUnit::Degree => std::f64::consts::PI / 180.0,
            AngleUnit::Radian => 1.0,
        }
    }

    /// Canonical symbol used in expressions.
    pub const fn symbol(self) -> &'static str {
        match self {
            AngleUnit::Degree => "deg",
            AngleUnit::Radian => "rad",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            AngleUnit::Degree => "Degree",
            AngleUnit::Radian => "Radian",
        }
    }
}

/// Any unit that can be written after a number in an expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Unit {
    Length(LengthUnit),
    Angle(AngleUnit),
}

impl Unit {
    pub const fn dim(self) -> Dim {
        match self {
            Unit::Length(_) => Dim::LENGTH,
            Unit::Angle(_) => Dim::ANGLE,
        }
    }

    /// SI size of one unit (meters or radians).
    pub const fn factor(self) -> f64 {
        match self {
            Unit::Length(u) => u.meters(),
            Unit::Angle(u) => u.radians(),
        }
    }

    pub const fn symbol(self) -> &'static str {
        match self {
            Unit::Length(u) => u.symbol(),
            Unit::Angle(u) => u.symbol(),
        }
    }

    /// Looks up a unit by any of its accepted spellings (`mm`, `inch`, `"`,
    /// `°`, `µm`, ...). Case sensitive except for spelled-out names.
    pub fn from_symbol(s: &str) -> Option<Unit> {
        use AngleUnit::*;
        use LengthUnit::*;
        let u = match s {
            "um" | "µm" | "μm" | "micron" | "microns" => Unit::Length(Micrometer),
            "mm" => Unit::Length(Millimeter),
            "cm" => Unit::Length(Centimeter),
            "m" => Unit::Length(Meter),
            "in" | "\"" => Unit::Length(Inch),
            "ft" | "'" => Unit::Length(Foot),
            "yd" => Unit::Length(Yard),
            "deg" | "°" => Unit::Angle(Degree),
            "rad" => Unit::Angle(Radian),
            _ => {
                let lower = s.to_ascii_lowercase();
                match lower.as_str() {
                    "micrometer" | "micrometers" | "micrometre" | "micrometres" => {
                        Unit::Length(Micrometer)
                    }
                    "millimeter" | "millimeters" | "millimetre" | "millimetres" => {
                        Unit::Length(Millimeter)
                    }
                    "centimeter" | "centimeters" | "centimetre" | "centimetres" => {
                        Unit::Length(Centimeter)
                    }
                    "meter" | "meters" | "metre" | "metres" => Unit::Length(Meter),
                    "inch" | "inches" => Unit::Length(Inch),
                    "foot" | "feet" => Unit::Length(Foot),
                    "yard" | "yards" => Unit::Length(Yard),
                    "degree" | "degrees" => Unit::Angle(Degree),
                    "radian" | "radians" => Unit::Angle(Radian),
                    _ => return None,
                }
            }
        };
        Some(u)
    }
}

impl From<LengthUnit> for Unit {
    fn from(u: LengthUnit) -> Unit {
        Unit::Length(u)
    }
}

impl From<AngleUnit> for Unit {
    fn from(u: AngleUnit) -> Unit {
        Unit::Angle(u)
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

impl fmt::Display for LengthUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

impl fmt::Display for AngleUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

/// Error returned when parsing a unit name fails.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown unit `{0}`")]
pub struct UnknownUnit(pub String);

impl FromStr for Unit {
    type Err = UnknownUnit;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Unit::from_symbol(s.trim()).ok_or_else(|| UnknownUnit(s.to_string()))
    }
}

impl FromStr for LengthUnit {
    type Err = UnknownUnit;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Unit::from_symbol(s.trim()) {
            Some(Unit::Length(u)) => Ok(u),
            _ => Err(UnknownUnit(s.to_string())),
        }
    }
}

impl FromStr for AngleUnit {
    type Err = UnknownUnit;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Unit::from_symbol(s.trim()) {
            Some(Unit::Angle(u)) => Ok(u),
            _ => Err(UnknownUnit(s.to_string())),
        }
    }
}

/// Unit preferences of a document (Onshape: Document > Workspace units).
///
/// Bare numbers typed into a length or angle field take these units, and
/// values are displayed in them with the given number of decimals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct UnitPrefs {
    pub length: LengthUnit,
    pub angle: AngleUnit,
    /// Decimal places shown for lengths (and unitless numbers).
    pub length_precision: u8,
    /// Decimal places shown for angles.
    pub angle_precision: u8,
    /// Onshape's other workspace units, in [`OTHER_UNITS`] order: (index of
    /// the default unit in its option list, display decimals).
    pub other: [(u8, u8); 9],
}

/// The other quantities of Onshape's Workspace units dialog: (label, group,
/// unit names, metric default, imperial default). Group 0 = Length, 1 =
/// Angle, 2 = Mechanical.
pub const OTHER_UNITS: [(&str, u8, &[&str], u8, u8); 9] = [
    ("Linear acceleration", 0, &["Meter per second squared", "Millimeter per second squared", "Centimeter per second squared", "Inch per second squared", "Foot per second squared"], 1, 3),
    ("Angular velocity", 1, &["Degree per second", "Radian per second", "Revolutions per minute"], 0, 0),
    ("Mass", 2, &["Kilogram", "Gram", "Pound", "Ounce"], 0, 2),
    ("Density", 2, &["Kilogram per cubic meter", "Gram per cubic centimeter", "Pound per cubic inch", "Pound per cubic foot", "Gram per liter", "Gram per milliliter"], 0, 2),
    ("Force", 2, &["Newton", "Kilonewton", "Pound-force"], 0, 2),
    ("Frequency", 2, &["Hertz", "Kilohertz"], 0, 0),
    ("Moment", 2, &["Newton-meter", "Newton-millimeter", "Inch-pound", "Foot-pound"], 0, 2),
    ("Pressure", 2, &["Pascal", "Kilopascal", "Megapascal", "Pound per square inch", "Gigapascal", "Kilopound per square inch"], 0, 3),
    ("Energy", 2, &["Joule", "Kilojoule", "Foot-pound force", "Inch-pound force"], 0, 2),
];

/// Onshape's option lists of [`OTHER_UNITS`] (Workspace units, Preferences ›
/// Units; live dom/dialog-workspace-units-options.txt): indices into each
/// list, in Onshape's alphabetical order. The stored indices never change
/// (documents keep them), so units Onshape does not offer are left out here
/// and new ones are appended to the lists above.
pub const OTHER_UNITS_SHOWN: [&[u8]; 9] = [
    &[4, 3, 0, 1],
    &[0, 1, 2],
    &[1, 0, 3, 2],
    &[1, 4, 5, 0, 3, 2],
    &[1, 0, 2],
    &[0],
    &[3, 2, 0, 1],
    &[4, 1, 5, 2, 0, 3],
    &[2, 0],
];

/// Onshape's Length default unit list, in its order.
pub const LENGTH_UNITS_SHOWN: [LengthUnit; 6] = [LengthUnit::Centimeter, LengthUnit::Foot, LengthUnit::Inch, LengthUnit::Meter, LengthUnit::Millimeter, LengthUnit::Yard];

impl Default for UnitPrefs {
    fn default() -> Self {
        UnitPrefs {
            length: LengthUnit::Millimeter,
            angle: AngleUnit::Degree,
            length_precision: 3,
            angle_precision: 3,
            // Live (Preferences › Units, behaviours 2665; a mm document's
            // Workspace units, 6760): Onshape's defaults for the other units
            // are the imperial ones (Pound, Pound per cubic inch, Pound-force…),
            // kept when only the length is switched to millimeters, so mass
            // properties read "(lb mm²)" (qa5 P1-14).
            other: OTHER_UNITS.map(|o| (o.4, 3)),
        }
    }
}

impl UnitPrefs {
    /// Conversion factor from a value expressed in the preferred units to SI
    /// for the given dimension (`mm^2` for areas, `deg` for angles, ...).
    pub fn si_factor(&self, dim: Dim) -> f64 {
        self.length.meters().powi(i32::from(dim.length))
            * self.angle.radians().powi(i32::from(dim.angle))
    }

    /// Inch / degree preferences.
    pub fn imperial() -> Self {
        UnitPrefs {
            length: LengthUnit::Inch,
            other: OTHER_UNITS.map(|o| (o.4, 3)),
            ..Default::default()
        }
    }

    /// Name of the default unit of `OTHER_UNITS[i]`.
    pub fn other_unit_name(&self, i: usize) -> &'static str {
        let (_, _, names, d, _) = OTHER_UNITS[i];
        names.get(self.other[i].0 as usize).or(names.get(d as usize)).copied().unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live: a millimeter document still has Onshape's default (imperial)
    /// mechanical units: Mass Pound (qa5 P1-14, "(lb mm²)").
    #[test]
    fn default_mass_unit_is_pound() {
        let u = UnitPrefs::default();
        assert_eq!(u.length, LengthUnit::Millimeter);
        assert_eq!(u.other_unit_name(2), "Pound");
    }

    #[test]
    fn shown_unit_lists_are_onshapes() {
        // Live dom/dialog-workspace-units-options.txt, in its order.
        let live: [&[&str]; 9] = [
            &["Foot per second squared", "Inch per second squared", "Meter per second squared", "Millimeter per second squared"],
            &["Degree per second", "Radian per second", "Revolutions per minute"],
            &["Gram", "Kilogram", "Ounce", "Pound"],
            &["Gram per cubic centimeter", "Gram per liter", "Gram per milliliter", "Kilogram per cubic meter", "Pound per cubic foot", "Pound per cubic inch"],
            &["Kilonewton", "Newton", "Pound-force"],
            &["Hertz"],
            &["Foot-pound", "Inch-pound", "Newton-meter", "Newton-millimeter"],
            &["Gigapascal", "Kilopascal", "Kilopound per square inch", "Megapascal", "Pascal", "Pound per square inch"],
            &["Foot-pound force", "Joule"],
        ];
        for (k, shown) in OTHER_UNITS_SHOWN.iter().enumerate() {
            let names: Vec<&str> = shown.iter().map(|&i| OTHER_UNITS[k].2[i as usize]).collect();
            assert_eq!(names, live[k]);
            // The metric and imperial defaults are offered.
            assert!(shown.contains(&OTHER_UNITS[k].3) && shown.contains(&OTHER_UNITS[k].4));
        }
        let lengths: Vec<&str> = LENGTH_UNITS_SHOWN.iter().map(|u| u.name()).collect();
        assert_eq!(lengths, ["Centimeter", "Foot", "Inch", "Meter", "Millimeter", "Yard"]);
    }

    #[test]
    fn symbols_round_trip() {
        for u in LengthUnit::ALL {
            assert_eq!(Unit::from_symbol(u.symbol()), Some(Unit::Length(u)));
            assert_eq!(u.symbol().parse::<LengthUnit>().unwrap(), u);
        }
        for u in AngleUnit::ALL {
            assert_eq!(Unit::from_symbol(u.symbol()), Some(Unit::Angle(u)));
        }
    }

    #[test]
    fn aliases() {
        assert_eq!(
            Unit::from_symbol("\""),
            Some(Unit::Length(LengthUnit::Inch))
        );
        assert_eq!(Unit::from_symbol("'"), Some(Unit::Length(LengthUnit::Foot)));
        assert_eq!(Unit::from_symbol("°"), Some(Unit::Angle(AngleUnit::Degree)));
        assert_eq!(
            Unit::from_symbol("µm"),
            Some(Unit::Length(LengthUnit::Micrometer))
        );
        assert_eq!(
            Unit::from_symbol("Inches"),
            Some(Unit::Length(LengthUnit::Inch))
        );
        assert_eq!(Unit::from_symbol("MM"), None);
        assert!("furlong".parse::<Unit>().is_err());
        assert!("deg".parse::<LengthUnit>().is_err());
    }

    #[test]
    fn factors() {
        assert_eq!(LengthUnit::Inch.meters(), 0.0254);
        assert!((AngleUnit::Degree.radians() * 180.0 - std::f64::consts::PI).abs() < 1e-15);
        let p = UnitPrefs::default();
        assert!((p.si_factor(Dim::AREA) - 1e-6).abs() < 1e-20);
        assert_eq!(p.si_factor(Dim::UNITLESS), 1.0);
    }

    #[test]
    fn prefs_serde_defaults() {
        let p: UnitPrefs = serde_json::from_str(r#"{"length":"Inch"}"#).unwrap();
        assert_eq!(p.length, LengthUnit::Inch);
        assert_eq!(p.angle, AngleUnit::Degree);
    }
}
