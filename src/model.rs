//! Core data model for Helm.
//!
//! These types represent the conceptual architecture:
//! voyages, bearings, observations, sightings, readings, and actions.

mod action;
mod bearing;
mod reading;
mod source;
mod voyage;

use serde::{Deserialize, Serialize};

pub use action::{Act, Action, IssueAct, PullRequestAct};
pub use bearing::Bearing;
pub use reading::Reading;
pub use source::{
    DirectoryEntry, DirectoryListing, FileContent, FileContents, Mark, Observation, Sighting,
};
pub use voyage::{Voyage, VoyageKind, VoyageStatus};

/// A single entry in the logbook, serialized as one line of JSONL.
///
/// Tagged enum so each line is self-describing when read back.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "entry", rename_all = "camelCase")]
pub enum LogbookEntry {
    /// A bearing was taken.
    Bearing(Bearing),

    /// An action was performed.
    Action(Action),
}
