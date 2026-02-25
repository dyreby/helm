//! Bearing types: immutable records of observation.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use super::{
    position::Position,
    source::{Moment, SourceQuery},
};

/// An immutable record of observation: what was planned, what was seen,
/// and what it means.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bearing {
    pub plan: BearingPlan,
    pub moment: Moment,
    pub position: Position,
    pub taken_at: Timestamp,
}

/// What to observe, described as scope and focus per source kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BearingPlan {
    pub sources: Vec<SourceQuery>,
}
