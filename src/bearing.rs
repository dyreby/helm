//! Bearing construction: the domain logic for taking a bearing.
//!
//! A bearing is built in two steps:
//!
//! 1. **Observe** — take one or more observations of the world.
//!    Each observation captures a mark and its sighting.
//! 2. **Record** — select the observations that matter,
//!    attach a reading, and seal the bearing.
//!
//! Observations are stored as separate artifacts.
//! The bearing records marks and observation references — lightweight,
//! always in the logbook.

use std::path::Path;

use crate::model::{Bearing, Mark, Observation, Reading};

/// Take a single observation: look at a mark and record what was seen.
///
/// Returns a self-contained `Observation` with its own timestamp.
/// The caller decides whether to include it in a bearing or discard it.
/// GitHub marks require `gh_config_dir` for authentication.
pub fn observe(mark: &Mark, gh_config_dir: Option<&Path>) -> Observation {
    let sighting = crate::observe::observe(mark, gh_config_dir);

    Observation {
        mark: mark.clone(),
        sighting,
        observed_at: jiff::Timestamp::now(),
    }
}

/// Assemble a bearing from observations and a reading.
///
/// Extracts marks from the observations and pairs them with
/// the given observation references (storage IDs).
/// The observations themselves are stored separately by the caller.
pub fn record_bearing(
    observations: &[Observation],
    observation_refs: Vec<u64>,
    reading_text: String,
) -> Result<Bearing, &'static str> {
    if observations.is_empty() {
        return Err("bearing must include at least one observation");
    }

    if reading_text.trim().is_empty() {
        return Err("reading text cannot be empty");
    }

    let marks: Vec<Mark> = observations.iter().map(|o| o.mark.clone()).collect();

    Ok(Bearing {
        marks,
        observation_refs,
        reading: Reading {
            text: reading_text,
            history: Vec::new(),
        },
        taken_at: jiff::Timestamp::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use tempfile::TempDir;

    use crate::model::Sighting;

    #[test]
    fn observe_then_record() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.txt"), "hello").unwrap();

        let mark = Mark::FileContents {
            paths: vec![dir.path().join("test.txt")],
        };

        // Step 1: observe.
        let observation = observe(&mark, None);

        let Sighting::FileContents { contents } = &observation.sighting else {
            panic!("expected FileContents sighting");
        };
        assert_eq!(contents.len(), 1);

        // Step 2: record with reading.
        let bearing =
            record_bearing(&[observation], vec![1], "One test file.".to_string()).unwrap();
        assert_eq!(bearing.reading.text, "One test file.");
        assert_eq!(bearing.marks.len(), 1);
        assert_eq!(bearing.observation_refs, vec![1]);
    }

    #[test]
    fn multiple_observations_in_one_bearing() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "aaa").unwrap();
        fs::write(dir.path().join("b.txt"), "bbb").unwrap();

        let obs1 = observe(
            &Mark::DirectoryTree {
                root: dir.path().to_path_buf(),
                skip: vec![],
                max_depth: None,
            },
            None,
        );

        let obs2 = observe(
            &Mark::FileContents {
                paths: vec![dir.path().join("a.txt")],
            },
            None,
        );

        let bearing = record_bearing(
            &[obs1, obs2],
            vec![1, 2],
            "Directory has two files, inspected one.".to_string(),
        )
        .unwrap();

        assert_eq!(bearing.marks.len(), 2);
        assert_eq!(bearing.observation_refs, vec![1, 2]);
    }

    #[test]
    fn rejects_empty_observations() {
        let err = record_bearing(&[], vec![], "Some reading.".to_string()).unwrap_err();
        assert_eq!(err, "bearing must include at least one observation");
    }

    #[test]
    fn rejects_empty_reading() {
        let dir = TempDir::new().unwrap();
        let observation = observe(
            &Mark::DirectoryTree {
                root: dir.path().to_path_buf(),
                skip: vec![],
                max_depth: None,
            },
            None,
        );

        let err = record_bearing(&[observation], vec![1], "  ".to_string()).unwrap_err();
        assert_eq!(err, "reading text cannot be empty");
    }

    #[test]
    fn discard_observation_not_in_bearing() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "aaa").unwrap();

        let keep = observe(
            &Mark::DirectoryTree {
                root: dir.path().to_path_buf(),
                skip: vec![],
                max_depth: None,
            },
            None,
        );

        // Take another observation but don't include it.
        let _discard = observe(
            &Mark::FileContents {
                paths: vec![dir.path().join("a.txt")],
            },
            None,
        );

        let bearing =
            record_bearing(&[keep], vec![1], "Only the survey matters.".to_string()).unwrap();
        assert_eq!(bearing.marks.len(), 1);
        assert_eq!(bearing.observation_refs, vec![1]);
    }
}
