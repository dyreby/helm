//! Local persistence for voyages and logbooks.
//!
//! Each voyage lives in its own directory under the storage root:
//!
//! ```text
//! <root>/<uuid>/
//!   voyage.json      # Voyage metadata
//!   logbook.jsonl    # Append-only logbook entries (bearings + action reports)
//! ```

use std::{fs, io, path::PathBuf};

// Traits must be in scope for `.lines()` on BufReader and `.write_all()` on File.
use io::{BufRead, Write};

use uuid::Uuid;

use crate::model::{LogbookEntry, Voyage};

/// Errors that can occur during storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("voyage not found: {0}")]
    VoyageNotFound(Uuid),

    #[error("voyage already exists: {0}")]
    VoyageAlreadyExists(Uuid),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = core::result::Result<T, StorageError>;

/// Local file-based storage for voyages and logbooks.
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    /// Creates a new storage instance rooted at the given directory.
    ///
    /// The directory is created if it doesn't exist.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Returns the default storage root: `~/.helm/voyages/`.
    pub fn default_root() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".helm").join("voyages"))
    }

    // ── Voyages ──

    /// Creates a new voyage, writing its metadata to disk.
    pub fn create_voyage(&self, voyage: &Voyage) -> Result<()> {
        let dir = self.voyage_dir(voyage.id);
        if dir.exists() {
            return Err(StorageError::VoyageAlreadyExists(voyage.id));
        }
        fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(voyage)?;
        fs::write(dir.join("voyage.json"), json)?;
        Ok(())
    }

    /// Updates a voyage's metadata on disk.
    pub fn update_voyage(&self, voyage: &Voyage) -> Result<()> {
        let path = self.voyage_dir(voyage.id).join("voyage.json");
        if !path.exists() {
            return Err(StorageError::VoyageNotFound(voyage.id));
        }
        let json = serde_json::to_string_pretty(voyage)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Loads a single voyage's metadata.
    pub fn load_voyage(&self, id: Uuid) -> Result<Voyage> {
        let path = self.voyage_dir(id).join("voyage.json");
        if !path.exists() {
            return Err(StorageError::VoyageNotFound(id));
        }
        let json = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Lists all voyages by reading each voyage directory's metadata.
    pub fn list_voyages(&self) -> Result<Vec<Voyage>> {
        let mut voyages = Vec::new();
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(voyages),
            Err(e) => return Err(e.into()),
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path().join("voyage.json");
            if path.is_file() {
                let json = fs::read_to_string(&path)?;
                voyages.push(serde_json::from_str(&json)?);
            }
        }
        voyages.sort_by(|a: &Voyage, b: &Voyage| a.created_at.cmp(&b.created_at));
        Ok(voyages)
    }

    // ── Logbook ──

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

    fn voyage_dir(&self, id: Uuid) -> PathBuf {
        self.root.join(id.to_string())
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
            kind: VoyageKind::OpenWaters,
            intent: "Fix the widget".into(),
            created_at: Timestamp::now(),
            status: VoyageStatus::Active,
        }
    }

    fn sample_bearing() -> Bearing {
        Bearing {
            id: Uuid::new_v4(),
            observations: vec![Observation {
                id: Uuid::new_v4(),
                subject: Subject::Files {
                    scope: vec![PathBuf::from("src/")],
                    focus: vec![],
                },
                sighting: Sighting::Files {
                    survey: vec![DirectorySurvey {
                        path: PathBuf::from("src/"),
                        entries: vec![DirectoryEntry {
                            name: "main.rs".into(),
                            is_dir: false,
                            size_bytes: Some(42),
                        }],
                    }],
                    inspections: vec![],
                },
                observed_at: Timestamp::now(),
            }],
            position: Position {
                text: "The project has a single main.rs file.".into(),
                history: vec![],
            },
            taken_at: Timestamp::now(),
        }
    }

    fn sample_action_report() -> ActionReport {
        ActionReport {
            plan: ActionPlan::WriteFiles {
                files: vec![FileWrite {
                    path: PathBuf::from("README.md"),
                    content: "# Hello".into(),
                }],
            },
            outcome: ActionOutcome::Success,
            completed_at: Timestamp::now(),
        }
    }

    #[test]
    fn create_and_load_voyage() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();

        storage.create_voyage(&voyage).unwrap();
        let loaded = storage.load_voyage(voyage.id).unwrap();

        assert_eq!(loaded.id, voyage.id);
        assert_eq!(loaded.intent, voyage.intent);
    }

    #[test]
    fn create_duplicate_voyage_fails() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();

        storage.create_voyage(&voyage).unwrap();
        let err = storage.create_voyage(&voyage).unwrap_err();

        assert!(matches!(err, StorageError::VoyageAlreadyExists(_)));
    }

    #[test]
    fn load_nonexistent_voyage_fails() {
        let (_dir, storage) = test_storage();
        let err = storage.load_voyage(Uuid::new_v4()).unwrap_err();

        assert!(matches!(err, StorageError::VoyageNotFound(_)));
    }

    #[test]
    fn update_voyage_status() {
        let (_dir, storage) = test_storage();
        let mut voyage = sample_voyage();

        storage.create_voyage(&voyage).unwrap();
        voyage.status = VoyageStatus::Completed {
            at: Timestamp::now(),
            summary: Some("Done.".into()),
        };
        storage.update_voyage(&voyage).unwrap();

        let loaded = storage.load_voyage(voyage.id).unwrap();
        assert!(matches!(loaded.status, VoyageStatus::Completed { .. }));
    }

    #[test]
    fn update_nonexistent_voyage_fails() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        let err = storage.update_voyage(&voyage).unwrap_err();

        assert!(matches!(err, StorageError::VoyageNotFound(_)));
    }

    #[test]
    fn list_voyages_empty() {
        let (_dir, storage) = test_storage();
        let voyages = storage.list_voyages().unwrap();

        assert!(voyages.is_empty());
    }

    #[test]
    fn list_voyages_returns_all_sorted_by_created_at() {
        let (_dir, storage) = test_storage();

        let mut v1 = sample_voyage();
        v1.intent = "First".into();
        v1.created_at = Timestamp::new(1_000_000_000, 0).unwrap();

        let mut v2 = sample_voyage();
        v2.intent = "Second".into();
        v2.created_at = Timestamp::new(2_000_000_000, 0).unwrap();

        // Create in reverse order to verify sorting.
        storage.create_voyage(&v2).unwrap();
        storage.create_voyage(&v1).unwrap();

        let voyages = storage.list_voyages().unwrap();
        assert_eq!(voyages.len(), 2);
        assert_eq!(voyages[0].intent, "First");
        assert_eq!(voyages[1].intent, "Second");
    }

    #[test]
    fn append_and_load_logbook_entries() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        let bearing = sample_bearing();
        let report = sample_action_report();

        storage
            .append_entry(voyage.id, &LogbookEntry::Bearing(bearing.clone()))
            .unwrap();
        storage
            .append_entry(voyage.id, &LogbookEntry::ActionReport(report.clone()))
            .unwrap();

        let entries = storage.load_logbook(voyage.id).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(matches!(entries[0], LogbookEntry::Bearing(_)));
        assert!(matches!(entries[1], LogbookEntry::ActionReport(_)));
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
        let bearing = sample_bearing();
        let err = storage
            .append_entry(Uuid::new_v4(), &LogbookEntry::Bearing(bearing))
            .unwrap_err();

        assert!(matches!(err, StorageError::VoyageNotFound(_)));
    }
}
