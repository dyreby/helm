//! Take Bearing flow: define plan, observe, write position.

use std::path::PathBuf;

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph},
};

use crate::model::{
    Bearing, BearingPlan, DirectorySurvey, FileContent, FileInspection, Moment, Observation,
    Position, SourceQuery,
};
use crate::observe;

/// Where in the bearing flow we are.
enum Step {
    /// Entering scope paths (directories to survey).
    Scope,

    /// Entering focus paths (files to inspect).
    Focus,

    /// Viewing the moment (observation results).
    ViewMoment,

    /// Entering position text.
    EnterPosition,
}

/// Result of completing the bearing flow.
pub struct BearingResult {
    pub bearing: Bearing,
}

/// The Take Bearing flow, driven step by step.
pub struct BearingFlow {
    step: Step,
    scope: Vec<PathBuf>,
    focus: Vec<PathBuf>,
    input: String,
    moment: Option<Moment>,
    plan: Option<BearingPlan>,
    scroll_offset: usize,
    moment_lines: Vec<String>,
}

impl BearingFlow {
    pub fn new() -> Self {
        Self {
            step: Step::Scope,
            scope: Vec::new(),
            focus: Vec::new(),
            input: String::new(),
            moment: None,
            plan: None,
            scroll_offset: 0,
            moment_lines: Vec::new(),
        }
    }

    /// Handle a character being typed.
    pub fn on_char(&mut self, c: char) {
        match self.step {
            Step::Scope | Step::Focus | Step::EnterPosition => {
                self.input.push(c);
            }
            Step::ViewMoment => {}
        }
    }

    /// Handle backspace.
    pub fn on_backspace(&mut self) {
        match self.step {
            Step::Scope | Step::Focus | Step::EnterPosition => {
                self.input.pop();
            }
            Step::ViewMoment => {}
        }
    }

    /// Handle Enter. Returns Some(BearingResult) when the flow is complete.
    pub fn on_enter(&mut self) -> Option<BearingResult> {
        match self.step {
            Step::Scope => {
                let trimmed = self.input.trim();
                if trimmed.is_empty() {
                    if self.scope.is_empty() {
                        return None; // Need at least one scope path.
                    }
                    self.step = Step::Focus;
                } else {
                    self.scope.push(PathBuf::from(trimmed));
                }
                self.input.clear();
                None
            }
            Step::Focus => {
                let trimmed = self.input.trim().to_string();
                if !trimmed.is_empty() {
                    self.focus.push(PathBuf::from(&trimmed));
                }
                self.input.clear();

                // Empty input means done adding focus — execute the plan.
                if trimmed.is_empty() {
                    self.execute();
                    self.step = Step::ViewMoment;
                }
                None
            }
            Step::ViewMoment => {
                self.step = Step::EnterPosition;
                self.input.clear();
                None
            }
            Step::EnterPosition => {
                let text = self.input.trim().to_string();
                if text.is_empty() {
                    return None; // Need some position text.
                }

                let bearing = Bearing {
                    plan: self.plan.take().expect("plan should exist"),
                    moment: self.moment.take().expect("moment should exist"),
                    position: Position {
                        text,
                        history: Vec::new(),
                    },
                    taken_at: jiff::Timestamp::now(),
                };

                Some(BearingResult { bearing })
            }
        }
    }

