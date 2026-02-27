//! Identity resolution for Helm commands.
//!
//! Every command that touches GitHub or writes to the logbook needs to know who is acting.
//! Rather than requiring `--as` on every invocation, identity is resolved through a chain:
//!
//! 1. `--as <identity>` — explicit per-command override
//! 2. `HELM_IDENTITY` env var — process/session level (set once per agent)
//! 3. `~/.helm/config.toml` — global default for single-identity users
//!
//! A resolved identity is a plain string that drives two things simultaneously:
//! logbook attribution and GitHub credential routing via `GH_CONFIG_DIR`.

use std::{env, fs};

use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    identity: Option<String>,
}

/// Error message shown when identity cannot be resolved.
pub const IDENTITY_REQUIRED: &str = "identity required: pass --as <identity>, \
    set HELM_IDENTITY, or add `identity = \"...\"` to ~/.helm/config.toml";

/// Resolve the acting identity from the tiered resolution chain.
///
/// Checks in order: explicit `--as` value, `HELM_IDENTITY` env var,
/// `~/.helm/config.toml`. Returns an error with [`IDENTITY_REQUIRED`]
/// when none of the sources yield a value.
pub fn resolve_identity(explicit: Option<&str>) -> Result<String, String> {
    // 1. Explicit --as flag.
    if let Some(id) = explicit {
        return Ok(id.to_string());
    }

    // 2. HELM_IDENTITY environment variable.
    if let Ok(id) = env::var("HELM_IDENTITY")
        && !id.is_empty()
    {
        return Ok(id);
    }

    // 3. ~/.helm/config.toml.
    if let Some(id) = read_config_identity()? {
        return Ok(id);
    }

    Err(IDENTITY_REQUIRED.to_string())
}

/// Read the `identity` field from `~/.helm/config.toml`, if it exists.
fn read_config_identity() -> Result<Option<String>, String> {
    let Some(home) = dirs::home_dir() else {
        return Ok(None);
    };

    let path = home.join(".helm").join("config.toml");

    let contents = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("failed to read {}: {e}", path.display())),
    };

    let config: Config = toml::from_str(&contents)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;

    Ok(config.identity.filter(|s| !s.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_wins() {
        // When an explicit identity is provided, it is returned immediately.
        // We can test this without touching the env or filesystem.
        let result = resolve_identity(Some("dyreby"));
        assert_eq!(result.unwrap(), "dyreby");
    }
}
