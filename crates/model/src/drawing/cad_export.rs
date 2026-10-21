//! Drawing export to DWG / DWT and DXF 2000+ (Export Drawing › Version)
//! through `acadrust`, laid out like Onshape's own exports (live
//! samples/drw2-asbu-aux.dxf): Onshape's layers (`cad_layers`) with their
//! line types and lineweights, filled triangles as SOLIDs, texts as MTEXT
//! in the Onshape-Standard styles (polylines for drawing symbols the CAD
//! fonts lack), element colors as true colors.

use super::export::needs_strokes;
use super::font;
use super::cad_fit::{FitCurve, fit_curve};
use super::cad_layers::CadLayer;
use super::sheet::{Anchor, SheetGeometry};
use acadrust::entities::Solid;
use acadrust::entities::hatch::{BoundaryEdge, BoundaryPath, Hatch, HatchPattern, HatchPatternLine, HatchPatternType, LineEdge};
use acadrust::entities::mtext::AttachmentPoint;
use acadrust::tables::linetype::LineTypeElement;
use acadrust::{Arc, CadDocument, Circle, Ellipse, Color, DwgWriter, DxfVersion, DxfWriter, EntityType, Layer, Line, LineType, LineWeight, LwPolyline, MText, TextStyle, Vector2, Vector3};

/// A DWG / DXF file version.
pub type CadVersion = DxfVersion;

/// DWG / DXF versions offered by Export Drawing (newest first).
pub const VERSIONS: [(&str, DxfVersion); 7] = [
    ("2018", DxfVersion::AC1032),
    ("2013", DxfVersion::AC1027),
    ("2010", DxfVersion::AC1024),
    ("2007", DxfVersion::AC1021),
    ("2004", DxfVersion::AC1018),
    ("2000", DxfVersion::AC1015),
    ("R14", DxfVersion::AC1014),
];

/// Onshape's line types (live AC1032 LTYPE table; inches, scaled to the
/// file's units).
const LINE_TYPES: [(&str, &str, &[f64]); 4] = [
    ("DASHED", "Dashed line __ __ __ __ __", &[0.5, -0.25]),
    ("ONS_PHANTOM", "Onshape Phantom line ______ __ __ ______", &[0.25, -0.05, 0.05, -0.05, 0.05, -0.05]),
    ("CENTER", "Center line ____ _ ____ _ ____ _", &[0.45, -0.09, 0.09, -0.09]),
    ("HIDDEN", "Hidden line ____ ____ ____ ____", &[0.1, -0.05]),
];

/// The layer table's order (live: 0, Defpoints, then these as used).
const LAYER_ORDER: [CadLayer; 24] = [
    CadLayer::Visible,
    CadLayer::Hidden,
    CadLayer::TangentLines,
    CadLayer::TangentHidden,
    CadLayer::ViewHatches,
    CadLayer::BorderFrame,
    CadLayer::BorderZones,
    CadLayer::TitleBlock,
    CadLayer::Tables,
    CadLayer::AnnotationText,
    CadLayer::AnnotationLines,
    CadLayer::SectionCuttingLine,
    CadLayer::Centermarks,
    CadLayer::OriginIndicators,
    CadLayer::Centerlines,
    CadLayer::DetailViewBorder,
    CadLayer::DetailViewParentBorder,
    CadLayer::BreakLines,
    CadLayer::VirtualSharps,
    CadLayer::SketchedGeometry,
    CadLayer::Images,
    CadLayer::BendLinesUp,
    CadLayer::BendLinesDown,
    CadLayer::BendTangentLines,
];

/// The text style of the drawing's text (live: Onshape-Standard on Noto
/// Sans; bold texts Onshape-Standard-Bold).
const TEXT_STYLES: [(&str, &str); 2] = [("Onshape-Standard", "NotoSans-Regular.ttf"), ("Onshape-Standard-Bold", "NotoSans-Bold.ttf")];

fn color(c: Option<[u8; 3]>) -> Color {
    match c {
        Some([r, g, b]) => Color::from_rgb(r, g, b),
        None => Color::ByLayer,
    }
}

/// How close (sheet mm) a polyline's points must be to a circle or ellipse
/// to be written as one.
const FIT_TOL: f64 = 1e-4;

