//! Bearing types: immutable records of observation.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use super::{reading::Reading, source::Observation};

/// An immutable record: what was observed, and what it means.
///
/// A bearing collects the observations you chose to keep
/// and seals them with your reading — your interpretation of what you saw.
/// Observations you took but discarded are simply not included.
///
/// Bearings are identified by position in the logbook stream, not by ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bearing {
    /// The observations that inform this bearing.
    pub observations: Vec<Observation>,

    /// The interpretation of what was observed.
    pub reading: Reading,

    /// When the bearing was sealed (reading attached, recorded to logbook).
    pub taken_at: Timestamp,
}
