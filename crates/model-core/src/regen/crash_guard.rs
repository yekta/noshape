//! Crash markers (qa DOC-POISON): a feature whose evaluation kills the
//! process (a crash inside the kernel) must not make its document
//! unopenable. While a feature is evaluated, a small marker file names it
//! (process id, feature id, feature name); it is removed when the
//! evaluation returns. A marker left behind by a process that is gone names
//! the feature that crashed it: opening the document then rolls its Part
//! Studio back above that feature (`noshape_ops::session`). Off until a
//! session enables it ([`enable`]), so tests and tools write nothing.

use crate::reference::FeatureId;
use std::path::PathBuf;
use std::sync::OnceLock;

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// The default marker folder: `<temp>/noshape-regen`.
pub fn default_dir() -> PathBuf {
    std::env::temp_dir().join("noshape-regen")
}

/// Write markers into `dir` from now on (the first call wins).
pub fn enable(dir: PathBuf) {
    if DIR.get().is_none() && std::fs::create_dir_all(&dir).is_ok() {
        let _ = DIR.set(dir);
    }
}

/// The marker folder, when enabled.
pub fn dir() -> Option<&'static PathBuf> {
    DIR.get()
}

/// Marks a feature as being evaluated by this thread until dropped.
pub struct Guard(Option<PathBuf>);

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Mark `feature` (named `name`) as evaluating on this thread.
pub fn enter(feature: FeatureId, name: &str) -> Guard {
    let Some(dir) = DIR.get() else { return Guard(None) };
    let thread: String = format!("{:?}", std::thread::current().id()).chars().filter(char::is_ascii_digit).collect();
    let path = dir.join(format!("{}-{thread}.marker", std::process::id()));
    match std::fs::write(&path, format!("{}\n{feature}\n{name}\n", std::process::id())) {
        Ok(()) => Guard(Some(path)),
        Err(_) => Guard(None),
    }
}

/// A marker on disk: the process that wrote it and the feature it was
/// evaluating.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marker {
    pub path: PathBuf,
    pub pid: u32,
    /// The feature id as [`FeatureId`]'s text ("F6547f48e1738").
    pub feature: String,
    pub name: String,
}

/// Every marker in `dir`.
pub fn markers(dir: &std::path::Path) -> Vec<Marker> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    rd.flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "marker"))
        .filter_map(|e| {
            let text = std::fs::read_to_string(e.path()).ok()?;
            let mut lines = text.lines();
            let pid = lines.next()?.trim().parse().ok()?;
            let feature = lines.next()?.trim().to_string();
            let name = lines.next().unwrap_or("").to_string();
            Some(Marker { path: e.path(), pid, feature, name })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_marker_lives_while_its_guard_does() {
        let dir = std::env::temp_dir().join(format!("noshape-regen-test-{}", std::process::id()));
        super::enable(dir.clone());
        let dir = super::dir().expect("enabled").clone();
        let id = crate::reference::FeatureId(0x5eed);
        let g = super::enter(id, "Draft 1");
        let mine = |m: &super::Marker| m.pid == std::process::id() && m.feature == id.to_string();
        assert!(super::markers(&dir).iter().any(|m| mine(m) && m.name == "Draft 1"));
        drop(g);
        assert!(!super::markers(&dir).iter().any(mine));
    }
}
