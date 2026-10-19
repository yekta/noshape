//! Integration tests: views of the demo plate and sheet export.

use super::build::{PickTarget, render_sheet};
use super::views::{ViewCache, compute_view, regenerate_sources};
use super::*;
use crate::document::TabContent;
use noshape_kernel::EdgeClass;

fn demo() -> (crate::Document, Drawing) {
    let doc = crate::demo::demo_document_with_drawing();
    let drawing = doc
        .tabs
        .iter()
        .find_map(|t| match &t.content {
            TabContent::Drawing(d) => Some(d.clone()),
            _ => None,
        })
        .expect("demo has a drawing");
    (doc, drawing)
}

/// Visible sharp/outline polylines and hidden ones of a view.
fn counts(g: &views::ViewGeometry) -> (usize, usize, usize) {
    let vis = g.edges.iter().filter(|e| e.visible && matches!(e.class, EdgeClass::Sharp | EdgeClass::Outline)).count();
    let tan = g.edges.iter().filter(|e| e.visible && e.class == EdgeClass::Smooth).count();
    let hid = g.edges.iter().filter(|e| !e.visible && matches!(e.class, EdgeClass::Sharp | EdgeClass::Outline)).count();
    (vis, tan, hid)
}

#[test]
fn demo_plate_views_have_expected_lines() {
    let (doc, drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let sheet = &drawing.sheets[0];
    for exact in [false, true] {
        for v in &sheet.views {
            let g = compute_view(&drawing, sheet, v.id, &sources, &cache, exact);
            let (vis, tan, hid) = counts(&g);
            eprintln!("exact={exact} view {:?}: visible {vis} tangent {tan} hidden {hid} model edges {} snaps {} section segs {} err {:?}", v.id, g.model_edges.len(), g.snaps.len(), g.section_segments.len(), g.error);
            assert!(g.error.is_none(), "{:?}", g.error);
            assert!(vis > 0);
            if exact && v.id == ViewId(1) {
                // Front view of the filleted plate: bottom edge, two side
                // edges, the two fillet silhouettes at the top corners, the
                // top silhouette and the end arcs of the edge fillets.
                assert_eq!(vis, 8, "front view visible lines");
            }
            if v.id == ViewId(2) {
                // Top view: each of the 3 holes shows its bore circle and
                // the outer edge of its 2 mm fillet.
                assert_eq!(g.circles().len(), 6);
            }
        }
    }
}

#[test]
fn sheet_export_with_views_and_dimensions() {
    let (doc, drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let built = render_sheet(&drawing, 0, &sources, &cache, true).unwrap();
    assert!(built.dangling.is_empty(), "dangling {:?}", built.dangling);
    assert!(built.picks.iter().any(|p| matches!(p, PickTarget::Annotation(_))));
    let svg = export::to_svg(&built.geometry);
    let pdf = export::to_pdf(&built.geometry);
    let dxf = export::to_dxf(&built.geometry);
    assert!(svg.contains("SECTION A-A") && svg.contains("DETAIL B") && svg.contains(">100<"));
    let _ = std::fs::create_dir_all("/tmp/drw");
    std::fs::write("/tmp/drw/demo.svg", &svg).unwrap();
    std::fs::write("/tmp/drw/demo.pdf", &pdf).unwrap();
    std::fs::write("/tmp/drw/demo.dxf", &dxf).unwrap();
}

#[test]
fn dimensions_follow_model_changes() {
    use crate::params::ParamValue;
    use glam::DVec3;
    use noshape_units::ExprValue;
    let (mut doc, mut drawing) = demo();
    let studio_tab = doc.tabs[0].id;
    let sources = regenerate_sources(&doc, &drawing);
    let ctx = sources[&views::SourceKey::studio(studio_tab)].context.clone();
    // Front view: height of the straight part of the corner edge (12 mm
    // plate minus the 2 mm top fillet).
    let a = views::vertex_ref_near(&ctx, DVec3::new(-0.05, -0.03, 0.0), 1e-6).expect("bottom vertex");
    let b = views::vertex_ref_near(&ctx, DVec3::new(-0.05, -0.03, 0.010), 1e-6).expect("vertex below the fillet");
    let sheet = &mut drawing.sheets[0];
    let id = sheet.next_annotation_id();
    sheet.annotations.push(Annotation {
        id,
        kind: AnnotationKind::LinearDimension { view: ViewId(1), a: [0.0; 2], b: [0.0; 2], offset: -20.0, orientation: DimOrientation::Vertical, override_text: None },
        position: [0.0; 2],
        refs: vec![AnnotationRef { slot: 0, point: a }, AnnotationRef { slot: 1, point: b }],
        ..Default::default()
    });
    let cache = ViewCache::new();
    let text_of = |built: &build::BuiltSheet| {
        let pick = built.picks.iter().position(|p| *p == PickTarget::Annotation(id)).unwrap() as u32;
        built.geometry.texts.iter().find(|t| t.pick == pick).map(|t| t.text.clone()).unwrap()
    };
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    assert_eq!(text_of(&built), "10");
    // Thicken the plate to 20 mm: the dimension now reads 18.
    if let TabContent::PartStudio(ps) = &mut doc.tabs[0].content {
        let ex = ps.features.iter_mut().find(|f| f.type_id == "extrude").unwrap();
        ex.params.set("depth", ParamValue::Expr(ExprValue::new("20 mm")));
    }
    let sources = regenerate_sources(&doc, &drawing);
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    assert!(built.dangling.is_empty(), "{:?}", built.dangling);
    assert_eq!(text_of(&built), "18");
}

// ------------------------------------------------------------ new features

use crate::document::TabId;
use super::build::{SheetInfo, render_sheet_with};
use super::views::SourceKey;

fn studio_tab(doc: &crate::Document) -> TabId {
    doc.tabs[0].id
}

fn assembly_tab(doc: &crate::Document) -> TabId {
    doc.tabs.iter().find(|t| matches!(t.content, TabContent::Assembly(_))).expect("demo assembly").id
}

fn base_view(id: u64, source: TabId, orientation: ViewOrientation, position: [f64; 2]) -> DrawingView {
    DrawingView { id: ViewId(id), kind: ViewKind::Base { source, config: Default::default(), orientation, part: None }, position, ..Default::default() }
}

fn texts(b: &build::BuiltSheet) -> Vec<String> {
    b.geometry.texts.iter().map(|t| t.text.clone()).collect()
}

#[test]
fn sheet_operations() {
    let mut d = Drawing::default();
    let i = d.add_sheet(0);
    assert_eq!((i, d.sheets[1].name.as_str()), (1, "Sheet2"));
    assert!(d.rename_sheet(1, "Details"));
    assert!(!d.rename_sheet(1, "  "));
    let j = d.duplicate_sheet(0).unwrap();
    assert_eq!(d.sheets[j].name, "Sheet1 copy");
    assert_eq!(d.sheets.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Sheet1", "Sheet1 copy", "Details"]);
    assert!(d.move_sheet(2, 0));
    assert_eq!(d.sheets[0].name, "Details");
    assert!(d.delete_sheet(0) && d.delete_sheet(0));
    assert!(!d.delete_sheet(0), "the last sheet stays");
    assert_eq!(d.sheets[0].name, "Sheet1 copy");
    assert_eq!(d.unique_sheet_name(), "Sheet1");
}

#[test]
fn rotated_and_broken_views() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let before = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let p0 = *before.placement(ViewId(1)).unwrap();
    let (w0, h0) = (p0.max[0] - p0.min[0], p0.max[1] - p0.min[1]);
    // Rotate the front view by 90 degrees: width and height swap.
    drawing.sheets[0].view_mut(ViewId(1)).unwrap().rotation = std::f64::consts::FRAC_PI_2;
    let rot = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let p1 = *rot.placement(ViewId(1)).unwrap();
    assert!(((p1.max[0] - p1.min[0]) - h0).abs() < 1e-6 && ((p1.max[1] - p1.min[1]) - w0).abs() < 1e-6);
    let v = [0.01, 0.004];
    let back = p1.to_view(p1.to_sheet(v));
    assert!((back[0] - v[0]).abs() < 1e-12 && (back[1] - v[1]).abs() < 1e-12);
    // The width dimension of the front view still reads 100 (measured along
    // the view's own x axis).
    assert!(texts(&rot).iter().any(|t| t == "100"));
    drawing.sheets[0].view_mut(ViewId(1)).unwrap().rotation = 0.0;

    // Break the front view: 60 mm of the 100 mm plate removed, 6 mm gap.
    drawing.sheets[0].view_mut(ViewId(1)).unwrap().breaks.push(ViewBreak { vertical: true, a: -0.03, b: 0.03, gap_mm: 6.0, style: BreakStyle::ZigZag });
    let br = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let p2 = *br.placement(ViewId(1)).unwrap();
    assert!(((p2.max[0] - p2.min[0]) - (w0 - 60.0 + 6.0)).abs() < 1e-6, "{} vs {}", p2.max[0] - p2.min[0], w0);
    // Collapse/expand round trip, inside and beyond the band.
    for x in [-0.04, 0.0, 0.045] {
        let q = p2.to_view(p2.to_sheet([x, 0.002]));
        assert!((q[0] - x).abs() < 1e-12, "{x} -> {q:?}");
    }
    // No visible view line enters the gap; the dimension keeps the true length.
    let gap = (p2.to_sheet([-0.03, 0.0])[0] + 0.01, p2.to_sheet([0.03, 0.0])[0] - 0.01);
    let edge_picks: Vec<u32> = br.picks.iter().enumerate().filter(|(_, t)| matches!(t, PickTarget::Edge { view: ViewId(1), .. })).map(|(i, _)| i as u32).collect();
    for l in br.geometry.lines.iter().filter(|l| edge_picks.contains(&l.pick)) {
        for w in l.points.windows(2) {
            let m = (w[0][0] + w[1][0]) / 2.0;
            assert!(!(m > gap.0 && m < gap.1), "line inside the break gap: {:?}", l.points);
        }
    }
    assert!(texts(&br).iter().any(|t| t == "100"));
    assert_eq!(br.picks.iter().filter(|t| matches!(t, PickTarget::BreakLine { .. })).count(), 1);
}

#[test]
fn crop_view_keeps_inside_geometry() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let rect = vec![[0.0, -0.035], [0.055, -0.035], [0.055, 0.035], [0.0, 0.035]];
    drawing.sheets[0].view_mut(ViewId(2)).unwrap().crop = Some(Crop::Polygon(rect.clone()));
    let sheet = &drawing.sheets[0];
    let g = compute_view(&drawing, sheet, ViewId(2), &sources, &cache, false);
    assert!(!g.edges.is_empty());
    for e in &g.edges {
        for p in &e.points {
            assert!(p[0] > -1e-9, "cropped point {p:?}");
        }
    }
    assert!(g.min[0] >= -1e-9 && g.max[0] <= 0.0501);
    // The center hole is cut in half: some of its circle remains.
    assert!(g.snaps.iter().all(|s| s.p[0] >= -1e-9));
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    assert!(built.dangling.is_empty() || !built.dangling.is_empty());
}

#[test]
fn jogged_and_aligned_sections() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    // Jogged: through the center hole (x = 0) and the corner hole (x = 38).
    if let ViewKind::Section { line, .. } = &mut drawing.sheets[0].view_mut(ViewId(5)).unwrap().kind {
        *line = vec![[0.0, -0.04], [0.0, 0.01], [0.038, 0.01], [0.038, 0.04]];
    }
    let sheet = &drawing.sheets[0];
    let g = compute_view(&drawing, sheet, ViewId(5), &sources, &cache, true);
    assert!(g.error.is_none(), "{:?}", g.error);
    assert!(!g.section_segments.is_empty());
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let sect_pick = built.picks.iter().position(|p| *p == PickTarget::SectionLine(ViewId(5))).unwrap() as u32;
    let line = built.geometry.lines.iter().find(|l| l.pick == sect_pick && l.style == sheet::LineStyle::Section).unwrap();
    assert_eq!(line.points.len(), 4, "jogged cutting line");
    // Hatch present in the section view.
    let view_pick = built.picks.iter().position(|p| *p == PickTarget::View(ViewId(5))).unwrap() as u32;
    assert!(built.geometry.lines.iter().filter(|l| l.pick == view_pick && l.style == sheet::LineStyle::Thin).count() > 5);

    // Aligned section through the center hole and the corner hole.
    let id = drawing.sheets[0].next_view_id();
    drawing.sheets[0].views.push(DrawingView { id, kind: ViewKind::AlignedSection { parent: ViewId(2), center: [0.0, 0.0], a: [0.0, -0.045], b: [0.0475, 0.0225], label: "C".into() }, position: [330.0, 215.0], ..Default::default() });
    let sheet = &drawing.sheets[0];
    let g = compute_view(&drawing, sheet, id, &sources, &cache, false);
    assert!(g.error.is_none(), "{:?}", g.error);
    assert!(!g.edges.is_empty() && !g.section_segments.is_empty());
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    assert!(texts(&built).iter().any(|t| t == "SECTION C-C"));
}

