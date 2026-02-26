//! CLI interface for Helm.
//!
//! Designed for agents and humans alike to record voyages and bearings from the command line.
//! Each subcommand is non-interactive: arguments in, structured output out.
//!
//! Commands are organized by clarity:
//!
//! - `helm voyage` — lifecycle commands grouped under the domain concept.
//! - `helm observe`, `helm record` — flat verbs, unambiguous on their own.
//!
//! Observing outputs an observation (subject + sighting) to stdout or a file.
//! Recording selects observations, attaches a position, and writes the bearing to the logbook.

use std::{fs, io, path::PathBuf};

// Trait must be in scope for `.read_to_string()` on stdin.
use io::Read;

use clap::{Parser, Subcommand, ValueEnum};
use uuid::Uuid;

use crate::{
    bearing,
    model::{LogbookEntry, Observation, Subject, Voyage, VoyageKind, VoyageStatus},
    storage::Storage,
};

/// Helm — navigate your work.
#[derive(Debug, Parser)]
#[command(name = "helm")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

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

    /// Record a bearing: attach a position to one or more observations.
    ///
    /// Reads observations from `--observation` files or stdin (single observation),
    /// attaches the position, and writes the bearing to the logbook.
    Record {
        /// Voyage ID.
        /// Full UUID or unambiguous prefix (e.g. `a3b` if only one ID starts with that).
        voyage: String,

        /// Your read on the state of the world.
        position: String,

        /// Paths to observation JSON files (from `helm observe --out`).
        /// Pass multiple times for multi-observation bearings.
        /// Reads a single observation from stdin if not provided.
        #[arg(long)]
        observation: Vec<PathBuf>,
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
    /// Displays observations and positions for each bearing, and outcomes for each action.
    /// The logbook tells the story through positions.
    Log {
        /// Voyage ID.
        /// Full UUID or unambiguous prefix (e.g. `a3b` if only one ID starts with that).
        voyage: String,
    },

    /// Mark a voyage as completed.
    ///
    /// Updates the voyage status and records a completion entry in the logbook.
    /// Optionally accepts a summary of what was accomplished or learned.
    Complete {
        /// Voyage ID.
        /// Full UUID or unambiguous prefix (e.g. `a3b` if only one ID starts with that).
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

    /// Observe the filesystem: survey directories and inspect files.
    Files {
        /// Directories to survey (list contents with metadata).
        #[arg(long)]
        scope: Vec<PathBuf>,

        /// Files to inspect (read full contents).
        #[arg(long)]
        focus: Vec<PathBuf>,
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
            position,
            observation,
        } => cmd_record(storage, &voyage, &position, &observation),
    }
}

fn cmd_new(storage: &Storage, intent: &str, kind: &VoyageKindArg) -> Result<(), String> {
    let voyage = Voyage {
        id: Uuid::new_v4(),
        kind: kind.to_domain(),
        intent: intent.to_string(),
        created_at: jiff::Timestamp::now(),
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
        println!("No active voyages.");
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
    let subject = match source {
        ObserveSource::RustProject { path } => Subject::RustProject { root: path.clone() },
        ObserveSource::Files { scope, focus } => {
            if scope.is_empty() && focus.is_empty() {
                return Err("specify at least one --scope or --focus".to_string());
            }
            Subject::Files {
                scope: scope.clone(),
                focus: focus.clone(),
            }
        }
    };

    let observation = bearing::observe(&subject);

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
    position: &str,
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

    // Seal the bearing.
    let sealed = bearing::record_bearing(observations, position.to_string())
        .map_err(|e| format!("failed to record bearing: {e}"))?;

    // Write bearing to logbook.
    storage
        .append_entry(voyage.id, &LogbookEntry::Bearing(sealed.clone()))
        .map_err(|e| format!("failed to save bearing: {e}"))?;

    let short_id = &sealed.id.to_string()[..8];
    eprintln!(
        "Bearing {short_id} recorded for voyage {}",
        &voyage.id.to_string()[..8]
    );
    eprintln!("Position: {position}");

    Ok(())
}

fn cmd_log(storage: &Storage, voyage_ref: &str) -> Result<(), String> {
    let voyage = resolve_voyage(storage, voyage_ref)?;

    println!("Voyage: {}", voyage.intent);
    println!("Created: {}", voyage.created_at);
    match &voyage.status {
        VoyageStatus::Active => println!("Status: active"),
        VoyageStatus::Completed { completed_at, summary } => {
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
        println!("Logbook is empty.");
        return Ok(());
    }

    for (i, entry) in entries.iter().enumerate() {
        match entry {
            LogbookEntry::Bearing(b) => {
                let short_id = &b.id.to_string()[..8];
                println!("── Bearing {} ({short_id}) ── {}", i + 1, b.taken_at);
                for obs in &b.observations {
                    match &obs.subject {
                        Subject::Files { scope, focus } => {
                            println!("  Subject: Files");
                            for s in scope {
                                println!("    scope: {}", s.display());
                            }
                            for f in focus {
                                println!("    focus: {}", f.display());
                            }
                        }
                        Subject::RustProject { root } => {
                            println!("  Subject: RustProject @ {}", root.display());
                        }
                    }
                }
                println!("  Position: {}", b.position.text);
                println!();
            }
            LogbookEntry::ActionReport(r) => {
                println!("── Action {} ── {}", i + 1, r.completed_at);
                println!("  Outcome: {:?}", r.outcome);
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
        completed_at: jiff::Timestamp::now(),
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
