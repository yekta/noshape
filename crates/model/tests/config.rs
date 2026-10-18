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