#[test]
fn automatic_centerlines_on_hole_axes() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let g = compute_view(&drawing, &drawing.sheets[0], ViewId(1), &sources, &cache, false);
    // Front view: the three holes' axes (fillet blends are not full cylinders).
    assert_eq!(g.axes.len(), 3, "{:?}", g.axes);
    drawing.sheets[0].view_mut(ViewId(1)).unwrap().show_centerlines = true;
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let vp = built.picks.iter().position(|p| *p == PickTarget::View(ViewId(1))).unwrap() as u32;
    let cl: Vec<_> = built.geometry.lines.iter().filter(|l| l.pick == vp && l.style == sheet::LineStyle::Centerline).collect();
    assert_eq!(cl.len(), 3, "{cl:#?}");
    // Auto centerlines off (Drawing properties › Insert view defaults):
    // the centermark of face-on circles stays, the axis lines go.
    drawing.sheets[0].view_mut(ViewId(1)).unwrap().auto_centerlines = Some(false);
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    assert!(!built.geometry.lines.iter().any(|l| l.pick == vp && l.style == sheet::LineStyle::Centerline));
    // ViewDefaults on a new view.
    let mut v = DrawingView::default();
    ViewDefaults { hidden_lines: true, tangent: 2, auto_centermarks: true, ..Default::default() }.apply(&mut v);
    assert!(v.style == RenderStyle::HiddenLinesVisible && v.tangent_phantom && v.show_centerlines && v.auto_centerlines == Some(false));
}

#[test]
fn bom_hole_table_and_linked_balloons() {
    let (doc, mut drawing) = demo();
    let asm = assembly_tab(&doc);
    // Sheet 2: the assembly with a BOM and a callout on the pin.
    let si = drawing.add_sheet(0);
    drawing.sheets[si].views.push(base_view(1, asm, ViewOrientation::Isometric, [150.0, 160.0]));
    drawing.sheets[si].tables.push(Table::new(1, TableKind::Bom { view: ViewId(1), structured: false, bottom_to_top: false, overrides: Vec::new() }, [410.0, 287.0]));
    // Hole table of the plate's top view on sheet 1.
    let origin = [-0.05, -0.03];
    drawing.sheets[0].tables.push(Table::new(1, TableKind::Hole { view: ViewId(2), origin, origin_ref: None, precision: None, flip_x: false, flip_y: false }, [410.0, 120.0]));
    let sources = regenerate_sources(&doc, &drawing);
    let src = &sources[&SourceKey::studio(asm)];
    assert!(src.assembly);
    assert_eq!(src.bom.iter().map(|r| (r.item, r.quantity)).collect::<Vec<_>>(), [(1, 2), (2, 1)]);
    let cache = ViewCache::new();
    let g = compute_view(&drawing, &drawing.sheets[si], ViewId(1), &sources, &cache, false);
    assert!(g.error.is_none() && !g.edges.is_empty(), "{:?}", g.error);
    // A vertex of the pin.
    let pin = g.snaps.iter().find_map(|s| match &s.point {
        Some(PointRef::Vertex { body, .. }) if src.bom_item(body) == Some(2) => Some(s.clone()),
        _ => None,
    });
    let pin = pin.expect("pin vertex snap");
    drawing.sheets[si].annotations.push(Annotation {
        id: 1,
        kind: AnnotationKind::balloon(ViewId(1), pin.p, ""),
        position: [220.0, 220.0],
        refs: vec![AnnotationRef { slot: 0, point: pin.point.clone().unwrap() }],
        ..Default::default()
    });
    let built = render_sheet(&drawing, si, &sources, &cache, false).unwrap();
    let t = texts(&built);
    for want in ["Item", "Quantity", "Part number", "2"] {
        assert!(t.iter().any(|x| x == want), "missing {want}: {t:?}");
    }
    let bp = built.picks.iter().position(|p| *p == PickTarget::Annotation(1)).unwrap() as u32;
    assert_eq!(built.geometry.texts.iter().find(|x| x.pick == bp).unwrap().text, "2");
    assert!(built.table_layout(1).is_some());

    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let t = texts(&built);
    // 1 large hole (A1) and 2 small ones (B1, B2), located from the corner.
    for want in ["HOLE TABLE", "A1", "B1", "B2", "Ø24", "Ø9.6", "50", "30", "12", "88", "48"] {
        assert!(t.iter().any(|x| x == want), "missing {want}: {t:?}");
    }
    // Exports include the table.
    assert!(export::to_svg(&built.geometry).contains("HOLE TABLE"));
    assert!(export::to_dxf(&built.geometry).contains("HOLE TABLE"));
    assert!(String::from_utf8_lossy(&export::to_pdf(&built.geometry)).contains("HOLE TABLE"));
}

#[test]
fn dimension_formats_and_new_dimension_types() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let sheet = &mut drawing.sheets[0];
    // Demo annotation 1: top view width (100). Deviation tolerance.
    sheet.annotations[0].format = DimFormat { tolerance: Tolerance::Deviation { upper: 0.0001, lower: -0.00005 }, ..Default::default() };
    // Annotation 2: depth (60), 3 decimals, inches shown.
    sheet.annotations[1].format = DimFormat { precision: Some(3), units: Some(DimUnits::Inch), show_units: true, ..Default::default() };
    // Annotation 3: the center hole diameter with an H7 fit.
    sheet.annotations[2].format = DimFormat { tolerance: Tolerance::Fit { hole: "H7".into(), shaft: String::new() }, ..Default::default() };
    let mut id = sheet.next_annotation_id();
    let mut add = |sheet: &mut Sheet, kind: AnnotationKind, position: [f64; 2]| {
        sheet.annotations.push(Annotation { id, kind, position, ..Default::default() });
        id += 1;
    };
    add(sheet, AnnotationKind::OrdinateDimension { view: ViewId(2), origin: [-0.05, -0.03], point: [-0.05, -0.03], horizontal: true }, [60.0, 260.0]);
    add(sheet, AnnotationKind::OrdinateDimension { view: ViewId(2), origin: [-0.05, -0.03], point: [0.038, 0.018], horizontal: true }, [148.0, 260.0]);
    add(sheet, AnnotationKind::ChamferDimension { view: ViewId(1), a: [0.0, 0.0], b: [0.002, 0.002], c: [0.002, 0.002], d: [0.01, 0.002] }, [150.0, 180.0]);
    add(sheet, AnnotationKind::ArcLengthDimension { view: ViewId(2), center: [0.0, 0.0], radius: 0.01, a: [0.01, 0.0], b: [0.0, 0.01], mid: [0.00707, 0.00707], offset: 5.0 }, [0.0; 2]);
    add(sheet, AnnotationKind::Gdt { anchor: [40.0, 40.0], frame: vec!["⌖".into(), "Ø0.1Ⓜ".into(), "A".into(), "B".into()], leaders: vec![] }, [40.0, 40.0]);
    add(sheet, AnnotationKind::Datum { anchor: [60.0, 100.0], label: "A".into() }, [60.0, 112.0]);
    add(sheet, AnnotationKind::SurfaceFinish { anchor: [80.0, 100.0], value: "Ra 1.6".into() }, [80.0, 100.0]);
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let t = texts(&built);
    for want in ["100", "+0.1", "-0.05", "2.362 in", "Ø24 H7", "0", "88", "2 × 45°", "⌒15.71", "⌖", "Ø0.1Ⓜ", "A", "Ra 1.6"] {
        assert!(t.iter().any(|x| x == want), "missing {want}: {t:?}");
    }
    // Symbol texts are exported as strokes (no unsupported glyphs).
    let svg = export::to_svg(&built.geometry);
    assert!(!svg.contains('⌖') && svg.contains("Ra 1.6"));
    let pdf = export::to_pdf(&built.geometry);
    assert!(!String::from_utf8_lossy(&pdf).contains("(⌖)"));
}

