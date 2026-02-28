//! Logbook storage: atomic seal-and-record operations.
//!
//! `record_steer` and `record_log` each perform a single transaction that:
//! 1. Reads the current slate (`target`, `artifact_hash`, `observed_at` per row).
//! 2. Inserts a logbook row.
//! 3. Copies slate rows into `bearing_observations`.
//! 4. Clears the slate.
//!
//! The slate's artifacts are already stored by the time these methods run —
//! `Storage::observe` handles that. The seal transaction only links existing
//! artifacts to the new logbook entry.

use jiff::Timestamp;
use uuid::Uuid;

use crate::model::{Bearing, EntryKind, LogbookEntry, Observation, Observe, Steer};

use super::{Result, Storage, StorageError, load_artifact};

impl Storage {
    /// Seal the slate into a bearing, record a steer entry, and clear the slate.
    ///
    /// All four steps are one atomic transaction. If any step fails, the
    /// logbook and slate are unchanged.
    pub fn record_steer(
        &self,
        voyage_id: Uuid,
        steer: &Steer,
        summary: &str,
        identity: &str,
        role: &str,
        method: &str,
    ) -> Result<()> {
        let action_json = serde_json::to_string(&EntryKind::Steer(steer.clone()))?;
        self.record_entry(voyage_id, &action_json, summary, identity, role, method)
    }

    /// Seal the slate into a bearing, record a log entry, and clear the slate.
    ///
    /// All four steps are one atomic transaction.
    pub fn record_log(
        &self,
        voyage_id: Uuid,
        status: &str,
        summary: &str,
        identity: &str,
        role: &str,
        method: &str,
    ) -> Result<()> {
        let action_json = serde_json::to_string(&EntryKind::Log(status.to_string()))?;
        self.record_entry(voyage_id, &action_json, summary, identity, role, method)
    }

    /// Load all logbook entries for a voyage.
    ///
    /// Each entry's bearing is reconstructed from `bearing_observations` joined with
    /// `artifacts`. Entries are returned in insertion order.
    // TODO: remove once `helm log show` is built.
    #[allow(dead_code)]
    pub fn load_logbook(&self, voyage_id: Uuid) -> Result<Vec<LogbookEntry>> {
        let conn = self.open_voyage(voyage_id)?;

        let rows: Vec<(i64, String, String, String, String, String, String)> = {
            let mut stmt = conn.prepare(
                "SELECT id, recorded_at, identity, action, summary,
                        COALESCE(role, ''), COALESCE(method, '')
                 FROM logbook
                 ORDER BY id",
            )?;
            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };

        rows.into_iter()
            .map(
                |(id, recorded_at_str, identity, action_json, summary, role, method)| {
                    let recorded_at = recorded_at_str
                        .parse::<Timestamp>()
                        .map_err(|e| StorageError::TimeParse(e.to_string()))?;

                    let kind: EntryKind = serde_json::from_str(&action_json)?;

                    let observations = load_bearing_observations(&conn, id)?;

                    Ok(LogbookEntry {
                        bearing: Bearing {
                            observations,
                            summary,
                        },
                        identity,
                        role,
                        method,
                        recorded_at,
                        kind,
                    })
                },
            )
            .collect()
    }
}

