use std::path::PathBuf;

use crate::{BlobHash, BranchId, MicroversionId, VersionId};

/// Store error.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("file already exists: {}", .0.display())]
    AlreadyExists(PathBuf),
    #[error("not a noshape document")]
    NotADocument,
    #[error("document schema version {found} is newer than supported version {supported}")]
    NewerSchema { found: i64, supported: i64 },
    #[error("unknown microversion {0}")]
    UnknownMicroversion(MicroversionId),
    #[error("unknown branch {0}")]
    UnknownBranch(BranchId),
    #[error("unknown version {0}")]
    UnknownVersion(VersionId),
    #[error("a branch named `{0}` already exists")]
    BranchNameTaken(String),
    #[error("name must not be empty")]
    EmptyName,
    #[error("the default branch cannot be deleted")]
    CannotDeleteDefaultBranch,
    #[error("document is open read-only")]
    ReadOnly,
    #[error("missing blob {0}")]
    MissingBlob(BlobHash),
    #[error("corrupt document: {0}")]
    Corrupt(String),
    #[error("the branch was changed outside the undo stack; undo history was cleared")]
    UndoStale,
}

pub type Result<T, E = StoreError> = std::result::Result<T, E>;
