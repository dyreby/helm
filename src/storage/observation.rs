//! Observation storage: store observations with sequential ID assignment.

use std::fs;

use uuid::Uuid;

use crate::model::Observation;

use super::{Result, Storage, StorageError};

impl Storage {
    /// Stores an observation as a numbered JSON file in the voyage's
    /// `observations/` directory. Returns the assigned ID.
    ///
    /// IDs are linear integers scoped to the voyage: `1.json`, `2.json`, etc.
    pub fn store_observation(&self, voyage_id: Uuid, observation: &Observation) -> Result<u64> {
        let dir = self.voyage_dir(voyage_id);
        if !dir.exists() {
            return Err(StorageError::VoyageNotFound(voyage_id));
        }
        let obs_dir = dir.join("observations");
        fs::create_dir_all(&obs_dir)?;

        let id = self.next_observation_id(voyage_id)?;
        let json = serde_json::to_string_pretty(observation)?;
        fs::write(obs_dir.join(format!("{id}.json")), json)?;
        Ok(id)
    }

    /// Returns the next observation ID for a voyage.
    ///
    /// Scans the `observations/` directory for the highest existing ID
    /// and returns one higher. Returns 1 if no observations exist.
    fn next_observation_id(&self, voyage_id: Uuid) -> Result<u64> {
        let obs_dir = self.voyage_dir(voyage_id).join("observations");
        if !obs_dir.exists() {
            return Ok(1);
        }
        let mut max_id: u64 = 0;
        for entry in fs::read_dir(&obs_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(stem) = name.strip_suffix(".json")
                && let Ok(id) = stem.parse::<u64>()
            {
                max_id = max_id.max(id);
            }
        }
        Ok(max_id + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

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

    fn sample_observation() -> Observation {
        Observation {
            mark: Mark::DirectoryTree {
                root: PathBuf::from("src/"),
                skip: vec![],
                max_depth: None,
            },
            sighting: Sighting::DirectoryTree {
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
    fn store_observation_assigns_sequential_ids() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        let obs = sample_observation();
        let id1 = storage.store_observation(voyage.id, &obs).unwrap();
        let id2 = storage.store_observation(voyage.id, &obs).unwrap();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn store_observation_creates_json_file() {
        let (dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        let obs = sample_observation();
        let id = storage.store_observation(voyage.id, &obs).unwrap();

        let obs_path = dir
            .path()
            .join("voyages")
            .join(voyage.id.to_string())
            .join("observations")
            .join(format!("{id}.json"));
        assert!(obs_path.exists());

        // Verify it round-trips.
        let json = fs::read_to_string(obs_path).unwrap();
        let loaded: Observation = serde_json::from_str(&json).unwrap();
        assert!(matches!(loaded.mark, Mark::DirectoryTree { .. }));
    }

    #[test]
    fn store_observation_nonexistent_voyage_fails() {
        let (_dir, storage) = test_storage();
        let obs = sample_observation();
        let err = storage.store_observation(Uuid::new_v4(), &obs).unwrap_err();

        assert!(matches!(err, StorageError::VoyageNotFound(_)));
    }
}
