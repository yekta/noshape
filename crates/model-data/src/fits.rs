//! Tolerance options of tolerant quantities (Onshape's ± "Tolerance
//! options" on Hole diameter, Extrude depth, Fillet radius, …) and limits
//! and fits for the Fit tolerance types: the deviations of a hole class
//! (H7) or shaft class (g6) at a nominal size come from Onshape's own fit
//! tables (ISO 286 in millimeters up to 500 mm, ANSI in inches up to
//! 19.69 in; see [`tables`]).

mod tables;

use crate::params::ParamValue;
use serde::{Deserialize, Serialize};

/// Fit standards (Onshape's "Standard" list, in its order).
pub const STANDARDS: [&str; 2] = ["ANSI", "ISO"];
/// The standard a new fit starts with.
pub const DEFAULT_STANDARD: &str = "ISO";
/// Fit types ("Fit type" list).
pub const FIT_TYPES: [&str; 4] = ["User defined", "Clearance", "Transition", "Interference"];
/// The "no shaft class" entry of the User defined shaft lists (its default).
pub const NO_SHAFT: &str = "None";

fn is_ansi(standard: &str) -> bool {
    standard == "ANSI"
}

fn class_table(ansi: bool, hole: bool) -> (&'static [f64], &'static [(&'static str, &'static [f64])]) {
    match (ansi, hole) {
        (false, true) => (&tables::ISO_HOLE_SIZES, tables::ISO_HOLES),
        (false, false) => (&tables::ISO_SHAFT_SIZES, tables::ISO_SHAFTS),
        (true, true) => (&tables::ANSI_HOLE_SIZES, tables::ANSI_HOLES),
        (true, false) => (&tables::ANSI_SHAFT_SIZES, tables::ANSI_SHAFTS),
    }
}

/// Preferred hole-basis fits (hole class, its shaft classes) of a fit type;
/// None for "User defined" (every class).
fn preferred(standard: &str, fit_type: &str) -> Option<&'static [(&'static str, &'static [&'static str])]> {
    let ansi = is_ansi(standard);
    Some(match fit_type {
        "Clearance" if ansi => tables::ANSI_CLEARANCE,
        "Clearance" => tables::ISO_CLEARANCE,
        "Transition" if ansi => tables::ANSI_TRANSITION,
        "Transition" => tables::ISO_TRANSITION,
        "Interference" if ansi => tables::ANSI_INTERFERENCE,
        "Interference" => tables::ISO_INTERFERENCE,
        _ => return None,
    })
}

/// Hole classes offered for a standard and fit type.
pub fn hole_classes(standard: &str, fit_type: &str) -> Vec<&'static str> {
    match preferred(standard, fit_type) {
        Some(p) => p.iter().map(|(h, _)| *h).collect(),
        None => class_table(is_ansi(standard), true).1.iter().map(|(c, _)| *c).collect(),
    }
}

/// Shaft classes offered for a standard, fit type and hole class ("None"
/// first in the User defined lists).
pub fn shaft_classes(standard: &str, fit_type: &str, hole: &str) -> Vec<&'static str> {
    match preferred(standard, fit_type) {
        Some(p) => p.iter().find(|(h, _)| *h == hole).map(|(_, s)| s.to_vec()).unwrap_or_default(),
        None => std::iter::once(NO_SHAFT).chain(class_table(is_ansi(standard), false).1.iter().map(|(c, _)| *c)).collect(),
    }
}

/// Why a fit has no limits (Onshape's regeneration errors).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitError {
    SizeTooLargeIso,
    SizeTooLargeAnsi,
    NotFound,
}

impl FitError {
    pub fn message(self) -> &'static str {
        match self {
            FitError::SizeTooLargeIso => "Nominal size is outside fit tolerance ranges. Values must be between 0 mm and 500 mm.",
            FitError::SizeTooLargeAnsi => "Nominal size is outside fit tolerance ranges. Values must be between 0 in and 19.69 in.",
            FitError::NotFound => "No fit tolerance selection available with these inputs.",
        }
    }
}

