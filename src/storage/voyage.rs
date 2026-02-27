//! Voyage storage: create, load, update, and list voyages.

use std::{fs, io};

use uuid::Uuid;

use crate::model::Voyage;

use super::{Result, Storage, StorageError};

impl Storage {
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
            identity: "john-agent".into(),
            kind: VoyageKind::OpenWaters,
            intent: "Fix the widget".into(),
            created_at: Timestamp::now(),
            status: VoyageStatus::Active,
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
            completed_at: Timestamp::now(),
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
}
