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

    /// Rows with hole callout texts from `callout(sheet, annotation)`.
    /// Item parsing "Parameter" splits hole callouts (one row per line) and
    /// chamfers (length, angle) into items n.1, n.2...; "Row" keeps one.
    /// `all` lists every annotation (the "Drawing definition list").
    pub fn inspection_rows_with(&self, callout: &dyn Fn(usize, &Annotation) -> Option<Vec<String>>, all: bool) -> Vec<InspectionRow> {
        let mut rows: Vec<InspectionRow> = Vec::new();
        let parse = !self.props.inspection_row_parsing;
        let len_unit = if self.units_mm { "mm" } else { "in" };
        for (si, s) in self.sheets.iter().enumerate() {
            for a in &s.annotations {
                let n = match a.inspection {
                    Some(n) => n,
                    None if all => 0,
                    None => continue,
                };
                let num = |k: Option<usize>| match (n, k) {
                    (0, _) => String::new(),
                    (n, Some(k)) => format!("{n}.{k}"),
                    (n, None) => n.to_string(),
                };
                let row = |k: Option<usize>, nominal: String, unit: &str| InspectionRow { id: a.id, number: num(k), sheet: si + 1, kind: a.kind.type_name().to_string(), unit: unit.to_string(), nominal, tolerance: String::new(), upper: String::new(), lower: String::new() };
                match &a.kind {
                    AnnotationKind::HoleCallout { .. } => {
                        let lines = callout(si, a).unwrap_or_default();
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
                    (Tolerance::Symmetric { value }, Some(v)) => (format!("±{}", fmt(*value, unit)), Some(v + value), Some(v - value)),
                    (Tolerance::Deviation { upper, lower }, Some(v)) => (format!("+{} / {}", fmt(*upper, unit), fmt(*lower, unit)), Some(v + upper), Some(v + lower)),
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
                    kind: a.kind.type_name().to_string(),
                    unit: unit.map(|u| u.to_string()).unwrap_or_default(),
                    nominal: nominal.map(|v| fmt(v, unit)).unwrap_or_default(),
                    tolerance: tol,
                    upper: up.map(|v| fmt(v, unit)).unwrap_or_default(),
                    lower: lo.map(|v| fmt(v, unit)).unwrap_or_default(),
                });
            }
        }
        if !all {
            // By number (1, 1.1, 1.2, 2...); the definition list keeps sheet order.
            rows.sort_by_key(|r| sort_key(&r.number));
        }
        rows
    }

    /// The "Inspection list" export (CSV).
    pub fn inspection_csv(&self) -> String {
        rows_csv(&self.inspection_rows())
    }
}

/// CSV of inspection rows (Inspection list / Drawing definition list).
pub fn rows_csv(rows: &[InspectionRow]) -> String {
    let mut out = String::from("Inspection item number,Sheet number,Annotation type,Unit,Nominal value,Tolerance,Upper limit,Lower limit\n");
    let q = |s: &str| if s.contains(',') || s.contains('"') { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
    for r in rows {
        out.push_str(&format!("{},{},{},{},{},{},{},{}\n", q(&r.number), r.sheet, q(&r.kind), q(&r.unit), q(&r.nominal), q(&r.tolerance), q(&r.upper), q(&r.lower)));
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
        AnnotationKind::AngularDimension { vertex, a: p, b: q, .. } => {
            let ang = |x: [f64; 2]| (x[1] - vertex[1]).atan2(x[0] - vertex[0]);
            let mut s = (ang(*q) - ang(*p)).abs();
            if s > std::f64::consts::PI {
                s = std::f64::consts::TAU - s;
            }
            (Some(s.to_degrees()), Some("deg"))
        }
        k if k.is_dimension() => (None, Some(len_unit)),
        _ => (None, None),
    }
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
        let first = csv.lines().nth(1).unwrap();
        assert_eq!(first, "1,1,Linear dimension,mm,50,±0.1,50.1,49.9");
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
}
