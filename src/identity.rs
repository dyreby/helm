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

use std::{env, fs, path::Path};

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
    let env_val = env::var("HELM_IDENTITY").ok();
    let home = dirs::home_dir();
    let config_path = home.as_deref().map(|h| h.join(".helm").join("config.toml"));
    resolve_inner(explicit, env_val.as_deref(), config_path.as_deref())
}

/// Inner resolution logic — separated from environment I/O for testability.
fn resolve_inner(
    explicit: Option<&str>,
    env_val: Option<&str>,
    config_path: Option<&Path>,
) -> Result<String, String> {
    // 1. Explicit --as flag.
    if let Some(id) = explicit {
        return Ok(id.to_string());
    }

    // 2. HELM_IDENTITY environment variable.
    if let Some(id) = env_val.filter(|s| !s.is_empty()) {
        return Ok(id.to_string());
    }

    // 3. ~/.helm/config.toml.
    if let Some(path) = config_path
        && let Some(id) = read_config_identity(path)?
    {
        return Ok(id);
    }

    Err(IDENTITY_REQUIRED.to_string())
}

/// Read the `identity` field from the given config path, if it exists.
fn read_config_identity(path: &Path) -> Result<Option<String>, String> {
    let contents = match fs::read_to_string(path) {
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
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;

    fn write_config(dir: &TempDir, contents: &str) -> PathBuf {
        let path = dir.path().join("config.toml");
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn explicit_wins_over_all() {
        let tmp = TempDir::new().unwrap();
        let config = write_config(&tmp, "identity = \"from-config\"\n");
        let result = resolve_inner(Some("explicit"), Some("from-env"), Some(&config));
        assert_eq!(result.unwrap(), "explicit");
    }

    #[test]
    fn env_var_used_when_no_explicit() {
        let result = resolve_inner(None, Some("from-env"), None);
        assert_eq!(result.unwrap(), "from-env");
    }

    #[test]
    fn env_var_wins_over_config() {
        let tmp = TempDir::new().unwrap();
        let config = write_config(&tmp, "identity = \"from-config\"\n");
        let result = resolve_inner(None, Some("from-env"), Some(&config));
        assert_eq!(result.unwrap(), "from-env");
    }

    #[test]
    fn empty_env_var_falls_through_to_config() {
        let tmp = TempDir::new().unwrap();
        let config = write_config(&tmp, "identity = \"from-config\"\n");
        let result = resolve_inner(None, Some(""), Some(&config));
        assert_eq!(result.unwrap(), "from-config");
    }

    #[test]
    fn config_used_when_no_explicit_or_env() {
        let tmp = TempDir::new().unwrap();
        let config = write_config(&tmp, "identity = \"from-config\"\n");
        let result = resolve_inner(None, None, Some(&config));
        assert_eq!(result.unwrap(), "from-config");
    }

    #[test]
    fn empty_identity_in_config_falls_through() {
        let tmp = TempDir::new().unwrap();
        let config = write_config(&tmp, "identity = \"\"\n");
        let result = resolve_inner(None, None, Some(&config));
        assert_eq!(result.unwrap_err(), IDENTITY_REQUIRED);
    }

    #[test]
    fn missing_config_file_is_not_an_error() {
        let tmp = TempDir::new().unwrap();
        let nonexistent = tmp.path().join("config.toml");
        let result = resolve_inner(None, None, Some(&nonexistent));
        assert_eq!(result.unwrap_err(), IDENTITY_REQUIRED);
    }

    #[test]
    fn no_sources_returns_required_error() {
        let result = resolve_inner(None, None, None);
        assert_eq!(result.unwrap_err(), IDENTITY_REQUIRED);
    }

    #[test]
    fn malformed_config_returns_parse_error() {
        let tmp = TempDir::new().unwrap();
        let config = write_config(&tmp, "not valid toml ][[\n");
        let result = resolve_inner(None, None, Some(&config));
        assert!(result.unwrap_err().contains("failed to parse"));
    }

    #[test]
    fn config_without_identity_field_falls_through() {
        let tmp = TempDir::new().unwrap();
        let config = write_config(&tmp, "some_other_field = \"value\"\n");
        let result = resolve_inner(None, None, Some(&config));
        assert_eq!(result.unwrap_err(), IDENTITY_REQUIRED);
    }
}
