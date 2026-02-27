//! Slate storage: observations accumulating between steer/log commands.
//!
//! The slate is a set: one observation per target, enforced by the database
//! (`target` is the primary key). Observing the same target twice replaces
//! the older entry — newest payload wins.

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::model::{Observation, Observe, Payload};

use super::{Result, Storage, StorageError};

impl Storage {
    /// Adds an observation to the voyage's slate.
    ///
    /// If the target is already on the slate, the previous observation is replaced
    /// (set semantics — one observation per target, newest wins).
    pub fn append_slate(&self, voyage_id: Uuid, observation: &Observation) -> Result<()> {
        let conn = self.open_db(voyage_id)?;

        // Serialize and compress the payload.
        let payload_json = serde_json::to_vec(&observation.payload)?;
        let hash = hex::encode(Sha256::digest(&payload_json));
        let compressed = zstd::encode_all(payload_json.as_slice(), 3)?;

        // Insert the blob if it isn't already stored.
        conn.execute(
            "INSERT OR IGNORE INTO blobs (hash, data) VALUES (?1, ?2)",
            rusqlite::params![hash, compressed],
        )?;

        // Upsert the slate entry. Replaces any previous observation for this target.
        let target_json = serde_json::to_string(&observation.target)?;
        conn.execute(
            "INSERT OR REPLACE INTO slate (target, blob_hash, observed_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![target_json, hash, observation.observed_at.to_string()],
        )?;

        Ok(())
    }

    /// Loads all observations from the voyage's slate.
    ///
    /// Returns an empty vec if the slate is empty.
    pub fn load_slate(&self, voyage_id: Uuid) -> Result<Vec<Observation>> {
        let conn = self.open_db(voyage_id)?;
        let mut stmt = conn.prepare(
            "SELECT s.target, s.observed_at, b.data
             FROM slate s
             JOIN blobs b ON s.blob_hash = b.hash",
        )?;
        let observations = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })?
            .map(|r| -> Result<Observation> {
                let (target_str, observed_at_str, compressed) = r?;
                let target: Observe = serde_json::from_str(&target_str)?;
                let payload_json = zstd::decode_all(compressed.as_slice())?;
                let payload: Payload = serde_json::from_slice(&payload_json)?;
                let observed_at = observed_at_str
                    .parse::<jiff::Timestamp>()
                    .map_err(|e| StorageError::Corrupt(format!("invalid observed_at: {e}")))?;
                Ok(Observation {
                    target,
                    payload,
                    observed_at,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(observations)
    }

    /// Clears the voyage's slate without sealing.
    ///
    /// Idempotent: does nothing if the slate is already empty.
    pub fn clear_slate(&self, voyage_id: Uuid) -> Result<()> {
        let conn = self.open_db(voyage_id)?;
        conn.execute("DELETE FROM slate", [])?;
        Ok(())
    }

    /// Removes a single observation from the slate by target.
    ///
    /// Returns `true` if an entry was erased, `false` if the target was not on the slate.
    /// Idempotent: calling again after the target is gone returns `false` without error.
    pub fn erase_slate(&self, voyage_id: Uuid, target: &Observe) -> Result<bool> {
        let conn = self.open_db(voyage_id)?;
        let target_json = serde_json::to_string(target)?;
        let rows = conn.execute(
            "DELETE FROM slate WHERE target = ?1",
            rusqlite::params![target_json],
        )?;
        Ok(rows > 0)
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

    fn issue_observation(number: u64) -> Observation {
        Observation {
            target: Observe::GitHubIssue { number },
            payload: Payload::DirectoryTree { listings: vec![] },
            observed_at: Timestamp::now(),
        }
    }

    #[test]
    fn append_and_load_slate() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        storage
            .append_slate(voyage.id, &sample_observation())
            .unwrap();
        storage
            .append_slate(voyage.id, &issue_observation(42))
            .unwrap();

        let loaded = storage.load_slate(voyage.id).unwrap();
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn append_slate_set_semantics_replaces_same_target() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        // Observe issue #42 twice — second should replace first.
        storage
            .append_slate(voyage.id, &issue_observation(42))
            .unwrap();
        storage
            .append_slate(voyage.id, &issue_observation(42))
            .unwrap();

        let loaded = storage.load_slate(voyage.id).unwrap();
        assert_eq!(loaded.len(), 1, "set semantics: only one entry per target");
        assert!(matches!(
            loaded[0].target,
            Observe::GitHubIssue { number: 42 }
        ));
    }

    #[test]
    fn load_slate_empty() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        let loaded = storage.load_slate(voyage.id).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn clear_slate_removes_all_entries() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        storage
            .append_slate(voyage.id, &sample_observation())
            .unwrap();
        storage.clear_slate(voyage.id).unwrap();

        let loaded = storage.load_slate(voyage.id).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn clear_slate_idempotent() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        // Clear with empty slate — should not error.
        storage.clear_slate(voyage.id).unwrap();
    }

    #[test]
    fn erase_slate_removes_target() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        storage
            .append_slate(voyage.id, &sample_observation())
            .unwrap();
        storage
            .append_slate(voyage.id, &issue_observation(42))
            .unwrap();

        let target = Observe::GitHubIssue { number: 42 };
        let erased = storage.erase_slate(voyage.id, &target).unwrap();

        assert!(erased);
        let loaded = storage.load_slate(voyage.id).unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(matches!(loaded[0].target, Observe::DirectoryTree { .. }));
    }

    #[test]
    fn erase_slate_returns_false_when_target_not_present() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        let target = Observe::GitHubIssue { number: 99 };
        let erased = storage.erase_slate(voyage.id, &target).unwrap();

        assert!(!erased);
    }

    #[test]
    fn erase_slate_idempotent() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        storage
            .append_slate(voyage.id, &issue_observation(42))
            .unwrap();

        let target = Observe::GitHubIssue { number: 42 };
        storage.erase_slate(voyage.id, &target).unwrap();
        // Second erase should not error, just return false.
        let erased = storage.erase_slate(voyage.id, &target).unwrap();
        assert!(!erased);
    }

    #[test]
    fn append_slate_nonexistent_voyage_fails() {
        let (_dir, storage) = test_storage();
        let err = storage
            .append_slate(Uuid::new_v4(), &sample_observation())
            .unwrap_err();
        assert!(matches!(err, StorageError::VoyageNotFound(_)));
    }

    #[test]
    fn load_slate_nonexistent_voyage_fails() {
        let (_dir, storage) = test_storage();
        let err = storage.load_slate(Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, StorageError::VoyageNotFound(_)));
    }

    #[test]
    fn clear_slate_nonexistent_voyage_fails() {
        let (_dir, storage) = test_storage();
        let err = storage.clear_slate(Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, StorageError::VoyageNotFound(_)));
    }
}
