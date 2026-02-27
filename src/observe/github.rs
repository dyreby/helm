//! GitHub source kind: PRs, issues, and repository listings.
//!
//! Fetches data via the `gh` CLI, authenticated using the voyage's identity.
//! Each focus item maps to one or more `gh` commands.

use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::model::{
    CheckRun, GitHubComment, GitHubIssueSummary, GitHubPullRequestSummary, GitHubSummary,
    IssueFocus, IssueSighting, PullRequestFocus, PullRequestSighting, RepositoryFocus,
    RepositorySighting, ReviewComment, Sighting,
};

/// Observe a pull request with the requested focus items.
///
/// Defaults to summary when no focus is specified.
pub fn observe_github_pull_request(
    number: u64,
    focus: &[PullRequestFocus],
    gh_config: &Path,
) -> Sighting {
    let focus = if focus.is_empty() {
        &[PullRequestFocus::Summary]
    } else {
        focus
    };

    let mut summary = None;
    let mut files = Vec::new();
    let mut checks = Vec::new();
    let mut diff = None;
    let mut comments = Vec::new();
    let mut reviews = Vec::new();

    for item in focus {
        match item {
            PullRequestFocus::Summary => {
                summary = fetch_pr_summary(number, gh_config);
            }
            PullRequestFocus::Files => {
                files = fetch_pr_files(number, gh_config);
            }
            PullRequestFocus::Checks => {
                checks = fetch_pr_checks(number, gh_config);
            }
            PullRequestFocus::Diff => {
                diff = fetch_pr_diff(number, gh_config);
            }
            PullRequestFocus::Comments => {
                comments = fetch_pr_comments(number, gh_config);
            }
            PullRequestFocus::Reviews => {
                reviews = fetch_pr_reviews(number, gh_config);
            }
        }
    }

    Sighting::GitHubPullRequest(Box::new(PullRequestSighting {
        summary,
        files,
        checks,
        diff,
        comments,
        reviews,
    }))
}

/// Observe an issue with the requested focus items.
///
/// Defaults to summary when no focus is specified.
pub fn observe_github_issue(number: u64, focus: &[IssueFocus], gh_config: &Path) -> Sighting {
    let focus = if focus.is_empty() {
        &[IssueFocus::Summary]
    } else {
        focus
    };

    let mut summary = None;
    let mut comments = Vec::new();

    for item in focus {
        match item {
            IssueFocus::Summary => {
                summary = fetch_issue_summary(number, gh_config);
            }
            IssueFocus::Comments => {
                comments = fetch_issue_comments(number, gh_config);
            }
        }
    }

    Sighting::GitHubIssue(Box::new(IssueSighting { summary, comments }))
}

/// Observe a repository with the requested focus items.
///
/// Defaults to both issues and pull requests when no focus is specified.
pub fn observe_github_repository(focus: &[RepositoryFocus], gh_config: &Path) -> Sighting {
    let focus = if focus.is_empty() {
        &[RepositoryFocus::Issues, RepositoryFocus::PullRequests]
    } else {
        focus
    };

    let mut issues = Vec::new();
    let mut pull_requests = Vec::new();

    for item in focus {
        match item {
            RepositoryFocus::Issues => {
                issues = fetch_repo_issues(gh_config);
            }
            RepositoryFocus::PullRequests => {
                pull_requests = fetch_repo_pull_requests(gh_config);
            }
        }
    }

    Sighting::GitHubRepository(Box::new(RepositorySighting {
        issues,
        pull_requests,
    }))
}

// ── gh CLI helpers ──

/// Run `gh` with the given args and return stdout, or `None` on failure.
fn gh(args: &[&str], gh_config: &Path) -> Option<String> {
    let output = Command::new("gh")
        .args(args)
        .env("GH_CONFIG_DIR", gh_config)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        None
    }
}

// ── Pull request fetchers ──

/// JSON shape returned by `gh pr view --json`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPrView {
    title: String,
    number: u64,
    state: String,
    author: GhActor,
    labels: Vec<GhLabel>,
    assignees: Vec<GhActor>,
    head_ref_name: String,
    base_ref_name: String,
    body: String,
}

#[derive(Deserialize)]
struct GhActor {
    login: String,
}

