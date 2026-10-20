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
    add_demo_feature_studio(&mut d);
    d
}

/// Source of the demo Feature Studio: a sketch-free "Flange" and a
/// "Bolt circle" that cuts holes into a selected planar face.
pub const DEMO_FEATURE_STUDIO: &str = r#"FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

annotation { "Feature Type Name" : "Flange", "Feature Type Description" : "Round flange with a hub, a bore, a bolt circle and rounded edges" }
export const flange = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Diameter" }
        isLength(definition.diameter, { (millimeter) : [1, 80, 10000], (inch) : 3 } as LengthBoundSpec);
        annotation { "Name" : "Thickness" }
        isLength(definition.thickness, { (millimeter) : [0.1, 8, 1000], (inch) : 0.3 } as LengthBoundSpec);
        annotation { "Name" : "Hub diameter" }
        isLength(definition.hubDiameter, { (millimeter) : [0.1, 36, 10000], (inch) : 1.4 } as LengthBoundSpec);
        annotation { "Name" : "Hub height" }
        isLength(definition.hubHeight, { (millimeter) : [0.1, 14, 1000], (inch) : 0.5 } as LengthBoundSpec);
        annotation { "Name" : "Bore diameter" }
        isLength(definition.bore, { (millimeter) : [0.1, 20, 10000], (inch) : 0.75 } as LengthBoundSpec);
        annotation { "Name" : "Bolt holes" }
        isInteger(definition.bolts, { (unitless) : [0, 6, 24] } as IntegerBoundSpec);
        annotation { "Name" : "Bolt hole diameter" }
        isLength(definition.boltDiameter, { (millimeter) : [0.1, 8, 1000], (inch) : 0.3 } as LengthBoundSpec);
        annotation { "Name" : "Round outer edges", "Default" : true }
        definition.round is boolean;
        if (definition.round)
        {
            annotation { "Name" : "Edge radius" }
            isLength(definition.radius, { (millimeter) : [0.01, 1.5, 100], (inch) : 0.06 } as LengthBoundSpec);
        }
    }
    {
        if (definition.hubDiameter >= definition.diameter)
            throw regenError("The hub must be smaller than the flange", ["hubDiameter"]);
        if (definition.bore >= definition.hubDiameter)
            throw regenError("The bore must be smaller than the hub", ["bore"]);
        const t = definition.thickness;
        const top = t + definition.hubHeight;
        const z = 0 * meter;
        fCylinder(context, id + "disk", { "bottomCenter" : vector(z, z, z), "topCenter" : vector(z, z, t), "radius" : definition.diameter / 2 });
        fCylinder(context, id + "hub", { "bottomCenter" : vector(z, z, t), "topCenter" : vector(z, z, top), "radius" : definition.hubDiameter / 2 });
        const body = qCreatedBy(id + "disk", EntityType.BODY);
        opBoolean(context, id + "join", { "tools" : qUnion([body, qCreatedBy(id + "hub", EntityType.BODY)]), "operationType" : BooleanOperationType.UNION });
        var cutters = [id + "bore"];
        fCylinder(context, id + "bore", { "bottomCenter" : vector(z, z, -1 * millimeter), "topCenter" : vector(z, z, top + 1 * millimeter), "radius" : definition.bore / 2 });
        if (definition.bolts > 0)
        {
            const pcd = (definition.diameter + definition.hubDiameter) / 4;
            fCylinder(context, id + "bolt", { "bottomCenter" : vector(pcd, z, -1 * millimeter), "topCenter" : vector(pcd, z, t + 1 * millimeter), "radius" : definition.boltDiameter / 2 });
            var transforms = [];
            var names = [];
            for (var i = 1; i < definition.bolts; i += 1)
            {
                transforms = append(transforms, rotationAround(line(vector(z, z, z), vector(0, 0, 1)), i * 360 / definition.bolts * degree));
                names = append(names, "bolt" ~ i);
            }
            opPattern(context, id + "bolts", { "entities" : qCreatedBy(id + "bolt", EntityType.BODY), "transforms" : transforms, "instanceNames" : names });
            cutters = concatenateArrays([cutters, [id + "bolt", id + "bolts"]]);
        }
        opBoolean(context, id + "cut", {
                "tools" : qUnion(mapArray(cutters, function(c) { return qCreatedBy(c, EntityType.BODY); })),
                "targets" : body,
                "operationType" : BooleanOperationType.SUBTRACTION
        });
        if (definition.round)
        {
            // The two circular edges on the outer rim.
            var rim = [];
            for (var e in evaluateQuery(context, qGeometry(qOwnedByBody(body, EntityType.EDGE), GeometryType.CIRCLE)))
            {
                if (tolerantEquals(evCurveDefinition(context, { "edge" : e }).radius, definition.diameter / 2))
                    rim = append(rim, e);
            }
            opFillet(context, id + "round", { "entities" : qUnion(rim), "radius" : definition.radius });
        }
    });