    /// Handle scroll up in the moment view.
    pub fn on_scroll_up(&mut self) {
        if matches!(self.step, Step::ViewMoment) && self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// Handle scroll down in the moment view.
    pub fn on_scroll_down(&mut self) {
        if matches!(self.step, Step::ViewMoment) {
            self.scroll_offset += 1;
        }
    }

    fn execute(&mut self) {
        let plan = BearingPlan {
            sources: vec![SourceQuery::Files {
                scope: self.scope.clone(),
                focus: self.focus.clone(),
            }],
        };

        let observations: Vec<Observation> = plan.sources.iter().map(observe::observe).collect();

        let moment = Moment { observations };
        self.moment_lines = format_moment(&moment);
        self.moment = Some(moment);
        self.plan = Some(plan);
        self.scroll_offset = 0;
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::vertical([
            Constraint::Length(3), // header
            Constraint::Min(0),    // content
            Constraint::Length(1), // input or help
        ])
        .split(area);

        let muted = Style::default().fg(Color::DarkGray);
        let highlight = Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);

        // Header.
        let step_label = match self.step {
            Step::Scope => "Take Bearing — Scope",
            Step::Focus => "Take Bearing — Focus",
            Step::ViewMoment => "Take Bearing — Moment",
            Step::EnterPosition => "Take Bearing — Position",
        };
        let header = Paragraph::new(Line::from(vec![Span::styled(step_label, highlight)]))
            .block(Block::default().padding(Padding::new(2, 0, 1, 0)));
        frame.render_widget(header, chunks[0]);

        // Content area.
        let content_area = chunks[1];
        let content_padding = Block::default().padding(Padding::new(2, 2, 0, 0));
        let inner = content_padding.inner(content_area);

        match self.step {
            Step::Scope => {
                let mut lines = vec![Line::from(Span::styled(
                    "Directories to survey (enter path, empty line when done):",
                    muted,
                ))];
                for p in &self.scope {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", p.display()),
                        Style::default().fg(Color::Gray),
                    )));
                }
                let content = Paragraph::new(lines).block(content_padding);
                frame.render_widget(content, content_area);
            }
            Step::Focus => {
                let mut lines = vec![Line::from(Span::styled(
                    "Files to inspect (enter path, empty line when done):",
                    muted,
                ))];
                for p in &self.focus {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", p.display()),
                        Style::default().fg(Color::Gray),
                    )));
                }
                let content = Paragraph::new(lines).block(content_padding);
                frame.render_widget(content, content_area);
            }
            Step::ViewMoment => {
                let visible_height = inner.height as usize;
                let total = self.moment_lines.len();
                let max_offset = total.saturating_sub(visible_height);
                let offset = self.scroll_offset.min(max_offset);

                let lines: Vec<Line> = self.moment_lines[offset..]
                    .iter()
                    .take(visible_height)
                    .map(|s| Line::from(s.as_str()))
                    .collect();

                let content = Paragraph::new(lines).block(content_padding);
                frame.render_widget(content, content_area);
            }
            Step::EnterPosition => {
                let lines = vec![Line::from(Span::styled(
                    "Describe the state of the world (a sentence or two):",
                    muted,
                ))];
                let content = Paragraph::new(lines).block(content_padding);
                frame.render_widget(content, content_area);
            }
        }

        // Input line / help line.
        match self.step {
            Step::Scope | Step::Focus | Step::EnterPosition => {
                let prompt = Paragraph::new(Line::from(vec![
                    Span::styled(" › ", highlight),
                    Span::styled(&self.input, Style::default().fg(Color::White)),
                    Span::styled("█", Style::default().fg(Color::DarkGray)),
                ]));
                frame.render_widget(prompt, chunks[2]);
            }
            Step::ViewMoment => {
                let help = Paragraph::new(Line::from(vec![Span::styled(
                    " ↑↓ scroll  ⏎ continue  esc cancel",
                    muted,
                )]));
                frame.render_widget(help, chunks[2]);
            }
        }
    }
}

/// Format a moment's observations into displayable lines.
fn format_moment(moment: &Moment) -> Vec<String> {
    let mut lines = Vec::new();

    for obs in &moment.observations {
        match obs {
            Observation::Files {
                survey,
                inspections,
            } => {
                format_surveys(&mut lines, survey);
                if !survey.is_empty() && !inspections.is_empty() {
                    lines.push(String::new());
                }
                format_inspections(&mut lines, inspections);
            }
        }
    }

    lines
}

fn format_surveys(lines: &mut Vec<String>, surveys: &[DirectorySurvey]) {
    for (i, survey) in surveys.iter().enumerate() {
        if i > 0 {
            lines.push(String::new());
        }
        lines.push(format!("{}:", survey.path.display()));
        if survey.entries.is_empty() {
            lines.push("  (empty)".to_string());
        }
        for entry in &survey.entries {
            let suffix = if entry.is_dir { "/" } else { "" };
            let size = entry
                .size_bytes
                .map(|s| format!("  ({s} bytes)"))
                .unwrap_or_default();
            lines.push(format!("  {}{suffix}{size}", entry.name));
        }
    }
}

