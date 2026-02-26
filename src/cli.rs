//! CLI interface for Helm.
//!
//! Designed for agents and humans alike to record voyages and bearings from the command line.
//! Each subcommand is non-interactive: arguments in, structured output out.
//!
//! Commands are organized by clarity:
//!
//! - `helm voyage` — lifecycle commands grouped under the domain concept.
//! - `helm observe`, `helm record` — flat verbs, unambiguous on their own.
//! - `helm act` — perform and record an action.
//!
//! Observing outputs an observation (mark + sighting) to stdout or a file.
//! Recording selects observations, attaches a reading, and writes the bearing to the logbook.
//! Acting performs git/GitHub operations and records them in the logbook.

use std::path::PathBuf;
use std::{fs, io, process};

// Trait must be in scope for `.read_to_string()` on stdin.
use io::Read;

use clap::{Parser, Subcommand, ValueEnum};
use jiff::Timestamp;
use uuid::Uuid;

use crate::model::{
    Action, ActionKind, IssueAction, LogbookEntry, Mark, Observation, PullRequestAction, Voyage,
    VoyageKind, VoyageStatus,
};
use crate::{bearing, storage::Storage};

/// Helm — navigate your work.
#[derive(Debug, Parser)]
#[command(name = "helm", after_long_help = WORKFLOW_HELP)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

const WORKFLOW_HELP: &str = r#"Workflow: resolving an issue
  1. helm voyage new "Resolve #42: fix widget crash" --kind resolve-issue
     → prints a voyage ID (e.g. a3b0fc12)
  2. Do the work — fix the bug, open the PR, get it merged.
  3. helm voyage complete a3b --summary "Fixed null check in widget init"

Stopping mid-voyage? Record a bearing so the next session has context:
  helm observe rust-project . --out obs.json
  helm record a3b --reading "Halfway through, refactoring widget module" --observation obs.json

Record actions:
  helm act a3b --as john-agent commit --message "Fix null check in widget init"
  helm act a3b --as john-agent push --branch fix-widget
  helm act a3b --as john-agent create-pull-request --branch fix-widget --title "Fix widget"
  helm act a3b --as john-agent merge-pull-request 45

