//! Bearing construction: creating observations from targets.
//!
//! Wraps the observe module to produce `Observation` values with timestamps.
//! Bearing curation (sealing observations into a bearing on steer/log) is
//! implemented in the steer and log commands.

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
