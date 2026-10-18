//! Feature Studios: custom features written in script, evaluated in Part Studios.

use noshape_model::configuration::ConfigValues;
use noshape_model::custom::{self, DocumentScripts, FeatureStudio};
use noshape_model::display::FeatureStatus;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{Reference, Role, TopoName};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::{Document, TabContent, TabId};
use noshape_units::{ExprValue, UnitPrefs};
use std::f64::consts::PI;
use std::sync::Arc;

struct Fixture {
    doc: Document,
    fs: TabId,
    units: UnitPrefs,
}

impl Fixture {
    fn new(source: &str) -> Self {
        let mut doc = Document::new();
        let fs = doc.add_tab("Feature Studio 1", TabContent::FeatureStudio(FeatureStudio { source: source.to_string() }));
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
        assert_eq!(s.status, FeatureStatus::Ok, "feature failed: {:?}", s.message);
    }
}

fn total_volume(r: &RegenResult) -> f64 {
    r.context.bodies.iter().map(|b| b.shape.volume().unwrap().abs()).sum()
}

const SHAPES: &str = r#"
feature("Box and pin", #{
    icon: "extrude",
    params: [
        length("size", "Size", mm(20)),
        length("radius", "Pin radius", mm(4)),
        length("fillet", "Pin fillet", mm(1)),
    ],
});

fn box_and_pin(ctx, p) {
    ctx.box([0, 0, 0], [p.size, p.size, p.size]);
    let pin = ctx.cylinder([mm(50), 0, 0], [0, 0, 1], p.radius, mm(10));
    // Fillet the two circular edges of the pin.
    let circles = ctx.filter(ctx.edges(pin), "CIRCLE");
    if circles.len() != 2 { throw "expected 2 circular edges, got " + circles.len(); }
    ctx.fillet(circles, p.fillet);
}
"#;

#[test]
fn box_and_filleted_cylinder() {
    let fx = Fixture::new(SHAPES);
    let scripts = DocumentScripts::from_document(&fx.doc);
    let (_, _, compiled) = scripts.studios().next().unwrap();
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    assert_eq!(compiled.features.len(), 1);

    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("Box and pin"), &fx.units).expect("custom feature type is registered");
    assert_eq!(f.name, "Box and pin 1");
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    assert_eq!(r.context.bodies.len(), 2, "box + pin");
    let cube = r.context.bodies.iter().find(|b| b.faces.len() == 6).expect("box with 6 faces");
    assert!((cube.shape.volume().unwrap() - 0.02f64.powi(3)).abs() < 1e-12);
    let pin = r.context.bodies.iter().find(|b| b.faces.len() != 6).unwrap();
    assert_eq!(pin.faces.len(), 5, "cylinder side, 2 caps, 2 fillets");
    let plain = PI * 0.004f64.powi(2) * 0.01;
    let v = pin.shape.volume().unwrap();
    assert!(v < plain && v > plain * 0.8, "filleted pin volume {v} vs {plain}");
    // Fillet faces are named as blends of the cylinder's edges.
    assert_eq!(pin.faces.iter().filter(|n| matches!(&***n, TopoName::Face { role: Role::Blend(_), .. })).count(), 2);
    // Bodies have distinct ids.
    assert_ne!(r.context.bodies[0].id, r.context.bodies[1].id);
}

const BOSS: &str = r#"
feature("Boss", #{
    params: [
        query("face", "Face", PLANAR_FACE, 1),
        length("height", "Height", mm(5)),
        bool("cut", "Cut instead", false),
    ],
});

fn boss(ctx, p) {
    if p.face.len() == 0 { throw "Select a planar face"; }
    let n = ctx.normal(p.face[0]);
    let area = ctx.area(p.face);
    if area <= 0.0 { throw "bad area"; }
    if p.cut {
        ctx.extrude(p.face, -p.height, #{ op: "remove" });
    } else {
        ctx.extrude(p.face, p.height, #{ op: "add" });
    }
}
"#;

#[test]
fn query_param_extrudes_selected_face() {
    let fx = Fixture::new(BOSS);
    let opts = fx.opts();
    let mut ps = PartStudio::new();
    let mut cube = ps.new_feature("cube", &fx.units).unwrap();
    cube.params.set("sideLength", expr("20 mm"));
    ps.features.push(cube);
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &fx.units, &opts);
    assert_ok(&r);
    // Top face of the cube: the face whose center is highest.
    let body = &r.context.bodies[0];
    let top = (0..body.faces.len())
        .max_by(|a, b| {
            let z = |i: usize| body.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().centroid().unwrap().z;
            z(*a).total_cmp(&z(*b))
        })
        .unwrap();
    let face = Reference::Face((*body.faces[top]).clone());
    let mut boss = ps.new_feature(&fx.ty("Boss"), &fx.units).unwrap();
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
    // Side faces of the boss carry the script feature's names.
    let named = r.context.bodies[0].faces.iter().filter(|n| n.creator() == Some(boss_id)).count();
    assert!(named >= 5, "boss faces named by the custom feature: {named}");

    // The bool parameter switches to a cut.
    ps.features[1].params.set("cut", ParamValue::Bool(true));
    let r = reg.regenerate(&ps, &ConfigValues::default(), &fx.units, &opts);
    assert_ok(&r);
    let expected = 0.02f64.powi(3) * 0.5;
    assert!((total_volume(&r) - expected).abs() < 1e-12);
}

const BLOCK: &str = r#"
feature("Block", #{
    params: [ length("width", "Width", mm(30)) ],
    evaluate: "make_block",
});

