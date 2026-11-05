//! The third-party notices shipped with the app (crates/ops/assets/THIRD-PARTY-NOTICES.txt, Help › About) are
//! Cargo.lock's: `UPDATE_NOTICES=1 cargo test -p noshape-ops --test all notices::` regenerates them.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};

const HEADER: &str = "noshape third-party notices\n\
    Generated from Cargo.lock by `UPDATE_NOTICES=1 cargo test -p noshape-ops --test all notices::` (crates/ops/tests/notices.rs);\n\
    do not edit by hand.\n";

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

/// A package's licence and notice files (LICENSE*, LICENCE*, COPYING*, NOTICE*, COPYRIGHT*, UNLICENSE), sorted.
fn licence_files(dir: &Path, declared: Option<&str>) -> Vec<PathBuf> {
    let mut out: BTreeSet<PathBuf> = BTreeSet::new();
    if let Some(f) = declared {
        out.insert(dir.join(f));
    }
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_uppercase();
        let is = ["LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT", "UNLICENSE"].iter().any(|p| name.starts_with(p));
        if is && !name.ends_with(".RS") && !name.ends_with(".TOML") && e.path().is_file() {
            out.insert(e.path());
        }
    }
    out.into_iter().filter(|p| p.is_file()).collect()
}

/// The notices for the workspace as Cargo.lock has it now.
fn generate() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let out = std::process::Command::new(env!("CARGO")).args(["metadata", "--format-version", "1", "--locked"]).current_dir(&root).output().expect("cargo metadata");
    assert!(out.status.success(), "cargo metadata: {}", String::from_utf8_lossy(&out.stderr));
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let packages: HashMap<&str, &serde_json::Value> = meta["packages"].as_array().unwrap().iter().map(|p| (p["id"].as_str().unwrap(), p)).collect();
    let nodes: HashMap<&str, &serde_json::Value> = meta["resolve"]["nodes"].as_array().unwrap().iter().map(|n| (n["id"].as_str().unwrap(), n)).collect();
    // The app and what it links: normal dependencies on every platform (not build scripts' or tests').
    let app = packages.values().find(|p| p["name"] == "noshape" && p["manifest_path"].as_str().unwrap().contains("crates/app")).expect("the app package")["id"].as_str().unwrap();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut queue = VecDeque::from([app]);
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id) {
            continue;
        }
        for d in nodes[id]["deps"].as_array().unwrap() {
            if d["dep_kinds"].as_array().unwrap().iter().any(|k| k["kind"].is_null()) {
                queue.push_back(d["pkg"].as_str().unwrap());
            }
        }
    }
    let ours = root.join("crates");
    // Each text once, with the packages that carry it.
    let mut texts: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut rows: BTreeSet<String> = BTreeSet::new();
    // Packages with no licence file of their own: the standard text of the first licence of theirs we have.
    const STANDARD: [&str; 5] = ["MIT", "Apache-2.0", "BSD-3-Clause", "MPL-2.0", "CC0-1.0"];
    let mut standard_used: BTreeSet<&str> = BTreeSet::new();
    for id in seen {
        let p = packages[id];
        let manifest = PathBuf::from(p["manifest_path"].as_str().unwrap());
        let dir = manifest.parent().unwrap();
        // (Both canonical: Windows' carry the \\?\ prefix.)
        if dir.canonicalize().is_ok_and(|d| d.starts_with(&ours)) {
            continue;
        }
        let label = format!("{} {}", p["name"].as_str().unwrap(), p["version"].as_str().unwrap());
        let licence = p["license"].as_str().unwrap_or("see its licence file");
        let files = licence_files(dir, p["license_file"].as_str());
        if files.is_empty() {
            let ids: Vec<&str> = licence.split(|c: char| c.is_whitespace() || "/()".contains(c)).filter(|t| !t.is_empty()).collect();
            let chosen = STANDARD.iter().find(|s| ids.contains(s)).unwrap_or_else(|| panic!("{label} ({licence}) ships no licence file and none of its licences has a standard text in assets/licenses"));
            standard_used.insert(chosen);
            let authors: Vec<&str> = p["authors"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
            let by = if authors.is_empty() { format!("the {} authors", p["name"].as_str().unwrap()) } else { authors.join(", ") };
            rows.insert(format!("{label}: {licence}; used under {chosen} (its standard text below), Copyright (c) {by}"));
        } else {
            rows.insert(format!("{label}: {licence}"));
        }
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap_or_default().replace("\r\n", "\n").trim().to_string();
            if !text.is_empty() {
                texts.entry(text).or_default().insert(label.clone());
            }
        }
    }
    let read = |p: &str| std::fs::read_to_string(assets().join(p)).unwrap().replace("\r\n", "\n").trim().to_string();
    let rule = "=".repeat(78);
    let mut s = String::from(HEADER);
    s += &format!("\n{rule}\nOpen CASCADE Technology 8.0.1, Copyright (c) Open Cascade SAS\nLicensed under the GNU Lesser General Public License version 2.1 with the Open CASCADE exception\n(version 1.0); its source: https://dev.opencascade.org (noshape's patches: patches/occt in its source).\n{rule}\n\n");
    s += &read("licenses/OCCT_LGPL_EXCEPTION.txt");
    s += "\n\n";
    s += &read("licenses/OCCT-LICENSE_LGPL_21.txt");
    s += &format!("\n\n{rule}\nFonts: Roboto (Apache License 2.0; Roboto SemiBold under the SIL Open Font License 1.1) and Inconsolata\n(SIL Open Font License 1.1)\n{rule}\n\n");
    for f in ["fonts/LICENSE-Roboto.txt", "fonts/OFL-Roboto-SemiBold.txt", "fonts/OFL-Inconsolata.txt"] {
        s += &format!("--- {f} ---\n\n{}\n\n", read(&format!("../../app/assets/{f}")));
    }
    s += &format!("{rule}\nRust packages ({}): name version: licence\n{rule}\n\n", rows.len());
    for r in &rows {
        s += r;
        s += "\n";
    }
    s += &format!("\n{rule}\nStandard licence texts, for the packages above without a licence file of their own\n{rule}\n");
    for id in standard_used {
        s += &format!("\n--- {id} ---\n\n{}\n", read(&format!("licenses/{id}.txt")));
    }
    s += &format!("\n{rule}\nTheir licence and notice files, each text once\n{rule}\n");
    for (text, users) in &texts {
        s += &format!("\n--- {} ---\n\n{text}\n", users.iter().cloned().collect::<Vec<_>>().join(", "));
    }
    s
}

/// The shipped notices are Cargo.lock's (regenerate with UPDATE_NOTICES=1), name Open CASCADE's LGPL and the fonts.
#[test]
fn the_shipped_notices_are_current() {
    let now = generate();
    let path = assets().join("THIRD-PARTY-NOTICES.txt");
    if std::env::var_os("UPDATE_NOTICES").is_some() {
        std::fs::write(&path, &now).unwrap();
    }
    // (A Windows checkout may have turned its line ends into CRLF.)
    let shipped = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
    assert!(shipped == now, "crates/ops/assets/THIRD-PARTY-NOTICES.txt is out of date with Cargo.lock: run UPDATE_NOTICES=1 cargo test -p noshape-ops --test all notices::");
    for needle in ["Open CASCADE exception", "GNU LESSER GENERAL PUBLIC LICENSE", "Roboto", "SIL OPEN FONT LICENSE", "gpui"] {
        assert!(shipped.contains(needle), "the notices mention {needle}");
    }
}
