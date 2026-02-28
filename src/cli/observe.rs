//! Observe command: gather observations onto the slate.

use std::{fs, path::PathBuf};

use crate::{
    bearing, identity,
    model::{Observe, Voyage},
    storage::Storage,
};

use super::target::ObserveTarget;

pub(super) fn cmd_observe(
    storage: &Storage,
    voyage: &Voyage,
    identity: Option<&str>,
    target: &ObserveTarget,
    out: Option<PathBuf>,
) -> Result<(), String> {
    let observe = match target {
        ObserveTarget::FileContents { read } => {
            if read.is_empty() {
                return Err("specify at least one --read".to_string());
            }
            Observe::FileContents {
                paths: read.clone(),
            }
        }
        ObserveTarget::DirectoryTree {
            root,
            skip,
            max_depth,
        } => Observe::DirectoryTree {
            root: root.clone(),
            skip: skip.clone(),
            max_depth: *max_depth,
        },
        ObserveTarget::RustProject { path } => Observe::RustProject { root: path.clone() },
        ObserveTarget::GitHubPullRequest { number } => {
            Observe::GitHubPullRequest { number: *number }
        }
        ObserveTarget::GitHubIssue { number } => Observe::GitHubIssue { number: *number },
        ObserveTarget::GitHubRepository => Observe::GitHubRepository,
    };

    let gh_config = if observe.needs_gh() {
        let id = identity::resolve_identity(identity)?;
        Some(super::gh_config_dir(&id)?)
    } else {
        None
    };

    let observation = bearing::observe(&observe, gh_config.as_deref());

    let json = serde_json::to_string_pretty(&observation)
        .map_err(|e| format!("failed to serialize observation: {e}"))?;

    storage
        .observe(voyage.id, &observation)
        .map_err(|e| format!("failed to append to slate: {e}"))?;

    match out {
        Some(path) => {
            fs::write(&path, &json)
                .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
            let summary = describe_observe_target(target);
            eprintln!("Observed {summary} → {}", path.display());
        }
        None => {
            println!("{json}");
        }
    }

    Ok(())
}

/// Short human-readable description of what was observed.
fn describe_observe_target(target: &ObserveTarget) -> String {
    match target {
        ObserveTarget::FileContents { read } => format!("{} file(s)", read.len()),
        ObserveTarget::DirectoryTree { root, .. } => {
            format!("directory tree at {}", root.display())
        }
        ObserveTarget::RustProject { path } => format!("Rust project at {}", path.display()),
        ObserveTarget::GitHubPullRequest { number } => format!("PR #{number}"),
        ObserveTarget::GitHubIssue { number } => format!("issue #{number}"),
        ObserveTarget::GitHubRepository => "repository".to_string(),
    }
}
