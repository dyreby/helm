//! Local persistence for voyages, logbooks, and the slate.
//!
//! Each voyage lives in its own directory under the storage root:
//!
//! ```text
//! <root>/<uuid>/
//!   voyage.json    # Voyage metadata
//!   logbook.jsonl  # Append-only logbook entries (bearings + steer/log records)
//!   slate.jsonl    # Observations since last steer/log, cleared on seal
//! ```

use std::{fs, io, path::PathBuf};

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
}

pub type Result<T> = core::result::Result<T, StorageError>;

/// Local file-based storage for voyages and logbooks.
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

    fn voyage_dir(&self, id: Uuid) -> PathBuf {
        self.root.join(id.to_string())
    }
}
