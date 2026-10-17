//! Sample documents (used by `noshape --demo` and tests).

use crate::configuration::ConfigValues;
use crate::document::{Document, TabContent};
use crate::params::ParamValue;
use crate::partstudio::PartStudio;
use crate::reference::{DatumPlane, Reference};
use crate::regen::{RegenOptions, Regenerator};
use crate::sketch_feature::SketchData;
use glam::DVec2;
use noshape_units::{ExprValue, UnitPrefs};

fn plate_sketch(w: f64, h: f64, r: f64) -> SketchData {
    let mut sk = noshape_sketch::Sketch::new();
    let p = [DVec2::new(-w / 2.0, -h / 2.0), DVec2::new(w / 2.0, -h / 2.0), DVec2::new(w / 2.0, h / 2.0), DVec2::new(-w / 2.0, h / 2.0)];
    let pts: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
    for i in 0..4 {
        let _ = sk.add_line_between(pts[i], pts[(i + 1) % 4]);
    }
    sk.add_circle(DVec2::new(0.0, 0.0), r);
    sk.add_circle(DVec2::new(-w / 2.0 + 0.012, -h / 2.0 + 0.012), r * 0.4);
    sk.add_circle(DVec2::new(w / 2.0 - 0.012, h / 2.0 - 0.012), r * 0.4);
    SketchData { sketch: sk, ..Default::default() }
}

/// A plate with holes, extruded and filleted.
pub fn plate_studio() -> PartStudio {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(plate_sketch(0.1, 0.06, 0.012))));
    let sid = sketch.id;
    ps.features.push(sketch);
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let outer = r.display.sketches[0].regions.iter().max_by(|a, b| a.area.partial_cmp(&b.area).unwrap()).unwrap().key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key: outer }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new("12 mm")));
    ps.features.push(ex);
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let body = &r.context.bodies[0];
    let top: Vec<Reference> = body.edges.iter().filter(|e| format!("{e:?}").contains("CapEnd")).map(|e| Reference::Edge((**e).clone())).collect();
    let mut f = ps.new_feature("fillet", &units).unwrap();
    f.params.set("entities", ParamValue::Query(top));
    f.params.set("radius", ParamValue::Expr(ExprValue::new("2 mm")));
    ps.features.push(f);
    ps
}

pub fn demo_document() -> Document {
    let mut d = Document::new();
    if let TabContent::PartStudio(ps) = &mut d.tabs[0].content {
        *ps = plate_studio();
    }
    add_demo_assembly(&mut d);
    d
}

/// A pin (cylinder, radius 12 mm, 40 mm long) centered on the origin.
pub fn pin_studio() -> PartStudio {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, 0.012);
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let key = r.display.sketches[0].regions[0].key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new("40 mm")));
    ex.params.set("symmetric", ParamValue::Bool(true));
    ps.features.push(ex);
    ps
}

