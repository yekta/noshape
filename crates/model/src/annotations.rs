//! Part Studio annotations from the Inspection panel tools (live psb 10-20
//! r, psb/r-900–944). They aren't features: they show in the graphics area
//! and in the Model definitions panel only. A datum is a boxed letter over a
//! filled triangle, lying in the plane of the face it marks, at a constant
//! on-screen size.

use crate::reference::Reference;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnnotationKind {
    Datum { letter: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub id: u32,
    pub kind: AnnotationKind,
    /// The face it is attached to.
    pub face: Reference,
    /// Where the face was picked (world).
    pub at: [f64; 3],
    /// The face plane's outward normal and the in-plane "up" of the symbol
    /// (screen-up when it was placed).
    pub normal: [f64; 3],
    pub up: [f64; 3],
    /// The letter box moved away from the triangle (screen pixels, in the
    /// symbol's plane axes), drawn with a dashed leader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f64; 2]>,
}

/// The next free datum letter: A, B, … Z, then AA, AB, …
pub fn next_datum_letter(existing: &[Annotation]) -> String {
    let used: Vec<&str> = existing
        .iter()
        .map(|a| match &a.kind {
            AnnotationKind::Datum { letter } => letter.as_str(),
        })
        .collect();
    let name = |mut n: usize| {
        let mut s = String::new();
        loop {
            s.insert(0, (b'A' + (n % 26) as u8) as char);
            if n < 26 {
                break;
            }
            n = n / 26 - 1;
        }
        s
    };
    (0..).map(name).find(|l| !used.contains(&l.as_str())).unwrap_or_default()
}

/// The datum letters, in order of creation.
pub fn datum_letters(all: &[Annotation]) -> Vec<String> {
    all.iter()
        .map(|a| match &a.kind {
            AnnotationKind::Datum { letter } => letter.clone(),
        })
        .collect()
}

/// A symbol's outline in its own plane, in screen pixels (x right, y up),
/// the triangle's apex at the origin's base: (box rectangle corners,
/// triangle) — live psb/r-914: a box about 18×28 px with the letter in its
/// upper part and the filled triangle at its bottom edge.
pub const DATUM_BOX: [[f64; 2]; 2] = [[-9.0, 0.0], [9.0, 26.0]];
pub const DATUM_TRIANGLE: [[f64; 2]; 3] = [[-5.0, 0.0], [5.0, 0.0], [0.0, 8.0]];
/// Cap height and baseline of the letter inside the box (pixels).
pub const DATUM_LETTER: (f64, f64) = (9.0, 11.0);

/// A row of Model definitions › Characteristics: a toleranced feature
/// parameter (live qa4 598, psb 10-21 b).
#[derive(Clone, Debug, PartialEq)]
pub struct Characteristic {
    pub feature: crate::reference::FeatureId,
    pub param: String,
    /// "Distance", "Angle", "Diameter" or "Fillet radius".
    pub kind: &'static str,
    pub dim: noshape_units::Dim,
    /// SI value.
    pub nominal: f64,
    pub tolerance: crate::fits::Tolerance,
    /// The feature is after the rollback bar (or after the feature being
    /// edited): live shows Nominal 0.000 on pink.
    pub rolled_back: bool,
}

impl Characteristic {
    /// The tolerance's type is Default: its value comes from the applied
    /// default tolerances ("Hide default tolerances" hides these rows).
    pub fn is_default(&self) -> bool {
        !matches!(self.tolerance.kind.as_str(), "Symmetrical" | "Deviation" | "Limits" | "Min" | "Max" | "Basic")
    }

    /// Nominal, tolerance, upper and lower limit cells (live psb 10-21 b),
    /// numbers in `unit_factor` (SI per display unit) with the tolerance's
    /// precision or `decimals`; `standard` is the Part Studio's default
    /// tolerances (a Default row takes its value).
    pub fn cells(&self, unit_factor: f64, decimals: usize, standard: Option<&str>) -> [String; 4] {
        let t = &self.tolerance;
        let decimals = t.precision.map(usize::from).unwrap_or(decimals);
        let n = |x: f64| noshape_units::to_fixed(x / unit_factor, decimals as u8);
        let nominal = if self.rolled_back { 0.0 } else { self.nominal };
        // Deviations are typed in the document unit; bare numbers mean that unit.
        let dev = |s: &str| s.trim().parse::<f64>().map(|v| v * unit_factor).unwrap_or_else(|_| noshape_units::evaluate_as(s, self.dim, &noshape_units::UnitPrefs::default(), &noshape_units::EmptyScope).map(|q| q.value).unwrap_or(0.0));
        let (u, l) = (dev(&t.upper), dev(&t.lower));
        let none = || "--".to_string();
        let [tol, upper, lower] = match t.kind.as_str() {
            "Symmetrical" => [format!("±{}", n(u)), n(nominal + u), n(nominal - u)],
            // Limits are typed as deviations too and read the same.
            "Deviation" | "Limits" => [format!("+{}/-{}", n(u), n(l)), n(nominal + u), n(nominal - l)],
            "Min" => ["MIN".into(), none(), n(nominal)],
            "Max" => ["MAX".into(), n(nominal), none()],
            "Basic" => [none(), none(), none()],
            _ => match standard.and_then(|s| default_tolerance(s, self.kind, self.nominal, t.precision)) {
                Some(d) => [format!("±{}", n(d)), n(nominal + d), n(nominal - d)],
                None => [none(), none(), none()],
            },
        };
        [n(nominal), tol, upper, lower]
    }
}

/// The toleranced parameters of `ps`'s features, in feature order (`draft`:
/// a feature open in its dialog, whose parameters replace the stored ones
/// or which is added at the end; the panel fills while the dialog is open).
/// Features from index `upto` on are rolled back.
pub fn characteristics(ps: &crate::partstudio::PartStudio, draft: Option<&crate::partstudio::Feature>, upto: usize) -> Vec<Characteristic> {
    let mut feats: Vec<&crate::partstudio::Feature> = ps.features.iter().map(|f| draft.filter(|d| d.id == f.id).unwrap_or(f)).collect();
    if let Some(d) = draft.filter(|d| !ps.features.iter().any(|f| f.id == d.id)) {
        feats.push(d);
    }
    let mut out = Vec::new();
    for (i, f) in feats.into_iter().enumerate().filter(|(_, f)| !f.suppressed) {
        for (key, v) in &f.params.0 {
            let Some(param) = key.strip_suffix("__tolerance") else { continue };
            let Some(tolerance) = crate::fits::Tolerance::parse(Some(v)) else { continue };
            let Some(e) = f.params.expr(param) else { continue };
            let q = e.cached.or_else(|| noshape_units::evaluate(&e.text, &noshape_units::EmptyScope).ok());
            let Some(q) = q else { continue };
            let kind = if param.to_lowercase().contains("diameter") {
                "Diameter"
            } else if q.dim == noshape_units::Dim::ANGLE {
                "Angle"
            } else if f.type_id == "fillet" && param == "radius" {
                "Fillet radius"
            } else {
                "Distance"
            };
            out.push(Characteristic { feature: f.id, param: param.to_string(), kind, dim: q.dim, nominal: q.value, tolerance, rolled_back: i >= upto });
        }
    }
    out
}

/// The default tolerance (SI: metres, radians) a Default row gets from
/// `standard` (live psb 10-21 c, ISO 2768-c: Distance 30 → ±0.5, Fillet
/// radius 6 → ±1, Angle 90° → ±1°; Precision: ±0.127, angles ±1°).
pub fn default_tolerance(standard: &str, kind: &str, nominal: f64, precision: Option<u8>) -> Option<f64> {
    let deg = std::f64::consts::PI / 180.0;
    if standard == PRECISION_STANDARD {
        return Some(match kind {
            "Angle" => deg,
            _ => match precision {
                Some(2) => 0.25e-3,
                Some(4) => 0.0127e-3,
                _ => 0.127e-3,
            },
        });
    }
    let groups = iso_2768(standard)?;
    let mm = nominal.abs() * 1000.0;
    let pick = |g: &ToleranceGroup, x: f64| -> Option<f64> {
        let row = g.rows.iter().find(|(label, _)| match label.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["Up", "to", v, ..] => v.parse::<f64>().is_ok_and(|v| x <= v),
            ["Over", ..] => true,
            _ => false,
        })?;
        row.1.trim_start_matches('±').trim_end_matches(" mm").trim_end_matches('°').parse::<f64>().ok()
    };
    match kind {
        "Fillet radius" => pick(&groups[3], mm).map(|v| v * 1e-3),
        // ISO 2768 sizes an angle by its shorter leg, which a parameter
        // doesn't have: the 10–50 mm band, the one that gives live's ±1° on
        // ISO 2768-c (to confirm on another grade).
        "Angle" => pick(&groups[1], 50.0).map(|v| v * deg),
        _ => pick(&groups[0], mm).map(|v| v * 1e-3),
    }
}

