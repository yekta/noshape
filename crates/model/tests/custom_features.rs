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
            let output: String = custom::run::outputs().values().map(|o| o.output.clone()).collect();
            panic!("feature failed: {:?}\nnotices:\n{}\noutput:\n{output}", s.message, notices.join("\n"));
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
        const bx = evBox3d(context, { "topology" : parts });
        if (abs(bx.maxCorner[2] - bx.minCorner[2] - 13 * millimeter) > 1e-9 * meter)
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
            ParamKind::Reference { kind } => format!("reference {kind:?}"),
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

/// Labelled selects (`UIHint.SHOW_LABEL`) as the std FeatureScript declares
/// them: a built-in dialog shows the label where Onshape does (qa5: Fill's
/// Continuity, Boundary surface's Boundary condition, Bend alignment, Jog's
/// Bounding type and Jog offset anchor were bare selects).
#[test]
fn std_show_label_hints_match_builtin() {
    use noshape_model::params::{Hints, ParamKind, ParamSpec};
    fn labelled(specs: &[ParamSpec], prefix: &str, out: &mut Vec<String>) {
        for p in specs {
            if p.hints.has(Hints::SHOW_LABEL) && !p.hints.has(Hints::ALWAYS_HIDDEN) {
                out.push(format!("{prefix}{}", p.id));
            }
            if let ParamKind::Array { item, .. } = &p.kind {
                labelled(item, &format!("{prefix}{}.", p.id), out);
            }
        }
    }
    let mut report = String::new();
    for (file, src) in noshape_featurescript::STD_SOURCES {
        for line in src.lines() {
            let Some(name) = line.strip_prefix("export const ").and_then(|l| l.split_once(" = defineFeature")).map(|(n, _)| n) else { continue };
            let Some(builtin) = noshape_model::features::get(name) else { continue };
            let mut doc = Document::new();
            let tab = doc.add_tab(file, TabContent::FeatureStudio(FeatureStudio::new(src)));
            let compiled = custom::compile(tab, src);
            let Some(def) = compiled.feature(name) else { continue };
            let (mut fs, mut b) = (Vec::new(), Vec::new());
            labelled(&def.feature_type.params, "", &mut fs);
            labelled(&builtin.params, "", &mut b);
            // Only parameters both dialogs have (ids the built-in renamed or
            // dropped are std_feature_dialogs_match_builtin_schemas's business).
            let mut all_fs = Vec::new();
            let mut all_b = Vec::new();
            fn ids(specs: &[ParamSpec], prefix: &str, out: &mut Vec<String>) {
                for p in specs {
                    out.push(format!("{prefix}{}", p.id));
                    if let ParamKind::Array { item, .. } = &p.kind {
                        ids(item, &format!("{prefix}{}.", p.id), out);
                    }
                }
            }
            ids(&def.feature_type.params, "", &mut all_fs);
            ids(&builtin.params, "", &mut all_b);
            let missing: Vec<&String> = fs.iter().filter(|i| all_b.contains(i) && !b.contains(i)).collect();
            let extra: Vec<&String> = b.iter().filter(|i| all_fs.contains(i) && !fs.contains(i)).collect();
            if !missing.is_empty() || !extra.is_empty() {
                report.push_str(&format!("{name}: label missing {missing:?}, label extra {extra:?}\n"));
            }
        }
    }
    assert!(report.is_empty(), "SHOW_LABEL differs from the std:\n{report}");
}

/// Edge selection fields take sketch curves exactly where the std
/// FeatureScript's "Filter" allows them, and model edges exactly where it
/// does (qa5 SEL1: Fillet and Chamfer took a sketch line though fillet.fs and
/// chamfer.fs say `SketchObject.NO`). Every feature, every query parameter.
#[test]
fn std_edge_filters_match_builtin() {
    use noshape_model::params::{Hints, ParamKind, ParamSpec, QueryFilter};
    const MODEL_EDGES: u32 = QueryFilter::EDGE.0 | QueryFilter::SOLID_EDGE.0 | QueryFilter::LINEAR_EDGE.0 | QueryFilter::CIRCULAR_EDGE.0;
    fn filters(specs: &[ParamSpec], prefix: &str, out: &mut Vec<(String, QueryFilter)>) {
        for p in specs {
            match &p.kind {
                // Hidden fields (set by editing logic) are never picked into.
                ParamKind::Query { .. } if p.hints.has(Hints::ALWAYS_HIDDEN) => {}
                ParamKind::Query { filter, .. } => out.push((format!("{prefix}{}", p.id), *filter)),
                ParamKind::Array { item, .. } => filters(item, &format!("{prefix}{}.", p.id), out),
                _ => {}
            }
        }
    }
    let mut report = String::new();
    for (file, src) in noshape_featurescript::STD_SOURCES {
        for line in src.lines() {
            let Some(name) = line.strip_prefix("export const ").and_then(|l| l.split_once(" = defineFeature")).map(|(n, _)| n) else { continue };
            let Some(builtin) = noshape_model::features::get(name) else { continue };
            let mut doc = Document::new();
            let tab = doc.add_tab(file, TabContent::FeatureStudio(FeatureStudio::new(src)));
            let compiled = custom::compile(tab, src);
            let Some(def) = compiled.feature(name) else { continue };
            let (mut fs, mut b) = (Vec::new(), Vec::new());
            filters(&def.feature_type.params, "", &mut fs);
            filters(&builtin.params, "", &mut b);
            for (id, std_f) in &fs {
                let Some((_, ours)) = b.iter().find(|(i, _)| i == id) else { continue };
                let edges = |f: &QueryFilter| (f.0 & MODEL_EDGES != 0, f.0 & QueryFilter::SKETCH_EDGE.0 != 0);
                let (s, o) = (edges(std_f), edges(ours));
                if (s.0 || s.1 || o.0 || o.1) && s != o {
                    report.push_str(&format!("{name}.{id}: std (model edges, sketch curves) {s:?}, ours {o:?}\n"));
                }
            }
        }
    }
    assert!(report.is_empty(), "edge filters differ from the std:\n{report}");
}

/// A custom feature's compound edge filters take the part's edges, not only sketch curves (LP-DIRECTION-PICK:
/// `EntityType.EDGE && QueryFilterCompound.ALLOWS_AXIS` and `EntityType.EDGE && GeometryType.LINE` picked nothing
/// on a part, because EDGE's bits and the other side's met only in SKETCH_EDGE).
#[test]
fn compound_edge_filters_take_model_edges() {
    use noshape_model::params::{ParamKind, QueryFilter};
    let field = |filter: &str| {
        let src = studio(&format!(
            r#"
annotation {{ "Feature Type Name" : "Picks" }}
export const picks = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {{
        annotation {{ "Name" : "Edges", "Filter" : {filter}, "MaxNumberOfPicks" : 2 }}
        definition.edges is Query;
    }}
    {{
    }});
"#
        ));
        let compiled = custom::compile(TabId(1), &src);
        let def = compiled.feature("picks").unwrap_or_else(|| panic!("{filter}: {:?}", compiled.diagnostics));
        match def.feature_type.params.iter().find(|p| p.id == "edges").map(|p| &p.kind) {
            Some(ParamKind::Query { filter, .. }) => *filter,
            other => panic!("{filter}: no query field ({:?})", other.is_some()),
        }
    };
    let axis = field("EntityType.EDGE && QueryFilterCompound.ALLOWS_AXIS");
    assert!(axis.intersects(QueryFilter::LINEAR_EDGE) && axis.intersects(QueryFilter::CIRCULAR_EDGE) && axis.intersects(QueryFilter::SKETCH_EDGE), "{axis:?}");
    let line = field("EntityType.EDGE && GeometryType.LINE");
    assert!(line.intersects(QueryFilter::LINEAR_EDGE) && line.intersects(QueryFilter::SKETCH_EDGE), "{line:?}");
    assert!(!line.intersects(QueryFilter(QueryFilter::CIRCULAR_EDGE.0 | QueryFilter::EDGE.0)), "a LINE field takes no curved edge: {line:?}");
    let circle = field("EntityType.EDGE && GeometryType.CIRCLE");
    assert!(circle.intersects(QueryFilter::CIRCULAR_EDGE) && !circle.intersects(QueryFilter::LINEAR_EDGE), "{circle:?}");
    // Unchanged: a plain edge field, and edges of solids.
    assert_eq!(field("EntityType.EDGE"), QueryFilter::ANY_EDGE);
    let solid = field("EntityType.EDGE && BodyType.SOLID");
    assert!(solid.intersects(QueryFilter::EDGE) && !solid.intersects(QueryFilter::SKETCH_EDGE), "{solid:?}");
}

