//! Inspection items (Onshape): numbered bubbles on dimensions and
//! annotations, placed by hand or automatically (drawing / sheet / view),
//! reordered, listed in the Inspection table and exported as the
//! "Inspection list" CSV.

use super::{Annotation, AnnotationKind, DimOrientation, Drawing, Tolerance, ViewId};

/// Automatic numbering scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Drawing,
    Sheet(usize),
    View(usize, ViewId),
}

/// Annotations that automatic inspection numbering covers: dimensions,
/// ordinates, surface finish, GD&T and callouts (unless excluded); not
/// basic or reference dimensions, datums or notes.
pub fn eligible(a: &Annotation, exclude_callouts: bool) -> bool {
    match &a.kind {
        AnnotationKind::Balloon { .. } => !exclude_callouts,
        AnnotationKind::SurfaceFinish { .. } | AnnotationKind::Gdt { .. } | AnnotationKind::HoleCallout { .. } => true,
        k if k.is_dimension() => !matches!(a.format.tolerance, Tolerance::Basic) && !a.format.prefix.starts_with('(') && !a.format.suffix.ends_with(')'),
        _ => false,
    }
}

impl Drawing {
    /// The next free inspection number.
    pub fn next_inspection_number(&self) -> u32 {
        self.sheets.iter().flat_map(|s| s.annotations.iter()).filter_map(|a| a.inspection).max().unwrap_or(0) + 1
    }

    /// Automatic: number every eligible annotation without one in the
    /// scope (sheet order, then annotation order). Returns how many.
    pub fn auto_inspection(&mut self, scope: Scope) -> usize {
        let mut next = self.next_inspection_number();
        let exclude = self.props.inspection_exclude_callouts;
        let mut n = 0;
        for (si, s) in self.sheets.iter_mut().enumerate() {
            let in_sheet = match scope {
                Scope::Drawing => true,
                Scope::Sheet(i) | Scope::View(i, _) => i == si,
            };
            if !in_sheet {
                continue;
            }
            for a in s.annotations.iter_mut().filter(|a| a.inspection.is_none() && eligible(a, exclude)) {
                if let Scope::View(_, v) = scope
                    && a.kind.view() != Some(v)
                {
                    continue;
                }
                a.inspection = Some(next);
                next += 1;
                n += 1;
            }
        }
        n
    }

    /// Remove every inspection item.
    pub fn remove_all_inspection(&mut self) {
        for a in self.sheets.iter_mut().flat_map(|s| s.annotations.iter_mut()) {
            a.inspection = None;
        }
    }

    /// Rows of the Inspection table / list: (number, sheet number, type,
    /// nominal, tolerance, upper, lower), by number.
    pub fn inspection_rows(&self) -> Vec<InspectionRow> {
        self.inspection_rows_with(&|_, _| None, false)
    }

