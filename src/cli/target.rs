//! Observable targets — what helm can look at.
//!
//! `ObserveTarget` is the extension surface for new observation types.
//! Both `helm observe` and `helm slate erase` accept an `ObserveTarget`,
//! so adding a new observable type extends both commands for free.
//!
//! See ADR 002 for the rationale.

use std::path::PathBuf;

use clap::Subcommand;

/// What helm can observe.
///
/// Each variant is a subcommand accepted by `helm observe` and `helm slate erase`.
/// Adding a new observation type means adding a variant here — both commands extend automatically.
#[derive(Debug, Subcommand)]
pub enum ObserveTarget {
    /// Read specific files.
    FileContents {
        /// Files to read (full contents).
        #[arg(long)]
        read: Vec<PathBuf>,
    },

    /// Walk a directory tree recursively.
    ///
    /// Respects `.gitignore` by default.
    DirectoryTree {
        /// Root directory to walk.
        root: PathBuf,

        /// Directory names to skip at any depth (e.g. `"target"`, `"node_modules"`).
        #[arg(long)]
        skip: Vec<String>,

        /// Maximum recursion depth (unlimited if not specified).
        #[arg(long)]
        max_depth: Option<u32>,
    },

    /// Observe a Rust project: full directory tree and documentation.
    RustProject {
        /// Path to the project root.
        path: PathBuf,
    },

    /// Observe a GitHub pull request.
    ///
    /// Always fetches everything: metadata, comments, diff, files, checks, and inline reviews.
    #[command(name = "github-pr")]
    GitHubPullRequest {
        /// PR number.
        number: u64,
    },

    /// Observe a GitHub issue.
    ///
    /// Always fetches metadata and comments.
    #[command(name = "github-issue")]
    GitHubIssue {
        /// Issue number.
        number: u64,
    },

    /// Observe a GitHub repository.
    ///
    /// Always fetches open issues and pull requests.
    #[command(name = "github-repo")]
    GitHubRepository,
}


