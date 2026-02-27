//! Git and GitHub shell-out helpers.

use std::path::PathBuf;
use std::process;

use crate::model::{ActionKind, IssueAction, PullRequestAction};

use super::ActionCommand;

/// Dispatch the action command and return the structured `ActionKind` on success.
pub(super) fn perform(
    action_cmd: &ActionCommand,
    gh_config: &PathBuf,
) -> Result<ActionKind, String> {
    match action_cmd {
        ActionCommand::Commit { message } => commit(message),
        ActionCommand::Push { branch } => push(branch),
        ActionCommand::CreatePullRequest {
            branch,
            title,
            body,
            base,
            reviewer,
        } => create_pr(gh_config, branch, title, body.as_deref(), base, reviewer),
        ActionCommand::MergePullRequest { number } => merge_pr(gh_config, *number),
        ActionCommand::CommentOnPullRequest { number, body } => {
            comment_pr(gh_config, *number, body)
        }
        ActionCommand::ReplyOnPullRequest {
            number,
            comment_id,
            body,
        } => reply_pr(gh_config, *number, *comment_id, body),
        ActionCommand::RequestReview { number, reviewer } => {
            request_review(gh_config, *number, reviewer)
        }
        ActionCommand::CreateIssue { title, body } => {
            create_issue(gh_config, title, body.as_deref())
        }
        ActionCommand::CloseIssue { number } => close_issue(gh_config, *number),
        ActionCommand::CommentOnIssue { number, body } => comment_issue(gh_config, *number, body),
    }
}

pub(super) fn commit(message: &str) -> Result<ActionKind, String> {
    run_cmd("git", &["commit", "-m", message], None)?;

    let sha = run_cmd_output("git", &["rev-parse", "HEAD"], None)?;

    Ok(ActionKind::Commit { sha })
}

pub(super) fn push(branch: &str) -> Result<ActionKind, String> {
    run_cmd("git", &["push", "origin", branch], None)?;

    let sha = run_cmd_output("git", &["rev-parse", "HEAD"], None)?;

    Ok(ActionKind::Push {
        branch: branch.to_string(),
        sha,
    })
}

pub(super) fn create_pr(
    gh_config: &PathBuf,
    branch: &str,
    title: &str,
    body: Option<&str>,
    base: &str,
    reviewers: &[String],
) -> Result<ActionKind, String> {
    let mut args = vec![
        "pr", "create", "--head", branch, "--base", base, "--title", title,
    ];
    if let Some(b) = body {
        args.extend(["--body", b]);
    }
    for r in reviewers {
        args.extend(["--reviewer", r]);
    }

    let output = run_cmd_output("gh", &args, Some(gh_config))?;
    let number = parse_pr_number_from_url(&output)?;

    Ok(ActionKind::PullRequest {
        number,
        action: PullRequestAction::Create,
    })
}

pub(super) fn merge_pr(gh_config: &PathBuf, number: u64) -> Result<ActionKind, String> {
    let num_str = number.to_string();
    run_cmd(
        "gh",
        &["pr", "merge", &num_str, "--squash", "--delete-branch"],
        Some(gh_config),
    )?;

    Ok(ActionKind::PullRequest {
        number,
        action: PullRequestAction::Merge,
    })
}

pub(super) fn comment_pr(
    gh_config: &PathBuf,
    number: u64,
    body: &str,
) -> Result<ActionKind, String> {
    let num_str = number.to_string();
    run_cmd(
        "gh",
        &["pr", "comment", &num_str, "--body", body],
        Some(gh_config),
    )?;

    Ok(ActionKind::PullRequest {
        number,
        action: PullRequestAction::Comment,
    })
}

pub(super) fn reply_pr(
    gh_config: &PathBuf,
    number: u64,
    comment_id: u64,
    body: &str,
) -> Result<ActionKind, String> {
    let repo = detect_repo()?;
    let endpoint = format!("repos/{repo}/pulls/{number}/comments");
    let in_reply_to = comment_id.to_string();
    run_cmd(
        "gh",
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
        Some(gh_config),
    )?;

    Ok(ActionKind::PullRequest {
        number,
        action: PullRequestAction::Reply,
    })
}

