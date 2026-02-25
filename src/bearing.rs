#![allow(dead_code)]

//! Bearing construction: the domain logic for taking a bearing.
//!
//! A bearing is built in steps: define scope, define focus, observe,
//! then state a position. This module owns the state machine; the
//! presentation layer (CLI, TUI) drives it with completed values.

use std::path::PathBuf;

use crate::model::{Bearing, BearingPlan, Moment, Observation, Position, SourceQuery};
use crate::observe;

/// The result of completing a bearing flow.
pub struct BearingResult {
    pub bearing: Bearing,
}

/// Builds a bearing step by step.
///
/// The flow is: add scope paths → finish scope → add focus paths →
/// finish focus (executes observation) → set position → get bearing.
pub struct BearingBuilder {
    scope: Vec<PathBuf>,
    focus: Vec<PathBuf>,
    moment: Option<Moment>,
    plan: Option<BearingPlan>,
    phase: Phase,
}

/// Where in the bearing construction process we are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Collecting directory paths to survey.
    Scope,

    /// Collecting file paths to inspect.
    Focus,

    /// Observation complete, ready to view the moment.
    Observed,

    /// Position needed.
    Position,
}

impl BearingBuilder {
    pub fn new() -> Self {
        Self {
            scope: Vec::new(),
            focus: Vec::new(),
            moment: None,
            plan: None,
            phase: Phase::Scope,
        }
    }

    /// Current phase of the builder.
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Add a directory path to survey. Only valid during the Scope phase.
    pub fn add_scope(&mut self, path: PathBuf) {
        assert!(
            self.phase == Phase::Scope,
            "add_scope called outside Scope phase"
        );
        self.scope.push(path);
    }

    /// The scope paths added so far.
    pub fn scope(&self) -> &[PathBuf] {
        &self.scope
    }

    /// Finish adding scope paths and move to the Focus phase.
    /// Requires at least one scope path.
    pub fn finish_scope(&mut self) -> Result<(), &'static str> {
        if self.scope.is_empty() {
            return Err("at least one scope path is required");
        }
        self.phase = Phase::Focus;
        Ok(())
    }

    /// Add a file path to inspect. Only valid during the Focus phase.
    pub fn add_focus(&mut self, path: PathBuf) {
        assert!(
            self.phase == Phase::Focus,
            "add_focus called outside Focus phase"
        );
        self.focus.push(path);
    }

    /// The focus paths added so far.
    pub fn focus(&self) -> &[PathBuf] {
        &self.focus
    }

    /// Finish adding focus paths, execute observation, and move to Observed.
    /// Focus can be empty (survey-only bearing).
    pub fn finish_focus(&mut self) {
        assert!(
            self.phase == Phase::Focus,
            "finish_focus called outside Focus phase"
        );

        let plan = BearingPlan {
            sources: vec![SourceQuery::Files {
                scope: self.scope.clone(),
                focus: self.focus.clone(),
            }],
        };

        let observations: Vec<Observation> = plan.sources.iter().map(observe::observe).collect();
        self.moment = Some(Moment { observations });
        self.plan = Some(plan);
        self.phase = Phase::Observed;
    }

    /// Access the moment after observation. Only valid in Observed or Position phase.
    pub fn moment(&self) -> Option<&Moment> {
        self.moment.as_ref()
    }

    /// Acknowledge the moment and move to the Position phase.
    pub fn acknowledge_moment(&mut self) {
        assert!(
            self.phase == Phase::Observed,
            "acknowledge_moment called outside Observed phase"
        );
        self.phase = Phase::Position;
    }

    /// Set the position text and produce the completed bearing.
    pub fn set_position(mut self, text: String) -> Result<Bearing, &'static str> {
        assert!(
            self.phase == Phase::Position,
            "set_position called outside Position phase"
        );

        if text.trim().is_empty() {
            return Err("position text cannot be empty");
        }

        Ok(Bearing {
            plan: self
                .plan
                .take()
                .expect("plan should exist after observation"),
            moment: self
                .moment
                .take()
                .expect("moment should exist after observation"),
            position: Position {
                text,
                history: Vec::new(),
            },
            taken_at: jiff::Timestamp::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use tempfile::TempDir;

    use crate::model::Observation;

    #[test]
    fn full_flow_produces_bearing() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.txt"), "hello").unwrap();

        let mut builder = BearingBuilder::new();
        assert_eq!(builder.phase(), Phase::Scope);

        builder.add_scope(dir.path().to_path_buf());
        builder.finish_scope().unwrap();
        assert_eq!(builder.phase(), Phase::Focus);

        builder.add_focus(dir.path().join("test.txt"));
        builder.finish_focus();
        assert_eq!(builder.phase(), Phase::Observed);

        // Verify moment has data.
        let moment = builder.moment().unwrap();
        match &moment.observations[0] {
            Observation::Files {
                survey,
                inspections,
            } => {
                assert_eq!(survey.len(), 1);
                assert_eq!(inspections.len(), 1);
            }
        }

        builder.acknowledge_moment();
        assert_eq!(builder.phase(), Phase::Position);

        let bearing = builder.set_position("One test file.".to_string()).unwrap();
        assert_eq!(bearing.position.text, "One test file.");
        assert_eq!(bearing.plan.sources.len(), 1);
    }

    #[test]
    fn empty_scope_is_rejected() {
        let mut builder = BearingBuilder::new();
        let err = builder.finish_scope().unwrap_err();
        assert_eq!(err, "at least one scope path is required");
    }

    #[test]
    fn empty_position_is_rejected() {
        let dir = TempDir::new().unwrap();
        let mut builder = BearingBuilder::new();

        builder.add_scope(dir.path().to_path_buf());
        builder.finish_scope().unwrap();
        builder.finish_focus();
        builder.acknowledge_moment();

        let err = builder.set_position("  ".to_string()).unwrap_err();
        assert_eq!(err, "position text cannot be empty");
    }

    #[test]
    fn focus_is_optional() {
        let dir = TempDir::new().unwrap();
        let mut builder = BearingBuilder::new();

        builder.add_scope(dir.path().to_path_buf());
        builder.finish_scope().unwrap();
        builder.finish_focus(); // No focus paths added.
        builder.acknowledge_moment();

        let bearing = builder.set_position("Survey only.".to_string()).unwrap();
        match &bearing.moment.observations[0] {
            Observation::Files { inspections, .. } => {
                assert!(inspections.is_empty());
            }
        }
    }
}