/// Standard library features whose generated dialog equals the built-in one.
const EXPECTED_IDENTICAL_DIALOGS: &[&str] = &[
    "bodyDraft", "booleanBodies", "boundarySurface", "chamfer", "circularPattern", "compositeCurve", "compositePart", "constrainedSurface", "cPlane", "cPoint", "curvePattern",
    "cutlist", "deleteBodies", "deleteFace", "draft", "enclose", "endcap", "extendSurface", "faceBlend", "intersectionCurve", "fill", "fitSpline", "frameTrim", "gusset", "isocline",
    "linearPattern", "mateConnector", "mirror", "modifyFillet", "trimCurve", "moveFace", "nameEntity", "offsetSurface", "cube", "projectCurves", "replaceFace", "revolve", "rib",
    "shell", "splitPart", "sweep", "thicken", "copyPart",
    // wrap: the built-in dialog has wrap.fs's collapsed "Position" group
    // (live L21); the FS compiler doesn't build that group header yet.
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
    // Source order, as Onshape's dropdowns (live L58), not the map's sorted keys.
    assert_eq!(ht::table_levels(table, &[]).iter().map(|o| o.0.as_str()).collect::<Vec<_>>(), ["ISO", "ANSI"]);
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

/// Operations and evaluations called directly from a custom feature
/// (tests/fs/ops_harness.fs checks values and prints one line per check).
#[test]
fn ops_and_evaluations_run_in_custom_features() {
    let fx = Fixture::new(include_str!("fs/ops_harness.fs"));
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("opsHarness"), &fx.units).unwrap();
    let fid = f.id;
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let _ = fx.regen(&mut reg, &ps);
    let out = custom::run::output_of(fid).expect("output");
    let notices: Vec<String> = out.notices.iter().map(|n| format!("{} {}: {}", n.module, n.pos, n.message)).collect();
    let bad: Vec<&str> = out.output.lines().filter(|l| !l.ends_with(" ok") && *l != "done").collect();
    assert!(bad.is_empty() && out.output.ends_with("done\n"), "{}\n{}", out.output, notices.join("\n"));
}

/// The same harness on 4 threads at once, twice each (lead 10-30: one land
/// run got a duplicated evOffsetDetection group, not reproduced in ~730 runs
/// here): every run passes and prints the same lines.
#[test]
fn ops_and_evaluations_are_the_same_on_many_threads() {
    let run = || {
        let fx = Fixture::new(include_str!("fs/ops_harness.fs"));
        let mut ps = PartStudio::new();
        let f = ps.new_feature(&fx.ty("opsHarness"), &fx.units).unwrap();
        let fid = f.id;
        ps.features.push(f);
        let mut reg = Regenerator::new();
        let _ = fx.regen(&mut reg, &ps);
        custom::run::output_of(fid).expect("output").output
    };
    let outs: Vec<String> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..4).map(|_| s.spawn(|| (0..2).map(|_| run()).collect::<Vec<_>>())).collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    for o in &outs {
        let bad: Vec<&str> = o.lines().filter(|l| !l.ends_with(" ok") && *l != "done").collect();
        assert!(bad.is_empty() && o.ends_with("done\n"), "{o}");
        assert_eq!(o, &outs[0]);
    }
}

/// std helix() around a circular edge, Turns and Pitch (a user's knurl
/// script: surfaceGeometry.fs calls `@alignCanonically(context, { "plane" :
/// plane })`, which the native read as a plane and every Circle helix threw).
#[test]
fn std_helix_from_a_circle_edge() {
    let src = studio(
        r#"
annotation { "Feature Type Name" : "Helix probe" }
export const helixProbe = defineFeature(function(context is Context, id is Id, definition is map)
    precondition {}
    {
        fCylinder(context, id + "cyl", { "bottomCenter" : vector(0, 0, 0) * millimeter, "topCenter" : vector(0, 0, 30) * millimeter, "radius" : 10 * millimeter });
        const circle = qNthElement(qGeometry(qCreatedBy(id + "cyl", EntityType.EDGE), GeometryType.CIRCLE), 0);
        const common = { "initEntities" : qNothing(), "axisType" : AxisType.CIRCLE, "edge" : circle, "oppositeDirection" : false,
                         "startType" : StartType.START_ANGLE, "startAngle" : 0 * degree, "endType" : EndType.HEIGHT,
                         "height" : 20 * millimeter, "handedness" : Direction.CW };
        helix(context, id + "turns", mergeMaps(common, { "pathType" : PathType.TURNS, "revolutions" : 4 }));
        helix(context, id + "pitch", mergeMaps(common, { "pathType" : PathType.PITCH, "helicalPitch" : 5 * millimeter }));
        // Four turns of radius 10 mm over 20 mm either way.
        const expected = 4 * sqrt((2 * PI * 10 * millimeter) ^ 2 + (5 * millimeter) ^ 2);
        for (var name in ["turns", "pitch"])
        {
            const edges = qCreatedBy(id + name, EntityType.EDGE);
            const length = evLength(context, { "entities" : edges });
            println(name ~ " " ~ size(evaluateQuery(context, edges)) ~ " " ~ toString(length));
            if (abs(length - expected) > 1e-3 * millimeter)
                throw regenError(name ~ ": helix length " ~ toString(length) ~ ", expected " ~ toString(expected));
        }
    });
"#,
    );
    let fx = Fixture::new(&src);
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("helixProbe"), &fx.units).unwrap();
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
}

/// A Feature Studio pinned to an older standard library gets that library
/// (a user's knurling script: `FeatureScript 1963;` and helix() with std
/// 1963's exported HelixType, private in 3095).
#[test]
fn std_1963_pinned_helix_types() {
    let body = r#"
annotation { "Feature Type Name" : "Pinned helix" }
export const pinnedHelix = defineFeature(function(context is Context, id is Id, definition is map)
    precondition {}
    {
        fCylinder(context, id + "cyl", { "bottomCenter" : vector(0, 0, 0) * millimeter, "topCenter" : vector(0, 0, 30) * millimeter, "radius" : 10 * millimeter });
        const circle = qNthElement(qGeometry(qCreatedBy(id + "cyl", EntityType.EDGE), GeometryType.CIRCLE), 0);
        const common = { "initEntities" : qNothing(), "edge" : circle, "oppositeDirection" : false, "handedness" : Direction.CW,
                         "startAngle" : 0 * degree, "height" : 20 * millimeter };
        helix(context, id + "pitch", mergeMaps(common, { "helixType" : HelixType.HEIGHT_PITCH, "helicalPitch" : 5 * millimeter }));
        helix(context, id + "turns", mergeMaps(common, { "helixType" : HelixType.HEIGHT_TURNS, "revolutions" : 4 }));
        const expected = 4 * sqrt((2 * PI * 10 * millimeter) ^ 2 + (5 * millimeter) ^ 2);
        for (var name in ["pitch", "turns"])
        {
            const length = evLength(context, { "entities" : qCreatedBy(id + name, EntityType.EDGE) });
            if (abs(length - expected) > 1e-3 * millimeter)
                throw regenError(name ~ ": helix length " ~ toString(length) ~ ", expected " ~ toString(expected));
        }
    });
"#;
    let pinned = |v: u32| format!("FeatureScript {v};\nimport(path : \"onshape/std/geometry.fs\", version : \"{v}.0\");\n{body}");
    let fx = Fixture::new(&pinned(1963));
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("pinnedHelix"), &fx.units).unwrap();
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    // As in Onshape, std 3095 does not export HelixType.
    let compiled = custom::compile(fx.fs, &pinned(3095));
    assert!(compiled.diagnostics.iter().any(|d| d.message.contains("HelixType")), "{:?}", compiled.diagnostics);
}

