//! Drawing elements that DXF / DWG export writes as one CAD object, not as
//! loose lines and texts (live samples/drw2-k-dxf2018.dxf): dimensions are
//! DIMENSION entities whose anonymous *D block holds what the sheet draws,
//! feature control frames TOLERANCE entities, hole callout texts an INSERT
//! of an anonymous *U block.

use super::sheet::{AnnotationEnv, SheetGeometry, linear_frame};
use super::{Annotation, AnnotationKind, DimOrientation};

/// A dimension's definition (sheet mm; `measurement` in model mm or
/// radians) for its DIMENSION entity.
#[derive(Clone, Debug, PartialEq)]
pub enum CadDimKind {
    /// Rotated linear: the extension lines' origins, the dimension line's
    /// end at the second extension line and its angle (radians).
    Linear { first: [f64; 2], second: [f64; 2], line: [f64; 2], rotation: f64 },
    /// Radius: the center and the arrow's point on the arc.
    Radial { center: [f64; 2], point: [f64; 2] },
    /// Diameter: the arrow's point on the circle and the opposite one.
    Diameter { chord: [f64; 2], far: [f64; 2] },
    /// Three-point angular: the vertex, a point on each side and on the arc.
    Angular { vertex: [f64; 2], first: [f64; 2], second: [f64; 2], arc: [f64; 2] },
    /// Ordinate: the origin, the feature, the leader's end; `x` measures
    /// along the sheet's x.
    Ordinate { origin: [f64; 2], feature: [f64; 2], leader: [f64; 2], x: bool },
}

/// A dimension for export.
#[derive(Clone, Debug, PartialEq)]
pub struct CadDim {
    /// Dual units (live asb-dual-2018: DIMALT; None: single).
    pub dual: Option<CadDual>,
    pub kind: CadDimKind,
    pub measurement: f64,
    /// The text's middle (sheet mm).
    pub text_mid: [f64; 2],
    /// Sheet mm per model mm of its view (DIMLFAC is the inverse).
    pub scale: f64,
}

/// A dimension's dual units in the export (live samples/asb-dual-2018):
/// DIMALTF (dual units per primary unit), DIMALTD, DIMAPOST ("[] in"), and
/// whether the dual goes below the value (text override "<>\P[]" rather
/// than "[]\P<>").
#[derive(Clone, Debug, PartialEq)]
pub struct CadDual {
    pub factor: f64,
    pub precision: u8,
    pub post: String,
    pub below: bool,
}

/// What a group of elements is.
#[derive(Clone, Debug, PartialEq)]
pub enum CadObject {
    Dimension(CadDim),
    /// A feature control frame (its boxes and cell texts): the rows'
    /// cells (symbol, tolerance, datums), the left end of the first row's
    /// middle and the text height (sheet mm).
    Tolerance { at: [f64; 2], rows: Vec<Vec<String>>, height: f64 },
    /// Texts written as a block at `at` (hole callouts).
    Symbol { at: [f64; 2] },
    /// A table: its top-left corner, column widths, row heights (top to
    /// bottom) and each row's cell texts (sheet mm): ACAD_TABLE over a *T
    /// block of what the sheet draws (2004+), else that block's INSERT.
    /// `rows`: each grid row's text style (live: per table kind, Title /
    /// Header / Data row); `aligned`: the cells' MTEXT carry their paragraph
    /// alignment (live: free tables only, `{\pxqc;…}`).
    Table { corner: [f64; 2], widths: Vec<f64>, heights: Vec<f64>, cells: Vec<Vec<String>>, rows: Vec<&'static str>, aligned: bool },
    /// A closed cubic spline as a chain of Bézier spans (`3n + 1` poles,
    /// sheet mm): SPLINE.
    Spline { poles: Vec<[f64; 2]> },
}

/// The closed uniform Catmull-Rom curve through `pts` (`views::closed_spline`)
/// as Bézier spans: `3n + 1` poles, the last the first.
pub fn catmull_rom_poles(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let n = pts.len();
    let mut out = Vec::with_capacity(3 * n + 1);
    for i in 0..n {
        let (p0, p1, p2, p3) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        out.push(p1);
        out.push([p1[0] + (p2[0] - p0[0]) / 6.0, p1[1] + (p2[1] - p0[1]) / 6.0]);
        out.push([p2[0] - (p3[0] - p1[0]) / 6.0, p2[1] - (p3[1] - p1[1]) / 6.0]);
    }
    if let Some(p) = pts.first() {
        out.push(*p);
    }
    out
}

/// Lines, texts and fills of object `obj` (`SheetGeometry::cad_objects`;
/// last span wins).
#[derive(Clone, Debug, PartialEq)]
pub struct CadMember {
    pub lines: std::ops::Range<usize>,
    pub texts: std::ops::Range<usize>,
    pub fills: std::ops::Range<usize>,
    pub obj: usize,
}

/// Per element, the object it belongs to (lines, texts, fills).
pub type CadMembers = (Vec<Option<usize>>, Vec<Option<usize>>, Vec<Option<usize>>);

impl SheetGeometry {
    /// Line `line` is the spline with these Bézier poles.
    pub(crate) fn add_cad_spline(&mut self, line: usize, poles: Vec<[f64; 2]>) {
        let obj = self.cad_objects.len();
        self.cad_objects.push(CadObject::Spline { poles });
        self.cad_members.push(CadMember { lines: line..line + 1, texts: 0..0, fills: 0..0, obj });
    }

