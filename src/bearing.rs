#![allow(dead_code)]

//! Bearing construction: the domain logic for taking a bearing.
//!
//! A bearing is built in two steps:
//!
//! 1. **Observe** — take one or more observations of the world.
//!    Each observation captures a subject and its sighting.
//! 2. **Record** — select the observations that matter,
//!    attach a position, and seal the bearing.

use uuid::Uuid;

use crate::model::{Bearing, Observation, Position, Subject};

/// Take a single observation: look at a subject and record what was seen.
///
/// Returns a self-contained `Observation` with its own ID and timestamp.
/// The caller decides whether to include it in a bearing or discard it.
pub fn observe(subject: &Subject) -> Observation {
    let sighting = crate::observe::observe(subject);

    Observation {
        id: Uuid::new_v4(),
        subject: subject.clone(),
        sighting,
        observed_at: jiff::Timestamp::now(),
    }
}

/// Assemble a bearing from observations and a position.
///
/// Call this after taking one or more observations.
/// The bearing seals the selected observations with your read on the world.
pub fn record_bearing(
    observations: Vec<Observation>,
    position_text: String,
) -> Result<Bearing, &'static str> {
    if observations.is_empty() {
        return Err("bearing must include at least one observation");
    }

    if position_text.trim().is_empty() {
        return Err("position text cannot be empty");
    }

    Ok(Bearing {
        id: Uuid::new_v4(),
        observations,
        position: Position {
            text: position_text,
            history: Vec::new(),
        },
        taken_at: jiff::Timestamp::now(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use crate::model::Sighting;

    use super::*;

    #[test]
    fn observe_then_record() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.txt"), "hello").unwrap();

        let subject = Subject::Files {
            scope: vec![dir.path().to_path_buf()],
            focus: vec![dir.path().join("test.txt")],
        };

        // Step 1: observe.
        let observation = observe(&subject);

        match &observation.sighting {
            Sighting::Files {
                survey,
                inspections,
            } => {
                assert_eq!(survey.len(), 1);
                assert_eq!(inspections.len(), 1);
            }
        }

        // Step 2: record with position.
        let bearing = record_bearing(vec![observation], "One test file.".to_string()).unwrap();
        assert_eq!(bearing.position.text, "One test file.");
        assert_eq!(bearing.observations.len(), 1);
    }

    #[test]
    fn multiple_observations_in_one_bearing() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "aaa").unwrap();
        fs::write(dir.path().join("b.txt"), "bbb").unwrap();

        let obs1 = observe(&Subject::Files {
            scope: vec![dir.path().to_path_buf()],
            focus: vec![],
        });

        let obs2 = observe(&Subject::Files {
            scope: vec![],
            focus: vec![dir.path().join("a.txt")],
        });

        let bearing = record_bearing(
            vec![obs1, obs2],
            "Directory has two files, inspected one.".to_string(),
        )
        .unwrap();

        assert_eq!(bearing.observations.len(), 2);
    }

    #[test]
    fn rejects_empty_observations() {
        let err = record_bearing(vec![], "Some position.".to_string()).unwrap_err();
        assert_eq!(err, "bearing must include at least one observation");
    }

    #[test]
    fn rejects_empty_position() {
        let dir = TempDir::new().unwrap();
        let observation = observe(&Subject::Files {
            scope: vec![dir.path().to_path_buf()],
            focus: vec![],
        });

        let err = record_bearing(vec![observation], "  ".to_string()).unwrap_err();
        assert_eq!(err, "position text cannot be empty");
    }

    #[test]
    fn discard_observation_not_in_bearing() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "aaa").unwrap();

        let keep = observe(&Subject::Files {
            scope: vec![dir.path().to_path_buf()],
            focus: vec![],
        });

        // Take another observation but don't include it.
        let _discard = observe(&Subject::Files {
            scope: vec![],
            focus: vec![dir.path().join("a.txt")],
        });

        let bearing = record_bearing(vec![keep], "Only the survey matters.".to_string()).unwrap();
        assert_eq!(bearing.observations.len(), 1);
    }
}
