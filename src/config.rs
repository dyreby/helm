//! Helm configuration.
//!
//! Loaded from `~/.helm/config.toml`. Created with defaults if missing.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Helm configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    /// The default identity for new voyages.
    /// Used when `--as` is not provided on `voyage new`.
    pub default_identity: String,
}

impl Config {
    /// Load config from `~/.helm/config.toml`.
    /// Returns an error if the file is missing or invalid.
    pub fn load() -> Result<Self, String> {
        let path = Self::path().ok_or("could not determine home directory")?;

        if !path.exists() {
            return Err(format!(
                "no config file found at {}\n\
                 Create one with at minimum:\n\n\
                 default-identity = \"your-github-username\"",
                path.display()
            ));
        }

        let contents = fs::read_to_string(&path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

        let config: Self = toml::from_str(&contents)
            .map_err(|e| format!("invalid config at {}: {e}", path.display()))?;

        if config.default_identity.is_empty() {
            return Err(format!(
                "default-identity is empty in {}\n\
                 Set it to your GitHub username.",
                path.display()
            ));
        }

        Ok(config)
    }

    /// The config file path: `~/.helm/config.toml`.
    pub fn path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".helm").join("config.toml"))
    }
}
