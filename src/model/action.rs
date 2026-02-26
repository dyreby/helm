//! Action types: records of things done during a voyage.
//!
//! An action is a single, immutable record of something that changed the world.
//! The logbook records what happened — not what was planned, attempted, or failed.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single thing done during a voyage.
///
/// Each action is one discrete act — push, create a PR, comment on an issue.
/// The logbook records one action per entry.
/// Details beyond what's captured here live on the platform (GitHub, git)
/// and can be observed through bearings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Unique identifier.
    pub id: Uuid,

    /// Who performed this action (e.g. `john-agent`, `dyreby`).
    pub identity: String,

    /// What was done.
    pub act: Act,

    /// When the action was performed.
    pub performed_at: Timestamp,
}

/// What was done.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Act {
    /// Pushed commits to a remote branch.
    Pushed {
        /// The branch that was pushed.
        branch: String,

        /// The commit SHA after pushing.
        sha: String,
    },

    /// Did something to a pull request.
    PullRequest {
        /// The pull request number.
        number: u64,

        /// What was done to it.
        act: PullRequestAct,
    },

    /// Did something to an issue.
    Issue {
        /// The issue number.
        number: u64,

        /// What was done to it.
        act: IssueAct,
    },
}

/// An act performed on a pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PullRequestAct {
    /// Created the pull request.
    Created,

    /// Merged the pull request.
    Merged,

    /// Left a comment on the pull request.
    Commented,

    /// Replied to a review comment thread.
    Replied,
}

/// An act performed on an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum IssueAct {
    /// Created the issue.
    Created,

    /// Closed the issue.
    Closed,

    /// Left a comment on the issue.
    Commented,
}
