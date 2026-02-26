//! Rust project source kind: project structure and documentation.
//!
//! Walks a Rust project tree, respects `.gitignore`, skips `target/`.
//! Produces a full directory tree (listings) and reads documentation files (contents).
//! Source code is not read — that's what `Files` is for on subsequent bearings.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use ignore::WalkBuilder;

use crate::model::{DirectoryEntry, DirectoryListing, FileContent, FileContents, Sighting};

/// Well-known documentation file names (case-insensitive matching).
///
/// This list will evolve as Helm encounters more project conventions.
const DOC_NAMES: &[&str] = &[
    "readme",
    "readme.md",
    "changelog",
    "changelog.md",
    "vision.md",
    "contributing",
    "contributing.md",
    "license",
    "license.md",
    "license-mit",
    "license-apache",
    "agents.md",
    "claude.md",
    "copilot-instructions.md",
    "cursorrules",
    ".cursorrules",
];

/// Returns true if a file is a documentation file.
///
/// Matches well-known doc names (case-insensitive) and any `.md` file in the project root
/// or a `docs/` directory.
fn is_doc_file(path: &Path, root: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    // Well-known doc names anywhere in the tree.
    if DOC_NAMES.iter().any(|d| name.eq_ignore_ascii_case(d)) {
        return true;
    }

    // Any .md file in the project root or docs/.
    let is_md = Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));

    if is_md
        && let Some(parent) = path.parent()
        && (parent == root
            || parent
                .file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case("docs")))
    {
        return true;
    }

    false
}

/// Observe a Rust project: full directory tree and all documentation.
///
/// Uses the `ignore` crate to respect `.gitignore` and skips `target/`.
/// The tree gives structure; docs give intent and context.
/// Source files are left for targeted `Files` queries on subsequent bearings.
pub fn observe_rust_project(root: &Path) -> Sighting {
    let mut dir_entries: BTreeMap<PathBuf, Vec<DirectoryEntry>> = BTreeMap::new();
    let mut contents: Vec<FileContents> = Vec::new();

    let walker = WalkBuilder::new(root)
        .hidden(false) // Show dotfiles (like .github/).
        .filter_entry(|entry| {
            // Skip target/ at any level.
            !(entry.file_type().is_some_and(|ft| ft.is_dir()) && entry.file_name() == "target")
        })
        .sort_by_file_name(Ord::cmp)
        .build();

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

        // Read documentation files only.
        if !is_dir && is_doc_file(path, root) {
            let content = match fs::read(path) {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(text) => FileContent::Text { content: text },
                    Err(e) => FileContent::Binary {
                        size_bytes: e.into_bytes().len() as u64,
                    },
                },
                Err(e) => FileContent::Error {
                    message: e.to_string(),
                },
            };
            contents.push(FileContents {
                path: path.to_path_buf(),
                content,
            });
        }
    }

    let listings = dir_entries
        .into_iter()
        .map(|(path, entries)| DirectoryListing { path, entries })
        .collect();

    Sighting::Files { listings, contents }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use tempfile::TempDir;

    /// Create a Rust project with docs and source files.
    fn setup_rust_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(root.join("README.md"), "# Test Project").unwrap();
        fs::write(root.join("VISION.md"), "# Vision").unwrap();

        fs::create_dir(root.join("docs")).unwrap();
        fs::write(root.join("docs/design.md"), "# Design doc").unwrap();

        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn hello() {}").unwrap();

        fs::create_dir(root.join("target")).unwrap();
        fs::write(root.join("target/debug.txt"), "build artifact").unwrap();

        fs::write(root.join(".gitignore"), "/target\n").unwrap();

        dir
    }

    #[test]
    fn tree_includes_all_non_ignored_entries() {
        let dir = setup_rust_project();
        let Sighting::Files {
            listings,
            contents: _,
        } = observe_rust_project(dir.path());

        // Root should have src/, docs/, Cargo.toml, README.md, etc. but not target/.
        let root_listing = listings.iter().find(|s| s.path == dir.path()).unwrap();
        let names: Vec<&str> = root_listing
            .entries
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert!(names.contains(&"Cargo.toml"));
        assert!(names.contains(&"src"));
        assert!(names.contains(&"docs"));
        assert!(names.contains(&"README.md"));
        assert!(!names.contains(&"target"));
    }

    #[test]
    fn reads_only_docs() {
        let dir = setup_rust_project();
        let Sighting::Files { contents, .. } = observe_rust_project(dir.path());

        let paths: Vec<String> = contents
            .iter()
            .map(|i| {
                i.path
                    .strip_prefix(dir.path())
                    .unwrap()
                    .display()
                    .to_string()
            })
            .collect();

        // Docs are read.
        assert!(paths.contains(&"README.md".to_string()));
        assert!(paths.contains(&"VISION.md".to_string()));
        assert!(paths.contains(&"docs/design.md".to_string()));

        // Source files are not.
        assert!(!paths.contains(&"src/main.rs".to_string()));
        assert!(!paths.contains(&"src/lib.rs".to_string()));
        assert!(!paths.contains(&"Cargo.toml".to_string()));
    }

    #[test]
    fn skips_target_directory() {
        let dir = setup_rust_project();
        let Sighting::Files { listings, contents } = observe_rust_project(dir.path());

        assert!(!listings.iter().any(|s| s.path.ends_with("target")));
        assert!(!contents.iter().any(|i| {
            i.path
                .strip_prefix(dir.path())
                .unwrap()
                .starts_with("target")
        }));
    }

    #[test]
    fn tree_includes_nested_structure() {
        let dir = setup_rust_project();
        let Sighting::Files {
            listings,
            contents: _,
        } = observe_rust_project(dir.path());

        // src/ directory should have its own listing.
        let src_listing = listings
            .iter()
            .find(|s| s.path == dir.path().join("src"))
            .unwrap();
        let names: Vec<&str> = src_listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"main.rs"));
        assert!(names.contains(&"lib.rs"));
    }
}