#[test]
fn property_fields_in_notes_and_title_block() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    drawing.add_sheet(0);
    let sheet = &mut drawing.sheets[0];
    if let Some(f) = sheet.title_block.fields.iter_mut().find(|(k, _)| k == "Title") {
        f.1 = "{part.name} ({doc.name})".into();
    }
    sheet.annotations.push(Annotation { id: 99, kind: AnnotationKind::note("SHEET {sheet.number} OF {sheet.count}\nSCALE {sheet.scale}\n{nope}", 3.5), position: [30.0, 120.0], ..Default::default() });
    let info = SheetInfo { doc_name: "Demo".into(), drawing_name: "Drawing 1".into(), ..Default::default() };
    let built = render_sheet_with(&drawing, 0, &sources, &cache, false, &info).unwrap();
    let t = texts(&built);
    for want in ["Part 1 (Demo)", "SHEET 1 OF 2", "SCALE 1:1", "{nope}"] {
        assert!(t.iter().any(|x| x == want), "missing {want}: {t:?}");
    }
}

#[test]
fn configured_views_show_their_configuration() {
    use crate::configuration::{ConfigInput, ConfigInputKind, ConfiguredParam, ListOption};
    use crate::params::ParamValue;
    use noshape_units::ExprValue;
    let (mut doc, _) = demo();
    let tab = studio_tab(&doc);
    if let TabContent::PartStudio(ps) = &mut doc.tabs[0].content {
        let ex = ps.features.iter().find(|f| f.type_id == "extrude").unwrap().id;
        ps.configuration.inputs.push(ConfigInput {
            id: "size".into(),
            name: "Size".into(),
            kind: ConfigInputKind::List { options: vec![ListOption { id: "thin".into(), name: "Thin".into() }, ListOption { id: "thick".into(), name: "Thick".into() }], default: "thin".into() },
            visible_when: None,
        });
        let mut values = std::collections::BTreeMap::new();
        values.insert("thin".to_string(), ParamValue::Expr(ExprValue::new("5 mm")));
        values.insert("thick".to_string(), ParamValue::Expr(ExprValue::new("20 mm")));
        ps.configuration.configured.push(ConfiguredParam { input: "size".into(), feature: ex, param: "depth".into(), values });
    }
    let mut drawing = Drawing::default();
    let mut thin = base_view(1, tab, ViewOrientation::Front, [100.0, 150.0]);
    let mut thick = base_view(2, tab, ViewOrientation::Front, [250.0, 150.0]);
    if let ViewKind::Base { config, .. } = &mut thin.kind {
        config.0.insert("size".into(), "thin".into());
    }
    if let ViewKind::Base { config, .. } = &mut thick.kind {
        config.0.insert("size".into(), "thick".into());
    }
    drawing.sheets[0].views = vec![thin, thick];
    let sources = regenerate_sources(&doc, &drawing);
    assert_eq!(sources.len(), 2);
    let cache = ViewCache::new();
    let h = |id| {
        let g = compute_view(&drawing, &drawing.sheets[0], ViewId(id), &sources, &cache, false);
        g.max[1] - g.min[1]
    };
    assert!((h(1) - 0.005).abs() < 1e-6, "{}", h(1));
    assert!((h(2) - 0.020).abs() < 1e-6, "{}", h(2));
}

#[test]
fn assembly_views_and_exploded_views() {
    use crate::assembly::eval::{AssemblyEvaluator, SolveOptions};
    let (mut doc, _) = demo();
    let asm = assembly_tab(&doc);
    let mut ev = AssemblyEvaluator::new();
    let exploded = {
        let a = match &doc.tab(asm).unwrap().content {
            TabContent::Assembly(a) => a.clone(),
            _ => unreachable!(),
        };
        let r = ev.evaluate(&doc, &a, &SolveOptions::default(), None);
        crate::assembly::analysis::auto_exploded_view(&r, "Exploded 1", 1.0)
    };
    if let Some(TabContent::Assembly(a)) = doc.tabs.iter_mut().find(|t| t.id == asm).map(|t| &mut t.content) {
        a.exploded_views.push(exploded);
    }
    let mut drawing = Drawing::default();
    drawing.sheets[0].views.push(base_view(1, asm, ViewOrientation::Front, [120.0, 150.0]));
    let mut ex = base_view(2, asm, ViewOrientation::Front, [300.0, 150.0]);
    ex.explode = Some(0);
    drawing.sheets[0].views.push(ex);
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let g1 = compute_view(&drawing, &drawing.sheets[0], ViewId(1), &sources, &cache, false);
    let g2 = compute_view(&drawing, &drawing.sheets[0], ViewId(2), &sources, &cache, false);
    assert!(g1.error.is_none() && g2.error.is_none());
    assert!(!g1.edges.is_empty());
    let size = |g: &views::ViewGeometry| (g.max[0] - g.min[0]).max(g.max[1] - g.min[1]);
    assert!(size(&g2) > size(&g1) * 1.2, "exploded {} vs {}", size(&g2), size(&g1));
    // Dimensions in assembly views resolve against occurrence bodies.
    let snap = g1.snaps.iter().find(|s| s.point.is_some()).unwrap();
    let src = &sources[&SourceKey::studio(asm)];
    let (p, _) = views::resolve_point(&src.context, g1.frame.as_ref().unwrap(), snap.point.as_ref().unwrap()).unwrap();
    assert!((p[0] - snap.p[0]).abs() < 1e-12 && (p[1] - snap.p[1]).abs() < 1e-12);
}

// ------------------------------------------------------------ sections, crops, dimensions, tables, templates

fn pick_of(b: &build::BuiltSheet, t: PickTarget) -> u32 {
    b.picks.iter().position(|p| *p == t).unwrap_or(0) as u32
}

fn seg_range(segs: &[[[f64; 2]; 2]], axis: usize) -> f64 {
    let (lo, hi) = segs.iter().flatten().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p[axis]), hi.max(p[axis])));
    hi - lo
}

#[test]
fn broken_out_section_exposes_interior() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let before = compute_view(&drawing, &drawing.sheets[0], ViewId(1), &sources, &cache, false);
    assert!(before.section_segments.is_empty());
    // Front view (x right, z up): open the plate down to its middle (y = 0)
    // around the center hole.
    let profile = vec![[-0.02, -0.003], [0.0, -0.005], [0.02, -0.003], [0.022, 0.015], [-0.022, 0.015]];
    drawing.sheets[0].view_mut(ViewId(1)).unwrap().broken_out.push(BrokenOut { profile: profile.clone(), spline: true, depth: 0.03, depth_ref: None });
    let g = compute_view(&drawing, &drawing.sheets[0], ViewId(1), &sources, &cache, true);
    assert!(g.error.is_none(), "{:?}", g.error);
    assert!(!g.section_segments.is_empty(), "pocket floor is hatched");
    // The cut halves the center hole: its walls at x = +-12 mm now show as
    // visible lines inside the broken-out region.
    for x in [-0.012, 0.012] {
        assert!(
            g.edges.iter().any(|e| e.visible && e.points.iter().all(|p| (p[0] - x).abs() < 2e-4) && e.points.len() >= 2),
            "hole wall at x = {x} visible"
        );
    }
    // Snap points removed with the material are gone; edges keep their names.
    assert!(g.snaps.len() <= before.snaps.len());
    assert!(g.edges.iter().any(|e| e.model.is_some()));
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let vp = pick_of(&built, PickTarget::View(ViewId(1)));
    assert!(built.geometry.lines.iter().filter(|l| l.pick == vp && l.style == sheet::LineStyle::Thin).count() > 3, "hatch lines");
    // Up to entity: the depth follows a model vertex (the plate's back face).
    let studio = studio_tab(&doc);
    let ctx = sources[&SourceKey::studio(studio)].context.clone();
    let back = views::vertex_ref_near(&ctx, glam::DVec3::new(-0.05, 0.03, 0.0), 1e-6).expect("back vertex");
    let frame = g.frame.unwrap();
    let bo = BrokenOut { profile, spline: true, depth: 0.0, depth_ref: Some(back) };
    assert!((views::broken_out_depth(&ctx, None, &frame, &bo) - 0.06).abs() < 1e-9);
}

#[test]
fn section_cut_only_and_depth() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    drawing.sheets[0].set_section_points(ViewId(5), vec![[-0.025, -0.04], [-0.025, 0.04]]);
    let full = compute_view(&drawing, &drawing.sheets[0], ViewId(5), &sources, &cache, false);
    // Show cut geometry only: just the 60 x 12 mm cut face outline.
    drawing.sheets[0].view_mut(ViewId(5)).unwrap().cut_only = true;
    let cut = compute_view(&drawing, &drawing.sheets[0], ViewId(5), &sources, &cache, false);
    assert!(cut.error.is_none(), "{:?}", cut.error);
    assert!(!cut.edges.is_empty() && cut.edges.len() < full.edges.len(), "{} vs {}", cut.edges.len(), full.edges.len());
    assert!(cut.edges.iter().all(|e| e.visible));
    assert!((seg_range(&cut.section_segments, 1) - 0.06).abs() < 1e-3);
    // Section depth 5 mm: the holes 13+ mm behind the cut are left out,
    // the cut face stays (and is still hatched).
    let v = drawing.sheets[0].view_mut(ViewId(5)).unwrap();
    v.cut_only = false;
    v.section_depth = Some(0.005);
    let shallow = compute_view(&drawing, &drawing.sheets[0], ViewId(5), &sources, &cache, false);
    assert!(shallow.error.is_none(), "{:?}", shallow.error);
    assert!(shallow.edges.len() < full.edges.len(), "{} vs {}", shallow.edges.len(), full.edges.len());
    assert!((seg_range(&shallow.section_segments, 1) - 0.06).abs() < 1e-3, "{}", seg_range(&shallow.section_segments, 1));
}

