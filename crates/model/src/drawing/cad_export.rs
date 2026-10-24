//! Drawing export to DWG / DWT and DXF (Export Drawing › Version) through
//! `acadrust`, laid out like Onshape's own exports (live
//! samples/drw2-k-dxf2018 / -dxfR14 / -dxfR12): Onshape's layers
//! (`cad_layers`) with their line types and lineweights, curves as CIRCLE /
//! ARC / ELLIPSE / SPLINE, dimensions as DIMENSION over *D blocks, feature
//! control frames as TOLERANCE, tables as ACAD_TABLE, images as OLE2FRAME,
//! filled triangles as SOLIDs, texts as MTEXT in the Onshape-Standard styles
//! (drawing symbols in Onshape's symbol font), element colors as true
//! colors; Release 11-12 keeps the view geometry only.

use super::export::aci;
use super::cad_fit::{FitCurve, fit_curve};
use super::cad_dimstyles;
use super::cad_layers::{CadLayer, CadTextStyle};
use super::cad_objects::{CadDim, CadDimKind, CadObject};
use acadrust::entities::table::Table as AcadTable;
use acadrust::entities::{Dimension, DimensionAngular3Pt, DimensionDiameter, DimensionLinear, DimensionOrdinate, DimensionRadius, Insert, Tolerance};
use acadrust::entities::{Polyline2D, PolylineFlags, Vertex2D};
use acadrust::compound_file::{BinaryRecord, CompoundEntry, CompoundFile, CompoundStorage, CompoundStream, CompoundStreamContent, StructuredStoragePayload};
use acadrust::entities::{Ole2Frame, OleFrameEnvelope, OleObjectType};
use acadrust::xdata::{ExtendedDataRecord, XDataValue};
use super::sheet::{Anchor, GeomImage, SheetGeometry};
use acadrust::entities::Solid;
use acadrust::entities::hatch::{BoundaryEdge, BoundaryPath, Hatch, HatchPattern, HatchPatternLine, HatchPatternType, LineEdge};
use acadrust::entities::mtext::AttachmentPoint;
use acadrust::tables::linetype::LineTypeElement;
use acadrust::{Arc, BlockRecord, Spline, CadDocument, Circle, Color, DimStyle, DwgWriter, DxfVersion, DxfWriter, Ellipse, EntityType, Handle, Layer, Line, LineType, LineWeight, MText, Point, TextStyle, Vector2, Vector3};

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

/// Export Drawing's DWG / DXF Version list (live dom/drw2-k-export-dialog).
pub const CAD_VERSIONS: [(&str, DxfVersion); 9] = [
    ("Release 11-12", DxfVersion::AC1009),
    ("Release 13", DxfVersion::AC1012),
    ("Release 14", DxfVersion::AC1014),
    ("2000", DxfVersion::AC1015),
    ("2004", DxfVersion::AC1018),
    ("2007", DxfVersion::AC1021),
    ("2010", DxfVersion::AC1024),
    ("2013", DxfVersion::AC1027),
    ("2018", DxfVersion::AC1032),
];
/// The Version DXF starts with (live: Release 14).
pub const DEFAULT_DXF: usize = 2;
/// The Version DWG starts with (live: 2000).
pub const DEFAULT_DWG: usize = 3;

/// Onshape's line types (live AC1032 LTYPE table; inches, scaled to the
/// file's units).
const LINE_TYPES: [(&str, &str, &[f64]); 4] = [
    ("DASHED", "Dashed line __ __ __ __ __", &[0.5, -0.25]),
    ("ONS_PHANTOM", "Onshape Phantom line ______ __ __ ______", &[0.25, -0.05, 0.05, -0.05, 0.05, -0.05]),
    ("CENTER", "Center line ____ _ ____ _ ____ _", &[0.45, -0.09, 0.09, -0.09]),
    ("HIDDEN", "Hidden line ____ ____ ____ ____", &[0.1, -0.05]),
];

/// The layer table's order (live drw2-k-dxf2018, asbu-aux: 0, Defpoints,
/// then these as used; the sheet-metal and virtual sharp layers, absent
/// from the drawing samples, last).
pub(crate) const LAYER_ORDER: [CadLayer; 25] = [
    CadLayer::Visible,
    CadLayer::TangentLines,
    CadLayer::ViewHatches,
    CadLayer::SectionJogLines,
    CadLayer::Hidden,
    CadLayer::TangentHidden,
    CadLayer::DetailViewBorder,
    CadLayer::BorderFrame,
    CadLayer::BorderZones,
    CadLayer::TitleBlock,
    CadLayer::Tables,
    CadLayer::AnnotationText,
    CadLayer::AnnotationLines,
    CadLayer::SectionCuttingLine,
    CadLayer::Centermarks,
    CadLayer::OriginIndicators,
    CadLayer::DetailViewParentBorder,
    CadLayer::SketchedGeometry,
    CadLayer::BreakLines,
    CadLayer::Centerlines,
    CadLayer::Images,
    CadLayer::VirtualSharps,
    CadLayer::BendLinesUp,
    CadLayer::BendLinesDown,
    CadLayer::BendTangentLines,
];

/// The text style of the drawing's text (live: Onshape-Standard on Noto
/// Sans; bold texts Onshape-Standard-Bold).
const TEXT_STYLES: [(&str, &str); 2] = [("Onshape-Standard", "NotoSans-Regular.ttf"), ("Onshape-Standard-Bold", "NotoSans-Bold.ttf")];
/// Dimension texts' style (live: no font file, Noto Sans in its xdata).
const DIM_TEXT_STYLE: (&str, &str) = ("Onshape-Standard-Dimension", "");
/// Hole callout texts' style.
const HOLE_CALLOUT_STYLE: (&str, &str) = ("Onshape-Standard-Hole-Callout", "NotoSans-Regular.ttf");
/// Live's text style table after "Standard" (drw2-en-ansi-a / drw2-a-iso-a3,
/// the same in every English template): name, font file, font family (the
/// ACAD xdata), and whether the style has the text height (0.12 in / 3.5 mm).
const STYLE_TABLE: [(&str, &str, &str, bool); 35] = [
    ("Onshape-Standard", "NotoSans-Regular.ttf", "Noto Sans", false),
    ("Onshape-Standard-Dimension", "", "Noto Sans", false),
    ("Onshape-Standard-Fields", "", "Noto Sans", false),
    ("Onshape-Standard-tcFields", "NotoSansTC-Regular.otf", "Noto Sans TC", false),
    ("Onshape-Standard-scFields", "NotoSansSC-Regular.otf", "Noto Sans SC", false),
    ("Onshape-Standard-KoreanFields", "NotoSansKR-Regular.otf", "Noto Sans KR", false),
    ("Onshape-Standard-JapaneseFields", "NotoSansJP-Regular.otf", "Noto Sans JP", false),
    ("Onshape-Standard-Bold", "NotoSans-Bold.ttf", "Noto Sans", false),
    ("TtfStyle", "", "Verdana", false),
    ("Onshape-Standard-GTolerance-TextStyle", "", "Noto Sans", false),
    ("Onshape-Standard-Table-DataRow", "NotoSans-Regular.ttf", "Noto Sans", false),
    ("Onshape-Standard-Table-TitleRow", "NotoSans-Bold.ttf", "Noto Sans", false),
    ("Onshape-Standard-Table-HeaderRow", "NotoSans-Bold.ttf", "Noto Sans", false),
    ("Onshape-Standard-BomTable-DataRow", "NotoSans-Regular.ttf", "Noto Sans", false),
    ("Onshape-Standard-BomTable-TitleRow", "NotoSans-Bold.ttf", "Noto Sans", false),
    ("Onshape-Standard-BomTable-HeaderRow", "NotoSans-Bold.ttf", "Noto Sans", false),
    ("Onshape-Standard-NoteStyle", "NotoSans-Regular.ttf", "Noto Sans", true),
    ("Onshape-Standard-Datum-Dimension", "NotoSans-Regular.ttf", "Noto Sans", true),
    ("Onshape-Standard-View-Label", "NotoSans-Regular.ttf", "Noto Sans", true),
    ("Onshape-Detail-View-Leader", "NotoSans-Regular.ttf", "Noto Sans", true),
    ("Onshape-Standard-Hole-Callout", "NotoSans-Regular.ttf", "Noto Sans", true),
    ("Onshape-Standard-HoleTable-DataRow", "NotoSans-Regular.ttf", "Noto Sans", false),
    ("Onshape-Standard-HoleTable-TitleRow", "NotoSans-Bold.ttf", "Noto Sans", false),
    ("Onshape-Standard-HoleTable-HeaderRow", "NotoSans-Bold.ttf", "Noto Sans", false),
    ("Onshape-Standard-Hole-Tag", "NotoSans-Regular.ttf", "Noto Sans", true),
    ("Onshape-Standard-RevisionTable-DataRow", "NotoSans-Regular.ttf", "Noto Sans", false),
    ("Onshape-Standard-RevisionTable-TitleRow", "NotoSans-Bold.ttf", "Noto Sans", false),
    ("Onshape-Standard-RevisionTable-HeaderRow", "NotoSans-Bold.ttf", "Noto Sans", false),
    ("Onshape-Standard-Revision-Marker", "NotoSans-Regular.ttf", "Noto Sans", true),
    ("Onshape-Standard-CutlistTable-DataRow", "NotoSans-Regular.ttf", "Noto Sans", false),
    ("Onshape-Standard-CutlistTable-TitleRow", "NotoSans-Bold.ttf", "Noto Sans", false),
    ("Onshape-Standard-CutlistTable-HeaderRow", "NotoSans-Bold.ttf", "Noto Sans", false),
    ("Onshape-Standard-CustomTable-DataRow", "NotoSans-Regular.ttf", "Noto Sans", false),
    ("Onshape-Standard-CustomTable-TitleRow", "NotoSans-Bold.ttf", "Noto Sans", false),
    ("Onshape-Standard-CustomTable-HeaderRow", "NotoSans-Bold.ttf", "Noto Sans", false),
];
/// Feature control frames' dimension style.
const GTOLERANCE_STYLE: &str = "Onshape-Standard-GTolerance";

