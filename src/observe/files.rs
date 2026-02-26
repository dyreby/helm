//! Files source kind: filesystem observation.
//!
//! Lists directory contents with metadata.
//! Reads file contents, distinguishing text from binary.

use std::{fs, path::Path};

use crate::model::{DirectoryEntry, DirectoryListing, FileContent, FileContents, Sighting};

/// Observe the filesystem: list directories, read files.
///
/// Each list path produces a directory listing with metadata.
/// Each read path produces file contents (text, binary, or error).
///
/// Paths that don't exist or can't be read produce error entries rather than panics.
/// Observation is always total.
pub fn observe_files(list: &[impl AsRef<Path>], read: &[impl AsRef<Path>]) -> Sighting {
    let listings = list.iter().map(|p| list_directory(p.as_ref())).collect();
    let contents = read.iter().map(|p| read_file(p.as_ref())).collect();

    Sighting::Files { listings, contents }
}

/// List a directory's immediate contents with metadata.
///
/// If the path is not a directory or can't be read, returns an empty entry list.
/// The path is always recorded so the caller knows what was attempted.
fn list_directory(path: &Path) -> DirectoryListing {
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

    DirectoryListing {
        path: path.to_path_buf(),
        entries,
    }
}

/// Read a file and classify its content.
///
/// - Valid UTF-8 → `FileContent::Text`
/// - Invalid UTF-8 → `FileContent::Binary` with size
/// - Read failure → `FileContent::Error` with message
fn read_file(path: &Path) -> FileContents {
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

    FileContents {
        path: path.to_path_buf(),
        content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

    use tempfile::TempDir;

    use crate::model::Sighting;

    /// Helper: create a temp directory with some files.
    fn setup_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
        fs::write(dir.path().join("empty.txt"), "").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("subdir").join("nested.txt"), "nested").unwrap();
        dir
    }

    // ── List tests ──

    #[test]
    fn list_returns_directory_contents() {
        let dir = setup_test_dir();
        let list = vec![dir.path().to_path_buf()];

        let obs = observe_files(&list, &Vec::<PathBuf>::new());
        let Sighting::Files { listings, contents } = obs;

        assert_eq!(listings.len(), 1);
        assert!(contents.is_empty());

        let listing = &listings[0];
        assert_eq!(listing.path, dir.path());

        let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["empty.txt", "hello.txt", "subdir"]);
    }

    #[test]
    fn list_entries_have_correct_metadata() {
        let dir = setup_test_dir();
        let list = vec![dir.path().to_path_buf()];

        let Sighting::Files { listings, .. } = observe_files(&list, &Vec::<PathBuf>::new());
        let listing = &listings[0];

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
    fn list_nonexistent_directory_returns_empty_entries() {
        let list = vec![PathBuf::from("/nonexistent/path/that/should/not/exist")];

        let Sighting::Files { listings, .. } = observe_files(&list, &Vec::<PathBuf>::new());
        assert_eq!(listings.len(), 1);
        assert_eq!(
            listings[0].path,
            PathBuf::from("/nonexistent/path/that/should/not/exist")
        );
        assert!(listings[0].entries.is_empty());
    }

    #[test]
    fn list_multiple_directories() {
        let dir = setup_test_dir();
        let list = vec![dir.path().to_path_buf(), dir.path().join("subdir")];

        let Sighting::Files { listings, .. } = observe_files(&list, &Vec::<PathBuf>::new());
        assert_eq!(listings.len(), 2);

        // First directory has 3 entries.
        assert_eq!(listings[0].entries.len(), 3);

        // Subdir has 1 entry.
        assert_eq!(listings[1].entries.len(), 1);
        assert_eq!(listings[1].entries[0].name, "nested.txt");
    }

    #[test]
    fn list_empty_directory() {
        let dir = TempDir::new().unwrap();
        let list = vec![dir.path().to_path_buf()];

        let Sighting::Files { listings, .. } = observe_files(&list, &Vec::<PathBuf>::new());
        assert_eq!(listings.len(), 1);
        assert!(listings[0].entries.is_empty());
    }

    #[test]
    fn list_entries_are_sorted_by_name() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("zebra.txt"), "z").unwrap();
        fs::write(dir.path().join("alpha.txt"), "a").unwrap();
        fs::write(dir.path().join("middle.txt"), "m").unwrap();

        let Sighting::Files { listings, .. } =
            observe_files(&[dir.path().to_path_buf()], &Vec::<PathBuf>::new());

        let names: Vec<&str> = listings[0].entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["alpha.txt", "middle.txt", "zebra.txt"]);
    }

    // ── Read tests ──

    #[test]
    fn read_text_file() {
        let dir = setup_test_dir();
        let read = vec![dir.path().join("hello.txt")];

        let Sighting::Files { contents, .. } = observe_files(&Vec::<PathBuf>::new(), &read);

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].path, read[0]);
        assert!(
            matches!(&contents[0].content, FileContent::Text { content: s } if s == "hello world")
        );
    }

    #[test]
    fn read_empty_file() {
        let dir = setup_test_dir();
        let read = vec![dir.path().join("empty.txt")];

        let Sighting::Files { contents, .. } = observe_files(&Vec::<PathBuf>::new(), &read);

        assert!(
            matches!(&contents[0].content, FileContent::Text { content: s } if s.is_empty())
        );
    }

    #[test]
    fn read_binary_file() {
        let dir = TempDir::new().unwrap();
        let binary_path = dir.path().join("image.bin");
        // Invalid UTF-8 sequence.
        fs::write(&binary_path, [0xFF, 0xFE, 0x00, 0x01, 0x80]).unwrap();

        let read = vec![binary_path.clone()];

        let Sighting::Files { contents, .. } = observe_files(&Vec::<PathBuf>::new(), &read);

        assert_eq!(contents.len(), 1);
        assert!(matches!(
            &contents[0].content,
            FileContent::Binary { size_bytes: 5 }
        ));
    }

    #[test]
    fn read_nonexistent_file_returns_error() {
        let read = vec![PathBuf::from("/nonexistent/file.txt")];

        let Sighting::Files { contents, .. } = observe_files(&Vec::<PathBuf>::new(), &read);

        assert_eq!(contents.len(), 1);
        assert!(matches!(&contents[0].content, FileContent::Error { .. }));
    }

    #[test]
    fn read_unreadable_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("secret.txt");
        fs::write(&path, "secret").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();

        let read = vec![path.clone()];

        let Sighting::Files { contents, .. } = observe_files(&Vec::<PathBuf>::new(), &read);

        assert!(matches!(&contents[0].content, FileContent::Error { .. }));

        // Restore permissions so tempdir cleanup works.
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[test]
    fn read_multiple_files() {
        let dir = setup_test_dir();
        let read = vec![
            dir.path().join("hello.txt"),
            dir.path().join("empty.txt"),
            dir.path().join("subdir").join("nested.txt"),
        ];

        let Sighting::Files { contents, .. } = observe_files(&Vec::<PathBuf>::new(), &read);

        assert_eq!(contents.len(), 3);
        assert!(
            matches!(&contents[0].content, FileContent::Text { content: s } if s == "hello world")
        );
        assert!(
            matches!(&contents[1].content, FileContent::Text { content: s } if s.is_empty())
        );
        assert!(
            matches!(&contents[2].content, FileContent::Text { content: s } if s == "nested")
        );
    }

    // ── Combined tests ──

    #[test]
    fn list_and_read_together() {
        let dir = setup_test_dir();
        let list = vec![dir.path().to_path_buf()];
        let read = vec![dir.path().join("hello.txt")];

        let Sighting::Files { listings, contents } = observe_files(&list, &read);

        assert_eq!(listings.len(), 1);
        assert_eq!(listings[0].entries.len(), 3);

        assert_eq!(contents.len(), 1);
        assert!(
            matches!(&contents[0].content, FileContent::Text { content: s } if s == "hello world")
        );
    }

    #[test]
    fn empty_list_and_read() {
        let Sighting::Files { listings, contents } =
            observe_files(&Vec::<PathBuf>::new(), &Vec::<PathBuf>::new());

        assert!(listings.is_empty());
        assert!(contents.is_empty());
    }

    // ── observe() dispatch test ──

    #[test]
    fn observe_dispatches_files_mark() {
        let dir = setup_test_dir();
        let mark = crate::model::Mark::Files {
            list: vec![dir.path().to_path_buf()],
            read: vec![dir.path().join("hello.txt")],
        };

        let sighting = crate::observe::observe(&mark);
        assert!(matches!(sighting, Sighting::Files { .. }));
    }
}
