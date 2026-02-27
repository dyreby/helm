//! Slate management commands: list, clear.

use clap::Subcommand;

use crate::{model::Voyage, storage::Storage};

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

    eprintln!("Slate cleared");
    Ok(())
}
