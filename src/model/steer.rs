//! Steer: intent-based actions that mutate collaborative state.

use serde::{Deserialize, Serialize};

/// Intent-based actions that mutate collaborative state.
///
/// Each variant is a steer subcommand — a deterministic flow
/// with a known shape. This enum grows as helm learns new capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Steer {
    /// Comment on an issue, PR, or inline review thread.
    Comment {
        /// Issue or PR number.
        number: u64,

        /// Comment body.
        body: String,

        /// Where the comment lands.
        target: CommentTarget,
    },

    /// Create an issue.
    CreateIssue,

    /// Update issue title or body.
    EditIssue,

    /// Close an issue.
    CloseIssue,

    /// Create a pull request.
    CreatePullRequest,

    /// Update PR title or body.
    EditPullRequest,

    /// Close a PR without merging.
    ClosePullRequest,

    /// Request reviewers on a PR.
    RequestReview,

    /// Merge a PR.
    MergePullRequest,
}

/// Where a comment lands — routes to the correct `gh` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CommentTarget {
    /// A top-level comment on an issue.
    Issue,

    /// A top-level comment on a pull request.
    PullRequest,

    /// A reply to an inline code review comment.
    ReviewFeedback {
        /// The comment being replied to.
        comment_id: u64,
    },
}
