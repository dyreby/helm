//! CLI interface for Helm.
//!
//! Designed for agents and humans alike to navigate voyages from the command line.
//! Each subcommand is non-interactive: arguments in, structured output out.
//!
//! Commands split into two groups:
//!
//! - `helm voyage new|list|end` — lifecycle management, no voyage context needed.
//! - `helm --voyage <id> <command>` — everything else, operating within a voyage.
//!
//! The `--voyage` flag takes a full UUID or unambiguous prefix.

mod format;
mod observe;
mod slate;
mod target;
mod voyage;

use std::path::PathBuf;

use clap::{ArgGroup, Parser, Subcommand};
use jiff::Timestamp;
use uuid::Uuid;

use crate::{
    bearing, identity,
    model::{CommentTarget, EntryKind, LogbookEntry, Steer, Voyage},
    steer,
    storage::Storage,
};

use slate::SlateCommand;
use target::ObserveTarget;
use voyage::VoyageCommand;

/// Helm — navigate your work.
#[derive(Debug, Parser)]
#[command(name = "helm", after_long_help = WORKFLOW_HELP)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

const WORKFLOW_HELP: &str = r#"Workflow: advancing an issue
  1. helm voyage new "Resolve #42: fix widget crash"
     → prints a voyage ID (e.g. a3b0fc12)
  2. helm observe --voyage a3b --as dyreby github-issue 42
  3. helm steer --voyage a3b --as dyreby --summary "Plan looks good" comment --issue 42 --body "Here's my plan: ..."
  4. helm voyage end --voyage a3b --status "Merged PR #45"

Identity (--as):
  --as is optional when identity is configured elsewhere.
  Resolution order: --as flag → HELM_IDENTITY env var → ~/.helm/config.toml