#[derive(Deserialize)]
struct GhLabel {
    name: String,
}

fn fetch_pr_summary(number: u64, gh_config: &Path) -> Option<GitHubSummary> {
    let num = number.to_string();
    let json = gh(
        &[
            "pr",
            "view",
            &num,
            "--json",
            "title,number,state,author,labels,assignees,headRefName,baseRefName,body",
        ],
        gh_config,
    )?;

    let pr: GhPrView = serde_json::from_str(&json).ok()?;

    Some(GitHubSummary {
        title: pr.title,
        number: pr.number,
        state: pr.state,
        author: pr.author.login,
        labels: pr.labels.into_iter().map(|l| l.name).collect(),
        assignees: pr.assignees.into_iter().map(|a| a.login).collect(),
        head_branch: Some(pr.head_ref_name),
        base_branch: Some(pr.base_ref_name),
        body: if pr.body.is_empty() {
            None
        } else {
            Some(pr.body)
        },
    })
}

/// JSON shape for `gh pr view --json files`.
#[derive(Deserialize)]
struct GhPrFiles {
    files: Vec<GhChangedFile>,
}

#[derive(Deserialize)]
struct GhChangedFile {
    path: String,
}

fn fetch_pr_files(number: u64, gh_config: &Path) -> Vec<String> {
    let num = number.to_string();
    let json = gh(&["pr", "view", &num, "--json", "files"], gh_config);

    json.and_then(|j| serde_json::from_str::<GhPrFiles>(&j).ok())
        .map(|f| f.files.into_iter().map(|f| f.path).collect())
        .unwrap_or_default()
}

/// JSON shape for `gh pr checks --json`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhCheckRun {
    name: String,
    state: String,
    // `gh pr checks --json` uses "state" and doesn't include a separate conclusion.
    // Map state to our model's status/conclusion.
}

