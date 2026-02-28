//! Voyage storage: create, load, update, and list voyages.

use jiff::Timestamp;
use uuid::Uuid;

use crate::model::{Voyage, VoyageStatus};

use super::{Result, SCHEMA_DDL, Storage, StorageError};

impl Storage {
    /// Creates a new voyage, initialising a fresh `SQLite` database for it.
    pub fn create_voyage(&self, voyage: &Voyage) -> Result<()> {
        let path = self.voyage_path(voyage.id);
        if path.exists() {
            return Err(StorageError::VoyageAlreadyExists(voyage.id));
        }
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch(SCHEMA_DDL)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let (status, ended_at, ended_status) = encode_status(&voyage.status);
        conn.execute(
            "INSERT INTO voyage (id, intent, created_at, status, ended_at, ended_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                voyage.id.to_string(),
                voyage.intent,
                voyage.created_at.to_string(),
                status,
                ended_at,
                ended_status,
            ],
        )?;

        Ok(())
    }

    /// Updates a voyage's metadata (used to transition status to ended).
    pub fn update_voyage(&self, voyage: &Voyage) -> Result<()> {
        let conn = self.open_voyage(voyage.id)?;
        let (status, ended_at, ended_status) = encode_status(&voyage.status);
        let affected = conn.execute(
            "UPDATE voyage SET status = ?1, ended_at = ?2, ended_status = ?3 WHERE id = ?4",
            rusqlite::params![status, ended_at, ended_status, voyage.id.to_string()],
        )?;

        if affected == 0 {
            return Err(StorageError::VoyageNotFound(voyage.id));
        }

        Ok(())
    }

    /// Loads a single voyage's metadata from its database.
    pub fn load_voyage(&self, id: Uuid) -> Result<Voyage> {
        let conn = self.open_voyage(id)?;
        conn.query_row(
            "SELECT id, intent, created_at, status, ended_at, ended_status FROM voyage LIMIT 1",
            [],
            decode_voyage,
        )
        .map_err(StorageError::from)
    }

    /// Lists all voyages by scanning the storage root for `*.sqlite` files.
    ///
    /// Old JSONL voyage directories are ignored.
    pub fn list_voyages(&self) -> Result<Vec<Voyage>> {
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };

        let mut voyages = Vec::new();
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            let is_sqlite = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e == "sqlite");
            if !is_sqlite {
                continue;
            }

            // Parse the UUID from the filename; skip non-UUID filenames.
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(id) = stem.parse::<Uuid>() else {
                continue;
            };

            match self.load_voyage(id) {
                Ok(v) => voyages.push(v),
                Err(StorageError::Db(_)) => {} // Corrupted or unrelated file; skip.
                Err(e) => return Err(e),
            }
        }

        voyages.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(voyages)
    }
}

/// Encode a `VoyageStatus` into its SQL column values.
fn encode_status(status: &VoyageStatus) -> (&'static str, Option<String>, Option<String>) {
    match status {
        VoyageStatus::Active => ("active", None, None),
        VoyageStatus::Ended { ended_at, status } => {
            ("ended", Some(ended_at.to_string()), status.clone())
        }
    }
}

/// Decode a voyage row from a rusqlite `Row`.
fn decode_voyage(row: &rusqlite::Row<'_>) -> rusqlite::Result<Voyage> {
    let id_str: String = row.get(0)?;
    let intent: String = row.get(1)?;
    let created_at_str: String = row.get(2)?;
    let status_str: String = row.get(3)?;
    let ended_at_str: Option<String> = row.get(4)?;
    let ended_status: Option<String> = row.get(5)?;

    let id = id_str.parse::<Uuid>().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;

    let created_at = created_at_str.parse::<Timestamp>().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
    })?;

    let status = match status_str.as_str() {
        "active" => VoyageStatus::Active,
        "ended" => {
            let ended_at = ended_at_str
                .as_deref()
                .unwrap_or_default()
                .parse::<Timestamp>()
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
            VoyageStatus::Ended {
                ended_at,
                status: ended_status,
            }
        }
        _ => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                format!("unknown voyage status: {status_str}").into(),
            ));
        }
    };

    Ok(Voyage {
        id,
        intent,
        created_at,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use jiff::Timestamp;
    use tempfile::TempDir;

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
        voyage.status = VoyageStatus::Ended {
            ended_at: Timestamp::now(),
            status: Some("Done.".into()),
        };
        storage.update_voyage(&voyage).unwrap();

        let loaded = storage.load_voyage(voyage.id).unwrap();
        assert!(matches!(loaded.status, VoyageStatus::Ended { .. }));
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