/// Every export layer a sheet uses.
fn used_layers(g: &SheetGeometry) -> std::collections::BTreeSet<CadLayer> {
    let mut out: std::collections::BTreeSet<CadLayer> = (0..g.lines.len()).map(|i| g.cad_line_layer(i)).collect();
    out.extend((0..g.texts.len()).map(|i| g.cad_text_layer(i)));
    out.extend((0..g.fills.len()).map(|i| g.cad_fill_layer(i)));
    out
}

/// The sheet as a CAD document (model space, `insunits` as in DXF, the
/// coordinates scaled from mm accordingly).
pub fn cad_document(g: &SheetGeometry, insunits: u8, version: DxfVersion) -> Result<CadDocument, String> {
    cad_document_pages(std::slice::from_ref(g), insunits, version)
}

/// Several sheets: the first in model space, the others in layouts
/// "Sheet2", "Sheet3"... (a DWT keeps sheet 2 as the continuation sheet).
pub fn cad_document_pages(pages: &[SheetGeometry], insunits: u8, version: DxfVersion) -> Result<CadDocument, String> {
    let k = match insunits {
        1 => 1.0 / 25.4,
        2 => 1.0 / 304.8,
        5 => 0.1,
        6 => 0.001,
        _ => 1.0,
    };
    let mut doc = CadDocument::with_version(version);
    doc.header.insertion_units = insunits as i16;
    for (name, description, pattern) in LINE_TYPES {
        let mut lt = LineType::new(name);
        lt.description = description.to_string();
        let to_units = |inch: f64| inch * 25.4 * k;
        for &p in pattern {
            lt.add_element(if p > 0.0 { LineTypeElement::dash(to_units(p)) } else { LineTypeElement::space(to_units(-p)) });
        }
        lt.pattern_length = pattern.iter().map(|p| to_units(p.abs())).sum();
        let _ = doc.line_types.add(lt);
    }
    for (name, font) in TEXT_STYLES {
        let mut st = TextStyle::new(name);
        st.font_file = font.to_string();
        let _ = doc.text_styles.add(st);
    }
    // Layers: 0, Defpoints, then the ones the sheets use, in live's order.
    let mut used = std::collections::BTreeSet::new();
    for g in pages {
        used.extend(used_layers(g));
    }
    let mut defpoints = Layer::new("Defpoints");
    defpoints.color = Color::from_index(7);
    let _ = doc.layers.add(defpoints);
    for l in LAYER_ORDER.iter().copied().filter(|l| used.contains(l)) {
        let mut layer = Layer::new(l.name(false));
        layer.color = Color::from_index(7);
        layer.line_type = l.line_type().to_string();
        layer.line_weight = match l.line_weight() {
            w if w >= 0 => LineWeight::Value(w),
            _ => LineWeight::Default,
        };
        let _ = doc.layers.add(layer);
    }
    for (pi, g) in pages.iter().enumerate() {
        let layout = (pi > 0).then(|| format!("Sheet{}", pi + 1));
        if let Some(name) = &layout {
            doc.add_layout(name).map_err(|e| e.to_string())?;
        }
        add_sheet(&mut doc, g, k, layout.as_deref())?;
    }
    Ok(doc)
}

/// MTEXT's escapes for literal backslashes and braces.
fn mtext_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('{', "\\{").replace('}', "\\}")
}