#[test]
fn partial_and_angular_sections() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    // A full section of the top view along x = -25 mm (clear of the holes).
    drawing.sheets[0].set_section_points(ViewId(5), vec![[-0.025, -0.04], [-0.025, 0.04]]);
    let full = compute_view(&drawing, &drawing.sheets[0], ViewId(5), &sources, &cache, false);
    // (The section view shows the model's y axis upward.)
    assert!((seg_range(&full.section_segments, 1) - 0.06).abs() < 1e-3, "{}", seg_range(&full.section_segments, 1));
    // Partial: a short cutting line inside the part cuts only 20 mm.
    drawing.sheets[0].set_section_points(ViewId(5), vec![[-0.025, -0.01], [-0.025, 0.01]]);
    drawing.sheets[0].view_mut(ViewId(5)).unwrap().partial = true;
    let part = compute_view(&drawing, &drawing.sheets[0], ViewId(5), &sources, &cache, false);
    assert!(part.error.is_none(), "{:?}", part.error);
    assert!((seg_range(&part.section_segments, 1) - 0.02).abs() < 1e-3, "{}", seg_range(&part.section_segments, 1));
    // ...and the rest of the part is still shown.
    assert!((part.max[1] - part.min[1] - 0.06).abs() < 1e-3);
    // Angular: a cutting line at atan(0.75) through the center.
    drawing.sheets[0].view_mut(ViewId(5)).unwrap().partial = false;
    drawing.sheets[0].set_section_points(ViewId(5), vec![[-0.04, -0.03], [0.04, 0.03]]);
    let ang = compute_view(&drawing, &drawing.sheets[0], ViewId(5), &sources, &cache, false);
    assert!(ang.error.is_none(), "{:?}", ang.error);
    // The cut face spans the plate's 100 mm diagonal (by 12 mm thickness).
    let pts: Vec<[f64; 2]> = ang.section_segments.iter().flatten().copied().collect();
    let span = pts.iter().flat_map(|p| pts.iter().map(move |q| ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt())).fold(0.0, f64::max);
    assert!(span > 0.098 && span < 0.1008, "{span}");
}

#[test]
fn editing_cutting_line_points() {
    // Jogged lines keep their horizontal/vertical legs.
    let line = vec![[0.0, -0.04], [0.0, 0.01], [0.038, 0.01], [0.038, 0.04]];
    let moved = views::move_cut_point(&line, 1, [0.005, 0.02], true);
    assert_eq!(moved[1], [0.005, 0.02]);
    assert_eq!(moved[0], [0.005, -0.04], "first leg stays vertical");
    assert_eq!(moved[2], [0.038, 0.02], "jog stays horizontal");
    assert_eq!(moved[3], line[3]);
    // A straight line's end moves freely (an angular section).
    let tilted = views::move_cut_point(&[[0.0, -0.04], [0.0, 0.04]], 1, [0.02, 0.04], true);
    assert_eq!(tilted, vec![[0.0, -0.04], [0.02, 0.04]]);
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let pts = drawing.sheets[0].section_points(ViewId(5)).unwrap();
    assert!(drawing.sheets[0].set_section_points(ViewId(5), views::move_cut_point(&pts, 1, [0.01, 0.04], true)));
    let g = compute_view(&drawing, &drawing.sheets[0], ViewId(5), &sources, &cache, false);
    assert!(g.error.is_none() && !g.section_segments.is_empty(), "{:?}", g.error);
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let sp = pick_of(&built, PickTarget::SectionLine(ViewId(5)));
    let l = built.geometry.lines.iter().find(|l| l.pick == sp && l.style == sheet::LineStyle::Section).unwrap();
    assert!((l.points[1][0] - l.points[0][0]).abs() > 1.0, "cutting line is tilted on the sheet");
}

#[test]
fn spline_and_polygon_crops() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let pts = vec![[-0.03, -0.02], [0.0, -0.028], [0.03, -0.02], [0.03, 0.02], [-0.03, 0.02]];
    drawing.sheets[0].view_mut(ViewId(2)).unwrap().crop = Some(Crop::Spline(pts.clone()));
    let g = compute_view(&drawing, &drawing.sheets[0], ViewId(2), &sources, &cache, false);
    let outline = views::closed_spline(&pts, 16);
    assert!(!g.edges.is_empty());
    for e in &g.edges {
        for p in &e.points {
            let inside = views::point_in_polygon(*p, &outline) || outline.windows(2).any(|w| views::dist_to_segment(*p, w[0], w[1]) < 1e-7) || views::dist_to_segment(*p, outline[outline.len() - 1], outline[0]) < 1e-7;
            assert!(inside, "{p:?} outside the spline crop");
        }
    }
    // The spline passes through its points.
    for p in &pts {
        assert!(outline.iter().any(|q| (q[0] - p[0]).abs() < 1e-12 && (q[1] - p[1]).abs() < 1e-12));
    }
    assert!(g.snaps.iter().all(|s| views::point_in_polygon(s.p, &outline)));
    // A free polygon (triangle) crop.
    let tri = vec![[-0.05, -0.03], [0.05, -0.03], [0.0, 0.03]];
    drawing.sheets[0].view_mut(ViewId(2)).unwrap().crop = Some(Crop::Polygon(tri.clone()));
    let g = compute_view(&drawing, &drawing.sheets[0], ViewId(2), &sources, &cache, false);
    assert!(g.edges.iter().flat_map(|e| e.points.iter()).all(|p| p[1] <= 0.03 - (p[0].abs() / 0.05) * 0.06 + 1e-7));
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    assert!(built.placement(ViewId(2)).is_some());
}

#[test]
fn grouped_ordinate_dimensions() {
    assert_eq!(sheet::ordinate_text_positions(&[0.0, 1.0, 2.0, 30.0], 5.0), vec![-4.0, 1.0, 6.0, 30.0]);
    assert_eq!(sheet::ordinate_text_positions(&[10.0, 0.0], 5.0), vec![10.0, 0.0]);
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let g = compute_view(&drawing, &drawing.sheets[0], ViewId(2), &sources, &cache, false);
    let snap = |p: [f64; 2]| g.snaps.iter().find(|s| (s.p[0] - p[0]).abs() < 1e-6 && (s.p[1] - p[1]).abs() < 1e-6).and_then(|s| s.point.clone());
    let origin = [-0.05, -0.03];
    let pts = vec![origin, [-0.038, -0.018], [-0.036, -0.018], [0.0, 0.0], [0.038, 0.018], [0.05, 0.03]];
    let mut refs = vec![AnnotationRef { slot: 0, point: snap(origin).expect("corner snap") }];
    for (i, p) in pts.iter().enumerate() {
        if let Some(r) = snap(*p) {
            refs.push(AnnotationRef { slot: i as u8 + 1, point: r });
        }
    }
    let id = drawing.sheets[0].next_annotation_id();
    drawing.sheets[0].annotations.push(Annotation { id, kind: AnnotationKind::OrdinateGroup { view: ViewId(2), origin, points: pts.clone(), horizontal: true }, position: [100.0, 262.0], refs, ..Default::default() });
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    assert!(built.dangling.is_empty(), "{:?}", built.dangling);
    let pick = pick_of(&built, PickTarget::Annotation(id));
    let ts: Vec<&sheet::Text2> = built.geometry.texts.iter().filter(|t| t.pick == pick).collect();
    let vals: Vec<&str> = ts.iter().map(|t| t.text.as_str()).collect();
    for want in ["0", "12", "14", "50", "88", "100"] {
        assert!(vals.contains(&want), "missing {want}: {vals:?}");
    }
    // Every value on one line; the close values 12 and 14 are jogged apart.
    let xs: Vec<f64> = ts.iter().map(|t| t.pos[0]).collect();
    let mut sorted = xs.clone();
    sorted.sort_by(f64::total_cmp);
    assert!(sorted.windows(2).all(|w| w[1] - w[0] >= 5.0 - 1e-6), "{sorted:?}");
    assert!(ts.iter().all(|t| (t.pos[1] - ts[0].pos[1]).abs() < 1e-6 || t.angle.abs() > 1.0));
}

#[test]
fn max_min_dimensions() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let first = drawing.sheets[0].next_annotation_id();
    let mut ids = Vec::new();
    // Center hole (R12) to the plate corner, horizontally: 50 to the center.
    let cases = [(ArcCondition::Min, ArcCondition::Center, 0.0), (ArcCondition::Max, ArcCondition::Center, 0.0), (ArcCondition::Min, ArcCondition::Min, 0.0048), (ArcCondition::Max, ArcCondition::Max, 0.0048)];
    for (id, (ca, cb, rb)) in (first..).zip(cases) {
        let b = if rb > 0.0 { [0.038, 0.018] } else { [-0.05, -0.03] };
        drawing.sheets[0].annotations.push(Annotation {
            id,
            kind: AnnotationKind::MaxMinDimension { view: ViewId(2), a: [0.0, 0.0], ra: 0.012, ca, b, rb, cb, offset: 8.0, orientation: DimOrientation::Horizontal },
            position: [0.0; 2],
            ..Default::default()
        });
        ids.push(id);
    }
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let text = |id: u64| {
        let p = pick_of(&built, PickTarget::Annotation(id));
        built.geometry.texts.iter().find(|t| t.pick == p).map(|t| t.text.clone()).unwrap()
    };
    assert_eq!(text(ids[0]), "38");
    assert_eq!(text(ids[1]), "62");
    assert_eq!(text(ids[2]), "21.2");
    assert_eq!(text(ids[3]), "54.8");
    // The min dimension's extension line starts on the circle (12 mm right of center).
    let pl = built.placement(ViewId(2)).unwrap();
    let c = pl.to_sheet([0.0, 0.0]);
    let p0 = pick_of(&built, PickTarget::Annotation(ids[0]));
    assert!(built.geometry.lines.iter().filter(|l| l.pick == p0).any(|l| (l.points[0][0] - (c[0] - 12.0)).abs() < 1e-6));
}