fn color(c: Option<[u8; 3]>) -> Color {
    match c {
        Some([r, g, b]) => Color::from_rgb(r, g, b),
        None => Color::ByLayer,
    }
}

/// Layers of view edges: their POLYLINEs carry the continuous-linetype
/// flag (live drw2-k-dxfR14, annotation polylines don't).
const VIEW_GEOMETRY: [CadLayer; 9] = [
    CadLayer::Visible,
    CadLayer::Hidden,
    CadLayer::TangentLines,
    CadLayer::TangentHidden,
    CadLayer::SectionJogLines,
    CadLayer::DetailViewBorder,
    CadLayer::BendLinesUp,
    CadLayer::BendLinesDown,
    CadLayer::BendTangentLines,
];

/// The OLE streams of a Paintbrush object and the bytes after the frame's
/// corners, as live writes them (drw2-k-dxf2018 OLE2FRAME).
const PBRUSH_COMPOBJ: [u8; 77] = [
    0x01, 0x00, 0xFE, 0xFF, 0x03, 0x0A, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x0A, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46, 0x07, 0x00, 0x00, 0x00, b'P', b'B', b'r', b'u', b's', b'h', 0x00, 0x07, 0x00,
    0x00, 0x00, b'P', b'B', b'r', b'u', b's', b'h', 0x00, 0x07, 0x00, 0x00, 0x00, b'P', b'B', b'r', b'u', b's', b'h', 0x00, 0xF4, 0x39, 0xB2, 0x71, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const OLE_STREAM: [u8; 20] = [0x01, 0x00, 0x00, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const OLE_FRAME_TAIL: [u8; 30] = [0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0x0C, 0, 0];

/// A 24-bit bottom-up BMP of top-down RGB pixels.
fn bmp24(w: u32, h: u32, rgb: &[u8]) -> Vec<u8> {
    let row = (w as usize * 3).div_ceil(4) * 4;
    let size = 54 + row * h as usize;
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&((row * h as usize) as u32).to_le_bytes());
    // 3937 px/m (100 dpi), as live's.
    out.extend_from_slice(&3937u32.to_le_bytes());
    out.extend_from_slice(&3937u32.to_le_bytes());
    out.extend_from_slice(&[0; 8]);
    for y in (0..h as usize).rev() {
        let start = out.len();
        for x in 0..w as usize {
            let i = (y * w as usize + x) * 3;
            let px = rgb.get(i..i + 3).unwrap_or(&[255, 255, 255]);
            out.extend_from_slice(&[px[2], px[1], px[0]]);
        }
        out.resize(start + row, 0);
    }
    out
}

/// How close (sheet mm) a polyline's points must be to a circle or ellipse
/// to be written as one.
const FIT_TOL: f64 = 1e-4;

/// File units per mm for an INSUNITS code (1 inch, 2 foot, 4 mm, 5 cm, 6 m,
/// 10 yard).
pub fn units_per_mm(insunits: u8) -> f64 {
    match insunits {
        1 => 1.0 / 25.4,
        2 => 1.0 / 304.8,
        5 => 0.1,
        6 => 0.001,
        10 => 1.0 / 914.4,
        _ => 1.0,
    }
}

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
    let k = units_per_mm(insunits);
    let mut doc = document_tables(&pages.iter().collect::<Vec<_>>(), insunits, version)?;
    for (pi, g) in pages.iter().enumerate() {
        let layout = (pi > 0).then(|| format!("Sheet{}", pi + 1));
        if let Some(name) = &layout {
            doc.add_layout(name).map_err(|e| e.to_string())?;
        }
        add_sheet(&mut doc, g, k, layout.as_deref(), version, 0.0)?;
    }
    Ok(doc)
}

/// Live: the gap between two sheets' contents in model space (5.019 in).
pub const SHEET_GAP_MM: f64 = 5.019 * 25.4;

/// The x extent (sheet mm) of a sheet's content: its frame and anything
/// placed off the sheet (live lays the sheets of one file out by it).
pub fn content_x_range(g: &SheetGeometry) -> [f64; 2] {
    let mut r = [0.0f64, g.width];
    let mut add = |x: f64| {
        r[0] = r[0].min(x);
        r[1] = r[1].max(x);
    };
    for p in g.lines.iter().flat_map(|l| l.points.iter()).chain(g.fills.iter().flat_map(|f| f.points.iter())) {
        add(p[0]);
    }
    for t in &g.texts {
        for p in super::font::text_box(t) {
            add(p[0]);
        }
    }
    for im in &g.images {
        add(im.pos[0]);
        add(im.pos[0] + im.size[0]);
    }
    r
}

/// Several sheets in ONE file as live writes it (drw2 l): every sheet in
/// model space, left to right, each one's content (`[lo, hi]` sheet mm,
/// measured before off-sheet content is excluded) starting
/// [`SHEET_GAP_MM`] after the previous one's (the first at x = 0), and
/// (2000+) a layout per sheet named after it with a viewport onto its frame.
pub fn cad_document_sheets(sheets: &[(String, SheetGeometry, [f64; 2])], insunits: u8, version: DxfVersion) -> Result<CadDocument, String> {
    let k = units_per_mm(insunits);
    let mut doc = document_tables(&sheets.iter().map(|s| &s.1).collect::<Vec<_>>(), insunits, version)?;
    let mut next = 0.0;
    let mut names: Vec<String> = Vec::new();
    for (i, (name, g, [lo, hi])) in sheets.iter().enumerate() {
        let dx = next - lo;
        next = hi + dx + SHEET_GAP_MM;
        add_sheet(&mut doc, g, k, None, version, dx)?;
        // Release 13 / 14: model space only (live drw2-k-dxfR14 has no layouts).
        if version <= DxfVersion::AC1014 {
            continue;
        }
        // Layout names are unique: a repeated sheet name gets " (2)"...
        let mut layout = name.clone();
        let mut n = 1;
        while names.contains(&layout) {
            n += 1;
            layout = format!("{name} ({n})");
        }
        names.push(layout.clone());
        if i == 0 {
            rename_layout(&mut doc, "Layout1", &layout);
        } else {
            doc.add_layout(&layout).map_err(|e| e.to_string())?;
        }
        let (w, h) = (g.width * k, g.height * k);
        let mut vp = acadrust::entities::Viewport::new();
        vp.id = 2;
        vp.center = Vector3::new(w / 2.0, h / 2.0, 0.0);
        (vp.width, vp.height, vp.view_height) = (w, h, h);
        vp.view_target = Vector3::new((dx + g.width / 2.0) * k, h / 2.0, 0.0);
        doc.add_entity_to_layout(EntityType::Viewport(Box::new(vp)), &layout).map_err(|e| e.to_string())?;
    }
    Ok(doc)
}

/// Rename a layout (its object and its ACAD_LAYOUT dictionary entry).
fn rename_layout(doc: &mut CadDocument, from: &str, to: &str) {
    let dict = doc.header.acad_layout_dict_handle;
    for o in doc.objects.values_mut() {
        match o {
            acadrust::objects::ObjectType::Layout(l) if l.name == from => l.name = to.to_string(),
            acadrust::objects::ObjectType::Dictionary(d) if d.handle == dict => {
                for e in d.entries.iter_mut().filter(|e| e.0 == from) {
                    e.0 = to.to_string();
                }
            }
            _ => {}
        }
    }
}

