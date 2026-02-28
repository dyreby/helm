//! Payload: what came back from an observation.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// What came back from an observation.
///
/// Stored as a content-addressed artifact in the voyage database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Payload {
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
    GitHubPullRequest(Box<PullRequestPayload>),

    /// Results from observing a GitHub issue.
    ///
    /// Boxed to keep variant sizes balanced.
    GitHubIssue(Box<IssuePayload>),

    /// Results from observing a GitHub repository.
    ///
    /// Boxed to keep variant sizes balanced.
    GitHubRepository(Box<RepositoryPayload>),

    /// Large payload stored in the hold, referenced by content hash.
    Hold { hash: String },
}

/// What was seen when observing a GitHub pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestPayload {
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
pub struct IssuePayload {
    /// Issue metadata.
    pub summary: Option<GitHubSummary>,

    /// Issue comments.
    pub comments: Vec<GitHubComment>,
}

/// What was seen when observing a GitHub repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryPayload {
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

    /// PR-specific: the head branch name.
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

/// A directory listing: what's at a given path.
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

/// The contents of a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileContents {
    pub path: PathBuf,
    pub content: FileContent,
}

/// What was found when reading a file.
///
/// No file-type field — the extension is in the path and the consumer
/// knows what to do with the content. Adding a kind enum would duplicate
/// derivable information and create a maintenance surface for every new
/// file type encountered.
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