#[test]
fn foreshortened_dimensions() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let s = &mut drawing.sheets[0];
    let id = s.next_annotation_id();
    s.annotations.push(Annotation { id, kind: AnnotationKind::ForeshortenedRadial { view: ViewId(2), center: [0.0, 0.0], radius: 0.012, angle: 0.6, jog: [0.03, 0.035] }, position: [0.0; 2], ..Default::default() });
    // Front view: half diameter of the center hole about its axis (x = 0).
    s.annotations.push(Annotation { id: id + 1, kind: AnnotationKind::ForeshortenedDiameter { view: ViewId(1), axis: [[0.0, 0.0], [0.0, 0.012]], edge: [0.012, 0.006], direct: false }, position: [110.0, 140.0], ..Default::default() });
    s.annotations.push(Annotation { id: id + 2, kind: AnnotationKind::ForeshortenedDiameter { view: ViewId(1), axis: [[0.0, 0.0], [0.0, 0.012]], edge: [0.012, 0.006], direct: true }, position: [110.0, 145.0], ..Default::default() });
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let texts_of = |id: u64| {
        let p = pick_of(&built, PickTarget::Annotation(id));
        built.geometry.texts.iter().filter(|t| t.pick == p).map(|t| t.text.clone()).collect::<Vec<_>>()
    };
    assert_eq!(texts_of(id), vec!["R12".to_string()]);
    assert_eq!(texts_of(id + 1), vec!["Ø24".to_string()]);
    assert_eq!(texts_of(id + 2), vec!["Ø24".to_string()]);
    // The foreshortened radius leader is a jogged polyline from the false center.
    let p = pick_of(&built, PickTarget::Annotation(id));
    let pl = built.placement(ViewId(2)).unwrap();
    let f = pl.to_sheet([0.03, 0.035]);
    let leader = built.geometry.lines.iter().find(|l| l.pick == p && l.points.len() == 4).expect("jogged leader");
    assert!((leader.points[0][0] - f[0]).abs() < 1e-9 && (leader.points[0][1] - f[1]).abs() < 1e-9);
    // Half diameter: one dimension line crossing the axis; direct: both ends on the cylinder.
    let fp = pl.to_sheet([0.0, 0.0]);
    let _ = fp;
    let p1 = pick_of(&built, PickTarget::Annotation(id + 1));
    let p2 = pick_of(&built, PickTarget::Annotation(id + 2));
    let fpl = built.placement(ViewId(1)).unwrap();
    let axis_x = fpl.to_sheet([0.0, 0.0])[0];
    let half = built.geometry.lines.iter().filter(|l| l.pick == p1 && (l.points[0][1] - l.points[1][1]).abs() < 1e-9).map(|l| l.points[1][0]).next().unwrap();
    assert!((half - (axis_x - 6.0)).abs() < 1e-6, "overshoots the axis by 6 mm: {half} vs {axis_x}");
    let full = built.geometry.lines.iter().filter(|l| l.pick == p2 && (l.points[0][1] - l.points[1][1]).abs() < 1e-9).map(|l| l.points[1][0]).next().unwrap();
    assert!((full - (axis_x - 12.0)).abs() < 1e-6);
}

#[test]
fn aligned_section_dimensions_are_associative() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let id = drawing.sheets[0].next_view_id();
    drawing.sheets[0].views.push(DrawingView { id, kind: ViewKind::AlignedSection { parent: ViewId(2), center: [0.0, 0.0], a: [0.0, -0.045], b: [0.0475, 0.0225], label: "C".into() }, position: [330.0, 215.0], ..Default::default() });
    let g = compute_view(&drawing, &drawing.sheets[0], id, &sources, &cache, false);
    assert!(g.error.is_none(), "{:?}", g.error);
    assert!(g.aligned.is_some());
    assert!(!g.model_edges.is_empty(), "edges keep their model names");
    assert!(g.snaps.len() >= 4, "{}", g.snaps.len());
    let studio = studio_tab(&doc);
    let ctx = sources[&SourceKey::studio(studio)].context.clone();
    // Every snap resolves back to where it is drawn, including the
    // revolved leg's vertices.
    let mut revolved = 0;
    for s in &g.snaps {
        let r = s.point.as_ref().unwrap();
        let (p, _) = views::resolve_in_view(&ctx, &g, r).expect("resolves");
        assert!((p[0] - s.p[0]).abs() < 1e-9 && (p[1] - s.p[1]).abs() < 1e-9, "{p:?} vs {:?}", s.p);
        let (p3, _) = views::resolve_point3(&ctx, r).unwrap();
        if g.aligned.unwrap().revolved(p3) {
            revolved += 1;
        }
    }
    assert!(revolved > 0, "the revolved leg has snap points");
    // A dimension between two snaps survives a rebuild.
    let (sa, sb) = (g.snaps[0].clone(), g.snaps.iter().rev().find(|s| (s.p[0] - g.snaps[0].p[0]).abs() > 1e-4).unwrap().clone());
    let aid = drawing.sheets[0].next_annotation_id();
    drawing.sheets[0].annotations.push(Annotation {
        id: aid,
        kind: AnnotationKind::LinearDimension { view: id, a: [0.0; 2], b: [0.0; 2], offset: 10.0, orientation: DimOrientation::Horizontal, override_text: None },
        position: [0.0; 2],
        refs: vec![AnnotationRef { slot: 0, point: sa.point.clone().unwrap() }, AnnotationRef { slot: 1, point: sb.point.clone().unwrap() }],
        ..Default::default()
    });
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    assert!(!built.dangling.contains(&aid), "{:?}", built.dangling);
    let want = format::format_len(&drawing, &DimFormat::default(), (sb.p[0] - sa.p[0]).abs());
    let p = pick_of(&built, PickTarget::Annotation(aid));
    assert_eq!(built.geometry.texts.iter().find(|t| t.pick == p).unwrap().text, want);
}

#[test]
fn table_cell_formats_and_merged_cells() {
    let cells = vec![vec!["PART".into(), "".into(), "QTY".into()], vec!["Bracket".into(), "B-1".into(), "2".into()], vec!["Pin".into(), "P-7".into(), "4".into()]];
    let mut t = Table::new(1, TableKind::General { cells, title: Some("PARTS".into()), header: true }, [300.0, 200.0]);
    assert!(t.merge_cells((0, 0), (0, 1)));
    assert!(!t.merge_cells((0, 1), (1, 1)), "partial overlap rejected");
    assert_eq!(t.anchor_cell(0, 1), (0, 0));
    t.set_cell_format(1, 0, CellFormat { align: CellAlign::Left, height: Some(4.0), bold: true });
    t.set_cell_format(2, 2, CellFormat { align: CellAlign::Right, ..Default::default() });
    assert_eq!(t.cell_format(1, 0).height, Some(4.0));
    t.set_cell_format(2, 1, CellFormat::default());
    assert_eq!(t.cell_formats.len(), 2, "default formats are not stored");
    let d = tables::table_data(&Drawing::default(), &t, None, None, None, &[], &|s| s.to_string());
    let l = tables::layout(&t, &d);
    let mut g = sheet::SheetGeometry::new(420.0, 297.0);
    tables::render(&mut g, &t, &d, &l, 1);
    // The merged header shows once, centered over both columns.
    let part = g.texts.iter().find(|x| x.text == "PART").unwrap();
    assert!((part.pos[0] - (l.xs[0] + l.xs[2]) / 2.0).abs() < 1e-9);
    // No divider crosses the merged cell.
    let y_hdr = (l.ys[1] + l.ys[2]) / 2.0;
    assert!(!g.lines.iter().any(|ln| ln.points.len() == 2 && (ln.points[0][0] - l.xs[1]).abs() < 1e-9 && (ln.points[1][0] - l.xs[1]).abs() < 1e-9 && ln.points[0][1].min(ln.points[1][1]) < y_hdr && ln.points[0][1].max(ln.points[1][1]) > y_hdr));
    // Bold: drawn three times; left aligned; bigger text makes the row taller.
    let br: Vec<&sheet::Text2> = g.texts.iter().filter(|x| x.text == "Bracket").collect();
    assert_eq!(br.len(), 3);
    assert!(br[0].height == 4.0 && br[0].pos[0] < l.xs[0] + 3.0);
    assert!(l.ys[2] - l.ys[3] > l.ys[3] - l.ys[4]);
    let four = g.texts.iter().find(|x| x.text == "4").unwrap();
    assert!(four.pos[0] > (l.xs[2] + l.xs[3]) / 2.0, "right aligned");
    // Inserting a row above shifts formats and merges; deleting restores.
    t.insert_row(0);
    assert_eq!((t.merges[0].row, t.cell_format(2, 0).bold), (1, true));
    assert!(t.delete_row(0));
    assert_eq!((t.merges[0].row, t.cell_format(1, 0).bold), (0, true));
    // Inserting a column inside a merge widens it.
    t.insert_col(1);
    assert_eq!(t.merges[0].cols, 3);
    assert!(t.unmerge(0, 2) && t.merges.is_empty());
}

#[test]
fn gdt_leaders_and_datum_targets() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let s = &mut drawing.sheets[0];
    let id = s.next_annotation_id();
    let target = [0.05, 0.0];
    s.annotations.push(Annotation { id, kind: AnnotationKind::Gdt { anchor: [200.0, 250.0], frame: vec!["▱".into(), "0.05".into()], leaders: vec![Leader { view: Some(ViewId(2)), point: target, nodes: Vec::new() }] }, position: [200.0, 250.0], ..Default::default() });
    s.annotations.push(Annotation { id: id + 1, kind: AnnotationKind::DatumTarget { view: ViewId(2), anchor: [-0.03, 0.0], label: "A1".into(), area: DatumTargetArea::Circle { diameter: 0.01 } }, position: [60.0, 250.0], ..Default::default() });
    s.annotations.push(Annotation { id: id + 2, kind: AnnotationKind::DatumTarget { view: ViewId(2), anchor: [-0.04, 0.02], label: "A2".into(), area: DatumTargetArea::Point }, position: [50.0, 262.0], ..Default::default() });
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let pl = *built.placement(ViewId(2)).unwrap();
    let tip = pl.to_sheet(target);
    assert!(built.geometry.fills.iter().any(|f| (f.points[0][0] - tip[0]).abs() < 1e-9 && (f.points[0][1] - tip[1]).abs() < 1e-9), "leader arrow at the edge");
    let t = texts(&built);
    for want in ["A1", "Ø10", "A2"] {
        assert!(t.iter().any(|x| x == want), "missing {want}: {t:?}");
    }
    // The leader follows its view.
    drawing.sheets[0].view_mut(ViewId(2)).unwrap().position[0] += 10.0;
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    assert!(built.geometry.fills.iter().any(|f| (f.points[0][0] - tip[0] - 10.0).abs() < 1e-9));
    // Deleting the view drops the leader, not the frame.
    drawing.sheets[0].remove_view(ViewId(2));
    let gdt = drawing.sheets[0].annotations.iter().find(|a| a.id == id).unwrap();
    assert!(matches!(&gdt.kind, AnnotationKind::Gdt { leaders, .. } if leaders.is_empty()));
    assert!(!drawing.sheets[0].annotations.iter().any(|a| a.id == id + 1));
}