Observe:
  helm observe --voyage a3b --as dyreby file-contents --read src/widget.rs
  helm observe --voyage a3b --as dyreby github-pr 42 --focus full
  helm observe --voyage a3b --as dyreby github-repo"#;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage voyages: create new ones, list existing, end them.
    Voyage {
        #[command(subcommand)]
        command: VoyageCommand,
    },

    /// Inspect and manage the slate for a voyage.
    Slate {
        #[command(subcommand)]
        command: SlateCommand,
    },

    /// Observe the world and add to the slate.
    ///
    /// Pure read, no side effects, repeatable.
    /// The observation JSON is written to `--out` (if given) or stdout.
    /// A human-readable summary is printed to stderr when writing to a file.
    /// GitHub observations require identity (`--as`, `HELM_IDENTITY`, or `~/.helm/config.toml`).
    Observe {
        /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b`).
        #[arg(long)]
        voyage: String,

        /// Identity to use for GitHub auth (e.g. `dyreby`).
        /// Falls back to `HELM_IDENTITY` env var, then `~/.helm/config.toml`.
        /// Ignored for local observations.
        #[arg(long = "as")]
        identity: Option<String>,

        #[command(subcommand)]
        target: ObserveTarget,

        /// Write the observation JSON to this file instead of stdout.
        #[arg(long, global = true)]
        out: Option<PathBuf>,
    },

    /// Steer: perform an intent-based action that mutates collaborative state.
    ///
    /// Seals a bearing from the slate, performs the action,
    /// records one logbook entry, and clears the slate.
    Steer {
        /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b`).
        #[arg(long)]
        voyage: String,

        /// Who is steering (e.g. `dyreby`).
        ///
        /// Falls back to `HELM_IDENTITY` env var, then `~/.helm/config.toml`.
        #[arg(long = "as")]
        identity: Option<String>,

        /// Why you're steering — orientation for the logbook entry.
        #[arg(long)]
        summary: String,

        #[command(subcommand)]
        action: SteerAction,
    },

    /// Log a deliberate state without mutating collaborative state.
    ///
    /// Same seal-and-clear behavior as steer. Use when the voyage reaches
    /// a state worth recording but there's nothing to change in the world.
    Log {
        /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b`).
        #[arg(long)]
        voyage: String,

        /// Who is logging (e.g. `dyreby`).
        ///
        /// Falls back to `HELM_IDENTITY` env var, then `~/.helm/config.toml`.
        #[arg(long = "as")]
        identity: Option<String>,

        /// Why you're logging — orientation for the logbook entry.
        #[arg(long)]
        summary: String,

        /// Freeform status to record.
        status: String,
    },
}

/// Steer subcommands.
#[derive(Debug, Subcommand)]
pub enum SteerAction {
    /// Comment on an issue, PR, or inline review thread.
    #[command(group(ArgGroup::new("target").required(true).args(["issue", "pr"])))]
    Comment {
        /// Comment on this issue number.
        #[arg(long, conflicts_with = "pr")]
        issue: Option<u64>,

        /// Comment on this PR number.
        #[arg(long, conflicts_with = "issue")]
        pr: Option<u64>,

        /// Reply to this inline review comment ID (requires `--pr`).
        #[arg(long, requires = "pr")]
        reply_to: Option<u64>,

        /// Comment body.
        #[arg(long)]
        body: String,
    },
}

/// Run the CLI, returning an error message on failure.
pub fn run(storage: &Storage) -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Command::Voyage { command } => match command {
            VoyageCommand::New { intent } => voyage::cmd_new(storage, &intent),
            VoyageCommand::List => voyage::cmd_list(storage),
            VoyageCommand::End { voyage, status } => {
                let voyage = resolve_voyage(storage, &voyage)?;
                voyage::cmd_end(storage, &voyage, status.as_deref())
            }
        },
        Command::Slate { command } => match command {
            SlateCommand::List { voyage } => {
                let voyage = resolve_voyage(storage, &voyage)?;
                slate::cmd_list(storage, &voyage)
            }
            SlateCommand::Clear { voyage } => {
                let voyage = resolve_voyage(storage, &voyage)?;
                slate::cmd_clear(storage, &voyage)
            }
        },
        Command::Observe {
            voyage,
            identity,
            target,
            out,
        } => {
            let voyage = resolve_voyage(storage, &voyage)?;
            // Identity is resolved lazily inside cmd_observe — local targets don't require it.
            observe::cmd_observe(storage, &voyage, identity.as_deref(), &target, out)
        }
        Command::Steer {
            voyage,
            identity,
            summary,
            action,
        } => {
            let voyage = resolve_voyage(storage, &voyage)?;
            let identity = identity::resolve_identity(identity.as_deref())?;
            cmd_steer(storage, &voyage, &identity, &summary, &action)
        }
        Command::Log {
            voyage,
            identity,
            summary,
            status,
        } => {
            let voyage = resolve_voyage(storage, &voyage)?;
            let identity = identity::resolve_identity(identity.as_deref())?;
            cmd_log(storage, &voyage, &identity, &summary, &status)
        }
    }
}

fn cmd_steer(
    storage: &Storage,
    voyage: &Voyage,
    identity: &str,
    summary: &str,
    action: &SteerAction,
) -> Result<(), String> {
    let gh_config = gh_config_dir(identity)?;

    // 1. Build the typed steer action from CLI args.
    let steer_action = build_steer_action(action);

    // 2. Seal the slate into a bearing — capture what we knew going in.
    let observations = storage
        .load_slate(voyage.id)
        .map_err(|e| format!("failed to load slate: {e}"))?;
    let bearing = bearing::seal(observations, summary.to_string());

    // 3. Perform the action — mutate collaborative state.
    //    Happens after seal so a failed perform leaves the logbook untouched.
    //    If perform succeeds but append fails, the steer is unlogged — acceptable
    //    gap for now; true atomicity would require a WAL or similar.
    steer::perform(&steer_action, &gh_config)?;

    // 4. Record one logbook entry.
    let entry = LogbookEntry {
        bearing,
        author: identity.to_string(),
        timestamp: Timestamp::now(),
        kind: EntryKind::Steer(steer_action),
    };
    storage
        .append_entry(voyage.id, &entry)
        .map_err(|e| format!("failed to append logbook entry: {e}"))?;

    // 5. Clear the slate.
    storage
        .clear_slate(voyage.id)
        .map_err(|e| format!("failed to clear slate: {e}"))?;

    eprintln!("Steered: {}", describe_steer_action(action));
    Ok(())
}

fn cmd_log(
    storage: &Storage,
    voyage: &Voyage,
    identity: &str,
    summary: &str,
    status: &str,
) -> Result<(), String> {
    // 1. Seal the slate into a bearing — capture what we knew going in.
    let observations = storage
        .load_slate(voyage.id)
        .map_err(|e| format!("failed to load slate: {e}"))?;
    let bearing = bearing::seal(observations, summary.to_string());

    // 2. Record one logbook entry. No collaborative state is mutated.
    let entry = LogbookEntry {
        bearing,
        author: identity.to_string(),
        timestamp: Timestamp::now(),
        kind: EntryKind::Log(status.to_string()),
    };
    storage
        .append_entry(voyage.id, &entry)
        .map_err(|e| format!("failed to append logbook entry: {e}"))?;

    // 3. Clear the slate.
    storage
        .clear_slate(voyage.id)
        .map_err(|e| format!("failed to clear slate: {e}"))?;

    eprintln!("Logged: {status}");
    Ok(())
}

/// Convert CLI steer args to the typed `Steer` model.
fn build_steer_action(action: &SteerAction) -> Steer {
    match action {
        SteerAction::Comment {
            issue,
            pr,
            reply_to,
            body,
        } => {
            // Clap's ArgGroup ensures exactly one of --issue or --pr is present.
            let (number, target) = match (issue, pr, reply_to) {
                (Some(n), None, None) => (*n, CommentTarget::Issue),
                (None, Some(n), None) => (*n, CommentTarget::PullRequest),
                (None, Some(n), Some(id)) => {
                    (*n, CommentTarget::ReviewFeedback { comment_id: *id })
                }
                _ => unreachable!("clap ArgGroup guarantees --issue or --pr is present"),
            };
            Steer::Comment {
                number,
                body: body.clone(),
                target,
            }
        }
    }
}

/// Resolve the `GH_CONFIG_DIR` for a given identity.
///
/// Each identity has its own config directory under `~/.helm/gh-config/<identity>/`.
/// The directory must exist and contain valid `gh` auth.
pub(super) fn gh_config_dir(identity: &str) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("could not determine home directory")?;
    let config_dir = home.join(".helm").join("gh-config").join(identity);
    if !config_dir.exists() {
        return Err(format!(
            "no GitHub config for identity '{identity}' — \
             expected directory at {}\n\
             Set up with: GH_CONFIG_DIR={} gh auth login",
            config_dir.display(),
            config_dir.display(),
        ));
    }
    Ok(config_dir)
}

/// Short human-readable description of what was steered.
fn describe_steer_action(action: &SteerAction) -> String {
    match action {
        SteerAction::Comment {
            issue,
            pr,
            reply_to,
            body: _,
        } => match (issue, pr, reply_to) {
            (Some(n), None, None) => format!("comment on issue #{n}"),
            (None, Some(n), None) => format!("comment on PR #{n}"),
            (None, Some(n), Some(id)) => format!("reply to review comment {id} on PR #{n}"),
            _ => "comment".to_string(),
        },
    }
}

/// Resolve a voyage reference (full UUID or unambiguous prefix) to a voyage.
pub(super) fn resolve_voyage(storage: &Storage, reference: &str) -> Result<Voyage, String> {
    // Try full UUID first.
    if let Ok(id) = reference.parse::<Uuid>() {
        return storage
            .load_voyage(id)
            .map_err(|e| format!("voyage not found: {e}"));
    }

    // Try as a prefix match against all voyages.
    let voyages = storage
        .list_voyages()
        .map_err(|e| format!("failed to list voyages: {e}"))?;

    let matches: Vec<&Voyage> = voyages
        .iter()
        .filter(|v| v.id.to_string().starts_with(reference))
        .collect();

    match matches.len() {
        0 => Err(format!("no voyage matching '{reference}'")),
        1 => Ok(matches[0].clone()),
        n => {
            let ids: Vec<String> = matches
                .iter()
                .map(|v| v.id.to_string()[..8].to_string())
                .collect();
            Err(format!(
                "'{reference}' is ambiguous — matches {n} voyages: {}",
                ids.join(", ")
            ))
        }
    }
}