/// Library "Onshape default tolerances", Type "ISO 2768-1": the classes of
/// the picker's "Default tolerances" list (live qa4 599 applies ISO 2768-c
/// when the picker opens).
pub const DEFAULT_TOLERANCES: [&str; 4] = ["ISO 2768-c", "ISO 2768-f", "ISO 2768-m", "ISO 2768-v"];
pub const DEFAULT_TOLERANCE_CLASS: &str = "ISO 2768-c";
/// The picker's Type list (live psb 10-21 c); the Precision type has no
/// grade.
pub const ISO_2768_1: &str = "ISO 2768-1";
pub const PRECISION_STANDARD: &str = "Precision (.xxx default)";
pub const TOLERANCE_TYPES: [&str; 2] = [ISO_2768_1, PRECISION_STANDARD];
/// The grades in the standard's own order (the tables' columns).
const ISO_2768_GRADES: [&str; 4] = ["ISO 2768-f", "ISO 2768-m", "ISO 2768-c", "ISO 2768-v"];

/// One block of Model definitions › Default tolerances: a title and
/// ("Up to 3 mm", "±0.2 mm") rows; an empty value where the standard gives
/// none.
#[derive(Clone, Debug, PartialEq)]
pub struct ToleranceGroup {
    pub title: &'static str,
    pub rows: Vec<(String, String)>,
}