fn fetch_pr_checks(number: u64, gh_config: &Path) -> Vec<CheckRun> {
    let num = number.to_string();
    let json = gh(&["pr", "checks", &num, "--json", "name,state"], gh_config);

    json.and_then(|j| serde_json::from_str::<Vec<GhCheckRun>>(&j).ok())
        .map(|runs| {
            runs.into_iter()
                .map(|r| {
                    let (status, conclusion) = match r.state.as_str() {
                        "SUCCESS" => ("completed".to_string(), Some("success".to_string())),
                        "FAILURE" => ("completed".to_string(), Some("failure".to_string())),
                        "PENDING" => ("in_progress".to_string(), None),
                        other => (other.to_lowercase(), None),
                    };
                    CheckRun {
                        name: r.name,
                        status,
                        conclusion,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn fetch_pr_diff(number: u64, gh_config: &Path) -> Option<String> {
    let num = number.to_string();
    let output = gh(&["pr", "diff", &num], gh_config)?;
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

/// JSON shape for `gh pr view --json comments`.
#[derive(Deserialize)]
struct GhPrComments {
    comments: Vec<GhComment>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhComment {
    author: GhActor,
    body: String,
    created_at: String,
}

fn fetch_pr_comments(number: u64, gh_config: &Path) -> Vec<GitHubComment> {
    let num = number.to_string();
    let json = gh(&["pr", "view", &num, "--json", "comments"], gh_config);

    json.and_then(|j| serde_json::from_str::<GhPrComments>(&j).ok())
        .map(|c| {
            c.comments
                .into_iter()
                .map(|c| GitHubComment {
                    author: c.author.login,
                    body: c.body,
                    created_at: c.created_at,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// JSON shape for inline review comments from the REST API.
#[derive(Deserialize)]
struct GhReviewComment {
    id: u64,
    path: String,
    line: Option<u64>,
    user: GhUser,
    body: String,
    created_at: String,
    in_reply_to_id: Option<u64>,
}

#[derive(Deserialize)]
struct GhUser {
    login: String,
}

fn fetch_pr_reviews(number: u64, gh_config: &Path) -> Vec<ReviewComment> {
    let num = number.to_string();
    // The GraphQL-based `gh pr view` doesn't expose inline review comments.
    // Use the REST API via `gh api`.
    let endpoint = format!("repos/{{owner}}/{{repo}}/pulls/{num}/comments");
    let json = gh(&["api", &endpoint, "--paginate"], gh_config);

    json.and_then(|j| serde_json::from_str::<Vec<GhReviewComment>>(&j).ok())
        .map(|comments| {
            comments
                .into_iter()
                .map(|c| ReviewComment {
                    id: c.id,
                    path: c.path,
                    line: c.line,
                    author: c.user.login,
                    body: c.body,
                    created_at: c.created_at,
                    in_reply_to_id: c.in_reply_to_id,
                })
                .collect()
        })
        .unwrap_or_default()
}

// ── Issue fetchers ──

/// JSON shape returned by `gh issue view --json`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhIssueView {
    title: String,
    number: u64,
    state: String,
    author: GhActor,
    labels: Vec<GhLabel>,
    assignees: Vec<GhActor>,
    body: String,
}

fn fetch_issue_summary(number: u64, gh_config: &Path) -> Option<GitHubSummary> {
    let num = number.to_string();
    let json = gh(
        &[
            "issue",
            "view",
            &num,
            "--json",
            "title,number,state,author,labels,assignees,body",
        ],
        gh_config,
    )?;

    let issue: GhIssueView = serde_json::from_str(&json).ok()?;

    Some(GitHubSummary {
        title: issue.title,
        number: issue.number,
        state: issue.state,
        author: issue.author.login,
        labels: issue.labels.into_iter().map(|l| l.name).collect(),
        assignees: issue.assignees.into_iter().map(|a| a.login).collect(),
        head_branch: None,
        base_branch: None,
        body: if issue.body.is_empty() {
            None
        } else {
            Some(issue.body)
        },
    })
}

/// JSON shape for `gh issue view --json comments`.
#[derive(Deserialize)]
struct GhIssueComments {
    comments: Vec<GhComment>,
}

fn fetch_issue_comments(number: u64, gh_config: &Path) -> Vec<GitHubComment> {
    let num = number.to_string();
    let json = gh(&["issue", "view", &num, "--json", "comments"], gh_config);

    json.and_then(|j| serde_json::from_str::<GhIssueComments>(&j).ok())
        .map(|c| {
            c.comments
                .into_iter()
                .map(|c| GitHubComment {
                    author: c.author.login,
                    body: c.body,
                    created_at: c.created_at,
                })
                .collect()
        })
        .unwrap_or_default()
}

// ── Repository fetchers ──

/// JSON shape for `gh issue list --json`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhIssueListing {
    title: String,
    number: u64,
    state: String,
    author: GhActor,
    labels: Vec<GhLabel>,
}

fn fetch_repo_issues(gh_config: &Path) -> Vec<GitHubIssueSummary> {
    let json = gh(
        &[
            "issue",
            "list",
            "--json",
            "title,number,state,author,labels",
            "--limit",
            "100",
        ],
        gh_config,
    );

    json.and_then(|j| serde_json::from_str::<Vec<GhIssueListing>>(&j).ok())
        .map(|issues| {
            issues
                .into_iter()
                .map(|i| GitHubIssueSummary {
                    number: i.number,
                    title: i.title,
                    state: i.state,
                    author: i.author.login,
                    labels: i.labels.into_iter().map(|l| l.name).collect(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// JSON shape for `gh pr list --json`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPrListing {
    title: String,
    number: u64,
    state: String,
    author: GhActor,
    labels: Vec<GhLabel>,
    head_ref_name: String,
}

fn fetch_repo_pull_requests(gh_config: &Path) -> Vec<GitHubPullRequestSummary> {
    let json = gh(
        &[
            "pr",
            "list",
            "--json",
            "title,number,state,author,labels,headRefName",
            "--limit",
            "100",
        ],
        gh_config,
    );

    json.and_then(|j| serde_json::from_str::<Vec<GhPrListing>>(&j).ok())
        .map(|prs| {
            prs.into_iter()
                .map(|p| GitHubPullRequestSummary {
                    number: p.number,
                    title: p.title,
                    state: p.state,
                    author: p.author.login,
                    labels: p.labels.into_iter().map(|l| l.name).collect(),
                    head_branch: p.head_ref_name,
                })
                .collect()
        })
        .unwrap_or_default()
}
