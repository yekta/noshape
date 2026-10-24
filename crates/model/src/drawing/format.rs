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
    /// Palette "Above text" / "Below text".
    pub above: Option<String>,
    pub below: Option<String>,
    /// Inspection dimension: an oval frame.
    pub oval: bool,
    /// The dual value, bracketed ("[2.36 in]"), and whether it goes below
    /// the primary (Dual › Dimension location Bottom) rather than above.
    pub dual: Option<String>,
    pub dual_below: bool,
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
        DimUnits::Meter | DimUnits::Foot | DimUnits::Yard => 4,
        DimUnits::Inch | DimUnits::InchFraction | DimUnits::FeetInches => 3,
    }
}

/// Inches as a whole number and a reduced fraction to 1/64: "1 3/8".
fn inch_fraction(inches: f64) -> String {
    let sign = if inches < 0.0 { "-" } else { "" };
    let x = inches.abs();
    let (mut whole, mut n) = (x.trunc(), ((x - x.trunc()) * 64.0).round() as u32);
    if n == 64 {
        whole += 1.0;
        n = 0;
    }
    let mut d = 64;
    while n > 0 && n % 2 == 0 {
        n /= 2;
        d /= 2;
    }
    match (whole as u64, n) {
        (w, 0) => format!("{sign}{w}"),
        (0, n) => format!("{sign}{n}/{d}"),
        (w, n) => format!("{sign}{w} {n}/{d}"),
    }
}

