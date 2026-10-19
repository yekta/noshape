//! Feature Studios: custom features written in FeatureScript, evaluated in Part Studios.

use noshape_model::configuration::ConfigValues;
use noshape_model::custom::{self, DocumentScripts, FeatureStudio};
use noshape_model::display::FeatureStatus;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::Reference;
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::{Document, TabContent, TabId};
use noshape_units::{ExprValue, UnitPrefs};
use std::f64::consts::PI;

struct Fixture {
    doc: Document,
    fs: TabId,
    units: UnitPrefs,
}

impl Fixture {
    fn new(source: &str) -> Self {
        let mut doc = Document::new();
        let fs = doc.add_tab("Feature Studio 1", TabContent::FeatureStudio(FeatureStudio::new(source)));
        // Compiling registers the custom feature schemas (new_feature needs them).
        let _ = DocumentScripts::from_document(&doc);
        Fixture { doc, fs, units: UnitPrefs::default() }
    }

    fn set_source(&mut self, source: &str) {
        if let Some(TabContent::FeatureStudio(f)) = self.doc.tab_mut(self.fs).map(|t| &mut t.content) {
            f.source = source.to_string();
        }
    }

    fn opts(&self) -> RegenOptions {
        RegenOptions { custom_features: Some(DocumentScripts::from_document(&self.doc)), ..Default::default() }
    }

    fn ty(&self, name: &str) -> String {
        custom::type_id(self.fs, name)
    }

    fn regen(&self, reg: &mut Regenerator, ps: &PartStudio) -> RegenResult {
        reg.regenerate(ps, &ConfigValues::default(), &self.units, &self.opts())
    }
}

fn expr(s: &str) -> ParamValue {
    ParamValue::Expr(ExprValue::new(s))
}

fn assert_ok(r: &RegenResult) {
    for s in &r.display.features {
        if s.status != FeatureStatus::Ok {
            let notices: Vec<String> = custom::run::outputs().values().flat_map(|o| o.notices.iter().map(|n| format!("{} {}: {}", n.module, n.pos, n.message))).collect();
            panic!("feature failed: {:?}\nnotices:\n{}", s.message, notices.join("\n"));
        }
    }
}

fn total_volume(r: &RegenResult) -> f64 {
    r.context.bodies.iter().map(|b| b.shape.volume().unwrap().abs()).sum()
}


/// A Feature Studio with the usual header.
fn studio(body: &str) -> String {
    format!("FeatureScript 3095;\nimport(path : \"onshape/std/geometry.fs\", version : \"3095.0\");\n{body}")
}

const SHAPES: &str = r#"
annotation { "Feature Type Name" : "Box and pin" }
export const boxAndPin = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Size" }
        isLength(definition.size, LENGTH_BOUNDS);
        annotation { "Name" : "Pin radius" }
        isLength(definition.radius, LENGTH_BOUNDS);
        annotation { "Name" : "Pin fillet" }
        isLength(definition.fillet, BLEND_BOUNDS);
    }
    {
        fCuboid(context, id + "box", { "corner1" : vector(0, 0, 0) * meter, "corner2" : vector(1, 1, 1) * definition.size });
        fCylinder(context, id + "pin", { "bottomCenter" : vector(50, 0, 0) * millimeter, "topCenter" : vector(50, 0, 10) * millimeter, "radius" : definition.radius });
        const circles = qGeometry(qCreatedBy(id + "pin", EntityType.EDGE), GeometryType.CIRCLE);
        if (size(evaluateQuery(context, circles)) != 2)
            throw regenError("expected 2 circular edges");
        opFillet(context, id + "fillet", { "entities" : circles, "radius" : definition.fillet });
    });
"#;

#[test]
fn box_and_filleted_cylinder() {
    let fx = Fixture::new(&studio(SHAPES));
    let scripts = DocumentScripts::from_document(&fx.doc);
    let (_, _, compiled) = scripts.studios().next().unwrap();
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    assert_eq!(compiled.features.len(), 1);
    let t = compiled.features[0].feature_type;
    assert_eq!(t.name, "Box and pin");
    assert_eq!(t.params.iter().map(|p| p.label).collect::<Vec<_>>(), ["Size", "Pin radius", "Pin fillet"]);

    let mut ps = PartStudio::new();
    let mut f = ps.new_feature(&fx.ty("boxAndPin"), &fx.units).expect("custom feature type is registered");
    assert_eq!(f.name, "Box and pin 1");
    f.params.set("size", expr("20 mm"));
    f.params.set("radius", expr("4 mm"));
    f.params.set("fillet", expr("1 mm"));
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    assert_eq!(r.context.bodies.len(), 2, "box + pin (sketches deleted)");
    assert!(r.context.sketches.is_empty(), "fCuboid / fCylinder delete their sketches");
    let cube = r.context.bodies.iter().find(|b| b.faces.len() == 6).expect("box with 6 faces");
    assert!((cube.shape.volume().unwrap() - 0.02f64.powi(3)).abs() < 1e-12);
    let pin = r.context.bodies.iter().find(|b| b.faces.len() != 6).unwrap();
    assert_eq!(pin.faces.len(), 5, "cylinder side, 2 caps, 2 fillets");
    let plain = PI * 0.004f64.powi(2) * 0.01;
    let v = pin.shape.volume().unwrap();
    assert!(v < plain && v > plain * 0.8, "filleted pin volume {v} vs {plain}");
    // Everything belongs to the custom feature.
    let fid = ps.features[0].id;
    assert!(r.context.bodies.iter().all(|b| b.faces.iter().all(|n| n.creator() == Some(fid))));
    assert_ne!(r.context.bodies[0].id, r.context.bodies[1].id);
}