/// evSurfaceDefinition of a B-spline face: std's BSplineSurface with live's
/// keys (live "fs (10-25 b)" (2): a loft from a circle to a rectangle).
#[test]
fn bspline_surface_definition() {
    let src = studio(
        r#"
annotation { "Feature Type Name" : "Spline face probe" }
export const splineFaceProbe = defineFeature(function(context is Context, id is Id, definition is map)
    precondition {}
    {
        const s1 = newSketchOnPlane(context, id + "s1", { "sketchPlane" : plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1)) });
        skCircle(s1, "c", { "center" : vector(0, 0) * millimeter, "radius" : 10 * millimeter });
        skSolve(s1);
        const s2 = newSketchOnPlane(context, id + "s2", { "sketchPlane" : plane(vector(0, 0, 20) * millimeter, vector(0, 0, 1)) });
        skRectangle(s2, "r", { "firstCorner" : vector(-8, -5) * millimeter, "secondCorner" : vector(8, 5) * millimeter });
        skSolve(s2);
        opLoft(context, id + "loft", { "profileSubqueries" : [qCreatedBy(id + "s1", EntityType.FACE), qCreatedBy(id + "s2", EntityType.FACE)], "bodyType" : ToolBodyType.SOLID });
        var splines = 0;
        for (var face in evaluateQuery(context, qCreatedBy(id + "loft", EntityType.FACE)))
        {
            const definition = evSurfaceDefinition(context, { "face" : face });
            if (definition.surfaceType != SurfaceType.SPLINE)
                continue;
            splines += 1;
            if (!(definition is BSplineSurface) || keys(definition) != ["controlPoints", "isRational", "isUPeriodic", "isVPeriodic", "surfaceType", "uDegree", "uKnots", "vDegree", "vKnots"])
                throw regenError("B-spline face: " ~ toString(keys(definition)));
            if (evSurfaceDefinition(context, { "face" : face, "returnBSplinesAsOther" : true }) != { "surfaceType" : SurfaceType.OTHER })
                throw regenError("returnBSplinesAsOther");
        }
        if (splines == 0)
            throw regenError("no B-spline face on the loft");
    });
"#,
    );
    let fx = Fixture::new(&src);
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("splineFaceProbe"), &fx.units).unwrap();
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
}

/// evCollision with one solid inside another (live "fs (10-25 f)"): touching
/// the outer boundary, each touching face interferes; touching nothing, one
/// TOOL_IN_TARGET / TARGET_IN_TOOL entry for the two bodies. std Hole's
/// dialog finds its Merge scope this way.
#[test]
fn collision_containment_as_live() {
    let src = studio(
        r#"
annotation { "Feature Type Name" : "Containment probe" }
export const containmentProbe = defineFeature(function(context is Context, id is Id, definition is map)
    precondition {}
    {
        fCuboid(context, id + "a", { "corner1" : vector(0, 0, 0) * millimeter, "corner2" : vector(40, 40, 40) * millimeter });
        fCylinder(context, id + "b", { "bottomCenter" : vector(20, 20, 26.5) * millimeter, "topCenter" : vector(20, 20, 40) * millimeter, "radius" : 2.5 * millimeter });
        fCylinder(context, id + "c", { "bottomCenter" : vector(20, 20, 10) * millimeter, "topCenter" : vector(20, 20, 30) * millimeter, "radius" : 2.5 * millimeter });
        const body = function(name) { return qCreatedBy(id + name, EntityType.BODY); };
        const types = function(tools, targets) { return mapArray(evCollision(context, { "tools" : tools, "targets" : targets }), function(x) { return x["type"]; }); };
        if (types(body("b"), body("a")) != [ClashType.INTERFERE, ClashType.INTERFERE])
            throw regenError("b in a: " ~ toString(types(body("b"), body("a"))));
        const inner = evCollision(context, { "tools" : body("c"), "targets" : body("a") });
        if (size(inner) != 1 || inner[0]["type"] != ClashType.TOOL_IN_TARGET || !areQueriesEquivalent(context, inner[0].tool, body("c")) || !areQueriesEquivalent(context, inner[0].target, body("a")))
            throw regenError("c in a: " ~ toString(inner));
        if (types(body("a"), body("c")) != [ClashType.TARGET_IN_TOOL])
            throw regenError("a around c: " ~ toString(types(body("a"), body("c"))));
    });
"#,
    );
    let fx = Fixture::new(&src);
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("containmentProbe"), &fx.units).unwrap();
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
}

/// Reported statuses replace the message but faulty parameters accumulate,
/// newest first, also through a bare enum thrown afterwards (live "fs (10-25
/// e)": std Hole's HOLE_NO_HITS keeps opHole's mapped Merge scope red).
#[test]
fn faulty_parameters_accumulate_across_reports() {
    let probe = |name: &str, body: &str| {
        format!(
            r#"
annotation {{ "Feature Type Name" : "{name}" }}
export const {name} = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {{
        annotation {{ "Name" : "A", "Filter" : EntityType.FACE }}
        definition.a is Query;
        annotation {{ "Name" : "B", "Filter" : EntityType.FACE }}
        definition.b is Query;
    }}
    {{
        reportFeatureError(context, id, ErrorStringEnum.HOLE_NO_HITS, ["a"]);
        {body}
    }});
"#
        )
    };
    let src = studio(&[
        probe("same", "reportFeatureError(context, id, ErrorStringEnum.HOLE_NO_HITS);"),
        probe("other", "reportFeatureError(context, id, ErrorStringEnum.REGEN_ERROR);"),
        probe("both", "reportFeatureError(context, id, ErrorStringEnum.HOLE_NO_HITS, [\"b\"]);"),
        probe("thrown", "throw ErrorStringEnum.HOLE_NO_HITS;"),
    ]
    .concat());
    let fx = Fixture::new(&src);
    for (name, faulty, message) in [
        ("same", vec!["a"], "None of the holes intersected a part."),
        ("other", vec!["a"], "Error regenerating."),
        ("both", vec!["b", "a"], "None of the holes intersected a part."),
        ("thrown", vec!["a"], "None of the holes intersected a part."),
    ] {
        let mut ps = PartStudio::new();
        let f = ps.new_feature(&fx.ty(name), &fx.units).unwrap();
        let fid = f.id;
        ps.features.push(f);
        let mut reg = Regenerator::new();
        let r = fx.regen(&mut reg, &ps);
        let st = r.display.feature_state(fid).expect("state");
        assert_eq!((st.faulty.clone(), st.message.as_deref()), (faulty.iter().map(|s| s.to_string()).collect::<Vec<_>>(), Some(message)), "{name}");
    }
}

/// FeatureScript debug() entities reach the feature's display state in
/// their colours, also from aborted sub-features and when the feature fails
/// (live "fs (10-25 c)": shown while its dialog is open).
#[test]
fn debug_entities_in_their_colors() {
    use noshape_model::display::ColoredDebug;
    let src = studio(
        r#"
annotation { "Feature Type Name" : "Debug probe" }
export const debugProbe = defineFeature(function(context is Context, id is Id, definition is map)
    precondition {}
    {
        fCuboid(context, id + "cube", { "corner1" : vector(0, 0, 0) * millimeter, "corner2" : vector(20, 20, 20) * millimeter });
        const cube = qCreatedBy(id + "cube", EntityType.BODY);
        debug(context, qNthElement(qCreatedBy(id + "cube", EntityType.FACE), 0));
        debug(context, qNthElement(qCreatedBy(id + "cube", EntityType.EDGE), 0), DebugColor.GREEN);
        debug(context, qNthElement(qCreatedBy(id + "cube", EntityType.VERTEX), 0), DebugColor.BLUE);
        debug(context, vector(30, 0, 0) * millimeter, DebugColor.MAGENTA);
        debug(context, vector(30, 0, 0) * millimeter, vector(30, 20, 10) * millimeter, DebugColor.ORANGE);
        throw regenError("Intentional failure after debug");
    });
"#,
    );
    let fx = Fixture::new(&src);
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("debugProbe"), &fx.units).unwrap();
    let fid = f.id;
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    let st = r.display.feature_state(fid).expect("state");
    assert_eq!(st.status, FeatureStatus::Error);
    let kinds: Vec<(&str, [u8; 3])> = st
        .debug
        .colored
        .iter()
        .map(|d| match d {
            ColoredDebug::Faces(_, c) => ("face", *c),
            ColoredDebug::Line(_, c) => ("line", *c),
            ColoredDebug::Point(_, c) => ("point", *c),
        })
        .collect();
    assert_eq!(kinds, [("face", [255, 0, 0]), ("line", [0, 255, 0]), ("point", [0, 0, 255]), ("point", [255, 0, 255]), ("line", [242, 255, 128])]);
}

