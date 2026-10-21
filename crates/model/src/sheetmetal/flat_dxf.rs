//! "Export DXF/DWG of Flat pattern" (live behaviours.md "(10) Export DXF/DWG
//! of Flat pattern", samples/qa5-sm-flat.dxf): the flat in model space, the
//! full Onshape layer table, the outline as LINEs on SHEETMETAL_CUT_LINES and
//! the bend centre lines on SHEETMETAL_BEND_LINES_UP / _DOWN (no labels).

use super::flat::FlatPattern;
use acadrust::tables::linetype::LineTypeElement;
use acadrust::{CadDocument, Color, DwgWriter, DxfVersion, DxfWriter, EntityType, Layer, Line, LineType, Vector3};

/// The export dialog's settings that change the file.
#[derive(Clone, Copy, Debug)]
pub struct FlatExport {
    /// DXF $INSUNITS of the chosen units (4 = millimeters).
    pub insunits: u8,
    pub version: DxfVersion,
    /// "Include bend centerlines" (on by default).
    pub bend_centerlines: bool,
}

impl Default for FlatExport {
    fn default() -> Self {
        // Live: DXF switches the version to Release 14.
        Self { insunits: 4, version: DxfVersion::AC1014, bend_centerlines: true }
    }
}

/// Onshape's flat-export line types (live LTYPE table: name, description,
/// pattern as written).
const LINE_TYPES: [(&str, &str, &[f64]); 6] = [
    ("ONS_PHANTOM", "Onshape Phantom line ______ __ __ ______", &[0.25, -0.05, 0.05, -0.05, 0.05, -0.05]),
    ("CENTER", "Center line ____ _ ____ _ ____ _", &[12.5 / 25.4, -2.5 / 25.4, 2.5 / 25.4, -2.5 / 25.4]),
    ("DASHED", "Dashed line __ __ __ __ __", &[0.5, -0.25]),
    ("ONS_DASHDOT", "Onshape Dashed with Dot _ . ____ . ____", &[0.5, -0.1, 0.01, -0.1]),
    ("ONS_DOTTED", "Onshape Dotted line . . . . . . .", &[0.01, -0.1]),
    ("HIDDEN", "Hidden line ____ ____ ____ ____", &[0.1, -0.05]),
];

/// The layer table after "0", in live's order: name, line type, on, frozen.
const LAYERS: [(&str, &str, bool, bool); 47] = [
    ("SECTION_CUTTING_LINE", "ONS_DASHDOT", true, false),
    ("DETAIL_VIEW_BORDER", "CONTINUOUS", true, false),
    ("DETAIL_VIEW_PARENT_BORDER", "ONS_PHANTOM", true, false),
    ("TEXT", "CONTINUOUS", true, false),
    ("SKETCHED_GEOMETRY", "CONTINUOUS", false, false),
    ("VIRTUAL_SHARPS", "CONTINUOUS", false, false),
    ("CENTERLINES", "CENTER", false, false),
    ("CENTERMARKS", "CONTINUOUS", false, false),
    ("TABLES", "CONTINUOUS", true, false),
    ("IMAGES", "CONTINUOUS", false, false),
    ("ANNOTATION_LINES", "CONTINUOUS", false, false),
    ("ANNOTATION_TEXT", "CONTINUOUS", false, false),
    ("SIDE_THREAD_SYMBOLS", "CONTINUOUS", true, false),
    ("SIDE_THREAD_SYMBOLS_HIDDEN", "HIDDEN", false, false),
    ("SURFACE_THREAD_SYMBOLS", "CONTINUOUS", true, false),
    ("SURFACE_THREAD_SYMBOLS_ON_", "CONTINUOUS", true, false),
    ("SURFACE_THREAD_SYMBOLS_HID", "HIDDEN", false, false),
    ("BLIND_THREAD_SYMBOLS", "CONTINUOUS", true, false),
    ("BLIND_THREAD_SYMBOLS_HIDDE", "HIDDEN", false, false),
    ("EXPLODE_LINES", "ONS_PHANTOM", true, false),
    ("HIDDEN_EXPLODE_LINES", "ONS_PHANTOM", false, false),
    ("BREAK_LINES", "CONTINUOUS", false, false),
    (BEND_UP, "CENTER", true, false),
    (BEND_DOWN, "CENTER", true, false),
    ("SHEETMETAL_BEND_TANGENT_LI", "CONTINUOUS", true, false),
    (CUT, "CONTINUOUS", true, false),
    ("SHEETMETAL_CBORE_CSINK_UP", "CONTINUOUS", true, false),
    ("SHEETMETAL_CBORE_CSINK_DOW", "CONTINUOUS", true, false),
    ("SHEETMETAL_CBORE_CSINK_UP_", "HIDDEN", false, false),
    ("SHEETMETAL_CBORE_CSINK_DO0", "HIDDEN", false, false),
    ("SHEETMETAL_FORM_TEAR", "CONTINUOUS", true, false),
    ("SHEETMETAL_FORM_UP", "CENTER", true, false),
    ("SHEETMETAL_FORM_DOWN", "CENTER", true, false),
    ("SHEETMETAL_FORM_CENTERMARK", "CENTER", true, false),
    ("REGION_ASSOC_POINTS", "CONTINUOUS", false, false),
    ("PATTERN_CENTERLINES", "CENTER", false, true),
    ("HIDDEN", "HIDDEN", false, false),
    ("VISIBLE", "CONTINUOUS", true, false),
    ("MODELSKETCH_VISIBLE", "CONTINUOUS", true, false),
    ("MODELSKETCH_POINT", "CONTINUOUS", true, false),
    ("MODELSKETCH_CONSTRUCTION", "CENTER", false, false),
    ("HIDDEN_BASE_VIEW", "CONTINUOUS", true, false),
    ("TANGENT_LINES", "CONTINUOUS", false, false),
    ("TANGENT_HIDDEN", "HIDDEN", false, false),
    ("INVISIBLE_LINES", "CONTINUOUS", false, false),
    ("VIEW_HATCHES", "CONTINUOUS", false, false),
    ("SECTION_JOG_LINES", "CONTINUOUS", false, false),
];

