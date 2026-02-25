//! Observation logic: read the world, produce structured observations.
//!
//! Each source kind has its own submodule that knows how to execute
//! a query against the relevant domain and return an observation.

mod files;

pub use files::observe_files;

use crate::model::{Observation, SourceQuery};

/// Execute a source query and return what was observed.
///
/// Pure observation — reads the world but never modifies it.
pub fn observe(query: &SourceQuery) -> Observation {
    match query {
        SourceQuery::Files { scope, focus } => observe_files(scope, focus),
    }
}
