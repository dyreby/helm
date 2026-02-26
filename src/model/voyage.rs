//! Voyage types: the unit of work in Helm.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A unit of work with intent, logbook, and outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voyage {
    pub id: Uuid,
    pub kind: VoyageKind,
    pub intent: String,
    pub created_at: Timestamp,
    pub status: VoyageStatus,
}

/// The kind of voyage, which frames the first bearing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoyageKind {
    /// Unscoped, general-purpose voyage.
    /// No prescribed framing.
    OpenWaters,

    /// Resolve a GitHub issue.
    /// Frames the voyage around understanding and closing a specific issue.
    ResolveIssue,
}

/// Where a voyage stands in its lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoyageStatus {
    /// Work is in progress.
    Active,

    /// The voyage is complete — returned to port, logbook sealed.
    Completed {
        /// When the voyage was completed.
        completed_at: Timestamp,

        /// What was accomplished or learned.
        summary: Option<String>,
    },
}
