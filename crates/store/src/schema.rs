//! Schema creation and migrations.

use rusqlite::{Connection, OptionalExtension};

use crate::{Result, StoreError};

/// `PRAGMA application_id` of noshape documents ("NSHP").
pub const APPLICATION_ID: i32 = 0x4E53_4850;

/// Current schema version. Bump and add a step to [`migrate`] when changing
/// the schema.
pub const SCHEMA_VERSION: i64 = 1;

const SCHEMA_V1: &str = r#"
CREATE TABLE schema_version (
    id      INTEGER PRIMARY KEY CHECK (id = 0),
    version INTEGER NOT NULL
);
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value BLOB NOT NULL
);
-- Content-addressed blobs. codec 0 = raw, 1 = zstd.
CREATE TABLE blobs (
    id    INTEGER PRIMARY KEY,
    hash  BLOB NOT NULL UNIQUE,
    size  INTEGER NOT NULL,
    codec INTEGER NOT NULL,
    data  BLOB NOT NULL
);
CREATE TABLE microversions (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    id          BLOB NOT NULL UNIQUE,
    branch      BLOB NOT NULL,
    parent1     BLOB,
    parent2     BLOB,
    created     INTEGER NOT NULL,
    author      TEXT NOT NULL,
    description TEXT NOT NULL,
    manifest    BLOB NOT NULL
);
CREATE INDEX microversions_branch ON microversions(branch);
CREATE TABLE branches (
    id           BLOB PRIMARY KEY,
    name         TEXT NOT NULL,
    head         BLOB NOT NULL,
    created      INTEGER NOT NULL,
    created_from BLOB,
    is_default   INTEGER NOT NULL DEFAULT 0,
    deleted      INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX branches_live_name ON branches(name) WHERE deleted = 0;
CREATE TABLE versions (
    seq          INTEGER PRIMARY KEY AUTOINCREMENT,
    id           BLOB NOT NULL UNIQUE,
    name         TEXT NOT NULL,
    description  TEXT NOT NULL,
    microversion BLOB NOT NULL,
    created      INTEGER NOT NULL,
    author       TEXT NOT NULL
);
INSERT INTO schema_version (id, version) VALUES (0, 1);
"#;

/// Connection settings shared by writers and readers.
pub(crate) fn configure(conn: &Connection, writer: bool) -> Result<()> {
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    conn.set_prepared_statement_cache_capacity(64);
    if writer {
        // WAL: readers never block the writer and vice versa; a crash (of the
        // app or the OS) can never corrupt the file. synchronous=NORMAL in WAL
        // mode is consistent after power loss but may drop the last commits;
        // call `Document::set_durable(true)` for fsync-per-commit.
        let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            log::warn!("could not enable WAL mode (got {mode})");
        }
        conn.pragma_update(None, "synchronous", "NORMAL")?;
    }
    conn.pragma_update(None, "cache_size", -32_000)?;
    Ok(())
}

/// Creates the schema in an empty database.
pub(crate) fn create(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    tx.pragma_update(None, "application_id", APPLICATION_ID)?;
    tx.execute_batch(SCHEMA_V1)?;
    tx.commit()?;
    Ok(())
}

/// Validates an existing database and migrates it to [`SCHEMA_VERSION`].
pub(crate) fn check_and_migrate(conn: &mut Connection, writable: bool) -> Result<()> {
    let app_id: i32 = conn.query_row("PRAGMA application_id", [], |r| r.get(0))?;
    if app_id != APPLICATION_ID {
        return Err(StoreError::NotADocument);
    }
    let version: Option<i64> = conn
        .query_row("SELECT version FROM schema_version WHERE id = 0", [], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|_| StoreError::NotADocument)?;
    let version = version.ok_or(StoreError::NotADocument)?;
    if version > SCHEMA_VERSION {
        return Err(StoreError::NewerSchema {
            found: version,
            supported: SCHEMA_VERSION,
        });
    }
    if version < SCHEMA_VERSION {
        if !writable {
            return Err(StoreError::ReadOnly);
        }
        migrate(conn, version)?;
    }
    Ok(())
}

/// Runs migration steps `from -> from+1 -> ... -> SCHEMA_VERSION` in one
/// transaction.
fn migrate(conn: &mut Connection, from: i64) -> Result<()> {
    let tx = conn.transaction()?;
    for v in from..SCHEMA_VERSION {
        migration_step(&tx, v)?;
    }
    tx.execute(
        "UPDATE schema_version SET version = ?1 WHERE id = 0",
        [SCHEMA_VERSION],
    )?;
    tx.commit()?;
    Ok(())
}

/// Upgrades the schema from version `v` to `v + 1`.
fn migration_step(_conn: &Connection, v: i64) -> Result<()> {
    #[allow(clippy::match_single_binding)]
    match v {
        // 1 => conn.execute_batch("ALTER TABLE ...")?,
        _ => Err(StoreError::Corrupt(format!(
            "no migration from schema version {v}"
        ))),
    }
}