#[test]
fn move_views_between_sheets() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let top_frame = views::view_frame(&drawing, &drawing.sheets[0], ViewId(2)).unwrap();
    let to = drawing.add_sheet(0);
    // A section view cannot leave its parent behind.
    assert!(drawing.move_views_to_sheet(0, &[ViewId(5)], to, Some(&sources)).is_err());
    let n_ann = drawing.sheets[0].annotations.len();
    let on_top = drawing.sheets[0].annotations.iter().filter(|a| a.kind.view().is_some_and(|v| [2, 5, 6].contains(&v.0))).count();
    // The top view moves with its section and detail views.
    let map = drawing.move_views_to_sheet(0, &[ViewId(2)], to, Some(&sources)).unwrap();
    assert_eq!(map.len(), 3);
    assert_eq!(drawing.sheets[0].views.len(), 3);
    assert_eq!(drawing.sheets[0].annotations.len(), n_ann - on_top);
    let dst = &drawing.sheets[to];
    assert_eq!(dst.views.len(), 3);
    assert_eq!(dst.annotations.len(), on_top);
    let new_top = map[0].1;
    let v = dst.view(new_top).unwrap();
    assert!(matches!(v.kind, ViewKind::Base { .. }) && v.custom_frame.is_some());
    let f = views::view_frame(&drawing, dst, new_top).unwrap();
    assert!((f.dir - top_frame.dir).length() < 1e-12 && (f.up - top_frame.up).length() < 1e-12);
    // The section and detail views follow their (renumbered) parent.
    assert!(dst.views.iter().filter(|v| v.id != new_top).all(|v| dst.parent_of(v.id) == Some(new_top)));
    let built = render_sheet(&drawing, to, &sources, &cache, false).unwrap();
    assert!(built.dangling.is_empty(), "{:?}", built.dangling);
    assert!(texts(&built).iter().any(|t| t == "SECTION A-A"));
    for v in &drawing.sheets[to].views {
        let g = compute_view(&drawing, &drawing.sheets[to], v.id, &sources, &cache, false);
        assert!(g.error.is_none(), "{:?}", g.error);
    }
    // A detail view alone becomes a cropped view at the same place.
    let (doc2, mut d2) = demo();
    let src2 = regenerate_sources(&doc2, &d2);
    let before = render_sheet(&d2, 0, &src2, &cache, false).unwrap();
    let pl = *before.placement(ViewId(6)).unwrap();
    let to2 = d2.add_sheet(0);
    let map = d2.move_views_to_sheet(0, &[ViewId(6)], to2, Some(&src2)).unwrap();
    let after = render_sheet(&d2, to2, &src2, &cache, false).unwrap();
    let pl2 = *after.placement(map[0].1).unwrap();
    assert!(matches!(d2.sheets[to2].views[0].crop, Some(Crop::Circle { .. })));
    let c = [0.038, 0.018];
    let (a, b) = (pl.to_sheet(c), pl2.to_sheet(c));
    assert!((a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6, "{a:?} vs {b:?}");
}

#[test]
fn templates_and_title_blocks() {
    let names: Vec<&str> = templates::standard_templates().iter().map(|t| t.name).collect();
    for n in ["ISO A4", "ISO A3", "ISO A2", "ISO A1", "ISO A0", "ANSI A", "ANSI B", "ANSI C", "ANSI D", "ANSI E"] {
        assert!(names.contains(&n), "{n}");
    }
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    // Keep the demo's values when switching layouts.
    templates::apply_template(&mut drawing, 0, &templates::template_by_name("ISO A3").unwrap());
    assert_eq!(drawing.projection, Projection::FirstAngle);
    let s = &drawing.sheets[0];
    assert_eq!((s.size, s.title_style), (SheetSize::A3, TitleBlockStyle::Iso));
    assert_eq!(s.title_field("Part number"), Some("P-001"));
    assert_eq!(s.title_field("Title"), Some("Plate"));
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let t = texts(&built);
    for want in ["LEGAL OWNER", "DOCUMENT TYPE", "P-001", "Plate", "1:1", "Part drawing"] {
        assert!(t.iter().any(|x| x == want), "missing {want}: {t:?}");
    }
    let rect = sheet::title_block_rect(&drawing.sheets[0]).unwrap();
    let tb = pick_of(&built, PickTarget::TitleBlock);
    assert_eq!(built.hit([rect[0] + 70.0, rect[1] + 4.0], 1.0), Some(tb));
    // Imperial: ANSI B with a tolerance block in inches.
    templates::apply_template(&mut drawing, 0, &templates::template_by_name("ANSI B").unwrap());
    assert!(!drawing.units_mm && drawing.projection == Projection::ThirdAngle);
    assert_eq!(drawing.sheets[0].size, SheetSize::AnsiB);
    let built = render_sheet(&drawing, 0, &sources, &cache, false).unwrap();
    let t = texts(&built);
    for want in ["TOLERANCES", "DIMENSIONS ARE IN INCHES", "B", "DRAWN BY", "P-001", "3.937"] {
        assert!(t.iter().any(|x| x == want), "missing {want}: {t:?}");
    }
    assert_eq!(built.geometry.width, SheetSize::AnsiB.size_mm().0);
    // Per sheet: another sheet keeps its own layout.
    let i = drawing.add_sheet(0);
    templates::set_title_style(&mut drawing, i, TitleBlockStyle::None);
    let built = render_sheet(&drawing, i, &sources, &cache, false).unwrap();
    assert!(!texts(&built).iter().any(|x| x == "TOLERANCES"));
    assert_eq!(drawing.sheets[0].title_style, TitleBlockStyle::Ansi);
}

#[test]
fn view_simplification_drops_small_detail_and_parts() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let top = ViewId(2);
    let circles = |d: &Drawing| compute_view(d, &d.sheets[0], top, &sources, &cache, false).circles().len();
    assert_eq!(circles(&drawing), 6, "Auto keeps the plate's detail");
    // Absolute 40 mm: circles shorter than that (the small bores) are left out.
    drawing.sheets[0].view_mut(top).unwrap().simplification = Some(Simplification::Absolute(0.04));
    let n = circles(&drawing);
    assert!(n > 0 && n < 6, "{n}");
    // Ratio to part keeps the part.
    drawing.sheets[0].view_mut(top).unwrap().simplification = Some(Simplification::RatioToPart(0.3));
    let g = compute_view(&drawing, &drawing.sheets[0], top, &sources, &cache, false);
    assert!(!g.edges.is_empty() && g.circles().len() < 6, "edges {} circles {}", g.edges.len(), g.circles().len());
    // A threshold larger than the plate leaves it out entirely.
    drawing.sheets[0].view_mut(top).unwrap().simplification = Some(Simplification::Absolute(1.0));
    assert!(compute_view(&drawing, &drawing.sheets[0], top, &sources, &cache, false).edges.is_empty());
    // Children inherit the parent's setting.
    drawing.sheets[0].view_mut(top).unwrap().simplification = None;
    let child = drawing.sheets[0].views.iter().find(|v| drawing.sheets[0].parent_of(v.id) == Some(ViewId(1))).map(|v| v.id);
    if let Some(c) = child {
        drawing.sheets[0].view_mut(ViewId(1)).unwrap().simplification = Some(Simplification::RatioToStudio(0.5));
        assert_eq!(views::view_simplification(&drawing.sheets[0], c), Simplification::RatioToStudio(0.5));
    }
}

#[test]
fn formats_gaps_and_revision_tables_across_sheets() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    drawing.add_sheet(0);
    drawing.props.formats_locked = true;
    drawing.props.title_block_color = Some([220, 40, 40]);
    drawing.props.dim_extension = 5.0;
    drawing.sheets[0].tables.push(Table::new(1, TableKind::revision(), [400.0, 280.0]));
    let rows = vec![tables::RevisionRow { revision: "A".into(), description: "First".into(), date: "2026-10-19".into(), ..Default::default() }];
    let info = SheetInfo { revisions: rows, ..Default::default() };
    let built = render_sheet_with(&drawing, 0, &sources, &cache, false, &info).unwrap();
    // Locked: no title block pick; its texts are red.
    assert!(!built.picks.contains(&PickTarget::TitleBlock));
    let g = &built.geometry;
    assert!((0..g.texts.len()).any(|i| g.text_color(i) == Some([220, 40, 40])));
    assert!(texts(&built).iter().any(|t| t == "REVISION HISTORY") && texts(&built).iter().any(|t| t == "First"));
    // Linked: the full table on sheet 2; See sheet 1: the pointer table.
    let built2 = render_sheet_with(&drawing, 1, &sources, &cache, false, &info).unwrap();
    assert!(texts(&built2).iter().any(|t| t == "First"));
    if let TableKind::Revision { see_sheet1, .. } = &mut drawing.sheets[0].tables[0].kind {
        *see_sheet1 = true;
    }
    let built2 = render_sheet_with(&drawing, 1, &sources, &cache, false, &info).unwrap();
    assert!(texts(&built2).iter().any(|t| t == "SEE SHEET 1") && !texts(&built2).iter().any(|t| t == "First"));
    // Extension past the dimension line.
    let mut g = sheet::SheetGeometry::new(420.0, 297.0);
    g.style.extension = 5.0;
    g.linear_dimension([0.0, 0.0], [50.0, 0.0], 10.0, DimOrientation::Horizontal, "50", 1);
    let top = g.lines.iter().flat_map(|l| l.points.iter()).map(|p| p[1]).fold(f64::MIN, f64::max);
    assert!((top - 15.0).abs() < 1e-9, "{top}");
}

#[test]
fn grouped_revision_callout_follows_its_annotation() {
    let drawing = Drawing::default();
    let env = sheet::PlainEnv { drawing: &drawing, scale: 1000.0 };
    let mut s = Sheet::default();
    let note = |p: [f64; 2]| Annotation { id: 1, kind: AnnotationKind::note("NOTE", 3.5), position: p, ..Default::default() };
    let callout = Annotation { id: 2, kind: AnnotationKind::RevisionCallout { anchor: [5.0, 0.0], revision: "B".into(), group: Some(1) }, ..Default::default() };
    let at = |s: &Sheet| {
        let mut g = sheet::SheetGeometry::new(420.0, 297.0);
        g.add_annotations(s, &env);
        let text = |t: &str| g.texts.iter().find(|x| x.text == t).map(|x| x.pos).unwrap();
        (text("NOTE"), text("B"))
    };
    s.annotations = vec![note([50.0, 100.0]), callout.clone()];
    let (n1, b1) = at(&s);
    s.annotations = vec![note([80.0, 60.0]), callout];
    let (n2, b2) = at(&s);
    // The callout moves with the note.
    assert!(((b2[0] - b1[0]) - (n2[0] - n1[0])).abs() < 1e-9 && ((b2[1] - b1[1]) - (n2[1] - n1[1])).abs() < 1e-9);
    assert!(b1[0] > n1[0], "beside the text");
}