/// opPolyline's bendRadii (std Routing curve): a tangent arc at each corner,
/// bend i at point i + 1; the wrong number of radii is
/// POLYLINE_BEND_RADII_INVALID_SIZE.
#[test]
fn polyline_bend_radii() {
    let src = studio(
        r#"
annotation { "Feature Type Name" : "Polyline probe" }
export const polylineProbe = defineFeature(function(context is Context, id is Id, definition is map)
    precondition {}
    {
        const mm = function(xs) { return mapArray(xs, function(x) { return x * millimeter; }); };
        const points = mm([vector(0, 0, 0), vector(50, 0, 0), vector(50, 40, 0), vector(50, 40, 30)]);
        // Live (fs 10-25 d): a tangent arc trimming both segments at each
        // bend; 0 leaves the corner sharp; a closed polyline bends its
        // closing corner too; two bends need only fit a segment together (5 + 25
        // of the 40 mm one, where half of it each would not do).
        const cases = [
            { "points" : points, "bendRadii" : mm([10, 5]), "edges" : 5, "length" : 113.5619449 },
            { "points" : points, "bendRadii" : mm([0, 5]), "edges" : 4, "length" : 117.8539816 },
            { "points" : mm([vector(0, 0, 0), vector(40, 0, 0), vector(40, 30, 0), vector(0, 30, 0), vector(0, 0, 0)]),
              "bendRadii" : mm([5, 5, 5, 5]), "edges" : 8, "length" : 131.4159265 },
            { "points" : points, "bendRadii" : mm([5, 25]), "edges" : 5, "length" : 120 - 60 + 30 * PI / 2 }];
        for (var i, c in cases)
        {
            opPolyline(context, id + ("case" ~ i), { "points" : c.points, "bendRadii" : c.bendRadii });
            const edges = qCreatedBy(id + ("case" ~ i), EntityType.EDGE);
            const length = evLength(context, { "entities" : edges });
            if (size(evaluateQuery(context, edges)) != c.edges || abs(length - c.length * millimeter) > 1e-6 * millimeter)
                throw regenError("case " ~ i ~ ": " ~ size(evaluateQuery(context, edges)) ~ " edges, " ~ toString(length));
        }
        // Live: the lines come first, then the arcs, each in path order.
        var lengths = [];
        for (var edge in evaluateQuery(context, qCreatedBy(id + "case0", EntityType.EDGE)))
        {
            const kind = evCurveDefinition(context, { "edge" : edge }) is Line ? "line" : "arc";
            lengths = append(lengths, kind ~ " " ~ roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 3));
        }
        if (lengths != ["line 40", "line 25", "line 25", "arc 15.708", "arc 7.854"])
            throw regenError("edge order: " ~ toString(lengths));
        for (var i, wrong in [{ "radii" : mm([10]), "error" : ErrorStringEnum.POLYLINE_BEND_RADII_INVALID_SIZE },
                              { "radii" : mm([30, 30]), "error" : ErrorStringEnum.POLYLINE_INVALID_BEND }])
        {
            try silent
            {
                opPolyline(context, id + ("wrong" ~ i), { "points" : points, "bendRadii" : wrong.radii });
                throw regenError("an error expected");
            }
            catch (e)
            {
                if (e.message != wrong.error)
                    throw e;
            }
        }
    });
"#,
    );
    let fx = Fixture::new(&src);
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("polylineProbe"), &fx.units).unwrap();
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
}

/// Evaluation options the natives used to ignore (found by the native
/// argument audit): evOwnerSketchPlane's `checkAllEntities` (std Sweep and
/// Loft profiles) and evRaycast's `includeIntersectionsBehind`.
#[test]
fn evaluation_options_std_passes() {
    let src = studio(
        r#"
annotation { "Feature Type Name" : "Options probe" }
export const optionsProbe = defineFeature(function(context is Context, id is Id, definition is map)
    precondition {}
    {
        for (var z in [0, 10])
        {
            const sketch = newSketchOnPlane(context, id + ("s" ~ z), { "sketchPlane" : plane(vector(0, 0, z) * millimeter, vector(0, 0, 1)) });
            skRectangle(sketch, "r", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(10, 5) * millimeter });
            skSolve(sketch);
        }
        const one = qCreatedBy(id + "s0", EntityType.EDGE);
        const both = qUnion([one, qCreatedBy(id + "s10", EntityType.EDGE)]);
        if (evOwnerSketchPlane(context, { "entity" : one, "checkAllEntities" : true }).origin != vector(0, 0, 0) * meter)
            throw regenError("one sketch: its plane");
        // Without the check the first entity's plane, with it none.
        evOwnerSketchPlane(context, { "entity" : both });
        try silent
        {
            evOwnerSketchPlane(context, { "entity" : both, "checkAllEntities" : true });
            throw regenError("two sketch planes: CANNOT_RESOLVE_PLANE expected");
        }
        catch (e)
        {
            if (e != ErrorStringEnum.CANNOT_RESOLVE_PLANE && e.message != ErrorStringEnum.CANNOT_RESOLVE_PLANE)
                throw e;
        }

        fCuboid(context, id + "box", { "corner1" : vector(0, 0, 0) * millimeter, "corner2" : vector(10, 10, 10) * millimeter });
        const faces = qCreatedBy(id + "box", EntityType.FACE);
        const ray = line(vector(20, 5, 5) * millimeter, vector(1, 0, 0));
        const ahead = evRaycast(context, { "entities" : faces, "ray" : ray, "closest" : false });
        const behind = evRaycast(context, { "entities" : faces, "ray" : ray, "closest" : false, "includeIntersectionsBehind" : true });
        if (size(ahead) != 0 || size(behind) != 2)
            throw regenError("raycast hits: " ~ size(ahead) ~ " ahead, " ~ size(behind) ~ " with those behind");
        // Live: farthest behind first, negative distances, the face and its parameter.
        if (abs(behind[0].distance + 20 * millimeter) > 1e-9 * meter || abs(behind[1].distance + 10 * millimeter) > 1e-9 * meter
            || behind[0].entityType != EntityType.FACE || norm(behind[0].parameter - vector(0.5, 0.5)) > 1e-9)
            throw regenError("raycast behind: " ~ toString(behind));

        // Live: `returnBSplinesAsOther` gives only `{ curveType : OTHER }`.
        opFitSpline(context, id + "spline", { "points" : [vector(0, 0, 30) * millimeter, vector(10, 5, 30) * millimeter, vector(20, 0, 30) * millimeter] });
        const spline = qCreatedBy(id + "spline", EntityType.EDGE);
        const asOther = evCurveDefinition(context, { "edge" : spline, "returnBSplinesAsOther" : true });
        const asSpline = evCurveDefinition(context, { "edge" : spline });
        if (asOther != { "curveType" : CurveType.OTHER } || !(asSpline is BSplineCurve) || asSpline.curveType != CurveType.SPLINE)
            throw regenError("curve definitions: " ~ toString(asOther) ~ " / " ~ toString(asSpline.curveType));

        // Live: with passOwners, faces collide as their bodies.
        fCuboid(context, id + "next", { "corner1" : vector(10, 0, 0) * millimeter, "corner2" : vector(20, 10, 10) * millimeter });
        const bodies = evCollision(context, { "tools" : qCreatedBy(id + "box", EntityType.BODY), "targets" : qCreatedBy(id + "next", EntityType.BODY) });
        const owners = evCollision(context, { "tools" : faces, "targets" : qCreatedBy(id + "next", EntityType.FACE), "passOwners" : true });
        if (size(bodies) == 0 || size(owners) != size(bodies))
            throw regenError("collisions: " ~ size(bodies) ~ " body against body, " ~ size(owners) ~ " faces as their owners");
    });
"#,
    );
    let fx = Fixture::new(&src);
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("optionsProbe"), &fx.units).unwrap();
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
}

/// More standard library features called from a custom feature.
#[test]
fn std_features_run_in_custom_features_2() {
    let fx = Fixture::new(include_str!("fs/std_harness2.fs"));
    let compiled = custom::compile(fx.fs, include_str!("fs/std_harness2.fs"));
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("stdHarness2"), &fx.units).unwrap();
    let fid = f.id;
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let _ = fx.regen(&mut reg, &ps);
    let out = custom::run::output_of(fid).expect("output");
    let notices: Vec<String> = out.notices.iter().map(|n| format!("{} {}: {}", n.module, n.pos, n.message)).collect();
    let bad: Vec<&str> = out.output.lines().filter(|l| !l.ends_with(" ok") && !l.starts_with("parts ")).collect();
    assert!(bad.is_empty(), "{}\n{}", out.output, notices.join("\n"));
}

/// Reference parameters (live dom/fs-refparams.txt): CSV / image / JSON file
/// tabs as TableData / ImageData / JSONData; an unset one is `{}`.
#[test]
fn reference_parameters_read_file_tabs() {
    let src = studio(
        r#"
annotation { "Feature Type Name" : "FS refs" }
export const fsRefs = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Table" }
        definition.table is TableData;
        annotation { "Name" : "Picture" }
        definition.picture is ImageData;
        annotation { "Name" : "Json" }
        definition.json is JSONData;
        annotation { "Name" : "Unset" }
        definition.unset is TableData;
    }
    {
        println(definition.table.csvData);
        println([definition.picture.imageWidth, definition.picture.imageHeight, definition.picture.mediaType, definition.picture.blobType]);
        println(definition.json.jsonData);
        println(definition.unset);
        println([definition.table.documentId, definition.table.versionId, definition.table.elementId, definition.picture.documentId, definition.json.microversionId]);
    });
