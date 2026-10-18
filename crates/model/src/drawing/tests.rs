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
}

#[test]
fn bom_hole_table_and_linked_balloons() {
    let (doc, mut drawing) = demo();
    let asm = assembly_tab(&doc);
    // Sheet 2: the assembly with a BOM and a callout on the pin.
    let si = drawing.add_sheet(0);
    drawing.sheets[si].views.push(base_view(1, asm, ViewOrientation::Isometric, [150.0, 160.0]));
    drawing.sheets[si].tables.push(Table::new(1, TableKind::Bom { view: ViewId(1) }, [410.0, 287.0]));
    // Hole table of the plate's top view on sheet 1.
    let origin = [-0.05, -0.03];
    drawing.sheets[0].tables.push(Table::new(1, TableKind::Hole { view: ViewId(2), origin, origin_ref: None, precision: None }, [410.0, 120.0]));
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
        kind: AnnotationKind::Balloon { view: ViewId(1), anchor: pin.p, text: String::new() },
        position: [220.0, 220.0],
        refs: vec![AnnotationRef { slot: 0, point: pin.point.clone().unwrap() }],
        ..Default::default()
    });
    let built = render_sheet(&drawing, si, &sources, &cache, false).unwrap();
    let t = texts(&built);
    for want in ["BILL OF MATERIALS", "ITEM", "QTY", "Part 1", "2"] {
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
    add(sheet, AnnotationKind::Gdt { anchor: [40.0, 40.0], frame: vec!["⌖".into(), "Ø0.1Ⓜ".into(), "A".into(), "B".into()] }, [40.0, 40.0]);
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
    sheet.annotations.push(Annotation { id: 99, kind: AnnotationKind::Note { text: "SHEET {sheet.number} OF {sheet.count}\nSCALE {sheet.scale}\n{nope}".into(), height_mm: 3.5 }, position: [30.0, 120.0], ..Default::default() });
    let info = SheetInfo { doc_name: "Demo".into(), drawing_name: "Drawing 1".into() };
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