/// Several sheets in one DXF file ([`cad_document_sheets`]; Release 11-12
/// has no layouts: the sheets side by side in model space).
pub fn to_dxf_sheets(sheets: &[(String, SheetGeometry, [f64; 2])], insunits: u8, version: DxfVersion) -> Result<Vec<u8>, String> {
    if version <= DxfVersion::AC1009 {
        return Ok(to_dxf_r12(&r12_sheets(sheets), insunits).into_bytes());
    }
    let mut doc = cad_document_sheets(sheets, insunits, version)?;
    strip_layouts_r14(&mut doc, version);
    DxfWriter::new(&doc).write_to_vec().map_err(|e| e.to_string())
}

/// Release 13 / 14 DXF has no layouts (LAYOUT objects came with 2000): live
/// drw2-k-dxfR14 has an empty ACAD_LAYOUT dictionary and no LAYOUT objects,
/// so the default "Model" / "Layout1" go and the block records point at none.
fn strip_layouts_r14(doc: &mut CadDocument, version: DxfVersion) {
    if version > DxfVersion::AC1014 {
        return;
    }
    let dict = doc.header.acad_layout_dict_handle;
    doc.objects.retain(|_, o| !matches!(o, acadrust::objects::ObjectType::Layout(_)));
    if let Some(acadrust::objects::ObjectType::Dictionary(d)) = doc.objects.get_mut(&dict) {
        d.entries.clear();
    }
    for b in doc.block_records.iter_mut() {
        b.layout = Handle::NULL;
    }
}

/// Several sheets in one DWG file, like [`to_dxf_sheets`].
pub fn to_dwg_sheets(sheets: &[(String, SheetGeometry, [f64; 2])], insunits: u8, version: DxfVersion) -> Result<Vec<u8>, String> {
    if version <= DxfVersion::AC1009 {
        return to_dwg(&r12_sheets(sheets), insunits, version);
    }
    let doc = cad_document_sheets(sheets, insunits, version)?;
    DwgWriter::write_to_vec(&doc).map_err(|e| e.to_string())
}

/// The sheets' Release 11-12 geometry ([`r12_geometry`]) side by side,
/// laid out like [`cad_document_sheets`].
fn r12_sheets(sheets: &[(String, SheetGeometry, [f64; 2])]) -> SheetGeometry {
    let first = sheets.first().map(|s| &s.1);
    let mut all = SheetGeometry::new(first.map_or(0.0, |g| g.width), first.map_or(0.0, |g| g.height));
    let mut next = 0.0;
    for (_, g, [lo, hi]) in sheets {
        let dx = next - lo;
        next = hi + dx + SHEET_GAP_MM;
        let r = r12_geometry(g);
        if all.cad_objects.is_empty() {
            all.style = r.style.clone();
        }
        all.cad_objects.extend(r.cad_objects.iter().cloned());
        for (li, l) in r.lines.iter().enumerate() {
            let i = all.lines.len();
            let mut l = l.clone();
            for p in &mut l.points {
                p[0] += dx;
            }
            all.lines.push(l);
            if let Some(c) = r.line_color(li) {
                all.colors.push(super::sheet::ColorSpan { lines: i..i + 1, texts: 0..0, fills: 0..0, rgb: c });
            }
            all.cad.push(super::cad_layers::CadSpan { lines: i..i + 1, texts: 0..0, fills: 0..0, layer: r.cad_line_layer(li) });
        }
    }
    all
}

/// The tables (line types, text and dimension styles, layers) for the
/// sheets `pages`.
fn document_tables(pages: &[&SheetGeometry], insunits: u8, version: DxfVersion) -> Result<CadDocument, String> {
    let k = units_per_mm(insunits);
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
    // The text height of the fixed-height styles: the dimension text height
    // (live 0.12 in ANSI, 3.5 mm ISO).
    let text_h = pages.first().map(|g| g.style.dim_text).unwrap_or(3.048) * k;
    for (name, font, family, sized) in STYLE_TABLE {
        let mut st = TextStyle::new(name);
        // A handle now: the dimension styles point at their text style by it.
        st.handle = doc.allocate_handle();
        st.font_file = font.to_string();
        st.true_type_font = family.to_string();
        if sized {
            (st.height, st.last_height) = (text_h, text_h);
        }
        let _ = doc.text_styles.add(st);
    }
    // R12–R14 write layer names in capitals (live drw2-k-dxfR14).
    let upper = version <= DxfVersion::AC1014;
    let dims = pages.iter().any(|g| g.cad_objects.iter().any(|o| matches!(o, CadObject::Dimension(_))));
    // Live's dimension style table (its template's, written whether or not
    // the drawing has dimensions): ANSI or ISO, the Onshape styles at the
    // drawing's dimension text height and extension.
    {
        let style = pages.first().map(|g| g.style.clone()).unwrap_or_default();
        let len = cad_dimstyles::table_mm(style.iso) * k;
        for &(name, vars) in cad_dimstyles::dim_styles(style.iso) {
            if doc.dim_styles.iter().any(|d| d.name.eq_ignore_ascii_case(name)) {
                continue;
            }
            let mut ds = DimStyle::new(name);
            for &(code, v) in vars {
                cad_dimstyles::set_dimvar(&mut ds, code, v, len);
            }
            if name.starts_with("Onshape-") {
                (ds.dimtxt, ds.dimexe) = (style.dim_text * k, style.extension * k);
            }
            // DIMTXSTY is written from the text style's handle (group 340).
            if let Some(h) = doc.text_styles.iter().find(|t| t.name == ds.dimtxsty).map(|t| t.handle) {
                ds.dimtxsty_handle = h;
            }
            let _ = doc.dim_styles.add(ds);
        }
    }
    // Layers: 0, Defpoints (with dimensions), then the ones the sheets
    // use, in live's order.
    let mut used = std::collections::BTreeSet::new();
    for g in pages {
        used.extend(used_layers(g));
    }
    if dims {
        let mut defpoints = Layer::new(if upper { "DEFPOINTS" } else { "Defpoints" });
        defpoints.color = Color::from_index(7);
        let _ = doc.layers.add(defpoints);
    }
    for l in LAYER_ORDER.iter().copied().filter(|l| used.contains(l)) {
        let mut layer = Layer::new(l.name(upper));
        layer.color = Color::from_index(7);
        layer.line_type = l.line_type().to_string();
        layer.line_weight = match l.line_weight() {
            w if w >= 0 => LineWeight::Value(w),
            _ => LineWeight::Default,
        };
        let _ = doc.layers.add(layer);
    }
    Ok(doc)
}

/// Drawing symbols (GD&T, hole and weld symbols) Noto Sans lacks.
fn is_drawing_symbol(c: char) -> bool {
    matches!(c as u32, 0x2190..=0x2BFF)
}

/// A text as MTEXT: backslashes and braces escaped, drawing symbols in
/// Onshape's symbol font (live `{\fDrawing Symbols Sans|c0;↧}.472`).
fn mtext_value(s: &str) -> String {
    let mut out = String::new();
    let mut run = String::new();
    let flush = |out: &mut String, run: &mut String| {
        if !run.is_empty() {
            out.push_str(&format!("{{\\fDrawing Symbols Sans|c0;{run}}}"));
            run.clear();
        }
    };
    for c in s.chars() {
        if is_drawing_symbol(c) {
            run.push(c);
            continue;
        }
        flush(&mut out, &mut run);
        match c {
            '\\' => out.push_str("\\\\"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            c => out.push(c),
        }
    }
    flush(&mut out, &mut run);
    out
}

/// What a text is written in: model space or a layout, a dimension's
/// block, a symbol block inserted at the point.
#[derive(Clone, Copy)]
enum TextIn {
    Space,
    Dimension,
    Symbol([f64; 2]),
    /// A table cell in its row's text style (`aligned`: with its paragraph
    /// alignment).
    Cell { style: &'static str, aligned: bool },
}

/// The MTEXT paragraph alignment code of an anchor (`\pxq?;`).
fn paragraph_align(a: Anchor) -> char {
    match a {
        Anchor::BottomCenter | Anchor::Center => 'c',
        Anchor::BottomLeft | Anchor::TopLeft => 'l',
    }
}

/// Where entities go: model space or a layout, or a block's definition.
#[derive(Clone, Copy)]
enum Owner<'a> {
    Space(Option<&'a str>),
    Block(Handle),
}

fn put(doc: &mut CadDocument, mut e: EntityType, owner: Owner) -> Result<Handle, String> {
    match owner {
        Owner::Space(Some(name)) => doc.add_entity_to_layout(e, name),
        Owner::Space(None) => doc.add_entity(e),
        Owner::Block(h) => {
            e.common_mut().owner_handle = h;
            doc.add_entity(e)
        }
    }
    .map_err(|e| e.to_string())
}

/// A new anonymous block "*<prefix><n>" (live numbers all anonymous blocks
/// in one sequence: *T1… *U9, *D10…).
fn anonymous_block(doc: &mut CadDocument, prefix: char) -> Result<(String, Handle), String> {
    let n = doc.block_records.iter().filter(|b| b.name.starts_with('*') && !b.name.eq_ignore_ascii_case("*Model_Space") && !b.name.eq_ignore_ascii_case("*Paper_Space")).count() + 1;
    let name = format!("*{prefix}{n}");
    let mut record = BlockRecord::new(&name);
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    record.flags.anonymous = true;
    let h = record.handle;
    doc.block_records.add(record)?;
    Ok((name, h))
}

/// An object's lines, texts and fills (indices).
type Members<'a> = (&'a [usize], &'a [usize], &'a [usize]);
/// A table's top-left corner, column widths, row heights and cell texts.
type TableGrid<'a> = ([f64; 2], &'a [f64], &'a [f64], &'a [Vec<String>], &'a [&'static str], bool);