pub(super) fn request_review(
    gh_config: &PathBuf,
    number: u64,
    reviewers: &[String],
) -> Result<ActionKind, String> {
    let num_str = number.to_string();
    for r in reviewers {
        run_cmd(
            "gh",
            &["pr", "edit", &num_str, "--add-reviewer", r],
            Some(gh_config),
        )?;
    }

    Ok(ActionKind::PullRequest {
        number,
        action: PullRequestAction::RequestedReview {
            reviewers: reviewers.to_vec(),
        },
    })
}

pub(super) fn create_issue(
    gh_config: &PathBuf,
    title: &str,
    body: Option<&str>,
) -> Result<ActionKind, String> {
    let mut args = vec!["issue", "create", "--title", title];
    if let Some(b) = body {
        args.extend(["--body", b]);
    }

    let output = run_cmd_output("gh", &args, Some(gh_config))?;
    let number = parse_issue_number_from_url(&output)?;

    Ok(ActionKind::Issue {
        number,
        action: IssueAction::Create,
    })
}

pub(super) fn close_issue(gh_config: &PathBuf, number: u64) -> Result<ActionKind, String> {
    let num_str = number.to_string();
    run_cmd("gh", &["issue", "close", &num_str], Some(gh_config))?;

    Ok(ActionKind::Issue {
        number,
        action: IssueAction::Close,
    })
}

pub(super) fn comment_issue(
    gh_config: &PathBuf,
    number: u64,
    body: &str,
) -> Result<ActionKind, String> {
    let num_str = number.to_string();
    run_cmd(
        "gh",
        &["issue", "comment", &num_str, "--body", body],
        Some(gh_config),
    )?;

    Ok(ActionKind::Issue {
        number,
        action: IssueAction::Comment,
    })
}

/// Run a command, returning an error if it fails.
pub(super) fn run_cmd(
    program: &str,
    args: &[&str],
    gh_config: Option<&PathBuf>,
) -> Result<(), String> {
    let mut cmd = process::Command::new(program);
    cmd.args(args);
    if let Some(config) = gh_config {
        cmd.env("GH_CONFIG_DIR", config);
    }

    let status = cmd
        .status()
        .map_err(|e| format!("failed to run {program}: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} exited with status {}",
            status.code().unwrap_or(-1)
        ))
    }
}

/// Run a command, capturing stdout and returning it trimmed.
pub(super) fn run_cmd_output(
    program: &str,
    args: &[&str],
    gh_config: Option<&PathBuf>,
) -> Result<String, String> {
    let mut cmd = process::Command::new(program);
    cmd.args(args);
    if let Some(config) = gh_config {
        cmd.env("GH_CONFIG_DIR", config);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("failed to run {program}: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "{program} exited with status {}: {stderr}",
            output.status.code().unwrap_or(-1)
        ))
    }
}

/// Detect the GitHub repo (owner/name) from the current directory.
pub(super) fn detect_repo() -> Result<String, String> {
    let output = run_cmd_output(
        "gh",
        &[
            "repo",
            "view",
            "--json",
            "nameWithOwner",
            "-q",
            ".nameWithOwner",
        ],
        None,
    )?;
    if output.is_empty() {
        return Err("could not detect GitHub repository from current directory".to_string());
    }
    Ok(output)
}

/// Parse a PR number from a GitHub PR URL (e.g. `https://github.com/owner/repo/pull/45`).
pub(super) fn parse_pr_number_from_url(url: &str) -> Result<u64, String> {
    url.rsplit('/')
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("could not parse PR number from: {url}"))
}

/// Parse an issue number from a GitHub issue URL (e.g. `https://github.com/owner/repo/issues/45`).
pub(super) fn parse_issue_number_from_url(url: &str) -> Result<u64, String> {
    url.rsplit('/')
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("could not parse issue number from: {url}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pr_number_from_github_url() {
        let url = "https://github.com/dyreby/helm/pull/45";
        assert_eq!(parse_pr_number_from_url(url).unwrap(), 45);
    }

    #[test]
    fn parse_issue_number_from_github_url() {
        let url = "https://github.com/dyreby/helm/issues/12";
        assert_eq!(parse_issue_number_from_url(url).unwrap(), 12);
    }
}