/// The general tolerances of an ISO 2768-1 class (fine, medium, coarse,
/// very coarse), as the panel lists them (live 599, ISO 2768-c: "Linear
/// tolerances", "Angular tolerances", "Chamfer distances", "Fillet radii").
/// Fillet radii use the standard's "external radii and chamfer heights"
/// row, like the chamfers.
pub fn iso_2768(name: &str) -> Option<Vec<ToleranceGroup>> {
    let class = ISO_2768_GRADES.iter().position(|n| *n == name)?;
    let num = |v: f64| {
        let s = noshape_units::to_fixed(v, 3);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let len = |v: Option<f64>| v.map(|v| format!("±{} mm", num(v))).unwrap_or_default();
    // Permissible deviations (mm) per class f, m, c, v, by nominal size.
    const LINEAR: [(&str, [Option<f64>; 4]); 9] = [
        ("Up to 0.5 mm", [None, None, None, None]),
        ("Up to 3 mm", [Some(0.05), Some(0.1), Some(0.2), None]),
        ("Up to 6 mm", [Some(0.05), Some(0.1), Some(0.3), Some(0.5)]),
        ("Up to 30 mm", [Some(0.1), Some(0.2), Some(0.5), Some(1.0)]),
        ("Up to 120 mm", [Some(0.15), Some(0.3), Some(0.8), Some(1.5)]),
        ("Up to 400 mm", [Some(0.2), Some(0.5), Some(1.2), Some(2.5)]),
        ("Up to 1000 mm", [Some(0.3), Some(0.8), Some(2.0), Some(4.0)]),
        ("Up to 2000 mm", [Some(0.5), Some(1.2), Some(3.0), Some(6.0)]),
        ("Up to 4000 mm", [None, Some(2.0), Some(4.0), Some(8.0)]),
    ];
    // Degrees, by the length of the shorter leg.
    const ANGULAR: [(&str, [f64; 4]); 5] = [
        ("Up to 10 mm", [1.0, 1.0, 1.5, 3.0]),
        ("Up to 50 mm", [0.5, 0.5, 1.0, 2.0]),
        ("Up to 120 mm", [1.0 / 3.0, 1.0 / 3.0, 0.5, 1.0]),
        ("Up to 400 mm", [1.0 / 6.0, 1.0 / 6.0, 0.25, 0.5]),
        ("Over 400 mm", [1.0 / 12.0, 1.0 / 12.0, 1.0 / 6.0, 1.0 / 3.0]),
    ];
    const RADII: [(&str, [Option<f64>; 4]); 4] = [
        ("Up to 0.5 mm", [None, None, None, None]),
        ("Up to 3 mm", [Some(0.2), Some(0.2), Some(0.4), Some(0.4)]),
        ("Up to 6 mm", [Some(0.5), Some(0.5), Some(1.0), Some(1.0)]),
        ("Over 6 mm", [Some(1.0), Some(1.0), Some(2.0), Some(2.0)]),
    ];
    let radii = || RADII.iter().map(|(l, v)| (l.to_string(), len(v[class]))).collect::<Vec<_>>();
    Some(vec![
        ToleranceGroup { title: "Linear tolerances", rows: LINEAR.iter().map(|(l, v)| (l.to_string(), len(v[class]))).collect() },
        ToleranceGroup { title: "Angular tolerances", rows: ANGULAR.iter().map(|(l, v)| (l.to_string(), format!("±{}°", num(v[class])))).collect() },
        ToleranceGroup { title: "Chamfer distances", rows: radii() },
        ToleranceGroup { title: "Fillet radii", rows: radii() },
    ])
}

/// The "Default tolerances" section of `standard` (live psb 10-21 c; the
/// Precision type lists its rows as sub-tables).
pub fn default_tolerance_groups(standard: &str) -> Option<Vec<ToleranceGroup>> {
    if standard != PRECISION_STANDARD {
        return iso_2768(standard);
    }
    let rows = |r: &[(&str, &str)]| r.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    Some(vec![
        ToleranceGroup { title: "Linear tolerances", rows: rows(&[("Workspace precision", "±0.127 mm"), ("X.XX", "±0.25 mm"), ("X.XXX", "±0.127 mm"), ("X.XXXX", "±0.0127 mm")]) },
        ToleranceGroup { title: "Angular tolerances", rows: rows(&[("", "±1°")]) },
        ToleranceGroup { title: "Chamfer angles", rows: rows(&[("", "±5°")]) },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_2768_coarse_matches_the_panel() {
        // Live 599 (ISO 2768-c).
        let g = iso_2768("ISO 2768-c").unwrap();
        assert_eq!(g[0].rows[0], ("Up to 0.5 mm".to_string(), String::new()));
        assert_eq!(g[0].rows[1], ("Up to 3 mm".to_string(), "±0.2 mm".to_string()));
        assert_eq!(g[0].rows[8], ("Up to 4000 mm".to_string(), "±4 mm".to_string()));
        let angles: Vec<&str> = g[1].rows.iter().map(|r| r.1.as_str()).collect();
        assert_eq!(angles, ["±1.5°", "±1°", "±0.5°", "±0.25°", "±0.167°"]);
        let chamfers: Vec<&str> = g[2].rows.iter().map(|r| r.1.as_str()).collect();
        assert_eq!(chamfers, ["", "±0.4 mm", "±1 mm", "±2 mm"]);
        assert_eq!(iso_2768("ISO 2768-f").unwrap()[1].rows[4].1, "±0.083°");
        assert!(iso_2768("ISO 286").is_none());
    }

    fn datum(letter: &str) -> Annotation {
        Annotation { id: 1, kind: AnnotationKind::Datum { letter: letter.into() }, face: Reference::Origin, at: [0.0; 3], normal: [0.0, 0.0, 1.0], up: [0.0, 1.0, 0.0], offset: None }
    }

    #[test]
    fn toleranced_parameters_are_characteristics() {
        use crate::fits::Tolerance;
        let c = Characteristic { feature: crate::reference::FeatureId(1), param: "depth".into(), kind: "Distance", dim: noshape_units::Dim::LENGTH, nominal: 0.020, tolerance: Tolerance { kind: "Symmetrical".into(), upper: "0".into(), ..Tolerance::default() }, rolled_back: false };
        let tol = |kind: &str, upper: &str, lower: &str| Characteristic { tolerance: Tolerance { kind: kind.into(), upper: upper.into(), lower: lower.into(), ..Tolerance::default() }, ..c.clone() };
        // Live psb 10-21 b (mm, 3 decimals).
        assert_eq!(c.cells(0.001, 3, None), ["20.000", "±0.000", "20.000", "20.000"]);
        assert_eq!(tol("Default", "", "").cells(0.001, 3, None), ["20.000", "--", "--", "--"]);
        assert_eq!(tol("Symmetrical", "0.1", "").cells(0.001, 3, None), ["20.000", "±0.100", "20.100", "19.900"]);
        assert_eq!(tol("Deviation", "0.2", "0.1").cells(0.001, 3, None), ["20.000", "+0.200/-0.100", "20.200", "19.900"]);
        assert_eq!(tol("Limits", "0.2", "0.1").cells(0.001, 3, None), ["20.000", "+0.200/-0.100", "20.200", "19.900"]);
        assert_eq!(tol("Min", "", "").cells(0.001, 3, None), ["20.000", "MIN", "--", "20.000"]);
        assert_eq!(tol("Max", "", "").cells(0.001, 3, None), ["20.000", "MAX", "20.000", "--"]);
        assert_eq!(tol("Basic", "", "").cells(0.001, 3, None), ["20.000", "--", "--", "--"]);
        let p1 = Characteristic { tolerance: Tolerance { precision: Some(1), ..tol("Symmetrical", "0.1", "").tolerance }, ..c.clone() };
        assert_eq!(p1.cells(0.001, 3, None), ["20.0", "±0.1", "20.1", "19.9"]);
        // A Default row takes the applied standard's value (live 10-21 c).
        let d30 = Characteristic { nominal: 0.030, ..tol("Default", "", "") };
        assert_eq!(d30.cells(0.001, 3, Some("ISO 2768-c")), ["30.000", "±0.500", "30.500", "29.500"]);
        assert_eq!(d30.cells(0.001, 3, Some(PRECISION_STANDARD))[1], "±0.127");
        let r6 = Characteristic { kind: "Fillet radius", nominal: 0.006, ..tol("Default", "", "") };
        assert_eq!(r6.cells(0.001, 3, Some("ISO 2768-c"))[1], "±1.000");
        let deg = std::f64::consts::PI / 180.0;
        let a90 = Characteristic { kind: "Angle", dim: noshape_units::Dim::ANGLE, nominal: 90.0 * deg, ..tol("Default", "", "") };
        assert_eq!(a90.cells(deg, 3, Some("ISO 2768-c"))[1], "±1.000");
        // Rolled back: Nominal 0.000.
        assert_eq!(Characteristic { rolled_back: true, ..c }.cells(0.001, 3, None)[0], "0.000");
    }

    #[test]
    fn datum_letters_follow_the_alphabet() {
        assert_eq!(next_datum_letter(&[]), "A");
        assert_eq!(next_datum_letter(&[datum("A"), datum("C")]), "B");
        let all: Vec<Annotation> = (0..26).map(|i| datum(&((b'A' + i) as char).to_string())).collect();
        assert_eq!(next_datum_letter(&all), "AA");
        assert_eq!(datum_letters(&[datum("A"), datum("B")]), ["A", "B"]);
    }
}