"#,
    );
    let mut fx = Fixture::new(&src);
    let csv = fx.doc.add_tab("fsrefs.csv", TabContent::File(noshape_model::file_tab::FileTab::new("fsrefs.csv", b"1,2,a\n3.5,4,b\nx,y,z\n", 0)));
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    png.extend_from_slice(&[0, 0, 0, 16, 0, 0, 0, 8, 8, 2, 0, 0, 0]);
    let pic = fx.doc.add_tab("fsrefs.png", TabContent::File(noshape_model::file_tab::FileTab::new("fsrefs.png", &png, 0)));
    let json = fx.doc.add_tab("fsrefs.json", TabContent::File(noshape_model::file_tab::FileTab::new("fsrefs.json", br#"{"a": [1, 2], "b": "c"}"#, 0)));
    let t = noshape_model::features::get(&fx.ty("fsRefs")).unwrap();
    assert!(matches!(t.params[0].kind, noshape_model::params::ParamKind::Reference { kind: noshape_model::params::ReferenceKind::Csv }));
    let mut ps = PartStudio::new();
    let mut f = ps.new_feature(&fx.ty("fsRefs"), &fx.units).unwrap();
    f.params.set("table", ParamValue::Text(csv.to_string()));
    f.params.set("picture", ParamValue::Text(pic.to_string()));
    f.params.set("json", ParamValue::Text(json.to_string()));
    let fid = f.id;
    ps.features.push(f);
    let opts = RegenOptions { custom_features: Some(DocumentScripts::from_document(&fx.doc)), ..Default::default() };
    let mut reg = Regenerator::new();
    assert_ok(&reg.regenerate(&ps, &ConfigValues::default(), &fx.units, &opts));
    let out = custom::run::output_of(fid).unwrap().output;
    assert_eq!(out, "[ [ 1 , 2 , a ] , [ 3.5 , 4 , b ] , [ x , y , z ] ]\n[ 16 , 8 , image/png , IMAGE ]\n{ a : [ 1 , 2 ] , b : c }\n{}\n[ BuiltIn: 1 , BuiltIn: 2 , BuiltIn: 3 , BuiltIn: 5 , BuiltIn: 12 ]\n", "{out}");
}

/// Other Part Studios for `PartStudioData` tests: one source studio.
struct SourceStudio(std::sync::Arc<noshape_model::regen::context::Context>);

impl std::fmt::Debug for SourceStudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SourceStudio")
    }
}

impl noshape_model::regen::derived::DerivedProvider for SourceStudio {
    fn context(&self, _tab: &str) -> Option<std::sync::Arc<noshape_model::regen::context::Context>> {
        Some(self.0.clone())
    }
    fn version(&self, _tab: &str) -> u64 {
        1
    }
}

#[test]
fn part_studio_reference_instantiates_parts() {
    let src = format!(
        "{}{}",
        studio(&format!("import(path : \"onshape/std/instantiator.fs\", version : \"3095.0\");\n{SHAPES}")),
        r#"
annotation { "Feature Type Name" : "Insert studio" }
export const insertStudio = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Studio" }
        definition.studio is PartStudioData;
    }
    {
        println(size(evaluateQuery(definition.studio.buildFunction({}), definition.studio.partQuery)));
        const instantiator = newInstantiator(id + "inst");
        addInstance(instantiator, definition.studio, { "transform" : transform(vector(0.1, 0, 0) * meter) });
        instantiate(context, instantiator);
    });
"#
    );
    let fx = Fixture::new(&src);
    // The source: a 20 mm box and a pin.
    let mut source = PartStudio::new();
    let mut f = source.new_feature(&fx.ty("boxAndPin"), &fx.units).unwrap();
    f.params.set("size", expr("20 mm"));
    f.params.set("radius", expr("4 mm"));
    f.params.set("fillet", expr("1 mm"));
    source.features.push(f);
    let src_ctx = fx.regen(&mut Regenerator::new(), &source);
    assert_ok(&src_ctx);
    let cube = src_ctx.context.bodies.iter().find(|b| b.faces.len() == 6).unwrap().id.clone();
    let provider: std::sync::Arc<dyn noshape_model::regen::derived::DerivedProvider> = std::sync::Arc::new(SourceStudio(src_ctx.context.clone()));
    for (value, parts) in [("aaaa".to_string(), 2), (format!("aaaa#{cube}"), 1)] {
        let mut ps = PartStudio::new();
        let mut f = ps.new_feature(&fx.ty("insertStudio"), &fx.units).unwrap();
        f.params.set("studio", ParamValue::Text(value.clone()));
        let fid = f.id;
        ps.features.push(f);
        let opts = RegenOptions { derived: Some(provider.clone()), ..fx.opts() };
        let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &fx.units, &opts);
        assert_ok(&r);
        assert_eq!(custom::run::output_of(fid).unwrap().output, format!("{parts}\n"), "{value}");
        assert_eq!(r.context.bodies.len(), parts, "{value}");
        // Moved 100 mm along x.
        let bb = r.context.bodies.iter().find(|b| b.faces.len() == 6).unwrap().shape.bounding_box(false).unwrap();
        assert!((bb.min.x - 0.1).abs() < 1e-6, "{value}: {bb:?}");
    }
}

