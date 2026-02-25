//! Files source kind: filesystem observation.
//!
//! Survey lists directory contents with metadata.
//! Inspect reads file contents, distinguishing text from binary.

use std::{fs, path::Path};

use crate::model::{DirectoryEntry, DirectorySurvey, FileContent, FileInspection, Observation};

/// Observe the filesystem: survey directories, inspect files.
///
/// Each scope path is surveyed (directory listing with metadata).
/// Each focus path is inspected (file contents read and classified).
///
/// Paths that don't exist or can't be read produce error entries rather than panics.
/// Observation is always total.
pub fn observe_files(scope: &[impl AsRef<Path>], focus: &[impl AsRef<Path>]) -> Observation {
    let survey = scope.iter().map(|p| survey_directory(p.as_ref())).collect();
    let inspections = focus.iter().map(|p| inspect_file(p.as_ref())).collect();

    Observation::Files {
        survey,
        inspections,
    }
}

/// List a directory's immediate contents with metadata.
///
/// If the path is not a directory or can't be read, returns an empty entry list.
/// The path is always recorded so the caller knows what was attempted.
fn survey_directory(path: &Path) -> DirectorySurvey {
    let entries = match fs::read_dir(path) {
        Ok(read_dir) => {
            let mut entries: Vec<DirectoryEntry> = read_dir
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    let metadata = entry.metadata().ok();
                    let is_dir = metadata.as_ref().is_some_and(fs::Metadata::is_dir);
                    let size_bytes = if is_dir {
                        None
                    } else {
                        metadata.as_ref().map(fs::Metadata::len)
                    };

                    Some(DirectoryEntry {
                        name: entry.file_name().to_string_lossy().into_owned(),
                        is_dir,
                        size_bytes,
                    })
                })
                .collect();

            // Sort for deterministic output.
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            entries
        }
        Err(_) => Vec::new(),
    };

    DirectorySurvey {
        path: path.to_path_buf(),
        entries,
    }
}