fn format_inspections(lines: &mut Vec<String>, inspections: &[FileInspection]) {
    for (i, inspection) in inspections.iter().enumerate() {
        if i > 0 {
            lines.push(String::new());
        }
        lines.push(format!("── {} ──", inspection.path.display()));
        match &inspection.content {
            FileContent::Text(text) => {
                for line in text.lines() {
                    lines.push(format!("  {line}"));
                }
                if text.is_empty() {
                    lines.push("  (empty file)".to_string());
                }
            }
            FileContent::Binary { size_bytes } => {
                lines.push(format!("  (binary, {size_bytes} bytes)"));
            }
            FileContent::Error(e) => {
                lines.push(format!("  (error: {e})"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use crate::model::Observation;

    fn type_str(flow: &mut BearingFlow, s: &str) {
        for c in s.chars() {
            flow.on_char(c);
        }
    }

    #[test]
    fn full_flow_produces_bearing() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.txt"), "hello").unwrap();

        let mut flow = BearingFlow::new();

        // Scope step: add directory, then empty line to continue.
        type_str(&mut flow, &dir.path().display().to_string());
        assert!(flow.on_enter().is_none());
        // Empty line to move to focus.
        assert!(flow.on_enter().is_none());

        // Focus step: add file, then empty line to execute.
        type_str(
            &mut flow,
            &dir.path().join("test.txt").display().to_string(),
        );
        assert!(flow.on_enter().is_none());
        // Empty line to execute observation.
        assert!(flow.on_enter().is_none());

        // ViewMoment step: press enter to continue.
        assert!(flow.on_enter().is_none());

        // EnterPosition step: type position.
        type_str(&mut flow, "Directory has one test file.");
        let result = flow.on_enter();

        assert!(result.is_some());
        let bearing = result.unwrap().bearing;
        assert_eq!(bearing.position.text, "Directory has one test file.");
        assert_eq!(bearing.plan.sources.len(), 1);

        // Verify the observation contains both survey and inspection.
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
    fn empty_scope_is_rejected() {
        let mut flow = BearingFlow::new();
        // Empty enter on scope step with no paths added.
        assert!(flow.on_enter().is_none());
        // Still on scope step — verify by adding a path (should work).
        type_str(&mut flow, "/tmp");
        assert!(flow.on_enter().is_none()); // Adds path, stays on scope.
    }

    #[test]
    fn empty_position_is_rejected() {
        let dir = TempDir::new().unwrap();
        let mut flow = BearingFlow::new();

        // Get through to position step.
        type_str(&mut flow, &dir.path().display().to_string());
        flow.on_enter(); // add scope
        flow.on_enter(); // empty → move to focus
        flow.on_enter(); // empty → execute, move to moment
        flow.on_enter(); // move to position

        // Empty position rejected.
        assert!(flow.on_enter().is_none());
    }

    #[test]
    fn focus_is_optional() {
        let dir = TempDir::new().unwrap();
        let mut flow = BearingFlow::new();

        type_str(&mut flow, &dir.path().display().to_string());
        flow.on_enter(); // add scope
        flow.on_enter(); // empty → move to focus
        flow.on_enter(); // empty → execute (no focus), move to moment
        flow.on_enter(); // move to position

        type_str(&mut flow, "Empty survey.");
        let result = flow.on_enter();
        assert!(result.is_some());

        match &result.unwrap().bearing.moment.observations[0] {
            Observation::Files { inspections, .. } => {
                assert!(inspections.is_empty());
            }
        }
    }

    #[test]
    fn format_moment_survey_and_inspection() {
        let moment = Moment {
            observations: vec![Observation::Files {
                survey: vec![DirectorySurvey {
                    path: PathBuf::from("src/"),
                    entries: vec![
                        crate::model::DirectoryEntry {
                            name: "main.rs".to_string(),
                            is_dir: false,
                            size_bytes: Some(100),
                        },
                        crate::model::DirectoryEntry {
                            name: "lib".to_string(),
                            is_dir: true,
                            size_bytes: None,
                        },
                    ],
                }],
                inspections: vec![FileInspection {
                    path: PathBuf::from("src/main.rs"),
                    content: FileContent::Text("fn main() {}".to_string()),
                }],
            }],
        };

        let lines = format_moment(&moment);
        assert!(lines.iter().any(|l| l.contains("src/:")));
        assert!(lines.iter().any(|l| l.contains("main.rs")));
        assert!(lines.iter().any(|l| l.contains("fn main()")));
    }
}
