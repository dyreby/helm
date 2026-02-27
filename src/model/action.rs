//! Action types: immutable records of operations Helm performed.
//!
//! One action = one kind. Each is a distinct moment in time,
//! recorded in the logbook as it happened.
//! Failed operations are not recorded — the logbook captures
//! what happened, not what was attempted.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single, immutable record of an operation Helm performed.
///
/// Carries the minimum data needed to identify what happened.
/// Content (PR bodies, comment text) lives on GitHub and can be
/// observed through bearings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub id: Uuid,

    /// Which identity performed this action (e.g. "dyreby", "john-agent").
    pub identity: String,

    /// What was done.
    pub kind: ActionKind,

    /// When the action was performed.
    pub performed_at: Timestamp,
}

/// What was done. Grouped by target, not by verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ActionKind {
    /// Committed changes locally.
    Commit { sha: String },

    /// Pushed commits to a branch.
    Push { branch: String, sha: String },

    /// An action on a pull request.
    PullRequest {
        number: u64,
        action: PullRequestAction,
    },

    /// An action on an issue.
    Issue { number: u64, action: IssueAction },
}

/// Things you can do to a pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PullRequestAction {
    /// Created a new pull request.
    Create,

    /// Merged the pull request.
    Merge,

    /// Left a comment on the pull request.
    Comment,

    /// Replied to an inline review comment.
    /// Distinct from `Comment` because "I addressed feedback"
    /// is a meaningful signal when reading the logbook.
    Reply,

    /// Requested review from one or more users.
    RequestedReview { reviewers: Vec<String> },
}

/// Things you can do to an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum IssueAction {
    /// Created a new issue.
    Create,

    /// Closed the issue.
    Close,

    /// Left a comment on the issue.
    Comment,
}
