//! Where a solid export's file goes (Options: Download, Download and store file in a new tab, Store file in a new tab)
//! and the Notifications bell's result, shared by Part Studios and assemblies.

use crate::doc_model::DocModel;
use gpui_kit::{App, AppContext as _, Entity};
use noshape_model::document::TabId;
use std::path::{Path, PathBuf};

/// Options › Store file in a new tab: the scratch path an export is written to before it is kept as a tab.
fn scratch(file: &str) -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("noshape-store-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(file))
}

/// Options › Store file in a new tab: write the export to a temporary file
/// (`write` returns the file written: a .zip for individual files) and keep
/// it as a file tab after `tab` (shared with the agent API:
/// `noshape_ops::export::store_file_tab`). Returns the stored file's name.
pub fn store_export(doc: &Entity<DocModel>, tab: TabId, file: &str, write: impl FnOnce(&Path) -> anyhow::Result<PathBuf>, cx: &mut App) -> anyhow::Result<String> {
    let written = write(&scratch(file)?)?;
    let name = store_written(doc, tab, &written, cx);
    let _ = std::fs::remove_file(&written);
    Ok(name)
}

/// Keep an exported file (already written to `path`) as a file tab after `tab`.
pub fn store_written(doc: &Entity<DocModel>, tab: TabId, path: &Path, cx: &mut App) -> String {
    let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    if let Ok(bytes) = std::fs::read(path) {
        doc.update(cx, |d, cx| d.edit_document(&noshape_ops::export::store_description(&name), cx, |d| {
            noshape_ops::export::store_file_tab(d, &name, &bytes, tab);
        }));
    }
    name
}

/// The bell's result of an export named `name` (live2 export-feel-1118: "l2 exp was exported successfully.", without
/// the extension).
pub fn notify_export(name: &str, done: anyhow::Result<()>, cx: &mut App) {
    match done {
        Ok(()) => crate::notifications::push(cx, "Export completed", format!("{name} was exported successfully."), false),
        Err(e) => crate::notifications::push(cx, "Export failed", format!("{name} could not be exported: {e:#}"), true),
    }
}

/// An export to write: `name` for the bell, `file` (with its extension) for Store file in a new tab's tab, the dialog's
/// `options` (0 Download, 1 Download and store file in a new tab, 2 Store file in a new tab) and `download`, the file
/// the user chose (None: Store file in a new tab, written to a scratch file).
pub struct ExportJob {
    pub name: String,
    pub file: String,
    pub options: usize,
    pub download: Option<PathBuf>,
}

/// Write an export off the UI thread (a big assembly's STEP takes seconds; live keeps working while its "being
/// prepared" bubble shows), then keep it as a tab after `tab` when the job's options say so and put the result in the
/// bell.
pub fn export_in_background(doc: Entity<DocModel>, tab: TabId, job: ExportJob, write: impl FnOnce(&Path) -> anyhow::Result<PathBuf> + Send + 'static, cx: &mut App) {
    let ExportJob { name, file, options, download } = job;
    let store = download.is_none() || options == 1;
    let scratch_file = download.is_none();
    let target = match download.map_or_else(|| scratch(&file), Ok) {
        Ok(p) => p,
        Err(e) => return notify_export(&name, Err(e.into()), cx),
    };
    let job = cx.background_spawn(async move { write(&target) });
    cx.spawn(async move |cx| {
        let done = job.await;
        cx.update(|cx| {
            let done = done.map(|written| {
                if store {
                    store_written(&doc, tab, &written, cx);
                }
                if scratch_file {
                    let _ = std::fs::remove_file(&written);
                }
            });
            notify_export(&name, done, cx);
        });
    })
    .detach();
}
