//! Source kinds: domains of observable reality.

use std::path::PathBuf;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// A self-contained observation: what you pointed the spyglass at and what you saw.
///
/// Observations are the building blocks of bearings.
/// Take as many as you want; only the ones you choose to record become part of a bearing.
/// Identified by position in the bearing's observation list, not by ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    /// What was observed.
    pub mark: Mark,

    /// What was seen.
    pub sighting: Sighting,

    /// When the observation was made.
    pub observed_at: Timestamp,
}

/// The mark of an observation: what you pointed the spyglass at.
///
/// Each variant describes a domain-specific scope.
/// Adding a new source kind means adding a variant here
/// and implementing its observation logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Mark {
    /// Filesystem structure and content.
    ///
    /// Lists directories and reads files exactly as specified.
    /// No recursion, filtering, or domain awareness.
    /// Domain-specific marks like `RustProject` add that intelligence.
    ///
    /// - `list`: directories to list immediate contents of.
    /// - `read`: files to read.
    Files {
        list: Vec<PathBuf>,
        read: Vec<PathBuf>,
    },

    /// A Rust project rooted at a directory.
    ///
    /// Walks the project tree, respects `.gitignore`, skips `target/`.
    /// Lists the full directory tree with metadata.
    /// Reads documentation files (everything else is left for targeted `Files` queries).
    RustProject { root: PathBuf },

    /// A GitHub pull request.
    ///
    /// Focus controls depth: summary for metadata,
    /// diff/comments/reviews/checks/files for details.
    /// Defaults to summary when no focus is specified.
    GitHubPullRequest {
        number: u64,
        focus: Vec<PullRequestFocus>,
    },

    /// A GitHub issue.
    ///
    /// Focus controls depth: summary for metadata, comments for discussion.
    /// Defaults to summary when no focus is specified.
    GitHubIssue { number: u64, focus: Vec<IssueFocus> },

    /// A GitHub repository.
    ///
    /// Lists open issues, pull requests, or both.
    GitHubRepository { focus: Vec<RepositoryFocus> },
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

/// What to observe about an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueFocus {
    /// Issue metadata: title, state, author, labels, assignees.
    Summary,

    /// Issue comments.
    Comments,
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

/// What was seen when observing a mark.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Sighting {
    /// Results from observing a filesystem mark.
    Files {
        /// Directory listings from listed paths.
        listings: Vec<DirectoryListing>,

        /// File contents from read paths.
        contents: Vec<FileContents>,
    },

    /// Results from observing a GitHub pull request.
    ///
    /// Boxed to keep variant sizes balanced.
    GitHubPullRequest(Box<PullRequestSighting>),

    /// Results from observing a GitHub issue.
    GitHubIssue(Box<IssueSighting>),

    /// Results from observing a GitHub repository.
    GitHubRepository(Box<RepositorySighting>),
}

/// What was seen when observing a GitHub pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestSighting {
    /// PR metadata.
    pub summary: Option<GitHubSummary>,

    /// Changed file paths.
    pub files: Vec<String>,

    /// CI check runs.
    pub checks: Vec<CheckRun>,

    /// Full diff text.
    pub diff: Option<String>,

    /// Top-level PR comments.
    pub comments: Vec<GitHubComment>,

    /// Inline review comments with threads.
    pub reviews: Vec<ReviewComment>,
}

/// What was seen when observing a GitHub issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueSighting {
    /// Issue metadata.
    pub summary: Option<GitHubSummary>,

    /// Issue comments.
    pub comments: Vec<GitHubComment>,
}

/// What was seen when observing a GitHub repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySighting {
    /// Open issues.
    pub issues: Vec<GitHubIssueSummary>,

    /// Open pull requests.
    pub pull_requests: Vec<GitHubPullRequestSummary>,
}

/// Metadata for a PR or issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubSummary {
    pub title: String,
    pub number: u64,
    pub state: String,
    pub author: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    /// PR-specific: the branch name.
    pub head_branch: Option<String>,
    /// PR-specific: the base branch name.
    pub base_branch: Option<String>,
    /// Body text.
    pub body: Option<String>,
}

/// A CI check run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckRun {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}

/// A top-level comment on a PR or issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubComment {
    pub author: String,
    pub body: String,
    pub created_at: String,
}

/// An inline review comment on a PR.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewComment {
    pub id: u64,
    pub path: String,
    pub line: Option<u64>,
    pub author: String,
    pub body: String,
    pub created_at: String,
    /// ID of the comment this replies to, if it's a thread reply.
    pub in_reply_to_id: Option<u64>,
}

/// Summary of an issue in a repository listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubIssueSummary {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub labels: Vec<String>,
}

/// Summary of a pull request in a repository listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubPullRequestSummary {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub labels: Vec<String>,
    pub head_branch: String,
}

/// A directory listing produced by listing a path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListing {
    pub path: PathBuf,
    pub entries: Vec<DirectoryEntry>,
}

/// A single entry in a directory listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntry {
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: Option<u64>,
}

/// The contents of a file produced by reading a path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileContents {
    pub path: PathBuf,
    pub content: FileContent,
}

/// What was found when reading a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FileContent {
    /// UTF-8 text content.
    Text { content: String },

    /// File was not valid UTF-8. Size recorded for reference.
    Binary { size_bytes: u64 },

    /// File could not be read.
    Error { message: String },
}
