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

use std::{fs, path::PathBuf};

use clap::{ArgGroup, Parser, Subcommand, ValueEnum};
use jiff::Timestamp;
use uuid::Uuid;

use crate::{
    bearing,
    model::{
        CommentTarget, EntryKind, LogbookEntry, Observe, PullRequestFocus, Steer, Voyage,
        VoyageStatus,
    },
    steer,
    storage::Storage,
};

use format::format_pr_focus;

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
  2. helm observe --voyage a3b --as john-agent github-issue 42
  3. helm steer --voyage a3b --as john-agent --summary "Plan looks good" comment --issue 42 --body "Here's my plan: ..."
  4. helm voyage end --voyage a3b --status "Merged PR #45"

Observe:
  helm observe --voyage a3b --as john-agent file-contents --read src/widget.rs
  helm observe --voyage a3b --as john-agent github-pr 42 --focus full
  helm observe --voyage a3b --as john-agent github-repo"#;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage voyages: create new ones, list existing, end them.
    Voyage {
        #[command(subcommand)]
        command: VoyageCommand,
    },

    /// Observe the world and add to the working set.
    ///
    /// Pure read, no side effects, repeatable.
    /// The observation JSON is written to `--out` (if given) or stdout.
    /// A human-readable summary is printed to stderr when writing to a file.
    /// GitHub observations require `--as`.
    Observe {
        /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b`).
        #[arg(long)]
        voyage: String,

        /// Identity to use for GitHub auth (e.g. `john-agent`).
        /// Required for GitHub observations; ignored for local observations.
        #[arg(long = "as")]
        identity: Option<String>,

        #[command(subcommand)]
        source: ObserveSource,

        /// Write the observation JSON to this file instead of stdout.
        #[arg(long, global = true)]
        out: Option<PathBuf>,
    },

    /// Steer: execute an intent-based action that mutates collaborative state.
    ///
    /// Seals a bearing from the working set, executes the action,
    /// records one logbook entry, and clears the working set.
    Steer {
        /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b`).
        #[arg(long)]
        voyage: String,

        /// Who is steering (e.g. `john-agent`).
        #[arg(long = "as")]
        identity: String,

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

        /// Who is logging (e.g. `john-agent`).
        #[arg(long = "as")]
        identity: String,

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

#[derive(Debug, Subcommand)]
pub enum VoyageCommand {
    /// Create a new voyage. Prints the voyage ID.
    New {
        /// What this voyage is about.
        intent: String,
    },

    /// List voyages.
    List,

