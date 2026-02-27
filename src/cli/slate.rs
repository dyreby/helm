//! Slate management commands: list, clear, erase.

use clap::Subcommand;

use crate::{model::Voyage, storage::Storage};

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

    /// Remove a single observation from the slate by target.
    ///
    /// Same target syntax as `helm observe`.
    /// Exits cleanly if the target is not on the slate.
    Erase {
        /// Voyage ID: full UUID or unambiguous prefix (e.g. `a3b`).
        #[arg(long)]
        voyage: String,

        /// What to erase — same syntax as `helm observe`.
        #[command(subcommand)]
        target: ObserveTarget,
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

pub(super) fn cmd_erase(
    storage: &Storage,
    voyage: &Voyage,
    target: &ObserveTarget,
) -> Result<(), String> {
    let observe = target.to_observe();
    let erased = storage
        .erase_slate(voyage.id, &observe)
        .map_err(|e| format!("failed to erase from slate: {e}"))?;

    if erased {
        eprintln!("Erased: {}", target.description());
    }

    Ok(())
}

pub(super) fn cmd_clear(storage: &Storage, voyage: &Voyage) -> Result<(), String> {
    storage
        .clear_slate(voyage.id)
        .map_err(|e| format!("failed to clear slate: {e}"))?;

    eprintln!("Slate cleared");
    Ok(())
}
