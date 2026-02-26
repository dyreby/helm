//! Action types: immutable records of something that changed the world.
//!
//! One action = one act. Each is a distinct moment in time,
//! recorded in the logbook as it happened.
//! Failed operations are not recorded — the logbook captures
//! what happened, not what was attempted.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single, immutable record of something that changed the world.
///
/// Actions carry the minimum data needed to identify what happened.
/// Content (PR bodies, comment text) lives on GitHub and can be
/// observed through bearings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub id: Uuid,

    /// Which identity performed this action (e.g. "dyreby", "john-agent").
    pub identity: String,

    /// What was done.
    pub act: Act,

    /// When the action was performed.
    pub performed_at: Timestamp,
}

/// What was done. Grouped by target, not by verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Act {
    /// Committed changes locally.
    Committed { sha: String },

    /// Pushed commits to a branch.
    Pushed { branch: String, sha: String },

    /// An action on a pull request.
    PullRequest { number: u64, act: PullRequestAct },

    /// An action on an issue.
    Issue { number: u64, act: IssueAct },
}

/// Things you can do to a pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PullRequestAct {
    /// Created a new pull request.
    Created,

    /// Merged the pull request.
    Merged,

    /// Left a comment on the pull request.
    Commented,

    /// Replied to an inline review comment.
    /// Distinct from `Commented` because "I addressed feedback"
    /// is a meaningful signal when reading the logbook.
    Replied,

    /// Requested review from one or more users.
    RequestedReview { reviewers: Vec<String> },
}

/// Things you can do to an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum IssueAct {
    /// Created a new issue.
    Created,

    /// Closed the issue.
    Closed,

    /// Left a comment on the issue.
    Commented,
}
