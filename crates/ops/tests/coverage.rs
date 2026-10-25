//! Every UI command has an agent operation (or is listed as planned or
//! UI-only), checked against the app's own command registries.

use noshape_ops::coverage::{Cover, MENUS, SHORTCUTS};
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

/// The menu item labels of the app's sources, by area (see
/// `coverage::MENUS`): `item("…")`, `icon_item("…")`, `OsMenuItem::new("…")`.
fn menu_labels() -> BTreeSet<String> {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/src");
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    for e in std::fs::read_dir(&src).expect("app sources").flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let area = if name.starts_with("assembly_") {
            "assembly"
        } else if name.starts_with("drawing_") {
            "drawing"
        } else if name.starts_with("variable_") || name == "history_panel.rs" {
            "document"
        } else if name == "feature_dialog.rs" {
            "partStudio"
        } else if name == "camera_menu.rs" {
            "view"
        } else {
            continue;
        };
        files.push((area.into(), e.path()));
    }
    for e in std::fs::read_dir(src.join("part_studio")).expect("part studio sources").flatten() {
        files.push(("partStudio".into(), e.path()));
    }
    let mut out = BTreeSet::new();
    for (area, path) in files {
        let text = std::fs::read_to_string(&path).expect("read source");
        for pat in ["item(\"", "OsMenuItem::new(\""] {
            let mut from = 0;
            while let Some(k) = text[from..].find(pat) {
                let at = from + k;
                from = at + pat.len();
                // `item(` alone or as `icon_item(`, not inside another name.
                if pat.starts_with("item") {
                    let before = text[..at].chars().next_back().unwrap_or(' ');
                    if (before.is_alphanumeric() || before == '_') && !text[..at].ends_with("icon_") {
                        continue;
                    }
                }
                let rest = &text[from..];
                let mut end = 0;
                let b = rest.as_bytes();
                while end < b.len() && !(b[end] == b'"' && (end == 0 || b[end - 1] != b'\\')) {
                    end += 1;
                }
                out.insert(format!("{area}:{}", &rest[..end]));
            }
        }
    }
    out
}

/// Every operation of the required groups has examples; each is a JSON
/// object naming only the operation's fields, with every required one.
#[test]
fn every_operation_has_examples() {
    use noshape_ops::examples;
    for (tool, list) in examples::EXAMPLES {
        let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("examples for unknown operation {tool}"));
        for e in *list {
            let v: serde_json::Value = serde_json::from_str(e).unwrap_or_else(|err| panic!("{tool}: example {e} isn't JSON: {err}"));
            let o = v.as_object().unwrap_or_else(|| panic!("{tool}: example {e} isn't an object"));
            for k in o.keys() {
                assert!(op.fields.iter().any(|f| f.name == k), "{tool}: example {e} names `{k}`, not a field");
            }
            for f in op.fields.iter().filter(|f| f.required) {
                assert!(o.contains_key(f.name), "{tool}: example {e} lacks required `{}`", f.name);
            }
        }
    }
    let missing: Vec<String> = registry::all().iter().filter(|o| examples::REQUIRED_GROUPS.contains(&o.group) && examples::of(&o.tool_name()).is_empty()).map(|o| o.tool_name()).collect();
    assert!(missing.is_empty(), "operations without examples (crates/ops/src/examples.rs): {missing:?}");
}

#[test]
fn every_menu_item_is_listed() {
    let labels = menu_labels();
    assert!(labels.len() > 300, "parsed {} menu labels; did the menus change shape?", labels.len());
    let listed: BTreeSet<&str> = MENUS.iter().map(|(id, _)| *id).collect();
    let missing: Vec<&String> = labels.iter().filter(|l| !listed.contains(l.as_str())).collect();
    assert!(missing.is_empty(), "menu items with no entry in crates/ops/src/coverage.rs MENUS: {missing:?}");
    let stale: Vec<&&str> = listed.iter().filter(|l| !labels.contains(**l)).collect();
    assert!(stale.is_empty(), "MENUS lists items the UI no longer has: {stale:?}");
    for (id, c) in MENUS {
        if let Cover::Op(t) = c {
            assert!(tool_exists(t), "{id}: no operation `{t}`");
        }
    }
}

/// Planned menu items, per area: these only go down (lower them as ops land).
#[test]
fn menu_coverage_report() {
    const CEILINGS: [(&str, usize); 4] = [("partStudio", 2), ("assembly", 1), ("drawing", 0), ("document", 23)];
    for (area, ceiling) in CEILINGS {
        let all: Vec<&(&str, Cover)> = MENUS.iter().filter(|(id, _)| id.starts_with(&format!("{area}:"))).collect();
        let planned: Vec<&str> = all.iter().filter(|c| matches!(c.1, Cover::Planned)).map(|c| c.0).collect();
        eprintln!("{area} menus: {} items, {} planned", all.len(), planned.len());
        assert!(planned.len() <= ceiling, "{area}: {} planned menu items ({planned:?}), ceiling {ceiling}", planned.len());
    }
}

/// Every toolbar / shortcut command has an operation or a UI-only reason:
/// none is left Planned.
#[test]
fn coverage_report() {
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
    let left: Vec<&str> = SHORTCUTS.iter().filter(|c| matches!(c.1, Cover::Planned)).map(|c| c.0).collect();
    assert!(left.is_empty(), "shortcut commands without an operation: {left:?}");
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
        let f = ps.new_feature(ty.id, &units).expect("new feature");
        let schema = values::feature_schema(&ty.params, &units, Some(&f.params));
        assert!(schema["properties"].is_object(), "{}", ty.id);
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

/// Conditional parameters say when the dialog shows them (agent usability:
/// an agent can't see the dialog).
#[test]
fn schema_says_when_parameters_show() {
    let units = noshape_units::UnitPrefs::default();
    let ps = noshape_model::PartStudio::new();
    let desc = |ty: &str, p: &str| {
        let t = noshape_model::features::get(ty).unwrap();
        let f = ps.new_feature(ty, &units).unwrap();
        values::feature_schema(&t.params, &units, Some(&f.params))["properties"][p]["description"].as_str().unwrap().to_string()
    };
    let depth = desc("hole", "holeDepthV3");
    assert!(depth.contains("shown with the defaults; hidden, and ignored, when endStyleV2 is UP_TO_NEXT or UP_TO_ENTITY or THROUGH"), "{depth}");
    let width = desc("chamfer", "width");
    assert!(width.contains("shown with the defaults"), "{width}");
    let other = desc("moveFace", "surfacesAndFaces");
    assert!(other.contains("hidden with the defaults") && other.contains("outputType is CREATE"), "{other}");
    let faces = desc("moveFace", "moveFaces");
    assert!(faces.contains("shown with the defaults") && faces.contains("outputType is CREATE"), "{faces}");
}

/// Everything a user can do in the window an agent can do too (navigation
/// and view actions included): the UI-only entries only go down (98 before
/// the sweep), and none may be added.
#[test]
fn ui_only_entries_only_go_down() {
    const CEILING: usize = 63;
    let ui: Vec<&str> = SHORTCUTS.iter().chain(MENUS.iter()).filter(|(_, c)| matches!(c, Cover::Ui(_))).map(|(id, _)| *id).collect();
    assert!(ui.len() <= CEILING, "{} UI-only entries (at most {CEILING}): give the new ones an operation\n{}", ui.len(), ui.join("\n"));
}