const BOSS: &str = r#"
annotation { "Feature Type Name" : "Boss" }
export const boss = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Face", "Filter" : EntityType.FACE && GeometryType.PLANE, "MaxNumberOfPicks" : 1 }
        definition.face is Query;
        annotation { "Name" : "Height" }
        isLength(definition.height, LENGTH_BOUNDS);
        annotation { "Name" : "Cut instead", "Default" : false }
        definition.cut is boolean;
    }
    {
        if (isQueryEmpty(context, definition.face))
            throw regenError("Select a planar face", ["face"]);
        const plane = evPlane(context, { "face" : definition.face });
        opExtrude(context, id + "extrude", {
                "entities" : definition.face,
                "direction" : definition.cut ? -plane.normal : plane.normal,
                "endBound" : BoundingType.BLIND,
                "endDepth" : definition.height
        });
        const owner = qOwnerBody(definition.face);
        opBoolean(context, id + "boolean", {
                "tools" : qCreatedBy(id + "extrude", EntityType.BODY),
                "targets" : owner,
                "operationType" : definition.cut ? BooleanOperationType.SUBTRACTION : BooleanOperationType.UNION
        });
    });
"#;

#[test]
fn query_param_extrudes_selected_face() {
    let fx = Fixture::new(&studio(BOSS));
    let opts = fx.opts();
    let mut ps = PartStudio::new();
    let mut cube = ps.new_feature("cube", &fx.units).unwrap();
    cube.params.set("sideLength", expr("20 mm"));
    ps.features.push(cube);
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &fx.units, &opts);
    assert_ok(&r);
    let body = &r.context.bodies[0];
    let top = (0..body.faces.len())
        .max_by(|a, b| {
            let z = |i: usize| body.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().centroid().unwrap().z;
            z(*a).total_cmp(&z(*b))
        })
        .unwrap();
    let face = Reference::Face((*body.faces[top]).clone());
    let mut boss = ps.new_feature(&fx.ty("boss"), &fx.units).unwrap();
    boss.params.set("face", ParamValue::Query(vec![face.clone()]));
    boss.params.set("height", expr("10 mm"));
    let boss_id = boss.id;
    ps.features.push(boss);
    let r = reg.regenerate(&ps, &ConfigValues::default(), &fx.units, &opts);
    assert_ok(&r);
    assert_eq!(r.context.bodies.len(), 1, "boss merged into the cube");
    let v = total_volume(&r);
    let expected = 0.02f64.powi(3) * 1.5;
    assert!((v - expected).abs() < 1e-12, "{v} vs {expected}");
    let named = r.context.bodies[0].faces.iter().filter(|n| n.creator() == Some(boss_id)).count();
    assert!(named >= 1, "boss faces named by the custom feature: {named}");

    ps.features[1].params.set("cut", ParamValue::Bool(true));
    let r = reg.regenerate(&ps, &ConfigValues::default(), &fx.units, &opts);
    assert_ok(&r);
    let expected = 0.02f64.powi(3) * 0.5;
    assert!((total_volume(&r) - expected).abs() < 1e-12, "{} vs {expected}", total_volume(&r));

    // No face: the custom error and its faulty parameter.
    ps.features[1].params.set("face", ParamValue::Query(vec![]));
    let r = reg.regenerate(&ps, &ConfigValues::default(), &fx.units, &opts);
    assert_eq!(r.display.features[1].status, FeatureStatus::Error);
    assert_eq!(r.display.features[1].message.as_deref(), Some("Select a planar face"));
}

const BLOCK: &str = r#"
annotation { "Feature Type Name" : "Block" }
export const block = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Width" }
        isLength(definition.width, { (millimeter) : [1, 30, 1000] } as LengthBoundSpec);
    }
    {
        fCuboid(context, id + "box", { "corner1" : vector(0, 0, 0) * millimeter, "corner2" : vector(definition.width, 20 * millimeter, 10 * millimeter) });
    });
"#;

/// The edge of the block at x = max, z = max (along y).
fn top_right_edge(r: &RegenResult) -> Reference {
    let b = &r.context.bodies[0];
    let bb = b.shape.bounding_box(false).unwrap();
    let i = (0..b.edges.len())
        .find(|&i| {
            let (p, q) = b.shape.sub_shape(noshape_kernel::ShapeType::Edge, i).unwrap().endpoints().unwrap();
            (p.x - bb.max.x).abs() < 1e-6 && (q.x - bb.max.x).abs() < 1e-6 && (p.z - bb.max.z).abs() < 1e-6 && (q.z - bb.max.z).abs() < 1e-6
        })
        .expect("edge");
    Reference::Edge((*b.edges[i]).clone())
}

