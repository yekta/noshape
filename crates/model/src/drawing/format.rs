//! Dimension value formatting (precision, units, tolerances) and property
//! fields (`{part.name}`, `{sheet.scale}`...) in notes, tables and the title
//! block.

use super::{DimFormat, DimUnits, Drawing, Tolerance};

/// Text of a dimension: the main line plus optional stacked tolerance
/// values (upper over lower) and a frame for basic dimensions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DimText {
    pub main: String,
    pub upper: Option<String>,
    pub lower: Option<String>,
    pub boxed: bool,
}

impl DimText {
    pub fn plain(s: impl Into<String>) -> Self {
        Self { main: s.into(), ..Default::default() }
    }
    /// Single-line rendering (for tests, lists and exports without stacking).
    pub fn flat(&self) -> String {
        let mut s = self.main.clone();
        if let (Some(u), Some(l)) = (&self.upper, &self.lower) {
            if s.is_empty() {
                s = format!("{u}/{l}");
            } else {
                s = format!("{s} {u}/{l}");
            }
        }
        s
    }
}

/// Default decimals per unit (trailing zeros trimmed).
pub fn default_digits(u: DimUnits) -> usize {
    match u {
        DimUnits::Millimeter => 2,
        DimUnits::Centimeter => 3,
        DimUnits::Meter | DimUnits::Foot => 4,
        DimUnits::Inch => 3,
    }
}

/// Format a number with fixed `precision` decimals, or `default_digits`
/// with trailing zeros removed.
pub fn format_number(v: f64, precision: Option<u8>, default_digits: usize) -> String {
    let s = match precision {
        Some(p) => format!("{v:.*}", p as usize),
        None => {
            let s = format!("{v:.default_digits$}");
            if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
        }
    };
    let neg_zero = s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.');
    if neg_zero { s[1..].to_string() } else { s }
}

/// Units a length dimension is shown in.
pub fn length_units(drawing: &Drawing, f: &DimFormat) -> DimUnits {
    f.units.or(drawing.props.units).unwrap_or(if drawing.units_mm { DimUnits::Millimeter } else { DimUnits::Inch })
}

/// Drawing properties applied to a formatted number: trailing zeros
/// (only with a fixed precision), leading zero, decimal separator.
fn finish(drawing: &Drawing, s: String, fixed: bool, tolerance: bool) -> String {
    let p = &drawing.props;
    let (leading, trailing) = if tolerance { (p.tolerance_leading_zeros, p.tolerance_trailing_zeros) } else { (p.leading_zeros, p.trailing_zeros) };
    let mut s = if fixed && !trailing && s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if !leading {
        if let Some(rest) = s.strip_prefix("0.") {
            s = format!(".{rest}");
        } else if let Some(rest) = s.strip_prefix("-0.") {
            s = format!("-.{rest}");
        }
    }
    if p.decimal_comma { s.replace('.', ",") } else { s }
}

/// A number with the dimension's precision, else the drawing's.
fn number(drawing: &Drawing, v: f64, precision: Option<u8>, drawing_precision: Option<u8>, digits: usize, tolerance: bool) -> String {
    let p = precision.or(drawing_precision);
    finish(drawing, format_number(v, p, digits), p.is_some(), tolerance)
}

/// A length (meters) in the dimension's units, without tolerance.
pub fn format_len(drawing: &Drawing, f: &DimFormat, meters: f64) -> String {
    let u = length_units(drawing, f);
    let s = number(drawing, meters / u.meters(), f.precision, drawing.props.precision, default_digits(u), false);
    if f.show_units { format!("{s} {}", u.symbol()) } else { s }
}

fn signed(s: String) -> String {
    if s.starts_with('-') || s.chars().all(|c| c == '0' || c == '.' || c == ',') { s } else { format!("+{s}") }
}