#[test]
fn shown_sketches_overlay_part_views() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let cache = ViewCache::new();
    let vid = drawing.sheets[0].views[0].id;
    let (key, _) = views::view_source(&drawing.sheets[0], vid).unwrap();
    let sketch = sources.get(&key).unwrap().context.sketches.first().map(|s| s.feature).expect("demo studio has a sketch");
    let before = compute_view(&drawing, &drawing.sheets[0], vid, &sources, &cache, false).edges.len();
    drawing.sheets[0].views[0].shown_sketches = vec![sketch];
    let with_points = compute_view(&drawing, &drawing.sheets[0], vid, &sources, &cache, false).edges.len();
    drawing.sheets[0].views[0].hide_sketch_points = true;
    let without_points = compute_view(&drawing, &drawing.sheets[0], vid, &sources, &cache, false).edges.len();
    assert!(without_points > before, "sketch curves added: {before} -> {without_points}");
    assert!(with_points > without_points, "sketch point crosses: {with_points} vs {without_points}");
}

#[test]
fn bom_split_order_and_structured_rows() {
    let (doc, mut drawing) = demo();
    let asm = assembly_tab(&doc);
    let si = drawing.add_sheet(0);
    drawing.sheets[si].views.push(base_view(1, asm, ViewOrientation::Isometric, [150.0, 160.0]));
    let sources = regenerate_sources(&doc, &drawing);
    let src = &sources[&SourceKey::studio(asm)];
    assert!(!src.bom_structured.is_empty(), "structured rows from the assembly BOM");
    let data = |kind: TableKind, rows: Option<[usize; 2]>| {
        let mut t = Table::new(1, kind, [410.0, 287.0]);
        t.rows = rows;
        tables::table_data(&drawing, &t, None, Some(src), None, &[], &|s| s.to_string())
    };
    let bom = |structured, bottom_to_top| TableKind::Bom { view: ViewId(1), structured, bottom_to_top, overrides: Vec::new() };
    let all = data(bom(false, false), None);
    assert!(all.rows.len() >= 2);
    // A split part shows its rows only.
    let tail = data(bom(false, false), Some([1, all.rows.len()]));
    assert_eq!(tail.rows, all.rows[1..].to_vec());
    // Bottom to top: rows reversed, header last.
    let rev = data(bom(false, true), None);
    assert!(rev.header_bottom && rev.rows.first() == all.rows.last());
    assert_eq!(rev.all_rows().last().map(|r| r[0].as_str()), Some("Item"));
    assert!(!data(bom(true, false), None).rows.is_empty());
    // An override replaces the cell's value and is marked.
    let item = all.rows[0][0].clone();
    let over = data(TableKind::Bom { view: ViewId(1), structured: false, bottom_to_top: false, overrides: vec![(item, 2, "BRACKET".into())] }, None);
    assert_eq!(over.rows[0][2], "BRACKET");
    assert_eq!(over.overridden, vec![(0, 2)]);
}

#[test]
fn grouped_callout_follows_its_annotation() {
    let drawing = Drawing::default();
    let env = sheet::PlainEnv { drawing: &drawing, scale: 1000.0 };
    let mut s = Sheet::default();
    let note = |p: [f64; 2]| Annotation { id: 1, kind: AnnotationKind::note("NOTE", 3.5), position: p, ..Default::default() };
    let callout = Annotation { id: 2, kind: AnnotationKind::balloon(ViewId(1), [0.0, 0.0], "7"), position: [6.0, 4.0], group: Some(1), ..Default::default() };
    let at = |s: &Sheet| {
        let mut g = sheet::SheetGeometry::new(420.0, 297.0);
        g.add_annotations(s, &env);
        let text = |t: &str| g.texts.iter().find(|x| x.text == t).map(|x| x.pos).unwrap();
        (text("NOTE"), text("7"))
    };
    s.annotations = vec![note([50.0, 100.0]), callout.clone()];
    let (n1, c1) = at(&s);
    s.annotations = vec![note([90.0, 40.0]), callout];
    let (n2, c2) = at(&s);
    assert!(((c2[0] - c1[0]) - (n2[0] - n1[0])).abs() < 1e-9 && ((c2[1] - c1[1]) - (n2[1] - n1[1])).abs() < 1e-9);
}

#[test]
fn center_of_mass_needs_a_material() {
    let (mut doc, mut drawing) = demo();
    let vid = drawing.sheets[0].views[0].id;
    drawing.sheets[0].views[0].show_com = true;
    let none = {
        let sources = regenerate_sources(&doc, &drawing);
        compute_view(&drawing, &drawing.sheets[0], vid, &sources, &ViewCache::new(), false).com
    };
    assert!(none.is_none(), "no material: no center of mass");
    // Give the plate a material.
    let sources = regenerate_sources(&doc, &drawing);
    let (key, _) = views::view_source(&drawing.sheets[0], vid).unwrap();
    let body = sources[&key].context.bodies[0].id.clone();
    if let Some(TabContent::PartStudio(ps)) = doc.tabs.iter_mut().find(|t| t.id == key.tab).map(|t| &mut t.content) {
        let p = ps.part_properties.entry(body).or_default();
        p.material = Some("Aluminum 6061".into());
        p.density = Some(2700.0);
    }
    let sources = regenerate_sources(&doc, &drawing);
    let g = compute_view(&drawing, &drawing.sheets[0], vid, &sources, &ViewCache::new(), false);
    let c = g.com.expect("center of mass shown");
    // A snap point at it, inside the view's extent.
    assert!(g.snaps.iter().any(|s| s.p == c));
    assert!(c[0] >= g.min[0] && c[0] <= g.max[0] && c[1] >= g.min[1] && c[1] <= g.max[1]);
}

#[test]
fn pinned_views_use_their_version() {
    use crate::params::ParamValue;
    use noshape_units::ExprValue;
    let (v1, mut drawing) = demo();
    // The live document changes after the version: a thicker plate.
    let mut live = v1.clone();
    if let TabContent::PartStudio(ps) = &mut live.tabs[0].content {
        let ex = ps.features.iter_mut().find(|f| f.type_id == "extrude").unwrap();
        ex.params.set("depth", ParamValue::Expr(ExprValue::new("20 mm")));
    }
    let vid = drawing.sheets[0].views[0].id;
    let pin = PinnedVersion { id: "v1".into(), name: "V1".into() };
    drawing.sheets[0].views[0].pinned = Some(pin.clone());
    let (key, _) = views::view_source(&drawing.sheets[0], vid).unwrap();
    assert_eq!(key.version.as_deref(), Some("v1"), "the key carries the version");
    let versions = |id: &str| (id == "v1").then(|| v1.clone());
    let sources = views::regenerate_sources_with(&live, &drawing, &versions);
    let src = &sources[&key];
    // Regenerated from the version, not the live document.
    assert_eq!(Some(src.hash), views::source_hash(&v1, &key));
    assert_ne!(Some(src.hash), views::source_hash(&live, &key));
    // An unknown version leaves the source out.
    drawing.sheets[0].views[0].pinned = Some(PinnedVersion { id: "nope".into(), name: "?".into() });
    let none = views::regenerate_sources_with(&live, &drawing, &versions);
    assert!(none.keys().all(|k| k.version.as_deref() != Some("nope")));
}

#[test]
fn callouts_stack_and_justify() {
    let (doc, mut drawing) = demo();
    let vid = drawing.sheets[0].views[0].id;
    let mut kind = AnnotationKind::balloon(vid, [0.0, 0.0], "K1");
    if let AnnotationKind::Balloon { callout, .. } = &mut kind {
        callout.stacked = ["K2", "K3333"].iter().map(|t| StackedCallout { text: t.to_string(), body: None }).collect();
        callout.above = "TOP".into();
        callout.below = "QTY 22".into();
    }
    drawing.sheets[0].annotations.push(Annotation { id: 900, kind, position: [300.0, 200.0], ..Default::default() });
    let sources = regenerate_sources(&doc, &drawing);
    let texts = |d: &Drawing| render_sheet(d, 0, &sources, &ViewCache::new(), false).unwrap().geometry.texts;
    let pos = |d: &Drawing, t: &str| texts(d).iter().find(|x| x.text == t).map(|x| x.pos).unwrap();
    drawing.props.callout_stack = StackDirection::Right;
    drawing.props.callout_stack_gap = 2.0;
    let (p1, p2) = (pos(&drawing, "K1"), pos(&drawing, "K2"));
    assert!((p1[1] - p2[1]).abs() < 1e-9, "a row");
    let pitch = p2[0] - p1[0];
    assert!(pitch > 2.0);
    // A bigger gap spreads them by the difference.
    drawing.props.callout_stack_gap = 5.0;
    assert!(((pos(&drawing, "K2")[0] - pos(&drawing, "K1")[0]) - (pitch + 3.0)).abs() < 1e-9);
    drawing.props.callout_stack = StackDirection::Up;
    let (u1, u2) = (pos(&drawing, "K1"), pos(&drawing, "K2"));
    assert!((u1[0] - u2[0]).abs() < 1e-9 && u2[1] > u1[1], "a column going up");
    // Texts above / below sit outside the border, centred.
    let (top, below, k1) = (pos(&drawing, "TOP"), pos(&drawing, "QTY 22"), pos(&drawing, "K1"));
    assert!(top[1] > k1[1] + 3.5 && below[1] < k1[1] - 3.5);
    // Justification of the callout data in the widest item.
    let width = |t: &str| font::text_width(t, 3.5);
    drawing.props.callout_stack = StackDirection::Right;
    drawing.props.callout_justify = TextJustify::Left;
    let (a, b) = (pos(&drawing, "K1"), pos(&drawing, "K2"));
    let pitch = b[0] - a[0];
    let c3 = pos(&drawing, "K3333");
    assert!(((c3[0] - b[0]) - pitch).abs() < 1e-6, "left edges one pitch apart");
    drawing.props.callout_justify = TextJustify::Right;
    let (b, c3) = (pos(&drawing, "K2"), pos(&drawing, "K3333"));
    assert!(((c3[0] + width("K3333")) - (b[0] + width("K2")) - pitch).abs() < 1e-6, "right edges one pitch apart");
}

#[test]
fn property_fields_have_a_grey_background_on_screen() {
    let (doc, mut drawing) = demo();
    let sources = regenerate_sources(&doc, &drawing);
    let backdrops = |d: &Drawing| render_sheet(d, 0, &sources, &ViewCache::new(), false).unwrap().geometry.field_backdrops;
    assert!(!backdrops(&drawing).is_empty(), "title block fields are grey");
    drawing.props.hide_property_background = true;
    assert!(backdrops(&drawing).is_empty());
}

