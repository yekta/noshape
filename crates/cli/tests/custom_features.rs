//! Custom (FeatureScript) features through the CLI, each call a fresh
//! process as an agent's shell calls are: editing a custom feature works
//! without adding one first, and a failing one says why (its FeatureScript
//! notices, as the notices panel's Monitor shows them).

use serde_json::{Value, json};
use std::process::Command;

/// One `noshape-cli call <tool> <args>` in its own process: (success, reply).
fn cli(tool: &str, args: Value) -> (bool, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_noshape-cli")).args(["call", tool, &args.to_string()]).output().expect("run noshape-cli");
    let text = String::from_utf8_lossy(&out.stdout);
    let v = serde_json::from_str(&text).unwrap_or_else(|_| panic!("{tool}: not JSON: {text} {}", String::from_utf8_lossy(&out.stderr)));
    (out.status.success(), v)
}

fn ok(tool: &str, args: Value) -> Value {
    let (good, v) = cli(tool, args);
    assert!(good, "{tool} failed: {v}");
    v
}

const SOURCE: &str = r#"FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

function checkWidth(width is ValueWithUnits)
{
    if (width > 100 * millimeter)
        throw regenError("Width over 100 mm", ["width"]);
}

annotation { "Feature Type Name" : "Knurl block" }
export const knurlBlock = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Width" }
        isLength(definition.width, { (millimeter) : [1, 30, 1000] } as LengthBoundSpec);
    }
    {
        println("knurl width " ~ toString(definition.width / millimeter));
        checkWidth(definition.width);
        fCuboid(context, id + "box", { "corner1" : vector(0, 0, 0) * millimeter, "corner2" : vector(definition.width, 20 * millimeter, 10 * millimeter) });
    });
"#;

#[test]
fn custom_feature_edit_and_notices_in_fresh_processes() {
    let dir = std::env::temp_dir().join(format!("noshape-cli-custom-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("knurl.noshape");
    let _ = std::fs::remove_file(&path);
    let doc = path.to_str().unwrap();
    ok("document_create", json!({ "path": doc }));
    ok("tab_create", json!({ "doc": doc, "kind": "featurestudio" }));
    ok("featurestudio_edit", json!({ "doc": doc, "source": SOURCE }));
    let c = ok("featurestudio_commit", json!({ "doc": doc }));
    let ty = c["feature_types"].as_array().unwrap().iter().find(|t| t["name"] == "Knurl block").unwrap_or_else(|| panic!("{c}"))["type"].as_str().unwrap().to_string();
    let f = ok("feature_add", json!({ "doc": doc, "tab": "Part Studio 1", "type": ty, "params": { "width": "40 mm" } }));
    assert_eq!(f["feature_status"]["status"], "Ok", "{f}");
    let feature = f["feature"].as_str().unwrap().to_string();

    // Editing it in a new process (no feature_add there first).
    let e = ok("feature_edit", json!({ "doc": doc, "tab": "Part Studio 1", "feature": feature, "params": { "width": "60 mm" } }));
    assert_eq!(e["feature_status"]["status"], "Ok", "{e}");
    let parts = ok("query_parts", json!({ "doc": doc, "tab": "Part Studio 1" }));
    let v = parts["parts"][0]["volume"].as_f64().unwrap();
    assert!((v - 12000.0).abs() < 1e-3, "{parts}");

    // Too wide: the feature fails, and says why.
    let e = ok("feature_edit", json!({ "doc": doc, "tab": "Part Studio 1", "feature": feature, "params": { "width": "150 mm" } }));
    assert_eq!(e["feature_status"]["status"], "Error", "{e}");
    let failed = &e["regeneration"]["failed"][0];
    let says = |n: &Value| n["notices"].as_array().is_some_and(|l| l.iter().any(|x| x["message"].as_str().is_some_and(|m| m.contains("Width over 100 mm"))));
    assert!(says(failed), "feature_edit's failure has the notices: {e}");

    // In a fresh process: feature_get, feature_list and the Monitor.
    let g = ok("feature_get", json!({ "doc": doc, "tab": "Part Studio 1", "feature": feature }));
    assert!(says(&g), "{g}");
    assert!(g["output"].as_str().unwrap().contains("knurl width 150"), "{g}");
    assert!(g["visible"].as_array().unwrap().iter().any(|p| p == "width"), "the custom type's parameters are known: {g}");
    let n = g["notices"].as_array().unwrap().iter().find(|x| x["message"].as_str().unwrap().contains("Width over 100 mm")).unwrap().clone();
    let stack = n["stack"].as_array().unwrap();
    assert!(stack.iter().any(|r| r["in"].as_str().unwrap().contains("checkWidth")), "the stack names the function: {n}");
    assert!(stack.iter().any(|r| r["at"].as_str().is_some_and(|a| a.contains(':'))), "rows have line:col: {n}");
    assert_eq!(stack.last().unwrap()["in"], "Part Studio 1 (Knurl block 1)", "{n}");

    let l = ok("feature_list", json!({ "doc": doc, "tab": "Part Studio 1" }));
    assert!(says(&l["failed"][0]), "{l}");

    let m = ok("featurestudio_monitor", json!({ "doc": doc, "part_studio": "Part Studio 1" }));
    assert!(says(&m), "{m}");
    assert!(m["output"].as_str().unwrap().contains("knurl width 150"), "{m}");
    let _ = std::fs::remove_dir_all(&dir);
}
