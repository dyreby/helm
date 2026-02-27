//! Observation logic: read the world, produce sightings.
//!
//! Each source kind has its own submodule that knows how to observe a mark
//! and return a sighting.

pub mod directory_tree;
mod file_contents;
pub mod github;
mod rust_project;

pub use directory_tree::observe_directory_tree;
pub use file_contents::observe_file_contents;
pub use github::{observe_github_issue, observe_github_pull_request, observe_github_repository};
pub use rust_project::observe_rust_project;

use std::path::Path;

use crate::model::{Mark, Sighting};

/// Observe a mark and return what was seen.
///
/// Pure observation — reads the world but never modifies it.
/// GitHub marks require a `gh_config_dir` for authentication.
pub fn observe(mark: &Mark, gh_config_dir: Option<&Path>) -> Sighting {
    match mark {
        Mark::FileContents { paths } => observe_file_contents(paths),
        Mark::DirectoryTree {
            root,
            skip,
            max_depth,
        } => observe_directory_tree(root, skip, *max_depth),
        Mark::RustProject { root } => observe_rust_project(root),
        Mark::GitHubPullRequest { number, focus } => {
            let config = gh_config_dir.expect("GitHub marks require gh_config_dir");
            observe_github_pull_request(*number, focus, config)
        }
        Mark::GitHubIssue { number, focus } => {
            let config = gh_config_dir.expect("GitHub marks require gh_config_dir");
            observe_github_issue(*number, focus, config)
        }
        Mark::GitHubRepository { focus } => {
            let config = gh_config_dir.expect("GitHub marks require gh_config_dir");
            observe_github_repository(focus, config)
        }
    }
}
