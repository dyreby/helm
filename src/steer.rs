//! Steer execution: running intent-based actions that mutate collaborative state.
//!
//! Each steer action maps to one or more `gh` CLI calls. Authentication uses
//! the caller's identity via `GH_CONFIG_DIR`.

use std::{path::Path, process::Command};

use crate::model::{CommentTarget, Steer};

/// Perform a steer action.
///
/// Returns `Ok(())` on success, or an error message describing what failed.
pub fn perform(steer: &Steer, gh_config: &Path) -> Result<(), String> {
    match steer {
        Steer::Comment {
            number,
            body,
            target,
        } => perform_comment(*number, body, target, gh_config),
        _ => Err("this steer action is not yet implemented".to_string()),
    }
}

fn perform_comment(
    number: u64,
    body: &str,
    target: &CommentTarget,
    gh_config: &Path,
) -> Result<(), String> {
    let num = number.to_string();
    match target {
        CommentTarget::Issue => gh(&["issue", "comment", &num, "--body", body], gh_config),
        CommentTarget::PullRequest => gh(&["pr", "comment", &num, "--body", body], gh_config),
        CommentTarget::ReviewFeedback { comment_id } => {
            let endpoint = format!("repos/{{owner}}/{{repo}}/pulls/comments/{comment_id}/replies");
            gh(
                &[
                    "api",
                    &endpoint,
                    "--method",
                    "POST",
                    "-f",
                    &format!("body={body}"),
                ],
                gh_config,
            )
        }
    }
}

/// Run a `gh` command with the given identity's config dir.
fn gh(args: &[&str], gh_config: &Path) -> Result<(), String> {
    let output = Command::new("gh")
        .args(args)
        .env("GH_CONFIG_DIR", gh_config)
        .output()
        .map_err(|e| format!("failed to run gh: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("gh command failed: {stderr}"))
    }
}