fn add_sheet(doc: &mut CadDocument, g: &SheetGeometry, k: f64, layout: Option<&str>) -> Result<(), String> {
    let p2 = |p: [f64; 2]| Vector2::new(p[0] * k, p[1] * k);
    let p3 = |p: [f64; 2]| Vector3::new(p[0] * k, p[1] * k, 0.0);
    let add = |doc: &mut CadDocument, e: EntityType| {
        match layout {
            Some(name) => doc.add_entity_to_layout(e, name),
            None => doc.add_entity(e),
        }
        .map(|_| ())
        .map_err(|e| e.to_string())
    };
    // Section hatches as HATCH entities (live: ANSI31, the 45° lines at the
    // view's spacing); their hatch lines aren't written.
    let mut hatch_line = vec![false; g.lines.len()];
    for h in &g.hatches {
        let n = hatch_line.len();
        for i in h.lines.clone().filter(|i| *i < n) {
            hatch_line[i] = true;
        }
        let mut e = Hatch::new();
        let mut pattern = HatchPattern::new("ANSI31");
        let s = h.spacing * k;
        pattern.add_line(HatchPatternLine { angle: std::f64::consts::FRAC_PI_4, base_point: Vector2::new(0.0, 0.0), offset: Vector2::new(-s / std::f64::consts::SQRT_2, s / std::f64::consts::SQRT_2), dash_lengths: Vec::new() });
        e.pattern = pattern;
        e.is_solid = false;
        e.pattern_type = HatchPatternType::Custom;
        e.pattern_angle = 0.0;
        // ANSI31's lines are 0.125 in apart at scale 1.
        e.pattern_scale = h.spacing / 3.175;
        for lp in &h.loops {
            let mut path = BoundaryPath::new();
            for (i, a) in lp.iter().enumerate() {
                let b = lp[(i + 1) % lp.len()];
                path.add_edge(BoundaryEdge::Line(LineEdge { start: p2(*a), end: p2(b) }));
            }
            e.paths.push(path);
        }
        if let Some(p) = h.loops.first().and_then(|l| l.first()) {
            e.seed_points.push(p2(*p));
        }
        e.common.layer = CadLayer::ViewHatches.name(false).into();
        add(doc, EntityType::Hatch(Box::new(e)))?;
    }
    for (li, l) in g.lines.iter().enumerate() {
        if l.points.len() < 2 || hatch_line[li] {
            continue;
        }
        let c = color(g.line_color(li));
        let layer = g.cad_line_layer(li).name(false).to_string();
        // Circles, arcs and ellipses as themselves (live), not polylines.
        if let Some(curve) = fit_curve(&l.points, l.closed, FIT_TOL) {
            let e = match curve {
                FitCurve::Circle { center, radius } => {
                    let mut e = Circle::from_center_radius(p3(center), radius * k);
                    (e.common.layer, e.common.color) = (layer, c);
                    EntityType::Circle(e)
                }
                FitCurve::Arc { center, radius, start, end } => {
                    let mut e = Arc::new();
                    (e.center, e.radius, e.start_angle, e.end_angle) = (p3(center), radius * k, start, end);
                    (e.common.layer, e.common.color) = (layer, c);
                    EntityType::Arc(e)
                }
                FitCurve::Ellipse { center, major, ratio, start, end } => {
                    let mut e = Ellipse::from_center_axes(p3(center), Vector3::new(major[0] * k, major[1] * k, 0.0), ratio);
                    (e.start_parameter, e.end_parameter) = (start, end);
                    (e.common.layer, e.common.color) = (layer, c);
                    EntityType::Ellipse(e)
                }
            };
            add(doc, e)?;
            continue;
        }
        if l.points.len() == 2 && !l.closed {
            let mut e = Line::from_points(p3(l.points[0]), p3(l.points[1]));
            e.common.layer = layer;
            e.common.color = c;
            add(doc, EntityType::Line(e))?;
        } else {
            let mut e = LwPolyline::from_points(l.points.iter().map(|p| p2(*p)).collect());
            e.is_closed = l.closed;
            e.common.layer = layer;
            e.common.color = c;
            add(doc, EntityType::LwPolyline(e))?;
        }
    }
    for (fi, f) in g.fills.iter().enumerate() {
        if f.points.len() < 3 {
            continue;
        }
        let layer = g.cad_fill_layer(fi).name(false);
        for i in 1..f.points.len() - 1 {
            let (a, b, c) = (p3(f.points[0]), p3(f.points[i]), p3(f.points[i + 1]));
            let mut e = Solid::new(a, b, c, c);
            e.common.layer = layer.into();
            e.common.color = color(g.fill_color(fi));
            add(doc, EntityType::Solid(e))?;
        }
    }
    for (ti, t) in g.texts.iter().enumerate() {
        let c = color(g.text_color(ti));
        let layer = g.cad_text_layer(ti).name(false);
        if needs_strokes(t) {
            for st in font::text_strokes(t).into_iter().filter(|s| s.len() >= 2) {
                let mut e = LwPolyline::from_points(st.iter().map(|p| p2(*p)).collect());
                e.common.layer = layer.into();
                e.common.color = c;
                add(doc, EntityType::LwPolyline(e))?;
            }
            continue;
        }
        // Live writes every text as MTEXT.
        let mut e = MText::new();
        e.value = mtext_escape(&t.text);
        e.height = t.height * k;
        e.rotation = t.angle;
        e.insertion_point = p3(t.pos);
        e.attachment_point = match t.anchor {
            Anchor::BottomLeft => AttachmentPoint::BottomLeft,
            Anchor::BottomCenter => AttachmentPoint::BottomCenter,
            Anchor::Center => AttachmentPoint::MiddleCenter,
            Anchor::TopLeft => AttachmentPoint::TopLeft,
        };
        e.style = if g.text_font(ti).is_some_and(|f| f.bold) { TEXT_STYLES[1].0 } else { TEXT_STYLES[0].0 }.to_string();
        e.common.layer = layer.into();
        e.common.color = c;
        add(doc, EntityType::MText(Box::new(e)))?;
    }
    Ok(())
}

