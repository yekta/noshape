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
    let ctx = sources[&studio_tab].context.clone();
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
