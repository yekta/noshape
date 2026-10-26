//! Configuration descriptions with display names (`interaction`; moved from noshape-model-data).

use crate::configuration::*;
use crate::params::{ParamValue, ParamValues};
use crate::partstudio::Feature;
use crate::reference::FeatureId;
use noshape_units::ExprValue;

#[test]
fn describes_non_default_values() {
    let cfg = Configuration {
        inputs: vec![
            ConfigInput { id: "s".into(), name: "Size".into(), kind: ConfigInputKind::List { options: vec![ListOption { id: "a".into(), name: "Small".into() }, ListOption { id: "b".into(), name: "Large".into() }], default: "a".into() }, visible_when: None, excluded: false },
            ConfigInput { id: "c".into(), name: "Fillet".into(), kind: ConfigInputKind::Checkbox { default: false }, visible_when: None, excluded: false },
            ConfigInput { id: "t".into(), name: "Label".into(), kind: ConfigInputKind::Text { default: "x".into() }, visible_when: None, excluded: false },
        ],
        configured: vec![],
    };
    assert_eq!(cfg.describe(&cfg.defaults()), "");
    let mut v = cfg.defaults();
    v.0.insert("s".into(), "b".into());
    v.0.insert("c".into(), "true".into());
    v.0.insert("t".into(), "y".into());
    assert_eq!(cfg.describe(&v), "Large, Fillet, Label = y");
}

#[test]
fn dialogs_edit_the_active_configuration() {
    let mut cfg = Configuration {
        inputs: vec![ConfigInput { id: "s".into(), name: "Size".into(), kind: ConfigInputKind::List { options: vec![ListOption { id: "a".into(), name: "Small".into() }, ListOption { id: "b".into(), name: "Large".into() }], default: "a".into() }, visible_when: None, excluded: false }],
        configured: vec![],
    };
    let e = |t: &str| ParamValue::Expr(ExprValue::new(t));
    let mut base = Feature { id: FeatureId(7), type_id: "extrude".into(), name: "Extrude 1".into(), suppressed: false, params: ParamValues::default(), folder: None, custom_name: false, hidden: false, suppression_expr: None };
    base.params.set("depth", e("12 mm"));
    cfg.configured.push(ConfiguredParam { input: "s".into(), feature: FeatureId(7), param: "depth".into(), values: [("a".to_string(), e("12 mm")), ("b".to_string(), e("30 mm"))].into_iter().collect() });
    let mut large = cfg.defaults();
    large.0.insert("s".into(), "b".into());
    let mut shown = cfg.for_editing(&base, &large);
    assert_eq!(shown.params.get("depth"), Some(&e("30 mm")));
    shown.params.set("depth", e("35 mm"));
    cfg.store_edit(&base, &mut shown, &large);
    assert_eq!(cfg.configured[0].values.get("b"), Some(&e("35 mm")));
    assert_eq!(cfg.configured[0].values.get("a"), Some(&e("12 mm")));
    assert_eq!(shown.params.get("depth"), Some(&e("12 mm")));
}

#[test]
fn a_configured_variable_names_itself_with_the_active_value() {
    // qa4 round 4 S4-a: the list row of a Variable configured S/L reads
    // "#w = 80 mm" under L (the row shows `for_editing`'s feature).
    let cfg = Configuration {
        inputs: vec![ConfigInput { id: "s".into(), name: "Size".into(), kind: ConfigInputKind::List { options: vec![ListOption { id: "a".into(), name: "S".into() }, ListOption { id: "b".into(), name: "L".into() }], default: "a".into() }, visible_when: None, excluded: false }],
        configured: vec![ConfiguredParam { input: "s".into(), feature: FeatureId(3), param: "lengthValue".into(), values: [("a".to_string(), ParamValue::Expr(ExprValue::new("40 mm"))), ("b".to_string(), ParamValue::Expr(ExprValue::new("80 mm")))].into_iter().collect() }],
    };
    let mut var = Feature { id: FeatureId(3), type_id: "assignVariable".into(), name: "Variable 1".into(), suppressed: false, params: ParamValues::default(), folder: None, custom_name: false, hidden: false, suppression_expr: None };
    var.params.set("name", ParamValue::Text("w".into()));
    var.params.set("lengthValue", ParamValue::Expr(ExprValue::new("40 mm")));
    let mut large = cfg.defaults();
    large.0.insert("s".into(), "b".into());
    assert_eq!(crate::interaction::display_name(&cfg.for_editing(&var, &large)), "#w = 80 mm");
    assert_eq!(crate::interaction::display_name(&cfg.for_editing(&var, &cfg.defaults())), "#w = 40 mm");
}