fn make_block(ctx, p) {
    ctx.box([0, 0, 0], [p.width, mm(20), mm(10)]);
}
"#;

#[test]
fn downstream_fillet_survives_param_change() {
    let fx = Fixture::new(BLOCK);
    let mut ps = PartStudio::new();
    let block = ps.new_feature(&fx.ty("Block"), &fx.units).unwrap();
    let block_id = block.id;
    ps.features.push(block);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);

    // The edge between the box's +X face (slot 1) and +Z face (slot 5) of
    // the script's first operation.
    let face = |slot: u32| Arc::new(TopoName::face(block_id, Role::Datum((1 << 8) | slot)));
    let edge = TopoName::edge(face(1), face(5), 0);
    assert!(r.context.bodies[0].edges.iter().any(|e| **e == edge), "script edge has a stable name: {:#?}", r.context.bodies[0].edges);

    let mut fillet = ps.new_feature("fillet", &fx.units).unwrap();
    fillet.params.set("entities", ParamValue::Query(vec![Reference::Edge(edge)]));
    fillet.params.set("radius", expr("2 mm"));
    ps.features.push(fillet);
    let removed = (4.0 - PI) / 4.0 * 0.002f64.powi(2) * 0.02;
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    let v = total_volume(&r);
    assert!((v - (0.03 * 0.02 * 0.01 - removed)).abs() < 1e-11, "volume {v}");

    // Change the script parameter: the fillet still finds its edge.
    ps.features[0].params.set("width", expr("45 mm"));
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    let v = total_volume(&r);
    assert!((v - (0.045 * 0.02 * 0.01 - removed)).abs() < 1e-11, "volume after edit {v}");

    // Editing the script body (not the declaration) keeps names too.
    let mut fx = fx;
    fx.set_source(&BLOCK.replace("ctx.box([0, 0, 0]", "ctx.box([mm(5), 0, 0]"));
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    let bb = r.context.bodies[0].shape.bounding_box(false).unwrap();
    assert!((bb.min.x - 0.005).abs() < 1e-5, "script edit regenerated the Part Studio: {}", bb.min.x);
}

#[test]
fn compile_and_runtime_errors_become_feature_errors() {
    let mut fx = Fixture::new(BLOCK);
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("Block"), &fx.units).unwrap();
    ps.features.push(f);
    let mut reg = Regenerator::new();
    assert_ok(&fx.regen(&mut reg, &ps));

    let status = |r: &RegenResult| (r.display.features[0].status, r.display.features[0].message.clone().unwrap_or_default());

    // Compile (parse) error.
    fx.set_source("feature(\"Block\", #{ params: [] });\nfn make_block(ctx, p) { let x = ; }\n");
    let (st, msg) = status(&fx.regen(&mut reg, &ps));
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.contains("Feature Studio has errors") && msg.contains("Line 2"), "{msg}");

    // Runtime error thrown by the script.
    fx.set_source(r#"feature("Block", #{ params: [] }); fn block(ctx, p) { throw "Nope, not today"; }"#);
    let (st, msg) = status(&fx.regen(&mut reg, &ps));
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.contains("Nope, not today"), "{msg}");

    // Runtime error from an operation (zero-size box) and a type error.
    fx.set_source(r#"feature("Block", #{ params: [] }); fn block(ctx, p) { ctx.box([0, 0, 0], [0, 1, 1]); }"#);
    let (st, msg) = status(&fx.regen(&mut reg, &ps));
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.contains("box size"), "{msg}");
    fx.set_source(r#"feature("Block", #{ params: [] }); fn block(ctx, p) { ctx.box("a", [1, 1, 1]); }"#);
    let (st, msg) = status(&fx.regen(&mut reg, &ps));
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.contains("vector"), "{msg}");

    // Infinite loops are stopped by the sandbox.
    fx.set_source(r#"feature("Block", #{ params: [] }); fn block(ctx, p) { loop { } }"#);
    let (st, msg) = status(&fx.regen(&mut reg, &ps));
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.contains("operation limit"), "{msg}");

    // The feature is no longer declared.
    fx.set_source(r#"feature("Other", #{ params: [] }); fn other(ctx, p) { }"#);
    let (st, msg) = status(&fx.regen(&mut reg, &ps));
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.contains("no longer defines"), "{msg}");

    // Feature Studio tab deleted.
    fx.doc.remove_tab(fx.fs);
    let (st, msg) = status(&fx.regen(&mut reg, &ps));
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.contains("no longer exists"), "{msg}");

    // Fixing the source recovers.
    let mut fx = Fixture::new(BLOCK);
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("Block"), &fx.units).unwrap();
    ps.features.push(f);
    fx.set_source("garbage (");
    assert_eq!(status(&fx.regen(&mut reg, &ps)).0, FeatureStatus::Error);
    fx.set_source(BLOCK);
    assert_ok(&fx.regen(&mut reg, &ps));
}

