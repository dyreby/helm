//! Working set storage: observations accumulating between steer/log commands.
//!
//! Observations are appended to `working.jsonl` as they arrive.
//! When steer or log is called, the working set is loaded, sealed into a
//! bearing, and then cleared. The file is removed on clear — a missing file
//! is a valid empty working set.

use std::{fs, io};

// Traits must be in scope for `.lines()` on `BufReader` and `.write_all()` on `File`.
use io::{BufRead, Write};

use uuid::Uuid;

use crate::model::Observation;

use super::{Result, Storage, StorageError};

// TODO: remove once observe (#99) is wired to the working set.
#[allow(dead_code)]
impl Storage {
    /// Appends an observation to the voyage's working set.
    pub fn append_working(&self, voyage_id: Uuid, observation: &Observation) -> Result<()> {
        let dir = self.voyage_dir(voyage_id);
        if !dir.exists() {
            return Err(StorageError::VoyageNotFound(voyage_id));
        }
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("working.jsonl"))?;
        let mut line = serde_json::to_string(observation)?;
        line.push('\n');
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    /// Loads all observations from the voyage's working set.
    ///
    /// Returns an empty vec if the working set file doesn't exist.
    pub fn load_working(&self, voyage_id: Uuid) -> Result<Vec<Observation>> {
        let path = self.voyage_dir(voyage_id).join("working.jsonl");
        if !path.exists() {
            let dir = self.voyage_dir(voyage_id);
            if !dir.exists() {
                return Err(StorageError::VoyageNotFound(voyage_id));
            }
            return Ok(Vec::new());
        }
        let file = fs::File::open(path)?;
        let reader = io::BufReader::new(file);
        let mut observations = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if !line.is_empty() {
                observations.push(serde_json::from_str(&line)?);
            }
        }
        Ok(observations)
    }

    /// Clears the voyage's working set by removing `working.jsonl`.
    ///
    /// Idempotent: does nothing if the file doesn't exist.
    pub fn clear_working(&self, voyage_id: Uuid) -> Result<()> {
        let dir = self.voyage_dir(voyage_id);
        if !dir.exists() {
            return Err(StorageError::VoyageNotFound(voyage_id));
        }
        let path = dir.join("working.jsonl");
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use jiff::Timestamp;
    use tempfile::TempDir;

    use crate::model::*;

    fn test_storage() -> (TempDir, Storage) {
        let dir = TempDir::new().unwrap();
        let storage = Storage::new(dir.path().join("voyages")).unwrap();
        (dir, storage)
    }

    fn sample_voyage() -> Voyage {
        Voyage {
            id: Uuid::new_v4(),
            identity: "john-agent".into(),
            intent: "Fix the widget".into(),
            created_at: Timestamp::now(),
            status: VoyageStatus::Active,
        }
    }

    fn sample_observation() -> Observation {
        Observation {
            target: Observe::DirectoryTree {
                root: PathBuf::from("src/"),
                skip: vec![],
                max_depth: None,
            },
            payload: Payload::DirectoryTree {
                listings: vec![DirectoryListing {
                    path: PathBuf::from("src/"),
                    entries: vec![DirectoryEntry {
                        name: "main.rs".into(),
                        is_dir: false,
                        size_bytes: Some(42),
                    }],
                }],
            },
            observed_at: Timestamp::now(),
        }
    }

    #[test]
    fn append_and_load_working() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        let obs = sample_observation();
        storage.append_working(voyage.id, &obs).unwrap();
        storage.append_working(voyage.id, &obs).unwrap();

        let loaded = storage.load_working(voyage.id).unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(matches!(loaded[0].target, Observe::DirectoryTree { .. }));
    }

    #[test]
    fn load_working_empty_when_no_file() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        let loaded = storage.load_working(voyage.id).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn clear_working_removes_file() {
        let (dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        storage
            .append_working(voyage.id, &sample_observation())
            .unwrap();
        storage.clear_working(voyage.id).unwrap();

        let working_path = dir
            .path()
            .join("voyages")
            .join(voyage.id.to_string())
            .join("working.jsonl");
        assert!(!working_path.exists());

        // Load after clear returns empty.
        let loaded = storage.load_working(voyage.id).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn clear_working_idempotent() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        // Clear with no file — should not error.
        storage.clear_working(voyage.id).unwrap();
    }

    #[test]
    fn append_working_nonexistent_voyage_fails() {
        let (_dir, storage) = test_storage();
        let err = storage
            .append_working(Uuid::new_v4(), &sample_observation())
            .unwrap_err();
        assert!(matches!(err, StorageError::VoyageNotFound(_)));
    }

    #[test]
    fn load_working_nonexistent_voyage_fails() {
        let (_dir, storage) = test_storage();
        let err = storage.load_working(Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, StorageError::VoyageNotFound(_)));
    }

    #[test]
    fn clear_working_nonexistent_voyage_fails() {
        let (_dir, storage) = test_storage();
        let err = storage.clear_working(Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, StorageError::VoyageNotFound(_)));
    }
}
