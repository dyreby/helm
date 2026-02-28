//! Bearing construction: observing targets and producing timestamped observations.
//!
//! `observe` wraps the observe module to produce timestamped `Observation` values.
//!
//! Sealing — assembling a `Bearing` from the slate — is handled atomically by
//! the storage layer. See `Storage::record_steer` and `Storage::record_log`.

use std::path::Path;

use crate::model::{Observation, Observe};

/// Observe a target and return a timestamped observation.
///
/// Pure read — never modifies the world.
/// GitHub targets require `gh_config_dir` for authentication.
pub fn observe(target: &Observe, gh_config_dir: Option<&Path>) -> Observation {
    let payload = crate::observe::observe(target, gh_config_dir);

    Observation {
        target: target.clone(),
        payload,
        observed_at: jiff::Timestamp::now(),
    }
}
