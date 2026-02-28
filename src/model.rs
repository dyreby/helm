//! Core data model for Helm.
//!
//! These types represent the conceptual architecture:
//! voyages, observations, bearings, steer actions, and logbook entries.

mod bearing;
mod observation;
mod observe;
mod payload;
mod steer;
mod voyage;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

pub use bearing::Bearing;
pub use observation::Observation;
pub use observe::Observe;
pub use payload::{
    CheckRun, DirectoryEntry, DirectoryListing, FileContent, FileContents, GitHubComment,
    GitHubIssueSummary, GitHubPullRequestSummary, GitHubSummary, IssuePayload, Payload,
    PullRequestPayload, RepositoryPayload, ReviewComment,
};
pub use steer::{CommentTarget, Steer};
pub use voyage::{Voyage, VoyageStatus};

/// A single entry in the logbook.
///
/// Identity is recorded per entry — multiple agents or people can steer the
/// same voyage. Role and method capture the cognitive framing and execution
/// engine used at the time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogbookEntry {
    /// The observations that informed this decision, plus summary.
    pub bearing: Bearing,

    /// Who acted.
    pub identity: String,

    /// What cognitive framing was adopted (e.g. `"reviewer"`, `"coder"`).
    pub role: String,

    /// How the thinking was done (e.g. `"claude-opus-4, thinking high"`, `"human"`).
    pub method: String,

    /// When this entry was recorded.
    pub recorded_at: Timestamp,

    /// What happened.
    pub kind: EntryKind,
}

/// What kind of logbook entry this is.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum EntryKind {
    /// A steering action: mutated collaborative state.
    Steer(Steer),

    /// A logged state: recorded without mutation.
    Log(String),
}
