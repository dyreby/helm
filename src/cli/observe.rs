//! Observe command: gather observations onto the slate.

use std::{fs, path::PathBuf};

use crate::{bearing, identity, model::Voyage, storage::Storage};

use super::target::ObserveTarget;

pub(super) fn cmd_observe(
    storage: &Storage,
    voyage: &Voyage,
    identity: Option<&str>,
    target: &ObserveTarget,
    out: Option<PathBuf>,
) -> Result<(), String> {
    let observe = target.to_observe();

    if let crate::model::Observe::FileContents { paths } = &observe
        && paths.is_empty()
    {
        return Err("specify at least one --read".to_string());
    }

    let gh_config = if observe.needs_gh() {
        let id = identity::resolve_identity(identity)?;
        Some(super::gh_config_dir(&id)?)
    } else {
        None
    };

    let observation = bearing::observe(&observe, gh_config.as_deref());

    let json = serde_json::to_string_pretty(&observation)
        .map_err(|e| format!("failed to serialize observation: {e}"))?;

    storage
        .append_slate(voyage.id, &observation)
        .map_err(|e| format!("failed to append to slate: {e}"))?;

    match out {
        Some(path) => {
            fs::write(&path, &json)
                .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
            eprintln!("Observed {} → {}", target.description(), path.display());
        }
        None => {
            println!("{json}");
        }
    }

    Ok(())
}