    /// Rows with hole callout and note texts from `shown(sheet, annotation)`.
    /// Item parsing "Parameter" splits hole callouts (one row per line) and
    /// chamfers (length, angle) into items n.1, n.2...; "Row" keeps one.
    /// `all` lists every annotation (the "Drawing definition list"): the
    /// numbered first, then the others by sheet (live export-1119), a hole
    /// callout one row per value ("Ø.236", "THRU", "M6x1.00" …), a note its
    /// text.
    pub fn inspection_rows_with(&self, shown: &dyn Fn(usize, &Annotation) -> Option<Vec<String>>, all: bool) -> Vec<InspectionRow> {
        let mut rows: Vec<InspectionRow> = Vec::new();
        let parse = !self.props.inspection_row_parsing;
        let len_unit = if self.units_mm { "mm" } else { "in" };
        for (si, s) in self.sheets.iter().enumerate() {
            for a in &s.annotations {
                let n = match a.inspection {
                    Some(n) => n,
                    None if all && !matches!(a.kind, AnnotationKind::Centermark { .. } | AnnotationKind::Centerline { .. } | AnnotationKind::CircleCenterline { .. } | AnnotationKind::VirtualSharp { .. }) => 0,
                    None => continue,
                };
                let num = |k: Option<usize>| match (n, k) {
                    (0, _) => String::new(),
                    (n, Some(k)) => format!("{n}.{k}"),
                    (n, None) => n.to_string(),
                };
                let row = |k: Option<usize>, nominal: String, unit: &str| InspectionRow { id: a.id, number: num(k), sheet: si + 1, kind: inspection_type(a), unit: unit.to_string(), nominal, tolerance: String::new(), upper: String::new(), lower: String::new() };
                match &a.kind {
                    AnnotationKind::HoleCallout { .. } if n == 0 => {
                        rows.extend(callout_values(&shown(si, a).unwrap_or_default()).into_iter().map(|v| {
                            let unit = if v.ends_with('°') { "deg" } else if v.starts_with(|c: char| c.is_ascii_alphabetic() || c == '#') && v != "THRU" { "N/A" } else { len_unit };
                            row(None, v, unit)
                        }));
                        continue;
                    }
                    AnnotationKind::Note { .. } => {
                        let text = shown(si, a).unwrap_or_default().join("\n");
                        rows.push(InspectionRow { tolerance: "N/A".into(), upper: "N/A".into(), lower: "N/A".into(), ..row(None, text, "N/A") });
                        continue;
                    }
                    AnnotationKind::HoleCallout { .. } => {
                        let lines = shown(si, a).unwrap_or_default();
                        if parse && lines.len() > 1 {
                            rows.extend(lines.into_iter().enumerate().map(|(k, l)| row(Some(k + 1), l, len_unit)));
                        } else {
                            rows.push(row(None, lines.join(" "), len_unit));
                        }
                        continue;
                    }
                    AnnotationKind::ChamferDimension { a: p, b: q, c, d, .. } => {
                        let l = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
                        let (u, v) = ([q[0] - p[0], q[1] - p[1]], [d[0] - c[0], d[1] - c[1]]);
                        let mut ang = (u[0] * v[1] - u[1] * v[0]).abs().atan2(u[0] * v[0] + u[1] * v[1]).to_degrees();
                        if ang > 90.0 {
                            ang = 180.0 - ang;
                        }
                        let (lt, at) = (fmt(l, Some(len_unit)), fmt(ang, Some("deg")));
                        if parse {
                            rows.push(row(Some(1), lt, len_unit));
                            rows.push(row(Some(2), at, "deg"));
                        } else {
                            rows.push(row(None, format!("{lt} X {at}°"), len_unit));
                        }
                        continue;
                    }
                    _ => {}
                }
                let (nominal, unit) = nominal_of(self, a);
                let (tol, up, lo) = match (&a.format.tolerance, nominal) {
                    (Tolerance::Symmetric { value }, Some(v)) => (format!("±{}", self.value_text(*value, unit)), Some(v + value), Some(v - value)),
                    (Tolerance::Deviation { upper, lower }, Some(v)) => (format!("+{} / {}", self.value_text(*upper, unit), self.value_text(*lower, unit)), Some(v + upper), Some(v + lower)),
                    (Tolerance::Limits { upper, lower }, Some(v)) => (String::new(), Some(v + upper), Some(v + lower)),
                    (Tolerance::Fit { hole, shaft }, _) => (format!("{hole}{}{shaft}", if hole.is_empty() || shaft.is_empty() { "" } else { "/" }), None, None),
                    (Tolerance::Min, _) => ("MIN".into(), None, None),
                    (Tolerance::Max, _) => ("MAX".into(), None, None),
                    _ => (String::new(), None, None),
                };
                rows.push(InspectionRow {
                    id: a.id,
                    number: num(None),
                    sheet: si + 1,
                    kind: inspection_type(a),
                    unit: unit.map(|u| u.to_string()).unwrap_or_default(),
                    // A radius or diameter reads as drawn, its R / Ø with it (live drw-inspection-diameter: "Ø.394").
                    nominal: overridden(a)
                        .map(|t| t.parse::<f64>().map_or(t.to_string(), |v| self.value_text(v * if unit == Some("in") { 0.0254 } else { 0.001 }, unit)))
                        .or_else(|| match (&a.kind, nominal) {
                            (AnnotationKind::RadialDimension { diameter, .. }, Some(v)) => {
                                let f = super::DimFormat { tolerance: Tolerance::None, prefix: String::new(), suffix: String::new(), reference: false, ..a.format.clone() };
                                Some(super::format::dim_text(self, &f, v, false, if *diameter { "Ø" } else { "R" }).main)
                            }
                            _ => None,
                        })
                        .or(nominal.map(|v| self.value_text(v, unit)))
                        .unwrap_or_default(),
                    tolerance: tol,
                    upper: up.map(|v| self.value_text(v, unit)).unwrap_or_default(),
                    lower: lo.map(|v| self.value_text(v, unit)).unwrap_or_default(),
                });
            }
        }
        // By number (1, 1.1, 1.2, 2...), the unnumbered after them in sheet order.
        rows.sort_by_key(|r| (r.number.is_empty(), sort_key(&r.number)));
        rows
    }

    /// The "Inspection list" export (CSV).
    pub fn inspection_csv(&self) -> String {
        rows_csv(self, &self.inspection_rows())
    }

    /// A value (meters or degrees) as the lists write it: in the drawing's length unit at its precision.
    fn value_text(&self, v: f64, unit: Option<&'static str>) -> String {
        match unit {
            Some("deg") | None => fmt(v, unit),
            _ => {
                let k = if unit == Some("in") { 1.0 / 0.0254 } else { 1000.0 };
                let s = super::format::format_number(v * k, self.props.precision, if unit == Some("in") { 3 } else { 2 });
                if self.props.precision.is_some() && !self.props.trailing_zeros && s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
            }
        }
    }
}

