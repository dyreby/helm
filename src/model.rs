#![allow(dead_code)]

//! Core data model for Helm.
//!
//! These types represent the conceptual architecture from VISION.md:
//! voyages, bearings, plans, moments, positions, and actions.

use std::path::PathBuf;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Voyage
// ---------------------------------------------------------------------------

/// A unit of work with intent, logbook, and outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voyage {
    pub id: Uuid,
    pub kind: VoyageKind,
    pub intent: String,
    pub created_at: Timestamp,
    pub status: VoyageStatus,
}

/// The kind of voyage, which frames the first bearing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoyageKind {
    /// Unscoped, general-purpose voyage. No prescribed framing.
    OpenWaters,
}

/// Where a voyage stands in its lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoyageStatus {
    /// Work is in progress.
    Active,

    /// Paused, can be resumed. The world may have changed.
    Paused,

    /// Completed with an outcome.
    Completed,
}

// ---------------------------------------------------------------------------
// Logbook
// ---------------------------------------------------------------------------

/// A single entry in the logbook, serialized as one line of JSONL.
///
/// Tagged enum so each line is self-describing when read back.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LogbookEntry {
    /// A bearing was taken.
    Bearing(Bearing),

    /// An action was executed.
    ActionReport(ActionReport),
}

// ---------------------------------------------------------------------------
// Bearing
// ---------------------------------------------------------------------------

/// An immutable record of observation: what was planned, what was seen,
/// and what it means.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bearing {
    pub plan: BearingPlan,
    pub moment: Moment,
    pub position: Position,
    pub taken_at: Timestamp,
}

/// What to observe, described as scope and focus per source kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BearingPlan {
    pub sources: Vec<SourceQuery>,
}

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
}

// ---------------------------------------------------------------------------
// Moment
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Position
// ---------------------------------------------------------------------------

/// A short, plain-text statement of the world's state.
///
/// Tracks the accepted text and the history of attempts that were challenged
/// along the way. The challenge history captures alignment gaps in the
/// collaboration, not agent failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// The accepted position text.
    pub text: String,

    /// Prior attempts that were challenged before arriving at the accepted text.
    pub history: Vec<PositionAttempt>,
}

/// A single attempt at stating a position, possibly challenged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionAttempt {
    /// The position text that was proposed.
    pub text: String,

    /// Who produced this text.
    pub source: PositionSource,

    /// Feedback that caused this attempt to be rejected.
    /// Present on challenged attempts, absent on the final accepted one.
    pub challenged_with: Option<String>,
}

/// Who produced a position's text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PositionSource {
    /// Generated by the LLM.
    Agent,

    /// Written or edited by the user.
    User,
}

// ---------------------------------------------------------------------------
// Action
// ---------------------------------------------------------------------------

/// Intent to affect the world.
///
/// Describes what should happen, not how. Reviewed and approved before
/// execution through Helm's gate (editor for text, yes/no for irreversible
/// operations).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ActionPlan {
    /// Write content to files.
    WriteFiles { files: Vec<FileWrite> },
}

/// A file to be written as part of an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWrite {
    pub path: PathBuf,
    pub content: String,
}

/// The outcome of executing an action.
///
/// Always contains the plan that produced it. No orphaned intents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionReport {
    pub plan: ActionPlan,
    pub outcome: ActionOutcome,
    pub completed_at: Timestamp,
}

/// What happened when an action was executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionOutcome {
    /// Action completed successfully.
    Success,

    /// Action failed with an error message.
    Failed(String),

    /// Action was rejected at the approval gate.
    Rejected,
}
