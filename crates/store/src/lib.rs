//! noshape-store: local document storage.
//!
//! A noshape document is a single `.noshape` file (SQLite, WAL mode) holding
//! the complete edit history Onshape-style:
//!
//! * the model serializes the document into named blobs (`"tab/<uuid>"`,
//!   `"document/meta"`, ...); blobs are content-addressed (blake3) and
//!   zstd-compressed, so a microversion that changes one tab stores only that
//!   tab's new bytes;
//! * every edit is a **microversion** (`id`, parents, branch, time, author,
//!   description, manifest of name -> blob hash);
//! * **branches** (Onshape workspaces) point at a head microversion; the
//!   default branch is "Main";
//! * **versions** are named, immutable pointers to microversions;
//! * **restore**, **merge** (three-way, per blob name) and **undo/redo**
//!   ([`UndoStack`]) always create new microversions; history is never
//!   rewritten.
//!
//! ```no_run
//! use noshape_store::Document;
//! let mut doc = Document::create("part.noshape", &[("tab/1", b"v1".as_slice())])?;
//! let main = doc.default_branch()?;
//! let mv = doc.commit(main, &[("tab/1", Some(b"v2".as_slice()))], "Edit Extrude 1")?;
//! assert_eq!(doc.read(mv, "tab/1")?.as_deref(), Some(b"v2".as_slice()));
//! # Ok::<(), noshape_store::StoreError>(())
//! ```

mod document;
mod error;
mod ids;
mod manifest;
mod schema;
mod undo;

#[cfg(test)]
mod tests;

pub use document::{
    BranchInfo, DEFAULT_BRANCH_NAME, Document, DocumentMeta, MergeOutcome, MicroversionInfo,
    StoreStats, Timestamp, VersionGraph, VersionInfo, local_user_name, os_login, os_user_name, owner_and_last_author, set_default_author,
};
pub use error::{Result, StoreError};
pub use ids::{BlobHash, BranchId, MicroversionId, VersionId};
pub use manifest::{ChangeKind, Manifest, NameChange, diff};
pub use schema::{APPLICATION_ID, SCHEMA_VERSION};
pub use undo::UndoStack;

/// Merge resolver that keeps the target branch's content.
pub fn take_ours(
    _: &str,
    _: Option<&[u8]>,
    ours: Option<&[u8]>,
    _: Option<&[u8]>,
) -> Option<Vec<u8>> {
    ours.map(<[u8]>::to_vec)
}

/// Merge resolver that takes the source branch's content.
pub fn take_theirs(
    _: &str,
    _: Option<&[u8]>,
    _: Option<&[u8]>,
    theirs: Option<&[u8]>,
) -> Option<Vec<u8>> {
    theirs.map(<[u8]>::to_vec)
}
