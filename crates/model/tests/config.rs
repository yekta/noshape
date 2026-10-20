use noshape_model::configuration::{ConfigInput, ConfigInputKind, ConfigValues, ConfiguredParam, ListOption, VariableKind};
use noshape_model::params::ParamValue;
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_units::{ExprValue, UnitPrefs};

#[test]
fn configured_depth_changes_volume() {
    let mut ps = noshape_model::demo::plate_studio();
    let extrude = ps.features[1].id;
    ps.configuration.inputs.push(ConfigInput {
        id: "size".into(),
        name: "Size".into(),
        kind: ConfigInputKind::List { options: vec![ListOption { id: "thin".into(), name: "Thin".into() }, ListOption { id: "thick".into(), name: "Thick".into() }], default: "thin".into() },
        visible_when: None,
        excluded: false,
    });
    ps.configuration.configured.push(ConfiguredParam {
        input: "size".into(),
        feature: extrude,
        param: "depth".into(),
        values: [("thin".to_string(), ParamValue::Expr(ExprValue::new("12 mm"))), ("thick".to_string(), ParamValue::Expr(ExprValue::new("24 mm")))].into_iter().collect(),
    });
    let units = UnitPrefs::default();
    let mut reg = Regenerator::new();
    let thin = reg.regenerate(&ps, &ps.configuration.defaults(), &units, &RegenOptions::default());
    let mut cv = ConfigValues::default();
    cv.0.insert("size".into(), "thick".into());
    let thick = reg.regenerate(&ps, &cv, &units, &RegenOptions::default());
    let v1 = thin.context.bodies[0].shape.volume().unwrap();
    let v2 = thick.context.bodies[0].shape.volume().unwrap();
    assert!(v2 > v1 * 1.8, "thin {v1} thick {v2}");
}

#[test]
fn configuration_variable_drives_expression() {
    let mut ps = noshape_model::demo::plate_studio();
    ps.configuration.inputs.push(ConfigInput {
        id: "t".into(),
        name: "t".into(),
        kind: ConfigInputKind::Variable { kind: VariableKind::Length, default: ExprValue::new("5 mm"), min: None, max: None },
        visible_when: None,
        excluded: false,
    });
    ps.features[1].params.set("depth", ParamValue::Expr(ExprValue::new("#t * 2")));
    let units = UnitPrefs::default();
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ps.configuration.defaults(), &units, &RegenOptions::default());
    for f in &r.display.features {
        assert_ne!(f.status, noshape_model::display::FeatureStatus::Error, "{:?}", f.message);
    }
    let bb = r.context.bodies[0].shape.bounding_box(false).unwrap();
    assert!((bb.max.z - bb.min.z - 0.010).abs() < 1e-6, "height {}", bb.max.z - bb.min.z);
}

#[derive(Debug)]
struct OneStudio {
    studio: std::sync::Mutex<noshape_model::partstudio::PartStudio>,
}

impl noshape_model::regen::derived::DerivedProvider for OneStudio {
    fn context(&self, _tab: &str) -> Option<std::sync::Arc<noshape_model::regen::context::Context>> {
        let ps = self.studio.lock().unwrap().clone();
        let mut reg = Regenerator::new();
        Some(reg.regenerate(&ps, &ps.configuration.defaults(), &UnitPrefs::default(), &RegenOptions::default()).context)
    }
    fn version(&self, _tab: &str) -> u64 {
        let ps = self.studio.lock().unwrap();
        let s = serde_json::to_string(&*ps).unwrap();
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    }
}

#[test]
fn derived_parts_follow_source() {
    let units = UnitPrefs::default();
    let provider = std::sync::Arc::new(OneStudio { studio: std::sync::Mutex::new(noshape_model::demo::plate_studio()) });
    let mut target = noshape_model::partstudio::PartStudio::new();
    let mut d = target.new_feature("importDerived", &units).unwrap();
    d.params.set("partStudio", ParamValue::Text("source".into()));
    target.features.push(d);
    let mut reg = Regenerator::new();
    let opts = RegenOptions { derived: Some(provider.clone()), ..Default::default() };
    let r = reg.regenerate(&target, &ConfigValues::default(), &units, &opts);
    assert_eq!(r.display.features[0].status, noshape_model::display::FeatureStatus::Ok, "{:?}", r.display.features[0].message);
    let v1 = r.context.bodies[0].shape.volume().unwrap();
    // Edit the source: thicker plate; the derived part must update.
    provider.studio.lock().unwrap().features[1].params.set("depth", ParamValue::Expr(ExprValue::new("24 mm")));
    let r2 = reg.regenerate(&target, &ConfigValues::default(), &units, &opts);
    let v2 = r2.context.bodies[0].shape.volume().unwrap();
    assert!(v2 > v1 * 1.5, "derived part did not update: {v1} -> {v2}");
}
