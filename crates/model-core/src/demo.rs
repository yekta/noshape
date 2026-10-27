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

/// A new document whose Part Studio holds the demo plate (`demo_document` in noshape-assembly adds the demo assembly
/// and Feature Studio).
pub fn plate_document() -> Document {
    let mut d = Document::new();
    if let TabContent::PartStudio(ps) = &mut d.tabs[0].content {
        *ps = plate_studio();
    }
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


