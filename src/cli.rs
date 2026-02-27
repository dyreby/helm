//! CLI interface for Helm.
//!
//! Designed for agents and humans alike to navigate voyages from the command line.
//! Each subcommand is non-interactive: arguments in, structured output out.
//!
//! Commands split into two groups:
//!
//! - `helm voyage new|list` — lifecycle management, no voyage context needed.
//! - `helm --voyage <id> <command>` — everything else, operating within a voyage.
//!
//! The `--voyage` flag takes a full UUID or unambiguous prefix.

mod format;

use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use jiff::Timestamp;
use uuid::Uuid;

use crate::config::Config;
use crate::model::{IssueFocus, Observe, PullRequestFocus, RepositoryFocus, Voyage, VoyageStatus};
use crate::{bearing, storage::Storage};

use format::{format_issue_focuses, format_pr_focuses, format_repo_focuses};

/// Helm — navigate your work.
#[derive(Debug, Parser)]
#[command(name = "helm", after_long_help = WORKFLOW_HELP)]
pub struct Cli {
    /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b`).
    /// Required for observe, steer, and log.
    #[arg(long, global = true)]
    voyage: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

const WORKFLOW_HELP: &str = r#"Workflow: advancing an issue
  1. helm voyage new --as john-agent "Resolve #42: fix widget crash"
     → prints a voyage ID (e.g. a3b0fc12)
  2. helm --voyage a3b observe github-issue 42 --focus summary --focus comments
  3. helm --voyage a3b steer comment 42 "Here's my plan: ..."
  4. helm --voyage a3b voyage end --status "Merged PR #45"

Observe:
  helm --voyage a3b observe file-contents --read src/widget.rs
  helm --voyage a3b observe github-pr 42 --focus summary --focus diff
  helm --voyage a3b observe github-repo --focus issues"#;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage voyages: create new ones, list existing.
    Voyage {
        #[command(subcommand)]
        command: VoyageCommand,
    },

    /// Observe the world and add to the working set.
    ///
    /// Pure read, no side effects, repeatable.
    /// The observation JSON is written to `--out` (if given) or stdout.
    /// A human-readable summary is printed to stderr when writing to a file.
    /// Requires `--voyage`.
    Observe {
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
    /// Requires `--voyage`.
    ///
    /// Not yet implemented — coming in a future mission.
    Steer {
        #[command(subcommand)]
        action: SteerAction,
    },

    /// Log a deliberate state without mutating collaborative state.
    ///
    /// Same seal-and-clear behavior as steer. Use when the voyage reaches
    /// a state worth recording but there's nothing to change in the world.
    /// Requires `--voyage`.
    ///
    /// Not yet implemented — coming in a future mission.
    Log {
        /// Freeform status to record.
        status: String,
    },
}

/// Steer subcommands (stub — fields defined when each is built).
#[derive(Debug, Subcommand)]
pub enum SteerAction {
    /// Comment on an issue or PR.
    Comment {
        /// Issue or PR number.
        number: u64,
        /// Comment body.
        body: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum VoyageCommand {
    /// Create a new voyage. Prints the voyage ID.
    New {
        /// Identity for this voyage (e.g. "john-agent").
        /// All commands on this voyage inherit this identity for GitHub auth.
        /// When omitted, the configured default identity is used.
        #[arg(long = "as")]
        identity: Option<String>,

        /// What this voyage is about.
        intent: String,
    },

    /// List active voyages.
    List,

    /// End a voyage.
    End {
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
    /// Fetches PR metadata, diff, comments, reviews, checks, or changed files.
    /// Defaults to summary when no `--focus` is specified.
    #[command(name = "github-pr")]
    GitHubPullRequest {
        /// PR number.
        number: u64,

        /// What to observe. Can be specified multiple times.
        #[arg(long, value_enum)]
        focus: Vec<PrFocusArg>,
    },

    /// Observe a GitHub issue.
    ///
    /// Fetches issue metadata or comments.
    /// Defaults to summary when no `--focus` is specified.
    #[command(name = "github-issue")]
    GitHubIssue {
        /// Issue number.
        number: u64,

        /// What to observe. Can be specified multiple times.
        #[arg(long, value_enum)]
        focus: Vec<IssueFocusArg>,
    },

    /// Observe a GitHub repository.
    ///
    /// Lists open issues, pull requests, or both.
    /// Defaults to both when no `--focus` is specified.
    #[command(name = "github-repo")]
    GitHubRepository {
        /// What to observe. Can be specified multiple times.
        #[arg(long, value_enum)]
        focus: Vec<RepoFocusArg>,
    },
}

/// CLI-facing PR focus, mapped to the domain `PullRequestFocus`.
#[derive(Debug, Clone, ValueEnum)]
pub enum PrFocusArg {
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

impl PrFocusArg {
    fn to_domain(&self) -> PullRequestFocus {
        match self {
            Self::Summary => PullRequestFocus::Summary,
            Self::Files => PullRequestFocus::Files,
            Self::Checks => PullRequestFocus::Checks,
            Self::Diff => PullRequestFocus::Diff,
            Self::Comments => PullRequestFocus::Comments,
            Self::Reviews => PullRequestFocus::Reviews,
        }
    }
}

/// CLI-facing issue focus, mapped to the domain `IssueFocus`.
#[derive(Debug, Clone, ValueEnum)]
pub enum IssueFocusArg {
    /// Issue metadata: title, state, author, labels, assignees.
    Summary,
    /// Issue comments.
    Comments,
}

impl IssueFocusArg {
    fn to_domain(&self) -> IssueFocus {
        match self {
            Self::Summary => IssueFocus::Summary,
            Self::Comments => IssueFocus::Comments,
        }
    }
}

/// CLI-facing repository focus, mapped to the domain `RepositoryFocus`.
#[derive(Debug, Clone, ValueEnum)]
pub enum RepoFocusArg {
    /// Open issues.
    Issues,
    /// Open pull requests.
    PullRequests,
}

impl RepoFocusArg {
    fn to_domain(&self) -> RepositoryFocus {
        match self {
            Self::Issues => RepositoryFocus::Issues,
            Self::PullRequests => RepositoryFocus::PullRequests,
        }
    }
}

/// Run the CLI, returning an error message on failure.
pub fn run(config: &Config, storage: &Storage) -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Command::Voyage { command } => match command {
            VoyageCommand::New { identity, intent } => {
                cmd_new(config, storage, identity.as_deref(), &intent)
            }
            VoyageCommand::List => cmd_list(storage),
            VoyageCommand::End { status } => {
                let voyage = require_voyage(storage, cli.voyage.as_deref())?;
                cmd_end(storage, &voyage, status.as_deref())
            }
        },
        Command::Observe { ref source, out } => {
            let voyage = require_voyage(storage, cli.voyage.as_deref())?;
            cmd_observe(&voyage, source, out)
        }
        Command::Steer { .. } => Err("steer not yet implemented".to_string()),
        Command::Log { .. } => Err("log not yet implemented".to_string()),
    }
}

/// Require that `--voyage` was provided and resolve it.
fn require_voyage(storage: &Storage, voyage_ref: Option<&str>) -> Result<Voyage, String> {
    let voyage_ref = voyage_ref.ok_or("this command requires --voyage <id>")?;
    resolve_voyage(storage, voyage_ref)
}

fn cmd_new(
    config: &Config,
    storage: &Storage,
    identity: Option<&str>,
    intent: &str,
) -> Result<(), String> {
    let identity = identity.unwrap_or(&config.default_identity);

    let voyage = Voyage {
        id: Uuid::new_v4(),
        identity: identity.to_string(),
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
        println!("{short_id}  [{status}] [{}]  {}", v.identity, v.intent);
    }

    Ok(())
}

fn cmd_observe(
    voyage: &Voyage,
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
                focus: focus.iter().map(PrFocusArg::to_domain).collect(),
            },
            true,
        ),
        ObserveSource::GitHubIssue { number, focus } => (
            Observe::GitHubIssue {
                number: *number,
                focus: focus.iter().map(IssueFocusArg::to_domain).collect(),
            },
            true,
        ),
        ObserveSource::GitHubRepository { focus } => (
            Observe::GitHubRepository {
                focus: focus.iter().map(RepoFocusArg::to_domain).collect(),
            },
            true,
        ),
    };

    let gh_config = if needs_gh {
        Some(gh_config_dir(&voyage.identity)?)
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
            let focuses =
                format_pr_focuses(&focus.iter().map(PrFocusArg::to_domain).collect::<Vec<_>>());
            format!("PR #{number} [{focuses}]")
        }
        ObserveSource::GitHubIssue { number, focus } => {
            let focuses = format_issue_focuses(
                &focus
                    .iter()
                    .map(IssueFocusArg::to_domain)
                    .collect::<Vec<_>>(),
            );
            format!("issue #{number} [{focuses}]")
        }
        ObserveSource::GitHubRepository { focus } => {
            let focuses = format_repo_focuses(
                &focus
                    .iter()
                    .map(RepoFocusArg::to_domain)
                    .collect::<Vec<_>>(),
            );
            format!("repository [{focuses}]")
        }
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