annotation { "Feature Type Name" : "Bolt circle", "Feature Type Description" : "Holes on a circle centered on a planar face" }
export const boltCircle = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Face", "Filter" : EntityType.FACE && GeometryType.PLANE, "MaxNumberOfPicks" : 1 }
        definition.face is Query;
        annotation { "Name" : "Holes" }
        isInteger(definition.count, { (unitless) : [1, 4, 36] } as IntegerBoundSpec);
        annotation { "Name" : "Bolt circle diameter" }
        isLength(definition.pcd, { (millimeter) : [0, 28, 10000], (inch) : 1 } as LengthBoundSpec);
        annotation { "Name" : "Hole diameter" }
        isLength(definition.diameter, { (millimeter) : [0.01, 3, 1000], (inch) : 0.125 } as LengthBoundSpec);
        annotation { "Name" : "Through all", "Default" : false }
        definition.through is boolean;
        if (!definition.through)
        {
            annotation { "Name" : "Depth" }
            isLength(definition.depth, { (millimeter) : [0.01, 6, 10000], (inch) : 0.25 } as LengthBoundSpec);
        }
    }
    {
        if (isQueryEmpty(context, definition.face))
            throw regenError("Select a planar face", ["face"]);
        const facePlane = evPlane(context, { "face" : definition.face });
        const center = evApproximateCentroid(context, { "entities" : definition.face });
        const sketch = newSketchOnPlane(context, id + "sketch", { "sketchPlane" : plane(center, facePlane.normal, facePlane.x) });
        for (var i = 0; i < definition.count; i += 1)
        {
            const a = i * 360 / definition.count * degree;
            skCircle(sketch, "hole" ~ i, { "center" : vector(cos(a), sin(a)) * definition.pcd / 2, "radius" : definition.diameter / 2 });
        }
        skSolve(sketch);
        opExtrude(context, id + "extrude", {
                "entities" : qSketchRegion(id + "sketch"),
                "direction" : -facePlane.normal,
                "endBound" : definition.through ? BoundingType.THROUGH_ALL : BoundingType.BLIND,
                "endDepth" : definition.depth
        });
        opBoolean(context, id + "cut", {
                "tools" : qCreatedBy(id + "extrude", EntityType.BODY),
                "targets" : qOwnerBody(definition.face),
                "operationType" : BooleanOperationType.SUBTRACTION
        });
        opDeleteBodies(context, id + "deleteSketch", { "entities" : qCreatedBy(id + "sketch", EntityType.BODY) });
    });
"#;

/// Add the demo Feature Studio and a "Flange" Part Studio using its
/// features (Flange, then Bolt circle on the hub's top face).
pub fn add_demo_feature_studio(doc: &mut Document) {
    let fs = doc.add_tab("Custom features", TabContent::FeatureStudio(crate::custom::FeatureStudio::new(DEMO_FEATURE_STUDIO)));
    let scripts = crate::custom::DocumentScripts::from_document(doc);
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let Some(flange) = ps.new_feature(&crate::custom::type_id(fs, "flange"), &units) else { return };
    ps.features.push(flange);
    let opts = RegenOptions { custom_features: Some(scripts), ..Default::default() };
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &opts);
    // Hub top: the highest planar face.
    let top = r.context.bodies.first().and_then(|b| {
        (0..b.faces.len())
            .filter_map(|i| {
                let f = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok()?;
                f.face_plane().ok()??;
                Some((i, f.centroid().ok()?.z))
            })
            .max_by(|a, c| a.1.total_cmp(&c.1))
            .map(|(i, _)| Reference::Face((*b.faces[i]).clone()))
    });
    if let (Some(top), Some(mut bc)) = (top, ps.new_feature(&crate::custom::type_id(fs, "boltCircle"), &units)) {
        bc.params.set("face", ParamValue::Query(vec![top]));
        ps.features.push(bc);
    }
    doc.add_tab("Flange", TabContent::PartStudio(ps));
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

/// Make the pin Part Studio configured: a list input "Length" switching the
/// pin length (Standard 40 mm, the default, or Long 60 mm).
pub fn configure_pin_length(ps: &mut PartStudio) {
    use crate::configuration::{ConfigInput, ConfigInputKind, ConfiguredParam, ListOption};
    let Some(ex) = ps.features.iter().find(|f| f.type_id == "extrude").map(|f| f.id) else { return };
    let opt = |id: &str| ListOption { id: id.into(), name: id.into() };
    ps.configuration.inputs.push(ConfigInput { id: "Length".into(), name: "Length".into(), kind: ConfigInputKind::List { options: vec![opt("Standard"), opt("Long")], default: "Standard".into() }, visible_when: None, excluded: false });
    ps.configuration.configured.push(ConfiguredParam {
        input: "Length".into(),
        feature: ex,
        param: "depth".into(),
        values: [("Standard".to_string(), ParamValue::Expr(ExprValue::new("40 mm"))), ("Long".to_string(), ParamValue::Expr(ExprValue::new("60 mm")))].into_iter().collect(),
    });
}

