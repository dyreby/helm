//! Action execution: drive git and GitHub operations.
//!
//! Each function executes a real operation (git push, gh pr create, etc.)
//! and returns the structured `Act` on success.
//! Failed operations return an error — they are not recorded in the logbook.

use std::process::Command;

use crate::{
    cli::ActCommand,
    model::{Act, IssueAct, PullRequestAct},
};

/// Execute an action and return the structured act on success.
pub fn execute(identity: &str, command: &ActCommand) -> Result<Act, String> {
    match command {
        ActCommand::Push { branch } => push(branch),
        ActCommand::CreatePullRequest {
            branch,
            title,
            body,
            base,
            reviewer,
        } => create_pull_request(
            identity,
            branch,
            title,
            body.as_deref(),
            base.as_deref(),
            reviewer.as_deref(),
        ),
        ActCommand::MergePullRequest { number } => merge_pull_request(identity, *number),
        ActCommand::CommentOnPullRequest { number, body } => {
            comment_on_pull_request(identity, *number, body)
        }
        ActCommand::ReplyToReviewComment {
            number,
            comment_id,
            body,
        } => reply_to_review_comment(identity, *number, *comment_id, body),
        ActCommand::CreateIssue {
            title,
            body,
            label,
        } => create_issue(identity, title, body.as_deref(), label),
        ActCommand::CloseIssue { number, reason } => {
            close_issue(identity, *number, reason.as_deref())
        }
        ActCommand::CommentOnIssue { number, body } => {
            comment_on_issue(identity, *number, body)
        }
    }
}

// ── Git operations ──

fn push(branch: &str) -> Result<Act, String> {
    run_git(&["push", "origin", branch])?;

    let sha = run_git(&["rev-parse", "HEAD"])?;

    Ok(Act::Pushed {
        branch: branch.to_string(),
        sha: sha.trim().to_string(),
    })
}

// ── Pull request operations ──

fn create_pull_request(
    identity: &str,
    branch: &str,
    title: &str,
    body: Option<&str>,
    base: Option<&str>,
    reviewer: Option<&str>,
) -> Result<Act, String> {
    let mut args = vec![
        "pr",
        "create",
        "--head",
        branch,
        "--title",
        title,
    ];

    if let Some(b) = body {
        args.extend(["--body", b]);
    }

    if let Some(b) = base {
        args.extend(["--base", b]);
    }

    if let Some(r) = reviewer {
        args.extend(["--reviewer", r]);
    }

    let output = run_gh(identity, &args)?;

    // gh pr create outputs the PR URL; extract the number from it.
    let number = parse_pr_number(&output)?;

    Ok(Act::PullRequest {
        number,
        act: PullRequestAct::Created,
    })
}

fn merge_pull_request(identity: &str, number: u64) -> Result<Act, String> {
    let num = number.to_string();
    run_gh(identity, &["pr", "merge", &num, "--squash"])?;

    Ok(Act::PullRequest {
        number,
        act: PullRequestAct::Merged,
    })
}

fn comment_on_pull_request(identity: &str, number: u64, body: &str) -> Result<Act, String> {
    let num = number.to_string();
    run_gh(identity, &["pr", "comment", &num, "--body", body])?;

    Ok(Act::PullRequest {
        number,
        act: PullRequestAct::Commented,
    })
}

fn reply_to_review_comment(
    identity: &str,
    number: u64,
    comment_id: u64,
    body: &str,
) -> Result<Act, String> {
    let endpoint = format!(
        "repos/{{owner}}/{{repo}}/pulls/{number}/comments"
    );
    let in_reply_to = comment_id.to_string();

    run_gh(
        identity,
        &[
            "api",
            &endpoint,
            "--method",
            "POST",
            "-f",
            &format!("body={body}"),
            "-F",
            &format!("in_reply_to={in_reply_to}"),
        ],
    )?;

    Ok(Act::PullRequest {
        number,
        act: PullRequestAct::Replied,
    })
}

// ── Issue operations ──

fn create_issue(
    identity: &str,
    title: &str,
    body: Option<&str>,
    labels: &[String],
) -> Result<Act, String> {
    let mut args = vec!["issue", "create", "--title", title];

    if let Some(b) = body {
        args.extend(["--body", b]);
    }

    for label in labels {
        args.extend(["--label", label]);
    }

    let output = run_gh(identity, &args)?;

    // gh issue create outputs the issue URL; extract the number from it.
    let number = parse_issue_number(&output)?;

    Ok(Act::Issue {
        number,
        act: IssueAct::Created,
    })
}

fn close_issue(identity: &str, number: u64, reason: Option<&str>) -> Result<Act, String> {
    let num = number.to_string();
    let mut args = vec!["issue", "close", &num];

    if let Some(r) = reason {
        args.extend(["--reason", r]);
    }

    run_gh(identity, &args)?;

    Ok(Act::Issue {
        number,
        act: IssueAct::Closed,
    })
}

fn comment_on_issue(identity: &str, number: u64, body: &str) -> Result<Act, String> {
    let num = number.to_string();
    run_gh(identity, &["issue", "comment", &num, "--body", body])?;

    Ok(Act::Issue {
        number,
        act: IssueAct::Commented,
    })
}

// ── Helpers ──

/// Run a git command and return its stdout on success.
fn run_git(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git {} failed: {stderr}", args.join(" ")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Run a gh command with the appropriate identity config and return stdout on success.
fn run_gh(identity: &str, args: &[&str]) -> Result<String, String> {
    let config_dir = gh_config_dir(identity);

    let output = Command::new("gh")
        .args(args)
        .env("GH_CONFIG_DIR", &config_dir)
        .output()
        .map_err(|e| format!("failed to run gh: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh {} failed: {stderr}", args.join(" ")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Returns the `GH_CONFIG_DIR` path for a given identity.
fn gh_config_dir(identity: &str) -> String {
    let home = dirs::home_dir().expect("could not determine home directory");
    home.join(".helm")
        .join("gh-config")
        .join(identity)
        .to_string_lossy()
        .to_string()
}

/// Extract a PR number from a GitHub PR URL.
///
/// Example: `https://github.com/owner/repo/pull/45` → `45`.
fn parse_pr_number(url: &str) -> Result<u64, String> {
    url.trim()
        .rsplit('/')
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("could not parse PR number from: {url}"))
}

/// Extract an issue number from a GitHub issue URL.
///
/// Example: `https://github.com/owner/repo/issues/42` → `42`.
fn parse_issue_number(url: &str) -> Result<u64, String> {
    url.trim()
        .rsplit('/')
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("could not parse issue number from: {url}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pr_number_from_url() {
        let url = "https://github.com/dyreby/helm/pull/45\n";
        assert_eq!(parse_pr_number(url).unwrap(), 45);
    }

    #[test]
    fn parse_pr_number_invalid() {
        assert!(parse_pr_number("not a url").is_err());
    }

    #[test]
    fn parse_issue_number_from_url() {
        let url = "https://github.com/dyreby/helm/issues/42\n";
        assert_eq!(parse_issue_number(url).unwrap(), 42);
    }

    #[test]
    fn parse_issue_number_invalid() {
        assert!(parse_issue_number("not a url").is_err());
    }

    #[test]
    fn gh_config_dir_uses_helm_path() {
        let dir = gh_config_dir("john-agent");
        assert!(dir.contains(".helm"));
        assert!(dir.contains("gh-config"));
        assert!(dir.contains("john-agent"));
    }
}
