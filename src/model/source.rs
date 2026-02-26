//! Source kinds: domains of observable reality.

use std::path::PathBuf;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// A self-contained observation: what you pointed the spyglass at and what you saw.
///
/// Observations are the building blocks of bearings.
/// Take as many as you want; only the ones you choose to record become part of a bearing.
/// Identified by position in the bearing's observation list, not by ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// What was observed.
    pub mark: Mark,

    /// What was seen.
    pub sighting: Sighting,

    /// When the observation was made.
    pub observed_at: Timestamp,
}

/// The mark of an observation: what you pointed the spyglass at.
///
/// Each variant describes a domain-specific scope.
/// Adding a new source kind means adding a variant here
/// and implementing its observation logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Mark {
    /// Filesystem structure and content.
    ///
    /// Lists directories and reads files exactly as specified.
    /// No recursion, filtering, or domain awareness.
    /// Domain-specific marks like `RustProject` add that intelligence.
    ///
    /// - `list`: directories to list immediate contents of.
    /// - `read`: files to read.
    Files {
        list: Vec<PathBuf>,
        read: Vec<PathBuf>,
    },

    /// A Rust project rooted at a directory.
    ///
    /// Walks the project tree, respects `.gitignore`, skips `target/`.
    /// Lists the full directory tree with metadata.
    /// Reads documentation files (everything else is left for targeted `Files` queries).
    RustProject { root: PathBuf },
}

/// What was seen when observing a mark.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Sighting {
    /// Results from observing a filesystem mark.
    Files {
        /// Directory listings from listed paths.
        listings: Vec<DirectoryListing>,

        /// File contents from read paths.
        contents: Vec<FileContents>,
    },
}

/// A directory listing produced by listing a path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryListing {
    pub path: PathBuf,
    pub entries: Vec<DirectoryEntry>,
}

/// A single entry in a directory listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: Option<u64>,
}

/// The contents of a file produced by reading a path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContents {
    pub path: PathBuf,
    pub content: FileContent,
}

/// What was found when reading a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FileContent {
    /// UTF-8 text content.
    Text { content: String },

    /// File was not valid UTF-8. Size recorded for reference.
    Binary { size_bytes: u64 },

    /// File could not be read.
    Error { message: String },
}