/// A typed value that overrides a dimension's.
fn overridden(a: &Annotation) -> Option<&str> {
    match &a.kind {
        AnnotationKind::LinearDimension { override_text: Some(t), .. } => Some(t.as_str()),
        _ => a.format.value.as_deref(),
    }
}

/// A hole callout's values, as the Drawing definition list rows them (live export-1119): each line's words but the
/// count ("4X") and the countersink's "X", a symbol joined to its value ("⌴Ø.433", "↧.256").
fn callout_values(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for l in lines {
        for (i, w) in l.split_whitespace().enumerate() {
            let count = i == 0 && w.len() > 1 && w.ends_with('X') && w[..w.len() - 1].chars().all(|c| c.is_ascii_digit());
            if count || w == "X" {
                continue;
            }
            out.push(w.to_string());
        }
    }
    out
}

/// CSV of inspection rows (Inspection list / Drawing definition list), as live's (export-1119): a BOM, CRLF lines,
/// "--" for no item number or value, the sheet's name and number.
pub fn rows_csv(drawing: &Drawing, rows: &[InspectionRow]) -> String {
    let mut out = String::from("\u{feff}Item,Sheet,Type,Units,Nominal value,Tolerance,Upper limit,Lower limit\r\n");
    let q = |s: &str| {
        let s = if s.is_empty() { "--".to_string() } else { s.replace('⌀', "Ø") };
        if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
    };
    for r in rows {
        let sheet = format!("{} ({})", drawing.sheets.get(r.sheet - 1).map_or("", |s| s.name.as_str()), r.sheet);
        let unit = if r.unit.is_empty() { "N/A" } else { r.unit.as_str() };
        out.push_str(&format!("{},{},{},{},{},{},{},{}\r\n", q(&r.number), q(&sheet), q(&r.kind), q(unit), q(&r.nominal), q(&r.tolerance), q(&r.upper), q(&r.lower)));
    }
    out
}

fn sort_key(n: &str) -> (u32, u32) {
    let (a, b) = n.split_once('.').unwrap_or((n, "0"));
    (a.parse().unwrap_or(0), b.parse().unwrap_or(0))
}

/// One row of the inspection list.
#[derive(Clone, Debug, PartialEq)]
pub struct InspectionRow {
    /// The annotation.
    pub id: u64,
    /// "3", or "3.1" for a parsed parameter; empty when unnumbered.
    pub number: String,
    pub sheet: usize,
    pub kind: String,
    pub unit: String,
    pub nominal: String,
    pub tolerance: String,
    pub upper: String,
    pub lower: String,
}

/// Value text in the drawing's length unit or degrees.
fn fmt(v: f64, unit: Option<&'static str>) -> String {
    let (k, d) = match unit {
        Some("in") => (1.0 / 0.0254, 3),
        Some("deg") => (1.0, 2),
        _ => (1000.0, 2),
    };
    super::format::format_number(v * k, None, d)
}

/// The measured value of a dimension (meters or degrees) and its unit.
fn nominal_of(d: &Drawing, a: &Annotation) -> (Option<f64>, Option<&'static str>) {
    let len_unit = if d.units_mm { "mm" } else { "in" };
    let dist = |p: [f64; 2], q: [f64; 2]| ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
    match &a.kind {
        AnnotationKind::LinearDimension { a: p, b: q, orientation, .. } => {
            let v = match orientation {
                DimOrientation::Horizontal => (q[0] - p[0]).abs(),
                DimOrientation::Vertical => (q[1] - p[1]).abs(),
                _ => dist(*p, *q),
            };
            (Some(v), Some(len_unit))
        }
        AnnotationKind::RadialDimension { radius, diameter, .. } => (Some(if *diameter { radius * 2.0 } else { *radius }), Some(len_unit)),
        k @ AnnotationKind::AngularDimension { .. } => {
            let (vertex, p, q) = k.angular_legs().expect("angular");
            let ang = |x: [f64; 2]| (x[1] - vertex[1]).atan2(x[0] - vertex[0]);
            let mut s = (ang(q) - ang(p)).abs();
            if s > std::f64::consts::PI {
                s = std::f64::consts::TAU - s;
            }
            (Some(s.to_degrees()), Some("deg"))
        }
        k if k.is_dimension() => (None, Some(len_unit)),
        _ => (None, None),
    }
}

