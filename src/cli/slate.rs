//! Slate management commands: list, clear, erase.

use clap::Subcommand;

use crate::{
    model::{Observe, Voyage},
    storage::Storage,
};

use super::target::ObserveTarget;

#[derive(Debug, Subcommand)]
pub enum SlateCommand {
    /// List observations on the slate for a voyage.
    ///
    /// Outputs a JSON array to stdout.
    /// An empty slate outputs `[]`.
    List {
        /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b`).
        #[arg(long)]
        voyage: String,
    },

    /// Clear the slate without sealing.
    ///
    /// Wipes all observations without creating a logbook entry.
    /// Idempotent: safe to run on an already-empty slate.
    Clear {
        /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b`).
        #[arg(long)]
        voyage: String,
    },

    /// Erase a single observation from the slate.
    ///
    /// Removes the observation for the given target.
    /// Exits non-zero if the target is not on the slate.
    Erase {
        /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b`).
        #[arg(long)]
        voyage: String,

        #[command(subcommand)]
        target: ObserveTarget,
    },
}

pub(super) fn cmd_erase(
    storage: &Storage,
    voyage: &Voyage,
    target: &ObserveTarget,
) -> Result<(), String> {
    let observe = observe_from_target(target)?;
    let description = describe_target(target);

    let erased = storage
        .erase_from_slate(voyage.id, &observe)
        .map_err(|e| format!("failed to erase from slate: {e}"))?;

    if !erased {
        return Err(format!("{description} is not on the slate"));
    }

    Ok(())
}

/// Convert a CLI target to the storage `Observe` type.
fn observe_from_target(target: &ObserveTarget) -> Result<Observe, String> {
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
    Ok(observe)
}

/// Short human-readable description of the target.
fn describe_target(target: &ObserveTarget) -> String {
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

pub(super) fn cmd_list(storage: &Storage, voyage: &Voyage) -> Result<(), String> {
    let observations = storage
        .load_slate(voyage.id)
        .map_err(|e| format!("failed to load slate: {e}"))?;

    let json = serde_json::to_string_pretty(&observations)
        .map_err(|e| format!("failed to serialize slate: {e}"))?;

    println!("{json}");
    Ok(())
}

pub(super) fn cmd_clear(storage: &Storage, voyage: &Voyage) -> Result<(), String> {
    storage
        .clear_slate(voyage.id)
        .map_err(|e| format!("failed to clear slate: {e}"))?;
    Ok(())
}
