//! Ctrl+click on a standard library symbol in a Feature Studio. Live
//! (dom/fs-nav-shortcuts.txt): Onshape opens the "std" document at version
//! 3095.0 in a new browser tab ("Versions are view only. Viewing 3095.0."),
//! on the module's tab ("evaluate.fs"), with the caret on the exported name
//! (evArea: line 152, column 17). Here: a local "std" document with one
//! Feature Studio per module, versioned "3095.0", built once in the config
//! directory and opened at that version in a new window.

use crate::gpu::Gpu;
use gpui_kit::App;
use noshape_model::custom::FeatureStudio;
use noshape_model::document::{Document, TabContent};
use std::path::PathBuf;

/// Where a standard library symbol is exported: module, 1-based line and column.
pub fn std_definition(name: &str) -> Option<(&'static str, usize, usize)> {
    for (module, src) in noshape_model::custom::STD_SOURCES {
        for (i, line) in src.lines().enumerate() {
            let Some(rest) = line.strip_prefix("export ") else { continue };
            let Some(decl) = ["function ", "const ", "predicate ", "enum ", "type "].iter().find_map(|k| rest.strip_prefix(k)) else { continue };
            let declared: String = decl.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if declared == name {
                let column = line.len() - decl.len() + 1;
                return Some((module, i + 1, column));
            }
        }
    }
    None
}

/// The local "std" document (created on first use).
fn std_document() -> anyhow::Result<PathBuf> {
    let version = noshape_model::custom::STD_VERSION;
    let dir = crate::paths::config_dir().join(format!("std-{version}"));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("std.noshape");
    if path.exists() {
        return Ok(path);
    }
    let mut doc = Document::new();
    doc.tabs.clear();
    for (module, src) in noshape_model::custom::STD_SOURCES {
        doc.add_tab(module, TabContent::FeatureStudio(FeatureStudio::new(src)));
    }
    let mut session = crate::session::Session::create(&path, doc)?;
    session.create_version(&format!("{version}.0"), "")?;
    Ok(path)
}

/// Open the std document at its version on `module`, caret at `line`, `column`.
pub fn open(module: &str, line: usize, column: usize, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    let path = std_document()?;
    let head = crate::session::Session::open(&path)?.head()?;
    crate::feature_studio_hooks::open_studio_in_version(path, head, module.to_string(), line, column, gpu, cx)
}
