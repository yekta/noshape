//! T2 probe: a document regenerates the same every time. NOSHAPE_PROBE_DOC (a .noshape path; never written: each run
//! works on a scratch copy), NOSHAPE_PROBE_TAB (a Part Studio's name), NOSHAPE_PROBE_RUNS (default 2). Each run opens
//! a fresh copy, moves the rollback bar past each feature in turn and records every part's kind, face / edge / vertex
//! counts and volume; the runs must agree, else the first feature where they part is reported.

use noshape_ops::registry;
use noshape_ops::Ctx;
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).map(|o| o.value).unwrap_or_else(|e| panic!("{tool}: {e:?}"))
}

/// Each feature's name and the parts after it, one line per part.
fn fingerprints(doc: &std::path::Path, tab: &str, run: usize) -> Vec<(String, Vec<String>)> {
    let dir = std::env::temp_dir().join(format!("noshape-determinism-{}-{run}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // (Only the owner reads the copy: the document may be private.)
    #[cfg(unix)]
    std::fs::set_permissions(&dir, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let copy = dir.join("doc.noshape");
    std::fs::copy(doc, &copy).unwrap();
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_open", json!({ "path": copy }));
    let list = call(&mut ctx, "feature_list", json!({ "tab": tab }));
    let names: Vec<String> = list["features"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap_or("").to_string()).collect();
    let mut out = Vec::new();
    for (i, name) in names.iter().enumerate() {
        call(&mut ctx, "feature_rollback", json!({ "tab": tab, "index": i + 1 }));
        let parts = call(&mut ctx, "query_parts", json!({ "tab": tab }));
        let mut lines: Vec<String> = parts.as_array().or_else(|| parts["parts"].as_array()).into_iter().flatten().map(|p| {
            format!("{} {} faces {} edges {} vertices {} volume {:.6}", p["name"].as_str().unwrap_or(""), p["kind"].as_str().unwrap_or(""), p["faces"], p["edges"], p["vertices"], p["volume"].as_f64().unwrap_or(f64::NAN))
        }).collect();
        lines.sort();
        out.push((name.clone(), lines));
    }
    let _ = std::fs::remove_dir_all(&dir);
    out
}

#[test]
#[ignore = "T2: probe"]
fn a_document_regenerates_the_same_every_time() {
    let Ok(doc) = std::env::var("NOSHAPE_PROBE_DOC") else { return };
    let tab = std::env::var("NOSHAPE_PROBE_TAB").expect("NOSHAPE_PROBE_TAB: the Part Studio's name");
    let runs: usize = std::env::var("NOSHAPE_PROBE_RUNS").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
    let doc = std::path::PathBuf::from(doc);
    let first = fingerprints(&doc, &tab, 0);
    println!("run 0: {} features", first.len());
    for run in 1..runs {
        let other = fingerprints(&doc, &tab, run);
        let parted = first.iter().zip(&other).position(|(a, b)| a != b);
        if let Some(i) = parted {
            panic!("run {run} parts from run 0 at feature {} \"{}\":\n  run 0: {:#?}\n  run {run}: {:#?}", i + 1, first[i].0, first[i].1, other[i].1);
        }
        println!("run {run}: the same at all {} features", other.len());
    }
}
