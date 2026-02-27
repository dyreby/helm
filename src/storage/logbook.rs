//! Logbook storage: append and load logbook entries.

use std::{fs, io};

// Traits must be in scope for `.lines()` on BufReader and `.write_all()` on File.
use io::{BufRead, Write};

use uuid::Uuid;

use crate::model::LogbookEntry;

use super::{Result, Storage, StorageError};

impl Storage {
    /// Appends a logbook entry to a voyage's logbook.
    pub fn append_entry(&self, voyage_id: Uuid, entry: &LogbookEntry) -> Result<()> {
        let dir = self.voyage_dir(voyage_id);
        if !dir.exists() {
            return Err(StorageError::VoyageNotFound(voyage_id));
        }
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("logbook.jsonl"))?;
        let mut line = serde_json::to_string(entry)?;
        line.push('\n');
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    /// Loads all logbook entries for a voyage.
    // TODO: remove once log (#101) is wired to the CLI.
    #[allow(dead_code)]
    pub fn load_logbook(&self, voyage_id: Uuid) -> Result<Vec<LogbookEntry>> {
        let path = self.voyage_dir(voyage_id).join("logbook.jsonl");
        if !path.exists() {
            let dir = self.voyage_dir(voyage_id);
            if !dir.exists() {
                return Err(StorageError::VoyageNotFound(voyage_id));
            }
            return Ok(Vec::new());
        }
        let file = fs::File::open(path)?;
        let reader = io::BufReader::new(file);
        let mut entries = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if !line.is_empty() {
                entries.push(serde_json::from_str(&line)?);
            }
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use jiff::Timestamp;
    use tempfile::TempDir;
    use uuid::Uuid;

    use crate::model::*;

    fn test_storage() -> (TempDir, Storage) {
        let dir = TempDir::new().unwrap();
        let storage = Storage::new(dir.path().join("voyages")).unwrap();
        (dir, storage)
    }

    fn sample_voyage() -> Voyage {
        Voyage {
            id: Uuid::new_v4(),
            intent: "Fix the widget".into(),
            created_at: Timestamp::now(),
            status: VoyageStatus::Active,
        }
    }

    fn sample_bearing() -> Bearing {
        Bearing {
            observations: vec![],
            summary: "The project has a single main.rs file.".into(),
        }
    }

    fn sample_steer_entry() -> LogbookEntry {
        LogbookEntry {
            bearing: sample_bearing(),
            author: "john-agent".into(),
            timestamp: Timestamp::now(),
            kind: EntryKind::Steer(Steer::Comment {
                number: 42,
                body: "Here's my plan.".into(),
                target: CommentTarget::Issue,
            }),
        }
    }

    fn sample_log_entry() -> LogbookEntry {
        LogbookEntry {
            bearing: sample_bearing(),
            author: "john-agent".into(),
            timestamp: Timestamp::now(),
            kind: EntryKind::Log("Waiting for review.".into()),
        }
    }

    #[test]
    fn append_and_load_logbook_entries() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        storage
            .append_entry(voyage.id, &sample_steer_entry())
            .unwrap();
        storage
            .append_entry(voyage.id, &sample_log_entry())
            .unwrap();

        let entries = storage.load_logbook(voyage.id).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(matches!(
            entries[0].kind,
            EntryKind::Steer(Steer::Comment { .. })
        ));
        assert!(matches!(entries[1].kind, EntryKind::Log(_)));
    }

    #[test]
    fn load_logbook_empty() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        let entries = storage.load_logbook(voyage.id).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn load_logbook_nonexistent_voyage_fails() {
        let (_dir, storage) = test_storage();
        let err = storage.load_logbook(Uuid::new_v4()).unwrap_err();

        assert!(matches!(err, StorageError::VoyageNotFound(_)));
    }

    #[test]
    fn append_entry_nonexistent_voyage_fails() {
        let (_dir, storage) = test_storage();
        let err = storage
            .append_entry(Uuid::new_v4(), &sample_steer_entry())
            .unwrap_err();

        assert!(matches!(err, StorageError::VoyageNotFound(_)));
    }
}
