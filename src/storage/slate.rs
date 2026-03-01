//! Slate storage: observations accumulating between steer/log commands.
//!
//! The slate is a set keyed by observation target — `INSERT OR REPLACE`
//! enforces one observation per target at write time.
//! Payloads are stored as compressed, content-addressed artifacts.

use uuid::Uuid;

use crate::model::{Observation, Observe};

use super::{Result, Storage, load_artifact, store_artifact};

impl Storage {
    /// Add an observation to the slate for a voyage.
    ///
    /// The payload is stored as a content-addressed artifact.
    /// If the same target was observed before, this replaces the previous entry.
    pub fn observe(&self, voyage_id: Uuid, observation: &Observation) -> Result<()> {
        let conn = self.open_voyage(voyage_id)?;

        let artifact_hash = store_artifact(&conn, &observation.payload)?;
        let target_json = serde_json::to_string(&observation.target)?;
        let observed_at = observation.observed_at.to_string();

        conn.execute(
            "INSERT OR REPLACE INTO slate (target, artifact_hash, observed_at)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![target_json, artifact_hash, observed_at],
        )?;

        Ok(())
    }

    /// Load all observations currently on the slate for a voyage.
    pub fn load_slate(&self, voyage_id: Uuid) -> Result<Vec<Observation>> {
        let conn = self.open_voyage(voyage_id)?;

        let mut stmt = conn.prepare(
            "SELECT s.target, s.observed_at, s.artifact_hash
             FROM slate s
             ORDER BY rowid",
        )?;

        let observations = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .map(|r| {
                let (target_json, observed_at_str, hash) = r?;

                let target: Observe = serde_json::from_str(&target_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                let observed_at = observed_at_str.parse().map_err(|e: jiff::Error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                Ok((target, observed_at, hash))
            })
            .collect::<rusqlite::Result<Vec<_>>>()?;

        observations
            .into_iter()
            .map(|(target, observed_at, hash)| {
                let payload = load_artifact(&conn, &hash)?;
                Ok(Observation {
                    target,
                    payload,
                    observed_at,
                })
            })
            .collect()
    }

    /// Erase a specific target from the slate.
    ///
    /// Returns `true` if the target was present and erased, `false` if it was not on the slate.
    pub fn erase_from_slate(&self, voyage_id: Uuid, target: &Observe) -> Result<bool> {
        let conn = self.open_voyage(voyage_id)?;
        let target_json = serde_json::to_string(target)?;
        let rows = conn.execute(
            "DELETE FROM slate WHERE target = ?1",
            rusqlite::params![target_json],
        )?;
        Ok(rows > 0)
    }

    /// Clear the entire slate without sealing.
    ///
    /// Wipes all observations without creating a logbook entry.
    /// Idempotent: safe to call on an already-empty slate.
    pub fn clear_slate(&self, voyage_id: Uuid) -> Result<()> {
        let conn = self.open_voyage(voyage_id)?;
        conn.execute("DELETE FROM slate", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use jiff::Timestamp;
    use tempfile::TempDir;

    use crate::{
        model::{DirectoryEntry, DirectoryListing, Payload, Voyage, VoyageStatus},
        storage::StorageError,
    };

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

    fn sample_observation(target: Observe) -> Observation {
        Observation {
            target,
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
    fn observe_and_load_slate() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        let obs = sample_observation(Observe::GitHubIssue { number: 1 });
        storage.observe(voyage.id, &obs).unwrap();

        let loaded = storage.load_slate(voyage.id).unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(matches!(
            loaded[0].target,
            Observe::GitHubIssue { number: 1 }
        ));
    }

    #[test]
    fn observe_same_target_replaces() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        let target = Observe::GitHubIssue { number: 42 };
        let obs1 = sample_observation(target.clone());
        let obs2 = sample_observation(target);

        storage.observe(voyage.id, &obs1).unwrap();
        storage.observe(voyage.id, &obs2).unwrap();

        // Same target observed twice — slate still has one entry.
        let loaded = storage.load_slate(voyage.id).unwrap();
        assert_eq!(loaded.len(), 1);
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
    fn erase_from_slate_removes_target() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        let target1 = Observe::GitHubIssue { number: 1 };
        let target2 = Observe::GitHubIssue { number: 2 };

        storage
            .observe(voyage.id, &sample_observation(target1.clone()))
            .unwrap();
        storage
            .observe(voyage.id, &sample_observation(target2))
            .unwrap();

        let erased = storage.erase_from_slate(voyage.id, &target1).unwrap();
        assert!(erased);

        let loaded = storage.load_slate(voyage.id).unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(matches!(
            loaded[0].target,
            Observe::GitHubIssue { number: 2 }
        ));
    }

    #[test]
    fn erase_from_slate_returns_false_when_not_present() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        let target = Observe::GitHubIssue { number: 99 };
        let erased = storage.erase_from_slate(voyage.id, &target).unwrap();
        assert!(!erased);
    }

    #[test]
    fn clear_slate_removes_all() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        storage
            .observe(
                voyage.id,
                &sample_observation(Observe::GitHubIssue { number: 1 }),
            )
            .unwrap();
        storage
            .observe(
                voyage.id,
                &sample_observation(Observe::GitHubIssue { number: 2 }),
            )
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

        // Clear with nothing on the slate — should not error.
        storage.clear_slate(voyage.id).unwrap();
    }

    #[test]
    fn observe_nonexistent_voyage_fails() {
        let (_dir, storage) = test_storage();
        let obs = sample_observation(Observe::GitHubIssue { number: 1 });
        let err = storage.observe(Uuid::new_v4(), &obs).unwrap_err();
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
