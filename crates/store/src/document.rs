//! The `.noshape` document file.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::manifest::{self, Manifest, NameChange};
use crate::schema;
use crate::{BlobHash, BranchId, MicroversionId, Result, StoreError, VersionId};

/// Name of the branch created with every document.
pub const DEFAULT_BRANCH_NAME: &str = "Main";

/// Blobs smaller than this are stored uncompressed.
const MIN_COMPRESS_SIZE: usize = 64;
/// zstd level for small blobs (best ratio per byte, cheap).
const ZSTD_LEVEL: i32 = 3;
/// zstd level for large blobs (keeps a 1 MB commit at a few ms).
const ZSTD_LEVEL_LARGE: i32 = 1;
const LARGE_BLOB: usize = 256 * 1024;
const MANIFEST_CACHE_LIMIT: usize = 512;

const CODEC_RAW: i64 = 0;
const CODEC_ZSTD: i64 = 1;

/// Milliseconds since the Unix epoch.
pub type Timestamp = i64;

pub(crate) fn now_ms() -> Timestamp {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Name of the local OS user, used as the author of microversions.
pub fn local_user_name() -> String {
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "unknown".to_string())
}

/// One committed document state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MicroversionInfo {
    pub id: MicroversionId,
    /// Creation order within the document (strictly increasing).
    pub seq: i64,
    /// No parents for the initial microversion, two for merges (first parent
    /// is the branch merged into).
    pub parents: Vec<MicroversionId>,
    /// Branch the microversion was created on.
    pub branch: BranchId,
    pub created: Timestamp,
    pub author: String,
    pub description: String,
    pub manifest_hash: BlobHash,
}

impl MicroversionInfo {
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

/// A branch (Onshape: workspace).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchInfo {
    pub id: BranchId,
    pub name: String,
    pub head: MicroversionId,
    pub created: Timestamp,
    /// Microversion the branch was created from (`None` for the default
    /// branch).
    pub created_from: Option<MicroversionId>,
    pub is_default: bool,
    /// Deleted branches keep their record so the graph can still label their
    /// microversions.
    pub deleted: bool,
}

/// A named, immutable version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionInfo {
    pub id: VersionId,
    pub name: String,
    pub description: String,
    pub microversion: MicroversionId,
    pub created: Timestamp,
    pub author: String,
}

/// Document-level metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub name: String,
    pub description: String,
    /// Opaque to the store (the model stores serialized unit preferences).
    pub default_units: String,
    pub created: Timestamp,
    pub modified: Timestamp,
    /// PNG bytes.
    pub thumbnail: Option<Vec<u8>>,
}

/// The complete version graph, for drawing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionGraph {
    /// Every microversion in creation order (parents always precede
    /// children).
    pub microversions: Vec<MicroversionInfo>,
    /// All branches, including deleted ones.
    pub branches: Vec<BranchInfo>,
    pub versions: Vec<VersionInfo>,
}

impl VersionGraph {
    pub fn get(&self, id: MicroversionId) -> Option<&MicroversionInfo> {
        self.microversions.iter().find(|m| m.id == id)
    }

    /// Children of `id` in creation order.
    pub fn children(&self, id: MicroversionId) -> Vec<MicroversionId> {
        self.microversions
            .iter()
            .filter(|m| m.parents.contains(&id))
            .map(|m| m.id)
            .collect()
    }

    /// Map from microversion to child ids, built in one pass.
    pub fn child_map(&self) -> HashMap<MicroversionId, Vec<MicroversionId>> {
        let mut map: HashMap<MicroversionId, Vec<MicroversionId>> = HashMap::new();
        for m in &self.microversions {
            for p in &m.parents {
                map.entry(*p).or_default().push(m.id);
            }
        }
        map
    }
}

/// Result of [`Document::merge`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeOutcome {
    /// New head of the target branch (the merge microversion, or the
    /// unchanged head if there was nothing to merge).
    pub microversion: MicroversionId,
    /// Merge base (lowest common ancestor) used for the three-way merge.
    pub base: MicroversionId,
    /// Names taken from the source branch without conflict.
    pub taken_from_source: Vec<String>,
    /// Names changed on both sides, resolved through the callback.
    pub conflicts: Vec<String>,
    /// True if the source was already contained in the target; no
    /// microversion was created.
    pub up_to_date: bool,
}

/// Storage statistics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StoreStats {
    pub microversions: u64,
    pub blobs: u64,
    pub raw_bytes: u64,
    pub stored_bytes: u64,
}