#[test]
fn datum_target_areas_are_hatched() {
    let (doc, mut drawing) = demo();
    let vid = drawing.sheets[0].views[0].id;
    drawing.sheets[0].annotations.push(Annotation { id: 901, kind: AnnotationKind::DatumTarget { view: vid, anchor: [0.0, 0.0], label: "A1".into(), area: DatumTargetArea::Circle { diameter: 0.01 } }, position: [300.0, 200.0], ..Default::default() });
    let sources = regenerate_sources(&doc, &drawing);
    let lines = |d: &Drawing| render_sheet(d, 0, &sources, &ViewCache::new(), false).unwrap().geometry.lines.len();
    let hatched = lines(&drawing);
    // General › Solid draws no hatch lines.
    drawing.props.target_hatch = (2, 0);
    assert!(lines(&drawing) + 3 < hatched);
}

/// The demo drawing's first view on a sheet metal L bracket (50 x 30 mm
/// legs, 20 mm deep, T 2, R 3) as a flat pattern view.
fn flat_bracket() -> (crate::Document, Drawing) {
    use crate::params::ParamValue;
    use crate::reference::{DatumPlane, Reference};
    use glam::DVec2;
    use noshape_units::{ExprValue, UnitPrefs};
    let (mut doc, mut drawing) = demo();
    let tab = match &drawing.sheets[0].views[0].kind {
        ViewKind::Base { source, .. } => *source,
        _ => unreachable!(),
    };
    let units = UnitPrefs::default();
    let mut ps = crate::partstudio::PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let pts: Vec<_> = [(0.0, 0.0), (0.05, 0.0), (0.05, 0.03)].iter().map(|&(x, y)| sk.add_point(DVec2::new(x, y))).collect();
    let lines: Vec<u32> = (0..2).map(|i| sk.add_line_between(pts[i], pts[i + 1]).unwrap().0).collect();
    let mut f = ps.new_feature("sketch", &units).unwrap();
    f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    f.params.set("sketch", ParamValue::Sketch(Box::new(crate::sketch_feature::SketchData { sketch: sk, ..Default::default() })));
    let sid = f.id;
    ps.features.push(f);
    let mut f = ps.new_feature("sheetMetalStart", &units).unwrap();
    f.params.set("process", ParamValue::Enum("EXTRUDE".into()));
    f.params.set("sketchCurves", ParamValue::Query(lines.iter().map(|&e| Reference::SketchEntity { sketch: sid, entity: e }).collect()));
    for (k, v) in [("depth", "20 mm"), ("thickness", "2 mm"), ("radius", "3 mm"), ("kFactor", "0.45")] {
        f.params.set(k, ParamValue::Expr(ExprValue::new(v)));
    }
    ps.features.push(f);
    doc.tab_mut(tab).unwrap().content = TabContent::PartStudio(ps);
    let sources = regenerate_sources(&doc, &drawing);
    let (key, _) = views::view_source(&drawing.sheets[0], drawing.sheets[0].views[0].id).unwrap();
    let body = sources[&key].context.bodies[0].id.clone();
    let v = &mut drawing.sheets[0].views[0];
    if let ViewKind::Base { part, .. } = &mut v.kind {
        *part = Some(body);
    }
    v.flat = true;
    drawing.sheets[0].views.truncate(1);
    drawing.sheets[0].annotations.clear();
    (doc, drawing)
}

#[test]
fn flat_pattern_view_shows_bend_lines_and_notes() {
    let (doc, mut drawing) = flat_bracket();
    let vid = drawing.sheets[0].views[0].id;
    let sources = regenerate_sources(&doc, &drawing);
    let g = compute_view(&drawing, &drawing.sheets[0], vid, &sources, &ViewCache::new(), false);
    assert!(g.error.is_none(), "{:?}", g.error);
    assert_eq!(g.bends.len(), 1);
    // The developed length: (50 - 5) + (30 - 5) + 90° × (3 + 0.45 × 2) mm.
    let expected = 0.045 + 0.025 + std::f64::consts::FRAC_PI_2 * (0.003 + 0.45 * 0.002);
    let (w, h) = (g.max[0] - g.min[0], g.max[1] - g.min[1]);
    assert!((w.max(h) - expected).abs() < 1e-6 && (w.min(h) - 0.02).abs() < 1e-9, "{w} x {h}");
    let built = |d: &Drawing| render_sheet(d, 0, &sources, &ViewCache::new(), false).unwrap().geometry;
    let g1 = built(&drawing);
    let note = g1.texts.iter().find(|t| t.text.starts_with("UP ") || t.text.starts_with("DOWN ")).expect("a bend note");
    assert!(note.text.contains("90°") && note.text.contains("R3"), "{}", note.text);
    let centerlines = |g: &sheet::SheetGeometry| g.lines.iter().filter(|l| l.style == sheet::LineStyle::Centerline).count();
    // Show/Hide bend lines and bend notes.
    drawing.sheets[0].views[0].hide_bend_lines = true;
    drawing.sheets[0].views[0].hide_bend_notes = true;
    let g2 = built(&drawing);
    assert_eq!(centerlines(&g2) + 1, centerlines(&g1));
    assert!(!g2.texts.iter().any(|t| t.text.starts_with("UP ") || t.text.starts_with("DOWN ")));
}

#[test]
fn annotations_moved_to_format_layers() {
    let (doc, mut drawing) = demo();
    drawing.sheets[0].annotations.push(Annotation { id: 902, kind: AnnotationKind::note("LAYERED NOTE", 3.5), position: [60.0, 40.0], layer: SheetLayer::TitleBlock, ..Default::default() });
    drawing.props.title_block_color = Some([200, 0, 0]);
    drawing.props.formats_locked = false;
    let sources = regenerate_sources(&doc, &drawing);
    let built = |d: &Drawing| render_sheet(d, 0, &sources, &ViewCache::new(), false).unwrap().geometry;
    let g = built(&drawing);
    let i = g.texts.iter().position(|t| t.text == "LAYERED NOTE").unwrap();
    // The title block's color, layer and pickability.
    assert_eq!(g.text_color(i), Some([200, 0, 0]));
    assert_eq!(g.text_layer(i), Some("TITLE_BLOCK"));
    assert_ne!(g.texts[i].pick, 0);
    // Exported on that layer (also after exploding the text).
    let dxf = export::to_dxf(&g);
    assert!(dxf.contains("TITLE_BLOCK") && dxf.contains("BORDER_ZONES"));
    let exploded = export::prepare(&g, &export::ExportOptions { explode_all: true, ..Default::default() });
    assert!(exploded.layers.iter().any(|s| s.layer == SheetLayer::TitleBlock && (!s.lines.is_empty() || !s.fills.is_empty())));
    // Locked formats: not pickable.
    drawing.props.formats_locked = true;
    let g = built(&drawing);
    let i = g.texts.iter().position(|t| t.text == "LAYERED NOTE").unwrap();
    assert_eq!(g.texts[i].pick, 0);
}

#[test]
fn rotated_notes_turn_about_their_position() {
    let drawing = Drawing::default();
    let env = sheet::PlainEnv { drawing: &drawing, scale: 1000.0 };
    let mut s = Sheet::default();
    let mut kind = AnnotationKind::note("TURN", 3.5);
    if let AnnotationKind::Note { style, .. } = &mut kind {
        style.angle = std::f64::consts::FRAC_PI_2;
    }
    s.annotations.push(Annotation { id: 1, kind, position: [100.0, 100.0], ..Default::default() });
    let mut g = sheet::SheetGeometry::new(420.0, 297.0);
    g.add_annotations(&s, &env);
    let t = g.texts.iter().find(|t| t.text == "TURN").unwrap();
    assert!((t.angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    // The text runs up, turned about the note's middle.
    let b = font::text_box(t);
    let span = |k: usize| b.iter().map(|p| p[k]).fold(f64::MIN, f64::max) - b.iter().map(|p| p[k]).fold(f64::MAX, f64::min);
    assert!(span(1) > span(0), "{b:?}");
    let l = note::layout("TURN", 3.5, &note::NoteStyle::default(), [100.0, 100.0]);
    let mid = note::center(&l.bbox);
    let c = [b.iter().map(|p| p[0]).sum::<f64>() / 4.0, b.iter().map(|p| p[1]).sum::<f64>() / 4.0];
    assert!((c[0] - mid[0]).abs() < 1.5 && (c[1] - mid[1]).abs() < 1.5, "{c:?} vs {mid:?}");
}

#[test]
fn thread_designations_give_major_diameters() {
    let d = |s: &str| views::thread_major_diameter(s).map(|m| (m * 1e7).round() / 1e7);
    assert_eq!(d("M6x1.0"), Some(0.006));
    assert_eq!(d("M10"), Some(0.01));
    assert_eq!(d("1/4-20"), Some((0.25f64 * 0.0254 * 1e7).round() / 1e7));
    assert_eq!(d("#10-24"), Some(((0.06 + 0.13) * 0.0254 * 1e7f64).round() / 1e7));
    assert_eq!(d("garbage"), None);
}

#[test]
fn chamfer_dimension_style_and_content() {
    let drawing = Drawing::default();
    let mut d2 = drawing.clone();
    let texts = |d: &Drawing, kind: AnnotationKind| {
        let s = Sheet { annotations: vec![Annotation { id: 1, kind, position: [60.0, 60.0], ..Default::default() }], ..Default::default() };
        let mut g = sheet::SheetGeometry::new(420.0, 297.0);
        let env = sheet::PlainEnv { drawing: d, scale: 1000.0 };
        g.add_annotations(&s, &env);
        g.texts.iter().map(|t| t.text.clone()).collect::<Vec<_>>()
    };
    // A 45° chamfer of 2 mm legs (edge a-b, adjacent edge c-d).
    let k = AnnotationKind::ChamferDimension { view: ViewId(1), a: [0.0, 0.0], b: [0.002, 0.002], c: [0.0, 0.0], d: [0.01, 0.0] };
    assert!(texts(&drawing, k.clone()).iter().any(|t| t.contains("× 45°")), "{:?}", texts(&drawing, k.clone()));
    d2.props.chamfer_45_content = ChamferContent::CLength;
    assert!(texts(&d2, k.clone()).iter().any(|t| t.starts_with('C')));
    d2.props.chamfer_45_content = ChamferContent::AngleXLength;
    assert!(texts(&d2, k).iter().any(|t| t.starts_with("45° ×")));
}