/// Fill the document's first Assembly tab with a demo assembly: two plates
/// (the second fastened on top of the first, turned 90°) and a pin in a
/// revolute mate through the center holes. Adds a "Pin" Part Studio.
pub fn add_demo_assembly(doc: &mut Document) {
    use crate::assembly::eval::AssemblyEvaluator;
    use crate::assembly::*;
    use crate::reference::Inference;
    let Some(plate_tab) = doc.tabs.iter().find(|t| matches!(t.content, TabContent::PartStudio(_))).map(|t| t.id) else { return };
    let pin_tab = doc.add_tab("Pin", TabContent::PartStudio(pin_studio()));
    let mut ev = AssemblyEvaluator::new();
    let cfg = ConfigValues::default();
    let (Some(plate), Some(pin)) = (ev.studio(doc, plate_tab, &cfg), ev.studio(doc, pin_tab, &cfg)) else { return };
    let (Some(pb), Some(qb)) = (plate.context.bodies.first(), pin.context.bodies.first()) else { return };
    let face_named = |b: &crate::regen::context::Body, needle: &str| b.faces.iter().find(|f| format!("{f:?}").contains(needle)).map(|f| Reference::Face((**f).clone()));
    let cylinder = |b: &crate::regen::context::Body, radius: f64| {
        b.faces.iter().enumerate().find_map(|(i, f)| {
            let face = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok()?;
            match face.face_surface().ok()? {
                noshape_kernel::Surface::Cylinder { radius: r, .. } if (r - radius).abs() < 1e-6 => Some(Reference::Face((**f).clone())),
                _ => None,
            }
        })
    };
    let (Some(top), Some(bottom), Some(hole), Some(pin_face)) = (face_named(pb, "CapEnd"), face_named(pb, "CapStart"), cylinder(pb, 0.012), cylinder(qb, 0.012)) else { return };
    let mc = |instance: u64, on: Reference, inference: Inference| MateConnectorRef { instance: InstanceId(instance), path: vec![], reference: Reference::Implicit { on: Box::new(on), inference }, flip: false, rotate_quarters: 0 };
    let inst = |id: u64, name: &str, studio, body: crate::reference::BodyId, at: [f64; 3], fixed| Instance {
        id: InstanceId(id),
        name: name.into(),
        source: InstanceSource::Part { studio, body, config: ConfigValues::default() },
        placement: Placement { translation: at, rotation: [0.0, 0.0, 0.0, 1.0] },
        fixed,
        hidden: false,
        suppressed: false,
    };
    let mate = |id: u64, name: &str, t: MateType, a, b| Mate {
        id: MateId(id),
        name: name.into(),
        mate_type: t,
        a,
        b,
        offset_z: None,
        offset_x: None,
        offset_y: None,
        rotation: None,
        flip_primary: false,
        reorient_secondary: 0,
        limits: vec![],
        suppressed: false,
    };
    let mut fastened = mate(4, "Fastened 1", MateType::Fastened, mc(1, top, Inference::Centroid), mc(2, bottom, Inference::Centroid));
    fastened.reorient_secondary = 1;
    let asm = Assembly {
        instances: vec![
            inst(1, "Part 1 <1>", plate_tab, pb.id.clone(), [0.0; 3], true),
            inst(2, "Part 1 <2>", plate_tab, pb.id.clone(), [0.0, 0.09, 0.0], false),
            inst(3, "Part 1 <1>", pin_tab, qb.id.clone(), [0.08, 0.0, 0.0], false),
        ],
        mates: vec![fastened, mate(5, "Revolute 1", MateType::Revolute, mc(1, hole, Inference::MidAxis), mc(3, pin_face, Inference::MidAxis))],
        ..Default::default()
    };
    let mut asm = asm;
    asm.instances[2].name = "Pin <1>".into();
    // Store solved positions so the document opens assembled.
    let res = ev.evaluate(doc, &asm, &eval::SolveOptions::default(), None);
    for (id, p) in &res.solved.placements {
        if let Some(i) = asm.instance_mut(*id) {
            i.placement = *p;
        }
    }
    if let Some(t) = doc.tabs.iter_mut().find(|t| matches!(t.content, TabContent::Assembly(_))) {
        t.content = TabContent::Assembly(asm);
    }
}

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
        sheet.views.push(view(5, ViewKind::Section { parent: ViewId(2), line: vec![[0.0, -0.04], [0.0, 0.04]], label: "A".into() }, [230.0, 215.0]));
        let mut detail = view(6, ViewKind::Detail { parent: ViewId(2), center: [0.038, 0.018], radius: 0.011, label: "B".into() }, [335.0, 115.0]);
        detail.scale = Some(2.0);
        sheet.views.push(detail);
        for (k, v) in [("Title", "Plate"), ("Drawn by", "noshape"), ("Part number", "P-001"), ("Revision", "A"), ("Material", "Aluminum 6061"), ("Sheet", "1 / 1")] {
            if let Some(f) = sheet.title_block.fields.iter_mut().find(|(key, _)| key == k) {
                f.1 = v.to_string();
            }
        }
    }
    let sources = regenerate_sources(&d, &drawing);
    let ctx = sources.get(&studio).map(|s| s.context.clone());
    let sheet = &mut drawing.sheets[0];
    let mut next = 1u64;
    let mut add = |sheet: &mut Sheet, kind: AnnotationKind, position: [f64; 2], refs: Vec<(u8, Option<PointRef>)>| {
        let refs = refs.into_iter().filter_map(|(slot, p)| p.map(|point| AnnotationRef { slot, point })).collect();
        sheet.annotations.push(Annotation { id: next, kind, position, refs });
        next += 1;
    };
    let v = |p: DVec3| ctx.as_ref().and_then(|c| vertex_ref_near(c, p, 1e-6));
    let c = |p: DVec3, largest: bool| ctx.as_ref().and_then(|cx| circle_ref_near(cx, p, 1e-6, largest));
    let lin = |view: u64, a: [f64; 2], b: [f64; 2], offset: f64, orientation: DimOrientation| AnnotationKind::LinearDimension { view: ViewId(view), a, b, offset, orientation, override_text: None };
    // Top view: overall width and depth.
    add(sheet, lin(2, [-0.05, 0.03], [0.05, 0.03], 8.0, DimOrientation::Horizontal), [0.0; 2], vec![(0, v(DVec3::new(-0.05, 0.03, 0.0))), (1, v(DVec3::new(0.05, 0.03, 0.0)))]);
    add(sheet, lin(2, [-0.05, -0.03], [-0.05, 0.03], 8.0, DimOrientation::Vertical), [0.0; 2], vec![(0, v(DVec3::new(-0.05, -0.03, 0.0))), (1, v(DVec3::new(-0.05, 0.03, 0.0)))]);
    // Hole sizes.
    add(sheet, AnnotationKind::RadialDimension { view: ViewId(2), center: [0.0, 0.0], radius: 0.012, diameter: true, angle: 2.4 }, [0.0; 2], vec![(0, c(DVec3::ZERO, false))]);
    add(sheet, AnnotationKind::RadialDimension { view: ViewId(6), center: [0.038, 0.018], radius: 0.0048, diameter: false, angle: 0.9 }, [0.0; 2], vec![(0, c(DVec3::new(0.038, 0.018, 0.0), false))]);
    // Front view: width and thickness.
    add(sheet, lin(1, [-0.05, 0.0], [0.05, 0.0], -10.0, DimOrientation::Horizontal), [0.0; 2], vec![(0, v(DVec3::new(-0.05, -0.03, 0.0))), (1, v(DVec3::new(0.05, -0.03, 0.0)))]);
    add(sheet, lin(1, [-0.05, 0.0], [-0.05, 0.012], 8.0, DimOrientation::Vertical), [0.0; 2], vec![(0, v(DVec3::new(-0.05, -0.03, 0.0)))]);
    add(sheet, AnnotationKind::Balloon { view: ViewId(4), anchor: [0.0, 0.0], text: "1".into() }, [385.0, 255.0], vec![(0, v(DVec3::new(0.05, -0.03, 0.0)))]);
    add(sheet, AnnotationKind::Note { text: "NOTES:\n1. BREAK ALL SHARP EDGES.\n2. DIMENSIONS IN MILLIMETERS.".into(), height_mm: 3.5 }, [25.0, 75.0], vec![]);
    let _ = d.add_tab("Drawing 1", TabContent::Drawing(drawing));
    d
}
