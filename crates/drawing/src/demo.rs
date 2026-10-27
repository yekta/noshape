//! Demo documents with drawings (they build drawing views).

use noshape_model_core::configuration::ConfigValues;
pub use noshape_assembly::demo::*;
use noshape_model_core::document::{Document, TabContent};

/// The demo document plus a Drawing tab of the plate: front/top/right and
/// isometric views, a section and a detail view, associative dimensions,
/// centermarks, a note, a callout and a filled-in title block.
pub fn demo_document_with_drawing() -> Document {
    use crate::drawing::views::{circle_ref_near, regenerate_sources, vertex_ref_near};
    use crate::drawing::*;
    use glam::DVec3;
    let mut d = demo_document();
    let studio = d.tabs[0].id;
    let mut drawing = Drawing::default();
    let view = |id: u64, kind: ViewKind, position: [f64; 2]| DrawingView {
        id: ViewId(id),
        kind,
        position,
        scale: None,
        style: RenderStyle::HiddenLinesRemoved,
        show_tangent_edges: true,
        show_centerlines: false,
        label: None,
        ..Default::default()
    };
    {
        let sheet = &mut drawing.sheets[0];
        sheet.views.push(view(1, ViewKind::Base { source: studio, config: ConfigValues::default(), orientation: ViewOrientation::Front, part: None }, [110.0, 150.0]));
        let mut top = view(2, ViewKind::Projected { parent: ViewId(1), direction: Some([0.0, 1.0]) }, [110.0, 215.0]);
        top.show_centerlines = true;
        sheet.views.push(top);
        let mut right = view(3, ViewKind::Projected { parent: ViewId(1), direction: Some([1.0, 0.0]) }, [230.0, 150.0]);
        right.style = RenderStyle::HiddenLinesVisible;
        sheet.views.push(right);
        sheet.views.push(view(4, ViewKind::Projected { parent: ViewId(1), direction: Some([1.0, 1.0]) }, [335.0, 215.0]));
        sheet.views.push(view(5, ViewKind::Section { parent: ViewId(2), line: vec![[0.0, -0.04], [0.0, 0.04]], label: "A".into(), cuts: Vec::new() }, [230.0, 215.0]));
        let mut detail = view(6, ViewKind::Detail { parent: ViewId(2), center: [0.038, 0.018], radius: 0.011, label: "B".into() }, [335.0, 115.0]);
        detail.scale = Some(2.0);
        sheet.views.push(detail);
        for (k, v) in [("Title", "Plate"), ("Drawn by", "noshape"), ("Part number", "P-001"), ("Revision", "A"), ("Material", "Aluminum 6061"), ("Sheet", "{sheet.number} / {sheet.count}")] {
            if let Some(f) = sheet.title_block.fields.iter_mut().find(|(key, _)| key == k) {
                f.1 = v.to_string();
            }
        }
    }
    let sources = regenerate_sources(&d, &drawing);
    let ctx = sources.get(&crate::drawing::views::SourceKey::studio(studio)).map(|s| s.context.clone());
    let sheet = &mut drawing.sheets[0];
    let mut next = 1u64;
    let mut add = |sheet: &mut Sheet, kind: AnnotationKind, position: [f64; 2], refs: Vec<(u8, Option<PointRef>)>| {
        let refs = refs.into_iter().filter_map(|(slot, p)| p.map(|point| AnnotationRef { slot, point })).collect();
        sheet.annotations.push(Annotation { id: next, kind, position, refs, ..Default::default() });
        next += 1;
    };
    let v = |p: DVec3| ctx.as_ref().and_then(|c| vertex_ref_near(c, p, 1e-6));
    let c = |p: DVec3, largest: bool| ctx.as_ref().and_then(|cx| circle_ref_near(cx, p, 1e-6, largest));
    let lin = |view: u64, a: [f64; 2], b: [f64; 2], offset: f64, orientation: DimOrientation| AnnotationKind::LinearDimension { view: ViewId(view), a, b, offset, orientation, override_text: None };
    // Top view: overall width and depth.
    add(sheet, lin(2, [-0.05, 0.03], [0.05, 0.03], 8.0, DimOrientation::Horizontal), [0.0; 2], vec![(0, v(DVec3::new(-0.05, 0.03, 0.0))), (1, v(DVec3::new(0.05, 0.03, 0.0)))]);
    add(sheet, lin(2, [-0.05, -0.03], [-0.05, 0.03], -8.0, DimOrientation::Vertical), [0.0; 2], vec![(0, v(DVec3::new(-0.05, -0.03, 0.0))), (1, v(DVec3::new(-0.05, 0.03, 0.0)))]);
    // Hole sizes.
    add(sheet, AnnotationKind::RadialDimension { view: ViewId(2), center: [0.0, 0.0], radius: 0.012, diameter: true, angle: 2.4, dim_line: false, knee: None }, [0.0; 2], vec![(0, c(DVec3::ZERO, false))]);
    add(sheet, AnnotationKind::RadialDimension { view: ViewId(6), center: [0.038, 0.018], radius: 0.0048, diameter: false, angle: 0.9, dim_line: false, knee: None }, [0.0; 2], vec![(0, c(DVec3::new(0.038, 0.018, 0.0), false))]);
    // Front view: width and thickness.
    add(sheet, lin(1, [-0.05, 0.0], [0.05, 0.0], -10.0, DimOrientation::Horizontal), [0.0; 2], vec![(0, v(DVec3::new(-0.05, -0.03, 0.0))), (1, v(DVec3::new(0.05, -0.03, 0.0)))]);
    add(sheet, lin(1, [-0.05, 0.0], [-0.05, 0.012], -8.0, DimOrientation::Vertical), [0.0; 2], vec![(0, v(DVec3::new(-0.05, -0.03, 0.0)))]);
    add(sheet, AnnotationKind::balloon(ViewId(4), [0.0, 0.0], "1"), [385.0, 255.0], vec![(0, v(DVec3::new(0.05, -0.03, 0.0)))]);
    add(sheet, AnnotationKind::note("NOTES:\n1. BREAK ALL SHARP EDGES.\n2. DIMENSIONS IN MILLIMETERS.", 3.5), [25.0, 75.0], vec![]);
    let _ = d.add_tab("Drawing 1", TabContent::Drawing(drawing));
    d
}