impl std::fmt::Display for FitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

/// Limits (lower, upper) in meters of a hole class (`hole_basis`) or shaft
/// class at a nominal size in meters (Onshape's `getFitToleranceLimits`:
/// the size range is the smallest one whose upper bound holds the size).
pub fn limits(nominal_m: f64, standard: &str, class: &str, hole_basis: bool) -> Result<(f64, f64), FitError> {
    let ansi = is_ansi(standard);
    if nominal_m.is_nan() || nominal_m < 0.0 || standard.is_empty() || class.is_empty() {
        return Err(FitError::NotFound);
    }
    let unit = if ansi { 0.0254 } else { 0.001 };
    let (sizes, classes) = class_table(ansi, hole_basis);
    let actual = nominal_m / unit - 1e-8;
    let Some(r) = sizes.iter().position(|s| *s >= actual) else {
        return Err(if ansi { FitError::SizeTooLargeAnsi } else { FitError::SizeTooLargeIso });
    };
    let (_, vals) = classes.iter().find(|(c, _)| *c == class).ok_or(FitError::NotFound)?;
    let (lo, up) = (vals[2 * r], vals[2 * r + 1]);
    if lo.is_nan() || up.is_nan() {
        return Err(FitError::NotFound);
    }
    // ISO deviations are tabulated in micrometres, ANSI ones in inches.
    let dev_unit = if ansi { unit } else { 1e-6 };
    Ok((lo * dev_unit, up * dev_unit))
}

/// ISO deviations (upper, lower) in meters of a hole class ("H7",
/// uppercase) or shaft class ("g6", lowercase) at a nominal size in meters.
pub fn deviations(nominal_m: f64, class: &str) -> Option<(f64, f64)> {
    let hole = class.chars().next().is_some_and(|c| c.is_ascii_uppercase());
    limits(nominal_m, "ISO", class, hole).ok().map(|(lo, up)| (up, lo))
}

/// Fit choice of a Fit tolerance: Standard ▸ Fit type ▸ Hole class ▸
/// Shaft class (Onshape's "Fit type" lookup table).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FitSpec {
    pub standard: String,
    pub fit_type: String,
    pub hole: String,
    /// "None" for no shaft class.
    pub shaft: String,
}

impl Default for FitSpec {
    fn default() -> Self {
        Self { standard: DEFAULT_STANDARD.into(), fit_type: FIT_TYPES[0].into(), hole: String::new(), shaft: String::new() }.normalized()
    }
}

impl FitSpec {
    /// The choice as the lookup table keeps it: each level keeps its value
    /// when the level's list offers it, else takes the level's default (ISO,
    /// the shaft class "None" in User defined lists, otherwise the first
    /// entry; live: User defined H7 → Clearance keeps H7 and picks g6, → ANSI
    /// keeps Clearance and H7 and picks f6).
    pub fn normalized(&self) -> FitSpec {
        let standard = if STANDARDS.contains(&self.standard.as_str()) { self.standard.clone() } else { DEFAULT_STANDARD.to_string() };
        let fit_type = if FIT_TYPES.contains(&self.fit_type.as_str()) { self.fit_type.clone() } else { FIT_TYPES[0].to_string() };
        let holes = hole_classes(&standard, &fit_type);
        let hole = if holes.contains(&self.hole.as_str()) { self.hole.clone() } else { holes.first().copied().unwrap_or_default().to_string() };
        let shafts = shaft_classes(&standard, &fit_type, &hole);
        let shaft = if shafts.contains(&self.shaft.as_str()) { self.shaft.clone() } else { shafts.first().copied().unwrap_or(NO_SHAFT).to_string() };
        FitSpec { standard, fit_type, hole, shaft }
    }

    /// The shaft class, None for "None".
    pub fn shaft_class(&self) -> Option<&str> {
        (self.shaft != NO_SHAFT && !self.shaft.is_empty()).then_some(self.shaft.as_str())
    }
}

