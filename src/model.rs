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
pub use observe::{Observe, PullRequestFocus};
pub use payload::{
    CheckRun, DirectoryEntry, DirectoryListing, FileContent, FileContents, GitHubComment,
    GitHubIssueSummary, GitHubPullRequestSummary, GitHubSummary, IssuePayload, Payload,
    PullRequestPayload, RepositoryPayload, ReviewComment,
};
pub use steer::Steer;
pub use voyage::{Voyage, VoyageStatus};

/// A single entry in the logbook, serialized as one line of JSONL.
///
/// Tagged enum so each line is self-describing when read back.
/// Identity is recorded per entry — multiple agents or people can
/// steer the same voyage.
// TODO: remove once steer (#100) and log (#101) are wired to the CLI.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogbookEntry {
    pub bearing: Bearing,
    pub author: String,
    pub timestamp: Timestamp,
    pub kind: EntryKind,
}

/// What kind of logbook entry this is.
// TODO: remove once steer (#100) and log (#101) are wired to the CLI.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum EntryKind {
    /// A steering action: mutated collaborative state.
    Steer(Steer),

    /// A logged state: recorded without mutation.
    Log(String),
}
