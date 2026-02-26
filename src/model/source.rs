//! Source kinds: domains of observable reality.

use std::path::PathBuf;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A self-contained observation: what you pointed the spyglass at and what you saw.
///
/// Observations are the building blocks of bearings.
/// Take as many as you want; only the ones you choose to record become part of a bearing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Unique identifier for this observation.
    pub id: Uuid,

    /// What was observed.
    pub subject: Subject,

    /// What was seen.
    pub sighting: Sighting,

    /// When the observation was made.
    pub observed_at: Timestamp,
}

/// The subject of an observation: what you pointed the spyglass at.
///
/// Each variant describes a domain-specific scope.
/// Adding a new source kind means adding a variant here
/// and implementing its observation logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Subject {
    /// Filesystem structure and content.
    ///
    /// Scope: directories to survey (list contents with metadata).
    /// Focus: specific files to inspect (read full contents).
    Files {
        scope: Vec<PathBuf>,
        focus: Vec<PathBuf>,
    },

    /// A Rust project rooted at a directory.
    ///
    /// Walks the project tree, respects `.gitignore`, skips `target/`.
    /// Survey: full directory tree with metadata.
    /// Focus: all source files (everything that isn't binary or ignored).
    RustProject { root: PathBuf },
}

/// What was seen when observing a subject.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Sighting {
    /// Results from observing a filesystem subject.
    Files {
        /// Directory listings from surveyed paths.
        survey: Vec<DirectorySurvey>,

        /// File contents from focused paths.
        inspections: Vec<FileInspection>,
    },
}

/// A directory listing produced by surveying a path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectorySurvey {
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

/// The contents of a file produced by inspecting a path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInspection {
    pub path: PathBuf,
    pub content: FileContent,
}

/// What was found when inspecting a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileContent {
    /// UTF-8 text content.
    Text(String),

    /// File was not valid UTF-8. Size recorded for reference.
    Binary { size_bytes: u64 },

    /// File could not be read.
    Error(String),
}
