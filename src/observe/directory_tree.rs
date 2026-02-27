//! `DirectoryTree` source kind: recursive directory walk with filtering.
//!
//! Walks a directory tree, respects `.gitignore`, and supports
//! skip patterns (directory names to skip at any depth) and
//! optional depth limits.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use ignore::WalkBuilder;

use crate::model::{DirectoryEntry, DirectoryListing, Sighting};

/// Walk a directory tree recursively with filtering.
///
/// Respects `.gitignore` by default. Shows dotfiles (like `.github/`).
/// `skip` names directories to skip at any depth.
/// `max_depth` limits recursion (`None` = unlimited).
///
/// Produces one `DirectoryListing` per directory in the tree.
/// Entries are sorted by name within each listing.
pub fn observe_directory_tree(root: &Path, skip: &[String], max_depth: Option<u32>) -> Sighting {
    let listings = walk_tree(root, skip, max_depth);
    Sighting::DirectoryTree { listings }
}

/// Walk the tree and collect listings.
///
/// Shared with `RustProject` observation, which adds doc reading
/// on top of the same tree walk.
pub fn walk_tree(root: &Path, skip: &[String], max_depth: Option<u32>) -> Vec<DirectoryListing> {
    let mut dir_entries: BTreeMap<PathBuf, Vec<DirectoryEntry>> = BTreeMap::new();

    let skip_owned: Vec<String> = skip.to_vec();
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false) // Show dotfiles (like .github/).
        .filter_entry(move |entry| {
            // Skip directories whose name matches a skip pattern.
            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                let name = entry.file_name().to_string_lossy();
                if skip_owned.iter().any(|s| s == name.as_ref()) {
                    return false;
                }
            }
            true
        })
        .sort_by_file_name(Ord::cmp);

    if let Some(depth) = max_depth {
        builder.max_depth(Some(depth as usize));
    }

    let walker = builder.build();

    for entry in walker.flatten() {
        let path = entry.path();

        if path == root {
            continue;
        }

        let metadata = entry.metadata().ok();
        let is_dir = metadata.as_ref().is_some_and(fs::Metadata::is_dir);

        // Record this entry under its parent directory.
        if let Some(parent) = path.parent() {
            let dir_entry = DirectoryEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir,
                size_bytes: if is_dir {
                    None
                } else {
                    metadata.as_ref().map(fs::Metadata::len)
                },
            };
            dir_entries
                .entry(parent.to_path_buf())
                .or_default()
                .push(dir_entry);
        }
    }

    dir_entries
        .into_iter()
        .map(|(path, entries): (PathBuf, Vec<DirectoryEntry>)| DirectoryListing { path, entries })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use tempfile::TempDir;

    fn setup_tree() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        fs::write(root.join("README.md"), "# Hello").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::create_dir(root.join("src/util")).unwrap();
        fs::write(root.join("src/util/helpers.rs"), "pub fn help() {}").unwrap();
        fs::create_dir(root.join("target")).unwrap();
        fs::write(root.join("target/debug.txt"), "artifact").unwrap();
        fs::create_dir(root.join("node_modules")).unwrap();
        fs::write(root.join("node_modules/pkg.json"), "{}").unwrap();
        fs::write(root.join(".gitignore"), "").unwrap();

        dir
    }

    #[test]
    fn walks_full_tree() {
        let dir = setup_tree();

        let Sighting::DirectoryTree { listings } = observe_directory_tree(dir.path(), &[], None)
        else {
            panic!("expected DirectoryTree sighting");
        };

        // Should have listings for root, src, src/util, target, node_modules.
        assert!(listings.len() >= 4);

        let root_listing = listings.iter().find(|l| l.path == dir.path()).unwrap();
        let names: Vec<&str> = root_listing
            .entries
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert!(names.contains(&"README.md"));
        assert!(names.contains(&"src"));
    }

    #[test]
    fn skip_directories() {
        let dir = setup_tree();

        let Sighting::DirectoryTree { listings } = observe_directory_tree(
            dir.path(),
            &["target".to_string(), "node_modules".to_string()],
            None,
        ) else {
            panic!("expected DirectoryTree sighting");
        };

        // target/ and node_modules/ should not appear.
        assert!(
            !listings
                .iter()
                .any(|l| l.path.ends_with("target") || l.path.ends_with("node_modules"))
        );

        let root_listing = listings.iter().find(|l| l.path == dir.path()).unwrap();
        let names: Vec<&str> = root_listing
            .entries
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert!(!names.contains(&"target"));
        assert!(!names.contains(&"node_modules"));
    }

    #[test]
    fn max_depth_limits_recursion() {
        let dir = setup_tree();

        // Depth 1 = root entries only.
        let Sighting::DirectoryTree { listings } = observe_directory_tree(dir.path(), &[], Some(1))
        else {
            panic!("expected DirectoryTree sighting");
        };

        // Only the root directory should have a listing.
        assert_eq!(listings.len(), 1);
        assert_eq!(listings[0].path, dir.path());

        // Depth 2 = root + immediate subdirectories.
        let Sighting::DirectoryTree { listings } = observe_directory_tree(dir.path(), &[], Some(2))
        else {
            panic!("expected DirectoryTree sighting");
        };

        // Should have root and its subdirectories, but not src/util/.
        assert!(listings.iter().any(|l| l.path == dir.path()));
        assert!(listings.iter().any(|l| l.path == dir.path().join("src")));
        assert!(
            !listings
                .iter()
                .any(|l| l.path == dir.path().join("src/util"))
        );
    }

    #[test]
    fn nested_structure_preserved() {
        let dir = setup_tree();

        let Sighting::DirectoryTree { listings } =
            observe_directory_tree(dir.path(), &["target".to_string()], None)
        else {
            panic!("expected DirectoryTree sighting");
        };

        let src_listing = listings
            .iter()
            .find(|l| l.path == dir.path().join("src"))
            .unwrap();
        let names: Vec<&str> = src_listing
            .entries
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert!(names.contains(&"main.rs"));
        assert!(names.contains(&"util"));

        let util_listing = listings
            .iter()
            .find(|l| l.path == dir.path().join("src/util"))
            .unwrap();
        assert_eq!(util_listing.entries.len(), 1);
        assert_eq!(util_listing.entries[0].name, "helpers.rs");
    }

    #[test]
    fn empty_directory_produces_root_only() {
        let dir = TempDir::new().unwrap();

        let Sighting::DirectoryTree { listings } = observe_directory_tree(dir.path(), &[], None)
        else {
            panic!("expected DirectoryTree sighting");
        };

        // Empty directory: no entries means no listings
        // (the root itself has nothing to list).
        assert!(listings.is_empty());
    }

    #[test]
    fn skip_and_max_depth_combined() {
        let dir = setup_tree();

        // Skip node_modules, limit to depth 2 (root + immediate children).
        let Sighting::DirectoryTree { listings } =
            observe_directory_tree(dir.path(), &["node_modules".to_string()], Some(2))
        else {
            panic!("expected DirectoryTree sighting");
        };

        // node_modules should not appear.
        let root_listing = listings.iter().find(|l| l.path == dir.path()).unwrap();
        let names: Vec<&str> = root_listing
            .entries
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert!(!names.contains(&"node_modules"));

        // Depth 2 means src/ contents are visible but src/util/ is not.
        assert!(listings.iter().any(|l| l.path == dir.path().join("src")));
        assert!(
            !listings
                .iter()
                .any(|l| l.path == dir.path().join("src/util"))
        );
    }

    // ── Dispatch test ──

    #[test]
    fn observe_dispatches_directory_tree_mark() {
        let dir = setup_tree();
        let mark = crate::model::Mark::DirectoryTree {
            root: dir.path().to_path_buf(),
            skip: vec!["target".to_string()],
            max_depth: None,
        };

        let sighting = crate::observe::observe(&mark, None);
        assert!(matches!(sighting, Sighting::DirectoryTree { .. }));
    }
}