#[test]
fn downstream_fillet_survives_param_change() {
    let fx = Fixture::new(&studio(BLOCK));
    let mut ps = PartStudio::new();
    let block = ps.new_feature(&fx.ty("block"), &fx.units).unwrap();
    ps.features.push(block);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    // The precondition's millimeter default.
    let v = total_volume(&r);
    assert!((v - 0.03 * 0.02 * 0.01).abs() < 1e-12, "default width 30 mm: {v}");

    let edge = top_right_edge(&r);
    let mut fillet = ps.new_feature("fillet", &fx.units).unwrap();
    fillet.params.set("entities", ParamValue::Query(vec![edge.clone()]));
    fillet.params.set("radius", expr("2 mm"));
    ps.features.push(fillet);
    let removed = (4.0 - PI) / 4.0 * 0.002f64.powi(2) * 0.02;
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    let v = total_volume(&r);
    assert!((v - (0.03 * 0.02 * 0.01 - removed)).abs() < 1e-11, "volume {v}");

    // Change the parameter: the fillet still finds its edge.
    ps.features[0].params.set("width", expr("45 mm"));
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    let v = total_volume(&r);
    assert!((v - (0.045 * 0.02 * 0.01 - removed)).abs() < 1e-11, "volume after edit {v}");

    // Editing the feature body (not its declaration) keeps names too.
    let mut fx = fx;
    fx.set_source(&studio(&BLOCK.replace("vector(0, 0, 0) * millimeter", "vector(5, 0, 0) * millimeter")));
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    let bb = r.context.bodies[0].shape.bounding_box(false).unwrap();
    assert!((bb.min.x - 0.005).abs() < 1e-5, "studio edit regenerated the Part Studio: {}", bb.min.x);

    // Out of the precondition's bounds.
    ps.features[0].params.set("width", expr("0.5 mm"));
    let r = fx.regen(&mut reg, &ps);
    assert_eq!(r.display.features[0].status, FeatureStatus::Error);
}

#[test]
fn compile_and_runtime_errors_become_feature_errors() {
    let mut fx = Fixture::new(&studio(BLOCK));
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("block"), &fx.units).unwrap();
    ps.features.push(f);
    let mut reg = Regenerator::new();
    assert_ok(&fx.regen(&mut reg, &ps));

    let status = |r: &RegenResult| (r.display.features[0].status, r.display.features[0].message.clone().unwrap_or_default());

    // Parse error.
    fx.set_source(&studio(&BLOCK.replace("fCuboid(", "var x = ;\nfCuboid(")));
    let (st, msg) = status(&fx.regen(&mut reg, &ps));
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.contains("Feature Studio has errors"), "{msg}");

    // A thrown error.
    fx.set_source(&studio(&BLOCK.replace("fCuboid(", "throw regenError(\"Nope, not today\");\nfCuboid(")));
    let (st, msg) = status(&fx.regen(&mut reg, &ps));
    assert_eq!((st, msg.as_str()), (FeatureStatus::Error, "Nope, not today"));

    // A failing operation: the ErrorStringEnum text.
    fx.set_source(&studio(&BLOCK.replace("fCuboid(context, id + \"box\"", "opFillet(context, id + \"f\", { \"entities\" : qNothing(), \"radius\" : 1 * millimeter });\nfCuboid(context, id + \"box\"")));
    let (st, msg) = status(&fx.regen(&mut reg, &ps));
    assert_eq!(st, FeatureStatus::Error);
    assert_eq!(msg, "Select edges or faces to fillet.");

    // A language error (string thrown by the runtime).
    fx.set_source(&studio(&BLOCK.replace("fCuboid(", "var m = undefined; m.x = 1;\nfCuboid(")));
    let (st, _) = status(&fx.regen(&mut reg, &ps));
    assert_eq!(st, FeatureStatus::Error);

    // The feature is no longer declared.
    fx.set_source(&studio(&BLOCK.replace("export const block", "export const other")));
    let (st, msg) = status(&fx.regen(&mut reg, &ps));
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.contains("no longer defines"), "{msg}");

    // Feature Studio tab deleted.
    fx.doc.remove_tab(fx.fs);
    let (st, msg) = status(&fx.regen(&mut reg, &ps));
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.contains("no longer exists"), "{msg}");

    // Fixing the source recovers.
    let mut fx = Fixture::new(&studio(BLOCK));
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("block"), &fx.units).unwrap();
    ps.features.push(f);
    fx.set_source("garbage (");
    assert_eq!(status(&fx.regen(&mut reg, &ps)).0, FeatureStatus::Error);
    fx.set_source(&studio(BLOCK));
    assert_ok(&fx.regen(&mut reg, &ps));
}

