//! Action types: records of things done during a voyage.
//!
//! An action is a single, immutable record of something that changed the world.
//! The logbook records what happened — not what was planned, attempted, or failed.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A timestamped, attributed record of an action taken during a voyage.
///
/// The logbook records one action record per entry.
/// Details beyond what's captured here live on the platform (GitHub, git)
/// and can be observed through bearings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRecord {
    /// Unique identifier.
    pub id: Uuid,

    /// Who performed this action (e.g. `john-agent`, `dyreby`).
    pub identity: String,

    /// What was done.
    pub action: Action,

    /// When the action was performed.
    pub performed_at: Timestamp,
}

/// A single discrete action taken during a voyage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Action {
    /// Pushed commits to a remote branch.
    Pushed {
        /// The branch that was pushed.
        branch: String,

        /// The commit SHA after pushing.
        sha: String,
    },

    /// Created a pull request.
    CreatedPullRequest {
        /// The pull request number.
        number: u64,
    },

    /// Merged a pull request.
    MergedPullRequest {
        /// The pull request number.
        number: u64,
    },

    /// Commented on a pull request.
    CommentedOnPullRequest {
        /// The pull request number.
        number: u64,
    },

    /// Replied to a review comment on a pull request.
    RepliedToReviewComment {
        /// The pull request number.
        number: u64,
    },

    /// Created an issue.
    CreatedIssue {
        /// The issue number.
        number: u64,
    },

    /// Closed an issue.
    ClosedIssue {
        /// The issue number.
        number: u64,
    },

    /// Commented on an issue.
    CommentedOnIssue {
        /// The issue number.
        number: u64,
    },
}
