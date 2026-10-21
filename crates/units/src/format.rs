//! User-facing formatting of quantities.

use crate::{AngleUnit, Dim, Quantity, UnitPrefs};

/// Formats `v` with at most `decimals` decimal places, trimming trailing
/// zeros and avoiding `-0`.
pub fn format_number(v: f64, decimals: u8) -> String {
    let mut s = to_fixed(v, decimals);
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

/// `v` with `decimals` decimals, rounded as JavaScript's `toFixed` (what
/// Onshape shows): an exact tie rounds away from zero (1.0625 → "1.063"),
/// where Rust's `{:.3}` rounds it to even ("1.062"; qa5 SM15).
pub fn to_fixed(v: f64, decimals: u8) -> String {
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
        e => format!("^{e}"),
    }
}

/// Unit suffix for a dimension in the preferred units, and whether it should
/// be separated from the number by a space.
fn unit_suffix(dim: Dim, prefs: &UnitPrefs) -> (String, bool) {
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
    (parts.join("*"), true)
}

/// Formats a quantity in the preferred units with the preferred precision,
/// e.g. `"25 mm"`, `"1.5 in"`, `"30°"`, `"625 mm²"`, `"0.5 rad"`, `"2.5"`.
pub fn format_quantity(q: Quantity, prefs: &UnitPrefs) -> String {
    let decimals = if q.dim == Dim::ANGLE {
        prefs.angle_precision
    } else {
        prefs.length_precision
    };
    format_quantity_with(q, prefs, decimals)
}

/// Like [`format_quantity`] with an explicit number of decimals.
pub fn format_quantity_with(q: Quantity, prefs: &UnitPrefs, decimals: u8) -> String {
    let v = q.value / prefs.si_factor(q.dim);
    let num = crate::decimal::localize_number(format_number(v, decimals));
    if q.dim.is_unitless() {
        return num;
    }
    let (suffix, space) = unit_suffix(q.dim, prefs);
    if space {
        format!("{num} {suffix}")
    } else {
        format!("{num}{suffix}")
    }
}

/// Formats with enough precision (12 decimals, trimmed) that the text parses
/// back to (almost exactly) the same value. Used to seed expression fields.
pub fn format_quantity_exact(q: Quantity, prefs: &UnitPrefs) -> String {
    format_quantity_with(q, prefs, 12)
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
            "2 mm*deg"
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
mod to_fixed_tests {
    use super::*;

    #[test]
    fn ties_round_away_from_zero_like_onshape() {
        assert_eq!(to_fixed(1.0625, 3), "1.063");
        assert_eq!(to_fixed(-1.0625, 3), "-1.063");
        assert_eq!(to_fixed(0.125, 2), "0.13");
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(9.9995, 3), "9.999"); // not a tie in binary (9.99949…)
        assert_eq!(to_fixed(0.9995, 3), "1.000"); // 0.99950000000000005…, above the tie
        assert_eq!(to_fixed(99.5, 0), "100");
        assert_eq!(to_fixed(1.0624, 3), "1.062");
        assert_eq!(to_fixed(1.25, 4), "1.2500");
        assert_eq!(format_number(1.0625, 3), "1.063");
    }
}
