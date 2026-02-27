//! Logbook storage: seal the slate and load logbook entries.

use jiff::Timestamp;
use uuid::Uuid;

use crate::model::{Bearing, EntryKind, LogbookEntry, Observation, Observe, Payload};

use super::{Result, Storage, StorageError};

impl Storage {
    /// Atomically seals the slate into a bearing, records a logbook entry, and clears the slate.
    ///
    /// All three steps run in a single `SQLite` transaction — inconsistent state is impossible.
    pub fn seal_slate(
        &self,
        voyage_id: Uuid,
        identity: &str,
        timestamp: Timestamp,
        summary: &str,
        kind: &EntryKind,
    ) -> Result<()> {
        let mut conn = self.open_db(voyage_id)?;
        let action_json = serde_json::to_string(kind)?;

        let tx = conn.transaction()?;

        // Insert the logbook row.
        tx.execute(
            "INSERT INTO logbook (recorded_at, identity, action, summary)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![timestamp.to_string(), identity, action_json, summary],
        )?;
        let logbook_id = tx.last_insert_rowid();

        // Copy slate → bearing_observations in one statement.
        tx.execute(
            "INSERT INTO bearing_observations (logbook_id, target, blob_hash, observed_at)
             SELECT ?1, target, blob_hash, observed_at FROM slate",
            rusqlite::params![logbook_id],
        )?;

        // Clear the slate.
        tx.execute("DELETE FROM slate", [])?;

        tx.commit()?;
        Ok(())
    }

    /// Loads all logbook entries for a voyage, oldest first.
    // TODO: remove once a read command (e.g. `helm log show`) is wired to the CLI.
    #[allow(dead_code)]
    pub fn load_logbook(&self, voyage_id: Uuid) -> Result<Vec<LogbookEntry>> {
        let conn = self.open_db(voyage_id)?;

        // Step 1: collect all logbook rows. The stmt borrow ends before the per-row queries.
        let rows: Vec<(i64, String, String, String, String)> = {
            let mut stmt = conn.prepare(
                "SELECT id, recorded_at, identity, action, summary
                 FROM logbook
                 ORDER BY id",
            )?;
            stmt.query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?
        };

        let mut entries = Vec::new();

        for (logbook_id, recorded_at_str, identity, action_str, summary) in rows {
            // Step 2: load observations for this entry.
            let observations: Vec<Observation> = {
                let mut stmt = conn.prepare(
                    "SELECT bo.target, bo.observed_at, b.data
                     FROM bearing_observations bo
                     JOIN blobs b ON bo.blob_hash = b.hash
                     WHERE bo.logbook_id = ?1",
                )?;
                stmt.query_map(rusqlite::params![logbook_id], |row| {
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
                .collect::<Result<Vec<_>>>()?
            };

            let bearing = Bearing {
                observations,
                summary,
            };
            let timestamp = recorded_at_str
                .parse::<Timestamp>()
                .map_err(|e| StorageError::Corrupt(format!("invalid recorded_at: {e}")))?;
            let kind: EntryKind = serde_json::from_str(&action_str)?;

            entries.push(LogbookEntry {
                bearing,
                author: identity,
                timestamp,
                kind,
            });
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

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

    fn steer_kind() -> EntryKind {
        EntryKind::Steer(Steer::Comment {
            number: 42,
            body: "Here's my plan.".into(),
            target: CommentTarget::Issue,
        })
    }

    fn log_kind() -> EntryKind {
        EntryKind::Log("Waiting for review.".into())
    }

    #[test]
    fn seal_writes_logbook_and_clears_slate() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();
        storage
            .append_slate(voyage.id, &sample_observation())
            .unwrap();

        storage
            .seal_slate(
                voyage.id,
                "john-agent",
                Timestamp::now(),
                "Plan looks good",
                &steer_kind(),
            )
            .unwrap();

        // Slate is cleared.
        let slate = storage.load_slate(voyage.id).unwrap();
        assert!(slate.is_empty());

        // Logbook has one entry.
        let entries = storage.load_logbook(voyage.id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].bearing.summary, "Plan looks good");
        assert_eq!(entries[0].author, "john-agent");
        assert!(matches!(entries[0].kind, EntryKind::Steer(_)));
    }

    #[test]
    fn seal_preserves_bearing_observations() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();
        storage
            .append_slate(voyage.id, &sample_observation())
            .unwrap();

        storage
            .seal_slate(
                voyage.id,
                "john-agent",
                Timestamp::now(),
                "summary",
                &steer_kind(),
            )
            .unwrap();

        let entries = storage.load_logbook(voyage.id).unwrap();
        assert_eq!(entries[0].bearing.observations.len(), 1);
        assert!(matches!(
            entries[0].bearing.observations[0].target,
            Observe::DirectoryTree { .. }
        ));
    }

    #[test]
    fn seal_empty_slate_is_valid() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        storage
            .seal_slate(
                voyage.id,
                "john-agent",
                Timestamp::now(),
                "nothing observed",
                &log_kind(),
            )
            .unwrap();

        let entries = storage.load_logbook(voyage.id).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].bearing.observations.is_empty());
    }

    #[test]
    fn multiple_seals_accumulate_logbook_entries() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        storage
            .seal_slate(voyage.id, "a", Timestamp::now(), "first", &steer_kind())
            .unwrap();
        storage
            .seal_slate(voyage.id, "b", Timestamp::now(), "second", &log_kind())
            .unwrap();

        let entries = storage.load_logbook(voyage.id).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].bearing.summary, "first");
        assert_eq!(entries[1].bearing.summary, "second");
    }

    #[test]
    fn load_logbook_empty() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        let entries = storage.load_logbook(voyage.id).unwrap();
        assert!(entries.is_empty());
    }
}