Check on voyages:
  helm voyage list           → see active voyages
  helm voyage log a3b        → see the trail of bearings and actions"#;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage voyages: units of work with intent and a logbook.
    Voyage {
        #[command(subcommand)]
        command: VoyageCommand,
    },

    /// Observe the world and output what was seen.
    ///
    /// Pure read, no side effects, repeatable.
    /// The observation is written as JSON to `--out` (if given) or stdout.
    /// A human-readable summary is printed to stderr when writing to a file.
    Observe {
        #[command(subcommand)]
        source: ObserveSource,

        /// Write the observation JSON to this file instead of stdout.
        #[arg(long, global = true)]
        out: Option<PathBuf>,
    },

    /// Record a bearing: attach a reading to one or more observations.
    ///
    /// Reads observations from `--observation` files or stdin (single observation),
    /// attaches the reading, and writes the bearing to the logbook.
    ///
    /// Bearings exist for continuity, not just documentation.
    /// Record one when you'd need context if you had to stop and come back
    /// in a new session. If you're finishing in this session, skip the bearing
    /// and use `helm voyage complete --summary` instead.
    Record {
        /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b` if only one ID starts with that).
        voyage: String,

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
    /// Each action records a single act (push, create PR, merge, comment, etc.)
    /// and the identity that performed it. The logbook captures what happened,
    /// not what was planned — failed operations are not recorded.
    ///
    /// Identity selects which GitHub account to use for `gh` commands.
    /// Git operations use ambient git config.
    Act {
        /// Voyage ID: full UUID or unambiguous prefix.
        voyage: String,

        /// Identity to act as (e.g. "dyreby", "john-agent").
        /// Selects the GitHub config directory for `gh` commands.
        #[arg(long = "as")]
        identity: String,

        /// The act to perform.
        #[command(subcommand)]
        act: ActCommand,
    },
}

/// Act subcommands — one per kind of action.
#[derive(Debug, Subcommand)]
pub enum ActCommand {
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
        /// What this voyage is about.
        intent: String,

        /// The kind of voyage.
        #[arg(long, value_enum, default_value_t = VoyageKindArg::OpenWaters)]
        kind: VoyageKindArg,
    },

    /// List active voyages.
    List,

    /// Show a voyage's logbook: the trail of bearings and actions.
    ///
    /// Displays observations and readings for each bearing,
    /// and identity/act for each action.
    /// The logbook tells the story through readings and actions.
    Log {
        /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b` if only one ID starts with that).
        voyage: String,
    },

    /// Mark a voyage as completed.
    ///
    /// Updates the voyage status to completed.
    /// The summary captures the outcome — what was accomplished or learned.
    /// If the voyage finishes in a single session, this is often the only
    /// record needed (no bearing required).
    Complete {
        /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b` if only one ID starts with that).
        voyage: String,

        /// Summary of what was accomplished or learned.
        #[arg(long)]
        summary: Option<String>,
    },
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
    /// Observe a Rust project: full directory tree and documentation.
    RustProject {
        /// Path to the project root.
        path: PathBuf,
    },

    /// Observe the filesystem: list directories and read files.
    Files {
        /// Directories to list (immediate contents with metadata).
        #[arg(long)]
        list: Vec<PathBuf>,

        /// Files to read (full contents).
        #[arg(long)]
        read: Vec<PathBuf>,
    },
}

/// Run the CLI, returning an error message on failure.
pub fn run(storage: &Storage) -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Command::Voyage { command } => match command {
            VoyageCommand::New { intent, kind } => cmd_new(storage, &intent, &kind),
            VoyageCommand::List => cmd_list(storage),
            VoyageCommand::Log { voyage } => cmd_log(storage, &voyage),
            VoyageCommand::Complete { voyage, summary } => {
                cmd_complete(storage, &voyage, summary.as_deref())
            }
        },
        Command::Observe { ref source, out } => cmd_observe(source, out),
        Command::Record {
            voyage,
            reading,
            observation,
        } => cmd_record(storage, &voyage, &reading, &observation),
        Command::Act {
            voyage,
            identity,
            act,
        } => cmd_act(storage, &voyage, &identity, &act),
    }
}

fn cmd_new(storage: &Storage, intent: &str, kind: &VoyageKindArg) -> Result<(), String> {
    let voyage = Voyage {
        id: Uuid::new_v4(),
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
        println!("{short_id}  [{status}] [{kind}]  {}", v.intent);
    }

    Ok(())
}

fn cmd_observe(source: &ObserveSource, out: Option<PathBuf>) -> Result<(), String> {
    let mark = match source {
        ObserveSource::RustProject { path } => Mark::RustProject { root: path.clone() },
        ObserveSource::Files { list, read } => {
            if list.is_empty() && read.is_empty() {
                return Err("specify at least one --list or --read".to_string());
            }
            Mark::Files {
                list: list.clone(),
                read: read.clone(),
            }
        }
    };

    let observation = bearing::observe(&mark);

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

fn cmd_record(
    storage: &Storage,
    voyage_ref: &str,
    reading: &str,
    observation_paths: &[PathBuf],
) -> Result<(), String> {
    let voyage = resolve_voyage(storage, voyage_ref)?;

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
        .map_err(|e| format!("failed to record bearing: {e}"))?;

    // Write bearing to logbook.
    storage
        .append_entry(voyage.id, &LogbookEntry::Bearing(sealed.clone()))
        .map_err(|e| format!("failed to save bearing: {e}"))?;

    eprintln!(
        "Bearing recorded for voyage {}",
        &voyage.id.to_string()[..8]
    );
    eprintln!("Reading: {reading}");

    Ok(())
}

fn cmd_act(
    storage: &Storage,
    voyage_ref: &str,
    identity: &str,
    act_cmd: &ActCommand,
) -> Result<(), String> {
    let voyage = resolve_voyage(storage, voyage_ref)?;

    if matches!(voyage.status, VoyageStatus::Completed { .. }) {
        return Err(format!(
            "voyage {} is already completed",
            &voyage.id.to_string()[..8]
        ));
    }

    let gh_config = gh_config_dir(identity)?;
    let act = act(act_cmd, &gh_config)?;

    let action = Action {
        id: Uuid::new_v4(),
        identity: identity.to_string(),
        kind: act,
        performed_at: Timestamp::now(),
    };

    storage
        .append_entry(voyage.id, &LogbookEntry::Action(action.clone()))
        .map_err(|e| format!("failed to save action: {e}"))?;

    let short_id = &voyage.id.to_string()[..8];
    eprintln!("Action recorded for voyage {short_id}");
    eprintln!("  as: {identity}");
    eprintln!("  {}", format_act(&action.kind));

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

/// Dispatch the act command and return the structured `ActionKind` on success.
fn act(act_cmd: &ActCommand, gh_config: &PathBuf) -> Result<ActionKind, String> {
    match act_cmd {
        ActCommand::Commit { message } => commit(message),
        ActCommand::Push { branch } => push(branch),
        ActCommand::CreatePullRequest {
            branch,
            title,
            body,
            base,
            reviewer,
        } => create_pr(gh_config, branch, title, body.as_deref(), base, reviewer),
        ActCommand::MergePullRequest { number } => merge_pr(gh_config, *number),
        ActCommand::CommentOnPullRequest { number, body } => comment_pr(gh_config, *number, body),
        ActCommand::ReplyOnPullRequest {
            number,
            comment_id,
            body,
        } => reply_pr(gh_config, *number, *comment_id, body),
        ActCommand::RequestReview { number, reviewer } => {
            request_review(gh_config, *number, reviewer)
        }
        ActCommand::CreateIssue { title, body } => create_issue(gh_config, title, body.as_deref()),
        ActCommand::CloseIssue { number } => close_issue(gh_config, *number),
        ActCommand::CommentOnIssue { number, body } => comment_issue(gh_config, *number, body),
    }
}

fn commit(message: &str) -> Result<ActionKind, String> {
    run_cmd("git", &["commit", "-m", message], None)?;

    let sha = run_cmd_output("git", &["rev-parse", "HEAD"], None)?;

    Ok(ActionKind::Commit { sha })
}

fn push(branch: &str) -> Result<ActionKind, String> {
    run_cmd("git", &["push", "origin", branch], None)?;

    let sha = run_cmd_output("git", &["rev-parse", "HEAD"], None)?;

    Ok(ActionKind::Push {
        branch: branch.to_string(),
        sha,
    })
}

fn create_pr(
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

fn merge_pr(gh_config: &PathBuf, number: u64) -> Result<ActionKind, String> {
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

fn comment_pr(gh_config: &PathBuf, number: u64, body: &str) -> Result<ActionKind, String> {
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

fn reply_pr(
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

fn request_review(
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

fn create_issue(
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

fn close_issue(gh_config: &PathBuf, number: u64) -> Result<ActionKind, String> {
    let num_str = number.to_string();
    run_cmd("gh", &["issue", "close", &num_str], Some(gh_config))?;

    Ok(ActionKind::Issue {
        number,
        action: IssueAction::Close,
    })
}

fn comment_issue(gh_config: &PathBuf, number: u64, body: &str) -> Result<ActionKind, String> {
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
fn run_cmd(program: &str, args: &[&str], gh_config: Option<&PathBuf>) -> Result<(), String> {
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
fn run_cmd_output(
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
fn detect_repo() -> Result<String, String> {
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
fn parse_pr_number_from_url(url: &str) -> Result<u64, String> {
    url.rsplit('/')
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("could not parse PR number from: {url}"))
}

/// Parse an issue number from a GitHub issue URL (e.g. `https://github.com/owner/repo/issues/45`).
fn parse_issue_number_from_url(url: &str) -> Result<u64, String> {
    url.rsplit('/')
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("could not parse issue number from: {url}"))
}

