//! "Export DXF/DWG of Flat pattern" (live behaviours.md "(10) Export DXF/DWG
//! of Flat pattern", samples/qa5-sm-flat.dxf): the flat in model space, the
//! full Onshape layer table, the outline as LINEs on SHEETMETAL_CUT_LINES and
//! the bend centre lines on SHEETMETAL_BEND_LINES_UP / _DOWN (no labels).

use super::flat::FlatPattern;
use acadrust::tables::linetype::LineTypeElement;
use acadrust::{CadDocument, Circle, Color, DwgWriter, DxfVersion, DxfWriter, EntityType, Layer, Line, LineType, Vector3};

/// The export dialog's settings that change the file.
#[derive(Clone, Copy, Debug)]
pub struct FlatExport {
    /// DXF $INSUNITS of the chosen units (4 = millimeters).
    pub insunits: u8,
    pub version: DxfVersion,
    /// "Include bend centerlines" (on by default).
    pub bend_centerlines: bool,
    /// "Include bend tangent lines": each bend zone's two edges.
    pub tangent_lines: bool,
    /// "Include counterbore and countersink lines": the heads' circles.
    pub cbore_lines: bool,
    /// "Include form feature outlines" / "… centermarks".
    pub form_outlines: bool,
    pub form_centermarks: bool,
}