    /// Everything since `m` is this table.
    pub(crate) fn add_cad_table(&mut self, m: super::sheet::GeomMark, layout: &super::tables::TableLayout, data: &super::tables::TableData, kind: &super::TableKind) {
        if layout.xs.len() < 2 || layout.ys.len() < 2 {
            return;
        }
        let mut cells: Vec<Vec<String>> = Vec::new();
        if layout.title {
            cells.push(vec![data.title.clone().unwrap_or_default()]);
        }
        cells.extend(data.all_rows().into_iter().cloned());
        let widths = layout.xs.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        let heights: Vec<f64> = layout.ys.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        // Row styles (live drw2-k-dxf2018 / drw2-en-ansi-a STYLE table).
        let [title, header, row] = table_row_styles(kind);
        let n = heights.len();
        let body = layout.title as usize;
        let header_row = data.header.is_some().then(|| if data.header_bottom { n - 1 } else { body });
        let rows = (0..n).map(|r| if layout.title && r == 0 { title } else if Some(r) == header_row { header } else { row }).collect();
        let aligned = matches!(kind, super::TableKind::General { .. });
        let obj = self.cad_objects.len();
        self.cad_objects.push(CadObject::Table { corner: [layout.xs[0], layout.ys[0]], widths, heights, cells, rows, aligned });
        self.cad_members.push(CadMember { lines: m.0..self.lines.len(), texts: m.1..self.texts.len(), fills: m.2..self.fills.len(), obj });
    }

    /// The object of each line, text and fill.
    pub fn cad_members_of(&self) -> CadMembers {
        let mut out = (vec![None; self.lines.len()], vec![None; self.texts.len()], vec![None; self.fills.len()]);
        for m in &self.cad_members {
            for (list, r) in [(&mut out.0, &m.lines), (&mut out.1, &m.texts), (&mut out.2, &m.fills)] {
                for i in r.clone() {
                    if let Some(x) = list.get_mut(i) {
                        *x = Some(m.obj);
                    }
                }
            }
        }
        out
    }