/// The Type column: how a dimension was made, not its orientation (live
/// request 1735: one edge "Aligned dimension" even when horizontal, two
/// points or a model dimension "2 point linear dimension", two lines "Line to
/// line angular dimension").
fn inspection_type(a: &Annotation) -> String {
    let kind = match &a.kind {
        AnnotationKind::LinearDimension { .. } if a.one_edge => "Aligned dimension",
        AnnotationKind::LinearDimension { .. } => "2 point linear dimension",
        AnnotationKind::AngularDimension { three_point: false, .. } => "Line to line angular dimension",
        AnnotationKind::HoleCallout { .. } => "Hole/thread callout",
        k => k.type_name(),
    };
    // Live export-1119: "Aligned dimension (overridden)".
    if a.kind.is_dimension() && overridden(a).is_some() { format!("{kind} (overridden)") } else { kind.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::DimFormat;

    #[test]
    fn automatic_numbering_skips_basic_and_notes_and_exports_csv() {
        let mut d = Drawing::default();
        let lin = |tol: Tolerance| Annotation {
            kind: AnnotationKind::LinearDimension { view: ViewId(1), a: [0.0, 0.0], b: [0.05, 0.0], offset: 5.0, orientation: DimOrientation::Horizontal, override_text: None },
            format: DimFormat { tolerance: tol, ..Default::default() },
            ..Default::default()
        };
        let s = &mut d.sheets[0];
        s.annotations.push(Annotation { id: 1, ..lin(Tolerance::Symmetric { value: 0.0001 }) });
        s.annotations.push(Annotation { id: 2, ..lin(Tolerance::Basic) });
        s.annotations.push(Annotation { id: 3, kind: AnnotationKind::note("N", 3.5), ..Default::default() });
        s.annotations.push(Annotation { id: 4, kind: AnnotationKind::balloon(ViewId(1), [0.0, 0.0], "1"), ..Default::default() });
        assert_eq!(d.auto_inspection(Scope::Drawing), 2);
        let nums: Vec<Option<u32>> = d.sheets[0].annotations.iter().map(|a| a.inspection).collect();
        assert_eq!(nums, vec![Some(1), None, None, Some(2)]);
        assert_eq!(d.next_inspection_number(), 3);
        let csv = d.inspection_csv();
        assert!(csv.starts_with("\u{feff}Item,Sheet,Type,Units,Nominal value,Tolerance,Upper limit,Lower limit\r\n"), "{csv}");
        let first = csv.lines().nth(1).unwrap();
        assert_eq!(first, format!("1,{} (1),2 point linear dimension,mm,50,±0.1,50.1,49.9", d.sheets[0].name));
        // One edge: Aligned, also when horizontal (live request 1735).
        d.sheets[0].annotations[0].one_edge = true;
        assert_eq!(d.inspection_rows()[0].kind, "Aligned dimension");
        d.sheets[0].annotations[0].one_edge = false;
        // Parameter parsing splits a chamfer; Row keeps one item.
        let s = &mut d.sheets[0];
        s.annotations.push(Annotation { id: 5, kind: AnnotationKind::ChamferDimension { view: ViewId(1), a: [0.0, 0.0], b: [0.001, 0.001], c: [0.0, 0.0], d: [0.01, 0.0] }, inspection: Some(3), ..Default::default() });
        let rows = d.inspection_rows();
        assert_eq!(rows.iter().map(|r| r.number.as_str()).collect::<Vec<_>>(), vec!["1", "2", "3.1", "3.2"]);
        assert_eq!(rows[3].nominal, "45");
        d.props.inspection_row_parsing = true;
        assert_eq!(d.inspection_rows().len(), 3);
        // The definition list has every annotation.
        assert_eq!(d.inspection_rows_with(&|_, _| None, true).len(), 5);
        d.sheets[0].annotations.pop();
        // Excluding callouts.
        d.remove_all_inspection();
        d.props.inspection_exclude_callouts = true;
        assert_eq!(d.auto_inspection(Scope::Sheet(0)), 1);
    }

    #[test]
    fn a_radius_or_diameter_nominal_reads_as_drawn() {
        let mut d = Drawing::default();
        let radial = |id: u64, diameter: bool| Annotation {
            id,
            kind: AnnotationKind::RadialDimension { view: ViewId(1), center: [0.0, 0.0], radius: 0.005, diameter, angle: 0.0, dim_line: false, knee: None },
            format: DimFormat { tolerance: Tolerance::Symmetric { value: 0.0001 }, ..Default::default() },
            inspection: Some(id as u32),
            ..Default::default()
        };
        d.sheets[0].annotations.extend([radial(1, true), radial(2, false)]);
        let rows = d.inspection_rows();
        // Live drw-inspection-diameter: "Ø.394" on an inch drawing; the tolerance in its own column.
        assert_eq!((rows[0].kind.as_str(), rows[0].nominal.as_str(), rows[0].tolerance.as_str()), ("Diameter dimension", "Ø10", "±0.1"));
        assert_eq!(rows[1].nominal, "R5");
    }
}