#[test]
fn queries_patterns_and_enums() {
    let src = r#"
export enum PegShape
{
    annotation { "Name" : "Round" }
    ROUND,
    annotation { "Name" : "Square" }
    SQUARE
}

annotation { "Feature Type Name" : "Ring of pegs" }
export const ringOfPegs = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Count" }
        isInteger(definition.count, { (unitless) : [1, 6, 50] } as IntegerBoundSpec);
        annotation { "Name" : "Radius" }
        isLength(definition.radius, LENGTH_BOUNDS);
        annotation { "Name" : "Shape" }
        definition.shape is PegShape;
    }
    {
        fCylinder(context, id + "base", { "bottomCenter" : vector(0, 0, 0) * meter, "topCenter" : vector(0, 0, 5) * millimeter, "radius" : definition.radius + 10 * millimeter });
        const r = definition.radius;
        if (definition.shape == PegShape.ROUND)
            fCylinder(context, id + "peg", { "bottomCenter" : vector(r, 0 * meter, 5 * millimeter), "topCenter" : vector(r, 0 * meter, 13 * millimeter), "radius" : 3 * millimeter });
        else
            fCuboid(context, id + "peg", { "corner1" : vector(r - 3 * millimeter, -3 * millimeter, 5 * millimeter), "corner2" : vector(r + 3 * millimeter, 3 * millimeter, 13 * millimeter) });
        var transforms = [];
        var names = [];
        for (var i = 1; i < definition.count; i += 1)
        {
            transforms = append(transforms, rotationAround(Z_AXIS, i * 360 / definition.count * degree));
            names = append(names, "" ~ i);
        }
        opPattern(context, id + "pattern", { "entities" : qCreatedBy(id + "peg", EntityType.BODY), "transforms" : transforms, "instanceNames" : names });
        if (size(evaluateQuery(context, qCreatedBy(id + "pattern", EntityType.BODY))) != definition.count - 1)
            throw "pattern count";
        opBoolean(context, id + "union", { "tools" : qCreatedBy(id, EntityType.BODY), "operationType" : BooleanOperationType.UNION });
        const parts = qAllModifiableSolidBodies();
        if (size(evaluateQuery(context, parts)) != 1)
            throw "union failed";
        const box = evBox3d(context, { "topology" : parts });
        if (abs(box.maxCorner[2] - box.minCorner[2] - 13 * millimeter) > 1e-9 * meter)
            throw "height";
        if (evVolume(context, { "entities" : parts }) <= 0 * meter ^ 3)
            throw "volume";
    });
"#;
    let fx = Fixture::new(&studio(src));
    let scripts = DocumentScripts::from_document(&fx.doc);
    let (_, _, c) = scripts.studios().next().unwrap();
    assert!(!c.has_errors(), "{:?}", c.diagnostics);
    let shape = c.features[0].feature_type.params.iter().find(|p| p.id == "shape").unwrap();
    match &shape.kind {
        noshape_model::params::ParamKind::Enum { options, default } => {
            assert_eq!(options.to_vec(), vec![("ROUND", "Round"), ("SQUARE", "Square")]);
            assert_eq!(*default, "ROUND");
        }
        _ => panic!("enum parameter"),
    }
    let mut ps = PartStudio::new();
    let mut f = ps.new_feature(&fx.ty("ringOfPegs"), &fx.units).unwrap();
    f.params.set("radius", expr("30 mm"));
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    assert_eq!(r.context.bodies.len(), 1);
    ps.features[0].params.set("shape", ParamValue::Enum("SQUARE".into()));
    ps.features[0].params.set("count", expr("4"));
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    let base = PI * 0.04f64.powi(2) * 0.005;
    let pegs = 4.0 * 0.006 * 0.006 * 0.008;
    assert!((total_volume(&r) - (base + pegs)).abs() < 1e-10, "{}", total_volume(&r));
    // Out-of-range integer parameter.
    ps.features[0].params.set("count", expr("80"));
    let r = fx.regen(&mut reg, &ps);
    assert_eq!(r.display.features[0].status, FeatureStatus::Error);
}

#[test]
fn visibility_follows_precondition_ifs() {
    let src = r#"
annotation { "Feature Type Name" : "Plate" }
export const plate = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Round corners", "Default" : true }
        definition.round is boolean;
        if (definition.round)
        {
            annotation { "Name" : "Corner radius" }
            isLength(definition.radius, BLEND_BOUNDS);
        }
        else
        {
            annotation { "Name" : "Chamfer" }
            isLength(definition.chamfer, BLEND_BOUNDS);
        }
    }
    {
    });
"#;
    let fx = Fixture::new(&studio(src));
    let t = noshape_model::features::get(&fx.ty("plate")).unwrap();
    let mut v = noshape_model::params::ParamValues::default();
    noshape_model::params::fill_defaults(&t.params, &mut v, &fx.units);
    let vis = |v: &noshape_model::params::ParamValues| t.params.iter().filter(|p| p.is_visible(v)).map(|p| p.id).collect::<Vec<_>>();
    assert_eq!(vis(&v), ["round", "radius"]);
    v.set("round", ParamValue::Bool(false));
    assert_eq!(vis(&v), ["round", "chamfer"]);
}

