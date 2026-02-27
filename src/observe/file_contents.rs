//! `FileContents` observation: read specific files.
//!
//! Reads file contents, distinguishing text from binary.

use std::{fs, path::Path};

use crate::model::{FileContent, FileContents, Payload};

/// Read specific files and return their contents.
///
/// Each path produces a `FileContents` entry (text, binary, or error).
/// Paths that don't exist or can't be read produce error entries rather than panics.
/// Observation is always total.
pub fn observe_file_contents(paths: &[impl AsRef<Path>]) -> Payload {
    let contents = paths.iter().map(|p| read_file(p.as_ref())).collect();
    Payload::FileContents { contents }
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

    #[test]
    fn read_text_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
        let read = vec![dir.path().join("hello.txt")];

        let Payload::FileContents { contents } = observe_file_contents(&read) else {
            panic!("expected FileContents payload");
        };

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].path, read[0]);
        assert!(
            matches!(&contents[0].content, FileContent::Text { content } if content == "hello world")
        );
    }

    #[test]
    fn read_empty_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("empty.txt"), "").unwrap();
        let read = vec![dir.path().join("empty.txt")];

        let Payload::FileContents { contents } = observe_file_contents(&read) else {
            panic!("expected FileContents payload");
        };

        assert!(
            matches!(&contents[0].content, FileContent::Text { content } if content.is_empty())
        );
    }

    #[test]
    fn read_binary_file() {
        let dir = TempDir::new().unwrap();
        let binary_path = dir.path().join("image.bin");
        // Invalid UTF-8 sequence.
        fs::write(&binary_path, [0xFF, 0xFE, 0x00, 0x01, 0x80]).unwrap();

        let Payload::FileContents { contents } = observe_file_contents(&[binary_path]) else {
            panic!("expected FileContents payload");
        };

        assert_eq!(contents.len(), 1);
        assert!(matches!(
            &contents[0].content,
            FileContent::Binary { size_bytes: 5 }
        ));
    }

    #[test]
    fn read_nonexistent_file_returns_error() {
        let Payload::FileContents { contents } =
            observe_file_contents(&[PathBuf::from("/nonexistent/file.txt")])
        else {
            panic!("expected FileContents payload");
        };

        assert_eq!(contents.len(), 1);
        assert!(matches!(&contents[0].content, FileContent::Error { .. }));
    }

    #[test]
    fn read_unreadable_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("secret.txt");
        fs::write(&path, "secret").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();

        let Payload::FileContents { contents } = observe_file_contents(std::slice::from_ref(&path))
        else {
            panic!("expected FileContents payload");
        };

        assert!(matches!(&contents[0].content, FileContent::Error { .. }));

        // Restore permissions so tempdir cleanup works.
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[test]
    fn read_multiple_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "aaa").unwrap();
        fs::write(dir.path().join("b.txt"), "bbb").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/c.txt"), "ccc").unwrap();

        let read = vec![
            dir.path().join("a.txt"),
            dir.path().join("b.txt"),
            dir.path().join("sub/c.txt"),
        ];

        let Payload::FileContents { contents } = observe_file_contents(&read) else {
            panic!("expected FileContents payload");
        };

        assert_eq!(contents.len(), 3);
        assert!(matches!(&contents[0].content, FileContent::Text { content } if content == "aaa"));
        assert!(matches!(&contents[1].content, FileContent::Text { content } if content == "bbb"));
        assert!(matches!(&contents[2].content, FileContent::Text { content } if content == "ccc"));
    }

    #[test]
    fn read_empty_paths() {
        let Payload::FileContents { contents } = observe_file_contents(&Vec::<PathBuf>::new())
        else {
            panic!("expected FileContents payload");
        };

        assert!(contents.is_empty());
    }

    // ── Dispatch test ──

    #[test]
    fn observe_dispatches_file_contents_target() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.txt"), "hello").unwrap();

        let target = crate::model::Observe::FileContents {
            paths: vec![dir.path().join("test.txt")],
        };

        let payload = crate::observe::observe(&target, None);
        assert!(matches!(payload, Payload::FileContents { .. }));
    }
}
