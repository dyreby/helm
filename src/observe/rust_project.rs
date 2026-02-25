//! Rust project source kind: full project observation.
//!
//! Walks a Rust project tree, respects `.gitignore`, skips `target/`.
//! Produces a tree-structured survey and inspects all source files.

use std::path::Path;

use ignore::WalkBuilder;

use crate::model::{DirectoryEntry, DirectorySurvey, FileContent, FileInspection, Observation};

/// Observe a Rust project: walk the tree, survey all directories,
/// inspect all files.
///
/// Uses the `ignore` crate to respect `.gitignore` and adds an
/// explicit skip for `target/`. Files that aren't valid UTF-8 are
/// recorded as binary rather than skipped.
pub fn observe_rust_project(root: &Path) -> Observation {
    let mut surveys: Vec<DirectorySurvey> = Vec::new();
    let mut inspections: Vec<FileInspection> = Vec::new();
    let mut dir_entries: std::collections::BTreeMap<std::path::PathBuf, Vec<DirectoryEntry>> =
        std::collections::BTreeMap::new();

    let walker = WalkBuilder::new(root)
        .hidden(false) // Show dotfiles (like .github/).
        .filter_entry(|entry| {
            // Skip target/ at any level.
            if entry.file_type().is_some_and(|ft| ft.is_dir()) && entry.file_name() == "target" {
                return false;
            }
            true
        })
        .sort_by_file_name(std::cmp::Ord::cmp)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();

        // Skip the root itself.
        if path == root {
            continue;
        }

        let metadata = entry.metadata().ok();
        let is_dir = metadata.as_ref().is_some_and(std::fs::Metadata::is_dir);

        // Record this entry under its parent directory.
        if let Some(parent) = path.parent() {
            let dir_entry = DirectoryEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir,
                size_bytes: if is_dir {
                    None
                } else {
                    metadata.as_ref().map(std::fs::Metadata::len)
                },
            };
            dir_entries
                .entry(parent.to_path_buf())
                .or_default()
                .push(dir_entry);
        }

        // Inspect files.
        if !is_dir {
            let content = match std::fs::read(path) {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(text) => FileContent::Text(text),
                    Err(e) => FileContent::Binary {
                        size_bytes: e.into_bytes().len() as u64,
                    },
                },
                Err(e) => FileContent::Error(e.to_string()),
            };
            inspections.push(FileInspection {
                path: path.to_path_buf(),
                content,
            });
        }
    }

    // Convert collected directory entries into surveys.
    for (path, entries) in dir_entries {
        surveys.push(DirectorySurvey { path, entries });
    }

    Observation::Files {
        survey: surveys,
        inspections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use tempfile::TempDir;

    /// Create a minimal Rust project structure.
    fn setup_rust_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Cargo.toml
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();

        // src/
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn hello() {}").unwrap();

        // target/ (should be skipped)
        fs::create_dir(root.join("target")).unwrap();
        fs::write(root.join("target/debug.txt"), "build artifact").unwrap();

        // .gitignore
        fs::write(root.join(".gitignore"), "/target\n").unwrap();

        dir
    }

    #[test]
    fn observes_project_structure() {
        let dir = setup_rust_project();
        let Observation::Files {
            survey,
            inspections: _,
        } = observe_rust_project(dir.path());

        // Should have surveys for root and src/.
        assert!(survey.len() >= 2);

        // Root survey should contain Cargo.toml, src/, .gitignore but not target/.
        let root_survey = survey.iter().find(|s| s.path == dir.path()).unwrap();
        let names: Vec<&str> = root_survey
            .entries
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert!(names.contains(&"Cargo.toml"));
        assert!(names.contains(&"src"));
        assert!(names.contains(&".gitignore"));
        assert!(!names.contains(&"target"));
    }

    #[test]
    fn inspects_all_source_files() {
        let dir = setup_rust_project();
        let Observation::Files { inspections, .. } = observe_rust_project(dir.path());

        let paths: Vec<String> = inspections
            .iter()
            .map(|i| {
                i.path
                    .strip_prefix(dir.path())
                    .unwrap()
                    .display()
                    .to_string()
            })
            .collect();

        assert!(paths.contains(&"Cargo.toml".to_string()));
        assert!(paths.contains(&"src/main.rs".to_string()));
        assert!(paths.contains(&"src/lib.rs".to_string()));
        assert!(paths.contains(&".gitignore".to_string()));

        // target/ contents should not be inspected.
        assert!(!paths.iter().any(|p| p.starts_with("target")));
    }

    #[test]
    fn skips_target_directory() {
        let dir = setup_rust_project();
        let Observation::Files {
            survey,
            inspections,
        } = observe_rust_project(dir.path());

        // No survey for target/.
        assert!(!survey.iter().any(|s| s.path.ends_with("target")));

        // No inspections under target/.
        assert!(!inspections.iter().any(|i| {
            i.path
                .strip_prefix(dir.path())
                .unwrap()
                .starts_with("target")
        }));
    }

    #[test]
    fn handles_binary_files() {
        let dir = setup_rust_project();
        let binary_path = dir.path().join("image.bin");
        fs::write(&binary_path, [0xFF, 0xFE, 0x00]).unwrap();

        let Observation::Files { inspections, .. } = observe_rust_project(dir.path());

        let binary = inspections.iter().find(|i| i.path == binary_path).unwrap();
        assert!(matches!(binary.content, FileContent::Binary { .. }));
    }
}
