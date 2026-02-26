//! Observation logic: read the world, produce sightings.
//!
//! Each source kind has its own submodule that knows how to observe a mark
//! and return a sighting.

mod files;
mod rust_project;

pub use files::observe_files;
pub use rust_project::observe_rust_project;

use crate::model::{Mark, Sighting};

/// Observe a mark and return what was seen.
///
/// Pure observation — reads the world but never modifies it.
pub fn observe(mark: &Mark) -> Sighting {
    match mark {
        Mark::Files { list, read } => observe_files(list, read),
        Mark::RustProject { root } => observe_rust_project(root),
    }
}
