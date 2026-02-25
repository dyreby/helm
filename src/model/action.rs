//! Action types: intent to affect the world and the resulting outcome.

use std::path::PathBuf;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// Intent to affect the world.
///
/// Describes what should happen, not how. Reviewed and approved before
/// execution through Helm's gate (editor for text, yes/no for irreversible
/// operations).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ActionPlan {
    /// Create a new file or overwrite an existing one entirely.
    WriteFiles { files: Vec<FileWrite> },

    /// Make surgical edits to existing files. Finds exact text and replaces it,
    /// leaving the rest of the file untouched.
    EditFiles { edits: Vec<FileEdit> },
}

/// A file to be created or overwritten.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWrite {
    pub path: PathBuf,
    pub content: String,
}

/// A surgical edit to an existing file: find exact text, replace it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdit {
    pub path: PathBuf,
    pub old_text: String,
    pub new_text: String,
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
