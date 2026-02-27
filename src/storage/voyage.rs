//! Voyage storage: create, load, update, and list voyages.

use std::{fs, io};

use rusqlite::Connection;
use uuid::Uuid;

use crate::model::{Voyage, VoyageStatus};

use super::{Result, Storage, StorageError};

impl Storage {
    /// Creates a new voyage, writing its metadata to a new `SQLite` file.
    pub fn create_voyage(&self, voyage: &Voyage) -> Result<()> {
        let conn = self.create_db(voyage.id)?;
        let (status, ended_at, ended_status) = serialize_status(&voyage.status);
        conn.execute(
            "INSERT INTO voyage (id, intent, created_at, status, ended_at, ended_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                voyage.id.to_string(),
                &voyage.intent,
                voyage.created_at.to_string(),
                status,
                ended_at,
                ended_status,
            ],
        )?;
        Ok(())
    }

    /// Updates a voyage's metadata.
    pub fn update_voyage(&self, voyage: &Voyage) -> Result<()> {
        let conn = self.open_db(voyage.id)?;
        let (status, ended_at, ended_status) = serialize_status(&voyage.status);
        let rows = conn.execute(
            "UPDATE voyage
             SET intent = ?1, created_at = ?2, status = ?3, ended_at = ?4, ended_status = ?5
             WHERE id = ?6",
            rusqlite::params![
                &voyage.intent,
                voyage.created_at.to_string(),
                status,
                ended_at,
                ended_status,
                voyage.id.to_string(),
            ],
        )?;
        if rows == 0 {
            return Err(StorageError::VoyageNotFound(voyage.id));
        }
        Ok(())
    }

    /// Loads a single voyage's metadata.
    pub fn load_voyage(&self, id: Uuid) -> Result<Voyage> {
        let conn = self.open_db(id)?;
        load_voyage_row(&conn)
    }

    /// Lists all voyages by reading each `.sqlite` file in the storage root.
    ///
    /// Unreadable or malformed files are silently skipped.
    pub fn list_voyages(&self) -> Result<Vec<Voyage>> {
        let mut voyages = Vec::new();
        let entries = match fs::read_dir(&self.root) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(voyages),
            Err(e) => return Err(e.into()),
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("sqlite") {
                continue;
            }
            let Ok(conn) = Connection::open(&path) else {
                continue;
            };
            if let Ok(v) = load_voyage_row(&conn) {
                voyages.push(v);
            }
        }
        voyages.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(voyages)
    }
}

/// Reads the single voyage row from an open connection.
fn load_voyage_row(conn: &Connection) -> Result<Voyage> {
    let (id_str, intent, created_at_str, status_str, ended_at_opt, ended_status_opt) = conn
        .query_row(
            "SELECT id, intent, created_at, status, ended_at, ended_status FROM voyage LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )?;

    let id = id_str
        .parse::<Uuid>()
        .map_err(|e| StorageError::Corrupt(format!("invalid voyage id: {e}")))?;
    let created_at = created_at_str
        .parse::<jiff::Timestamp>()
        .map_err(|e| StorageError::Corrupt(format!("invalid created_at: {e}")))?;
    let status = deserialize_status(&status_str, ended_at_opt.as_deref(), ended_status_opt)?;

    Ok(Voyage {
        id,
        intent,
        created_at,
        status,
    })
}

/// Converts a `VoyageStatus` to column values for the voyage table.
fn serialize_status(status: &VoyageStatus) -> (String, Option<String>, Option<String>) {
    match status {
        VoyageStatus::Active => ("active".to_string(), None, None),
        VoyageStatus::Ended { ended_at, status } => (
            "ended".to_string(),
            Some(ended_at.to_string()),
            status.clone(),
        ),
    }
}

/// Reconstructs a `VoyageStatus` from voyage table column values.
fn deserialize_status(
    status: &str,
    ended_at: Option<&str>,
    ended_status: Option<String>,
) -> Result<VoyageStatus> {
    match status {
        "active" => Ok(VoyageStatus::Active),
        "ended" => {
            let ended_at_str = ended_at.ok_or_else(|| {
                StorageError::Corrupt("voyage is ended but ended_at is null".into())
            })?;
            let ended_at = ended_at_str
                .parse::<jiff::Timestamp>()
                .map_err(|e| StorageError::Corrupt(format!("invalid ended_at: {e}")))?;
            Ok(VoyageStatus::Ended {
                ended_at,
                status: ended_status,
            })
        }
        other => Err(StorageError::Corrupt(format!(
            "unknown voyage status: {other}"
        ))),
    }
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