/// One sheet's writing context.
struct Sheet<'a> {
    g: &'a SheetGeometry,
    /// File units per sheet mm.
    k: f64,
    /// Shift along x (sheet mm): the sheet's place in model space.
    dx: f64,
    /// R12–R14 layer names in capitals.
    upper: bool,
    version: DxfVersion,
}

impl Sheet<'_> {
    /// The drawing's Onshape dimension style `suffix` ("$0" linear, "$2"
    /// angular, "$3" diameter, "$4" radius) of its family (ANSI / ISO).
    fn dim_style(&self, suffix: &str) -> String {
        format!("{}{suffix}", cad_dimstyles::family(self.g.style.iso))
    }

    fn p2(&self, p: [f64; 2]) -> Vector2 {
        Vector2::new((p[0] + self.dx) * self.k, p[1] * self.k)
    }
    fn p3(&self, p: [f64; 2]) -> Vector3 {
        Vector3::new((p[0] + self.dx) * self.k, p[1] * self.k, 0.0)
    }

    /// A POLYLINE (live writes no LWPOLYLINE; view edges with the
    /// continuous-linetype flag, 70 = 128).
    fn polyline(&self, points: &[[f64; 2]], closed: bool, continuous: bool) -> Polyline2D {
        let mut e = Polyline2D::new();
        e.flags = PolylineFlags::from_bits(if continuous { 128 } else { 0 });
        e.flags.set_closed(closed);
        for p in points {
            e.add_vertex(Vertex2D::new(self.p3(*p)));
        }
        e
    }

    /// Line `li` as a CIRCLE / ARC / ELLIPSE when it is one (live), else a
    /// LINE or polyline. In a dimension block (`dim`) on ANNOTATION_TEXT in
    /// color 7, 0.13 mm (live *D blocks).
    fn line(&self, doc: &mut CadDocument, li: usize, owner: Owner, dim: bool) -> Result<(), String> {
        let l = &self.g.lines[li];
        if l.points.len() < 2 {
            return Ok(());
        }
        let c = match self.g.line_color(li) {
            None if dim => Color::from_index(7),
            c => color(c),
        };
        let layer = self.g.cad_line_layer(li).name(self.upper).to_string();
        let (p3, k) = (|p| self.p3(p), self.k);
        let mut e = match fit_curve(&l.points, l.closed, FIT_TOL) {
            Some(FitCurve::Circle { center, radius }) => EntityType::Circle(Circle::from_center_radius(p3(center), radius * k)),
            Some(FitCurve::Arc { center, radius, start, end }) => {
                let mut e = Arc::new();
                (e.center, e.radius, e.start_angle, e.end_angle) = (p3(center), radius * k, start, end);
                EntityType::Arc(e)
            }
            Some(FitCurve::Ellipse { center, major, ratio, start, end }) => {
                let mut e = Ellipse::from_center_axes(p3(center), Vector3::new(major[0] * k, major[1] * k, 0.0), ratio);
                (e.start_parameter, e.end_parameter) = (start, end);
                EntityType::Ellipse(e)
            }
            None if l.points.len() == 2 && !l.closed => EntityType::Line(Line::from_points(p3(l.points[0]), p3(l.points[1]))),
            None => {
                let continuous = VIEW_GEOMETRY.contains(&self.g.cad_line_layer(li));
                EntityType::Polyline2D(self.polyline(&l.points, l.closed, continuous))
            }
        };
        let common = e.common_mut();
        (common.layer, common.color) = (layer, c);
        if dim {
            common.line_weight = LineWeight::Value(13);
        }
        put(doc, e, owner).map(|_| ())
    }

    /// Fill `fi` as SOLID triangles.
    fn fill(&self, doc: &mut CadDocument, fi: usize, owner: Owner, dim: bool) -> Result<(), String> {
        let f = &self.g.fills[fi];
        if f.points.len() < 3 {
            return Ok(());
        }
        let layer = self.g.cad_fill_layer(fi).name(self.upper);
        let c = match self.g.fill_color(fi) {
            None if dim => Color::from_index(7),
            c => color(c),
        };
        for i in 1..f.points.len() - 1 {
            let (a, b, t) = (self.p3(f.points[0]), self.p3(f.points[i]), self.p3(f.points[i + 1]));
            let mut e = Solid::new(a, b, t, t);
            e.common.layer = layer.into();
            e.common.color = c;
            put(doc, EntityType::Solid(e), owner)?;
        }
        Ok(())
    }

    /// Text `ti` as MTEXT (live writes every text as MTEXT): in a dimension
    /// block in Onshape-Standard-Dimension with "\A1;" first and the CAD
    /// codes for Ø ° ±; in a symbol block (hole callout) on layer 0 in
    /// color ByBlock, in Onshape-Standard-Hole-Callout, relative to `at`.
    fn text(&self, doc: &mut CadDocument, ti: usize, owner: Owner, kind: TextIn) -> Result<(), String> {
        let t = &self.g.texts[ti];
        let c = match (self.g.text_color(ti), kind) {
            (None, TextIn::Dimension) => Color::from_index(7),
            (_, TextIn::Symbol(_)) => Color::ByBlock,
            (c, _) => color(c),
        };
        let layer = match kind {
            TextIn::Symbol(_) => "0",
            _ => self.g.cad_text_layer(ti).name(self.upper),
        };
        let mut e = MText::new();
        e.value = match kind {
            TextIn::Dimension => format!("\\A1;{}", mtext_value(t.text.trim()).replace('Ø', "%%c").replace('°', "%%d").replace('±', "%%p")),
            TextIn::Symbol(_) => format!("\\A1;{}", mtext_value(&t.text).replace('Ø', "%%c")),
            // Live notes: `{\pxql;…}` (the paragraph's alignment).
            TextIn::Space if self.g.cad_text_style(ti) == Some(CadTextStyle::Note) => format!("{{\\pxq{};{}}}", paragraph_align(t.anchor), mtext_value(&t.text)),
            TextIn::Space => mtext_value(&t.text),
            TextIn::Cell { aligned: true, .. } => format!("{{\\pxq{};{}}}", paragraph_align(t.anchor), mtext_value(&t.text)),
            TextIn::Cell { .. } => format!("{{{}}}", mtext_value(&t.text)),
        };
        e.height = t.height * self.k;
        e.rotation = t.angle;
        let at = match kind {
            TextIn::Symbol(o) => [t.pos[0] - o[0], t.pos[1] - o[1]],
            _ => t.pos,
        };
        e.insertion_point = self.p3(at);
        e.attachment_point = match t.anchor {
            Anchor::BottomLeft => AttachmentPoint::BottomLeft,
            Anchor::BottomCenter => AttachmentPoint::BottomCenter,
            Anchor::Center => AttachmentPoint::MiddleCenter,
            Anchor::TopLeft => AttachmentPoint::TopLeft,
        };
        e.style = match kind {
            TextIn::Dimension => DIM_TEXT_STYLE.0,
            TextIn::Symbol(_) => HOLE_CALLOUT_STYLE.0,
            TextIn::Cell { style, .. } => style,
            TextIn::Space => match self.g.cad_text_style(ti) {
                Some(st) => st.name(),
                None if self.g.text_font(ti).is_some_and(|f| f.bold) => TEXT_STYLES[1].0,
                None => TEXT_STYLES[0].0,
            },
        }
        .to_string();
        e.common.layer = layer.into();
        e.common.color = c;
        put(doc, EntityType::MText(Box::new(e)), owner).map(|_| ())
    }

    /// A feature control frame as TOLERANCE (live: on ANNOTATION_LINES in
    /// Onshape-Standard-GTolerance; per row symbol, tolerance 1 and 2,
    /// datums 1–3 and three more fields, split by %%v; rows by ^J).
    fn tolerance(&self, doc: &mut CadDocument, at: [f64; 2], rows: &[Vec<String>], height: f64, owner: Owner) -> Result<(), String> {
        let mut e = Tolerance::new();
        e.insertion_point = self.p3(at);
        e.direction = Vector3::new(1.0, 0.0, 0.0);
        e.text = rows
            .iter()
            .map(|r| {
                let f = |i: usize| r.get(i).map(|c| mtext_value(c)).unwrap_or_default();
                let sym = r.first().filter(|c| !c.is_empty()).map(|c| format!("{{\\fDrawing Symbols Sans;{c}}}")).unwrap_or_default();
                [sym, f(1), String::new(), f(2), f(3), f(4), String::new(), String::new(), String::new()].join("%%v")
            })
            .collect::<Vec<_>>()
            .join("^J");
        e.dimension_style_name = GTOLERANCE_STYLE.into();
        e.text_height = height * self.k;
        e.common.layer = CadLayer::AnnotationLines.name(self.upper).into();
        put(doc, EntityType::Tolerance(e), owner).map(|_| ())
    }

    /// A table: its elements in a new *T block, and over it an ACAD_TABLE
    /// (live 2018) or, before 2004, the block's INSERT (live R14).
    fn table(&self, doc: &mut CadDocument, (corner, widths, heights, cells, rows, aligned): TableGrid, members: Members, owner: Owner) -> Result<(), String> {
        let (name, block) = anonymous_block(doc, 'T')?;
        for &i in members.0 {
            self.line(doc, i, Owner::Block(block), false)?;
        }
        for &i in members.2 {
            self.fill(doc, i, Owner::Block(block), false)?;
        }
        // Each cell text in its row's style (rows top to bottom from `corner`).
        let row_of = |y: f64| {
            let mut top = corner[1];
            for (r, h) in heights.iter().enumerate() {
                if y >= top - h - 1e-6 {
                    return r;
                }
                top -= h;
            }
            heights.len().saturating_sub(1)
        };
        for &i in members.1 {
            let style = rows.get(row_of(self.g.texts[i].pos[1])).copied().unwrap_or(TEXT_STYLES[0].0);
            self.text(doc, i, Owner::Block(block), TextIn::Cell { style, aligned })?;
        }
        let layer = CadLayer::Tables.name(self.upper);
        if self.version < DxfVersion::AC1018 {
            let mut e = Insert::new(name, Vector3::new(0.0, 0.0, 0.0));
            e.common.layer = layer.into();
            return put(doc, EntityType::Insert(e), owner).map(|_| ());
        }
        let mut e = AcadTable::new(self.p3(corner), heights.len(), widths.len());
        for (i, w) in widths.iter().enumerate() {
            e.set_column_width(i, w * self.k);
        }
        for (i, h) in heights.iter().enumerate() {
            e.set_row_height(i, h * self.k);
        }
        for (r, row) in cells.iter().enumerate() {
            for (c, text) in row.iter().enumerate() {
                e.set_cell_text(r, c, &mtext_value(text));
            }
        }
        (e.block_name, e.block_record_handle) = (name, Some(block));
        e.common.layer = layer.into();
        put(doc, EntityType::Table(Box::new(e)), owner).map(|_| ())
    }

    /// An image as OLE2FRAME on IMAGES (live: an embedded "Paintbrush
    /// Picture", its BMP in the OLE object's \x01Ole10Native stream). Needs
    /// the decoded pixels (`GeomImage::rgb`).
    fn image(&self, doc: &mut CadDocument, im: &GeomImage, owner: Owner) -> Result<(), String> {
        let Some((w, h, rgb)) = &im.rgb else { return Ok(()) };
        let bmp = bmp24(*w, *h, rgb);
        let stream = |name: &str, data: Vec<u8>| CompoundEntry::Stream(CompoundStream { name: name.into(), class_id: [0; 16], state_bits: 0, creation_time: 0, modified_time: 0, content: CompoundStreamContent::Binary(BinaryRecord::split(&data, 4096)) });
        let mut native = (bmp.len() as u32).to_le_bytes().to_vec();
        native.extend_from_slice(&bmp);
        let mut root = CompoundStorage::root();
        // CLSID_PBrush {0003000A-0000-0000-C000-000000000046}.
        root.class_id = [0x0A, 0x00, 0x03, 0x00, 0, 0, 0, 0, 0xC0, 0, 0, 0, 0, 0, 0, 0x46];
        root.entries = vec![stream("\u{1}CompObj", PBRUSH_COMPOBJ.to_vec()), stream("\u{1}Ole", OLE_STREAM.to_vec()), stream("\u{1}Ole10Native", native)];
        let file = CompoundFile { root, ..Default::default() };
        let mut e = Ole2Frame::new();
        e.source_application = "Paintbrush Picture".into();
        e.upper_left_corner = self.p3([im.pos[0], im.pos[1] + im.size[1]]);
        e.lower_right_corner = self.p3([im.pos[0] + im.size[0], im.pos[1]]);
        e.ole_object_type = OleObjectType::Embedded;
        e.storage = StructuredStoragePayload { compound_file: Some(file), ..Default::default() };
        e.envelope = OleFrameEnvelope::Geometry { marker: 0x5580, extension_records: BinaryRecord::split(&OLE_FRAME_TAIL, 4096) };
        e.common.layer = CadLayer::Images.name(self.upper).into();
        put(doc, EntityType::Ole2Frame(Box::new(e)), owner).map(|_| ())
    }

    /// A closed spline (Bézier spans) as SPLINE, on line `li`'s layer.
    fn spline(&self, doc: &mut CadDocument, poles: &[[f64; 2]], li: usize, owner: Owner) -> Result<(), String> {
        let mut e = Spline::new();
        e.degree = 3;
        (e.flags.closed, e.flags.planar) = (true, true);
        e.control_points = poles.iter().map(|p| self.p3(*p)).collect();
        // Clamped (4 knots at the ends), each joint a triple knot.
        let spans = poles.len().saturating_sub(1) / 3;
        e.knots = std::iter::once(0.0).chain((0..=spans).flat_map(|i| std::iter::repeat_n(i as f64 / spans.max(1) as f64, 3))).chain(std::iter::once(1.0)).collect();
        e.normal = Vector3::new(0.0, 0.0, 1.0);
        e.common.layer = self.g.cad_line_layer(li).name(self.upper).into();
        e.common.color = color(self.g.line_color(li));
        put(doc, EntityType::Spline(e), owner).map(|_| ())
    }

    /// Texts as an anonymous *U block and its INSERT at `at` (live: a hole
    /// callout's text, the INSERT on ANNOTATION_TEXT in color 7).
    fn symbol(&self, doc: &mut CadDocument, at: [f64; 2], texts: &[usize], owner: Owner) -> Result<(), String> {
        let (name, block) = anonymous_block(doc, 'U')?;
        for &i in texts {
            self.text(doc, i, Owner::Block(block), TextIn::Symbol(at))?;
        }
        let mut e = Insert::new(name, self.p3(at));
        e.common.layer = CadLayer::AnnotationText.name(self.upper).into();
        e.common.color = Color::from_index(7);
        put(doc, EntityType::Insert(e), owner).map(|_| ())
    }

    /// A dimension: its elements in a new *D block with the definition
    /// points on Defpoints, and the DIMENSION entity (live drw2-k-dxf2018:
    /// Onshape-Standard$0 linear, $2 angular, $3 diameter, $4 radius,
    /// Onshape-OrdinateStyle; DIMLFAC the view's scale in the DSTYLE xdata).
    fn dimension(&self, doc: &mut CadDocument, d: &CadDim, members: Members, owner: Owner, upper: bool) -> Result<(), String> {
        let (name, block) = anonymous_block(doc, 'D')?;
        for &i in members.0 {
            self.line(doc, i, Owner::Block(block), true)?;
        }
        for &i in members.2 {
            self.fill(doc, i, Owner::Block(block), true)?;
        }
        // Dual (live samples/asb-dual-2018): the value and its dual in one
        // MTEXT, "\A1;[2.36 in]\P60" (the dual first unless it goes below).
        match (&d.dual, members.1) {
            (Some(du), &[i, j]) => {
                let (dual, primary) = if self.g.texts[i].text.starts_with('[') { (i, j) } else { (j, i) };
                let (top, bottom) = if du.below { (primary, dual) } else { (dual, primary) };
                let conv = |ti: usize| mtext_value(self.g.texts[ti].text.trim()).replace('Ø', "%%c").replace('°', "%%d").replace('±', "%%p");
                let t = &self.g.texts[primary];
                let mut e = MText::new();
                e.value = format!("\\A1;{}\\P{}", conv(top), conv(bottom));
                e.height = t.height * self.k;
                e.rotation = t.angle;
                e.insertion_point = self.p3(d.text_mid);
                e.attachment_point = AttachmentPoint::MiddleCenter;
                e.style = DIM_TEXT_STYLE.0.to_string();
                e.common.layer = self.g.cad_text_layer(primary).name(self.upper).into();
                e.common.color = match self.g.text_color(primary) {
                    None => Color::from_index(7),
                    c => color(c),
                };
                put(doc, EntityType::MText(Box::new(e)), Owner::Block(block))?;
            }
            _ => {
                for &i in members.1 {
                    self.text(doc, i, Owner::Block(block), TextIn::Dimension)?;
                }
            }
        }
        let p3 = |p| self.p3(p);
        let unit = self.k;
        let (mut dim, defs, style, prefix) = match &d.kind {
            CadDimKind::Linear { first, second, line, rotation } => {
                let mut e = DimensionLinear::new(p3(*first), p3(*second));
                (e.definition_point, e.rotation) = (p3(*line), *rotation);
                e.base.actual_measurement = d.measurement * unit;
                (Dimension::Linear(e), vec![*first, *second, *line, *line], self.dim_style("$0"), "")
            }
            CadDimKind::Radial { center, point } => {
                let mut e = DimensionRadius::new(p3(*center), p3(*point));
                e.base.actual_measurement = d.measurement * unit;
                (Dimension::Radius(e), vec![*center, *point], self.dim_style("$4"), "R<>")
            }
            CadDimKind::Diameter { chord, far } => {
                let mut e = DimensionDiameter::new(p3(*chord), p3(*far));
                e.base.actual_measurement = d.measurement * unit;
                (Dimension::Diameter(e), vec![*chord, *far], self.dim_style("$3"), "%%c<>")
            }
            CadDimKind::Angular { vertex, first, second, arc } => {
                let mut e = DimensionAngular3Pt::new(p3(*vertex), p3(*first), p3(*second));
                e.definition_point = p3(*arc);
                e.base.actual_measurement = d.measurement;
                (Dimension::Angular3Pt(e), vec![*first, *second, *vertex, *arc], self.dim_style("$2"), "")
            }
            CadDimKind::Ordinate { origin, feature, leader, x } => {
                let mut e = DimensionOrdinate::new(p3(*feature), p3(*leader), *x);
                e.definition_point = p3(*origin);
                (Dimension::Ordinate(e), vec![*feature, *leader], "Onshape-OrdinateStyle".to_string(), "")
            }
        };
        let defpoints = if upper { "DEFPOINTS" } else { "Defpoints" };
        for p in defs {
            let mut e = Point::new();
            e.location = p3(p);
            e.common.layer = defpoints.into();
            e.common.color = Color::ByBlock;
            put(doc, EntityType::Point(e), Owner::Block(block))?;
        }
        let base = dim.base_mut();
        base.block_name = name;
        base.style_name = style;
        base.text_middle_point = p3(d.text_mid);
        base.text_user_positioned = true;
        // Dual: the alternate units' placeholder "[]" and the value "<>".
        if let Some(du) = &d.dual {
            base.text = if du.below { "<>\\P[]".to_string() } else { "[]\\P<>".to_string() };
        }
        base.common.layer = CadLayer::AnnotationText.name(upper).into();
        // DSTYLE overrides: DIMSCALE 1, DIMLFAC (model per sheet), DIMPOST.
        let mut x = ExtendedDataRecord::new("ACAD");
        for v in [
            XDataValue::String("DSTYLE".into()),
            XDataValue::ControlString("{".into()),
            XDataValue::Integer16(40),
            XDataValue::Real(1.0),
            XDataValue::Integer16(144),
            XDataValue::Real(1.0 / d.scale.max(1e-12)),
            XDataValue::Integer16(3),
            XDataValue::String(prefix.into()),
        ] {
            x.add_value(v);
        }
        // Dual: DIMALT 1, DIMALTF, DIMALTD, DIMAPOST ("[] in").
        if let Some(du) = &d.dual {
            for v in [
                XDataValue::Integer16(170),
                XDataValue::Integer16(1),
                XDataValue::Integer16(143),
                XDataValue::Real(du.factor),
                XDataValue::Integer16(171),
                XDataValue::Integer16(du.precision as i16),
                XDataValue::Integer16(4),
                XDataValue::String(du.post.clone()),
            ] {
                x.add_value(v);
            }
        }
        x.add_value(XDataValue::ControlString("}".into()));
        base.common.extended_data.add_record(x);
        put(doc, EntityType::Dimension(Box::new(dim)), owner).map(|_| ())
    }
}

