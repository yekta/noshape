//! Embeds the FeatureScript standard library (`std/*.fs`).

use std::io::Write;

fn main() {
    // Read at run time: a build script compiled in one worktree can be reused
    // by another sharing the target dir (`env!` would point at the first).
    let dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("std");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("std directory")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".fs"))
        .collect();
    names.sort();
    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("std_sources.rs");
    let mut f = std::fs::File::create(out).expect("create std_sources.rs");
    writeln!(f, "/// (file name, source) of every standard library module, sorted by name.").unwrap();
    writeln!(f, "pub static STD_SOURCES: &[(&str, &str)] = &[").unwrap();
    for n in names {
        writeln!(f, "    ({n:?}, include_str!({:?})),", dir.join(&n).display().to_string()).unwrap();
    }
    writeln!(f, "];").unwrap();
    // Older standard libraries that Feature Studios pin (`std-versions/<version>/`).
    let vdir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("std-versions");
    println!("cargo:rerun-if-changed={}", vdir.display());
    let mut versions: Vec<(u32, std::path::PathBuf)> = std::fs::read_dir(&vdir)
        .map(|d| d.filter_map(|e| e.ok()).filter_map(|e| Some((e.file_name().to_string_lossy().parse().ok()?, e.path()))).collect())
        .unwrap_or_default();
    versions.sort();
    writeln!(f, "/// (version, its modules sorted by name) of the older standard libraries embedded.").unwrap();
    writeln!(f, "pub static STD_VERSIONS: &[(u32, &[(&str, &str)])] = &[").unwrap();
    for (v, d) in versions {
        println!("cargo:rerun-if-changed={}", d.display());
        let mut names: Vec<String> = std::fs::read_dir(&d).expect("std version dir").filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".fs")).collect();
        names.sort();
        writeln!(f, "    ({v}, &[").unwrap();
        for n in names {
            writeln!(f, "        ({n:?}, include_str!({:?})),", d.join(&n).display().to_string()).unwrap();
        }
        writeln!(f, "    ]),").unwrap();
    }
    writeln!(f, "];").unwrap();
}
