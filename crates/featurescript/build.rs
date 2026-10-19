//! Embeds the FeatureScript standard library (`std/*.fs`).

use std::io::Write;

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("std");
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
}
