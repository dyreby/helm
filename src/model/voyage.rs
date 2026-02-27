//! Voyage types: the unit of work in Helm.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A unit of work with intent, logbook, and outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Voyage {
    pub id: Uuid,

    /// The identity sailing this voyage (e.g. `"john-agent"`, `"dyreby"`).
    ///
    /// Set at creation — from `--as` or the configured default identity.
    /// Inherited by commands on this voyage that need GitHub auth.
    pub identity: String,

    pub intent: String,
    pub created_at: Timestamp,
    pub status: VoyageStatus,
}

/// Where a voyage stands in its lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum VoyageStatus {
    /// Work is in progress.
    Active,

    /// The voyage is complete — returned to port, logbook sealed.
    Ended {
        /// When the voyage ended.
        ended_at: Timestamp,

        /// Freeform status: what was accomplished, learned, or left open.
        status: Option<String>,
    },
}
