//! Local persistence for voyages, logbooks, and the slate.
//!
//! Each voyage is stored in a single `SQLite` file:
//!
//! ```text
//! <root>/<uuid>.sqlite
//! ```
//!
//! The schema is initialised on `create_voyage` and versioned via
//! `PRAGMA user_version`. Connections always enable foreign key enforcement.

use std::{fmt::Write as _, io, path::PathBuf};

use rusqlite::Connection;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::model::Payload;

mod logbook;
mod slate;
mod voyage;

/// DDL run once when a voyage database is created.
///
/// Sets the schema version and creates all tables.
/// `PRAGMA foreign_keys = ON` is set per-connection in `open_voyage`,
/// not here — it is not persisted.
const SCHEMA_DDL: &str = "
PRAGMA user_version = 1;

CREATE TABLE voyage (
    id           TEXT PRIMARY KEY,
    intent       TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    status       TEXT NOT NULL CHECK(status IN ('active', 'ended')),
    ended_at     TEXT,
    ended_status TEXT
);

CREATE TABLE artifacts (
    hash   TEXT PRIMARY KEY,
    data   BLOB NOT NULL,
    status TEXT NOT NULL DEFAULT 'stowed' CHECK(status IN ('stowed', 'reduced', 'jettisoned'))
);

CREATE TABLE artifact_derivations (
    source_hash  TEXT NOT NULL REFERENCES artifacts(hash),
    derived_hash TEXT NOT NULL REFERENCES artifacts(hash),
    method       TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    PRIMARY KEY (source_hash, derived_hash)
);

CREATE TABLE slate (
    target        TEXT PRIMARY KEY,
    artifact_hash TEXT NOT NULL REFERENCES artifacts(hash),
    observed_at   TEXT NOT NULL
);

CREATE TABLE logbook (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    recorded_at TEXT NOT NULL,
    identity    TEXT NOT NULL,
    action      TEXT NOT NULL,
    summary     TEXT NOT NULL,
    role        TEXT NOT NULL,
    method      TEXT NOT NULL
);

CREATE TABLE bearing_observations (
    logbook_id    INTEGER NOT NULL REFERENCES logbook(id),
    target        TEXT NOT NULL,
    artifact_hash TEXT NOT NULL REFERENCES artifacts(hash),
    observed_at   TEXT NOT NULL
);
";

/// Errors that can occur during storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("voyage not found: {0}")]
    VoyageNotFound(Uuid),

    #[error("voyage already exists: {0}")]
    VoyageAlreadyExists(Uuid),

    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("compression error: {0}")]
    Compression(String),

    #[error("time parse error: {0}")]
    TimeParse(String),
}

pub type Result<T> = core::result::Result<T, StorageError>;

/// SQLite-backed storage for voyages and logbooks.
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    /// Creates a new storage instance rooted at the given directory.
    ///
    /// The directory is created if it doesn't exist.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Returns the default storage root: `~/.helm/voyages/`.
    pub fn default_root() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".helm").join("voyages"))
    }

    fn voyage_path(&self, id: Uuid) -> PathBuf {
        self.root.join(format!("{id}.sqlite"))
    }

    /// Opens a connection to an existing voyage database.
    ///
    /// Returns [`StorageError::VoyageNotFound`] if the file does not exist.
    /// Enables foreign key enforcement on every connection.
    fn open_voyage(&self, id: Uuid) -> Result<Connection> {
        let path = self.voyage_path(id);
        if !path.exists() {
            return Err(StorageError::VoyageNotFound(id));
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(conn)
    }
}

/// Compress `data` with zstd at level 3.
fn compress(data: &[u8]) -> Result<Vec<u8>> {
    zstd::encode_all(data, 3).map_err(|e| StorageError::Compression(e.to_string()))
}

/// Decompress zstd-compressed `data`.
fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    zstd::decode_all(data).map_err(|e| StorageError::Compression(e.to_string()))
}

/// Compute a hex-encoded SHA-256 hash of `data`.
fn sha256_hex(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    hash.iter().fold(String::with_capacity(64), |mut s, b| {
        write!(s, "{b:02x}").unwrap();
        s
    })
}

/// Store a payload artifact in the database.
///
/// The payload is serialised to JSON, hashed (SHA-256 of the uncompressed JSON),
/// and stored compressed. If the artifact already exists, this is a no-op.
/// Returns the artifact hash.
fn store_artifact(conn: &Connection, payload: &Payload) -> Result<String> {
    let json = serde_json::to_string(payload)?;
    let hash = sha256_hex(json.as_bytes());
    let compressed = compress(json.as_bytes())?;
    conn.execute(
        "INSERT OR IGNORE INTO artifacts (hash, data, status) VALUES (?1, ?2, 'stowed')",
        rusqlite::params![hash, compressed],
    )?;
    Ok(hash)
}

/// Load and decompress a payload artifact from the database by hash.
fn load_artifact(conn: &Connection, hash: &str) -> Result<Payload> {
    let compressed: Vec<u8> = conn.query_row(
        "SELECT data FROM artifacts WHERE hash = ?1",
        rusqlite::params![hash],
        |row| row.get(0),
    )?;
    let json = decompress(&compressed)?;
    Ok(serde_json::from_slice(&json)?)
}
