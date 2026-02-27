//! Observation logic: read the world, produce payloads.
//!
//! Each target variant has its own submodule that knows how to observe it
//! and return a payload.

pub mod directory_tree;
mod file_contents;
pub mod github;
mod rust_project;

pub use directory_tree::observe_directory_tree;
pub use file_contents::observe_file_contents;
pub use github::{observe_github_issue, observe_github_pull_request, observe_github_repository};
pub use rust_project::observe_rust_project;

use std::path::Path;

use crate::model::{Observe, Payload};

/// Observe a target and return what came back.
///
/// Pure observation — reads the world but never modifies it.
/// GitHub targets require a `gh_config_dir` for authentication.
pub fn observe(target: &Observe, gh_config_dir: Option<&Path>) -> Payload {
    match target {
        Observe::FileContents { paths } => observe_file_contents(paths),
        Observe::DirectoryTree {
            root,
            skip,
            max_depth,
        } => observe_directory_tree(root, skip, *max_depth),
        Observe::RustProject { root } => observe_rust_project(root),
        Observe::GitHubPullRequest { number, focus } => {
            let config = gh_config_dir.expect("GitHub targets require gh_config_dir");
            observe_github_pull_request(*number, focus, config)
        }
        Observe::GitHubIssue { number, focus } => {
            let config = gh_config_dir.expect("GitHub targets require gh_config_dir");
            observe_github_issue(*number, focus, config)
        }
        Observe::GitHubRepository { focus } => {
            let config = gh_config_dir.expect("GitHub targets require gh_config_dir");
            observe_github_repository(focus, config)
        }
    }
}