    /// The dimensions, feature control frames and hole callouts among the
    /// annotations laid out from `starts` (each one's first text, line and
    /// fill) to `end`.
    pub(crate) fn add_cad_objects(&mut self, starts: &[(&Annotation, usize, usize, usize)], end: super::sheet::GeomMark, env: &dyn AnnotationEnv) {
        let base = self.style.clone();
        for (k, &(a, t0, l0, f0)) in starts.iter().enumerate() {
            let (l1, t1, f1) = starts.get(k + 1).map(|s| (s.2, s.1, s.3)).unwrap_or(end);
            if t0 == t1 && l0 == l1 {
                continue;
            }
            let mut style = base.clone();
            if let Some(st) = &a.style {
                st.apply(&mut style, true);
            }
            match &a.kind {
                AnnotationKind::Gdt { anchor, frame, extra, .. } => {
                    // The boxes (closed rectangles) and their centred texts.
                    let boxes: Vec<usize> = (l0..l1).filter(|&i| self.lines[i].closed && self.lines[i].points.len() == 4).collect();
                    let cells: Vec<usize> = (t0..t1).filter(|&i| self.texts[i].anchor == super::sheet::Anchor::Center).collect();
                    let Some(h) = cells.first().map(|&i| self.texts[i].height) else { continue };
                    let obj = self.cad_objects.len();
                    let rows = std::iter::once(frame.clone()).chain(extra.rows.iter().cloned()).collect();
                    // Rows are 7/3 of the text tall (`add_annotations`).
                    self.cad_objects.push(CadObject::Tolerance { at: [anchor[0], anchor[1] + h * 7.0 / 6.0], rows, height: h });
                    for i in boxes {
                        self.cad_members.push(CadMember { lines: i..i + 1, texts: 0..0, fills: 0..0, obj });
                    }
                    for i in cells {
                        self.cad_members.push(CadMember { lines: 0..0, texts: i..i + 1, fills: 0..0, obj });
                    }
                    continue;
                }
                AnnotationKind::HoleCallout { .. } if t1 > t0 => {
                    let obj = self.cad_objects.len();
                    self.cad_objects.push(CadObject::Symbol { at: self.texts[t0].pos });
                    self.cad_members.push(CadMember { lines: 0..0, texts: t0..t1, fills: 0..0, obj });
                    continue;
                }
                _ => {}
            }
            let (kind, measurement, view) = match &a.kind {
                AnnotationKind::LinearDimension { view, a: pa, b: pb, offset, orientation, .. } => {
                    let p = env.to_sheet(*view, *pa);
                    let q = env.to_sheet(*view, *pb);
                    let f = linear_frame(p, q, *offset, *orientation, &a.jogs, style.lock_jog_45, style.geometry_gap, style.extension);
                    // As the sheet measures it (view coordinates, along the
                    // view's axes for horizontal / vertical ones).
                    let d = [pb[0] - pa[0], pb[1] - pa[1]];
                    let rot = env.rotation(*view);
                    let (len, rotation) = match orientation {
                        DimOrientation::Aligned => (d[0].hypot(d[1]), (q[1] - p[1]).atan2(q[0] - p[0])),
                        o => {
                            let (axis, angle) = if *o == DimOrientation::Horizontal { ([1.0, 0.0], 0.0) } else { ([0.0, 1.0], std::f64::consts::FRAC_PI_2) };
                            let va = [axis[0] * (-rot).cos() - axis[1] * (-rot).sin(), axis[0] * (-rot).sin() + axis[1] * (-rot).cos()];
                            ((d[0] * va[0] + d[1] * va[1]).abs(), angle)
                        }
                    };
                    (CadDimKind::Linear { first: p, second: q, line: f.dq, rotation }, len, *view)
                }
                AnnotationKind::RadialDimension { view, center, radius, diameter, angle, .. } => {
                    let c = env.to_sheet(*view, *center);
                    let r = radius * env.scale(*view);
                    let dir = [angle.cos(), angle.sin()];
                    let tip = [c[0] + dir[0] * r, c[1] + dir[1] * r];
                    if *diameter {
                        (CadDimKind::Diameter { chord: tip, far: [c[0] - dir[0] * r, c[1] - dir[1] * r] }, radius * 2.0, *view)
                    } else {
                        (CadDimKind::Radial { center: c, point: tip }, *radius, *view)
                    }
                }
                AnnotationKind::AngularDimension { view, vertex, a: pa, b: pb, radius, .. } => {
                    let v = env.to_sheet(*view, *vertex);
                    let pa = env.to_sheet(*view, *pa);
                    let pb = env.to_sheet(*view, *pb);
                    let a0 = (pa[1] - v[1]).atan2(pa[0] - v[0]);
                    let mut a1 = (pb[1] - v[1]).atan2(pb[0] - v[0]);
                    if a1 < a0 {
                        a1 += std::f64::consts::TAU;
                    }
                    let mid = (a0 + a1) / 2.0;
                    (CadDimKind::Angular { vertex: v, first: pa, second: pb, arc: [v[0] + radius * mid.cos(), v[1] + radius * mid.sin()] }, a1 - a0, *view)
                }
                AnnotationKind::OrdinateDimension { view, origin, point, horizontal } => {
                    let value = if *horizontal { (point[0] - origin[0]).abs() } else { (point[1] - origin[1]).abs() };
                    (CadDimKind::Ordinate { origin: env.to_sheet(*view, *origin), feature: env.to_sheet(*view, *point), leader: a.position, x: *horizontal }, value, *view)
                }
                _ => continue,
            };
            let pts: Vec<[f64; 2]> = self.texts[t0..t1].iter().flat_map(super::font::text_box).collect();
            let text_mid = if pts.is_empty() {
                a.position
            } else {
                let (x0, x1) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])));
                let (y0, y1) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p[1]), hi.max(p[1])));
                [(x0 + x1) / 2.0, (y0 + y1) / 2.0]
            };
            let obj = self.cad_objects.len();
            let dual = env.dual_cad(a, matches!(kind, CadDimKind::Angular { .. }));
            self.cad_objects.push(CadObject::Dimension(CadDim { dual, kind, measurement, text_mid, scale: env.scale(view) }));
            self.cad_members.push(CadMember { lines: l0..l1, texts: t0..t1, fills: f0..f1, obj });
        }
    }
}

/// The Title / Header / Data row text styles of a table kind (live).
fn table_row_styles(kind: &super::TableKind) -> [&'static str; 3] {
    use super::TableKind as K;
    match kind {
        K::Bom { .. } => ["Onshape-Standard-BomTable-TitleRow", "Onshape-Standard-BomTable-HeaderRow", "Onshape-Standard-BomTable-DataRow"],
        K::Hole { .. } => ["Onshape-Standard-HoleTable-TitleRow", "Onshape-Standard-HoleTable-HeaderRow", "Onshape-Standard-HoleTable-DataRow"],
        K::Revision { .. } => ["Onshape-Standard-RevisionTable-TitleRow", "Onshape-Standard-RevisionTable-HeaderRow", "Onshape-Standard-RevisionTable-DataRow"],
        K::CutList { .. } => ["Onshape-Standard-CutlistTable-TitleRow", "Onshape-Standard-CutlistTable-HeaderRow", "Onshape-Standard-CutlistTable-DataRow"],
        K::Custom { .. } => ["Onshape-Standard-CustomTable-TitleRow", "Onshape-Standard-CustomTable-HeaderRow", "Onshape-Standard-CustomTable-DataRow"],
        _ => ["Onshape-Standard-Table-TitleRow", "Onshape-Standard-Table-HeaderRow", "Onshape-Standard-Table-DataRow"],
    }
}
