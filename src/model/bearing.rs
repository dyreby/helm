//! Bearing types: immutable records of observation.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use super::{reading::Reading, source::Mark};

/// An immutable record: what was observed, and what it means.
///
/// A bearing records the marks (what you looked at) and a reading
/// (what you made of it). Observations — the raw sightings — are
/// stored as separate prunable artifacts, referenced by ID.
///
/// Bearings are identified by position in the logbook stream, not by ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bearing {
    /// The marks that inform this bearing — what was looked at.
    pub marks: Vec<Mark>,

    /// References to stored observations, by voyage-scoped ID.
    /// Each ID corresponds to a file in the voyage's `observations/` directory.
    pub observation_refs: Vec<u64>,

    /// The interpretation of what was observed.
    pub reading: Reading,

    /// When the bearing was sealed (reading attached, recorded to logbook).
    pub taken_at: Timestamp,
}
