//! On Windows the app is a GUI program: a console program it starts (tasklist, net, powershell, an agent's .cmd)
//! flashes a console window unless started with CREATE_NO_WINDOW (P0 11-01: opening a document from the Documents
//! page flashed one per open, from `tasklist`). Every source that starts a process sets it, or is listed here with
//! why it doesn't need to.

/// Sources that start processes without CREATE_NO_WINDOW, and why that's right.
const ALLOWED: &[(&str, &str)] = &[
    ("app/src/cli_mode.rs", "macOS only: `open` for a link outside the .app"),
    ("launcher/src/main.rs", "noshape.com, a console program: it starts noshape.exe, a GUI program, in its terminal"),
];

fn sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).expect("read dir").flatten() {
        let p = e.path();
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn every_process_the_app_starts_on_windows_has_no_console_window() {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for c in std::fs::read_dir(&crates).expect("crates").flatten() {
        let src = c.path().join("src");
        if src.is_dir() {
            sources(&src, &mut files);
        }
    }
    let mut bad = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).expect("read");
        let spawns = text.contains("process::Command") || text.contains("use std::process::{") && text.contains("Command::new(");
        let rel = f.strip_prefix(&crates).unwrap_or(f).to_string_lossy().replace('\\', "/");
        if spawns && !text.contains("creation_flags") && !ALLOWED.iter().any(|(a, _)| *a == rel) {
            bad.push(rel);
        }
    }
    assert!(bad.is_empty(), "these sources start processes without CREATE_NO_WINDOW on Windows (a console window flashes from the app): {bad:?}");
    assert!(files.len() > 100, "found {} sources", files.len());
}
