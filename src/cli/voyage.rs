//! Voyage lifecycle commands: new, list, end.

use clap::Subcommand;
use jiff::Timestamp;
use uuid::Uuid;

use crate::{
    model::{Voyage, VoyageStatus},
    storage::Storage,
};

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

pub(super) fn cmd_new(storage: &Storage, intent: &str) -> Result<(), String> {
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

pub(super) fn cmd_list(storage: &Storage) -> Result<(), String> {
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

pub(super) fn cmd_end(
    storage: &Storage,
    voyage: &Voyage,
    status: Option<&str>,
) -> Result<(), String> {
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