impl Default for FlatExport {
    fn default() -> Self {
        // Live: DXF switches the version to Release 14; only the bend
        // centerlines are on.
        Self { insunits: 4, version: DxfVersion::AC1014, bend_centerlines: true, tangent_lines: false, cbore_lines: false, form_outlines: false, form_centermarks: false }
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
const TANGENT: &str = "SHEETMETAL_BEND_TANGENT_LI";
const CBORE_UP: &str = "SHEETMETAL_CBORE_CSINK_UP";
const CBORE_DOWN: &str = "SHEETMETAL_CBORE_CSINK_DOW";
const FORM_TEAR: &str = "SHEETMETAL_FORM_TEAR";
const FORM_UP: &str = "SHEETMETAL_FORM_UP";
const FORM_DOWN: &str = "SHEETMETAL_FORM_DOWN";
const FORM_MARK: &str = "SHEETMETAL_FORM_CENTERMARK";

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
    flats_document(std::slice::from_ref(flat), o)
}

/// Flats (the export's Scope: a part, its model's parts, the Part Studio's)
/// as one CAD document, each where it lies.
pub fn flats_document(flats: &[FlatPattern], o: &FlatExport) -> Result<CadDocument, String> {
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
    for flat in flats {
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
        if o.tangent_lines {
            for b in &flat.bend_lines {
                let d = (b.b - b.a).normalize_or_zero();
                let side = glam::DVec2::new(-d.y, d.x) * (b.allowance * 0.5);
                add(&mut doc, b.a + side, b.b + side, TANGENT)?;
                add(&mut doc, b.a - side, b.b - side, TANGENT)?;
            }
        }
        if o.cbore_lines {
            for h in &flat.heads {
                let mut e = EntityType::Circle(Circle::from_center_radius(p3(h.center), h.radius * k));
                e.common_mut().layer = (if h.up { CBORE_UP } else { CBORE_DOWN }).into();
                doc.add_entity(e).map_err(|e| e.to_string())?;
            }
        }
        for f in &flat.forms {
            let layer = if f.up { FORM_UP } else { FORM_DOWN };
            for l in f.loops.iter().filter(|_| o.form_outlines) {
                for (i, a) in l.iter().enumerate() {
                    let b = l[(i + 1) % l.len()];
                    if a.distance(b) > 1e-12 {
                        add(&mut doc, *a, b, layer)?;
                    }
                }
            }
            for (l, tear) in f.curves.iter().filter(|_| o.form_outlines) {
                for w in l.windows(2) {
                    if w[0].distance(w[1]) > 1e-12 {
                        add(&mut doc, w[0], w[1], if *tear { FORM_TEAR } else { layer })?;
                    }
                }
            }
            if o.form_centermarks {
                // Live 1616 / 1623: a ±0.25 mm cross on the form's point.
                let r = 0.25e-3;
                add(&mut doc, f.center - glam::DVec2::X * r, f.center + glam::DVec2::X * r, FORM_MARK)?;
                add(&mut doc, f.center - glam::DVec2::Y * r, f.center + glam::DVec2::Y * r, FORM_MARK)?;
            }
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

/// Flats as one DXF file.
pub fn flats_dxf(flats: &[FlatPattern], o: &FlatExport) -> Result<Vec<u8>, String> {
    let doc = flats_document(flats, o)?;
    DxfWriter::new(&doc).write_to_vec().map_err(|e| e.to_string())
}

/// Flats as one DWG file.
pub fn flats_dwg(flats: &[FlatPattern], o: &FlatExport) -> Result<Vec<u8>, String> {
    let doc = flats_document(flats, o)?;
    DwgWriter::write_to_vec(&doc).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheetmetal::flat::FlatBendLine;
    use glam::DVec2;

    /// A 60 × 40 mm flat at `x` with one bend line up its middle.
    fn flat(x: f64) -> FlatPattern {
        let p = |a: f64, b: f64| DVec2::new((x + a) * 1e-3, b * 1e-3);
        FlatPattern {
            frame: noshape_kernel::Plane::xy(),
            loops: vec![vec![p(0.0, 0.0), p(60.0, 0.0), p(60.0, 40.0), p(0.0, 40.0)]],
            bend_lines: vec![FlatBendLine { a: p(30.0, 0.0), b: p(30.0, 40.0), up: true, angle: std::f64::consts::FRAC_PI_2, radius: 2e-3, k_factor: 0.45, allowance: 4e-3, bend: 0 }],
            forms: Vec::new(),
            heads: Vec::new(),
            shape: None,
            thickness: 2e-3,
            warnings: Vec::new(),
            built_for: None,
        }
    }

    /// The export's Scope over several parts writes them all into one file,
    /// each where it lies; the bend lines go with "Include bend centerlines".
    #[test]
    fn several_flats_go_into_one_file() {
        let flats = [flat(0.0), flat(100.0)];
        let doc = flats_document(&flats, &FlatExport::default()).unwrap();
        let on = |layer: &str| doc.entities().filter(|e| e.common().layer == layer).count();
        assert_eq!((on(CUT), on(BEND_UP)), (8, 2));
        let far = doc.entities().any(|e| matches!(e, EntityType::Line(l) if (l.start.x - 160.0).abs() < 1e-9 || (l.end.x - 160.0).abs() < 1e-9));
        assert!(far, "the second flat at x 100…160 mm");
        let doc = flats_document(&flats, &FlatExport { bend_centerlines: false, ..FlatExport::default() }).unwrap();
        assert_eq!(doc.entities().filter(|e| e.common().layer == BEND_UP).count(), 0);
        assert!(flats_dwg(&flats, &FlatExport { version: DxfVersion::AC1015, ..FlatExport::default() }).is_ok_and(|b| !b.is_empty()));
    }

    /// The export's other checkboxes: bend tangent lines (the bend zone's
    /// edges), counterbore / countersink heads, form outlines and marks.
    #[test]
    fn the_flat_export_options_write_their_layers() {
        let mut f = flat(0.0);
        f.heads.push(crate::sheetmetal::flat::FlatHead { center: DVec2::new(0.01, 0.02), radius: 4e-3, up: true });
        f.forms.push(crate::sheetmetal::flat::FlatForm { loops: vec![vec![DVec2::new(0.045, 0.015), DVec2::new(0.055, 0.015), DVec2::new(0.055, 0.025)]], curves: Vec::new(), center: DVec2::new(0.05, 0.02), up: false });
        let on = |o: &FlatExport, layer: &str| flats_document(std::slice::from_ref(&f), o).unwrap().entities().filter(|e| e.common().layer == layer).count();
        let none = FlatExport::default();
        let all = FlatExport { tangent_lines: true, cbore_lines: true, form_outlines: true, form_centermarks: true, ..none };
        for (layer, n) in [(TANGENT, 2), (CBORE_UP, 1), (FORM_DOWN, 3), (FORM_MARK, 2)] {
            assert_eq!((on(&none, layer), on(&all, layer)), (0, n), "{layer}");
        }
        // The tangent lines run the allowance apart, centred on the bend line.
        let doc = flats_document(std::slice::from_ref(&f), &all).unwrap();
        let mut xs: Vec<f64> = doc.entities().filter_map(|e| match e { EntityType::Line(l) if e.common().layer == TANGENT => Some(l.start.x), _ => None }).collect();
        xs.sort_by(f64::total_cmp);
        assert!((xs[0] - 28.0).abs() < 1e-9 && (xs[1] - 32.0).abs() < 1e-9, "{xs:?}");
    }
}
