//! Source kinds: domains of observable reality.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A domain-specific query describing what to survey and what to inspect.
///
/// Each variant owns its natural scope and focus types. Adding a new source
/// kind means adding a variant here and implementing its survey/inspect logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SourceQuery {
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

/// What was observed when a bearing was taken.
///
/// Contains the raw payloads from each source kind in the bearing plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Moment {
    pub observations: Vec<Observation>,
}

/// The result of executing a single source query.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Observation {
    /// Results from a Files source query.
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