#[test]
fn custom_tables_evaluate() {
    let src = format!(
        "{}{}",
        studio(SHAPES),
        r#"
annotation { "Table Type Name" : "Part sizes" }
export const partSizes = defineTable(function(context is Context, definition is map) returns Table
    precondition
    {
    }
    {
        const columns = [tableColumnDefinition("name", "Part"), tableColumnDefinition("faces", "Faces", TableTextAlignment.RIGHT), tableColumnDefinition("x", "Width")];
        var rows = [];
        for (var body in evaluateQuery(context, qAllModifiableSolidBodies()))
        {
            const bx = evBox3d(context, { "topology" : body });
            const faces = size(evaluateQuery(context, qOwnedByBody(body, EntityType.FACE)));
            rows = append(rows, tableRow({ "name" : templateString({ "template" : "Body #n", "n" : faces }), "faces" : faces, "x" : bx.maxCorner[0] - bx.minCorner[0] }, body));
        }
        rows = append(rows, tableRow({ "name" : tableCellError("?", "no body"), "faces" : tableCellWithInfo(0, "an info"), "x" : valueWithUnitsAndPrecision(25.4 * millimeter, 4) }));
        rows = append(rows, tableRow({ "name" : "area", "x" : 2 * inch * inch, "faces" : 1 / 3 }));
        rows = append(rows, tableRow({ "name" : "volume", "x" : 1.5 * meter ^ 3, "faces" : 30 * degree }));
        return table("Sizes", columns, rows);
    });

annotation { "Table Type Name" : "Two" }
export const two = defineTable(function(context is Context, definition is map) returns TableArray
    precondition
    {
    }
    {
        return tableArray([table("First", [tableColumnDefinition("a", "A")], [tableRow({ "a" : "one" })]), table("Second", [tableColumnDefinition("b", "B")], [])]);
    });

annotation { "Table Type Name" : "Nonconforming" }
export const bad = defineTable(function(context is Context, definition is map) returns TableArray
    {
        return tableArray([]);
    });
"#
    );
    let fx = Fixture::new(&src);
    let scripts = DocumentScripts::from_document(&fx.doc);
    let (_, _, compiled) = scripts.studios().next().unwrap();
    // Live dom/fs-tables-*: a table without a precondition is only warned about.
    assert_eq!(compiled.tables.iter().map(|t| t.display_name.as_str()).collect::<Vec<_>>(), ["Part sizes", "Two"]);
    let notes: Vec<String> = compiled.diagnostics.iter().map(|d| d.message.clone()).collect();
    assert_eq!(notes.first().map(|s| s.as_str()), Some("Importing geometry.fs when defining tables will likely hurt performance; consider importing table.fs and specific required modules instead."), "{notes:?}");
    assert!(notes.iter().any(|n| n == "Nonconforming table function 'bad': Missing precondition"), "{notes:?}");
    let mut ps = PartStudio::new();
    let mut f = ps.new_feature(&fx.ty("boxAndPin"), &fx.units).unwrap();
    f.params.set("size", expr("20 mm"));
    f.params.set("radius", expr("4 mm"));
    f.params.set("fillet", expr("1 mm"));
    ps.features.push(f);
    let r = fx.regen(&mut Regenerator::new(), &ps);
    assert_ok(&r);
    let out = custom::table::evaluate_table(&*scripts, &fx.fs.to_string(), "partSizes", &r.context, &fx.units);
    let tables = out.tables.expect("table runs");
    assert_eq!(tables.len(), 1);
    let t = &tables[0];
    assert_eq!(t.title, "Sizes");
    assert_eq!(t.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Part", "Faces", "Width"]);
    assert_eq!(t.columns[1].alignment.as_deref(), Some("RIGHT"));
    assert_eq!(t.rows.len(), 5);
    let cube = t.rows.iter().find(|r| r.cell("faces").map(|c| c.text.as_str()) == Some("6")).expect("cube row");
    assert_eq!(cube.cell("name").unwrap().text, "Body 6");
    assert_eq!(cube.cell("x").unwrap().text, "20 mm");
    assert_eq!(cube.entities.len(), 1);
    let err_row = &t.rows[2];
    let err = err_row.cell("name").unwrap();
    assert_eq!((err.text.as_str(), err.error.as_deref()), ("?", Some("no body")));
    let info = err_row.cell("faces").unwrap();
    assert_eq!((info.text.as_str(), info.info.as_deref()), ("0", Some("an info")));
    // Live cell texts: precision keeps its zeros, document units, superscripts.
    assert_eq!(err_row.text("x"), "25.4000 mm");
    assert_eq!((t.rows[3].text("x"), t.rows[3].text("faces")), ("1290.32 mm²", "0.333"));
    assert_eq!((t.rows[4].text("x"), t.rows[4].text("faces")), ("1500000000 mm³", "30 deg"));
    assert_eq!(t.rows[3].text("missing"), "Not set");
    // A TableArray shows only its non-empty tables.
    let two = custom::table::evaluate_table(&*scripts, &fx.fs.to_string(), "two", &r.context, &fx.units).tables.unwrap();
    assert_eq!(two.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["First"]);
}

/// Feature Studio › Import of other tabs (live dom/fs-import-*): a file
/// exports BLOB_DATA (its reference-parameter map), a Part Studio `build`;
/// two un-namespaced file imports clash on BLOB_DATA.
#[test]
fn imports_of_files_and_part_studios() {
    let mut doc = Document::new();
    let csv = doc.add_tab("fsrefs.csv", TabContent::File(noshape_model::file_tab::FileTab::new("fsrefs.csv", b"1,2,a\n3.5,4,b\nx,y,z\n", 0)));
    let json = doc.add_tab("fsrefs.json", TabContent::File(noshape_model::file_tab::FileTab::new("fsrefs.json", br#"{"a": [1, 2], "b": "c"}"#, 0)));
    let ps = doc.add_tab("Part Studio 1", TabContent::PartStudio(PartStudio::new()));
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    png.extend_from_slice(&[0, 0, 0, 16, 0, 0, 0, 8, 8, 2, 0, 0, 0]);
    let img = doc.add_tab("fsrefs.png", TabContent::File(noshape_model::file_tab::FileTab::new("fsrefs.png", &png, 0)));
    let src = format!(
        "FeatureScript 3095;\nimport(path : \"onshape/std/geometry.fs\", version : \"3095.0\");\nCsv::import(path : \"{csv}\", version : \"0\");\nPs::import(path : \"{ps}\", version : \"0\");\nImg::import(path : \"{img}\", version : \"0\");\n{}",
        r#"
annotation { "Feature Type Name" : "Imports" }
export const imports = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        println(Csv::BLOB_DATA.csvData);
        println(Csv::BLOB_DATA.blobType);
        println(Ps::build);
        const sketch = newSketchOnPlane(context, id + "sk", { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1)) });
        skImage(sketch, "image1", { "blobInfo" : Img::BLOB_DATA, "firstCorner" : vector(0, 0) * inch, "secondCorner" : vector(1, 1) * inch });
        skSolve(sketch);
    });
"#
    );
    let fs = doc.add_tab("Feature Studio 1", TabContent::FeatureStudio(FeatureStudio::new(&src)));
    let scripts = DocumentScripts::from_document(&doc);
    let compiled = scripts.studios().find(|(t, _, _)| *t == fs).unwrap().2.clone();
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    let units = UnitPrefs::default();
    let mut host = PartStudio::new();
    let f = host.new_feature(&custom::type_id(fs, "imports"), &units).unwrap();
    let fid = f.id;
    host.features.push(f);
    let opts = RegenOptions { custom_features: Some(scripts), ..Default::default() };
    let r = Regenerator::new().regenerate(&host, &ConfigValues::default(), &units, &opts);
    assert_ok(&r);
    assert_eq!(custom::run::output_of(fid).unwrap().output, "[ [ 1 , 2 , a ] , [ 3.5 , 4 , b ] , [ x , y , z ] ]\nCSV\n<function 6>\n");
    // skImage: the picture in its box (live document/91).
    assert_eq!(r.context.sketches.iter().map(|s| s.images.len()).sum::<usize>(), 1);

    // Two file imports without namespaces: BLOB_DATA is defined twice.
    let clash = format!("FeatureScript 3095;\nimport(path : \"{csv}\", version : \"0\");\nimport(path : \"{json}\", version : \"0\");\n");
    if let Some(TabContent::FeatureStudio(f)) = doc.tab_mut(fs).map(|t| &mut t.content) {
        f.source = clash;
    }
    let scripts = DocumentScripts::from_document(&doc);
    let compiled = scripts.studios().find(|(t, _, _)| *t == fs).unwrap().2.clone();
    let notes: Vec<&str> = compiled.diagnostics.iter().map(|d| d.message.as_str()).collect();
    assert!(notes.contains(&"Duplicate top level symbol name 'BLOB_DATA'") && notes.contains(&"Duplicate top level symbol name 'processedForeingDataInfo'"), "{notes:?}");
}

/// A configured source for `PartStudioData` (live dom/fs-refcfg): regenerates
/// in the asked configuration.
struct ConfiguredStudio {
    ps: PartStudio,
    opts: RegenOptions,
}

impl std::fmt::Debug for ConfiguredStudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ConfiguredStudio")
    }
}

impl noshape_model::regen::derived::DerivedProvider for ConfiguredStudio {
    fn context(&self, tab: &str) -> Option<std::sync::Arc<noshape_model::regen::context::Context>> {
        self.context_configured(tab, &self.ps.configuration.defaults())
    }
    fn context_configured(&self, _tab: &str, config: &ConfigValues) -> Option<std::sync::Arc<noshape_model::regen::context::Context>> {
        let mut values = self.ps.configuration.defaults();
        values.0.extend(config.0.clone());
        Some(Regenerator::new().regenerate(&self.ps, &values, &UnitPrefs::default(), &self.opts).context)
    }
    fn configuration(&self, _tab: &str) -> Option<noshape_model::configuration::Configuration> {
        Some(self.ps.configuration.clone())
    }
    fn feature_type(&self, _tab: &str, feature: noshape_model::reference::FeatureId) -> Option<String> {
        self.ps.features.iter().find(|f| f.id == feature).map(|f| f.type_id.clone())
    }
    fn version(&self, _tab: &str) -> u64 {
        2
    }
}

/// A part made by Sketch + Extrude prints as Onshape's SWEPT_BODY query
/// (live dom/fs-partquery) and still resolves in `buildFunction`'s context.
#[test]
fn part_studio_reference_swept_body_query() {
    let src = studio(
        r#"
annotation { "Feature Type Name" : "Part query" }
export const partQuery = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Studio" }
        definition.studio is PartStudioData;
    }
    {
        println(definition.studio.partQuery);
        println(size(evaluateQuery(definition.studio.buildFunction({}), definition.studio.partQuery)));
    });