#[test]
fn demo_feature_studio_regenerates() {
    let doc = noshape_model::demo::demo_document();
    let scripts = DocumentScripts::from_document(&doc);
    let (_, _, c) = scripts.studios().next().expect("demo has a Feature Studio");
    assert!(!c.has_errors(), "{:?}", c.diagnostics);
    assert_eq!(c.features.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["flange", "boltCircle"]);
    let tab = doc.tabs.iter().find(|t| t.name == "Flange").expect("Flange Part Studio");
    let TabContent::PartStudio(ps) = &tab.content else { panic!() };
    assert_eq!(ps.features.len(), 2);
    let opts = RegenOptions { custom_features: Some(scripts.clone()), ..Default::default() };
    let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &doc.settings.units, &opts);
    assert_ok(&r);
    assert_eq!(r.context.bodies.len(), 1);
    let v = total_volume(&r);
    let disk = PI * 0.04f64.powi(2) * 0.008;
    assert!(v > disk * 0.8 && v < disk * 1.4, "flange volume {v}");
    let TabContent::PartStudio(plate) = &doc.tabs[0].content else { panic!() };
    assert_ok(&Regenerator::new().regenerate(plate, &ConfigValues::default(), &doc.settings.units, &opts));
}

#[test]
fn new_feature_studio_template_compiles() {
    let fx = Fixture::new(&FeatureStudio::template().source);
    let scripts = DocumentScripts::from_document(&fx.doc);
    let (_, _, c) = scripts.studios().next().unwrap();
    assert!(!c.has_errors(), "{:?}", c.diagnostics);
    assert!(c.features.is_empty());
}

/// The user's own Onshape feature (Y Threads) on a Ø10 x 20 mm cylinder.
#[test]
#[ignore = "FeatureScript: runs to the end, but the kernel's helical thread sweep fuses badly (addTeeth union keeps only the teeth, the chamfer cut then leaves a 0-volume solid); needs sweep/boolean robustness"]
fn real_onshape_feature_y_threads() {
    let Ok(src) = std::fs::read_to_string("/root/onshape-featurescripts/y-threads.fs") else { return };
    let cylinder = studio(
        r#"
annotation { "Feature Type Name" : "Test cylinder" }
export const testCylinder = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        fCylinder(context, id + "c", { "bottomCenter" : vector(0, 0, 0) * meter, "topCenter" : vector(0, 0, 20) * millimeter, "radius" : 5 * millimeter });
    });
"#,
    );
    let mut doc = Document::new();
    let threads = doc.add_tab("Y Threads", TabContent::FeatureStudio(FeatureStudio::new(&src)));
    let cyl = doc.add_tab("Cylinder", TabContent::FeatureStudio(FeatureStudio::new(&cylinder)));
    let scripts = DocumentScripts::from_document(&doc);
    for (_, name, c) in scripts.studios() {
        assert!(!c.has_errors(), "{name}: {:?}", c.diagnostics);
    }
    let units = UnitPrefs::default();
    let opts = RegenOptions { custom_features: Some(scripts.clone()), ..Default::default() };
    let mut ps = PartStudio::new();
    ps.features.push(ps.new_feature(&custom::type_id(cyl, "testCylinder"), &units).unwrap());
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &opts);
    assert_ok(&r);
    let b = &r.context.bodies[0];
    let side = (0..b.faces.len())
        .find(|&i| matches!(b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().face_surface().unwrap(), noshape_kernel::Surface::Cylinder { .. }))
        .expect("cylindrical face");
    let mut f = ps.new_feature(&custom::type_id(threads, "yThreads"), &units).unwrap();
    f.params.set("faces", ParamValue::Query(vec![Reference::Face((*b.faces[side]).clone())]));
    ps.features.push(f);
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &opts);
    assert_ok(&r);
    assert_eq!(r.context.bodies.len(), 1, "threads cut into the shaft");
    let v = total_volume(&r);
    let plain = PI * 0.005f64.powi(2) * 0.02;
    // Default custom profile: pitch 1 mm, 45°, root/tip 0.2: depth 0.3 mm.
    let core = PI * 0.0047f64.powi(2) * 0.02;
    assert!(v < plain && v > core, "threaded volume {v} (plain {plain}, core {core})");
}