fn add_sheet(doc: &mut CadDocument, g: &SheetGeometry, k: f64, layout: Option<&str>, version: DxfVersion, dx: f64) -> Result<(), String> {
    let upper = version <= DxfVersion::AC1014;
    let sh = Sheet { g, k, dx, upper, version };
    let owner = Owner::Space(layout);
    let p2 = |p: [f64; 2]| sh.p2(p);
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
        e.common.layer = CadLayer::ViewHatches.name(upper).into();
        put(doc, EntityType::Hatch(Box::new(e)), owner)?;
    }
    // Dimensions with their elements.
    let (lo, to, fo) = g.cad_members_of();
    let of = |list: &[Option<usize>], obj: usize| -> Vec<usize> { list.iter().enumerate().filter(|(_, o)| **o == Some(obj)).map(|(i, _)| i).collect() };
    for (obj, o) in g.cad_objects.iter().enumerate() {
        let (l, t, f) = (of(&lo, obj), of(&to, obj), of(&fo, obj));
        if l.is_empty() && t.is_empty() && f.is_empty() {
            continue;
        }
        match o {
            CadObject::Dimension(d) => sh.dimension(doc, d, (&l, &t, &f), owner, upper)?,
            CadObject::Tolerance { at, rows, height } => sh.tolerance(doc, *at, rows, *height, owner)?,
            CadObject::Symbol { at } => sh.symbol(doc, *at, &t, owner)?,
            CadObject::Spline { poles } => sh.spline(doc, poles, l[0], owner)?,
            CadObject::Table { corner, widths, heights, cells, rows, aligned } => sh.table(doc, (*corner, widths, heights, cells, rows, *aligned), (&l, &t, &f), owner)?,
        }
    }
    for im in &g.images {
        sh.image(doc, im, owner)?;
    }
    for li in 0..g.lines.len() {
        if !hatch_line[li] && lo[li].is_none() {
            sh.line(doc, li, owner, false)?;
        }
    }
    for fi in (0..g.fills.len()).filter(|i| fo[*i].is_none()) {
        sh.fill(doc, fi, owner, false)?;
    }
    for ti in (0..g.texts.len()).filter(|i| to[*i].is_none()) {
        sh.text(doc, ti, owner, TextIn::Space)?;
    }
    Ok(())
}

