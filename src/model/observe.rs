//! Observe: the central enum for what helm can look at.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The central enum.
///
/// Each variant describes something helm can look at.
/// Adding a new observation type means adding a variant here
/// and implementing its observation logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Observe {
    /// Read specific files.
    FileContents { paths: Vec<PathBuf> },

    /// Recursive directory walk with filtering.
    ///
    /// Respects `.gitignore` by default.
    /// `skip` names directories to skip at any depth (e.g. `"target"`, `"node_modules"`).
    /// `max_depth` limits recursion depth (`None` = unlimited).
    DirectoryTree {
        root: PathBuf,
        skip: Vec<String>,
        max_depth: Option<u32>,
    },

    /// A Rust project rooted at a directory.
    ///
    /// Walks the project tree, lists structure, reads documentation only.
    /// An orientation observation — use `FileContents` for targeted reads.
    RustProject { root: PathBuf },

    /// A GitHub issue.
    GitHubIssue { number: u64, focus: Vec<IssueFocus> },

    /// A GitHub pull request.
    GitHubPullRequest {
        number: u64,
        focus: Vec<PullRequestFocus>,
    },

    /// A GitHub repository.
    GitHubRepository { focus: Vec<RepositoryFocus> },
}

/// What to observe about an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueFocus {
    /// Issue metadata: title, state, author, labels, assignees.
    Summary,

    /// Issue comments.
    Comments,
}

/// What to observe about a pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PullRequestFocus {
    /// PR metadata: title, state, author, labels, assignees.
    Summary,

    /// Changed file paths.
    Files,

    /// CI check status.
    Checks,

    /// Full diff.
    Diff,

    /// Top-level PR comments.
    Comments,

    /// Inline review comments with threads.
    Reviews,
}

/// What to observe about a repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RepositoryFocus {
    /// Open issues.
    Issues,

    /// Open pull requests.
    PullRequests,
}