/// Tolerance options of a tolerant quantity (Onshape: precision, tolerance
/// type, its values and, for diameters, the fit), stored as
/// `<param>__tolerance` JSON text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tolerance {
    pub kind: String,
    pub upper: String,
    pub lower: String,
    /// Decimal places; None = workspace precision.
    pub precision: Option<u8>,
    /// Fit choice (kept while another type is chosen, like Onshape's
    /// remembered lookup value).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<FitSpec>,
}

impl Default for Tolerance {
    /// Live (dom/dialog-hole-tolerance.txt): turning tolerances on gives
    /// "Workspace precision" and the type "Default" (no values); value
    /// fields start at 0 (FeatureScript `NONNEGATIVE_ZERO_DEFAULT_*_BOUNDS`).
    fn default() -> Self {
        Self { kind: "Default".into(), upper: "0".into(), lower: "0".into(), precision: None, fit: None }
    }
}

/// Fit tolerance types (diameters only).
pub const FIT_KINDS: [&str; 3] = ["Fit", "Fit with tolerance", "Fit (tolerance only)"];

/// Hole callout parts of a fit tolerance on a diameter.
#[derive(Clone, Debug, PartialEq)]
pub struct HoleFit {
    /// Hole class shown after the size ("H7"); None for Fit (tolerance only).
    pub class: Option<String>,
    /// Stacked deviations (upper, lower) in meters; None for Fit.
    pub deviations: Option<(f64, f64)>,
    /// Decimal places override of the tolerance.
    pub precision: Option<u8>,
}