/// Binary DWG (DWT is the same file as a template).
pub fn to_dwg(g: &SheetGeometry, insunits: u8, version: DxfVersion) -> Result<Vec<u8>, String> {
    to_dwg_pages(std::slice::from_ref(g), insunits, version)
}

pub fn to_dwg_pages(pages: &[SheetGeometry], insunits: u8, version: DxfVersion) -> Result<Vec<u8>, String> {
    let doc = cad_document_pages(pages, insunits, version)?;
    DwgWriter::write_to_vec(&doc).map_err(|e| e.to_string())
}

/// ASCII DXF of a 2000+ version.
pub fn to_dxf_version(g: &SheetGeometry, insunits: u8, version: DxfVersion) -> Result<Vec<u8>, String> {
    let doc = cad_document(g, insunits, version)?;
    DxfWriter::new(&doc).write_to_vec().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::sheet::LineStyle;
    use crate::drawing::{DimOrientation, Sheet};

    fn sample() -> SheetGeometry {
        let sheet = Sheet::default();
        let (w, h) = sheet.size.size_mm();
        let mut g = SheetGeometry::new(w, h);
        g.add_border_and_title(&sheet);
        g.rect(50.0, 50.0, 150.0, 120.0, LineStyle::Visible);
        g.line([50.0, 85.0], [150.0, 85.0], LineStyle::Hidden);
        g.linear_dimension([50.0, 120.0], [150.0, 120.0], 10.0, DimOrientation::Horizontal, "100", 1);
        g
    }

    #[test]
    fn dwg_and_dxf_round_trip() {
        let g = sample();
        for (name, v) in VERSIONS {
            let dwg = to_dwg(&g, 4, v).unwrap();
            let back = acadrust::DwgReader::from_stream(std::io::Cursor::new(dwg.clone())).read().unwrap();
            assert!(back.entities().count() >= g.lines.len(), "{name}");
            // An independent reader (cadkit-dwg, used by Insert DXF or DWG) opens it too.
            let other = noshape_sketch::dwg::parse_dwg(&dwg).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!other.entities.is_empty(), "{name}");
        }
        // DWT: two sheets, the second in a layout.
        let dwt = to_dwg_pages(&[g.clone(), g.clone()], 4, DxfVersion::AC1027).unwrap();
        assert_eq!(&dwt[..6], b"AC1027");
        let dxf = String::from_utf8(to_dxf_version(&g, 1, DxfVersion::AC1015).unwrap()).unwrap();
        assert!(dxf.contains("AC1015") && dxf.contains("HIDDEN") && dxf.contains("TEXT"));
    }

    /// Live (samples/drw2-asbu-aux.dxf): Onshape's layers with their line
    /// types and weights, MTEXT, circles and arcs as CIRCLE / ARC.
    #[test]
    fn layers_mtext_and_curves_like_onshape() {
        let mut g = sample();
        let m = g.mark();
        let circle: Vec<[f64; 2]> = (0..48).map(|i| {
            let t = i as f64 / 48.0 * std::f64::consts::TAU;
            [100.0 + 10.0 * t.cos(), 80.0 + 10.0 * t.sin()]
        }).collect();
        g.lines.push(crate::drawing::sheet::Polyline2 { points: circle, style: LineStyle::Visible, closed: true, pick: 0 });
        let arc: Vec<[f64; 2]> = (0..=12).map(|i| {
            let t = i as f64 / 12.0 * std::f64::consts::FRAC_PI_2;
            [60.0 + 5.0 * t.cos(), 60.0 + 5.0 * t.sin()]
        }).collect();
        g.lines.push(crate::drawing::sheet::Polyline2 { points: arc, style: LineStyle::Visible, closed: false, pick: 0 });
        g.cad_since(m, CadLayer::Visible);
        let m = g.mark();
        g.line([10.0, 10.0], [20.0, 20.0], LineStyle::Centerline);
        g.cad_since(m, CadLayer::Centermarks);
        let dxf = String::from_utf8(to_dxf_version(&g, 4, DxfVersion::AC1032).unwrap()).unwrap();
        let doc = acadrust::DxfReader::from_reader(std::io::Cursor::new(dxf.as_bytes().to_vec())).unwrap().read().unwrap();
        let layer = |n: &str| doc.layers.iter().find(|l| l.name == n).cloned();
        let visible = layer("Visible").expect("Visible layer");
        assert_eq!(visible.line_weight, LineWeight::Value(40));
        assert_eq!(layer("HIDDEN").unwrap().line_type, "HIDDEN");
        assert!(layer("CENTERMARKS").is_some() && layer("Defpoints").is_some());
        assert!(layer("VISIBLE").is_none() && layer("ANNOTATION").is_none(), "no old layer names");
        let kinds: Vec<&str> = doc.entities().map(|e| match e {
            EntityType::Circle(_) => "CIRCLE",
            EntityType::Arc(_) => "ARC",
            EntityType::MText(_) => "MTEXT",
            EntityType::Text(_) => "TEXT",
            _ => "",
        }).collect();
        assert!(kinds.contains(&"CIRCLE") && kinds.contains(&"ARC") && kinds.contains(&"MTEXT") && !kinds.contains(&"TEXT"));
        assert!(doc.entities().any(|e| matches!(e, EntityType::Circle(c) if c.common.layer == "Visible" && (c.radius - 10.0).abs() < 1e-6)));
    }

    /// Live: a section's hatch is one HATCH (ANSI31) on VIEW_HATCHES, not lines.
    #[test]
    fn section_hatch_is_a_hatch_entity() {
        let mut g = SheetGeometry::new(297.0, 210.0);
        let segs = [[[0.0, 0.0], [20.0, 0.0]], [[20.0, 10.0], [20.0, 0.0]], [[0.0, 10.0], [20.0, 10.0]], [[0.0, 0.0], [0.0, 10.0]]];
        let loops = crate::drawing::cad_layers::chain_loops(&segs, 1e-6).unwrap();
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].len(), 4);
        let l0 = g.lines.len();
        for l in crate::drawing::build::hatch(&segs, 2.5) {
            g.line(l[0], l[1], LineStyle::Thin);
        }
        assert!(g.lines.len() > l0);
        g.hatches.push(crate::drawing::cad_layers::HatchRegion { loops, spacing: 2.5, lines: l0..g.lines.len() });
        let dxf = String::from_utf8(to_dxf_version(&g, 4, DxfVersion::AC1032).unwrap()).unwrap();
        let doc = acadrust::DxfReader::from_reader(std::io::Cursor::new(dxf.as_bytes().to_vec())).unwrap().read().unwrap();
        let hatches: Vec<&acadrust::entities::hatch::Hatch> = doc.entities().filter_map(|e| if let EntityType::Hatch(h) = e { Some(h.as_ref()) } else { None }).collect();
        assert_eq!(hatches.len(), 1);
        assert_eq!(hatches[0].pattern.name, "ANSI31");
        assert_eq!(hatches[0].common.layer, "VIEW_HATCHES");
        assert_eq!(hatches[0].paths[0].edges.len(), 4);
        assert!(!doc.entities().any(|e| matches!(e, EntityType::Line(_))), "the hatch lines aren't written");
    }
}