/// An open `.noshape` document.
///
/// A `Document` owns one SQLite connection and is `Send` but not `Sync`.
/// For background loading open additional read-only handles with
/// [`Document::open_read_only`] (or [`Document::reader`]) on other threads;
/// WAL mode lets them read concurrently with commits.
pub struct Document {
    conn: Connection,
    path: PathBuf,
    read_only: bool,
    author: String,
    manifest_cache: RefCell<HashMap<MicroversionId, Arc<Manifest>>>,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("path", &self.path)
            .field("read_only", &self.read_only)
            .finish_non_exhaustive()
    }
}

const MV_COLS: &str =
    "m.seq, m.id, m.branch, m.parent1, m.parent2, m.created, m.author, m.description, m.manifest";

fn mv_from_row(r: &Row<'_>) -> rusqlite::Result<MicroversionInfo> {
    let p1: Option<MicroversionId> = r.get(3)?;
    let p2: Option<MicroversionId> = r.get(4)?;
    Ok(MicroversionInfo {
        seq: r.get(0)?,
        id: r.get(1)?,
        branch: r.get(2)?,
        parents: p1.into_iter().chain(p2).collect(),
        created: r.get(5)?,
        author: r.get(6)?,
        description: r.get(7)?,
        manifest_hash: r.get(8)?,
    })
}

const BRANCH_COLS: &str = "id, name, head, created, created_from, is_default, deleted";

fn branch_from_row(r: &Row<'_>) -> rusqlite::Result<BranchInfo> {
    Ok(BranchInfo {
        id: r.get(0)?,
        name: r.get(1)?,
        head: r.get(2)?,
        created: r.get(3)?,
        created_from: r.get(4)?,
        is_default: r.get(5)?,
        deleted: r.get(6)?,
    })
}

const VERSION_COLS: &str = "id, name, description, microversion, created, author";

fn version_from_row(r: &Row<'_>) -> rusqlite::Result<VersionInfo> {
    Ok(VersionInfo {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        microversion: r.get(3)?,
        created: r.get(4)?,
        author: r.get(5)?,
    })
}

/// Recursive CTE selecting all ancestors (inclusive) of `?{param}` into
/// table `name`.
fn ancestors_cte(name: &str, param: u32) -> String {
    format!(
        "{name}(id) AS (
            SELECT ?{param}
            UNION
            SELECT m.parent1 FROM microversions m JOIN {name} ON m.id = {name}.id
                WHERE m.parent1 IS NOT NULL
            UNION
            SELECT m.parent2 FROM microversions m JOIN {name} ON m.id = {name}.id
                WHERE m.parent2 IS NOT NULL
        )"
    )
}

// ---------------------------------------------------------------------------
// Blob primitives (work on any connection or transaction)
// ---------------------------------------------------------------------------

fn put_blob(conn: &Connection, data: &[u8]) -> Result<BlobHash> {
    let hash = BlobHash::of(data);
    let exists = conn
        .prepare_cached("SELECT 1 FROM blobs WHERE hash = ?1")?
        .exists([hash])?;
    if !exists {
        let compressed = if data.len() >= MIN_COMPRESS_SIZE {
            zstd::bulk::compress(
                data,
                if data.len() >= LARGE_BLOB {
                    ZSTD_LEVEL_LARGE
                } else {
                    ZSTD_LEVEL
                },
            )
            .ok()
            .filter(|c| c.len() < data.len())
        } else {
            None
        };
        let (codec, payload) = match &compressed {
            Some(c) => (CODEC_ZSTD, c.as_slice()),
            None => (CODEC_RAW, data),
        };
        conn.prepare_cached("INSERT INTO blobs (hash, size, codec, data) VALUES (?1, ?2, ?3, ?4)")?
            .execute(params![hash, data.len() as i64, codec, payload])?;
    }
    Ok(hash)
}

fn get_blob(conn: &Connection, hash: BlobHash) -> Result<Option<Vec<u8>>> {
    let mut stmt = conn.prepare_cached("SELECT size, codec, data FROM blobs WHERE hash = ?1")?;
    let mut rows = stmt.query([hash])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let size: i64 = row.get(0)?;
    let codec: i64 = row.get(1)?;
    let data = row.get_ref(2)?.as_blob().map_err(rusqlite::Error::from)?;
    let out = match codec {
        CODEC_RAW => data.to_vec(),
        CODEC_ZSTD => zstd::bulk::decompress(data, size as usize)
            .map_err(|e| StoreError::Corrupt(format!("blob {hash}: {e}")))?,
        c => {
            return Err(StoreError::Corrupt(format!(
                "blob {hash}: unknown codec {c}"
            )));
        }
    };
    if out.len() as i64 != size {
        return Err(StoreError::Corrupt(format!("blob {hash}: size mismatch")));
    }
    Ok(Some(out))
}

