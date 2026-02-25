//! CLI interface for Helm.
//!
//! Designed for agents and humans to record voyages and bearings from the command line.
//! Each subcommand is non-interactive: arguments in, structured output out.
//!
//! The core bearing flow is two commands:
//!
//! 1. `helm observe` — run a plan, output the moment (what was observed).
//! 2. `helm record` — attach a position to a moment, seal the bearing.
//!
//! `helm observe` writes the moment to a file (`--out`) or stdout.
//! The caller decides how to handle it.
//! `helm record` reads the moment back from a file (`--moment`) or stdin.

use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::bearing::{observe_bearing, record_bearing};
use crate::model::{
    BearingPlan, LogbookEntry, MomentRecord, SourceQuery, Voyage, VoyageKind, VoyageStatus,
};
use crate::storage::Storage;

/// Helm — navigate your work.
#[derive(Debug, Parser)]
#[command(name = "helm")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
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

    /// Observe the world: run a plan and output what was seen.
    ///
    /// Pure read, no side effects, repeatable.
    /// The moment is written as JSON to `--out` (if given) or stdout.
    /// A human-readable summary is printed to stderr when writing to a file.
    Observe {
        #[command(subcommand)]
        source: ObserveSource,

        /// Write the moment JSON to this file instead of stdout.
        #[arg(long, global = true)]
        out: Option<PathBuf>,
    },

    /// Record a bearing: attach a position to an observation.
    ///
    /// Reads the moment from `--moment` (file) or stdin, attaches the position,
    /// and writes the bearing to the logbook and the moment to `moments.jsonl`.
    Record {
        /// Voyage ID (full UUID or unambiguous prefix).
        voyage: String,

        /// Your read on the state of the world.
        position: String,

        /// Path to the moment JSON file (from `helm observe --out`).
        /// Reads from stdin if not provided.
        #[arg(long)]
        moment: Option<PathBuf>,
    },

    /// Show a voyage's logbook: the trail of bearings and actions.
    ///
    /// Displays plans and positions for each bearing, and outcomes for each action.
    /// The logbook tells the story through positions.
    Log {
        /// Voyage ID (full UUID or unambiguous prefix).
        voyage: String,
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
        Command::New { intent, kind } => cmd_new(storage, &intent, &kind),
        Command::List => cmd_list(storage),
        Command::Observe { ref source, out } => cmd_observe(source, out),
        Command::Record {
            voyage,
            position,
            moment,
        } => cmd_record(storage, &voyage, &position, moment),
        Command::Log { voyage } => cmd_log(storage, &voyage),
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
            VoyageStatus::Paused => "paused",
            VoyageStatus::Completed => "completed",
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
    let plan = match source {
        ObserveSource::RustProject { path } => BearingPlan {
            sources: vec![SourceQuery::RustProject { root: path.clone() }],
        },
        ObserveSource::Files { scope, focus } => {
            if scope.is_empty() && focus.is_empty() {
                return Err("specify at least one --scope or --focus".to_string());
            }
            BearingPlan {
                sources: vec![SourceQuery::Files {
                    scope: scope.clone(),
                    focus: focus.clone(),
                }],
            }
        }
    };

    let moment_record = observe_bearing(&plan).map_err(|e| format!("observation failed: {e}"))?;

    // Serialize the observation output: plan + moment record together,
    // so `helm record` has everything it needs.
    let output = ObservationOutput {
        plan,
        moment_record,
    };

    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| format!("failed to serialize moment: {e}"))?;

    match out {
        Some(path) => {
            fs::write(&path, &json)
                .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
            // Summary to stderr so the agent sees it regardless.
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
    moment_path: Option<PathBuf>,
) -> Result<(), String> {
    let voyage = resolve_voyage(storage, voyage_ref)?;

    // Read the observation output from file or stdin.
    let json = if let Some(path) = moment_path {
        fs::read_to_string(&path).map_err(|e| format!("failed to read {}: {e}", path.display()))?
    } else {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("failed to read stdin: {e}"))?;
        buf
    };

    let output: ObservationOutput =
        serde_json::from_str(&json).map_err(|e| format!("invalid moment JSON: {e}"))?;

    // Seal the bearing.
    let bearing = record_bearing(output.plan, &output.moment_record, position.to_string())
        .map_err(|e| format!("failed to record bearing: {e}"))?;

    // Write bearing to logbook and moment to moments file.
    storage
        .append_entry(voyage.id, &LogbookEntry::Bearing(bearing.clone()))
        .map_err(|e| format!("failed to save bearing: {e}"))?;

    storage
        .save_moment(voyage.id, &output.moment_record)
        .map_err(|e| format!("failed to save moment: {e}"))?;

    let short_id = &bearing.id.to_string()[..8];
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
    println!("Status: {:?}", voyage.status);
    println!("Created: {}", voyage.created_at);
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
                for source in &b.plan.sources {
                    match source {
                        SourceQuery::Files { scope, focus } => {
                            println!("  Source: Files");
                            for s in scope {
                                println!("    scope: {}", s.display());
                            }
                            for f in focus {
                                println!("    focus: {}", f.display());
                            }
                        }
                        SourceQuery::RustProject { root } => {
                            println!("  Source: RustProject @ {}", root.display());
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

/// The full output of `helm observe`, consumed by `helm record`.
///
/// Bundles the plan and moment record so `helm record` has everything it needs
/// to seal a bearing without re-observing.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObservationOutput {
    plan: BearingPlan,
    moment_record: MomentRecord,
}
