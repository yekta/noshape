//! User-facing formatting of quantities.

use crate::{AngleUnit, Dim, Quantity, UnitPrefs};

/// Formats `v` with at most `decimals` decimal places, trimming trailing
/// zeros and avoiding `-0`.
pub fn format_number(v: f64, decimals: u8) -> String {
    let mut s = format!("{:.*}", usize::from(decimals), v);
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