/// Format an act for human-readable display.
fn format_act(act: &ActionKind) -> String {
    match act {
        ActionKind::Commit { sha } => {
            format!("committed ({sha})")
        }
        ActionKind::Push { branch, sha } => {
            format!("pushed to {branch} ({sha})")
        }
        ActionKind::PullRequest { number, action } => {
            let verb = match action {
                PullRequestAction::Create => "created",
                PullRequestAction::Merge => "merged",
                PullRequestAction::Comment => "commented on",
                PullRequestAction::Reply => "replied on",
                PullRequestAction::RequestedReview { .. } => "requested review on",
            };
            format!("{verb} PR #{number}")
        }
        ActionKind::Issue { number, action } => {
            let verb = match action {
                IssueAction::Create => "created",
                IssueAction::Close => "closed",
                IssueAction::Comment => "commented on",
            };
            format!("{verb} issue #{number}")
        }
    }
}

fn cmd_log(storage: &Storage, voyage_ref: &str) -> Result<(), String> {
    let voyage = resolve_voyage(storage, voyage_ref)?;

    println!("Voyage: {}", voyage.intent);
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
                        Mark::Files { list, read } => {
                            println!("  Mark: Files");
                            for l in list {
                                println!("    list: {}", l.display());
                            }
                            for r in read {
                                println!("    read: {}", r.display());
                            }
                        }
                        Mark::RustProject { root } => {
                            println!("  Mark: RustProject @ {}", root.display());
                        }
                    }
                }
                println!("  Reading: {}", b.reading.text);
                println!();
            }
            LogbookEntry::Action(a) => {
                println!("── Action {} ── {}", i + 1, a.performed_at);
                println!("  as: {}", a.identity);
                println!("  {}", format_act(&a.kind));
                println!();
            }
        }
    }

    Ok(())
}

