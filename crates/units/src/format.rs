//! User-facing formatting of quantities.

use crate::{AngleUnit, Dim, Quantity, UnitPrefs};

/// Formats `v` with at most `decimals` decimal places as live's value formatter (`formatValue`):
/// `Math.round(v·10^d)/10^d`, the float product rounded half up (12.3455 → "12.346", −2.5 at 0 → "-2"), trailing zeros
/// trimmed, no `-0`.
pub fn format_number(v: f64, decimals: u8) -> String {
    if !v.is_finite() {
        return format!("{v}");
    }
    let mut s = format!("{:.*}", usize::from(decimals), round_like_live(v, decimals));
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

/// `Math.round(v·10^d)/10^d` (JavaScript's `Math.round`: half up, toward +∞).
pub fn round_like_live(v: f64, decimals: u8) -> f64 {
    let f = 10f64.powi(i32::from(decimals));
    (v * f + 0.5).floor() / f
}

/// A number as live's value formatter writes it: [`format_number`], or `v.toExponential(d)` when |v| ≥ 1e6, |v| ≤ 1e-5
/// or it rounds to 0 ("4.643e+7", "4.000e-4").
pub fn format_value(v: f64, decimals: u8) -> String {
    if !v.is_finite() || v == 0.0 || !(v.abs() >= 1e6 || v.abs() <= 1e-5 || round_like_live(v, decimals) == 0.0) {
        return format_number(v, decimals);
    }
    to_exponential(v, decimals)
}

/// `v.toExponential(d)` as JavaScript writes it: "4.643e+7", "-4.000e-4".
fn to_exponential(v: f64, decimals: u8) -> String {
    let s = format!("{:.*e}", usize::from(decimals), v);
    match s.split_once('e') {
        Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
        _ => s,
    }
}

/// `v` with `decimals` decimals as live's readouts write it: `(v ± 1e-10).toFixed(d)` (the nudge makes a decimal tie
/// round away from zero: 12.3455 → "12.346", −8.0345 → "-8.035"); JavaScript's `toFixed` rounds an exact tie away from
/// zero too (1.0625 → "1.063"), where Rust's `{:.3}` rounds it to even ("1.062"; qa5 SM15). Trailing zeros stay.
pub fn to_fixed(v: f64, decimals: u8) -> String {
    let v = if v == 0.0 || !v.is_finite() { v } else { v + v.signum() * 1e-10 };
    let d = usize::from(decimals);
    let s = format!("{:.*}", d, v);
    if !v.is_finite() {
        return s;
    }
    // A tie has a finite expansion ending in 5 one digit past `d`; Rust
    // prints exact digits, so 24 more show it (…5000…).
    let long = format!("{:.*}", d + 25, v.abs());
    let cut = long.find('.').map_or(long.len(), |p| if d == 0 { p } else { p + 1 + d });
    let tail = long[cut..].trim_start_matches('.');
    if !(tail.starts_with('5') && tail[1..].bytes().all(|b| b == b'0')) {
        return s;
    }
    // Away from zero: the truncated digits plus one in the last place.
    let mut digits: Vec<u8> = long[..cut].trim_end_matches('.').bytes().collect();
    let mut i = digits.len();
    loop {
        if i == 0 {
            digits.insert(0, b'1');
            break;
        }
        i -= 1;
        match digits[i] {
            b'.' => continue,
            b'9' => digits[i] = b'0',
            c => {
                digits[i] = c + 1;
                break;
            }
        }
    }
    let out = String::from_utf8(digits).unwrap_or(s);
    if v < 0.0 { format!("-{out}") } else { out }
}

fn superscript(e: i8) -> String {
    match e {
        1 => String::new(),
        2 => "²".into(),
        3 => "³".into(),
        4 => "⁴".into(),
        e => format!("^{e}"),
    }
}

/// Unit suffix for a dimension in the preferred units, and whether it should
/// be separated from the number by a space. Shown, a compound unit is live's: its units sorted and joined by a space
/// ("deg mm"); `parseable` keeps the expression form ("mm*deg").
fn unit_suffix(dim: Dim, prefs: &UnitPrefs, parseable: bool) -> (String, bool) {
    if dim == Dim::ANGLE && prefs.angle == AngleUnit::Degree {
        return ("°".into(), false);
    }
    let mut parts = Vec::new();
    if dim.length != 0 {
        parts.push(format!(
            "{}{}",
            prefs.length.symbol(),
            superscript(dim.length)
        ));
    }
    if dim.angle != 0 {
        parts.push(format!(
            "{}{}",
            prefs.angle.symbol(),
            superscript(dim.angle)
        ));
    }
    if parseable {
        return (parts.join("*"), true);
    }
    parts.sort();
    (parts.join(" "), true)
}

/// The smallest magnitude shown as a number (SI): FeatureScript's zero length (1e-8 m) and zero angle (1e-11 rad) to
/// the dimension's powers; anything smaller reads "0".
fn zero_tolerance(dim: Dim) -> f64 {
    1e-8f64.powi(i32::from(dim.length).abs()) * 1e-11f64.powi(i32::from(dim.angle).abs())
}

/// Formats a quantity in the preferred units with the preferred precision,
/// e.g. `"25 mm"`, `"1.5 in"`, `"30°"`, `"625 mm²"`, `"0.5 rad"`, `"2.5"`.
/// Live's value formatter writes a value in exponent form when |v| ≥ 1e6, |v| ≤ 1e-5 or it rounds to 0 ("2.000e+6
/// mm", "4.000e-4 mm"); below the zero tolerance it is "0".
pub fn format_quantity(q: Quantity, prefs: &UnitPrefs) -> String {
    // A unit of lengths and angles takes the larger of their precisions (live numfmt-1118: "deg mm" at 2 / 4 shows 4).
    let decimals = match (q.dim.length != 0, q.dim.angle != 0) {
        (true, true) => prefs.length_precision.max(prefs.angle_precision),
        (false, true) => prefs.angle_precision,
        _ => prefs.length_precision,
    };
    let v = q.value / prefs.si_factor(q.dim);
    if !q.dim.is_unitless() && q.value.abs() < zero_tolerance(q.dim) {
        return format_quantity_with(Quantity::new(0.0, q.dim), prefs, decimals);
    }
    with_unit(crate::decimal::localize_number(format_value(v, decimals)), q.dim, prefs, false)
}

/// Like [`format_quantity`] with an explicit number of decimals (no exponent form).
pub fn format_quantity_with(q: Quantity, prefs: &UnitPrefs, decimals: u8) -> String {
    let v = q.value / prefs.si_factor(q.dim);
    with_unit(crate::decimal::localize_number(format_number(v, decimals)), q.dim, prefs, false)
}

fn with_unit(num: String, dim: Dim, prefs: &UnitPrefs, parseable: bool) -> String {
    if dim.is_unitless() {
        return num;
    }
    let (suffix, space) = unit_suffix(dim, prefs, parseable);
    if space {
        format!("{num} {suffix}")
    } else {
        format!("{num}{suffix}")
    }
}

/// Formats with enough precision (12 decimals, trimmed) that the text parses
/// back to (almost exactly) the same value. Used to seed expression fields.
pub fn format_quantity_exact(q: Quantity, prefs: &UnitPrefs) -> String {
    let v = q.value / prefs.si_factor(q.dim);
    with_unit(crate::decimal::localize_number(format_number(v, 12)), q.dim, prefs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LengthUnit, evaluate};

    #[test]
    fn numbers() {
        assert_eq!(format_number(25.0, 3), "25");
        assert_eq!(format_number(1.23456, 3), "1.235");
        assert_eq!(format_number(1.5, 0), "2");
        assert_eq!(format_number(-0.0001, 3), "0");
        assert_eq!(format_number(-1.25, 2), "-1.25");
        assert_eq!(format_number(100.0, 0), "100");
    }

    #[test]
    fn quantities() {
        let mm = UnitPrefs::default();
        let inch = UnitPrefs::imperial();
        assert_eq!(format_quantity(Quantity::mm(25.0), &mm), "25 mm");
        assert_eq!(format_quantity(Quantity::mm(38.1), &inch), "1.5 in");
        assert_eq!(format_quantity(Quantity::degrees(30.0), &mm), "30°");
        let rad = UnitPrefs {
            angle: AngleUnit::Radian,
            ..mm
        };
        assert_eq!(format_quantity(Quantity::radians(0.5), &rad), "0.5 rad");
        assert_eq!(format_quantity(Quantity::unitless(2.5), &mm), "2.5");
        assert_eq!(
            format_quantity(Quantity::mm(5.0) * Quantity::mm(5.0), &mm),
            "25 mm²"
        );
        assert_eq!(
            format_quantity(
                Quantity::mm(2.0) * Quantity::mm(2.0) * Quantity::mm(2.0),
                &mm
            ),
            "8 mm³"
        );
        assert_eq!(
            format_quantity(Quantity::mm(1.0) * Quantity::degrees(2.0), &mm),
            "2 deg mm"
        );
        assert_eq!(
            format_quantity(Quantity::unitless(1.0) / Quantity::mm(4.0), &mm),
            "0.25 mm^-1"
        );
        let cm = UnitPrefs {
            length: LengthUnit::Centimeter,
            length_precision: 1,
            ..mm
        };
        assert_eq!(format_quantity(Quantity::mm(12.34), &cm), "1.2 cm");
    }

    #[test]
    fn formatted_text_parses_back() {
        let prefs = UnitPrefs::default();
        let s = crate::EmptyScope;
        for q in [
            Quantity::mm(25.0),
            Quantity::degrees(30.0),
            Quantity::mm(5.0) * Quantity::mm(5.0),
            Quantity::mm(1.0) * Quantity::degrees(2.0),
            Quantity::unitless(1.0) / Quantity::mm(4.0),
            Quantity::inches(1.0 / 3.0),
        ] {
            let text = format_quantity_exact(q, &prefs);
            let back = evaluate(&text, &s).unwrap();
            assert_eq!(back.dim, q.dim, "{text}");
            assert!(
                (back.value - q.value).abs() <= 1e-12 * q.value.abs(),
                "{text}"
            );
        }
    }
}

#[cfg(test)]
mod live_format_tests {
    use super::*;

    /// Live numfmt-1118: a compound unit shows the largest precision of its units; a pure length or angle its own.
    #[test]
    fn a_compound_unit_takes_its_units_largest_precision() {
        let prefs = UnitPrefs { length_precision: 2, angle_precision: 4, ..UnitPrefs::default() };
        let q = Quantity::mm(1.0) * Quantity::degrees(2.123456);
        assert_eq!(format_quantity(q, &prefs), "2.1235 deg mm");
        assert_eq!(format_quantity(Quantity::mm(1.123456), &prefs), "1.12 mm");
        assert_eq!(format_quantity(Quantity::degrees(1.123456), &prefs), "1.1235°");
        let flipped = UnitPrefs { length_precision: 4, angle_precision: 2, ..UnitPrefs::default() };
        assert_eq!(format_quantity(q, &flipped), "2.1235 deg mm");
    }

    /// Live's formatValue (behaviours "number and unit display rules", 11-18): Math.round of the float product.
    #[test]
    fn values_round_as_live() {
        assert_eq!(format_number(12.3455, 3), "12.346");
        assert_eq!(format_number(1.2345, 3), "1.235");
        assert_eq!(format_number(2.675, 2), "2.68");
        assert_eq!(format_number(-2.5, 0), "-2");
        assert_eq!(format_number(-1.0625, 3), "-1.062");
        assert_eq!(format_number(25.0, 3), "25");
    }

    /// Readouts: (v ± 1e-10).toFixed(p), trailing zeros kept.
    #[test]
    fn readouts_round_ties_away_as_live() {
        assert_eq!(to_fixed(12.3455, 3), "12.346");
        assert_eq!(to_fixed(-8.0345, 3), "-8.035");
        assert_eq!(to_fixed(90.0, 3), "90.000");
    }

    /// Exponent form at |v| ≥ 1e6, ≤ 1e-5 or rounding to 0; compound units sorted and spaced, ⁴.
    #[test]
    fn exponents_and_compound_units_as_live() {
        let mm = UnitPrefs::default();
        assert_eq!(format_quantity(Quantity::mm(2_000_000.0), &mm), "2.000e+6 mm");
        assert_eq!(format_quantity(Quantity::mm(0.0004), &mm), "4.000e-4 mm");
        assert_eq!(format_quantity(Quantity::mm(999_999.0), &mm), "999999 mm");
        assert_eq!(format_quantity(Quantity::mm(0.0), &mm), "0 mm");
        assert_eq!(format_quantity(Quantity::mm(1e-9), &mm), "0 mm");
        let cube = Quantity::mm(359.0) * Quantity::mm(359.0) * Quantity::mm(360.25);
        assert_eq!(format_quantity(cube, &mm), "4.643e+7 mm³");
        let i = Quantity::mm(2.0) * Quantity::mm(2.0) * Quantity::mm(2.0) * Quantity::mm(2.0);
        assert_eq!(format_quantity(i, &mm), "16 mm⁴");
        assert_eq!(format_quantity(Quantity::mm(1.0) * Quantity::degrees(2.0), &mm), "2 deg mm");
        assert_eq!(format_quantity_exact(Quantity::mm(1.0) * Quantity::degrees(2.0), &mm), "2 mm*deg");
    }
}

#[cfg(test)]
mod to_fixed_tests {
    use super::*;

    #[test]
    fn ties_round_away_from_zero_like_onshape() {
        assert_eq!(to_fixed(1.0625, 3), "1.063");
        assert_eq!(to_fixed(-1.0625, 3), "-1.063");
        assert_eq!(to_fixed(0.125, 2), "0.13");
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(9.9995, 3), "10.000"); // 9.99949… in binary, nudged past the tie (live's readouts)
        assert_eq!(to_fixed(0.9995, 3), "1.000"); // 0.99950000000000005…, above the tie
        assert_eq!(to_fixed(99.5, 0), "100");
        assert_eq!(to_fixed(1.0624, 3), "1.062");
        assert_eq!(to_fixed(1.25, 4), "1.2500");
        assert_eq!(format_number(1.0625, 3), "1.063");
    }
}