/// The live "FS probe" (onshape-research/live/dom/fs-probe-output.txt,
/// behaviours.md "FS probe (fs part 2)"): printed output, notices with
/// positions, the part's volume and the dialog's defaults.
#[test]
fn fs_probe_matches_onshape() {
    let fx = Fixture::new(include_str!("../../featurescript/src/fsprobe.fs.txt"));
    let t = noshape_model::features::get(&fx.ty("fsProbe")).expect("FS probe is registered");
    assert_eq!(t.name, "FS probe");
    let mut v = noshape_model::params::ParamValues::default();
    noshape_model::params::fill_defaults(&t.params, &mut v, &fx.units);
    let vis = |v: &noshape_model::params::ParamValues| t.params.iter().filter(|p| p.is_visible(v)).map(|p| p.id).collect::<Vec<_>>();
    assert_eq!(vis(&v), ["size", "corner", "count"]);
    v.set("corner", ParamValue::Enum("ROUNDED".into()));
    assert_eq!(vis(&v), ["size", "corner", "radius", "count"]);

    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("fsProbe"), &fx.units).unwrap();
    assert_eq!(f.name, "FS probe 1");
    let fid = f.id;
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    assert!((total_volume(&r) - 8000e-9).abs() < 1e-12, "volume {}", total_volume(&r));

    let out = custom::run::output_of(fid).expect("probe output");
    let lines: Vec<&str> = out.output.lines().collect();
    let want = [
        "0.050800000000000005 meter",
        "0.3333333333333333",
        "0.050800000000000005 meter",
        "[ 1 , a , true ]",
        "{ k : 1.5 meter }",
        "ValueWithUnits : { \"unit\" : UnitSpec : { \"meter\" : 1 } , \"value\" : 2 }",
        "normal (0, -1, 0) origin (0 meter, 0 meter, 0 meter) x (1, 0, 0)",
        "normal (-1, 0, 0) origin (0 meter, 0.01 meter, 0.01 meter) x (0, -1, 0)",
        "12",
    ];
    assert_eq!(&lines[..want.len()], &want, "println output:\n{}", out.output);
    let q = lines[want.len()];
    assert!(q.starts_with("[ { queryType : TRANSIENT , transientId : ") && q.ends_with(" } ]"), "{q}");
    assert_eq!(&lines[want.len() + 1..], &["Execution error"; 5]);

    let notices: Vec<String> = out.notices.iter().map(|n| format!("{} {}:{} {}", n.module.rsplit('/').next().unwrap_or_default(), n.pos.line, n.pos.col, n.message)).collect();
    let tab = format!("tab:{}", fx.fs);
    let want = [
        format!("{tab} 36:15 Attempt to dereference non-container undefined"),
        format!("{tab} 37:36 Array index 3 out of bounds."),
        format!("{tab} 38:23 Can not add ValueWithUnits (map) and number."),
        "math.fs 97:12 @sqrt: Invalid number (NaN)".to_string(),
        format!("{tab} 40:15 Function undefinedFunctionCall with 0 argument(s) not found"),
    ];
    assert_eq!(notices, want);
    // The stack of the first notice: the feature lambda, then defineFeature's call.
    let n = &out.notices[0];
    assert_eq!(n.trace.last().map(|f| (&*f.function, f.module.as_ref(), f.pos.line, f.pos.col)), Some(("const fsProbe", "onshape/std/feature.fs", 61, 17)));
    assert_eq!(out.feature_name, "FS probe 1");
}

/// The live "FS errors" probe (onshape-research/live/dom/fs-errors3.txt):
/// runtime error texts and positions, printed values.
#[test]
fn fs_errors_match_onshape() {
    let fx = Fixture::new(include_str!("../../featurescript/src/fserrors3.fs.txt"));
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("fsErrors"), &fx.units).unwrap();
    let fid = f.id;
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let _ = fx.regen(&mut reg, &ps);
    let out = custom::run::output_of(fid).expect("output");
    let tab = format!("tab:{}", fx.fs);
    let mut got: Vec<String> = out
        .notices
        .iter()
        .map(|n| {
            let m = if *n.module == *tab { String::new() } else { format!("{} ", n.module.rsplit('/').next().unwrap_or_default()) };
            format!("{m}{}:{} {}", n.pos.line, n.pos.col, n.message)
        })
        .collect();
    got.sort_by_key(|s| (s.chars().next().is_some_and(|c| c.is_alphabetic()), s.split([' ', ':']).find_map(|t| t.parse::<u32>().ok()).unwrap_or(0)));
    let want = [
        "7:36 Precondition of guarded failed (x > 0)",
        "8:36 Return value should be number, was string",
        "9:24 Precondition or predicate statement must evaluate to boolean, value is number",
        "10:30 Maximum call stack depth exceeded.",
        "19:23 Array index -1 out of bounds.",
        "20:23 Array index is not an integer.",
        "21:23 Array index must be number, value is string",
        "22:23 Can not add array and number.",
        "23:23 Can not subtract number and string.",
        "24:23 Can not multiply number and string.",
        "25:23 Can not divide number and array.",
        "26:23 Can not compare string and number.",
        "27:23 Operand for '-' was not a number.",
        "28:23 Operand for '!' was not a boolean.",
        "29:15 If statement condition must be boolean, value is number",
        "30:15 Value assigned to variable should be number, was string",
        "31:23 Call typed(number) does not match typed(string)",
        "32:15 Function two with 1 argument(s) not found",
        "33:44 Called value must be function, value is undefined",
        "34:15 Iterator loop requires map or array, value is number",
        "35:23 Attempt to dereference non-container \"abc\"",
        "36:36 Attempt to dereference non-container 5",
        "37:15 throw custom",
        "38:15 throw my message",
        "39:15 throw { \"a\" : 1 }",
        "40:23 Type error: value \"FOO\" cannot be enum Shade",
        "41:23 Type error: value 1 cannot be enum Shade",
        "46:37 Box-style access requires box, value is number",
        "48:23 Invalid number (NaN)",
        "52:23 Can not compare map and map.",
        "73:82 Type error: value 1 cannot be enum Shade",
        "units.fs 558:5 Precondition of operator< failed (ValueWithUnits compared to nonzero number)",
        "debug.fs 563:5 @printTimer: Unknown timer: \"nope\"",
        "units.fs 577:14 Precondition of operator+ failed (lhs.unit == rhs.unit)",
        "units.fs 888:12 @acos: Invalid number (NaN)",
        "evaluate.fs 1153:12 @evPlane: CANNOT_RESOLVE_ENTITIES",
    ];
    let missing: Vec<&&str> = want.iter().filter(|w| !got.iter().any(|g| g == *w)).collect();
    let extra: Vec<&String> = got.iter().filter(|g| !want.contains(&g.as_str())).collect();
    assert!(missing.is_empty() && extra.is_empty(), "missing: {missing:#?}\nunexpected: {extra:#?}");
    assert_eq!(got, want, "order");

    let lines: Vec<&str> = out.output.lines().collect();
    let mut want: Vec<&str> = vec!["Execution error"; 18];
    want.extend(["custom", "{ customMessage : my message , message : CUSTOM_ERROR }", "{ a : 1 }"]);
    want.extend(["Execution error"; 7]);
    want.push("inf");
    want.extend(["Execution error"; 5]);
    want.extend(["{ undefined : 1 }", "{ faultyParameters : [ face ] , message : CANNOT_RESOLVE_ENTITIES }"]);
    assert_eq!(&lines[..want.len()], &want[..], "printed:\n{}", out.output);
    let rest = &lines[want.len()..];
    assert!(rest[0].starts_with("Timer \"t1\": ") && rest[0].ends_with(" ms"), "{}", rest[0]);
    let want_rest = [
        "Execution error",
        "1e+21",
        "1e-7",
        "1.23456789123e+8",
        "-0",
        "0.3333333333333333 meter",
        "2 meter^2",
        "0.05235987755982989 radian",
        "(0.025400000000000002 meter, 0.050800000000000005 meter, 0.0762 meter)",
        "RED",
        "undefined",
    ];
    assert_eq!(&rest[1..12], &want_rest[..], "printed:\n{}", out.output);
    // Then Box: n, <function n>, the feature id, Context : BuiltIn: n (object numbers differ), and the array.
    assert!(rest[12].starts_with("Box: ") && rest[13].starts_with("<function ") && rest[15].starts_with("Context : BuiltIn: "), "{rest:?}");
    assert_eq!(rest[16], "[ a , [ b ] , { c : d } ]");
}

