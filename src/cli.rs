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
//! Identity is set at voyage creation and inherited by all commands.
//! The `--voyage` flag takes a full UUID or unambiguous prefix.

mod actions;
mod format;

use std::path::PathBuf;
use std::{fs, io};

// Trait must be in scope for `.read_to_string()` on stdin.
use io::Read;

use clap::{Parser, Subcommand, ValueEnum};
use jiff::Timestamp;
use uuid::Uuid;

use crate::config::Config;
use crate::model::{
    Action, IssueFocus, LogbookEntry, Mark, Observation, PullRequestFocus, RepositoryFocus, Voyage,
    VoyageKind, VoyageStatus,
};
use crate::{bearing, storage::Storage};

use actions::perform;
use format::{format_action, format_issue_focuses, format_pr_focuses, format_repo_focuses};

/// Helm — navigate your work.
#[derive(Debug, Parser)]
#[command(name = "helm", after_long_help = WORKFLOW_HELP)]
pub struct Cli {
    /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b`).
    /// Required for observe, bearing, action, complete, and log.
    #[arg(long, global = true)]
    voyage: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

const WORKFLOW_HELP: &str = r#"Workflow: resolving an issue
  1. helm voyage new --as john-agent --kind resolve-issue "Resolve #42: fix widget crash"
     → prints a voyage ID (e.g. a3b0fc12)
  2. Do the work — fix the bug, open the PR, get it merged.
  3. helm --voyage a3b complete --summary "Fixed null check in widget init"

Stopping mid-voyage? Take a bearing so the next session has context:
  helm --voyage a3b observe rust-project . --out obs.json
  helm --voyage a3b observe file-contents --read src/widget.rs --out widget.json
  helm --voyage a3b bearing --reading "Halfway through, refactoring widget module" --observation obs.json --observation widget.json

Observe GitHub:
  helm --voyage a3b observe github-pr 42 --focus summary --focus diff
  helm --voyage a3b observe github-issue 10 --focus comments
  helm --voyage a3b observe github-repo --focus issues

Perform actions:
  helm --voyage a3b action commit --message "Fix null check in widget init"
  helm --voyage a3b action push --branch fix-widget
  helm --voyage a3b action create-pull-request --branch fix-widget --title "Fix widget"
  helm --voyage a3b action merge-pull-request 45

Check on voyages:
  helm voyage list              → see active voyages
  helm --voyage a3b log         → see the trail of bearings and actions"#;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage voyages: create new ones, list existing.
    Voyage {
        #[command(subcommand)]
        command: VoyageCommand,
    },

    /// Observe the world and output what was seen.
    ///
    /// Pure read, no side effects, repeatable.
    /// The observation is written as JSON to `--out` (if given) or stdout.
    /// A human-readable summary is printed to stderr when writing to a file.
    /// Requires `--voyage`.
    Observe {
        #[command(subcommand)]
        source: ObserveSource,

        /// Write the observation JSON to this file instead of stdout.
        #[arg(long, global = true)]
        out: Option<PathBuf>,
    },

    /// Take a bearing: attach a reading to one or more observations.
    ///
    /// Reads observations from `--observation` files or stdin (single observation),
    /// attaches the reading, and writes the bearing to the logbook.
    ///
    /// Bearings exist for continuity, not just documentation.
    /// Take one when you'd need context if you had to stop and come back
    /// in a new session. If you're finishing in this session, skip the bearing
    /// and use `helm --voyage <id> complete --summary` instead.
    /// Requires `--voyage`.
    Bearing {
        /// Your reading of the observed mark.
        #[arg(long)]
        reading: String,

        /// Paths to observation JSON files (from `helm observe --out`).
        /// Pass multiple times for multi-observation bearings.
        /// Reads a single observation from stdin if not provided.
        #[arg(long)]
        observation: Vec<PathBuf>,
    },

    /// Perform an action.
    ///
    /// Each action performs a single operation (push, create PR, merge, comment, etc.)
    /// and records it in the logbook.
    /// Identity is inherited from the voyage.
    /// The logbook captures what happened, not what was planned —
    /// failed operations are not recorded.
    /// Requires `--voyage`.
    Action {
        /// The action to perform.
        #[command(subcommand)]
        action: ActionCommand,
    },

    /// Mark a voyage as completed.
    ///
    /// Updates the voyage status to completed.
    /// The summary captures the outcome — what was accomplished or learned.
    /// If the voyage finishes in a single session, this is often the only
    /// record needed (no bearing required).
    /// Requires `--voyage`.
    Complete {
        /// Summary of what was accomplished or learned.
        #[arg(long)]
        summary: Option<String>,
    },

    /// Show a voyage's logbook: the trail of bearings and actions.
    ///
    /// Displays observations and readings for each bearing,
    /// and identity/act for each action.
    /// The logbook tells the story through readings and actions.
    /// Requires `--voyage`.
    Log,
}

/// Action subcommands — one per kind of action.
#[derive(Debug, Subcommand)]
pub enum ActionCommand {
    /// Commit staged changes.
    ///
    /// Commits whatever is currently staged.
    /// Records the resulting commit SHA in the logbook.
    Commit {
        /// Commit message.
        #[arg(long)]
        message: String,
    },

    /// Push commits to a branch.
    Push {
        /// Branch name.
        #[arg(long)]
        branch: String,
    },

    /// Create a pull request.
    CreatePullRequest {
        /// Branch to create the PR from.
        #[arg(long)]
        branch: String,

        /// PR title.
        #[arg(long)]
        title: String,

        /// PR body.
        #[arg(long)]
        body: Option<String>,

        /// Base branch (defaults to main).
        #[arg(long, default_value = "main")]
        base: String,

        /// Request review from these users.
        #[arg(long)]
        reviewer: Vec<String>,
    },

    /// Merge a pull request (squash merge).
    MergePullRequest {
        /// PR number.
        number: u64,
    },

    /// Comment on a pull request.
    CommentOnPullRequest {
        /// PR number.
        number: u64,

        /// Comment body.
        #[arg(long)]
        body: String,
    },

    /// Reply to an inline review comment on a pull request.
    ReplyOnPullRequest {
        /// PR number.
        number: u64,

        /// The review comment ID to reply to.
        #[arg(long)]
        comment_id: u64,

        /// Reply body.
        #[arg(long)]
        body: String,
    },

    /// Request review on a pull request.
    RequestReview {
        /// PR number.
        number: u64,

        /// Users to request review from.
        #[arg(long, required = true)]
        reviewer: Vec<String>,
    },

    /// Create a new issue.
    CreateIssue {
        /// Issue title.
        #[arg(long)]
        title: String,

        /// Issue body.
        #[arg(long)]
        body: Option<String>,
    },

    /// Close an issue.
    CloseIssue {
        /// Issue number.
        number: u64,
    },

    /// Comment on an issue.
    CommentOnIssue {
        /// Issue number.
        number: u64,

        /// Comment body.
        #[arg(long)]
        body: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum VoyageCommand {
    /// Create a new voyage. Prints the voyage ID.
    New {
        /// Identity for this voyage (e.g. "john-agent").
        /// All commands on this voyage inherit this identity for GitHub auth.
        /// When omitted, system default `gh` auth is used.
        #[arg(long = "as")]
        identity: Option<String>,

        /// What this voyage is about.
        intent: String,

        /// The kind of voyage.
        #[arg(long, value_enum, default_value_t = VoyageKindArg::OpenWaters)]
        kind: VoyageKindArg,
    },

    /// List active voyages.
    List,
}

/// CLI-facing voyage kind, mapped to the domain `VoyageKind`.
#[derive(Debug, Clone, ValueEnum)]
pub enum VoyageKindArg {
    /// General-purpose voyage.
    OpenWaters,

    /// Resolve a GitHub issue.
    ResolveIssue,
}

impl VoyageKindArg {
    fn to_domain(&self) -> VoyageKind {
        match self {
            Self::OpenWaters => VoyageKind::OpenWaters,
            Self::ResolveIssue => VoyageKind::ResolveIssue,
        }
    }
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

        /// Directory names to skip at any depth (e.g. "target", "`node_modules`").
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
            VoyageCommand::New {
                identity,
                intent,
                kind,
            } => cmd_new(config, storage, identity.as_deref(), &intent, &kind),
            VoyageCommand::List => cmd_list(storage),
        },
        Command::Observe { ref source, out } => {
            let voyage = require_voyage(storage, cli.voyage.as_deref())?;
            cmd_observe(&voyage, source, out)
        }
        Command::Bearing {
            reading,
            observation,
        } => {
            let voyage = require_voyage(storage, cli.voyage.as_deref())?;
            cmd_bearing(storage, &voyage, &reading, &observation)
        }
        Command::Action { action } => {
            let voyage = require_voyage(storage, cli.voyage.as_deref())?;
            cmd_action(storage, &voyage, &action)
        }
        Command::Complete { summary } => {
            let voyage = require_voyage(storage, cli.voyage.as_deref())?;
            cmd_complete(storage, &voyage, summary.as_deref())
        }
        Command::Log => {
            let voyage = require_voyage(storage, cli.voyage.as_deref())?;
            cmd_log(storage, &voyage)
        }
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
    kind: &VoyageKindArg,
) -> Result<(), String> {
    let identity = identity.unwrap_or(&config.default_identity);

    let voyage = Voyage {
        id: Uuid::new_v4(),
        identity: identity.to_string(),
        kind: kind.to_domain(),
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
        println!("No active voyages");
        return Ok(());
    }

    for v in &voyages {
        let status = match v.status {
            VoyageStatus::Active => "active",
            VoyageStatus::Completed { .. } => "completed",
        };
        let kind = match v.kind {
            VoyageKind::OpenWaters => "open-waters",
            VoyageKind::ResolveIssue => "resolve-issue",
        };
        let short_id = &v.id.to_string()[..8];
        println!(
            "{short_id}  [{status}] [{kind}] [{}]  {}",
            v.identity, v.intent
        );
    }

    Ok(())
}

fn cmd_observe(
    voyage: &Voyage,
    source: &ObserveSource,
    out: Option<PathBuf>,
) -> Result<(), String> {
    let (mark, needs_gh) = match source {
        ObserveSource::FileContents { read } => {
            if read.is_empty() {
                return Err("specify at least one --read".to_string());
            }
            (
                Mark::FileContents {
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
            Mark::DirectoryTree {
                root: root.clone(),
                skip: skip.clone(),
                max_depth: *max_depth,
            },
            false,
        ),
        ObserveSource::RustProject { path } => (Mark::RustProject { root: path.clone() }, false),
        ObserveSource::GitHubPullRequest { number, focus } => (
            Mark::GitHubPullRequest {
                number: *number,
                focus: focus.iter().map(PrFocusArg::to_domain).collect(),
            },
            true,
        ),
        ObserveSource::GitHubIssue { number, focus } => (
            Mark::GitHubIssue {
                number: *number,
                focus: focus.iter().map(IssueFocusArg::to_domain).collect(),
            },
            true,
        ),
        ObserveSource::GitHubRepository { focus } => (
            Mark::GitHubRepository {
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

    let observation = bearing::observe(&mark, gh_config.as_deref());

    let json = serde_json::to_string_pretty(&observation)
        .map_err(|e| format!("failed to serialize observation: {e}"))?;

    match out {
        Some(path) => {
            fs::write(&path, &json)
                .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
            eprintln!("Observation written to {}", path.display());
        }
        None => {
            println!("{json}");
        }
    }

    Ok(())
}

fn cmd_bearing(
    storage: &Storage,
    voyage: &Voyage,
    reading: &str,
    observation_paths: &[PathBuf],
) -> Result<(), String> {
    // Load observations from files or stdin.
    let observations = if observation_paths.is_empty() {
        // Read a single observation from stdin.
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("failed to read stdin: {e}"))?;
        let obs: Observation =
            serde_json::from_str(&buf).map_err(|e| format!("invalid observation JSON: {e}"))?;
        vec![obs]
    } else {
        // Read each observation file.
        observation_paths
            .iter()
            .map(|path| {
                let json = fs::read_to_string(path)
                    .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
                serde_json::from_str(&json)
                    .map_err(|e| format!("invalid observation JSON in {}: {e}", path.display()))
            })
            .collect::<Result<Vec<Observation>, String>>()?
    };

    // Store each observation as a separate artifact, collecting refs.
    let mut observation_refs = Vec::with_capacity(observations.len());
    for obs in &observations {
        let id = storage
            .store_observation(voyage.id, obs)
            .map_err(|e| format!("failed to store observation: {e}"))?;
        observation_refs.push(id);
    }

    // Seal the bearing with marks + refs (no inlined sightings).
    let sealed = bearing::record_bearing(&observations, observation_refs, reading.to_string())
        .map_err(|e| format!("failed to take bearing: {e}"))?;

    // Write bearing to logbook.
    storage
        .append_entry(voyage.id, &LogbookEntry::Bearing(sealed.clone()))
        .map_err(|e| format!("failed to save bearing: {e}"))?;

    eprintln!("Bearing taken for voyage {}", &voyage.id.to_string()[..8]);
    eprintln!("Reading: {reading}");

    Ok(())
}

fn cmd_action(
    storage: &Storage,
    voyage: &Voyage,
    action_cmd: &ActionCommand,
) -> Result<(), String> {
    if matches!(voyage.status, VoyageStatus::Completed { .. }) {
        return Err(format!(
            "voyage {} is already completed",
            &voyage.id.to_string()[..8]
        ));
    }

    let gh_config = gh_config_dir(&voyage.identity)?;
    let act = perform(action_cmd, &gh_config)?;

    let action = Action {
        id: Uuid::new_v4(),
        kind: act,
        performed_at: Timestamp::now(),
    };

    storage
        .append_entry(voyage.id, &LogbookEntry::Action(action.clone()))
        .map_err(|e| format!("failed to save action: {e}"))?;

    let short_id = &voyage.id.to_string()[..8];
    eprintln!("Action performed for voyage {short_id}");
    eprintln!("  {}", format_action(&action.kind));

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

fn cmd_log(storage: &Storage, voyage: &Voyage) -> Result<(), String> {
    println!("Voyage: {}", voyage.intent);
    println!("Identity: {}", voyage.identity);
    println!("Created: {}", voyage.created_at);
    match &voyage.status {
        VoyageStatus::Active => println!("Status: active"),
        VoyageStatus::Completed {
            completed_at,
            summary,
        } => {
            println!("Status: completed ({completed_at})");
            if let Some(s) = summary {
                println!("Summary: {s}");
            }
        }
    }
    println!();

    let entries = storage
        .load_logbook(voyage.id)
        .map_err(|e| format!("failed to load logbook: {e}"))?;

    if entries.is_empty() {
        println!("Logbook is empty");
        return Ok(());
    }

    for (i, entry) in entries.iter().enumerate() {
        match entry {
            LogbookEntry::Bearing(b) => {
                println!("── Bearing {} ── {}", i + 1, b.taken_at);
                for mark in &b.marks {
                    match mark {
                        Mark::FileContents { paths } => {
                            println!("  Mark: FileContents");
                            for p in paths {
                                let path: &PathBuf = p;
                                println!("    read: {}", path.display());
                            }
                        }
                        Mark::DirectoryTree {
                            root,
                            skip,
                            max_depth,
                        } => {
                            print!("  Mark: DirectoryTree @ {}", root.display());
                            if !skip.is_empty() {
                                print!(" (skip: {})", skip.join(", "));
                            }
                            if let Some(depth) = max_depth {
                                print!(" (depth: {depth})");
                            }
                            println!();
                        }
                        Mark::RustProject { root } => {
                            println!("  Mark: RustProject @ {}", root.display());
                        }
                        Mark::GitHubPullRequest { number, focus } => {
                            let focuses = format_pr_focuses(focus);
                            println!("  Mark: GitHub PR #{number} [{focuses}]");
                        }
                        Mark::GitHubIssue { number, focus } => {
                            let focuses = format_issue_focuses(focus);
                            println!("  Mark: GitHub Issue #{number} [{focuses}]");
                        }
                        Mark::GitHubRepository { focus } => {
                            let focuses = format_repo_focuses(focus);
                            println!("  Mark: GitHub Repository [{focuses}]");
                        }
                    }
                }
                println!("  Reading: {}", b.reading.text);
                println!();
            }
            LogbookEntry::Action(a) => {
                println!("── Action {} ── {}", i + 1, a.performed_at);
                println!("  {}", format_action(&a.kind));
                println!();
            }
        }
    }

    Ok(())
}

fn cmd_complete(storage: &Storage, voyage: &Voyage, summary: Option<&str>) -> Result<(), String> {
    if matches!(voyage.status, VoyageStatus::Completed { .. }) {
        return Err(format!(
            "voyage {} is already completed",
            &voyage.id.to_string()[..8]
        ));
    }

    let mut voyage = voyage.clone();
    voyage.status = VoyageStatus::Completed {
        completed_at: Timestamp::now(),
        summary: summary.map(String::from),
    };
    storage
        .update_voyage(&voyage)
        .map_err(|e| format!("failed to update voyage: {e}"))?;

    let short_id = &voyage.id.to_string()[..8];
    eprintln!("Voyage {short_id} completed");
    if let Some(s) = summary {
        eprintln!("Summary: {s}");
    }

    Ok(())
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
