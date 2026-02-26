//! Observation logic: read the world, produce sightings.
//!
//! Each source kind has its own submodule that knows how to observe a subject
//! and return a sighting.

mod files;
mod rust_project;

pub use files::observe_files;
pub use rust_project::observe_rust_project;

use crate::model::{Sighting, Subject};

/// Observe a subject and return what was seen.
///
/// Pure observation — reads the world but never modifies it.
pub fn observe(subject: &Subject) -> Sighting {
    match subject {
        Subject::Files { scope, focus } => observe_files(scope, focus),
        Subject::RustProject { root } => observe_rust_project(root),
    }
}