"#,
    );
    let fx = Fixture::new(&src);
    let plate = noshape_model::demo::plate_studio();
    let extrude = plate.features.iter().find(|f| f.type_id == "extrude").unwrap().id;
    let sketch = plate.features.iter().find(|f| f.type_id == "sketch").unwrap().id;
    let source = ConfiguredStudio { ps: plate, opts: fx.opts() };
    let ctx = noshape_model::regen::derived::DerivedProvider::context(&source, "aaaa").unwrap();
    let body = ctx.bodies.iter().find(|b| b.id.feature == extrude).expect("the extruded plate").id.clone();
    let provider: std::sync::Arc<dyn noshape_model::regen::derived::DerivedProvider> = std::sync::Arc::new(source);
    let mut ps = PartStudio::new();
    let mut f = ps.new_feature(&fx.ty("partQuery"), &fx.units).unwrap();
    f.params.set("studio", ParamValue::Text(format!("aaaa#{body}")));
    let fid = f.id;
    ps.features.push(f);
    let opts = RegenOptions { derived: Some(provider), ..fx.opts() };
    assert_ok(&Regenerator::new().regenerate(&ps, &ConfigValues::default(), &fx.units, &opts));
    let out = custom::run::output_of(fid).unwrap().output;
    let head = format!("{{ queryType : UNION , subqueries : [ {{ disambiguationData : [ {{ disambiguationType : ORIGINAL_DEPENDENCY , originals : [ {{ entityType : EDGE , historyType : CREATION , operationId : [ {sketch}.wireOp ] , queryType : SKETCH_ENTITY , sketchEntityId : ");
    assert!(out.starts_with(&head), "{out}");
    assert!(out.contains(&format!("entityType : BODY , historyType : CREATION , operationId : [ {extrude}.opExtrude ] , queryType : SWEPT_BODY }} ] }}\n")), "{out}");
    assert!(out.ends_with("\n1\n"), "{out}");
}

#[test]
fn part_studio_reference_configurations() {
    use noshape_model::configuration::{ConfigInput, ConfigInputKind, ListOption, VariableKind};
    let src = format!(
        "{}{}",
        studio(SHAPES),
        r#"
annotation { "Feature Type Name" : "Show studio" }
export const showStudio = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Studio" }
        definition.studio is PartStudioData;
    }
    {
        println(definition.studio.configuration);
        println(definition.studio.configurationData);
        println(definition.studio.referenceType);
        println(definition.studio.partQuery);
        const built = definition.studio.buildFunction({ "Len" : 30 * millimeter, "Size" : "Large" });
        const cube = qContainsPoint(qEverything(EntityType.BODY), vector(30, 15, 15) * millimeter);
        println(size(evaluateQuery(built, cube)));
    });
"#
    );
    let fx = Fixture::new(&src);
    let mut source = PartStudio::new();
    source.configuration.inputs.push(ConfigInput { id: "len1".into(), name: "Len".into(), kind: ConfigInputKind::Variable { kind: VariableKind::Length, default: ExprValue::new("10 mm"), min: None, max: None }, visible_when: None, excluded: false });
    source.configuration.inputs.push(ConfigInput {
        id: "size1".into(),
        name: "Size".into(),
        kind: ConfigInputKind::List { options: vec![ListOption { id: "s".into(), name: "Small".into() }, ListOption { id: "l".into(), name: "Large".into() }], default: "s".into() },
        visible_when: None,
        excluded: false,
    });
    source.configuration.inputs.push(ConfigInput { id: "hole1".into(), name: "Hole".into(), kind: ConfigInputKind::Checkbox { default: false }, visible_when: None, excluded: false });
    let mut f = source.new_feature(&fx.ty("boxAndPin"), &fx.units).unwrap();
    f.params.set("size", expr("#Len"));
    f.params.set("radius", expr("4 mm"));
    f.params.set("fillet", expr("1 mm"));
    source.features.push(f);
    let provider: std::sync::Arc<dyn noshape_model::regen::derived::DerivedProvider> = std::sync::Arc::new(ConfiguredStudio { ps: source, opts: fx.opts() });
    let mut ps = PartStudio::new();
    let mut f = ps.new_feature(&fx.ty("showStudio"), &fx.units).unwrap();
    // The chosen configuration: Len 20 mm, Hole on.
    f.params.set("studio", ParamValue::Text("aaaa\tlen1=20 mm;hole1=true".into()));
    let fid = f.id;
    ps.features.push(f);
    let opts = RegenOptions { derived: Some(provider), ..fx.opts() };
    assert_ok(&Regenerator::new().regenerate(&ps, &ConfigValues::default(), &fx.units, &opts));
    let out = custom::run::output_of(fid).unwrap().output;
    assert_eq!(
        out,
        "{ Hole : true , Len : 0.02 meter , Size : Small }\n\
         { Hole : { defaultValue : false } , Len : { defaultValue : 0.01 meter } , Size : { defaultValue : Small , options : { Large : Large , Small : Small } } }\n\
         WORKSPACE\n\
         { queryType : UNION , subqueries : [ { queryType : UNION , subqueries : [ { entityType : BODY , queryType : EVERYTHING } ] } ] }\n\
         1\n"
    );
}

/// std sheet metal (sheetMetalStart & co.) through updateSheetMetalGeometry,
/// against live dom/fs-smprobe{,2}-{1,2}.txt (tests/fs/sm_harness.fs).
#[test]
#[ignore = "T2: 45 s std sheet metal"]
fn std_sheet_metal_matches_onshape() {
    let fx = Fixture::new(include_str!("fs/sm_harness.fs"));
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("smHarness"), &fx.units).unwrap();
    let fid = f.id;
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let _ = fx.regen(&mut reg, &ps);
    let out = custom::run::output_of(fid).expect("output");
    // With each notice's call trace (innermost last).
    let notices: Vec<String> = out.notices.iter().map(|n| format!("{} {}: {}{}", n.module, n.pos, n.message, n.trace.iter().map(|f| format!("\n    {} {} {}", f.function, f.module, f.pos)).collect::<String>())).collect();
    println!("{}\n{}", out.output, notices.join("\n"));
    // Face areas (mm², largest first) of the live parts (sm2 = sm1 + std Flange).
    let want = [
        "sm1 parts 1 areas [ 800 , 800 , 80 , 80 , 40 , 40 ]", "sm3 parts 1 areas [ 420 , 420 , 270 , 270 , 82.4668 , 47.1239 , 42 , 42 , 27 , 27 , 22.5 , 22.5 , 6.4795 , 6.4795 ]", "sm2 parts 1 areas [ 760 , 760 , 280 , 280 , 188.4956 , 80 , 80 , 62.8319 , 38 , 38 , 14 , 14 , 6.2832 , 6.2832 ]", "sm2 flat ok", "parts 3", "sm3 definitions { EDGE : 8 , FACE : 4 , NONE : 0 , VERTEX : 2 }",
        // sm4: CONVERT tray, rips with the minimal clearance and simple corners (live dom/fs-smprobe3-4.txt).
        "sm4 parts 1 areas [ 1064 , 1064 , 759.24 , 759.24 , 759.24 , 759.24 , 569.24 , 569.24 , 569.24 , 569.24 , 119.3805 , 119.3805 , 87.9646 , 87.9646 , 59.6903 , 59.6903 , 43.9823 , 43.9823 , 39.96 , 39.96 , 29.96 , 29.96 , 19 , 19 , 19 , 19 , 19 , 19 , 19 , 19 , 2.3562 , 2.3562 , 2.3562 , 2.3562 , 2.3562 , 2.3562 , 2.3562 , 2.3562 , 0.98 , 0.98 , 0.98 , 0.98 , 0.98 , 0.98 , 0.98 , 0.98 ]",
        "sm4 joints { BEND : 4 , RIP : 4 }",
        // sm7: closed Hem, a rolled 180° bend in the definition (live dom/fs-smprobe3-7.txt).
        "sm7 parts 1 areas [ 719.6 , 719.6 , 419.6 , 419.6 , 252.584 , 80 , 80 , 35.98 , 35.98 , 20.98 , 20.98 , 6.346 , 6.346 , 1.2566 ]",
        // sm8: a Tab merged into the plate (live dom/fs-smprobe3-8.txt).
        "sm8 parts 1 areas [ 850 , 850 , 80 , 40 , 40 , 30 , 30 , 20 , 10 , 10 ]",
        // Bend reliefs of partial flanges and corner reliefs (live dom/fs-smprobe3-{5,6,6c}.txt,
        // dom/fs-smprobe4-*.txt). Not yet equal: ROUND (sm6b, sm6d), CLOSED (sm6h).
        "sm5 flanged parts 1 areas [ 767.9534 , 767.9534 , 100 , 100 , 94.2478 , 80 , 40 , 40 , 40 , 31.4159 , 15.75 , 15.75 , 10 , 10 , 6.6759 , 6.6759 , 6.2832 , 6.2832 , 4 , 4 , 2 , 2 ]",
        "sm5b flanged parts 1 areas [ 740.2329 , 740.2329 , 100 , 100 , 94.2478 , 62.8319 , 40 , 20 , 20 , 20 , 8.5 , 8.5 , 6 , 6 , 5 , 5 , 4 , 4 , 3.927 , 3.927 , 2.3562 , 2.3562 ]",
        "sm5c flanged parts 1 areas [ 739.75 , 739.75 , 100 , 100 , 94.2478 , 62.8319 , 40 , 20 , 20 , 20 , 8.5 , 8.5 , 6.75 , 6.75 , 5 , 5 , 4.75 , 4.75 , 3.927 , 3.927 , 1.5 , 1.5 ]",
        "sm5d flanged parts 1 areas [ 760 , 760 , 100 , 100 , 94.2478 , 62.8319 , 40 , 20 , 20 , 20 , 10 , 10 , 5 , 5 , 2.4071 , 2.4071 , 0.4802 , 0.4802 ]",
        "sm6 flanged parts 1 areas [ 741 , 741 , 199.9 , 199.9 , 183.7832 , 99.9 , 99.9 , 89.5354 , 79.96 , 78 , 61.2611 , 39.96 , 38 , 29.8451 , 10 , 10 , 10 , 10 , 6.2832 , 6.2832 , 6.2832 , 6.2832 , 1.96 , 1.96 ]",
        "sm6c flanged parts 1 areas [ 740.4433 , 740.4433 , 198.6121 , 198.6121 , 180.2671 , 98.6121 , 98.6121 , 86.0193 , 79.96 , 78 , 60.089 , 39.96 , 38 , 28.6731 , 10 , 10 , 8.5077 , 8.5077 , 6.2832 , 6.2832 , 6.2832 , 6.2832 , 3.4523 , 3.4523 , 1.4923 , 1.4923 , 1.4923 , 1.4923 ]",
        "sm6e flanged parts 1 areas [ 684 , 684 , 199.9 , 199.9 , 179.0708 , 119.3805 , 99.9 , 99.9 , 84.823 , 56.5487 , 39.98 , 38 , 19.98 , 18 , 5 , 5 , 5 , 5 , 3.927 , 3.927 , 3.927 , 3.927 , 1.98 , 1.98 ]",
        "sm6f flanged parts 1 areas [ 683.0743 , 683.0743 , 197.0694 , 197.0694 , 174.5369 , 116.358 , 97.0694 , 97.0694 , 80.2892 , 53.5261 , 39.98 , 38 , 19.98 , 18 , 5 , 5 , 4.0379 , 4.0379 , 3.927 , 3.927 , 3.927 , 3.927 , 2.9421 , 2.9421 , 0.9621 , 0.9621 , 0.9621 , 0.9621 ]",
        "sm6g flanged parts 1 areas [ 683.6685 , 683.6685 , 198.4285 , 198.4285 , 176.3575 , 117.5717 , 98.4285 , 98.4285 , 82.1097 , 54.7398 , 39.98 , 38 , 19.98 , 18 , 5 , 5 , 4.4242 , 4.4242 , 3.927 , 3.927 , 3.927 , 3.927 , 2.5558 , 2.5558 , 0.5758 , 0.5758 , 0.5758 , 0.5758 ]",
    ];
    let lines: Vec<&str> = out.output.lines().collect();
    assert!(want.iter().all(|w| lines.contains(w)), "{}\n{}", out.output, notices.join("\n"));
}