/// Standard library features (extrude, fillet, patterns, ...) called from a
/// custom feature (tests/fs/std_harness.fs prints one line per feature).
#[test]
fn std_features_run_in_custom_features() {
    let fx = Fixture::new(include_str!("fs/std_harness.fs"));
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("stdHarness"), &fx.units).unwrap();
    let fid = f.id;
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    let out = custom::run::output_of(fid).expect("output");
    let notices: Vec<String> = out.notices.iter().map(|n| format!("{} {}: {}", n.module, n.pos, n.message)).collect();
    eprintln!("{}\n{}\nstatus {:?}", out.output, notices.join("\n"), r.display.features.first().map(|s| (&s.status, &s.message)));
    let bad: Vec<&str> = out.output.lines().filter(|l| !l.ends_with(" ok") && !l.starts_with("parts ")).collect();
    assert!(bad.is_empty(), "{}", out.output);
}

/// Dialogs generated from the standard library's own feature preconditions
/// against the built-in feature schemas (which follow Onshape's dialogs).
#[test]
fn std_feature_dialogs_match_builtin_schemas() {
    use noshape_model::params::{Hints, ParamKind, ParamSpec, ParamValues, fill_defaults};
    let units = UnitPrefs::default();
    fn kind(p: &ParamSpec) -> String {
        match &p.kind {
            ParamKind::Bool { default } => format!("bool={default}"),
            ParamKind::Enum { options, default } => format!("enum{:?}={default}", options),
            ParamKind::Num(n) => format!("{:?} {}..{} ={}", n.kind, n.min, n.max, n.default_metric),
            ParamKind::Text { default } => format!("text={default:?}"),
            ParamKind::Query { max_picks, .. } => format!("query max {max_picks:?}"),
            ParamKind::Array { item, .. } => format!("array[{}]", item.iter().map(|i| format!("{}:{}", i.id, kind(i))).collect::<Vec<_>>().join(", ")),
            ParamKind::Lookup { table } => format!("lookup {table}"),
            ParamKind::Sketch => "sketch".into(),
        }
    }
    let describe = |t: &noshape_model::features::FeatureType| {
        let mut v = ParamValues::default();
        fill_defaults(&t.params, &mut v, &units);
        let shown: Vec<String> = t.params.iter().filter(|p| p.is_visible(&v) && !p.hints.has(Hints::ALWAYS_HIDDEN)).map(|p| format!("{}:{}", p.id, p.label)).collect();
        let all: Vec<String> = t.params.iter().filter(|p| !p.hints.has(Hints::ALWAYS_HIDDEN)).map(|p| format!("{} {}", p.id, kind(p))).collect();
        (shown, all)
    };
    let mut report = String::new();
    let (mut same, mut total) = (0, 0);
    let mut identical: Vec<&str> = Vec::new();
    for (file, src) in noshape_featurescript::STD_SOURCES {
        for line in src.lines() {
            let Some(name) = line.strip_prefix("export const ").and_then(|l| l.split_once(" = defineFeature")).map(|(n, _)| n) else { continue };
            let Some(builtin) = noshape_model::features::get(name) else { continue };
            total += 1;
            let mut doc = Document::new();
            let tab = doc.add_tab(file, TabContent::FeatureStudio(FeatureStudio::new(src)));
            let compiled = custom::compile(tab, src);
            let Some(def) = compiled.feature(name) else {
                report.push_str(&format!("{file} {name}: not generated ({:?})\n", compiled.diagnostics.iter().take(2).map(|d| d.to_string()).collect::<Vec<_>>()));
                continue;
            };
            let (fs_shown, fs_all) = describe(def.feature_type);
            let (b_shown, b_all) = describe(builtin);
            if fs_shown == b_shown && fs_all == b_all {
                same += 1;
                identical.push(name);
                continue;
            }
            report.push_str(&format!("== {name} ({file})\n"));
            if fs_shown != b_shown {
                report.push_str(&format!("  shown fs:      {fs_shown:?}\n  shown builtin: {b_shown:?}\n"));
            }
            let id_of = |s: &String| s.split(' ').next().unwrap_or_default().to_string();
            for x in &fs_all {
                match b_all.iter().find(|y| id_of(y) == id_of(x)) {
                    None => report.push_str(&format!("  only fs:      {x}\n")),
                    Some(y) if y != x => report.push_str(&format!("  fs:      {x}\n  builtin: {y}\n")),
                    _ => {}
                }
            }
            for y in b_all.iter().filter(|y| !fs_all.iter().any(|x| id_of(x) == id_of(y))) {
                report.push_str(&format!("  only builtin: {y}\n"));
            }
            let order = |v: &[String], other: &[String]| v.iter().map(id_of).filter(|i| other.iter().any(|o| id_of(o) == *i)).collect::<Vec<_>>();
            if order(&fs_all, &b_all) != order(&b_all, &fs_all) {
                report.push_str("  (order differs)\n");
            }
        }
    }
    eprintln!("{same} of {total} identical: {identical:?}\n{report}");
    // The others differ where the built-in schema deviates from the std
    // (labels, enum ids, bounds) or use lookup tables / reference types.
    let expected = EXPECTED_IDENTICAL_DIALOGS;
    let lost: Vec<&&str> = expected.iter().filter(|n| !identical.contains(n)).collect();
    assert!(lost.is_empty(), "dialogs no longer identical: {lost:?}\n{report}");
}

