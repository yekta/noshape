//! Onshape's Trash for the documents page (live documents/47–49, 69–71):
//! "Send to trash" moves a document or folder here, the Trash place lists
//! them (row menu Details, Restore, Delete; "Empty trash…"), and items are
//! deleted automatically after 30 days. Items live in
//! `<config>/trash/<id>/<file name>` with an index of where they came from.

use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Onshape: "Documents in the trash will be automatically deleted after 30 days."
const KEEP_MS: i64 = 30 * 24 * 3600 * 1000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrashItem {
    pub id: String,
    /// Where it was (restored there).
    pub original: PathBuf,
    /// Display name (document name or folder name).
    pub name: String,
    pub folder: bool,
    /// When it was trashed (unix ms).
    pub trashed: i64,
}

impl TrashItem {
    /// The trashed file or folder.
    pub fn stored(&self) -> PathBuf {
        dir().join(&self.id).join(self.original.file_name().unwrap_or_default())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Trash {
    pub items: Vec<TrashItem>,
}

fn dir() -> PathBuf {
    crate::paths::config_dir().join("trash")
}

fn index() -> PathBuf {
    dir().join("index.json")
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// `.noshape` side files that travel with a document.
fn with_sidecars(p: &Path) -> Vec<PathBuf> {
    let mut out = vec![p.to_path_buf()];
    for suffix in ["-wal", "-shm"] {
        let mut s = p.as_os_str().to_os_string();
        s.push(suffix);
        out.push(PathBuf::from(s));
    }
    out
}

/// Move a file or folder, copying when a rename can't (other file system).
fn move_path(from: &Path, to: &Path) -> anyhow::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if from.is_dir() {
        copy_dir(from, to)?;
        std::fs::remove_dir_all(from)?;
    } else {
        std::fs::copy(from, to)?;
        std::fs::remove_file(from)?;
    }
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let target = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

impl Trash {
    /// The index, with items older than 30 days (and lost ones) deleted.
    pub fn load() -> Self {
        let mut t: Trash = std::fs::read(index()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let now = now_ms();
        let before = t.items.len();
        t.items.retain(|i| {
            let keep = now - i.trashed < KEEP_MS && i.stored().exists();
            if !keep {
                let _ = std::fs::remove_dir_all(dir().join(&i.id));
            }
            keep
        });
        if t.items.len() != before {
            t.save();
        }
        t
    }

    fn save(&self) {
        let _ = std::fs::create_dir_all(dir());
        let _ = std::fs::write(index(), serde_json::to_vec_pretty(self).unwrap_or_default());
    }

    pub fn get(&self, id: &str) -> Option<&TrashItem> {
        self.items.iter().find(|i| i.id == id)
    }

    /// The item holding this trashed document path.
    pub fn item_of(&self, stored: &Path) -> Option<&TrashItem> {
        let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let stored = canon(stored);
        self.items.iter().find(|i| stored.starts_with(canon(&dir().join(&i.id))))
    }

    /// The trashed documents (stored paths), for the library scan.
    pub fn documents(&self) -> Vec<PathBuf> {
        self.items.iter().filter(|i| !i.folder).map(|i| i.stored()).collect()
    }

    /// Send a closed document (with its side files) or a folder to the trash.
    pub fn send(&mut self, path: &Path, name: &str, folder: bool) -> anyhow::Result<TrashItem> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let file_name = path.file_name().context("no file name")?.to_owned();
        let id = format!("{:x}{:04x}", now_ms(), self.items.len() % 0xffff);
        let slot = dir().join(&id);
        std::fs::create_dir_all(&slot)?;
        if folder {
            move_path(&path, &slot.join(&file_name))?;
        } else {
            for (from, to) in with_sidecars(&path).into_iter().zip(with_sidecars(&slot.join(&file_name))) {
                if from.exists() {
                    move_path(&from, &to)?;
                }
            }
        }
        let item = TrashItem { id, original: path, name: name.to_string(), folder, trashed: now_ms() };
        self.items.push(item.clone());
        self.save();
        Ok(item)
    }

    /// Put an item back where it was (a free name if that place is taken).
    pub fn restore(&mut self, id: &str) -> anyhow::Result<PathBuf> {
        let Some(item) = self.get(id).cloned() else { bail!("This item is no longer in the trash") };
        let parent = item.original.parent().context("no folder")?.to_path_buf();
        std::fs::create_dir_all(&parent)?;
        let mut dest = item.original.clone();
        if dest.exists() {
            let stem = dest.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let ext = dest.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
            dest = (1..).map(|n| parent.join(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).expect("free name");
        }
        let stored = item.stored();
        if item.folder {
            move_path(&stored, &dest)?;
        } else {
            for (from, to) in with_sidecars(&stored).into_iter().zip(with_sidecars(&dest)) {
                if from.exists() {
                    move_path(&from, &to)?;
                }
            }
        }
        let _ = std::fs::remove_dir_all(dir().join(id));
        self.items.retain(|i| i.id != id);
        self.save();
        Ok(dest)
    }

    /// Delete an item permanently.
    pub fn delete(&mut self, id: &str) -> anyhow::Result<()> {
        let slot = dir().join(id);
        if slot.exists() {
            std::fs::remove_dir_all(&slot)?;
        }
        self.items.retain(|i| i.id != id);
        self.save();
        Ok(())
    }

    /// "Empty trash…": delete every item.
    pub fn empty(&mut self) -> anyhow::Result<()> {
        for id in self.items.iter().map(|i| i.id.clone()).collect::<Vec<_>>() {
            self.delete(&id)?;
        }
        Ok(())
    }
}
