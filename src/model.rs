//! Core data model for Helm.
//!
//! These types represent the conceptual architecture:
//! voyages, bearings, observations, sightings, positions, and actions.

mod action;
mod bearing;
mod position;
mod source;
mod voyage;

use serde::{Deserialize, Serialize};

pub use action::{Act, Action, IssueAct, PullRequestAct};
pub use bearing::Bearing;
// TODO: remove allow once TUI or CLI consumes position history.
#[allow(unused_imports)]
pub use position::{Position, PositionAttempt, PositionSource};
pub use source::{
    DirectoryEntry, DirectorySurvey, FileContent, FileInspection, Observation, Sighting, Subject,
};
pub use voyage::{Voyage, VoyageKind, VoyageStatus};

/// A single entry in the logbook, serialized as one line of JSONL.
///
/// Tagged enum so each line is self-describing when read back.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LogbookEntry {
    /// A bearing was taken.
    Bearing(Bearing),

    /// An action was performed.
    Action(Action),
}