#[test]
fn queries_patterns_and_measurements() {
    let src = r#"
feature("Ring of pegs", #{
    params: [ int("count", "Count", 6, 1, 50), length("radius", "Radius", mm(30)), enum_("shape", "Shape", ["round", "square"]) ],
});
fn ring_of_pegs(ctx, p) {
    let base = ctx.cylinder([0, 0, 0], [0, 0, 1], p.radius + mm(10), mm(5), #{ id: "base" });
    let peg = if p.shape == "round" {
        ctx.cylinder([p.radius, 0, mm(5)], [0, 0, 1], mm(3), mm(8))
    } else {
        ctx.box([p.radius - mm(3), mm(-3), mm(5)], [mm(6), mm(6), mm(8)])
    };
    let copies = ctx.circular_pattern(peg, [0, 0, 0], [0, 0, 1], p.count, deg(360));
    if copies.len() != p.count - 1 { throw "pattern count"; }
    ctx.union(base, peg + copies);
    if ctx.parts().len() != 1 { throw "union failed"; }
    let bb = ctx.bbox(ctx.parts());
    if abs(bb.size[2] - mm(13)) > 1e-9 { throw "height " + bb.size[2]; }
    let top = ctx.filter(ctx.faces(ctx.parts()), "PLANE");
    if top.len() < 2 { throw "faces"; }
    if ctx.volume(ctx.parts()) <= 0.0 { throw "volume"; }
    if ctx.length(ctx.filter(ctx.edges(base), "CIRCLE")) <= 0.0 { throw "length"; }
}
"#;
    let fx = Fixture::new(src);
    let mut ps = PartStudio::new();
    let f = ps.new_feature(&fx.ty("Ring of pegs"), &fx.units).unwrap();
    ps.features.push(f);
    let mut reg = Regenerator::new();
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    assert_eq!(r.context.bodies.len(), 1);
    ps.features[0].params.set("shape", ParamValue::Enum("square".into()));
    ps.features[0].params.set("count", expr("4"));
    let r = fx.regen(&mut reg, &ps);
    assert_ok(&r);
    let base = PI * 0.04f64.powi(2) * 0.005;
    let pegs = 4.0 * 0.006 * 0.006 * 0.008;
    assert!((total_volume(&r) - (base + pegs)).abs() < 1e-10);
    // Out-of-range integer parameter.
    ps.features[0].params.set("count", expr("80"));
    let r = fx.regen(&mut reg, &ps);
    assert_eq!(r.display.features[0].status, FeatureStatus::Error);
}

#[test]
fn demo_feature_studio_regenerates() {
    let doc = noshape_model::demo::demo_document();
    let scripts = DocumentScripts::from_document(&doc);
    let (_, _, c) = scripts.studios().next().expect("demo has a Feature Studio");
    assert!(!c.has_errors(), "{:?}", c.diagnostics);
    assert_eq!(c.features.len(), 2);
    let tab = doc.tabs.iter().find(|t| t.name == "Flange").expect("Flange Part Studio");
    let TabContent::PartStudio(ps) = &tab.content else { panic!() };
    assert_eq!(ps.features.len(), 2);
    let opts = RegenOptions { custom_features: Some(scripts.clone()), ..Default::default() };
    let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), &doc.settings.units, &opts);
    assert_ok(&r);
    assert_eq!(r.context.bodies.len(), 1);
    // Flange: disk + hub - bore - 6 bolt holes, rim filleted, 4 holes in the hub.
    let v = total_volume(&r);
    let disk = PI * 0.04f64.powi(2) * 0.008;
    assert!(v > disk * 0.8 && v < disk * 1.4, "flange volume {v}");
    // The plate Part Studio (first tab) is unchanged and still regenerates.
    let TabContent::PartStudio(plate) = &doc.tabs[0].content else { panic!() };
    assert_ok(&Regenerator::new().regenerate(plate, &ConfigValues::default(), &doc.settings.units, &opts));
}

#[test]
fn new_feature_studio_template_works() {
    let fx = Fixture::new(&FeatureStudio::template().source);
    let mut ps = PartStudio::new();
    ps.features.push(ps.new_feature(&fx.ty("Rounded block"), &fx.units).unwrap());
    let r = fx.regen(&mut Regenerator::new(), &ps);
    assert_ok(&r);
    assert_eq!(r.context.bodies[0].faces.len(), 10, "box + 4 rounded vertical edges");
}