/// Full text of a dimension measuring `value` (meters, or radians when
/// `angular`); `symbol` precedes the value (e.g. "Ø", "R").
pub fn dim_text(drawing: &Drawing, f: &DimFormat, value: f64, angular: bool, symbol: &str) -> DimText {
    let p = &drawing.props;
    let num_with = |v: f64, tol: bool| -> String {
        if angular {
            format!("{}°", number(drawing, v.to_degrees(), f.precision, if tol { p.angular_tolerance_precision } else { p.angular_precision }, 2, tol))
        } else {
            let u = length_units(drawing, f);
            number(drawing, v / u.meters(), f.precision, if tol { p.tolerance_precision.or(p.precision) } else { p.precision }, default_digits(u), tol)
        }
    };
    let num = |v: f64| num_with(v, false);
    let tol_num = |v: f64| num_with(v, true);
    let units = if f.show_units && !angular { format!(" {}", length_units(drawing, f).symbol()) } else { String::new() };
    let value_text = |v: f64| format!("{}{symbol}{}{units}{}", f.prefix, num(v), f.suffix);
    let mut t = DimText { main: value_text(value), ..Default::default() };
    match &f.tolerance {
        Tolerance::None => {}
        Tolerance::Symmetric { value: tol } => t.main = format!("{}{symbol}{} ±{}{units}{}", f.prefix, num(value), tol_num(tol.abs()), f.suffix),
        Tolerance::Deviation { upper, lower } => {
            t.upper = Some(signed(tol_num(*upper)));
            t.lower = Some(signed(tol_num(*lower)));
        }
        Tolerance::Limits { upper, lower } => {
            t.main = String::new();
            t.upper = Some(value_text(value + upper));
            t.lower = Some(value_text(value + lower));
        }
        Tolerance::Basic => t.boxed = true,
        Tolerance::Min => t.main.push_str(" MIN"),
        Tolerance::Max => t.main.push_str(" MAX"),
        Tolerance::Fit { hole, shaft } => {
            let fit = match (hole.trim(), shaft.trim()) {
                ("", "") => String::new(),
                (h, "") => h.to_string(),
                ("", s) => s.to_string(),
                (h, s) => format!("{h}/{s}"),
            };
            if !fit.is_empty() {
                t.main = format!("{} {fit}", t.main);
            }
        }
    }
    t
}

// ------------------------------------------------------------ property fields

/// Property fields available in notes, tables and title block values.
pub const FIELDS: &[(&str, &str)] = &[
    ("part.name", "Part name"),
    ("part.number", "Part number"),
    ("part.material", "Material"),
    ("part.description", "Description"),
    ("model.name", "Source tab name"),
    ("doc.name", "Document name"),
    ("drawing.name", "Drawing tab name"),
    ("sheet.name", "Sheet name"),
    ("sheet.number", "Sheet number"),
    ("sheet.count", "Number of sheets"),
    ("sheet.scale", "Sheet scale"),
    ("sheet.size", "Sheet size"),
    ("date", "Today's date"),
    ("units", "Drawing units"),
];

