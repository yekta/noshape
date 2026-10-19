//! Drawing export to DWG / DWT and DXF 2000+ (Export Drawing › Version)
//! through `acadrust`: lines and polylines on per-style layers with their
//! line types, filled triangles as SOLIDs, texts as TEXT (or polylines for
//! drawing symbols the CAD fonts lack), element colors as true colors.

use super::export::needs_strokes;
use super::font;
use super::sheet::{Anchor, LineStyle, SheetGeometry};
use acadrust::entities::Solid;
use acadrust::entities::text::{TextHorizontalAlignment, TextVerticalAlignment};
use acadrust::tables::linetype::LineTypeElement;
use acadrust::{CadDocument, Color, DwgWriter, DxfVersion, DxfWriter, EntityType, Layer, Line, LineType, LwPolyline, Text, Vector2, Vector3};

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

const STYLES: [LineStyle; 8] = [LineStyle::Visible, LineStyle::Hidden, LineStyle::Tangent, LineStyle::Phantom, LineStyle::Centerline, LineStyle::Thin, LineStyle::Border, LineStyle::Section];

fn line_type_of(st: LineStyle) -> &'static str {
    match st {
        LineStyle::Hidden => "HIDDEN",
        LineStyle::Centerline | LineStyle::Section => "CENTER",
        LineStyle::Phantom => "PHANTOM",
        _ => "Continuous",
    }
}

fn color(c: Option<[u8; 3]>) -> Color {
    match c {
        Some([r, g, b]) => Color::from_rgb(r, g, b),
        None => Color::ByLayer,
    }
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
    for (name, pattern) in [("HIDDEN", &[3.0, -1.5][..]), ("CENTER", &[8.0, -1.5, 1.5, -1.5][..]), ("PHANTOM", &[8.0, -1.5, 1.5, -1.5, 1.5, -1.5][..])] {
        let mut lt = LineType::new(name);
        lt.description = name.to_string();
        for &p in pattern {
            lt.add_element(if p > 0.0 { LineTypeElement::dash(p * k) } else { LineTypeElement::space(-p * k) });
        }
        lt.pattern_length = pattern.iter().map(|p: &f64| p.abs() * k).sum();
        let _ = doc.line_types.add(lt);
    }
    for st in STYLES.iter().map(|s| s.layer()).chain(["ANNOTATION", "BORDER_ZONES", "TITLE_BLOCK"]).collect::<std::collections::BTreeSet<_>>() {
        let mut layer = Layer::new(st);
        layer.color = Color::from_index(7);
        layer.line_type = STYLES.iter().find(|s| s.layer() == st).map(|s| line_type_of(*s)).unwrap_or("Continuous").to_string();
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
    for (li, l) in g.lines.iter().enumerate() {
        if l.points.len() < 2 {
            continue;
        }
        let c = color(g.line_color(li));
        if l.points.len() == 2 && !l.closed {
            let mut e = Line::from_points(p3(l.points[0]), p3(l.points[1]));
            e.common.layer = g.line_layer(li).to_string();
            e.common.color = c;
            add(doc, EntityType::Line(e))?;
        } else {
            let mut e = LwPolyline::from_points(l.points.iter().map(|p| p2(*p)).collect());
            e.is_closed = l.closed;
            e.common.layer = g.line_layer(li).to_string();
            e.common.color = c;
            add(doc, EntityType::LwPolyline(e))?;
        }
    }
    for (fi, f) in g.fills.iter().enumerate() {
        if f.points.len() < 3 {
            continue;
        }
        for i in 1..f.points.len() - 1 {
            let (a, b, c) = (p3(f.points[0]), p3(f.points[i]), p3(f.points[i + 1]));
            let mut e = Solid::new(a, b, c, c);
            e.common.layer = g.fill_layer(fi).unwrap_or("ANNOTATION").into();
            e.common.color = color(g.fill_color(fi));
            add(doc, EntityType::Solid(e))?;
        }
    }
    for (ti, t) in g.texts.iter().enumerate() {
        let c = color(g.text_color(ti));
        if needs_strokes(t) {
            for st in font::text_strokes(t).into_iter().filter(|s| s.len() >= 2) {
                let mut e = LwPolyline::from_points(st.iter().map(|p| p2(*p)).collect());
                e.common.layer = g.text_layer(ti).unwrap_or("ANNOTATION").into();
                e.common.color = c;
                add(doc, EntityType::LwPolyline(e))?;
            }
            continue;
        }
        let mut e = Text::new();
        e.value = t.text.clone();
        e.height = t.height * k;
        e.rotation = t.angle;
        e.insertion_point = p3(t.pos);
        let (h, v) = match t.anchor {
            Anchor::BottomLeft => (TextHorizontalAlignment::Left, TextVerticalAlignment::Baseline),
            Anchor::BottomCenter => (TextHorizontalAlignment::Center, TextVerticalAlignment::Baseline),
            Anchor::Center => (TextHorizontalAlignment::Center, TextVerticalAlignment::Middle),
            Anchor::TopLeft => (TextHorizontalAlignment::Left, TextVerticalAlignment::Top),
        };
        if !matches!(t.anchor, Anchor::BottomLeft) {
            e.alignment_point = Some(p3(t.pos));
        }
        e.horizontal_alignment = h;
        e.vertical_alignment = v;
        e.common.layer = g.text_layer(ti).unwrap_or("ANNOTATION").into();
        e.common.color = c;
        add(doc, EntityType::Text(e))?;
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
}