fn cmd_complete(storage: &Storage, voyage_ref: &str, summary: Option<&str>) -> Result<(), String> {
    let mut voyage = resolve_voyage(storage, voyage_ref)?;

    if matches!(voyage.status, VoyageStatus::Completed { .. }) {
        return Err(format!(
            "voyage {} is already completed",
            &voyage.id.to_string()[..8]
        ));
    }

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

    #[test]
    fn format_commit_action_kind() {
        let kind = ActionKind::Commit {
            sha: "abc1234".to_string(),
        };
        assert_eq!(format_act(&kind), "committed (abc1234)");
    }

    #[test]
    fn format_push_action_kind() {
        let kind = ActionKind::Push {
            branch: "main".to_string(),
            sha: "abc1234".to_string(),
        };
        assert_eq!(format_act(&kind), "pushed to main (abc1234)");
    }

    #[test]
    fn format_pr_action_kinds() {
        let cases = [
            (PullRequestAction::Create, "created PR #10"),
            (PullRequestAction::Merge, "merged PR #10"),
            (PullRequestAction::Comment, "commented on PR #10"),
            (PullRequestAction::Reply, "replied on PR #10"),
            (
                PullRequestAction::RequestedReview {
                    reviewers: vec!["alice".to_string()],
                },
                "requested review on PR #10",
            ),
        ];
        for (pr_action, expected) in cases {
            let kind = ActionKind::PullRequest {
                number: 10,
                action: pr_action,
            };
            assert_eq!(format_act(&kind), expected);
        }
    }

    #[test]
    fn format_issue_action_kinds() {
        let cases = [
            (IssueAction::Create, "created issue #5"),
            (IssueAction::Close, "closed issue #5"),
            (IssueAction::Comment, "commented on issue #5"),
        ];
        for (issue_action, expected) in cases {
            let kind = ActionKind::Issue {
                number: 5,
                action: issue_action,
            };
            assert_eq!(format_act(&kind), expected);
        }
    }
}
