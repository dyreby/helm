//! Steer: intent-based actions that mutate collaborative state.

use serde::{Deserialize, Serialize};

/// Intent-based actions that mutate collaborative state.
///
/// Each variant is a steer subcommand — a deterministic flow
/// with a known shape. This enum grows as helm learns new capabilities.
///
/// Variant fields are defined when each subcommand is built.
// TODO: remove once steer (#100) is wired to the CLI.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Steer {
    /// Comment on an issue or PR.
    Comment,

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

    /// Reply to an inline code review comment on a PR.
    ReplyInline,

    /// Merge a PR.
    MergePullRequest,
}