/// Read a file and classify its content.
///
/// - Valid UTF-8 → `FileContent::Text`
/// - Invalid UTF-8 → `FileContent::Binary` with size
/// - Read failure → `FileContent::Error` with message
fn inspect_file(path: &Path) -> FileInspection {
    let content = match fs::read(path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => FileContent::Text(text),
            Err(e) => FileContent::Binary {
                size_bytes: e.into_bytes().len() as u64,
            },
        },
        Err(e) => FileContent::Error(e.to_string()),
    };

    FileInspection {
        path: path.to_path_buf(),
        content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

    use tempfile::TempDir;

    use crate::model::{FileContent, Observation};

    /// Helper: create a temp directory with some files.
    fn setup_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
        fs::write(dir.path().join("empty.txt"), "").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("subdir").join("nested.txt"), "nested").unwrap();
        dir
    }

    // ── Survey tests ──

    #[test]
    fn survey_lists_directory_contents() {
        let dir = setup_test_dir();
        let scope = vec![dir.path().to_path_buf()];

        let obs = observe_files(&scope, &Vec::<PathBuf>::new());
        let Observation::Files {
            survey,
            inspections,
        } = obs;

        assert_eq!(survey.len(), 1);
        assert!(inspections.is_empty());

        let listing = &survey[0];
        assert_eq!(listing.path, dir.path());

        let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["empty.txt", "hello.txt", "subdir"]);
    }

    #[test]
    fn survey_entries_have_correct_metadata() {
        let dir = setup_test_dir();
        let scope = vec![dir.path().to_path_buf()];

        let Observation::Files { survey, .. } = observe_files(&scope, &Vec::<PathBuf>::new());
        let listing = &survey[0];

        let hello = listing
            .entries
            .iter()
            .find(|e| e.name == "hello.txt")
            .unwrap();
        assert!(!hello.is_dir);
        assert_eq!(hello.size_bytes, Some(11)); // "hello world"

        let empty = listing
            .entries
            .iter()
            .find(|e| e.name == "empty.txt")
            .unwrap();
        assert!(!empty.is_dir);
        assert_eq!(empty.size_bytes, Some(0));

        let subdir = listing.entries.iter().find(|e| e.name == "subdir").unwrap();
        assert!(subdir.is_dir);
        assert_eq!(subdir.size_bytes, None); // Directories don't report size.
    }

    #[test]
    fn survey_nonexistent_directory_returns_empty_entries() {
        let scope = vec![PathBuf::from("/nonexistent/path/that/should/not/exist")];

        let Observation::Files { survey, .. } = observe_files(&scope, &Vec::<PathBuf>::new());
        assert_eq!(survey.len(), 1);
        assert_eq!(
            survey[0].path,
            PathBuf::from("/nonexistent/path/that/should/not/exist")
        );
        assert!(survey[0].entries.is_empty());
    }

    #[test]
    fn survey_multiple_directories() {
        let dir = setup_test_dir();
        let scope = vec![dir.path().to_path_buf(), dir.path().join("subdir")];

        let Observation::Files { survey, .. } = observe_files(&scope, &Vec::<PathBuf>::new());
        assert_eq!(survey.len(), 2);

        // First directory has 3 entries.
        assert_eq!(survey[0].entries.len(), 3);

        // Subdir has 1 entry.
        assert_eq!(survey[1].entries.len(), 1);
        assert_eq!(survey[1].entries[0].name, "nested.txt");
    }

    #[test]
    fn survey_empty_directory() {
        let dir = TempDir::new().unwrap();
        let scope = vec![dir.path().to_path_buf()];

        let Observation::Files { survey, .. } = observe_files(&scope, &Vec::<PathBuf>::new());
        assert_eq!(survey.len(), 1);
        assert!(survey[0].entries.is_empty());
    }

    #[test]
    fn survey_entries_are_sorted_by_name() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("zebra.txt"), "z").unwrap();
        fs::write(dir.path().join("alpha.txt"), "a").unwrap();
        fs::write(dir.path().join("middle.txt"), "m").unwrap();

        let Observation::Files { survey, .. } =
            observe_files(&[dir.path().to_path_buf()], &Vec::<PathBuf>::new());

        let names: Vec<&str> = survey[0].entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["alpha.txt", "middle.txt", "zebra.txt"]);
    }

    // ── Inspect tests ──

    #[test]
    fn inspect_text_file() {
        let dir = setup_test_dir();
        let focus = vec![dir.path().join("hello.txt")];

        let Observation::Files { inspections, .. } = observe_files(&Vec::<PathBuf>::new(), &focus);

        assert_eq!(inspections.len(), 1);
        assert_eq!(inspections[0].path, focus[0]);
        assert!(matches!(&inspections[0].content, FileContent::Text(s) if s == "hello world"));
    }

    #[test]
    fn inspect_empty_file() {
        let dir = setup_test_dir();
        let focus = vec![dir.path().join("empty.txt")];

        let Observation::Files { inspections, .. } = observe_files(&Vec::<PathBuf>::new(), &focus);

        assert!(matches!(&inspections[0].content, FileContent::Text(s) if s.is_empty()));
    }

    #[test]
    fn inspect_binary_file() {
        let dir = TempDir::new().unwrap();
        let binary_path = dir.path().join("image.bin");
        // Invalid UTF-8 sequence.
        fs::write(&binary_path, [0xFF, 0xFE, 0x00, 0x01, 0x80]).unwrap();

        let focus = vec![binary_path.clone()];

        let Observation::Files { inspections, .. } = observe_files(&Vec::<PathBuf>::new(), &focus);

        assert_eq!(inspections.len(), 1);
        assert!(matches!(
            &inspections[0].content,
            FileContent::Binary { size_bytes: 5 }
        ));
    }

    #[test]
    fn inspect_nonexistent_file_returns_error() {
        let focus = vec![PathBuf::from("/nonexistent/file.txt")];

        let Observation::Files { inspections, .. } = observe_files(&Vec::<PathBuf>::new(), &focus);

        assert_eq!(inspections.len(), 1);
        assert!(matches!(&inspections[0].content, FileContent::Error(_)));
    }

    #[test]
    fn inspect_unreadable_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("secret.txt");
        fs::write(&path, "secret").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();

        let focus = vec![path.clone()];

        let Observation::Files { inspections, .. } = observe_files(&Vec::<PathBuf>::new(), &focus);

        assert!(matches!(&inspections[0].content, FileContent::Error(_)));

        // Restore permissions so tempdir cleanup works.
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[test]
    fn inspect_multiple_files() {
        let dir = setup_test_dir();
        let focus = vec![
            dir.path().join("hello.txt"),
            dir.path().join("empty.txt"),
            dir.path().join("subdir").join("nested.txt"),
        ];

        let Observation::Files { inspections, .. } = observe_files(&Vec::<PathBuf>::new(), &focus);

        assert_eq!(inspections.len(), 3);
        assert!(matches!(&inspections[0].content, FileContent::Text(s) if s == "hello world"));
        assert!(matches!(&inspections[1].content, FileContent::Text(s) if s.is_empty()));
        assert!(matches!(&inspections[2].content, FileContent::Text(s) if s == "nested"));
    }

    // ── Combined tests ──

    #[test]
    fn survey_and_inspect_together() {
        let dir = setup_test_dir();
        let scope = vec![dir.path().to_path_buf()];
        let focus = vec![dir.path().join("hello.txt")];

        let Observation::Files {
            survey,
            inspections,
        } = observe_files(&scope, &focus);

        assert_eq!(survey.len(), 1);
        assert_eq!(survey[0].entries.len(), 3);

        assert_eq!(inspections.len(), 1);
        assert!(matches!(&inspections[0].content, FileContent::Text(s) if s == "hello world"));
    }

    #[test]
    fn empty_scope_and_focus() {
        let Observation::Files {
            survey,
            inspections,
        } = observe_files(&Vec::<PathBuf>::new(), &Vec::<PathBuf>::new());

        assert!(survey.is_empty());
        assert!(inspections.is_empty());
    }

    // ── observe() dispatch test ──

    #[test]
    fn observe_dispatches_files_query() {
        let dir = setup_test_dir();
        let query = crate::model::SourceQuery::Files {
            scope: vec![dir.path().to_path_buf()],
            focus: vec![dir.path().join("hello.txt")],
        };

        let obs = crate::observe::observe(&query);
        assert!(matches!(obs, Observation::Files { .. }));
    }
}
