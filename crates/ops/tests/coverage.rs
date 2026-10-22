//! Every UI command has an agent operation (or is listed as planned or
//! UI-only), checked against the app's own command registries.

use noshape_ops::coverage::{Cover, SHORTCUTS};
use noshape_ops::{registry, values};
use std::collections::BTreeSet;

const SHORTCUTS_RS: &str = include_str!("../../app/src/shortcuts.rs");
const SHORTCUT_CMDS_RS: &str = include_str!("../../app/src/part_studio/shortcut_cmds.rs");

/// `<prefix>:<name>` ids of the shortcut registry (`ENTRIES`).
fn shortcut_ids() -> Vec<String> {
    let start = SHORTCUTS_RS.find("pub const ENTRIES").expect("ENTRIES");
    let body = &SHORTCUTS_RS[start..start + SHORTCUTS_RS[start..].find("\n};").expect("end of ENTRIES")];
    let mut ids = Vec::new();
    let mut prefix = "";
    for line in body.lines() {
        let t = line.trim();
        if t.ends_with('[') && t.contains('"') {
            prefix = t.split('"').nth(1).expect("prefix");
        } else if let Some(rest) = t.strip_prefix("(\"") {
            ids.push(format!("{prefix}:{}", &rest[..rest.find('"').expect("name")]));
        }
    }
    ids
}

/// Part Studio shortcut names that open a feature: (name, feature type).
fn feature_shortcuts() -> Vec<(String, String)> {
    let start = SHORTCUT_CMDS_RS.find("const FEATURES").expect("FEATURES");
    let body = &SHORTCUT_CMDS_RS[start..start + SHORTCUT_CMDS_RS[start..].find("];").expect("end")];
    body.lines()
        .filter_map(|l| {
            let p: Vec<&str> = l.split('"').collect();
            (p.len() >= 4).then(|| (p[1].to_string(), p[3].to_string()))
        })
        .collect()
}

fn tool_exists(name: &str) -> bool {
    registry::by_tool_name(name).is_some()
}

#[test]
fn every_ui_command_is_listed() {
    let ids = shortcut_ids();
    assert!(ids.len() > 200, "parsed {} shortcut ids; did shortcuts.rs change shape?", ids.len());
    let features = feature_shortcuts();
    assert!(features.len() > 60);
    assert!(tool_exists("feature_add") && tool_exists("sketch_create"));
    let listed: BTreeSet<&str> = SHORTCUTS.iter().map(|(id, _)| *id).collect();
    let mut missing = Vec::new();
    for id in &ids {
        let name = id.split_once(':').map(|x| x.1).unwrap_or("").trim_end_matches('*');
        if id.starts_with("partStudio:")
            && let Some((_, ty)) = features.iter().find(|(n, _)| n == name)
        {
            assert!(noshape_model::features::get(ty).is_some(), "{id} opens unknown feature type {ty}");
            continue;
        }
        if !listed.contains(id.as_str()) {
            missing.push(id.clone());
        }
    }
    assert!(missing.is_empty(), "UI commands with no entry in crates/ops/src/coverage.rs: {missing:?}");
    let stale: Vec<&&str> = listed.iter().filter(|l| !ids.iter().any(|i| i == **l)).collect();
    assert!(stale.is_empty(), "coverage.rs lists commands the UI no longer has: {stale:?}");
    for (id, c) in SHORTCUTS {
        if let Cover::Op(t) = c {
            assert!(tool_exists(t), "{id}: no operation `{t}`");
        }
    }
}

/// Coverage only goes up: lower `PLANNED_CEILING` as operations land.
#[test]
fn coverage_report() {
    const PLANNED_CEILING: usize = 37;
    let features = feature_shortcuts().len();
    let ops = SHORTCUTS.iter().filter(|c| matches!(c.1, Cover::Op(_))).count();
    let planned = SHORTCUTS.iter().filter(|c| matches!(c.1, Cover::Planned)).count();
    let ui = SHORTCUTS.iter().filter(|c| matches!(c.1, Cover::Ui(_))).count();
    let total = features + SHORTCUTS.len();
    let types = noshape_model::features::all().count();
    eprintln!(
        "agent API coverage: {}/{} UI commands ({} feature shortcuts, {ops} other ops), {ui} UI-only, {planned} planned; {types} feature types via feature_add; {} operations",
        features + ops + ui,
        total,
        features,
        registry::all().len()
    );
    assert!(planned <= PLANNED_CEILING, "more planned commands ({planned}) than before ({PLANNED_CEILING})");
}

/// Every built-in feature type's dialog defaults convert to the API's JSON
/// and back unchanged, and its schema generates.
#[test]
fn every_feature_type_round_trips_through_json() {
    let units = noshape_units::UnitPrefs::default();
    let doc = noshape_model::Document::new();
    let ps = noshape_model::PartStudio::new();
    let mut n = 0;
    for ty in noshape_model::features::all() {
        let schema = values::feature_schema(&ty.params, &units);
        assert!(schema["properties"].is_object(), "{}", ty.id);
        let f = ps.new_feature(ty.id, &units).expect("new feature");
        let json = values::values_to_json(Some(&ty.params), &f.params);
        let mut back = f.params.clone();
        let obj = json.as_object().unwrap().clone();
        values::apply(&ty.params, &mut back, &obj, &units, &doc).unwrap_or_else(|e| panic!("{}: {}", ty.id, e.to_json()));
        for (k, v) in &f.params.0 {
            if matches!(v, noshape_model::ParamValue::Sketch(_)) {
                continue;
            }
            // Expressions keep their text; cached values are recomputed.
            let same = match (v, back.get(k)) {
                (noshape_model::ParamValue::Expr(a), Some(noshape_model::ParamValue::Expr(b))) => a.text == b.text,
                (a, b) => Some(a) == b,
            };
            assert!(same, "{}.{k}: {v:?} -> {:?}", ty.id, back.get(k));
        }
        n += 1;
    }
    assert!(n >= 86, "{n} feature types");
}

/// docs/AGENT_API.md's operation reference is the registry's (regenerate
/// with `UPDATE_DOCS=1 cargo test -p noshape-ops --test coverage`).
#[test]
fn agent_api_reference_is_current() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/AGENT_API.md");
    let doc = std::fs::read_to_string(path).expect("docs/AGENT_API.md");
    let new = noshape_ops::reference::splice(&doc).expect("the generated-reference markers in docs/AGENT_API.md");
    if std::env::var_os("UPDATE_DOCS").is_some() {
        std::fs::write(path, &new).expect("write docs/AGENT_API.md");
        return;
    }
    assert!(new == doc, "docs/AGENT_API.md's operation reference is out of date: run `UPDATE_DOCS=1 cargo test -p noshape-ops --test coverage`");
}
