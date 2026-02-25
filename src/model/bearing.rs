//! Bearing types: immutable records of observation.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{position::Position, source::SourceQuery};

/// An immutable record of observation: what was planned, what was seen, and what it means.
///
/// The moment (raw observation data) is stored separately in `moments.jsonl` and linked by `id`.
/// The bearing in the logbook carries the plan, position, and timestamps —
/// everything needed to tell the story.
/// The moment is available for deeper inspection when present, but may be pruned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bearing {
    /// Unique identifier. Links this bearing to its moment in `moments.jsonl`.
    pub id: Uuid,

    /// The plan that was executed.
    pub plan: BearingPlan,

    /// The agent's or user's read on the state of the world.
    pub position: Position,

    /// When the bearing was sealed (position attached, recorded to logbook).
    pub taken_at: Timestamp,
}

/// What to observe, described as scope and focus per source kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BearingPlan {
    pub sources: Vec<SourceQuery>,
}
