#![allow(dead_code)]

//! Bearing construction: the domain logic for taking a bearing.
//!
//! A bearing is the record of a single observation: what was planned,
//! what was seen, and what it means. The plan is constructed by the
//! caller (with or without LLM assistance), and this module handles
//! execution and assembly.

use crate::model::{Bearing, BearingPlan, Moment, Observation, Position};
use crate::observe;

/// Execute a bearing plan and assemble the bearing with the given position.
///
/// The plan describes what to observe (source queries across any domain).
/// Each query is executed to produce observations, which form the moment.
/// The position is a human-approved statement of what the world looks like.
pub fn take_bearing(plan: BearingPlan, position_text: String) -> Result<Bearing, &'static str> {
    if plan.sources.is_empty() {
        return Err("bearing plan must have at least one source query");
    }
    if position_text.trim().is_empty() {
        return Err("position text cannot be empty");
    }

    let observations: Vec<Observation> = plan.sources.iter().map(observe::observe).collect();
    let moment = Moment { observations };

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
    fn takes_bearing_with_survey_and_inspection() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.txt"), "hello").unwrap();

        let plan = BearingPlan {
            sources: vec![SourceQuery::Files {
                scope: vec![dir.path().to_path_buf()],
                focus: vec![dir.path().join("test.txt")],
            }],
        };

        let bearing = take_bearing(plan, "One test file.".to_string()).unwrap();
        assert_eq!(bearing.position.text, "One test file.");
        assert_eq!(bearing.plan.sources.len(), 1);

        match &bearing.moment.observations[0] {
            Observation::Files {
                survey,
                inspections,
            } => {
                assert_eq!(survey.len(), 1);
                assert_eq!(inspections.len(), 1);
            }
        }
    }

    #[test]
    fn rejects_empty_plan() {
        let plan = BearingPlan { sources: vec![] };
        let err = take_bearing(plan, "Something.".to_string()).unwrap_err();
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
        let err = take_bearing(plan, "  ".to_string()).unwrap_err();
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

        let bearing = take_bearing(plan, "Survey only.".to_string()).unwrap();
        match &bearing.moment.observations[0] {
            Observation::Files { inspections, .. } => {
                assert!(inspections.is_empty());
            }
        }
    }
}