/// Exact numbers of common std features (tests/fs/std_reference.fs, the same
/// file run live: dom/fs-stdref.txt). Each line: volume, area, face/edge/vertex
/// counts, box. Not yet equal, from the kernel: filletCorner (volume 1e-6 mm³
/// off), sweepLineArc (box 1610.0001 vs 1610), loftSquareCircle (live splits
/// the circle into four arcs: 6/12/8 faces/edges/vertices, ours 7/15/10).
#[test]
fn std_features_match_onshape() {
    let fx = Fixture::new(include_str!("fs/std_reference.fs"));
    let compiled = custom::compile(fx.fs, include_str!("fs/std_reference.fs"));
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("stdReference"), &fx.units).unwrap();
    let fid = f.id;
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let _ = fx.regen(&mut reg, &ps);
    let out = custom::run::output_of(fid).expect("output");
    let notices: Vec<String> = out.notices.iter().map(|n| format!("{} {}: {}{}", n.module, n.pos, n.message, n.trace.iter().map(|f| format!("\n    {} {} {}", f.function, f.module, f.pos)).collect::<String>())).collect();
    println!("{}\n{}", out.output, notices.join("\n"));
    assert!(out.output.ends_with("done\n"), "{}\n{}", out.output, notices.join("\n"));
    let want = [
        "extrudeBlind v 3000 a 1300 f/e/v 6/12/8 box (0, 0, 0) (20, 10, 15)",
        "extrudeSymmetricDraft v 856.115481 a 514.921821 f/e/v 4/3/0 box (94.562557, -5.437443, -5) (105.437443, 5.437443, 5)",
        "extrudeSecondDirection v 1400 a 760 f/e/v 6/12/8 box (200, 0, -4) (210, 10, 10)",
        "revolve90 v 471.238898 a 386.305284 f/e/v 6/12/8 box (300, 0, 0) (310, 10, 8)",
        "filletEdge v 7961.371669 a 2370.384947 f/e/v 7/15/10 box (400, 0, 0) (420, 20, 20)",
        "chamferEqual v 7960 a 2372.568542 f/e/v 7/15/10 box (600, 0, 0) (620, 20, 20)",
        "chamferTwoOffsets v 7920 a 2361.442719 f/e/v 7/15/10 box (700, 0, 0) (720, 20, 20)",
        "chamferOffsetAngle v 7948.038476 a 2369.444864 f/e/v 7/15/10 box (800, 0, 0) (820, 20, 20)",
        "shellTop v 2653.5 a 3658 f/e/v 11/24/16 box (900, 0, 0) (920, 20, 20)",
        "draftTwoFaces v 8699.909308 a 2543.037732 f/e/v 6/12/8 box (998.250227, 0, 0) (1021.749773, 20, 20)",
        "booleanSubtract v 6994.690351 a 2802.12386 f/e/v 7/14/8 box (1100, 0, 0) (1120, 20, 20)",
        "booleanIntersect threw { message : BOOLEAN_BAD_INPUT }",
        "linearPatternPart v 24000 a 7200 f/e/v 18/36/24 box (1300, 0, 0) (1320, 70, 20)",
        "circularPatternPart v 251.327412 a 351.858377 f/e/v 12/8/0 box (1408, 8, 0) (1432, 32, 5)",
        "mirrorPart v 16000 a 4800 f/e/v 12/24/16 box (1500, 0, 0) (1540, 20, 20)",
        "thickenFace v 800 a 960 f/e/v 6/12/8 box (1800, 0, 20) (1820, 20, 22)",
        "moveFaceOffset v 9200 a 2640 f/e/v 6/12/8 box (1900, 0, 0) (1923, 20, 20)",
        "splitByPlane v 8000 a 3243.274043 f/e/v 12/24/16 box (2000, 0, 0) (2020, 20, 20)",
        "deleteFilletFace v 8000 a 2400 f/e/v 6/12/8 box (2100, 0, 0) (2120, 20, 20)",
    ];
    let lines: Vec<&str> = out.output.lines().collect();
    let missing: Vec<&&str> = want.iter().filter(|w| !lines.contains(w)).collect();
    assert!(missing.is_empty(), "missing {:?}\n{}", missing, out.output);
}

/// Regeneration time of std features run from FeatureScript (sheet metal
/// set, tests/fs/sm_harness.fs; extrude, fillet & co., tests/fs/std_reference.fs).
/// Run in release: `cargo test --release --test all custom_features:: -- --ignored --nocapture bench_std_regen`.
#[test]
#[ignore = "benchmark"]
fn bench_std_regen() {
    for (name, source, ty) in [("sheet metal", include_str!("fs/sm_harness.fs"), "smHarness"), ("std reference", include_str!("fs/std_reference.fs"), "stdReference")] {
        let fx = Fixture::new(source);
        let mut ps = PartStudio::new();
        let f = ps.new_feature(&fx.ty(ty), &fx.units).unwrap();
        ps.features.push(f);
        // The first run compiles std and the studio.
        let _ = fx.regen(&mut Regenerator::new(), &ps);
        let runs = 5;
        let start = std::time::Instant::now();
        for _ in 0..runs {
            let _ = fx.regen(&mut Regenerator::new(), &ps);
        }
        println!("{name}: {:.1} ms per regen", start.elapsed().as_secs_f64() * 1000.0 / runs as f64);
    }
}
