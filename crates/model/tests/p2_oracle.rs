//! P2 oracle (live "psb (10-26 a) P2 oracle"): variables, configurations
//! and Derived in the Part Studio, compared with live's volumes and values.

use glam::DVec2;
use noshape_model::configuration::{ConfigInput, ConfigInputKind, ConfigValues, ConfiguredParam, ListOption, SUPPRESSED_PARAM, VariableKind};
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_sketch::ConstraintKind;
use noshape_units::{ExprValue, UnitPrefs};

fn e(s: &str) -> ParamValue {
    ParamValue::Expr(ExprValue::new(s))
}

struct Studio {
    ps: PartStudio,
    units: UnitPrefs,
}

impl Studio {
    fn new() -> Self {
        Self { ps: PartStudio::new(), units: UnitPrefs::default() }
    }

    fn add(&mut self, ty: &str, params: &[(&str, ParamValue)]) -> FeatureId {
        let mut f = self.ps.new_feature(ty, &self.units).unwrap();
        for (k, v) in params {
            f.params.set(k, v.clone());
        }
        let id = f.id;
        self.ps.features.push(f);
        id
    }

    /// A Variable feature (`ty`: LENGTH, NUMBER, ANGLE or ANY).
    fn var(&mut self, name: &str, ty: &str, value: &str) -> FeatureId {
        let key = match ty {
            "LENGTH" => "lengthValue",
            "NUMBER" => "numberValue",
            "ANGLE" => "angleValue",
            _ => "anyValue",
        };
        let v = if ty == "ANY" { ParamValue::Text(value.into()) } else { e(value) };
        self.add("assignVariable", &[("name", ParamValue::Text(name.into())), ("variableType", ParamValue::Enum(ty.into())), (key, v)])
    }

