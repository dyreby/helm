//! Bearing: orientation at the moment of decision.

use serde::{Deserialize, Serialize};

use super::observation::Observation;

/// Orientation at the moment of decision.
///
/// Curated from the working set when steer or log is called.
/// One bearing per log entry — many observations feed into
/// one understanding of where you are.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bearing {
    /// The observations that informed this decision.
    pub observations: Vec<Observation>,

    /// Freeform interpretation of the current state.
    pub summary: String,
}