/// What Release 11-12 keeps (live drw2-k-dxfR12: view edges and sketches
/// only; no text, dimensions, tables, hatches, format or annotations).
const R12_LAYERS: [CadLayer; 9] = [
    CadLayer::Visible,
    CadLayer::TangentLines,
    CadLayer::SectionJogLines,
    CadLayer::Hidden,
    CadLayer::TangentHidden,
    CadLayer::SketchedGeometry,
    CadLayer::BendLinesUp,
    CadLayer::BendLinesDown,
    CadLayer::BendTangentLines,
];

/// The sheet as Release 11-12 keeps it: its view edges and sketch lines.
pub fn r12_geometry(g: &SheetGeometry) -> SheetGeometry {
    let mut out = SheetGeometry::new(g.width, g.height);
    (out.style, out.cad_objects) = (g.style.clone(), g.cad_objects.clone());
    for (li, l) in g.lines.iter().enumerate() {
        let layer = g.cad_line_layer(li);
        if !R12_LAYERS.contains(&layer) {
            continue;
        }
        let i = out.lines.len();
        out.lines.push(l.clone());
        if let Some(c) = g.line_color(li) {
            out.colors.push(super::sheet::ColorSpan { lines: i..i + 1, texts: 0..0, fills: 0..0, rgb: c });
        }
        out.cad.push(super::cad_layers::CadSpan { lines: i..i + 1, texts: 0..0, fills: 0..0, layer });
    }
    out
}

