//! Observation: a single look at the world and what came back.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use super::{observe::Observe, payload::Payload};

/// A single observation: what was looked at and what came back.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    /// What was looked at.
    pub target: Observe,

    /// What came back.
    pub payload: Payload,

    /// When the observation was made.
    pub observed_at: Timestamp,
}