    /// A rectangle on Top from the origin, `w` × `h` (dimension
    /// expressions), extruded NEW by `d`. Returns (sketch, extrude).
    fn block(&mut self, w: &str, h: &str, d: &str) -> (FeatureId, FeatureId) {
        let mut sk = noshape_sketch::Sketch::new();
        let r = sk.corner_rectangle(DVec2::ZERO, DVec2::new(0.04, 0.02));
        sk.add_constraint(ConstraintKind::Fix { entity: r.points[0] }).unwrap();
        let cw = sk.add_constraint(ConstraintKind::Length { curve: r.curves[0], value: 0.04 }).unwrap();
        let ch = sk.add_constraint(ConstraintKind::Length { curve: r.curves[3], value: 0.02 }).unwrap();
        let mut data = SketchData { sketch: sk, ..Default::default() };
        data.dimension_exprs.insert(cw.0, ExprValue::new(w));
        data.dimension_exprs.insert(ch.0, ExprValue::new(h));
        let s = self.add("sketch", &[("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(data)))]);
        let r = self.regen(&ConfigValues::default());
        let key = r.display.sketches.iter().find(|x| x.feature == s).and_then(|x| x.regions.first()).map(|g| g.key.clone()).expect("a region");
        let x = self.add("extrude", &[("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: s, key }])), ("depth", e(d)), ("operationType", ParamValue::Enum("NEW".into()))]);
        (s, x)
    }

    fn regen(&self, cv: &ConfigValues) -> RegenResult {
        Regenerator::new().regenerate(&self.ps, cv, &self.units, &RegenOptions::default())
    }
}

fn volume_mm3(r: &RegenResult) -> f64 {
    r.context.bodies.iter().map(|b| b.shape.volume().unwrap()).sum::<f64>() * 1e9
}

fn report(case: &str, r: &RegenResult) {
    let st: Vec<String> = r.display.features.iter().map(|f| format!("{:?}{}", f.status, f.message.as_deref().map(|m| format!(" ({m})")).unwrap_or_default())).collect();
    eprintln!("{case}: volume {:.7} mm³, parts {}, statuses {st:?}", volume_mm3(r), r.context.bodies.len());
    for (k, q) in &r.display.variables {
        eprintln!("  #{k} = {q:?}");
    }
}

fn list(ps: &mut PartStudio, id: &str, name: &str, options: &[&str]) {
    ps.configuration.inputs.push(ConfigInput {
        id: id.into(),
        name: name.into(),
        kind: ConfigInputKind::List { options: options.iter().map(|o| ListOption { id: o.to_lowercase(), name: (*o).into() }).collect(), default: options[0].to_lowercase() },
        visible_when: None,
        excluded: false,
    });
}

fn cv(pairs: &[(&str, &str)]) -> ConfigValues {
    let mut c = ConfigValues::default();
    for (k, v) in pairs {
        c.0.insert((*k).into(), (*v).into());
    }
    c
}

/// V1 / V2: a variable chain with functions and mixed units; the Variable
/// table shows document units ("12.7 mm" for 0.5 in). Live rejects #d =
/// "#a + 5 mm * cos(#ang * 1 rad)" (atan(1) is an angle, so that is rad²);
/// built with cos(#ang).
#[test]
fn p2_oracle_variables() {
    for (case, a, volume, values) in [
        ("V1", "10 mm", 18877.31, [("a", "10 mm"), ("b", "45.4 mm"), ("c", "46.488 mm"), ("n", "3"), ("ang", "45 deg"), ("w", "46.488 mm"), ("h", "30 mm"), ("d", "13.536 mm")]),
        ("V2", "0.5 in", 32390.66, [("a", "12.7 mm"), ("b", "50.8 mm"), ("c", "52.363 mm"), ("n", "3"), ("ang", "45 deg"), ("w", "52.363 mm"), ("h", "38.1 mm"), ("d", "16.236 mm")]),
    ] {
        let mut s = Studio::new();
        s.var("a", "LENGTH", a);
        s.var("b", "LENGTH", "#a * 2 + 1 in");
        s.var("c", "LENGTH", "sqrt(#b^2 + #a^2)");
        s.var("n", "NUMBER", "3");
        s.var("ang", "ANY", "atan(1)");
        s.var("w", "LENGTH", "#c");
        s.var("h", "LENGTH", "#a * #n");
        s.var("d", "LENGTH", "#a + 5 mm * cos(#ang)");
        s.block("#w", "#h", "#d");
        let r = s.regen(&ConfigValues::default());
        report(case, &r);
        assert!((volume_mm3(&r) - volume).abs() < 0.01, "{case}: {} vs live {volume}", volume_mm3(&r));
        for (name, text) in values {
            let q = r.display.variables[name];
            // As the Variable table writes it (config_panel.rs: "deg", not "°").
            assert_eq!(noshape_units::format_quantity(q, &s.units).replace('°', " deg"), text, "{case} #{name}");
        }
    }
}

#[test]
fn p2_oracle_configurations() {
    // C1: configured depth, fillet suppressed only in Large.
    let mut s = Studio::new();
    let (_, x) = s.block("40 mm", "20 mm", "10 mm");
    let r = s.regen(&ConfigValues::default());
    let b = &r.context.bodies[0];
    let vertical: Vec<Reference> = b
        .shape
        .edges()
        .iter()
        .enumerate()
        .filter(|(_, ed)| {
            let i = ed.edge_info().unwrap();
            (i.start.z - i.end.z).abs() > 1e-6
        })
        .map(|(i, _)| Reference::Edge((*b.edges[i]).clone()))
        .collect();
    let fil = s.add("fillet", &[("entities", ParamValue::Query(vertical)), ("radius", e("2 mm"))]);
    list(&mut s.ps, "size", "Size", &["Small", "Medium", "Large"]);
    let depth = |v: &str| (v.to_lowercase(), e(match v { "Small" => "10 mm", "Medium" => "20 mm", _ => "35 mm" }));
    s.ps.configuration.configured.push(ConfiguredParam { input: "size".into(), feature: x, param: "depth".into(), values: ["Small", "Medium", "Large"].iter().map(|v| depth(v)).collect() });
    s.ps.configuration.configured.push(ConfiguredParam {
        input: "size".into(),
        feature: fil,
        param: SUPPRESSED_PARAM.into(),
        values: [("small".to_string(), ParamValue::Bool(false)), ("medium".to_string(), ParamValue::Bool(false)), ("large".to_string(), ParamValue::Bool(true))].into_iter().collect(),
    });
    for (size, volume) in [("small", 7965.664), ("medium", 15931.33), ("large", 28000.0)] {
        let r = s.regen(&cv(&[("size", size)]));
        report(&format!("C1 {size}"), &r);
        assert!((volume_mm3(&r) - volume).abs() < 0.01, "C1 {size}: {} vs live {volume}", volume_mm3(&r));
    }
    // C2: configuration variable #len as the width.
    let mut s = Studio::new();
    list(&mut s.ps, "size", "Size", &["Small", "Medium", "Large"]);
    s.ps.configuration.inputs.push(ConfigInput {
        id: "len".into(),
        name: "len".into(),
        kind: ConfigInputKind::Variable { kind: VariableKind::Length, default: ExprValue::new("30 mm"), min: None, max: None },
        visible_when: None,
        excluded: false,
    });
    s.block("#len", "20 mm", "10 mm");
    // Live: a configuration variable is its own input (no value per List
    // row), and the Variable table doesn't list it.
    for (len, volume) in [("30 mm", 6000.0), ("40 mm", 8000.0), ("55 mm", 11000.0)] {
        let r = s.regen(&cv(&[("len", len)]));
        report(&format!("C2 {len}"), &r);
        assert!((volume_mm3(&r) - volume).abs() < 1e-6, "C2 {len}");
        assert!(!r.display.variables.contains_key("len"));
    }
    // C3: a checkbox configuring Symmetric.
    let mut s = Studio::new();
    let (_, x) = s.block("40 mm", "20 mm", "10 mm");
    s.ps.configuration.inputs.push(ConfigInput { id: "mirror".into(), name: "Mirror".into(), kind: ConfigInputKind::Checkbox { default: false }, visible_when: None, excluded: false });
    s.ps.configuration.configured.push(ConfiguredParam {
        input: "mirror".into(),
        feature: x,
        param: "symmetric".into(),
        values: [("false".to_string(), ParamValue::Bool(false)), ("true".to_string(), ParamValue::Bool(true))].into_iter().collect(),
    });
    for (checked, z) in [("false", (0.0, 10.0)), ("true", (-5.0, 5.0))] {
        let r = s.regen(&cv(&[("mirror", checked)]));
        let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
        assert!((bb.min.z * 1e3 - z.0).abs() < 1e-6 && (bb.max.z * 1e3 - z.1).abs() < 1e-6, "C3 {checked}: {bb:?}");
        assert!((volume_mm3(&r) - 8000.0).abs() < 1e-6);
    }
}

/// Source studios for Derived by tab name, regenerated in the asked
/// configuration.
#[derive(Debug)]
struct Sources(Vec<(&'static str, PartStudio)>);

impl noshape_model::regen::derived::DerivedProvider for Sources {
    fn context(&self, tab: &str) -> Option<std::sync::Arc<noshape_model::regen::context::Context>> {
        self.context_configured(tab, &ConfigValues::default())
    }
    fn context_configured(&self, tab: &str, values: &ConfigValues) -> Option<std::sync::Arc<noshape_model::regen::context::Context>> {
        let (_, ps) = self.0.iter().find(|(t, _)| *t == tab)?;
        let cv = ps.configuration.effective(values);
        Some(Regenerator::new().regenerate(ps, &cv, &UnitPrefs::default(), &RegenOptions::default()).context)
    }
    fn version(&self, _tab: &str) -> u64 {
        0
    }
    fn configuration(&self, tab: &str) -> Option<noshape_model::configuration::Configuration> {
        self.0.iter().find(|(t, _)| *t == tab).map(|(_, ps)| ps.configuration.clone())
    }
}

/// D1–D3: Derived of the C1 studio (Large; Medium at a location) and of
/// only V1's sketch.
#[test]
fn p2_oracle_derived() {
    // The C1 source.
    let mut c1 = Studio::new();
    let (_, x) = c1.block("40 mm", "20 mm", "10 mm");
    let r = c1.regen(&ConfigValues::default());
    let b = &r.context.bodies[0];
    let vertical: Vec<Reference> = b.shape.edges().iter().enumerate().filter(|(_, ed)| { let i = ed.edge_info().unwrap(); (i.start.z - i.end.z).abs() > 1e-6 }).map(|(i, _)| Reference::Edge((*b.edges[i]).clone())).collect();
    let fil = c1.add("fillet", &[("entities", ParamValue::Query(vertical)), ("radius", e("2 mm"))]);
    list(&mut c1.ps, "size", "Size", &["Small", "Medium", "Large"]);
    let depth = |v: &str| (v.to_lowercase(), e(match v { "Small" => "10 mm", "Medium" => "20 mm", _ => "35 mm" }));
    c1.ps.configuration.configured.push(ConfiguredParam { input: "size".into(), feature: x, param: "depth".into(), values: ["Small", "Medium", "Large"].iter().map(|v| depth(v)).collect() });
    c1.ps.configuration.configured.push(ConfiguredParam { input: "size".into(), feature: fil, param: SUPPRESSED_PARAM.into(), values: [("small".to_string(), ParamValue::Bool(false)), ("medium".to_string(), ParamValue::Bool(false)), ("large".to_string(), ParamValue::Bool(true))].into_iter().collect() });
    // The V1 source.
    let mut v1 = Studio::new();
    v1.var("a", "LENGTH", "10 mm");
    v1.var("b", "LENGTH", "#a * 2 + 1 in");
    v1.var("c", "LENGTH", "sqrt(#b^2 + #a^2)");
    v1.var("n", "NUMBER", "3");
    let (v1_sketch, _) = v1.block("#c", "#a * #n", "5 mm");
    let provider = std::sync::Arc::new(Sources(vec![("c1", c1.ps.clone()), ("v1", v1.ps.clone())]));
    let opts = RegenOptions { derived: Some(provider), ..Default::default() };
    let regen = |s: &Studio| Regenerator::new().regenerate(&s.ps, &ConfigValues::default(), &s.units, &opts);
    let derived = |s: &mut Studio, tab: &str, cfg: Option<&str>| {
        let d = s.add("importDerived", &[("partStudio", ParamValue::Text(tab.into()))]);
        if let Some(c) = cfg {
            s.ps.feature_mut(d).unwrap().params.set("configuration:size", ParamValue::Text(c.into()));
        }
        d
    };
    // D1: Large, then Fillet 1 mm on the 4 top edges.
    let mut d1 = Studio::new();
    derived(&mut d1, "c1", Some("large"));
    let r = regen(&d1);
    let b = &r.context.bodies[0];
    let top: Vec<Reference> = b.shape.edges().iter().enumerate().filter(|(_, ed)| { let i = ed.edge_info().unwrap(); i.start.z > 0.0349 && i.end.z > 0.0349 }).map(|(i, _)| Reference::Edge((*b.edges[i]).clone())).collect();
    assert_eq!(top.len(), 4);
    d1.add("fillet", &[("entities", ParamValue::Query(top)), ("radius", e("1 mm"))]);
    let r = regen(&d1);
    report("D1", &r);
    // Live 27974.71; ours removes 0.08 mm³ more where two 1 mm fillets meet
    // a sharp vertical edge at the corners (FILLET-CORNER-3, kern).
    assert!((volume_mm3(&r) - 27974.71).abs() < 0.1, "D1: {}", volume_mm3(&r));
    assert_eq!(r.context.bodies[0].name, "Part 1");
    // D2: Medium at (100, 50, 0).
    let mut d2 = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p = sk.add_point(DVec2::new(0.1, 0.05));
    let s2 = d2.add("sketch", &[("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
    let d = derived(&mut d2, "c1", Some("medium"));
    d2.ps.feature_mut(d).unwrap().params.set("location", ParamValue::Query(vec![Reference::SketchEntity { sketch: s2, entity: p.0 }]));
    let r = regen(&d2);
    report("D2", &r);
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    let got = [bb.min.x, bb.min.y, bb.min.z, bb.max.x, bb.max.y, bb.max.z].map(|v| (v * 1e6).round() / 1e3);
    assert_eq!(got, [100.0, 50.0, 0.0, 140.0, 70.0, 20.0], "D2");
    assert!((volume_mm3(&r) - 15931.33).abs() < 0.01);
    // D3: only V1's Sketch 1, then Extrude 5 mm of its region.
    let mut d3 = Studio::new();
    let d = derived(&mut d3, "v1", None);
    d3.ps.feature_mut(d).unwrap().params.set("parts", ParamValue::Query(vec![Reference::Body(noshape_model::BodyId { feature: v1_sketch, index: u32::MAX })]));
    let r = regen(&d3);
    assert!(r.context.bodies.is_empty(), "D3: only the sketch");
    let key = r.display.sketches.iter().find(|x| x.feature == v1_sketch).and_then(|x| x.regions.first()).map(|g| g.key.clone()).expect("the derived sketch's region");
    d3.add("extrude", &[("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: v1_sketch, key }])), ("depth", e("5 mm")), ("operationType", ParamValue::Enum("NEW".into()))]);
    let r = regen(&d3);
    report("D3", &r);
    assert!((volume_mm3(&r) - 6973.242).abs() < 0.01, "D3: {}", volume_mm3(&r));
}