/// DXF Release 11-12 (AC1009) of a drawing sheet as live writes it: the
/// view edges ([`r12_geometry`]) as LINE / CIRCLE / ARC / POLYLINE on the
/// capitalised layers 0, DEFPOINTS (with dimensions) and the used ones,
/// the line types those use.
pub fn to_dxf_r12(g: &SheetGeometry, insunits: u8) -> String {
    use std::fmt::Write;
    let k = units_per_mm(insunits);
    let g = r12_geometry(g);
    let mut s = String::new();
    let pair = |s: &mut String, code: i32, v: &str| {
        let _ = writeln!(s, "{code:>3}\n{v}");
    };
    let num = |x: f64| format!("{x}");
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in g.lines.iter().flat_map(|l| l.points.iter()) {
        for a in 0..2 {
            lo[a] = lo[a].min(p[a] * k);
            hi[a] = hi[a].max(p[a] * k);
        }
    }
    if g.lines.is_empty() {
        (lo, hi) = ([0.0; 2], [0.0; 2]);
    }
    pair(&mut s, 0, "SECTION");
    pair(&mut s, 2, "HEADER");
    pair(&mut s, 9, "$ACADVER");
    pair(&mut s, 1, "AC1009");
    pair(&mut s, 9, "$DWGCODEPAGE");
    pair(&mut s, 3, "ansi_1252");
    pair(&mut s, 9, "$INSBASE");
    for (c, v) in [(10, 0.0), (20, 0.0), (30, 0.0)] {
        pair(&mut s, c, &num(v));
    }
    for (name, p) in [("$EXTMIN", lo), ("$EXTMAX", hi)] {
        pair(&mut s, 9, name);
        pair(&mut s, 10, &num(p[0]));
        pair(&mut s, 20, &num(p[1]));
        pair(&mut s, 30, "0.0");
    }
    pair(&mut s, 0, "ENDSEC");
    pair(&mut s, 0, "SECTION");
    pair(&mut s, 2, "TABLES");
    // Layers and the line types they use (live order; DASHED always).
    let used: std::collections::BTreeSet<CadLayer> = (0..g.lines.len()).map(|i| g.cad_line_layer(i)).collect();
    let layers: Vec<CadLayer> = LAYER_ORDER.iter().copied().filter(|l| used.contains(l)).collect();
    let line_types: Vec<(&str, &str, &[f64])> = std::iter::once(("CONTINUOUS", "Solid line", &[][..])).chain(LINE_TYPES.iter().copied().filter(|(name, _, _)| *name == "DASHED" || layers.iter().any(|l| l.line_type() == *name))).collect();
    pair(&mut s, 0, "TABLE");
    pair(&mut s, 2, "LTYPE");
    pair(&mut s, 70, &line_types.len().to_string());
    for (name, description, pattern) in &line_types {
        pair(&mut s, 0, "LTYPE");
        pair(&mut s, 2, name);
        pair(&mut s, 70, "0");
        pair(&mut s, 3, description);
        pair(&mut s, 72, "65");
        pair(&mut s, 73, &pattern.len().to_string());
        let to_units = |inch: f64| inch * 25.4 * k;
        pair(&mut s, 40, &num(pattern.iter().map(|x| to_units(x.abs())).sum()));
        for p in pattern.iter() {
            pair(&mut s, 49, &num(to_units(*p)));
        }
    }
    pair(&mut s, 0, "ENDTAB");
    let mut names: Vec<(&str, &str)> = vec![("0", "CONTINUOUS")];
    if !g.cad_objects.is_empty() {
        names.push(("DEFPOINTS", "CONTINUOUS"));
    }
    names.extend(layers.iter().map(|l| (l.name(true), if l.line_type() == "Continuous" { "CONTINUOUS" } else { l.line_type() })));
    pair(&mut s, 0, "TABLE");
    pair(&mut s, 2, "LAYER");
    pair(&mut s, 70, &names.len().to_string());
    for (name, lt) in names {
        pair(&mut s, 0, "LAYER");
        pair(&mut s, 2, name);
        pair(&mut s, 70, "0");
        pair(&mut s, 62, "7");
        pair(&mut s, 6, lt);
    }
    pair(&mut s, 0, "ENDTAB");
    pair(&mut s, 0, "TABLE");
    pair(&mut s, 2, "STYLE");
    pair(&mut s, 70, "2");
    for (name, font) in [("STANDARD", "txt"), ("ONSHAPE-STANDARD", TEXT_STYLES[0].1)] {
        pair(&mut s, 0, "STYLE");
        pair(&mut s, 2, name);
        pair(&mut s, 70, "0");
        pair(&mut s, 40, "0.0");
        pair(&mut s, 41, "1.0");
        pair(&mut s, 50, "0.0");
        pair(&mut s, 71, "0");
        pair(&mut s, 42, &num(3.048 * k));
        pair(&mut s, 3, font);
        pair(&mut s, 4, "");
    }
    pair(&mut s, 0, "ENDTAB");
    pair(&mut s, 0, "ENDSEC");
    pair(&mut s, 0, "SECTION");
    pair(&mut s, 2, "BLOCKS");
    pair(&mut s, 0, "ENDSEC");
    pair(&mut s, 0, "SECTION");
    pair(&mut s, 2, "ENTITIES");
    let xy = |s: &mut String, c: i32, p: [f64; 2]| {
        pair(s, c, &num(p[0] * k));
        pair(s, c + 10, &num(p[1] * k));
        pair(s, c + 20, "0.0");
    };
    for (li, l) in g.lines.iter().enumerate() {
        if l.points.len() < 2 {
            continue;
        }
        let layer = g.cad_line_layer(li).name(true);
        let color = g.line_color(li).map(|c| aci(Some(c)).to_string());
        let head = |s: &mut String, kind: &str| {
            pair(s, 0, kind);
            pair(s, 8, layer);
            if let Some(c) = &color {
                pair(s, 62, c);
            }
        };
        match fit_curve(&l.points, l.closed, FIT_TOL) {
            Some(FitCurve::Circle { center, radius }) => {
                head(&mut s, "CIRCLE");
                xy(&mut s, 10, center);
                pair(&mut s, 40, &num(radius * k));
            }
            Some(FitCurve::Arc { center, radius, start, end }) => {
                head(&mut s, "ARC");
                xy(&mut s, 10, center);
                pair(&mut s, 40, &num(radius * k));
                pair(&mut s, 50, &num(start.to_degrees()));
                pair(&mut s, 51, &num(end.to_degrees()));
            }
            _ if l.points.len() == 2 && !l.closed => {
                head(&mut s, "LINE");
                xy(&mut s, 10, l.points[0]);
                xy(&mut s, 11, l.points[1]);
            }
            _ => {
                // Ellipses and splines too (R12 has neither).
                head(&mut s, "POLYLINE");
                pair(&mut s, 66, "1");
                xy(&mut s, 10, [0.0, 0.0]);
                pair(&mut s, 70, if l.closed { "1" } else { "0" });
                for p in &l.points {
                    pair(&mut s, 0, "VERTEX");
                    pair(&mut s, 8, layer);
                    xy(&mut s, 10, *p);
                }
                pair(&mut s, 0, "SEQEND");
                pair(&mut s, 8, layer);
            }
        }
    }
    pair(&mut s, 0, "ENDSEC");
    pair(&mut s, 0, "EOF");
    s
}

/// A DXF file of `version` (Release 11-12 is [`to_dxf_r12`]).
pub fn to_dxf_file(g: &SheetGeometry, insunits: u8, version: DxfVersion) -> Result<Vec<u8>, String> {
    if version <= DxfVersion::AC1009 {
        return Ok(to_dxf_r12(g, insunits).into_bytes());
    }
    to_dxf_version(g, insunits, version)
}