/// A length (meters) as a number in `u` (fractions and feet-and-inches
/// spelled out).
fn unit_number(drawing: &Drawing, meters: f64, u: DimUnits, precision: Option<u8>, drawing_precision: Option<u8>, tolerance: bool) -> String {
    match u {
        DimUnits::InchFraction => inch_fraction(meters / 0.0254),
        DimUnits::FeetInches => {
            let inches = meters / 0.0254;
            let ft = (inches.abs() / 12.0).trunc() * inches.signum();
            let rest = (inches - ft * 12.0).abs();
            format!("{}' {}\"", ft as i64, number(drawing, rest, precision, drawing_precision, 3, tolerance))
        }
        _ => number(drawing, meters / u.meters(), precision, drawing_precision, default_digits(u), tolerance),
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

/// An angle in degrees: Onshape's leading / trailing zero settings are for
/// lengths only, so a set precision keeps its digits ("90.0°", DSM2).
fn angle_number(drawing: &Drawing, deg: f64, precision: Option<u8>) -> String {
    let s = format_number(deg, precision, 2);
    if drawing.props.decimal_comma { s.replace('.', ",") } else { s }
}

/// A number with the dimension's precision, else the drawing's.
fn number(drawing: &Drawing, v: f64, precision: Option<u8>, drawing_precision: Option<u8>, digits: usize, tolerance: bool) -> String {
    let p = precision.or(drawing_precision);
    finish(drawing, format_number(v, p, digits), p.is_some(), tolerance)
}

/// A length (meters) in the dimension's units, without tolerance.
pub fn format_len(drawing: &Drawing, f: &DimFormat, meters: f64) -> String {
    let u = length_units(drawing, f);
    let s = unit_number(drawing, meters, u, f.precision, drawing.props.precision, false);
    if f.show_units && !u.symbol().is_empty() { format!("{s} {}", u.symbol()) } else { s }
}

fn signed(s: String) -> String {
    if s.starts_with('-') || s.chars().all(|c| c == '0' || c == '.' || c == ',') { s } else { format!("+{s}") }
}

/// A fit deviation (meters) with its sign, in the dimension's units and
/// the drawing's tolerance precision: "+0.015", "−0.000" (a zero lower
/// deviation keeps the minus sign; live, dialogs/hole-35).
pub fn format_deviation(drawing: &Drawing, f: &DimFormat, meters: f64, lower: bool) -> String {
    let u = length_units(drawing, f);
    let p = &drawing.props;
    let s = number(drawing, meters / u.meters(), f.precision, p.tolerance_precision.or(p.precision), default_digits(u), true);
    match s.strip_prefix('-') {
        Some(abs) => format!("−{abs}"),
        None if lower && s.chars().all(|c| c == '0' || c == '.' || c == ',') => format!("−{s}"),
        None => format!("+{s}"),
    }
}

/// Markers of stacked text (upper over lower) inside a one-line label such
/// as a hole callout line: `STACK_OPEN upper STACK_SEP lower STACK_CLOSE`.
pub const STACK_OPEN: char = '\u{E000}';
pub const STACK_SEP: char = '\u{E001}';
pub const STACK_CLOSE: char = '\u{E002}';

/// `upper` stacked over `lower` (see [`STACK_OPEN`]).
pub fn stacked(upper: &str, lower: &str) -> String {
    format!("{STACK_OPEN}{upper}{STACK_SEP}{lower}{STACK_CLOSE}")
}

/// A label split around its stacked part: (before, (upper, lower), after).
pub fn split_stacked(s: &str) -> (&str, Option<(&str, &str)>, &str) {
    let parts = s.split_once(STACK_OPEN).and_then(|(a, rest)| {
        let (pair, b) = rest.split_once(STACK_CLOSE)?;
        let (u, l) = pair.split_once(STACK_SEP)?;
        Some((a, (u, l), b))
    });
    match parts {
        Some((a, p, b)) => (a, Some(p), b),
        None => (s, None, ""),
    }
}

/// One-line text of a label with a stacked part ("⌀6.35 H7 +0.015/−0 THRU").
pub fn flatten_stacked(s: &str) -> String {
    match split_stacked(s) {
        (a, Some((u, l)), b) => format!("{a}{u}/{l}{b}"),
        (a, None, _) => a.to_string(),
    }
}

/// Full text of a dimension measuring `value` (meters, or radians when
/// `angular`); `symbol` precedes the value (e.g. "Ø", "R").
pub fn dim_text(drawing: &Drawing, f: &DimFormat, value: f64, angular: bool, symbol: &str) -> DimText {
    let p = &drawing.props;
    let num_with = |v: f64, tol: bool| -> String {
        if angular {
            // Unset: Onshape's template defaults (0.1°, tolerances 0°).
            format!("{}°", angle_number(drawing, v.to_degrees(), f.precision.or(if tol { p.angular_tolerance_precision.or(Some(0)) } else { p.angular_precision.or(Some(1)) })))
        } else {
            let u = length_units(drawing, f);
            unit_number(drawing, v, u, f.precision, if tol { p.tolerance_precision.or(p.precision) } else { p.precision }, tol)
        }
    };
    let num = |v: f64| num_with(v, false);
    let tol_num = |v: f64| num_with(v, true);
    let units = if f.show_units && !angular { format!(" {}", length_units(drawing, f).symbol()) } else { String::new() };
    let value_text = |v: f64| format!("{}{symbol}{}{units}{}", f.prefix, num(v), f.suffix);
    let mut t = DimText { main: value_text(value), ..Default::default() };
    match &f.tolerance {
        Tolerance::None => {}
        // Live "60±0.1": no space before ±.
        Tolerance::Symmetric { value: tol } => t.main = format!("{}{symbol}{}±{}{units}{}", f.prefix, num(value), tol_num(tol.abs()), f.suffix),
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
            let fo = &f.fit;
            let h = if fo.hide_hole { "" } else { hole.trim() };
            let sh = if fo.hide_shaft { "" } else { shaft.trim() };
            // Deviations of the hole class (else the shaft's) for "Fit with
            // tolerance" and "Fit (Tolerance only)", at the fit precision.
            let class = if hole.trim().is_empty() { shaft.trim() } else { hole.trim() };
            let dev = (fo.mode != super::FitMode::Fit && !angular).then(|| crate::fits::deviations(value, class)).flatten();
            let fp = DimFormat { precision: fo.precision.or(p.fit_tolerance_precision).or(f.precision), ..f.clone() };
            let devs = dev.map(|(up, lo)| (format_deviation(drawing, &fp, up, false), format_deviation(drawing, &fp, lo, true)));
            let classes = match (h, sh) {
                ("", "") => String::new(),
                (h, "") => h.to_string(),
                ("", s) => s.to_string(),
                (h, s) => format!("{h}/{s}"),
            };
            match (fo.mode, devs) {
                (super::FitMode::ToleranceOnly, Some((up, lo))) => {
                    t.upper = Some(up);
                    t.lower = Some(lo);
                }
                (super::FitMode::WithTolerance, Some((up, lo))) => {
                    if !classes.is_empty() {
                        t.main = format!("{} {classes}", t.main);
                    }
                    t.upper = Some(up);
                    t.lower = Some(lo);
                }
                // Stacked classes (hole over shaft) unless diagonal.
                _ if !h.is_empty() && !sh.is_empty() && fo.stacking != super::FitStacking::Diagonal => {
                    t.upper = Some(h.to_string());
                    t.lower = Some(sh.to_string());
                }
                _ if !classes.is_empty() => t.main = format!("{} {classes}", t.main),
                _ => {}
            }
        }
    }
    // Palette: parentheses, above / below text, the inspection oval.
    if f.reference {
        if t.main.is_empty() {
            t.upper = t.upper.map(|u| format!("({u})"));
            t.lower = t.lower.map(|l| format!("({l})"));
        } else if t.upper.is_some() {
            t.main = format!("({}", t.main);
            t.lower = t.lower.map(|l| format!("{l})"));
        } else {
            t.main = format!("({})", t.main);
        }
    }
    t.above = (!f.above.is_empty()).then(|| f.above.clone());
    t.below = (!f.below.is_empty()).then(|| f.below.clone());
    t.oval = f.inspection_frame;
    t.dual = dual_text(drawing, f, value, angular);
    t.dual_below = p.dual.dim_location == super::DualLocation::Bottom;
    t
}

/// The bracketed dual value of a dimension (live asb 10-24 dual
/// dimensions): "[2.36 in]", "[2.36±0.004 in]" (the dual tolerance
/// precision, zeros kept), "[2.36]" without Show dual unit. No Ø / R,
/// prefix, suffix or fit class; angular dimensions have none. The
/// palette's Single unit / Dual unit override the drawing's setting.
fn dual_text(drawing: &Drawing, f: &DimFormat, value: f64, angular: bool) -> Option<String> {
    let p = &drawing.props;
    if angular || !f.dual.unwrap_or(p.dual.show) {
        return None;
    }
    let du = dual_units(drawing, f);
    let tol = match &f.tolerance {
        Tolerance::Symmetric { value: t } => format!("±{:.*}", p.dual.tolerance_precision.unwrap_or(3) as usize, t.abs() / du.meters()),
        _ => String::new(),
    };
    Some(format!("[{}{tol}{}]", dual_number(drawing, du, value), dual_unit(drawing, du)))
}

/// The dual units of a length shown with `f` (the other system).
fn dual_units(drawing: &Drawing, f: &DimFormat) -> super::DimUnits {
    drawing.props.dual.units_for(!length_units(drawing, f).imperial())
}

/// A length in the dual units at the dual precision (unset: live's 0.123).
fn dual_number(drawing: &Drawing, du: super::DimUnits, meters: f64) -> String {
    unit_number(drawing, meters, du, Some(drawing.props.dual.precision.unwrap_or(3)), None, false)
}

/// " in" / " mm" with Show dual unit, else nothing.
fn dual_unit(drawing: &Drawing, du: super::DimUnits) -> String {
    if drawing.props.dual.show_unit && !du.symbol().is_empty() { format!(" {}", du.symbol()) } else { String::new() }
}

/// A hole callout length's dual, with its unit ("6.35 mm"), when the
/// callout shows duals (Drawing properties › Dual, or its palette's Dual
/// unit / Single unit).
pub fn dual_len(drawing: &Drawing, f: &DimFormat, meters: f64) -> Option<String> {
    if !f.dual.unwrap_or(drawing.props.dual.show) {
        return None;
    }
    let du = dual_units(drawing, f);
    Some(format!("{}{}", dual_number(drawing, du, meters), dual_unit(drawing, du)))
}

// ------------------------------------------------------------ property fields

/// Note panel › Insert drawing property, in Onshape's order (live
/// behaviours.md "Note panel popups and values").
pub const DRAWING_FIELDS: &[(&str, &str)] = &[
    ("drawing.drawn_by", "Drawing drawn by"),
    ("drawing.date_drawn", "Drawing date drawn"),
    ("drawing.changed_by", "Drawing last changed by"),
    ("drawing.changed_date", "Drawing last changed date"),
    ("drawing.name", "Drawing name"),
    ("drawing.description", "Drawing description"),
    ("drawing.number", "Drawing part number"),
    ("drawing.revision", "Drawing revision"),
    ("drawing.state", "Drawing state"),
    ("drawing.title1", "Drawing title 1"),
    ("drawing.title2", "Drawing title 2"),
    ("drawing.title3", "Drawing title 3"),
    ("sheet.number", "Sheet number"),
    ("sheet.scale", "Sheet scale"),
    ("sheet.size", "Sheet size"),
    ("sheet.count", "Total sheets"),
    ("release.approver", "Drawing approved by"),
    ("release.date", "Drawing approved date"),
    ("drawing.category", "Category"),
    ("drawing.not_revision_managed", "Not revision managed"),
    ("drawing.revision_description", "Revision description"),
    ("drawing.inspection_count", "Inspection count"),
];

/// Note panel › Insert sheet reference property (the sheet's part), in
/// Onshape's order.
pub const SHEET_REF_FIELDS: &[(&str, &str)] = &[
    ("part.name", "Name"),
    ("part.description", "Description"),
    ("part.number", "Part number"),
    ("part.revision", "Revision"),
    ("part.state", "State"),
    ("part.vendor", "Vendor"),
    ("part.project", "Project"),
    ("part.product_line", "Product line"),
    ("part.title1", "Title 1"),
    ("part.title2", "Title 2"),
    ("part.title3", "Title 3"),
    ("part.material", "Material"),
    ("part.category", "Category"),
    ("part.not_revision_managed", "Not revision managed"),
    ("part.exclude_from_bom", "Exclude from all BOMs"),
    ("part.unit_of_measure", "Unit of measure"),
    ("part.mass", "Mass"),
    ("part.subassembly_bom", "Subassembly BOM behavior"),
    ("part.tessellation", "Tessellation quality"),
];

/// Other property keys notes and title blocks use.
pub const OTHER_FIELDS: &[(&str, &str)] = &[("model.name", "Source tab name"), ("doc.name", "Document name"), ("user.name", "User name"), ("sheet.name", "Sheet name"), ("date", "Today's date"), ("units", "Drawing units")];

/// Whether `key` is a property field.
pub fn is_field(key: &str) -> bool {
    DRAWING_FIELDS.iter().chain(SHEET_REF_FIELDS).chain(OTHER_FIELDS).any(|(k, _)| *k == key)
}

/// Whether a property field holds a date (Date time format applies).
pub fn is_date_field(key: &str) -> bool {
    key == "date" || key.ends_with("date") || key.ends_with("date_drawn")
}

/// Replace `{key}` occurrences using `lookup`; unknown keys are kept as
/// typed, known but empty values become "----" (like Onshape).
pub fn expand_fields(text: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    expand_fields_with(text, lookup, "----")
}

/// [`expand_fields`] with the empty-value placeholder (Notes › Hide
/// property placeholder: "").
pub fn expand_fields_with(text: &str, lookup: &dyn Fn(&str) -> Option<String>, placeholder: &str) -> String {
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
                // `{key|format}`: a text format or a date time format.
                let (key, fmt) = match after[..j].split_once('|') {
                    Some((k, f)) => (k.trim(), Some(f.trim())),
                    None => (after[..j].trim(), None),
                };
                match lookup(key) {
                    Some(v) if v.is_empty() => out.push_str(placeholder),
                    Some(v) => out.push_str(&fmt.map(|f| field_format(&v, f)).unwrap_or(v)),
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

/// Text formats of inserted properties.
pub const TEXT_FORMATS: [(&str, &str); 5] = [("upper", "All uppercase"), ("lower", "All lowercase"), ("sentence", "Sentence case"), ("title", "Title case"), ("", "Unmodified")];

/// Date time formats of date properties.
/// Onshape's list (live behaviours.md "Note panel popups and values"); `m` is
/// the month, as in Onshape's patterns.
pub const DATE_FORMATS: [&str; 23] = [
    "yyyy-m-d",
    "yyyy-mm-dd",
    "yyyy/mm/dd",
    "m.d.yyyy",
    "m/d/yyyy",
    "m/d/yy",
    "mm-yy",
    "mm/dd/yyyy",
    "mmm. d, yy",
    "mmmm yy",
    "mmmm d, yyyy",
    "d-mmm-yy",
    "d mmmm yy",
    "dd.mm.yyyy",
    "dd/mm/yyyy",
    "dd/mm/yy",
    "dd.mm.yy",
    "d-mmm-yyyy",
    "dd-mm-yyyy",
    "dddd, mmmm d, yyyy",
    "dd mmm yy",
    "yyyy",
    "mm/dd/yy",
];

/// Apply a property's text format (`upper`, `lower`, `sentence`, `title`)
/// or, to YYYY-MM-DD dates, a date time format (`yyyy`, `yy`, `MMMM`,
/// `MMM`, `MM`, `M`, `dddd`, `dd`, `d`).
pub fn field_format(v: &str, fmt: &str) -> String {
    match fmt {
        "upper" => return v.to_uppercase(),
        "lower" => return v.to_lowercase(),
        "sentence" => {
            let l = v.to_lowercase();
            let mut c = l.chars();
            return c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default();
        }
        "title" => {
            return v
                .split(' ')
                .map(|w| {
                    let l = w.to_lowercase();
                    let mut c = l.chars();
                    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(" ");
        }
        _ => {}
    }
    let parts: Vec<i64> = v.split('-').filter_map(|p| p.parse().ok()).collect();
    let [y, m, d] = parts[..] else { return v.to_string() };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return v.to_string();
    }
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    const DAYS: [&str; 7] = ["Thursday", "Friday", "Saturday", "Sunday", "Monday", "Tuesday", "Wednesday"];
    // Days since the epoch (1970-01-01 was a Thursday).
    let days = {
        let (yy, mm) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
        let era = yy.div_euclid(400);
        let yoe = yy - era * 400;
        let doy = (153 * mm + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    };
    let mut out = String::new();
    let f: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < f.len() {
        let c = f[i];
        let mut n = 1;
        while i + n < f.len() && f[i + n] == c {
            n += 1;
        }
        match (c, n) {
            ('y', 4) => out.push_str(&format!("{y:04}")),
            ('y', _) => out.push_str(&format!("{:02}", y % 100)),
            ('M' | 'm', 4) => out.push_str(MONTHS[m as usize - 1]),
            ('M' | 'm', 3) => out.push_str(&MONTHS[m as usize - 1][..3]),
            ('M' | 'm', 2) => out.push_str(&format!("{m:02}")),
            ('M' | 'm', _) => out.push_str(&m.to_string()),
            ('d', 4) => out.push_str(DAYS[days.rem_euclid(7) as usize]),
            ('d', 2) => out.push_str(&format!("{d:02}")),
            ('d', _) => out.push_str(&d.to_string()),
            _ => (0..n).for_each(|_| out.push(c)),
        }
        i += n;
    }
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
    fn fit_deviations_stack_in_callouts() {
        let mut d = Drawing::default();
        d.props.precision = Some(3);
        let f = DimFormat::default();
        let line = format!("⌀6.350 H7 {} THRU", stacked(&format_deviation(&d, &f, 15e-6, false), &format_deviation(&d, &f, 0.0, true)));
        assert_eq!(flatten_stacked(&line), "⌀6.350 H7 +0.015/−0.000 THRU");
        assert_eq!(split_stacked(&line), ("⌀6.350 H7 ", Some(("+0.015", "−0.000")), " THRU"));
        assert_eq!(format_deviation(&d, &f, -5e-6, false), "−0.005");
        assert_eq!(split_stacked("⌀5 THRU"), ("⌀5 THRU", None, ""));
    }

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
        // Angles use the angular precision; the zero settings are for
        // lengths only (Onshape: "Length trailing zeros").
        d.props.angular_precision = Some(1);
        assert_eq!(dim_text(&d, &f, std::f64::consts::FRAC_PI_4, true, "").main, "45,0°");
        d.props.trailing_zeros = true;
        assert_eq!(dim_text(&d, &f, std::f64::consts::FRAC_PI_4, true, "").main, "45,0°");
    }

    #[test]
    fn note_text_codes() {
        assert_eq!(text_codes("90%%d %%C10 %%p0.1"), "90° ⌀10 ±0.1");
        assert_eq!(text_codes("A\\U+00AEB \\u+2300"), "A®B ⌀");
        assert_eq!(text_codes("100%"), "100%");
        assert_eq!(expand_fields("%%c{x}", &|k| (k == "x").then(|| "5".into())), "⌀5");
        assert_eq!(expand_fields("{x|upper} {x|title}", &|k| (k == "x").then(|| "steel plate".into())), "STEEL PLATE Steel Plate");
        assert_eq!(field_format("2026-10-19", "dd.MM.yyyy"), "19.10.2026");
        assert_eq!(field_format("2026-10-19", "d-MMM-yy"), "19-Oct-26");
        assert_eq!(field_format("2026-10-19", "dddd,MMMM d,yyyy"), "Monday,October 19,2026");
        assert_eq!(field_format("2026-10-19", "M/d/yyyy"), "10/19/2026");
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
        assert_eq!(dim_text(&d, &f, 0.01, false, "").main, "10.00±0.05");
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
        // Unset angular precision: Onshape's 0.1°.
        assert_eq!(dim_text(&d, &DimFormat::default(), std::f64::consts::FRAC_PI_4, true, "").main, "45.0°");
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

    #[test]
    fn palette_options_and_units() {
        use crate::drawing::{FitMode, FitOpts, FitStacking};
        let d = Drawing::default();
        // Inches fractional, feet and inches, yards.
        let f = |u| DimFormat { units: Some(u), ..Default::default() };
        assert_eq!(dim_text(&d, &f(DimUnits::InchFraction), 0.0254 * 1.375, false, "").main, "1 3/8");
        assert_eq!(dim_text(&d, &f(DimUnits::InchFraction), 0.0254 * 0.5, false, "").main, "1/2");
        assert_eq!(dim_text(&d, &f(DimUnits::FeetInches), 0.0254 * 14.5, false, "").main, "1' 2.5\"");
        assert_eq!(dim_text(&d, &f(DimUnits::Yard), 0.9144 * 2.0, false, "").main, "2");
        // Parentheses, above / below text, the inspection oval.
        let t = dim_text(&d, &DimFormat { reference: true, above: "TYP".into(), below: "2X".into(), inspection_frame: true, ..Default::default() }, 0.0125, false, "");
        assert_eq!((t.main.as_str(), t.above.as_deref(), t.below.as_deref(), t.oval), ("(12.5)", Some("TYP"), Some("2X"), true));
        // Fit: classes stacked by default, inline when diagonal; with
        // tolerance adds the H7 deviations; tolerance only shows just them.
        let fit = |mode, stacking| DimFormat { tolerance: Tolerance::Fit { hole: "H7".into(), shaft: "g6".into() }, fit: FitOpts { mode, stacking, ..Default::default() }, ..Default::default() };
        let t = dim_text(&d, &fit(FitMode::Fit, FitStacking::WithoutLine), 0.010, false, "");
        assert_eq!((t.main.as_str(), t.upper.as_deref(), t.lower.as_deref()), ("10", Some("H7"), Some("g6")));
        assert_eq!(dim_text(&d, &fit(FitMode::Fit, FitStacking::Diagonal), 0.010, false, "").main, "10 H7/g6");
        let t = dim_text(&d, &fit(FitMode::WithTolerance, FitStacking::WithoutLine), 0.010, false, "");
        assert_eq!(t.main, "10 H7/g6");
        assert!(t.upper.as_deref().is_some_and(|u| u.starts_with('+')), "{t:?}");
        let t = dim_text(&d, &fit(FitMode::ToleranceOnly, FitStacking::WithoutLine), 0.010, false, "");
        assert_eq!(t.main, "10");
        assert!(t.upper.is_some() && t.lower.is_some());
    }

    /// Live asb 10-24 dual dimensions (mm drawing, dual inches 0.12 /
    /// tolerance 0.123): "[2.36 in]" over "60"; ±0.1 → "[2.36±0.004 in]";
    /// no Ø / R in the dual; none on angular dimensions; Show dual unit off
    /// drops " in"; the palette's Single unit / Dual unit win.
    #[test]
    fn dual_dimensions_like_onshape() {
        let mut d = Drawing { units_mm: true, ..Default::default() };
        d.props.dual = super::super::DualDims { show: true, show_unit: true, precision: Some(2), ..Default::default() };
        let f = DimFormat::default();
        let t = dim_text(&d, &f, 0.060, false, "");
        assert_eq!((t.main.as_str(), t.dual.as_deref(), t.dual_below), ("60", Some("[2.36 in]"), false));
        let f = DimFormat { tolerance: Tolerance::Symmetric { value: 0.0001 }, ..Default::default() };
        assert_eq!(dim_text(&d, &f, 0.060, false, "").dual.as_deref(), Some("[2.36±0.004 in]"));
        assert_eq!(dim_text(&d, &DimFormat::default(), 0.010, false, "Ø").dual.as_deref(), Some("[0.39 in]"));
        assert_eq!(dim_text(&d, &DimFormat::default(), 135f64.to_radians(), true, "").dual, None);
        d.props.dual.show_unit = false;
        d.props.dual.dim_location = super::super::DualLocation::Bottom;
        let t = dim_text(&d, &DimFormat::default(), 0.060, false, "");
        assert_eq!((t.dual.as_deref(), t.dual_below), (Some("[2.36]"), true));
        assert_eq!(dim_text(&d, &DimFormat { dual: Some(false), ..Default::default() }, 0.060, false, "").dual, None);
        d.props.dual.show = false;
        assert_eq!(dim_text(&d, &DimFormat { dual: Some(true), ..Default::default() }, 0.060, false, "").dual.as_deref(), Some("[2.36]"));
    }
}