impl Storage {
    /// Inner implementation shared by `record_steer` and `record_log`.
    fn record_entry(
        &self,
        voyage_id: Uuid,
        action_json: &str,
        summary: &str,
        identity: &str,
        role: &str,
        method: &str,
    ) -> Result<()> {
        let mut conn = self.open_voyage(voyage_id)?;
        let tx = conn.transaction()?;

        // Collect slate rows before inserting — prepared statement borrows tx.
        let slate_rows: Vec<(String, String, String)> = {
            let mut stmt =
                tx.prepare("SELECT target, artifact_hash, observed_at FROM slate ORDER BY rowid")?;
            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };

        let now = Timestamp::now().to_string();
        tx.execute(
            "INSERT INTO logbook (recorded_at, identity, action, summary, role, method)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![now, identity, action_json, summary, role, method],
        )?;
        let logbook_id = tx.last_insert_rowid();

        for (target, artifact_hash, observed_at) in &slate_rows {
            tx.execute(
                "INSERT INTO bearing_observations
                 (logbook_id, target, artifact_hash, observed_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![logbook_id, target, artifact_hash, observed_at],
            )?;
        }

        tx.execute("DELETE FROM slate", [])?;
        tx.commit()?;

        Ok(())
    }
}

/// Load the observations stored in `bearing_observations` for a logbook entry.
fn load_bearing_observations(
    conn: &rusqlite::Connection,
    logbook_id: i64,
) -> Result<Vec<Observation>> {
    let rows: Vec<(String, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT bo.target, bo.observed_at, bo.artifact_hash
             FROM bearing_observations bo
             WHERE bo.logbook_id = ?1
             ORDER BY bo.rowid",
        )?;
        stmt.query_map(rusqlite::params![logbook_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };

    rows.into_iter()
        .map(|(target_json, observed_at_str, hash)| {
            let target: Observe = serde_json::from_str(&target_json)?;
            let observed_at = observed_at_str
                .parse::<Timestamp>()
                .map_err(|e| StorageError::TimeParse(e.to_string()))?;
            let payload = load_artifact(conn, &hash)?;
            Ok(Observation {
                target,
                payload,
                observed_at,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use jiff::Timestamp;
    use tempfile::TempDir;

    use crate::{
        model::{
            CommentTarget, DirectoryEntry, DirectoryListing, Observe, Payload, Steer, Voyage,
            VoyageStatus,
        },
        storage::Storage,
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

    fn sample_observation() -> crate::model::Observation {
        crate::model::Observation {
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
    fn record_steer_seals_and_clears_slate() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();
        storage.observe(voyage.id, &sample_observation()).unwrap();

        let steer = Steer::Comment {
            number: 42,
            body: "Here's my plan.".into(),
            target: CommentTarget::Issue,
        };
        storage
            .record_steer(
                voyage.id,
                &steer,
                "Ready to steer",
                "alice",
                "coder",
                "human",
            )
            .unwrap();

        // Slate should be empty after sealing.
        let slate = storage.load_slate(voyage.id).unwrap();
        assert!(slate.is_empty());
    }

    #[test]
    fn record_log_seals_and_clears_slate() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();
        storage.observe(voyage.id, &sample_observation()).unwrap();

        storage
            .record_log(
                voyage.id,
                "Waiting for review.",
                "All looks good",
                "alice",
                "reviewer",
                "human",
            )
            .unwrap();

        let slate = storage.load_slate(voyage.id).unwrap();
        assert!(slate.is_empty());
    }

    #[test]
    fn record_steer_on_empty_slate() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();

        // Sealing an empty slate should succeed.
        let steer = Steer::Comment {
            number: 1,
            body: "Empty slate steer.".into(),
            target: CommentTarget::Issue,
        };
        storage
            .record_steer(voyage.id, &steer, "summary", "alice", "coder", "human")
            .unwrap();
    }

    #[test]
    fn load_logbook_after_record() {
        let (_dir, storage) = test_storage();
        let voyage = sample_voyage();
        storage.create_voyage(&voyage).unwrap();
        storage.observe(voyage.id, &sample_observation()).unwrap();

        let steer = Steer::Comment {
            number: 42,
            body: "Comment body.".into(),
            target: CommentTarget::Issue,
        };
        storage
            .record_steer(
                voyage.id,
                &steer,
                "Steering now",
                "alice",
                "coder",
                "claude",
            )
            .unwrap();

        storage
            .record_log(
                voyage.id,
                "Waiting.",
                "Logged state",
                "bob",
                "reviewer",
                "human",
            )
            .unwrap();

        let entries = storage.load_logbook(voyage.id).unwrap();
        assert_eq!(entries.len(), 2);

        assert!(matches!(entries[0].kind, EntryKind::Steer(_)));
        assert_eq!(entries[0].identity, "alice");
        assert_eq!(entries[0].role, "coder");
        assert_eq!(entries[0].method, "claude");
        assert_eq!(entries[0].bearing.summary, "Steering now");
        assert_eq!(entries[0].bearing.observations.len(), 1);

        assert!(matches!(entries[1].kind, EntryKind::Log(_)));
        assert_eq!(entries[1].identity, "bob");
        // Slate was empty at the time of the log entry.
        assert_eq!(entries[1].bearing.observations.len(), 0);
    }

    #[test]
    fn record_steer_nonexistent_voyage_fails() {
        let (_dir, storage) = test_storage();
        let steer = Steer::Comment {
            number: 1,
            body: "Body.".into(),
            target: CommentTarget::Issue,
        };
        let err = storage
            .record_steer(Uuid::new_v4(), &steer, "s", "i", "r", "m")
            .unwrap_err();
        assert!(matches!(err, StorageError::VoyageNotFound(_)));
    }
}