/// A DWG file of `version` (Release 11-12 keeps the view geometry only).
pub fn to_dwg_file(g: &SheetGeometry, insunits: u8, version: DxfVersion) -> Result<Vec<u8>, String> {
    if version <= DxfVersion::AC1009 {
        return to_dwg(&r12_geometry(g), insunits, version);
    }
    to_dwg(g, insunits, version)
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
    let mut doc = cad_document(g, insunits, version)?;
    strip_layouts_r14(&mut doc, version);
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

    /// drw2 l (live drw2-l-3sheets.dxf): three sheets in one file, Sheet2
    /// with content 5 in left of its frame: the sheets side by side in model
    /// space, each content box 5.019 in after the previous one, a layout per
    /// sheet named after it whose viewport looks at that sheet.
    #[test]
    fn several_sheets_go_in_one_file_like_live() {
        let mut b = sample();
        b.line([-127.0, 100.0], [-120.0, 100.0], LineStyle::Visible);
        let sheets: Vec<(String, SheetGeometry, [f64; 2])> = [("Sheet1", sample()), ("Sheet2", b), ("Sheet3", sample())]
            .into_iter()
            .map(|(n, g)| {
                let r = content_x_range(&g);
                (n.to_string(), g, r)
            })
            .collect();
        let doc = cad_document_sheets(&sheets, 1, DxfVersion::AC1032).unwrap();
        let mut layouts: Vec<String> = doc.objects.values().filter_map(|o| if let acadrust::objects::ObjectType::Layout(l) = o { Some(l.name.clone()) } else { None }).collect();
        layouts.sort();
        assert_eq!(layouts, ["Model", "Sheet1", "Sheet2", "Sheet3"]);
        let dxf = String::from_utf8(to_dxf_sheets(&sheets, 1, DxfVersion::AC1032).unwrap()).unwrap();
        let lines: Vec<&str> = dxf.lines().map(str::trim).collect();
        let (mut targets, mut in_vp) = (Vec::new(), false);
        for pair in lines.chunks(2).filter(|p| p.len() == 2) {
            if pair[0] == "100" {
                in_vp = pair[1] == "AcDbViewport";
            }
            if in_vp && pair[0] == "17" {
                targets.push(pair[1].parse::<f64>().unwrap());
            }
        }
        targets.retain(|t| *t > 1.0);
        targets.sort_by(f64::total_cmp);
        let w = sheets[0].1.width / 25.4;
        let left2 = w + 5.019 + 5.0;
        let left3 = left2 + w + 5.019;
        let want = [w / 2.0, left2 + w / 2.0, left3 + w / 2.0];
        assert_eq!(targets.len(), 3, "{targets:?}");
        for (t, e) in targets.iter().zip(want) {
            assert!((t - e).abs() < 1e-6, "{targets:?} vs {want:?}");
        }
        // Release 14: model space only (no layouts); Release 11-12 likewise.
        // Live drw2-k-dxfR14: no LAYOUT objects at all (not even Model / Layout1).
        let r14 = String::from_utf8(to_dxf_sheets(&sheets, 1, DxfVersion::AC1014).unwrap()).unwrap();
        let lines: Vec<&str> = r14.lines().map(str::trim).collect();
        let objects = lines.chunks(2).filter(|p| p.len() == 2 && p[0] == "0").map(|p| p[1]).collect::<Vec<_>>();
        assert!(r14.contains("AC1014") && !objects.contains(&"LAYOUT"), "{objects:?}");
        assert!(!lines.contains(&"Layout1") && !lines.contains(&"Sheet1"));
        let back = acadrust::DxfReader::from_reader(std::io::Cursor::new(r14.into_bytes())).unwrap().read().unwrap();
        assert!(back.entities().count() >= 3 * sheets[0].1.lines.len());
        let r12 = String::from_utf8(to_dxf_sheets(&sheets, 1, DxfVersion::AC1009).unwrap()).unwrap();
        assert!(r12.contains("AC1009") && !r12.contains("AcDbViewport"));
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
        assert_eq!(layer("Hidden").unwrap().line_type, "HIDDEN");
        // Defpoints only with dimensions (live drw2-de-ansi-a has none).
        assert!(layer("CENTERMARKS").is_some() && layer("Defpoints").is_none());
        assert!(layer("VISIBLE").is_none() && layer("HIDDEN").is_none() && layer("ANNOTATION").is_none(), "no old layer names");
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

    /// Live drw2-k-dxf2018: a table is an ACAD_TABLE over a *T block (an
    /// INSERT in R14), a feature control frame a TOLERANCE, a broken-out
    /// profile a SPLINE, an image an OLE2FRAME (Paintbrush Picture).
    #[test]
    fn tables_tolerances_splines_and_images_like_onshape() {
        use crate::drawing::cad_objects::{CadMember, catmull_rom_poles};
        let mut g = SheetGeometry::new(279.4, 215.9);
        let m = g.mark();
        for y in [100.0, 92.0, 84.0] {
            g.line([10.0, y], [60.0, y], LineStyle::Thin);
        }
        g.texts.push(crate::drawing::sheet::Text2 { pos: [12.0, 94.0], height: 3.0, text: "Item".into(), angle: 0.0, anchor: Anchor::BottomLeft, pick: 0 });
        g.cad_since(m, CadLayer::Tables);
        g.cad_objects.push(CadObject::Table { corner: [10.0, 100.0], widths: vec![50.0], heights: vec![8.0, 8.0], cells: vec![vec!["Item".into()], vec!["1".into()]], rows: vec!["Onshape-Standard-BomTable-HeaderRow", "Onshape-Standard-BomTable-DataRow"], aligned: false });
        g.cad_members.push(CadMember { lines: m.0..g.lines.len(), texts: m.1..g.texts.len(), fills: 0..0, obj: 0 });
        let l = g.lines.len();
        g.lines.push(crate::drawing::sheet::Polyline2 { points: vec![[100.0, 100.0], [110.0, 100.0], [110.0, 107.0], [100.0, 107.0]], style: LineStyle::Thin, closed: true, pick: 0 });
        g.texts.push(crate::drawing::sheet::Text2 { pos: [105.0, 103.5], height: 3.0, text: "⌖".into(), angle: 0.0, anchor: Anchor::Center, pick: 0 });
        g.cad_objects.push(CadObject::Tolerance { at: [100.0, 103.5], rows: vec![vec!["⌖".into(), "0.1".into(), "A".into()]], height: 3.0 });
        g.cad_members.push(CadMember { lines: l..l + 1, texts: g.texts.len() - 1..g.texts.len(), fills: 0..0, obj: 1 });
        let profile = [[150.0, 50.0], [170.0, 50.0], [170.0, 70.0], [150.0, 70.0]];
        let m = g.mark();
        g.lines.push(crate::drawing::sheet::Polyline2 { points: crate::drawing::views::closed_spline(&profile, 16), style: LineStyle::Visible, closed: true, pick: 0 });
        g.cad_since(m, CadLayer::SectionJogLines);
        g.add_cad_spline(m.0, catmull_rom_poles(&profile));
        g.images.push(crate::drawing::sheet::GeomImage { pos: [200.0, 20.0], size: [4.0, 2.0], data_base64: "".into(), rgb: Some((2, 1, std::sync::Arc::new(vec![255, 0, 0, 0, 0, 255]))) });
        let doc = acadrust::DxfReader::from_reader(std::io::Cursor::new(to_dxf_version(&g, 1, DxfVersion::AC1032).unwrap())).unwrap().read().unwrap();
        let kinds: Vec<&str> = doc.entities().map(|e| e.as_entity().entity_type()).collect();
        for k in ["ACAD_TABLE", "TOLERANCE", "SPLINE", "OLE2FRAME"] {
            assert!(kinds.contains(&k), "{k} in {kinds:?}");
        }
        assert!(!kinds.contains(&"LWPOLYLINE"), "{kinds:?}");
        assert!(doc.entities().any(|e| matches!(e, EntityType::Spline(s) if s.common.layer == "SECTION_JOG_LINES" && s.control_points.len() == 13)));
        assert!(doc.entities().any(|e| matches!(e, EntityType::Tolerance(t) if t.text.contains("0.1%%v%%vA") && t.common.layer == "ANNOTATION_LINES")));
        let r14 = acadrust::DxfReader::from_reader(std::io::Cursor::new(to_dxf_version(&g, 1, DxfVersion::AC1014).unwrap())).unwrap().read().unwrap();
        assert!(r14.entities().any(|e| matches!(e, EntityType::Insert(i) if i.block_name.starts_with("*T") && i.common.layer == "TABLES")));
        assert!(!r14.entities().any(|e| matches!(e, EntityType::Table(_))));
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

    /// Live drw2-en-ansi-a / drw2-a-iso-a3-noborder: the template's whole
    /// DIMSTYLE table (21 ANSI, 23 ISO records in live's order; the
    /// Onshape-ISO-25 family in ISO drawings), lengths in the export units.
    #[test]
    fn dimension_styles_like_onshape() {
        let read = |g: &SheetGeometry, units: u8| {
            let dxf = to_dxf_version(g, units, DxfVersion::AC1032).unwrap();
            acadrust::DxfReader::from_reader(std::io::Cursor::new(dxf)).unwrap().read().unwrap()
        };
        let mut g = sample();
        let d = read(&g, 1);
        let names: Vec<String> = d.dim_styles.iter().map(|s| s.name.clone()).collect();
        assert_eq!(names.len(), 21, "{names:?}");
        for n in ["Standard", "Onshape-Standard", "ANSI$SH", "Onshape-Standard$0", "Onshape-Standard$7", "Onshape-Standard-GTolerance", "Onshape-OrdinateStyle", "Onshape-ChamferDimStyle", "Onshape-SketchDrivenDimStyle$4"] {
            assert!(names.iter().any(|x| x == n), "{n} {names:?}");
        }
        let st = |d: &CadDocument, n: &str| d.dim_styles.iter().find(|s| s.name == n).cloned().unwrap();
        // Inches (INSUNITS 1): live's values as they are, the text height the drawing's.
        let ord = st(&d, "Onshape-OrdinateStyle");
        assert!((ord.dimasz - 0.02).abs() < 1e-9 && (ord.dimtxt - g.style.dim_text / 25.4).abs() < 1e-9, "{} {}", ord.dimasz, ord.dimtxt);
        assert_eq!((ord.dimdec, ord.dimclrd, ord.dimlwd), (3, 7, 13));
        assert_eq!(st(&d, "Onshape-SketchDrivenDimStyle$0").dimclrt, 251);
        assert_eq!(st(&d, "Onshape-Standard-GTolerance").dimtxsty, "Onshape-Standard-GTolerance-TextStyle");
        // ISO, millimetres: the Onshape-ISO-25 family, the dimensions in it.
        g.style.iso = true;
        let d = read(&g, 4);
        let names: Vec<String> = d.dim_styles.iter().map(|s| s.name.clone()).collect();
        assert_eq!(names.len(), 23, "{names:?}");
        assert!(names.iter().any(|x| x == "Onshape-ISO-25$3") && !names.iter().any(|x| x.starts_with("Onshape-Standard$")), "{names:?}");
        let iso = st(&d, "Onshape-ISO-25$0");
        assert!((iso.dimasz - 2.5).abs() < 1e-9 && (iso.dimgap - 0.625).abs() < 1e-9 && iso.dimdec == 2, "{} {} {}", iso.dimasz, iso.dimgap, iso.dimdec);
    }
}
