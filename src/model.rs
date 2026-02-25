//! Core data model for Helm.
//!
//! These types represent the conceptual architecture from VISION.md:
//! voyages, bearings, plans, moments, positions, and actions.

#![allow(dead_code, unused_imports)]

mod action;
mod bearing;
mod position;
mod source;
mod voyage;

pub use action::{ActionOutcome, ActionPlan, ActionReport, FileEdit, FileWrite};
pub use bearing::{Bearing, BearingPlan};
pub use position::{Position, PositionAttempt, PositionSource};
pub use source::{
    DirectoryEntry, DirectorySurvey, FileContent, FileInspection, Moment, Observation, SourceQuery,
};
pub use voyage::{Voyage, VoyageKind, VoyageStatus};

/// A single entry in the logbook, serialized as one line of JSONL.
///
/// Tagged enum so each line is self-describing when read back.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum LogbookEntry {
    /// A bearing was taken.
    Bearing(Bearing),

    /// An action was performed.
    ActionReport(ActionReport),
}
