#![allow(dead_code)]

//! Bearing construction: the domain logic for taking a bearing.
//!
//! A bearing is built in steps: add source queries, observe the world,
//! then state a position. This module owns the state machine; the
//! presentation layer (CLI, TUI) drives it with completed values.
//!
//! Source queries are domain-specific (Files, GitHub, etc.) and are
//! constructed by the caller. The builder doesn't know what kinds
//! exist — it just collects and executes them.

use crate::model::{Bearing, BearingPlan, Moment, Observation, Position, SourceQuery};
use crate::observe;

/// The result of completing a bearing flow.
pub struct BearingResult {
    pub bearing: Bearing,
}

/// Builds a bearing step by step.
///
/// The flow is: add source queries → observe (executes all queries) →
/// set position → get bearing.
pub struct BearingBuilder {
    sources: Vec<SourceQuery>,
    moment: Option<Moment>,
    plan: Option<BearingPlan>,
    phase: Phase,
}

/// Where in the bearing construction process we are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Collecting source queries that describe what to observe.
    Planning,

    /// Observation complete, moment available for review.
    Observed,

    /// Ready for the user to state a position.
    Position,
}

impl BearingBuilder {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            moment: None,
            plan: None,
            phase: Phase::Planning,
        }
    }

    /// Current phase of the builder.
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Add a source query to the bearing plan. Only valid during Planning.
    pub fn add_source(&mut self, query: SourceQuery) {
        assert!(
            self.phase == Phase::Planning,
            "add_source called outside Planning phase"
        );
        self.sources.push(query);
    }

    /// The source queries added so far.
    pub fn sources(&self) -> &[SourceQuery] {
        &self.sources
    }

    /// Execute all source queries and move to Observed.
    /// Requires at least one source query.
    pub fn observe(&mut self) -> Result<(), &'static str> {
        assert!(
            self.phase == Phase::Planning,
            "observe called outside Planning phase"
        );
        if self.sources.is_empty() {
            return Err("at least one source query is required");
        }

        let plan = BearingPlan {
            sources: self.sources.clone(),
        };
        let observations: Vec<Observation> = plan.sources.iter().map(observe::observe).collect();

        self.moment = Some(Moment { observations });
        self.plan = Some(plan);
        self.phase = Phase::Observed;
        Ok(())
    }

    /// Access the moment after observation.
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
        assert_eq!(builder.phase(), Phase::Planning);

        builder.add_source(SourceQuery::Files {
            scope: vec![dir.path().to_path_buf()],
            focus: vec![dir.path().join("test.txt")],
        });
        builder.observe().unwrap();
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
    fn no_sources_is_rejected() {
        let mut builder = BearingBuilder::new();
        let err = builder.observe().unwrap_err();
        assert_eq!(err, "at least one source query is required");
    }

    #[test]
    fn empty_position_is_rejected() {
        let dir = TempDir::new().unwrap();
        let mut builder = BearingBuilder::new();

        builder.add_source(SourceQuery::Files {
            scope: vec![dir.path().to_path_buf()],
            focus: vec![],
        });
        builder.observe().unwrap();
        builder.acknowledge_moment();

        let err = builder.set_position("  ".to_string()).unwrap_err();
        assert_eq!(err, "position text cannot be empty");
    }

    #[test]
    fn survey_only_bearing() {
        let dir = TempDir::new().unwrap();
        let mut builder = BearingBuilder::new();

        builder.add_source(SourceQuery::Files {
            scope: vec![dir.path().to_path_buf()],
            focus: vec![],
        });
        builder.observe().unwrap();
        builder.acknowledge_moment();

        let bearing = builder.set_position("Survey only.".to_string()).unwrap();
        match &bearing.moment.observations[0] {
            Observation::Files { inspections, .. } => {
                assert!(inspections.is_empty());
            }
        }
    }
}
