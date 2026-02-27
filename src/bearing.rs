//! Bearing construction: creating observations from targets and sealing bearings.
//!
//! `observe` wraps the observe module to produce timestamped `Observation` values.
//! `seal` builds a `Bearing` from the slate, deduplicating by target and
//! keeping the newest observation per target. Caller-side pruning (removing
//! unwanted observations) happens before seal via `helm slate erase`.

use std::path::Path;

use crate::model::{Bearing, Observation, Observe};

/// Observe a target and return a timestamped observation.
///
/// Pure read — never modifies the world.
/// GitHub targets require `gh_config_dir` for authentication.
pub fn observe(target: &Observe, gh_config_dir: Option<&Path>) -> Observation {
    let payload = crate::observe::observe(target, gh_config_dir);

    Observation {
        target: target.clone(),
        payload,
        observed_at: jiff::Timestamp::now(),
    }
}

/// Seal the slate into a bearing.
///
/// Deduplicates by target, keeping the newest observation when the same target was observed
/// multiple times.
/// Chronological order is preserved in the output.
///
/// Note: with `SQLite` storage, the slate enforces set semantics at write time.
/// The dedup logic here is a no-op in production but retained for the unit tests below.
// TODO: remove once `helm log show` or similar is built and this can be validated end-to-end.
#[allow(dead_code)]
pub fn seal(observations: Vec<Observation>, summary: String) -> Bearing {
    // Iterate in reverse (newest first), keep the first occurrence of each target,
    // then reverse to restore chronological order.
    let mut seen: Vec<Observe> = Vec::new();
    let mut deduped: Vec<Observation> = Vec::new();

    for obs in observations.into_iter().rev() {
        if !seen.contains(&obs.target) {
            seen.push(obs.target.clone());
            deduped.push(obs);
        }
    }

    deduped.reverse();

    Bearing {
        observations: deduped,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use jiff::Timestamp;

    use crate::model::{DirectoryEntry, DirectoryListing, Observe, Payload};

    fn obs(target: Observe) -> Observation {
        Observation {
            target,
            payload: Payload::DirectoryTree {
                listings: vec![DirectoryListing {
                    path: PathBuf::from("."),
                    entries: vec![DirectoryEntry {
                        name: "main.rs".into(),
                        is_dir: false,
                        size_bytes: Some(1),
                    }],
                }],
            },
            observed_at: Timestamp::now(),
        }
    }

    #[test]
    fn seal_deduplicates_by_target_keeps_newest() {
        let issue_42_old = obs(Observe::GitHubIssue { number: 42 });
        let file_obs = obs(Observe::DirectoryTree {
            root: PathBuf::from("src/"),
            skip: vec![],
            max_depth: None,
        });
        let issue_42_new = obs(Observe::GitHubIssue { number: 42 });

        let observations = vec![issue_42_old.clone(), file_obs.clone(), issue_42_new.clone()];

        let bearing = seal(observations, "summary".into());

        // Deduped: file_obs and issue_42_new (oldest issue_42 dropped).
        assert_eq!(bearing.observations.len(), 2);
        assert!(matches!(
            bearing.observations[0].target,
            Observe::DirectoryTree { .. }
        ));
        assert!(matches!(
            bearing.observations[1].target,
            Observe::GitHubIssue { number: 42 }
        ));
        assert_eq!(
            bearing.observations[1].observed_at,
            issue_42_new.observed_at
        );
    }

    #[test]
    fn seal_preserves_order_when_no_duplicates() {
        let a = obs(Observe::GitHubIssue { number: 1 });
        let b = obs(Observe::GitHubIssue { number: 2 });
        let c = obs(Observe::GitHubIssue { number: 3 });

        let bearing = seal(vec![a, b, c], "summary".into());

        assert_eq!(bearing.observations.len(), 3);
        assert!(matches!(
            bearing.observations[0].target,
            Observe::GitHubIssue { number: 1 }
        ));
        assert!(matches!(
            bearing.observations[2].target,
            Observe::GitHubIssue { number: 3 }
        ));
    }

    #[test]
    fn seal_empty_slate() {
        let bearing = seal(vec![], "nothing to see".into());
        assert!(bearing.observations.is_empty());
        assert_eq!(bearing.summary, "nothing to see");
    }
}