/// The live capture's Feature Studio "fs tables" (FS sizes, FS two tables;
/// behaviours.md "Custom tables (defineTable)").
pub const FS_TABLES: &str = r#"FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

annotation { "Table Type Name" : "FS sizes", "Table Type Description" : "Sizes of the parts" }
export const fsSizes = defineTable(function(context is Context, definition is map) returns Table
    precondition
    {
    }
    {
        const columns = [tableColumnDefinition("name", "Part"), tableColumnDefinition("faces", "Faces", TableTextAlignment.RIGHT), tableColumnDefinition("x", "Width"), tableColumnDefinition("misc", "Misc")];
        var rows = [];
        for (var body in evaluateQuery(context, qAllModifiableSolidBodies()))
        {
            const bb = evBox3d(context, { "topology" : body });
            const faces = size(evaluateQuery(context, qOwnedByBody(body, EntityType.FACE)));
            rows = append(rows, tableRow({ "name" : templateString({ "template" : "Body #n (#w)", "n" : faces, "w" : bb.maxCorner[0] - bb.minCorner[0] }), "faces" : faces, "x" : bb.maxCorner[0] - bb.minCorner[0], "misc" : 1 / 3 }, body));
        }
        rows = append(rows, tableRow({ "name" : tableCellError("?", "no body"), "faces" : tableCellWithInfo(0, "an info"), "x" : valueWithUnitsAndPrecision(25.4 * millimeter, 4), "misc" : 30 * degree }));
        rows = append(rows, tableRow({ "name" : "area", "x" : 2 * inch * inch, "misc" : 1.5 * meter ^ 3 }));
        println("table ran");
        return table("Sizes", columns, rows);
    });

annotation { "Table Type Name" : "FS two tables" }
export const fsTwo = defineTable(function(context is Context, definition is map) returns TableArray
    precondition
    {
    }
    {
        return tableArray([table("First", [tableColumnDefinition("a", "A")], [tableRow({ "a" : "one" })]), table("Second", [tableColumnDefinition("b", "B")], [])]);
    });
"#;

/// The demo document with a drawing plus the "fs tables" Feature Studio
/// (Part Studio Custom tables panel, drawing custom tables).
pub fn custom_tables_document() -> Document {
    let mut doc = demo_document_with_drawing();
    doc.add_tab("fs tables", TabContent::FeatureStudio(noshape_model_core::custom::FeatureStudio::new(FS_TABLES)));
    doc
}
