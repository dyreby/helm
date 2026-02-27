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
    /// Read specific files.
    ///
    /// The simplest filesystem mark.
    /// Returns the content of each file (text, binary, or error).
    FileContents { paths: Vec<PathBuf> },

    /// Recursive directory walk with filtering.
    ///
    /// Respects `.gitignore` by default.
    /// `skip` names directories to skip at any depth (e.g. "target", "`node_modules`").
    /// `max_depth` limits recursion depth (`None` = unlimited).
    DirectoryTree {
        root: PathBuf,
        skip: Vec<String>,
        max_depth: Option<u32>,
    },

    /// A Rust project rooted at a directory.
    ///
    /// Walks the project tree (respects `.gitignore`, skips `target/`).
    /// Lists the full directory tree with metadata.
    /// Reads documentation files only — README, VISION, CONTRIBUTING,
    /// agent instructions, etc. Source code is not read.
    ///
    /// This is an orientation mark. Use `FileContents` with targeted paths
    /// to read specific source files on subsequent observations.
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
///
/// Each variant corresponds to a Mark variant.
/// The sighting is the heavy payload — full file contents, directory trees, API responses.
/// Stored separately from the bearing and prunable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Sighting {
    /// Contents of specific files.
    FileContents { contents: Vec<FileContents> },

    /// Recursive directory tree.
    DirectoryTree { listings: Vec<DirectoryListing> },

    /// Rust project structure and documentation.
    RustProject {
        listings: Vec<DirectoryListing>,
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

/// A directory listing: what's at this path.
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
///
/// No file-type field (Rust, Markdown, TOML, etc.) — the file extension
/// is in the `path` and the consumer knows what to do with the content.
/// Adding a kind enum would duplicate derivable information and create
/// a maintenance surface for every new file type encountered.
///
/// If structured parsing is ever needed (Rust AST, Markdown sections),
/// that's a different sighting type, not a field here.
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