    /// End a voyage.
    End {
        /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b`).
        #[arg(long)]
        voyage: String,

        /// Freeform status: what was accomplished, learned, or left open.
        #[arg(long)]
        status: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ObserveSource {
    /// Read specific files.
    FileContents {
        /// Files to read (full contents).
        #[arg(long)]
        read: Vec<PathBuf>,
    },

    /// Walk a directory tree recursively.
    ///
    /// Respects `.gitignore` by default.
    DirectoryTree {
        /// Root directory to walk.
        root: PathBuf,

        /// Directory names to skip at any depth (e.g. `"target"`, `"node_modules"`).
        #[arg(long)]
        skip: Vec<String>,

        /// Maximum recursion depth (unlimited if not specified).
        #[arg(long)]
        max_depth: Option<u32>,
    },

    /// Observe a Rust project: full directory tree and documentation.
    RustProject {
        /// Path to the project root.
        path: PathBuf,
    },

    /// Observe a GitHub pull request.
    ///
    /// `summary` fetches metadata and comments.
    /// `full` fetches everything: metadata, comments, diff, files, checks, and inline reviews.
    /// Defaults to summary when `--focus` is not specified.
    #[command(name = "github-pr")]
    GitHubPullRequest {
        /// PR number.
        number: u64,

        /// How much to fetch.
        #[arg(long, value_enum, default_value_t = PrFocusArg::Summary)]
        focus: PrFocusArg,
    },

    /// Observe a GitHub issue.
    ///
    /// Always fetches metadata and comments.
    #[command(name = "github-issue")]
    GitHubIssue {
        /// Issue number.
        number: u64,
    },

    /// Observe a GitHub repository.
    ///
    /// Always fetches open issues and pull requests.
    #[command(name = "github-repo")]
    GitHubRepository,
}

/// CLI-facing PR focus, mapped to the domain `PullRequestFocus`.
#[derive(Debug, Clone, ValueEnum)]
pub enum PrFocusArg {
    /// PR metadata and comments.
    Summary,
    /// Everything: metadata, comments, diff, files, checks, and inline reviews.
    Full,
}

impl PrFocusArg {
    fn to_domain(&self) -> PullRequestFocus {
        match self {
            Self::Summary => PullRequestFocus::Summary,
            Self::Full => PullRequestFocus::Full,
        }
    }
}

/// Run the CLI, returning an error message on failure.
pub fn run(storage: &Storage) -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Command::Voyage { command } => match command {
            VoyageCommand::New { intent } => cmd_new(storage, &intent),
            VoyageCommand::List => cmd_list(storage),
            VoyageCommand::End { voyage, status } => {
                let voyage = resolve_voyage(storage, &voyage)?;
                cmd_end(storage, &voyage, status.as_deref())
            }
        },
        Command::Observe {
            voyage,
            identity,
            ref source,
            out,
        } => {
            let voyage = resolve_voyage(storage, &voyage)?;
            cmd_observe(&voyage, identity.as_deref(), source, out)
        }
        Command::Steer {
            voyage,
            identity,
            summary,
            action,
        } => {
            let voyage = resolve_voyage(storage, &voyage)?;
            cmd_steer(storage, &voyage, &identity, &summary, &action)
        }
        Command::Log {
            voyage,
            identity,
            summary,
            status,
        } => {
            let voyage = resolve_voyage(storage, &voyage)?;
            cmd_log(storage, &voyage, &identity, &summary, &status)
        }
    }
}

fn cmd_new(storage: &Storage, intent: &str) -> Result<(), String> {
    let voyage = Voyage {
        id: Uuid::new_v4(),
        intent: intent.to_string(),
        created_at: Timestamp::now(),
        status: VoyageStatus::Active,
    };

    storage
        .create_voyage(&voyage)
        .map_err(|e| format!("failed to create voyage: {e}"))?;

    println!("{}", voyage.id);
    Ok(())
}

fn cmd_list(storage: &Storage) -> Result<(), String> {
    let voyages = storage
        .list_voyages()
        .map_err(|e| format!("failed to list voyages: {e}"))?;

    if voyages.is_empty() {
        println!("No voyages");
        return Ok(());
    }

    for v in &voyages {
        let status = match v.status {
            VoyageStatus::Active => "active",
            VoyageStatus::Ended { .. } => "ended",
        };
        let short_id = &v.id.to_string()[..8];
        println!("{short_id}  [{status}]  {}", v.intent);
    }

    Ok(())
}

fn cmd_observe(
    voyage: &Voyage,
    identity: Option<&str>,
    source: &ObserveSource,
    out: Option<PathBuf>,
) -> Result<(), String> {
    let (target, needs_gh) = match source {
        ObserveSource::FileContents { read } => {
            if read.is_empty() {
                return Err("specify at least one --read".to_string());
            }
            (
                Observe::FileContents {
                    paths: read.clone(),
                },
                false,
            )
        }
        ObserveSource::DirectoryTree {
            root,
            skip,
            max_depth,
        } => (
            Observe::DirectoryTree {
                root: root.clone(),
                skip: skip.clone(),
                max_depth: *max_depth,
            },
            false,
        ),
        ObserveSource::RustProject { path } => (Observe::RustProject { root: path.clone() }, false),
        ObserveSource::GitHubPullRequest { number, focus } => (
            Observe::GitHubPullRequest {
                number: *number,
                focus: focus.to_domain(),
            },
            true,
        ),
        ObserveSource::GitHubIssue { number } => (Observe::GitHubIssue { number: *number }, true),
        ObserveSource::GitHubRepository => (Observe::GitHubRepository, true),
    };

    let gh_config = if needs_gh {
        let id = identity.ok_or("GitHub observations require --as <identity>".to_string())?;
        Some(gh_config_dir(id)?)
    } else {
        None
    };

    let observation = bearing::observe(&target, gh_config.as_deref());

    let json = serde_json::to_string_pretty(&observation)
        .map_err(|e| format!("failed to serialize observation: {e}"))?;

    match out {
        Some(path) => {
            fs::write(&path, &json)
                .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
            let summary = describe_observe_source(source);
            eprintln!("Observed {summary} → {}", path.display());
        }
        None => {
            println!("{json}");
        }
    }

    // TODO(#99): append observation to voyage's working set here.
    let _ = voyage;

    Ok(())
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

    // 2. Curate a bearing from the working set — seal what we knew going in.
    let observations = storage
        .load_working(voyage.id)
        .map_err(|e| format!("failed to load working set: {e}"))?;
    let bearing = bearing::seal(observations, summary.to_string());

    // 3. Execute the action — mutate collaborative state.
    //    Happens after seal so a failed execute leaves the logbook untouched.
    //    If execute succeeds but append fails, the steer is unlogged — acceptable
    //    gap for now; true atomicity would require a WAL or similar.
    steer::execute(&steer_action, &gh_config)?;

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

    // 5. Clear the working set.
    storage
        .clear_working(voyage.id)
        .map_err(|e| format!("failed to clear working set: {e}"))?;

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
    // 1. Curate a bearing from the working set — seal what we knew going in.
    let observations = storage
        .load_working(voyage.id)
        .map_err(|e| format!("failed to load working set: {e}"))?;
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

    // 3. Clear the working set.
    storage
        .clear_working(voyage.id)
        .map_err(|e| format!("failed to clear working set: {e}"))?;

    eprintln!("Logged: {status}");
    Ok(())
}

fn cmd_end(storage: &Storage, voyage: &Voyage, status: Option<&str>) -> Result<(), String> {
    if matches!(voyage.status, VoyageStatus::Ended { .. }) {
        return Err(format!(
            "voyage {} is already ended",
            &voyage.id.to_string()[..8]
        ));
    }

    let mut voyage = voyage.clone();
    voyage.status = VoyageStatus::Ended {
        ended_at: Timestamp::now(),
        status: status.map(String::from),
    };
    storage
        .update_voyage(&voyage)
        .map_err(|e| format!("failed to update voyage: {e}"))?;

    let short_id = &voyage.id.to_string()[..8];
    eprintln!("Voyage {short_id} ended");
    if let Some(s) = status {
        eprintln!("Status: {s}");
    }

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
fn gh_config_dir(identity: &str) -> Result<PathBuf, String> {
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

/// Short human-readable description of what was observed.
fn describe_observe_source(source: &ObserveSource) -> String {
    match source {
        ObserveSource::FileContents { read } => format!("{} file(s)", read.len()),
        ObserveSource::DirectoryTree { root, .. } => {
            format!("directory tree at {}", root.display())
        }
        ObserveSource::RustProject { path } => format!("Rust project at {}", path.display()),
        ObserveSource::GitHubPullRequest { number, focus } => {
            format!("PR #{number} [{}]", format_pr_focus(&focus.to_domain()))
        }
        ObserveSource::GitHubIssue { number } => format!("issue #{number}"),
        ObserveSource::GitHubRepository => "repository".to_string(),
    }
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
fn resolve_voyage(storage: &Storage, reference: &str) -> Result<Voyage, String> {
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