const CUT: &str = "SHEETMETAL_CUT_LINES";
const BEND_UP: &str = "SHEETMETAL_BEND_LINES_UP";
const BEND_DOWN: &str = "SHEETMETAL_BEND_LINES_DOWN";

/// Units per meter of a DXF $INSUNITS code.
fn units_per_meter(insunits: u8) -> f64 {
    match insunits {
        1 => 1.0 / 0.0254,
        2 => 1.0 / 0.3048,
        5 => 100.0,
        6 => 1.0,
        10 => 1.0 / 0.9144,
        _ => 1000.0,
    }
}

/// The flat as a CAD document.
pub fn flat_document(flat: &FlatPattern, o: &FlatExport) -> Result<CadDocument, String> {
    let mut doc = CadDocument::with_version(o.version);
    doc.header.insertion_units = o.insunits as i16;
    for (name, description, pattern) in LINE_TYPES {
        let mut lt = LineType::new(name);
        lt.description = description.to_string();
        for &p in pattern {
            lt.add_element(if p > 0.0 { LineTypeElement::dash(p) } else { LineTypeElement::space(-p) });
        }
        lt.pattern_length = pattern.iter().map(|p| p.abs()).sum();
        let _ = doc.line_types.add(lt);
    }
    for (name, line_type, on, frozen) in LAYERS {
        let mut layer = Layer::new(name);
        layer.color = Color::from_index(7);
        layer.line_type = line_type.to_string();
        if !on {
            layer.turn_off();
        }
        if frozen {
            layer.freeze();
        }
        let _ = doc.layers.add(layer);
    }
    let k = units_per_meter(o.insunits);
    let p3 = |p: glam::DVec2| Vector3::new(p.x * k, p.y * k, 0.0);
    let add = |doc: &mut CadDocument, a: glam::DVec2, b: glam::DVec2, layer: &str| -> Result<(), String> {
        let mut e = EntityType::Line(Line::from_points(p3(a), p3(b)));
        e.common_mut().layer = layer.into();
        doc.add_entity(e).map(|_| ()).map_err(|e| e.to_string())
    };
    for l in &flat.loops {
        for (i, a) in l.iter().enumerate() {
            let b = l[(i + 1) % l.len()];
            if a.distance(b) > 1e-12 {
                add(&mut doc, *a, b, CUT)?;
            }
        }
    }
    if o.bend_centerlines {
        for b in &flat.bend_lines {
            add(&mut doc, b.a, b.b, if b.up { BEND_UP } else { BEND_DOWN })?;
        }
    }
    Ok(doc)
}

/// The flat as a DXF file.
pub fn flat_dxf(flat: &FlatPattern, o: &FlatExport) -> Result<Vec<u8>, String> {
    let doc = flat_document(flat, o)?;
    DxfWriter::new(&doc).write_to_vec().map_err(|e| e.to_string())
}

/// The flat as a DWG file.
pub fn flat_dwg(flat: &FlatPattern, o: &FlatExport) -> Result<Vec<u8>, String> {
    let doc = flat_document(flat, o)?;
    DwgWriter::write_to_vec(&doc).map_err(|e| e.to_string())
}
