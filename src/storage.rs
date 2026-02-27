//! Local persistence for voyages, logbooks, and the slate.
//!
//! Each voyage lives in its own `SQLite` file under the storage root:
//!
//! ```text
//! <root>/<uuid>.sqlite
//! ```
//!
//! See ADR 004 for the design rationale and schema.

use std::{fs, io, path::PathBuf};

use rusqlite::Connection;
use uuid::Uuid;

mod logbook;
mod slate;
mod voyage;

/// Errors that can occur during storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("voyage not found: {0}")]
    VoyageNotFound(Uuid),

    #[error("voyage already exists: {0}")]
    VoyageAlreadyExists(Uuid),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("corrupt data: {0}")]
    Corrupt(String),
}

pub type Result<T> = core::result::Result<T, StorageError>;

/// Local SQLite-based storage for voyages and logbooks.
///
/// Each voyage is stored in a separate `<id>.sqlite` file under `root`.
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    /// Creates a new storage instance rooted at the given directory.
    ///
    /// The directory is created if it doesn't exist.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Returns the default storage root: `~/.helm/voyages/`.
    pub fn default_root() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".helm").join("voyages"))
    }

    fn voyage_db_path(&self, id: Uuid) -> PathBuf {
        self.root.join(format!("{id}.sqlite"))
    }

    /// Opens an existing voyage database.
    ///
    /// Returns `VoyageNotFound` if the file does not exist.
    fn open_db(&self, id: Uuid) -> Result<Connection> {
        let path = self.voyage_db_path(id);
        if !path.exists() {
            return Err(StorageError::VoyageNotFound(id));
        }
        let conn = Connection::open(&path)?;
        configure_conn(&conn)?;
        Ok(conn)
    }

    /// Creates and initializes a new voyage database.
    ///
    /// Returns `VoyageAlreadyExists` if the file already exists.
    fn create_db(&self, id: Uuid) -> Result<Connection> {
        let path = self.voyage_db_path(id);
        if path.exists() {
            return Err(StorageError::VoyageAlreadyExists(id));
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch(SCHEMA)?;
        configure_conn(&conn)?;
        Ok(conn)
    }
}

/// Applies per-connection pragmas.
fn configure_conn(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(())
}

/// Database schema. Applied once on voyage creation.
///
/// `PRAGMA user_version = 1` marks the schema version.
/// Migrations will check this value and increment on upgrade.
const SCHEMA: &str = "
PRAGMA user_version = 1;

CREATE TABLE voyage (
    id           TEXT PRIMARY KEY,
    intent       TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    status       TEXT NOT NULL CHECK(status IN ('active', 'ended')),
    ended_at     TEXT,
    ended_status TEXT
);

CREATE TABLE blobs (
    hash TEXT PRIMARY KEY,
    data BLOB NOT NULL
);

CREATE TABLE slate (
    target      TEXT PRIMARY KEY,
    blob_hash   TEXT NOT NULL REFERENCES blobs(hash),
    observed_at TEXT NOT NULL
);

CREATE TABLE logbook (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    recorded_at TEXT NOT NULL,
    identity    TEXT NOT NULL,
    action      TEXT NOT NULL,
    summary     TEXT NOT NULL
);

CREATE TABLE bearing_observations (
    logbook_id  INTEGER NOT NULL REFERENCES logbook(id),
    target      TEXT NOT NULL,
    blob_hash   TEXT NOT NULL REFERENCES blobs(hash),
    observed_at TEXT NOT NULL
);
";
