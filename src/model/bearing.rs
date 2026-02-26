//! Bearing types: immutable records of observation.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use super::{position::Position, source::Observation};

/// An immutable record: what was observed, and what it means.
///
/// A bearing collects the observations you chose to keep
/// and seals them with your position — your read on what you saw.
/// Observations you took but discarded are simply not included.
///
/// Bearings are identified by position in the logbook stream, not by ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bearing {
    /// The observations that inform this bearing.
    pub observations: Vec<Observation>,

    /// The agent's or user's read on the state of the world.
    pub position: Position,

    /// When the bearing was sealed (position attached, recorded to logbook).
    pub taken_at: Timestamp,
}
