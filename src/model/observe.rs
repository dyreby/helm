//! Observe: the central enum for what helm can look at.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The central enum.
///
/// Each variant describes something helm can look at.
/// Adding a new observation type means adding a variant here
/// and implementing its observation logic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    ///
    /// Always fetches metadata and comments.
    GitHubIssue { number: u64 },

    /// A GitHub pull request.
    ///
    /// Always fetches everything: metadata, comments, diff, files, checks, and inline reviews.
    GitHubPullRequest { number: u64 },

    /// A GitHub repository.
    ///
    /// Always fetches open issues and pull requests.
    GitHubRepository,
}