/// Standard library features whose generated dialog equals the built-in one.
const EXPECTED_IDENTICAL_DIALOGS: &[&str] = &[
    "bodyDraft", "booleanBodies", "boundarySurface", "chamfer", "circularPattern", "compositeCurve", "compositePart", "constrainedSurface", "cPlane", "cPoint", "curvePattern",
    "cutlist", "deleteBodies", "deleteFace", "draft", "enclose", "endcap", "extendSurface", "faceBlend", "intersectionCurve", "fill", "fitSpline", "frameTrim", "gusset", "isocline",
    "linearPattern", "mateConnector", "mirror", "modifyFillet", "trimCurve", "moveFace", "nameEntity", "offsetSurface", "cube", "projectCurves", "replaceFace", "revolve", "rib",
    "shell", "splitPart", "sweep", "thicken", "copyPart", "wrap",
];

/// A "Lookup Table" parameter: the dialog's levels come from the table and
/// the feature gets a LookupTablePath ({ level : key }).
#[test]
fn lookup_table_parameters() {
    let src = studio(
        r##"
const SIZES = {
    "name" : "standard", "displayName" : "Standard", "default" : "ISO",
    "entries" : {
        "ISO" : { "name" : "size", "displayName" : "Size", "default" : "M4", "entries" : { "M3" : { "d" : "3 mm" }, "M4" : { "d" : "4 mm" } } },
        "ANSI" : { "name" : "size", "displayName" : "Size", "default" : "#8", "entries" : { "#6" : { "d" : "0.138 in" }, "#8" : { "d" : "0.164 in" } } }
    }
};
annotation { "Feature Type Name" : "Bolt size" }
export const boltSize = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Size", "Lookup Table" : SIZES }
        definition.size is LookupTablePath;
    }
    {
        println(definition.size);
        println(lookupTableGetValue(getLookupTable(SIZES, definition.size).d));
    });
"##,
    );
    let fx = Fixture::new(&src);
    let t = noshape_model::features::get(&fx.ty("boltSize")).unwrap();
    let noshape_model::params::ParamKind::Lookup { table } = &t.params[0].kind else { panic!("{:?}", t.params[0].id) };
    use noshape_model::features::hole_tables as ht;
    assert_eq!(ht::table_level_name(table, &[]), Some("Standard"));
    assert_eq!(ht::table_levels(table, &[]).iter().map(|o| o.0.as_str()).collect::<Vec<_>>(), ["ANSI", "ISO"]);
    assert_eq!(ht::complete_path(table, &[]), ["ISO", "M4"]);
    assert_eq!(ht::complete_path(table, &["ANSI".to_string()]), ["ANSI", "#8"]);

    let mut ps = PartStudio::new();
    let mut f = ps.new_feature(&fx.ty("boltSize"), &fx.units).unwrap();
    f.params.set("size", ParamValue::Lookup(vec!["ANSI".into(), "#6".into()]));
    let fid = f.id;
    ps.features.push(f);
    let mut reg = Regenerator::new();
    assert_ok(&fx.regen(&mut reg, &ps));
    let out = custom::run::output_of(fid).unwrap().output;
    assert_eq!(out, "{ size : #6 , standard : ANSI }\n0.0035052000000000004 meter\n", "{out}");
}