/// Fill the document's first Assembly tab with a demo assembly: two plates
/// (the second fastened on top of the first, turned 90°) and a pin in a
/// revolute mate through the center holes. Adds a "Pin" Part Studio, which
/// is configured (Length: Standard / Long; the instance uses the default).
pub fn add_demo_assembly(doc: &mut Document) {
    use crate::assembly::eval::AssemblyEvaluator;
    use crate::assembly::*;
    use crate::reference::Inference;
    let Some(plate_tab) = doc.tabs.iter().find(|t| matches!(t.content, TabContent::PartStudio(_))).map(|t| t.id) else { return };
    let mut pin_ps = pin_studio();
    configure_pin_length(&mut pin_ps);
    let pin_tab = doc.add_tab("Pin", TabContent::PartStudio(pin_ps));
    let mut ev = AssemblyEvaluator::new();
    let cfg = ConfigValues::default();
    let (Some(plate), Some(pin)) = (ev.studio(doc, plate_tab, &cfg), ev.studio(doc, pin_tab, &cfg)) else { return };
    let (Some(pb), Some(qb)) = (plate.context.bodies.first(), pin.context.bodies.first()) else { return };
    // Planar cap faces of the extrusion (not fillet blends whose names mention the caps).
    let face_named = |b: &crate::regen::context::Body, needle: &str| {
        b.faces
            .iter()
            .find(|f| match &***f {
                crate::reference::TopoName::Face { role: crate::reference::Role::CapEnd(_), .. } => needle == "CapEnd",
                crate::reference::TopoName::Face { role: crate::reference::Role::CapStart(_), .. } => needle == "CapStart",
                _ => false,
            })
            .map(|f| Reference::Face((**f).clone()))
    };
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
    let mc = |instance: u64, on: Reference, inference: Inference| MateConnectorRef::on(InstanceId(instance), vec![], Reference::Implicit { on: Box::new(on), inference });
    let inst = |id: u64, name: &str, studio, body: crate::reference::BodyId, at: [f64; 3], fixed| Instance {
        id: InstanceId(id),
        name: name.into(),
        source: InstanceSource::Part { studio, body, config: ConfigValues::default() },
        placement: Placement { translation: at, rotation: [0.0, 0.0, 0.0, 1.0] },
        fixed,
        hidden: false,
        suppressed: false,
        lock: SubassemblyLock::Flexible,
        nested: Vec::new(),
        best_tessellation: false,
        reference_designator: None,
        unsuppressed_if: None,
    };
    let mate = |id: u64, name: &str, t: MateType, a, b| Mate::new(MateId(id), name.into(), t, a, b);
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
        t.content = TabContent::Assembly(Box::new(asm));
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
        for (k, v) in [("Title", "Plate"), ("Drawn by", "noshape"), ("Part number", "P-001"), ("Revision", "A"), ("Material", "Aluminum 6061"), ("Sheet", "1 / 1")] {
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
    add(sheet, AnnotationKind::RadialDimension { view: ViewId(2), center: [0.0, 0.0], radius: 0.012, diameter: true, angle: 2.4, dim_line: false }, [0.0; 2], vec![(0, c(DVec3::ZERO, false))]);
    add(sheet, AnnotationKind::RadialDimension { view: ViewId(6), center: [0.038, 0.018], radius: 0.0048, diameter: false, angle: 0.9, dim_line: false }, [0.0; 2], vec![(0, c(DVec3::new(0.038, 0.018, 0.0), false))]);
    // Front view: width and thickness.
    add(sheet, lin(1, [-0.05, 0.0], [0.05, 0.0], -10.0, DimOrientation::Horizontal), [0.0; 2], vec![(0, v(DVec3::new(-0.05, -0.03, 0.0))), (1, v(DVec3::new(0.05, -0.03, 0.0)))]);
    add(sheet, lin(1, [-0.05, 0.0], [-0.05, 0.012], -8.0, DimOrientation::Vertical), [0.0; 2], vec![(0, v(DVec3::new(-0.05, -0.03, 0.0)))]);
    add(sheet, AnnotationKind::balloon(ViewId(4), [0.0, 0.0], "1"), [385.0, 255.0], vec![(0, v(DVec3::new(0.05, -0.03, 0.0)))]);
    add(sheet, AnnotationKind::note("NOTES:\n1. BREAK ALL SHARP EDGES.\n2. DIMENSIONS IN MILLIMETERS.", 3.5), [25.0, 75.0], vec![]);
    let _ = d.add_tab("Drawing 1", TabContent::Drawing(drawing));
    d
}
