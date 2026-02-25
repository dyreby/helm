#![allow(dead_code)]

//! Bearing construction: the domain logic for taking a bearing.
//!
//! A bearing is built in two steps:
//!
//! 1. **Observe** — execute a plan and get back the plan + moment.
//!    The caller reviews the moment before proceeding.
//! 2. **Record** — attach a position to the observation, producing
//!    the final immutable bearing.

use crate::model::{Bearing, BearingPlan, Moment, Observation, Position};
use crate::observe;

/// Execute a bearing plan and return the plan paired with what was observed.
///
/// The caller reviews the moment, then calls `record_bearing` with a position.
pub fn observe_bearing(plan: BearingPlan) -> Result<(BearingPlan, Moment), &'static str> {
    if plan.sources.is_empty() {
        return Err("bearing plan must have at least one source query");
    }

    let observations: Vec<Observation> = plan.sources.iter().map(observe::observe).collect();
    let moment = Moment { observations };

    Ok((plan, moment))
}

/// Assemble a bearing from an observed plan, moment, and position text.
///
/// Call this after reviewing the moment from `observe_bearing`.
pub fn record_bearing(
    plan: BearingPlan,
    moment: Moment,
    position_text: String,
) -> Result<Bearing, &'static str> {
    if position_text.trim().is_empty() {
        return Err("position text cannot be empty");
    }

    Ok(Bearing {
        plan,
        moment,
        position: Position {
            text: position_text,
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

    use crate::model::{Observation, SourceQuery};

    #[test]
    fn observe_then_record() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.txt"), "hello").unwrap();

        let plan = BearingPlan {
            sources: vec![SourceQuery::Files {
                scope: vec![dir.path().to_path_buf()],
                focus: vec![dir.path().join("test.txt")],
            }],
        };

        // Step 1: observe.
        let (plan, moment) = observe_bearing(plan).unwrap();

        // Caller reviews the moment.
        match &moment.observations[0] {
            Observation::Files {
                survey,
                inspections,
            } => {
                assert_eq!(survey.len(), 1);
                assert_eq!(inspections.len(), 1);
            }
        }

        // Step 2: record with position.
        let bearing = record_bearing(plan, moment, "One test file.".to_string()).unwrap();
        assert_eq!(bearing.position.text, "One test file.");
        assert_eq!(bearing.plan.sources.len(), 1);
    }

    #[test]
    fn rejects_empty_plan() {
        let plan = BearingPlan { sources: vec![] };
        let err = observe_bearing(plan).unwrap_err();
        assert_eq!(err, "bearing plan must have at least one source query");
    }

    #[test]
    fn rejects_empty_position() {
        let dir = TempDir::new().unwrap();
        let plan = BearingPlan {
            sources: vec![SourceQuery::Files {
                scope: vec![dir.path().to_path_buf()],
                focus: vec![],
            }],
        };

        let (plan, moment) = observe_bearing(plan).unwrap();
        let err = record_bearing(plan, moment, "  ".to_string()).unwrap_err();
        assert_eq!(err, "position text cannot be empty");
    }

    #[test]
    fn survey_only_bearing() {
        let dir = TempDir::new().unwrap();
        let plan = BearingPlan {
            sources: vec![SourceQuery::Files {
                scope: vec![dir.path().to_path_buf()],
                focus: vec![],
            }],
        };

        let (plan, moment) = observe_bearing(plan).unwrap();
        let bearing = record_bearing(plan, moment, "Survey only.".to_string()).unwrap();
        match &bearing.moment.observations[0] {
            Observation::Files { inspections, .. } => {
                assert!(inspections.is_empty());
            }
        }
    }
}