impl Tolerance {
    /// Types of length and angle quantities (FeatureScript
    /// `LengthToleranceType` / `AngleToleranceType`).
    pub const KINDS: &'static [&'static str] = &["Default", "Symmetrical", "Deviation", "Limits", "Min", "Max", "Basic"];
    /// Types of diameters (`DiameterToleranceType`; live hole diameter list).
    pub const DIAMETER_KINDS: &'static [&'static str] = &["Default", "Symmetrical", "Deviation", "Limits", "Min", "Max", "Fit", "Fit with tolerance", "Fit (tolerance only)"];
    /// Precision choices (`PrecisionType`): None = "Workspace precision".
    pub const PRECISIONS: [(Option<u8>, &'static str); 8] =
        [(None, "Workspace precision"), (Some(0), "0"), (Some(1), "0.1"), (Some(2), "0.12"), (Some(3), "0.123"), (Some(4), "0.1234"), (Some(5), "0.12345"), (Some(6), "0.123456")];

    pub fn kinds(diameter: bool) -> &'static [&'static str] {
        if diameter { Self::DIAMETER_KINDS } else { Self::KINDS }
    }

    pub fn precision_label(p: Option<u8>) -> &'static str {
        Self::PRECISIONS.iter().find(|(v, _)| *v == p).map(|(_, l)| *l).unwrap_or("Workspace precision")
    }

    pub fn parse(v: Option<&ParamValue>) -> Option<Self> {
        match v {
            Some(ParamValue::Text(t)) => serde_json::from_str(t).ok(),
            _ => None,
        }
    }

    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn is_fit(&self) -> bool {
        FIT_KINDS.contains(&self.kind.as_str())
    }

    /// The fit choice (the default one when none was made yet).
    pub fn fit_spec(&self) -> FitSpec {
        self.fit.as_ref().map(FitSpec::normalized).unwrap_or_default()
    }

    /// Value fields for the type: (slot, label).
    pub fn value_labels(&self) -> Vec<(u8, &'static str)> {
        match self.kind.as_str() {
            "Symmetrical" => vec![(0, "Deviation")],
            "Deviation" | "Limits" => vec![(0, "Upper"), (1, "Lower")],
            _ => Vec::new(),
        }
    }

    /// Callout parts of a fit tolerance on a hole of `nominal_m` diameter:
    /// Fit → "Ø6.350 H7", Fit with tolerance → "Ø6.350 H7 +0.015/−0.000",
    /// Fit (tolerance only) → "Ø6.350 +0.015/−0.000" (live, dialogs/hole-35).
    /// Ok(None) for other types; Err when the fit has no limits at this size.
    pub fn hole_fit(&self, nominal_m: f64) -> Result<Option<HoleFit>, FitError> {
        if !self.is_fit() {
            return Ok(None);
        }
        let spec = self.fit_spec();
        let deviations = if self.kind == "Fit" {
            None
        } else {
            let (lo, up) = limits(nominal_m, &spec.standard, &spec.hole, true)?;
            Some((up, lo))
        };
        let class = (self.kind != "Fit (tolerance only)").then(|| spec.hole.clone());
        Ok(Some(HoleFit { class, deviations, precision: self.precision }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ISO deviations (upper, lower) in µm.
    fn um(nominal_mm: f64, class: &str) -> (i32, i32) {
        let (u, l) = deviations(nominal_mm / 1000.0, class).unwrap();
        ((u * 1e6).round() as i32, (l * 1e6).round() as i32)
    }

    #[test]
    fn iso_286_table_values() {
        // Holes.
        assert_eq!(um(25.0, "H7"), (21, 0));
        assert_eq!(um(10.0, "H7"), (15, 0));
        assert_eq!(um(6.35, "H7"), (15, 0));
        assert_eq!(um(50.0, "H8"), (39, 0));
        assert_eq!(um(10.0, "K7"), (5, -10));
        assert_eq!(um(15.0, "K7"), (6, -12));
        assert_eq!(um(15.0, "M7"), (0, -18));
        assert_eq!(um(15.0, "N7"), (-5, -23));
        assert_eq!(um(2.0, "N7"), (-4, -14));
        assert_eq!(um(6.0, "F8"), (28, 10));
        assert_eq!(um(25.0, "F7"), (41, 20));
        assert_eq!(um(10.0, "G7"), (20, 5));
        assert_eq!(um(6.35, "A9"), (316, 280));
        assert_eq!(um(100.0, "P7"), (-24, -59));
        assert_eq!(um(400.0, "H11"), (360, 0));
        // Shafts.
        assert_eq!(um(25.0, "g6"), (-7, -20));
        assert_eq!(um(10.0, "g6"), (-5, -14));
        assert_eq!(um(15.0, "g6"), (-6, -17));
        assert_eq!(um(25.0, "f7"), (-20, -41));
        assert_eq!(um(15.0, "k6"), (12, 1));
        assert_eq!(um(20.0, "p6"), (35, 22));
        assert_eq!(um(50.0, "s6"), (59, 43));
        assert_eq!(um(40.0, "h7"), (0, -25));
        // js6 at 18–30 mm: ±IT6/2 = ±6.5 µm.
        let (u, l) = deviations(0.02, "js6").unwrap();
        assert!((u - 6.5e-6).abs() < 1e-12 && (l + 6.5e-6).abs() < 1e-12);
        assert_eq!(um(30.0, "u6"), (61, 48));
        // Range bounds include their upper size.
        assert_eq!(um(3.0, "H7"), (10, 0));
        assert_eq!(um(3.0001, "H7"), (12, 0));
    }

    #[test]
    fn ansi_inch_table_values() {
        let thou = |inch: f64, class: &str, hole: bool| {
            let (lo, up) = limits(inch * 0.0254, "ANSI", class, hole).unwrap();
            ((up / 0.0254 * 1e5).round() as i32, (lo / 0.0254 * 1e5).round() as i32)
        };
        // 0.25 in: H7 +0.0006/0, f6 -0.0004/-0.0007 (ANSI B4.1 RC4 0.24–0.40).
        assert_eq!(thou(0.25, "H7", true), (60, 0));
        assert_eq!(thou(1.0, "H8", true), (120, 0));
        assert_eq!(thou(0.1, "g4", false), (-10, -25));
        assert!(limits(20.0 * 0.0254, "ANSI", "H7", true) == Err(FitError::SizeTooLargeAnsi));
    }

    #[test]
    fn errors() {
        assert_eq!(limits(0.6, "ISO", "H7", true), Err(FitError::SizeTooLargeIso));
        // T classes start above 24 mm.
        assert_eq!(limits(0.01, "ISO", "T5", true), Err(FitError::NotFound));
        assert!(limits(0.03, "ISO", "T5", true).is_ok());
        assert_eq!(limits(0.01, "ISO", "Q7", true), Err(FitError::NotFound));
        assert_eq!(FitError::SizeTooLargeIso.message(), "Nominal size is outside fit tolerance ranges. Values must be between 0 mm and 500 mm.");
    }

    #[test]
    fn class_lists_match_onshape() {
        // Live: dom/dialog-hole-tolerance-fit.txt, …-fit-results.txt.
        let iso_holes = hole_classes("ISO", "User defined");
        assert_eq!(iso_holes.len(), 148);
        assert_eq!(&iso_holes[..6], &["A9", "A10", "A11", "A12", "A13", "B8"]);
        assert_eq!(iso_holes.last(), Some(&"Z10"));
        let iso_shafts = shaft_classes("ISO", "User defined", "A9");
        assert_eq!(iso_shafts.len(), 150);
        assert_eq!(&iso_shafts[..3], &["None", "a9", "a10"]);
        assert_eq!(iso_shafts.last(), Some(&"z9"));
        assert_eq!(hole_classes("ISO", "Clearance"), ["H7", "H8", "H9", "H11"]);
        assert_eq!(hole_classes("ANSI", "Clearance"), ["H5", "H6", "H7", "H8", "H9", "H10", "H11"]);
        assert_eq!(shaft_classes("ANSI", "Clearance", "H7"), ["f6", "h6"]);
    }

    #[test]
    fn fit_choice_cascade() {
        let d = FitSpec::default();
        assert_eq!((d.standard.as_str(), d.fit_type.as_str(), d.hole.as_str(), d.shaft.as_str()), ("ISO", "User defined", "A9", "None"));
        let h7 = FitSpec { hole: "H7".into(), ..d }.normalized();
        let clearance = FitSpec { fit_type: "Clearance".into(), ..h7 }.normalized();
        assert_eq!((clearance.hole.as_str(), clearance.shaft.as_str()), ("H7", "g6"));
        let ansi = FitSpec { standard: "ANSI".into(), ..clearance }.normalized();
        assert_eq!((ansi.fit_type.as_str(), ansi.hole.as_str(), ansi.shaft.as_str()), ("Clearance", "H7", "f6"));
        assert_eq!(ansi.shaft_class(), Some("f6"));
    }

    #[test]
    fn tolerance_parse_and_callout() {
        // Values saved before fits and Default existed still parse.
        let old = Tolerance::parse(Some(&ParamValue::Text(r#"{"kind":"Symmetrical","upper":"0.1","lower":"0.1","precision":null}"#.into()))).unwrap();
        assert_eq!((old.kind.as_str(), old.upper.as_str(), old.fit.is_none()), ("Symmetrical", "0.1", true));
        let mut t = Tolerance { kind: "Fit with tolerance".into(), fit: Some(FitSpec { hole: "H7".into(), ..FitSpec::default() }), ..Tolerance::default() };
        let back = Tolerance::parse(Some(&ParamValue::Text(t.encode()))).unwrap();
        assert_eq!(back, t);
        let f = t.hole_fit(0.00635).unwrap().unwrap();
        assert_eq!(f.class.as_deref(), Some("H7"));
        let (u, l) = f.deviations.unwrap();
        assert!((u - 15e-6).abs() < 1e-12 && l.abs() < 1e-12);
        t.kind = "Fit".into();
        assert_eq!(t.hole_fit(0.00635).unwrap().unwrap().deviations, None);
        t.kind = "Fit (tolerance only)".into();
        assert_eq!(t.hole_fit(0.00635).unwrap().unwrap().class, None);
        t.kind = "Symmetrical".into();
        assert_eq!(t.hole_fit(0.00635), Ok(None));
        assert_eq!(Tolerance::precision_label(Some(3)), "0.123");
    }
}