struct NewMicroversion<'a> {
    branch: BranchId,
    parents: &'a [MicroversionId],
    manifest: &'a Manifest,
    author: &'a str,
    description: &'a str,
}

/// Inserts a microversion and moves the branch head to it.
fn insert_microversion(conn: &Connection, mv: NewMicroversion<'_>) -> Result<MicroversionId> {
    let manifest_hash = put_blob(conn, &manifest::encode(mv.manifest))?;
    let id = MicroversionId::generate();
    let now = now_ms();
    conn.prepare_cached(
        "INSERT INTO microversions
            (id, branch, parent1, parent2, created, author, description, manifest)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?
    .execute(params![
        id,
        mv.branch,
        mv.parents.first(),
        mv.parents.get(1),
        now,
        mv.author,
        mv.description,
        manifest_hash
    ])?;
    conn.prepare_cached("UPDATE branches SET head = ?1 WHERE id = ?2")?
        .execute(params![id, mv.branch])?;
    conn.prepare_cached("INSERT OR REPLACE INTO meta (key, value) VALUES ('modified', ?1)")?
        .execute([now.to_string().into_bytes()])?;
    Ok(id)
}

fn live_head(conn: &Connection, branch: BranchId) -> Result<MicroversionId> {
    conn.prepare_cached("SELECT head FROM branches WHERE id = ?1 AND deleted = 0")?
        .query_row([branch], |r| r.get(0))
        .optional()?
        .ok_or(StoreError::UnknownBranch(branch))
}

fn check_name(name: &str) -> Result<&str> {
    let n = name.trim();
    if n.is_empty() {
        Err(StoreError::EmptyName)
    } else {
        Ok(n)
    }
}

impl Document {
    // -----------------------------------------------------------------------
    // Opening
    // -----------------------------------------------------------------------

    /// Creates a new document file with one branch ("Main") whose first
    /// microversion contains `initial` (name -> bytes). Fails if the file
    /// exists. The document name defaults to the file stem.
    pub fn create<N: AsRef<str>, B: AsRef<[u8]>>(
        path: impl AsRef<Path>,
        initial: &[(N, B)],
    ) -> Result<Document> {
        let path = path.as_ref();
        if path.exists() {
            return Err(StoreError::AlreadyExists(path.to_path_buf()));
        }
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        schema::configure(&conn, true)?;
        schema::create(&mut conn)?;
        let author = local_user_name();
        let tx = conn.transaction()?;
        let mut m = Manifest::new();
        for (name, data) in initial {
            m.insert(name.as_ref().to_string(), put_blob(&tx, data.as_ref())?);
        }
        let branch = BranchId::generate();
        let now = now_ms();
        // Insert the branch with a placeholder head; insert_microversion
        // moves it.
        tx.execute(
            "INSERT INTO branches (id, name, head, created, created_from, is_default)
             VALUES (?1, ?2, ?3, ?4, NULL, 1)",
            params![
                branch,
                DEFAULT_BRANCH_NAME,
                MicroversionId(uuid::Uuid::nil()),
                now
            ],
        )?;
        insert_microversion(
            &tx,
            NewMicroversion {
                branch,
                parents: &[],
                manifest: &m,
                author: &author,
                description: "Create document",
            },
        )?;
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        for (k, v) in [("name", name), ("created", now.to_string())] {
            tx.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                params![k, v.into_bytes()],
            )?;
        }
        tx.commit()?;
        Ok(Document {
            conn,
            path: path.to_path_buf(),
            read_only: false,
            author,
            manifest_cache: RefCell::default(),
        })
    }

    /// Opens an existing document for reading and writing (migrating the
    /// schema if it is older).
    pub fn open(path: impl AsRef<Path>) -> Result<Document> {
        Self::open_impl(path.as_ref(), false)
    }

    /// Opens a read-only handle, e.g. for a background loading thread. It
    /// can be used while another handle commits.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Document> {
        Self::open_impl(path.as_ref(), true)
    }

    /// Opens another read-only handle on the same file.
    pub fn reader(&self) -> Result<Document> {
        Self::open_read_only(&self.path)
    }

    fn open_impl(path: &Path, read_only: bool) -> Result<Document> {
        if !path.exists() {
            return Err(StoreError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} not found", path.display()),
            )));
        }
        let flags = if read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX
        };
        let mut conn = Connection::open_with_flags(path, flags)?;
        schema::configure(&conn, !read_only).map_err(|e| match e {
            StoreError::Sqlite(rusqlite::Error::SqliteFailure(f, _))
                if f.code == rusqlite::ErrorCode::NotADatabase =>
            {
                StoreError::NotADocument
            }
            e => e,
        })?;
        schema::check_and_migrate(&mut conn, !read_only)?;
        Ok(Document {
            conn,
            path: path.to_path_buf(),
            read_only,
            author: local_user_name(),
            manifest_cache: RefCell::default(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Author recorded on new microversions and versions (defaults to the
    /// OS user name).
    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn set_author(&mut self, author: impl Into<String>) {
        self.author = author.into();
    }

    /// `true`: fsync on every commit (survives power loss without losing
    /// the last commits). `false` (default): WAL `synchronous=NORMAL`, which
    /// is still corruption-free after a crash but may lose the most recent
    /// commits on power failure.
    pub fn set_durable(&self, durable: bool) -> Result<()> {
        self.conn
            .pragma_update(None, "synchronous", if durable { "FULL" } else { "NORMAL" })?;
        Ok(())
    }

    /// Writes a compacted, self-contained copy of the document to `dest`
    /// (Save As / backup). Safe while the document is in use.
    pub fn backup_to(&self, dest: impl AsRef<Path>) -> Result<()> {
        let dest = dest.as_ref();
        if dest.exists() {
            return Err(StoreError::AlreadyExists(dest.to_path_buf()));
        }
        self.conn
            .execute("VACUUM INTO ?1", [dest.to_string_lossy()])?;
        Ok(())
    }

    /// Moves WAL content into the main file (done automatically too).
    pub fn checkpoint(&self) -> Result<()> {
        self.writable()?;
        self.conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        Ok(())
    }

    fn writable(&self) -> Result<()> {
        if self.read_only {
            Err(StoreError::ReadOnly)
        } else {
            Ok(())
        }
    }

    // -----------------------------------------------------------------------
    // Reading
    // -----------------------------------------------------------------------

    /// Manifest (name -> blob hash) of a microversion. Cached in memory.
    pub fn manifest(&self, mv: MicroversionId) -> Result<Arc<Manifest>> {
        let m = load_manifest_cached(&self.conn, &self.manifest_cache, mv)?;
        self.cache_manifest(mv, m.clone());
        Ok(m)
    }

    fn cache_manifest(&self, mv: MicroversionId, m: Arc<Manifest>) {
        let mut cache = self.manifest_cache.borrow_mut();
        if cache.len() >= MANIFEST_CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(mv, m);
    }

    /// Names present in a microversion.
    pub fn names(&self, mv: MicroversionId) -> Result<Vec<String>> {
        Ok(self.manifest(mv)?.keys().cloned().collect())
    }

    /// Content of blob `name` in microversion `mv` (`None` if absent).
    pub fn read(&self, mv: MicroversionId, name: &str) -> Result<Option<Vec<u8>>> {
        let m = self.manifest(mv)?;
        match m.get(name) {
            None => Ok(None),
            Some(&h) => Ok(Some(
                get_blob(&self.conn, h)?.ok_or(StoreError::MissingBlob(h))?,
            )),
        }
    }

    /// Content of a blob by hash.
    pub fn read_blob(&self, hash: BlobHash) -> Result<Option<Vec<u8>>> {
        get_blob(&self.conn, hash)
    }

    /// Every blob of a microversion.
    pub fn read_all(&self, mv: MicroversionId) -> Result<BTreeMap<String, Vec<u8>>> {
        let m = self.manifest(mv)?;
        m.iter()
            .map(|(name, &h)| {
                Ok((
                    name.clone(),
                    get_blob(&self.conn, h)?.ok_or(StoreError::MissingBlob(h))?,
                ))
            })
            .collect()
    }

    /// Names that changed between two microversions.
    pub fn diff(&self, from: MicroversionId, to: MicroversionId) -> Result<Vec<NameChange>> {
        Ok(manifest::diff(&*self.manifest(from)?, &*self.manifest(to)?))
    }

    pub fn microversion(&self, id: MicroversionId) -> Result<MicroversionInfo> {
        self.conn
            .prepare_cached(&format!(
                "SELECT {MV_COLS} FROM microversions m WHERE m.id = ?1"
            ))?
            .query_row([id], mv_from_row)
            .optional()?
            .ok_or(StoreError::UnknownMicroversion(id))
    }

    /// Current head of a (live) branch.
    pub fn head(&self, branch: BranchId) -> Result<MicroversionId> {
        live_head(&self.conn, branch)
    }

    /// History of a branch, newest first, following first parents from its
    /// head (merged-in microversions of other branches are reachable through
    /// `parents[1]` of merge microversions).
    pub fn history(&self, branch: BranchId) -> Result<Vec<MicroversionInfo>> {
        self.history_from(self.head(branch)?, None)
    }

    /// First-parent history starting at `mv` (inclusive), newest first, at
    /// most `limit` entries.
    pub fn history_from(
        &self,
        mv: MicroversionId,
        limit: Option<usize>,
    ) -> Result<Vec<MicroversionInfo>> {
        let limit = limit.map(|l| l as i64).unwrap_or(i64::MAX);
        if limit == 0 {
            return Ok(Vec::new());
        }
        let sql = format!(
            "WITH RECURSIVE h(id, depth) AS (
                SELECT ?1, 0
                UNION ALL
                SELECT m.parent1, h.depth + 1 FROM microversions m JOIN h ON m.id = h.id
                    WHERE m.parent1 IS NOT NULL AND h.depth + 1 < ?2
            )
            SELECT {MV_COLS} FROM h JOIN microversions m ON m.id = h.id ORDER BY h.depth"
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![mv, limit], mv_from_row)?;
        let out = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        if out.is_empty() {
            return Err(StoreError::UnknownMicroversion(mv));
        }
        Ok(out)
    }

    /// True if `ancestor` is `of` or one of its (transitive) parents.
    pub fn is_ancestor(&self, ancestor: MicroversionId, of: MicroversionId) -> Result<bool> {
        let sql = format!(
            "WITH RECURSIVE {} SELECT EXISTS(SELECT 1 FROM a WHERE id = ?2)",
            ancestors_cte("a", 1)
        );
        Ok(self
            .conn
            .prepare_cached(&sql)?
            .query_row(params![of, ancestor], |r| r.get(0))?)
    }

    /// Lowest common ancestor of two microversions (the most recently
    /// created common ancestor, which is never an ancestor of another common
    /// ancestor).
    pub fn merge_base(
        &self,
        a: MicroversionId,
        b: MicroversionId,
    ) -> Result<Option<MicroversionId>> {
        let sql = format!(
            "WITH RECURSIVE {}, {}
             SELECT m.id FROM microversions m
             WHERE m.id IN (SELECT id FROM a) AND m.id IN (SELECT id FROM b)
             ORDER BY m.seq DESC LIMIT 1",
            ancestors_cte("a", 1),
            ancestors_cte("b", 2)
        );
        Ok(self
            .conn
            .prepare_cached(&sql)?
            .query_row(params![a, b], |r| r.get(0))
            .optional()?)
    }

    /// The full DAG: all microversions, branches and versions.
    pub fn graph(&self) -> Result<VersionGraph> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {MV_COLS} FROM microversions m ORDER BY m.seq"
        ))?;
        let microversions = stmt
            .query_map([], mv_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {BRANCH_COLS} FROM branches ORDER BY is_default DESC, created, rowid"
        ))?;
        let branches = stmt
            .query_map([], branch_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(VersionGraph {
            microversions,
            branches,
            versions: self.list_versions()?,
        })
    }

    // -----------------------------------------------------------------------
    // Committing
    // -----------------------------------------------------------------------

    /// Applies `changes` (name -> new content, `None` deletes the name) on
    /// top of the branch head and records a new microversion with the given
    /// description (e.g. "Edit Extrude 1"). Only blobs whose content is new
    /// to the document are stored. Runs in one transaction.
    ///
    /// If the changes leave the manifest identical, no microversion is
    /// created and the current head is returned.
    pub fn commit<N: AsRef<str>, B: AsRef<[u8]>>(
        &mut self,
        branch: BranchId,
        changes: &[(N, Option<B>)],
        description: &str,
    ) -> Result<MicroversionId> {
        self.writable()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let head = live_head(&tx, branch)?;
        let base = load_manifest_cached(&tx, &self.manifest_cache, head)?;
        let mut m = (*base).clone();
        for (name, data) in changes {
            match data {
                Some(d) => {
                    m.insert(name.as_ref().to_string(), put_blob(&tx, d.as_ref())?);
                }
                None => {
                    m.remove(name.as_ref());
                }
            }
        }
        if m == *base {
            return Ok(head);
        }
        let id = insert_microversion(
            &tx,
            NewMicroversion {
                branch,
                parents: &[head],
                manifest: &m,
                author: &self.author,
                description,
            },
        )?;
        tx.commit()?;
        self.cache_manifest(id, Arc::new(m));
        Ok(id)
    }

    /// Creates a microversion on `branch` whose manifest equals `manifest`.
    fn commit_manifest(
        &mut self,
        branch: BranchId,
        manifest: Arc<Manifest>,
        extra_parent: Option<MicroversionId>,
        description: &str,
    ) -> Result<MicroversionId> {
        self.writable()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let head = live_head(&tx, branch)?;
        if extra_parent.is_none() {
            let current = load_manifest_cached(&tx, &self.manifest_cache, head)?;
            if *current == *manifest {
                return Ok(head);
            }
        }
        let parents: Vec<MicroversionId> = std::iter::once(head).chain(extra_parent).collect();
        let id = insert_microversion(
            &tx,
            NewMicroversion {
                branch,
                parents: &parents,
                manifest: &manifest,
                author: &self.author,
                description,
            },
        )?;
        tx.commit()?;
        self.cache_manifest(id, manifest);
        Ok(id)
    }

    /// Onshape "Restore": creates a new microversion on `branch` whose
    /// content equals `to` (which may be from any branch). History is kept.
    /// Returns the head unchanged if the content already matches.
    pub fn restore(&mut self, branch: BranchId, to: MicroversionId) -> Result<MicroversionId> {
        let target = self.microversion(to)?;
        let version_name: Option<String> = self
            .conn
            .prepare_cached(
                "SELECT name FROM versions WHERE microversion = ?1 ORDER BY seq LIMIT 1",
            )?
            .query_row([to], |r| r.get(0))
            .optional()?;
        let description = match version_name {
            Some(v) => format!("Restore to version \"{v}\""),
            None => format!("Restore to \"{}\"", target.description),
        };
        self.restore_with_description(branch, to, &description)
    }

    /// [`Document::restore`] with an explicit description.
    pub fn restore_with_description(
        &mut self,
        branch: BranchId,
        to: MicroversionId,
        description: &str,
    ) -> Result<MicroversionId> {
        let m = self.manifest(to)?;
        self.commit_manifest(branch, m, None, description)
    }

    /// Three-way merge of branch `from` into branch `into`, at blob-name
    /// granularity:
    ///
    /// * the merge base is the lowest common ancestor of both heads;
    /// * names unchanged on one side take the other side's content
    ///   (including deletion);
    /// * names changed identically on both sides keep that content;
    /// * names changed differently on both sides call
    ///   `resolver(name, base, ours, theirs)` where *ours* is `into` and
    ///   *theirs* is `from`; `None` input means "absent", and returning
    ///   `None` deletes the name.
    ///
    /// Creates a merge microversion on `into` with parents
    /// `[into head, from head]`. If `from` is already contained in `into`,
    /// nothing is created (`up_to_date`). If `into` is an ancestor of `from`
    /// the result equals `from` but is still recorded as a merge
    /// microversion on `into` (no fast-forward), like Onshape.
    pub fn merge<F>(
        &mut self,
        from: BranchId,
        into: BranchId,
        mut resolver: F,
    ) -> Result<MergeOutcome>
    where
        F: FnMut(&str, Option<&[u8]>, Option<&[u8]>, Option<&[u8]>) -> Option<Vec<u8>>,
    {
        self.writable()?;
        let theirs_head = self.head(from)?;
        let ours_head = self.head(into)?;
        let base = self
            .merge_base(ours_head, theirs_head)?
            .ok_or_else(|| StoreError::Corrupt("branches share no ancestor".into()))?;
        if base == theirs_head {
            return Ok(MergeOutcome {
                microversion: ours_head,
                base,
                taken_from_source: Vec::new(),
                conflicts: Vec::new(),
                up_to_date: true,
            });
        }
        let bm = self.manifest(base)?;
        let om = self.manifest(ours_head)?;
        let tm = self.manifest(theirs_head)?;
        let from_name = self.branch(from)?.name;
        let into_name = self.branch(into)?.name;

        let names: BTreeSet<&String> = bm.keys().chain(om.keys()).chain(tm.keys()).collect();
        let mut result = Manifest::new();
        let mut taken = Vec::new();
        let mut conflicts = Vec::new();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if live_head(&tx, into)? != ours_head || live_head(&tx, from)? != theirs_head {
            return Err(StoreError::Corrupt("branch moved during merge".into()));
        }
        for name in names {
            let (b, o, t) = (bm.get(name), om.get(name), tm.get(name));
            let chosen = if o == t {
                o.copied()
            } else if o == b {
                taken.push(name.clone());
                t.copied()
            } else if t == b {
                o.copied()
            } else {
                conflicts.push(name.clone());
                let load = |h: Option<&BlobHash>| -> Result<Option<Vec<u8>>> {
                    match h {
                        None => Ok(None),
                        Some(&h) => Ok(Some(get_blob(&tx, h)?.ok_or(StoreError::MissingBlob(h))?)),
                    }
                };
                let (bb, ob, tb) = (load(b)?, load(o)?, load(t)?);
                match resolver(name, bb.as_deref(), ob.as_deref(), tb.as_deref()) {
                    Some(data) => Some(put_blob(&tx, &data)?),
                    None => None,
                }
            };
            if let Some(h) = chosen {
                result.insert(name.clone(), h);
            }
        }
        let description = format!("Merge \"{from_name}\" into \"{into_name}\"");
        let id = insert_microversion(
            &tx,
            NewMicroversion {
                branch: into,
                parents: &[ours_head, theirs_head],
                manifest: &result,
                author: &self.author,
                description: &description,
            },
        )?;
        tx.commit()?;
        self.cache_manifest(id, Arc::new(result));
        Ok(MergeOutcome {
            microversion: id,
            base,
            taken_from_source: taken,
            conflicts,
            up_to_date: false,
        })
    }

    // -----------------------------------------------------------------------
    // Branches
    // -----------------------------------------------------------------------

    /// The default ("Main") branch.
    pub fn default_branch(&self) -> Result<BranchId> {
        Ok(self
            .conn
            .prepare_cached("SELECT id FROM branches WHERE is_default = 1 LIMIT 1")?
            .query_row([], |r| r.get(0))?)
    }

    /// Live branches, default first, then by creation.
    pub fn list_branches(&self) -> Result<Vec<BranchInfo>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {BRANCH_COLS} FROM branches WHERE deleted = 0
             ORDER BY is_default DESC, created, rowid"
        ))?;
        Ok(stmt
            .query_map([], branch_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Branch by id (also returns deleted branches).
    pub fn branch(&self, id: BranchId) -> Result<BranchInfo> {
        self.conn
            .prepare_cached(&format!("SELECT {BRANCH_COLS} FROM branches WHERE id = ?1"))?
            .query_row([id], branch_from_row)
            .optional()?
            .ok_or(StoreError::UnknownBranch(id))
    }

    /// Live branch by name.
    pub fn branch_by_name(&self, name: &str) -> Result<Option<BranchInfo>> {
        Ok(self
            .conn
            .prepare_cached(&format!(
                "SELECT {BRANCH_COLS} FROM branches WHERE name = ?1 AND deleted = 0"
            ))?
            .query_row([name], branch_from_row)
            .optional()?)
    }

    /// Creates a branch whose head is `from` (no new microversion).
    pub fn create_branch(&mut self, from: MicroversionId, name: &str) -> Result<BranchId> {
        self.writable()?;
        let name = check_name(name)?;
        self.microversion(from)?;
        if self.branch_by_name(name)?.is_some() {
            return Err(StoreError::BranchNameTaken(name.to_string()));
        }
        let id = BranchId::generate();
        self.conn.execute(
            "INSERT INTO branches (id, name, head, created, created_from, is_default)
             VALUES (?1, ?2, ?3, ?4, ?3, 0)",
            params![id, name, from, now_ms()],
        )?;
        Ok(id)
    }

    pub fn rename_branch(&mut self, id: BranchId, name: &str) -> Result<()> {
        self.writable()?;
        let name = check_name(name)?;
        self.head(id)?;
        if let Some(other) = self.branch_by_name(name)?
            && other.id != id
        {
            return Err(StoreError::BranchNameTaken(name.to_string()));
        }
        self.conn.execute(
            "UPDATE branches SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        Ok(())
    }

    /// Deletes a branch. Its microversions stay in the graph. The default
    /// branch cannot be deleted.
    pub fn delete_branch(&mut self, id: BranchId) -> Result<()> {
        self.writable()?;
        let b = self.branch(id)?;
        if b.deleted {
            return Err(StoreError::UnknownBranch(id));
        }
        if b.is_default {
            return Err(StoreError::CannotDeleteDefaultBranch);
        }
        self.conn
            .execute("UPDATE branches SET deleted = 1 WHERE id = ?1", [id])?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Versions
    // -----------------------------------------------------------------------

    /// Names an immutable version at `mv`.
    pub fn create_version(
        &mut self,
        mv: MicroversionId,
        name: &str,
        description: &str,
    ) -> Result<VersionId> {
        self.writable()?;
        let name = check_name(name)?;
        self.microversion(mv)?;
        let id = VersionId::generate();
        self.conn.execute(
            "INSERT INTO versions (id, name, description, microversion, created, author)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, name, description, mv, now_ms(), self.author],
        )?;
        Ok(id)
    }

    /// All versions in creation order.
    pub fn list_versions(&self) -> Result<Vec<VersionInfo>> {
        let mut stmt = self
            .conn
            .prepare_cached(&format!("SELECT {VERSION_COLS} FROM versions ORDER BY seq"))?;
        Ok(stmt
            .query_map([], version_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn version(&self, id: VersionId) -> Result<VersionInfo> {
        self.conn
            .prepare_cached(&format!(
                "SELECT {VERSION_COLS} FROM versions WHERE id = ?1"
            ))?
            .query_row([id], version_from_row)
            .optional()?
            .ok_or(StoreError::UnknownVersion(id))
    }

    // -----------------------------------------------------------------------
    // Metadata
    // -----------------------------------------------------------------------

    /// Raw metadata value.
    pub fn get_meta(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .conn
            .prepare_cached("SELECT value FROM meta WHERE key = ?1")?
            .query_row([key], |r| r.get(0))
            .optional()?)
    }

    /// Sets (or with `None`, removes) a raw metadata value. Metadata is not
    /// versioned.
    pub fn set_meta(&mut self, key: &str, value: Option<&[u8]>) -> Result<()> {
        self.writable()?;
        match value {
            Some(v) => self
                .conn
                .prepare_cached("INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)")?
                .execute(params![key, v])?,
            None => self
                .conn
                .prepare_cached("DELETE FROM meta WHERE key = ?1")?
                .execute([key])?,
        };
        Ok(())
    }

    fn meta_string(&self, key: &str) -> Result<String> {
        Ok(self
            .get_meta(key)?
            .map(|v| String::from_utf8_lossy(&v).into_owned())
            .unwrap_or_default())
    }

    /// Document metadata.
    pub fn meta(&self) -> Result<DocumentMeta> {
        Ok(DocumentMeta {
            name: self.meta_string("name")?,
            description: self.meta_string("description")?,
            default_units: self.meta_string("default_units")?,
            created: self.meta_string("created")?.parse().unwrap_or(0),
            modified: self.meta_string("modified")?.parse().unwrap_or(0),
            thumbnail: self.get_meta("thumbnail")?,
        })
    }

    pub fn set_name(&mut self, name: &str) -> Result<()> {
        self.set_meta("name", Some(name.as_bytes()))
    }

    pub fn set_description(&mut self, description: &str) -> Result<()> {
        self.set_meta("description", Some(description.as_bytes()))
    }

    pub fn set_default_units(&mut self, units: &str) -> Result<()> {
        self.set_meta("default_units", Some(units.as_bytes()))
    }

    /// Sets or clears the PNG thumbnail.
    pub fn set_thumbnail(&mut self, png: Option<&[u8]>) -> Result<()> {
        self.set_meta("thumbnail", png)
    }

    /// Storage statistics.
    pub fn stats(&self) -> Result<StoreStats> {
        let microversions: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM microversions", [], |r| r.get(0))?;
        let (blobs, raw, stored): (i64, i64, i64) = self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(size), 0), COALESCE(SUM(length(data)), 0) FROM blobs",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        Ok(StoreStats {
            microversions: microversions as u64,
            blobs: blobs as u64,
            raw_bytes: raw as u64,
            stored_bytes: stored as u64,
        })
    }
}

fn load_manifest_cached(
    conn: &Connection,
    cache: &RefCell<HashMap<MicroversionId, Arc<Manifest>>>,
    mv: MicroversionId,
) -> Result<Arc<Manifest>> {
    if let Some(m) = cache.borrow().get(&mv) {
        return Ok(m.clone());
    }
    let hash: BlobHash = conn
        .prepare_cached("SELECT manifest FROM microversions WHERE id = ?1")?
        .query_row([mv], |r| r.get(0))
        .optional()?
        .ok_or(StoreError::UnknownMicroversion(mv))?;
    let bytes = get_blob(conn, hash)?.ok_or(StoreError::MissingBlob(hash))?;
    Ok(Arc::new(manifest::decode(&bytes)?))
}