/// Replace `{key}` occurrences using `lookup`; unknown keys are kept as
/// typed, known but empty values become "----" (like Onshape).
pub fn expand_fields(text: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    let codes = text_codes(text);
    let text = codes.as_str();
    if !text.contains('{') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('}') {
            Some(j) if !after[..j].contains('{') => {
                let key = after[..j].trim();
                match lookup(key) {
                    Some(v) if v.is_empty() => out.push_str("----"),
                    Some(v) => out.push_str(&v),
                    None => {
                        out.push('{');
                        out.push_str(&after[..j]);
                        out.push('}');
                    }
                }
                rest = &after[j + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Onshape text codes in notes and dimension text: `%%d` °, `%%c` ⌀,
/// `%%p` ±, and `\U+00AE`-style unicode characters.
pub fn text_codes(text: &str) -> String {
    if !text.contains("%%") && !text.contains("\\U+") && !text.contains("\\u+") {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let lower = rest.get(..3).map(str::to_ascii_lowercase);
        match lower.as_deref() {
            Some("%%d") => {
                out.push('°');
                rest = &rest[3..];
                continue;
            }
            Some("%%c") => {
                out.push('⌀');
                rest = &rest[3..];
                continue;
            }
            Some("%%p") => {
                out.push('±');
                rest = &rest[3..];
                continue;
            }
            Some("\\u+") => {
                let hex: String = rest[3..].chars().take_while(|c| c.is_ascii_hexdigit()).take(4).collect();
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).filter(|_| hex.len() >= 4) {
                    out.push(ch);
                    rest = &rest[3 + hex.len()..];
                    continue;
                }
            }
            _ => {}
        }
        let ch = rest.chars().next().expect("not empty");
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out
}

/// Today's date (UTC) as YYYY-MM-DD.
pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (y, m, d) = civil_from_days(secs.div_euclid(86400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since 1970-01-01 -> (year, month, day) (Howard Hinnant's algorithm).
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {

    #[test]
    fn drawing_properties_shape_dimension_text() {
        let mut d = Drawing::default();
        let f = DimFormat::default();
        // Defaults: millimeters, trailing zeros trimmed.
        assert_eq!(format_len(&d, &f, 0.0125), "12.5");
        // Inches, precision 0.123 with trailing zeros, comma separator, no leading zero.
        d.props.units = Some(DimUnits::Inch);
        d.props.precision = Some(3);
        assert_eq!(format_len(&d, &f, 0.0254 * 0.5), "0.500");
        d.props.leading_zeros = false;
        assert_eq!(format_len(&d, &f, 0.0254 * 0.5), ".500");
        d.props.trailing_zeros = false;
        assert_eq!(format_len(&d, &f, 0.0254 * 0.5), ".5");
        d.props.decimal_comma = true;
        assert_eq!(format_len(&d, &f, 0.0254 * 1.25), "1,25");
        // Angles use the angular precision.
        d.props.angular_precision = Some(1);
        assert_eq!(dim_text(&d, &f, std::f64::consts::FRAC_PI_4, true, "").main, "45°");
        d.props.trailing_zeros = true;
        assert_eq!(dim_text(&d, &f, std::f64::consts::FRAC_PI_4, true, "").main, "45,0°");
    }

    #[test]
    fn note_text_codes() {
        assert_eq!(text_codes("90%%d %%C10 %%p0.1"), "90° ⌀10 ±0.1");
        assert_eq!(text_codes("A\\U+00AEB \\u+2300"), "A®B ⌀");
        assert_eq!(text_codes("100%"), "100%");
        assert_eq!(expand_fields("%%c{x}", &|k| (k == "x").then(|| "5".into())), "⌀5");
    }

    use super::*;

    #[test]
    fn numbers_and_tolerances() {
        let d = Drawing::default();
        let f = DimFormat::default();
        assert_eq!(dim_text(&d, &f, 0.0125, false, "").main, "12.5");
        let f = DimFormat { precision: Some(3), ..Default::default() };
        assert_eq!(dim_text(&d, &f, 0.0125, false, "Ø").main, "Ø12.500");
        let f = DimFormat { tolerance: Tolerance::Symmetric { value: 0.00005 }, precision: Some(2), ..Default::default() };
        assert_eq!(dim_text(&d, &f, 0.01, false, "").main, "10.00 ±0.05");
        let f = DimFormat { tolerance: Tolerance::Deviation { upper: 0.0001, lower: -0.00005 }, ..Default::default() };
        let t = dim_text(&d, &f, 0.01, false, "");
        assert_eq!((t.main.as_str(), t.upper.as_deref(), t.lower.as_deref()), ("10", Some("+0.1"), Some("-0.05")));
        let f = DimFormat { tolerance: Tolerance::Limits { upper: 0.0001, lower: -0.0001 }, ..Default::default() };
        let t = dim_text(&d, &f, 0.01, false, "");
        assert_eq!((t.upper.as_deref(), t.lower.as_deref()), (Some("10.1"), Some("9.9")));
        let f = DimFormat { tolerance: Tolerance::Fit { hole: "H7".into(), shaft: "g6".into() }, ..Default::default() };
        assert_eq!(dim_text(&d, &f, 0.012, false, "Ø").main, "Ø12 H7/g6");
        let f = DimFormat { units: Some(DimUnits::Inch), show_units: true, ..Default::default() };
        assert_eq!(dim_text(&d, &f, 0.0254, false, "").main, "1 in");
        assert_eq!(dim_text(&d, &DimFormat::default(), std::f64::consts::FRAC_PI_4, true, "").main, "45°");
        assert!(dim_text(&d, &DimFormat { tolerance: Tolerance::Basic, ..Default::default() }, 0.01, false, "").boxed);
    }

    #[test]
    fn fields() {
        let look = |k: &str| match k {
            "part.name" => Some("Plate".to_string()),
            "part.number" => Some(String::new()),
            _ => None,
        };
        assert_eq!(expand_fields("{part.name} #{part.number} {nope} {", &look), "Plate #---- {nope} {");
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20744), (2026, 10, 18));
    }
}
