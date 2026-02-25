//! Take Bearing screen: TUI wrapper around `BearingBuilder`.
//!
//! Handles character input, rendering, and scrolling. Feeds completed
//! values into the domain-level builder.

use std::path::PathBuf;

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph},
};

use crate::bearing::{BearingBuilder, BearingResult, Phase};
use crate::model::{DirectorySurvey, FileContent, FileInspection, Moment, Observation};

/// TUI screen for the Take Bearing flow.
///
/// Drives a `BearingBuilder` with user input collected character by character
/// (the standard pattern for text input in ratatui, which delivers raw key events).
pub struct BearingScreen {
    builder: BearingBuilder,
    input: String,
    scroll_offset: usize,
    moment_lines: Vec<String>,
}

impl BearingScreen {
    pub fn new() -> Self {
        Self {
            builder: BearingBuilder::new(),
            input: String::new(),
            scroll_offset: 0,
            moment_lines: Vec::new(),
        }
    }

    /// Handle a character being typed.
    pub fn on_char(&mut self, c: char) {
        if self.builder.phase() != Phase::Observed {
            self.input.push(c);
        }
    }

    /// Handle backspace.
    pub fn on_backspace(&mut self) {
        if self.builder.phase() != Phase::Observed {
            self.input.pop();
        }
    }

    /// Handle Enter. Returns `Some(BearingResult)` when the flow is complete.
    pub fn on_enter(&mut self) -> Option<BearingResult> {
        match self.builder.phase() {
            Phase::Scope => {
                let trimmed = self.input.trim().to_string();
                self.input.clear();
                if trimmed.is_empty() {
                    // Empty line = done adding scope.
                    let _ = self.builder.finish_scope(); // Silently stays if empty.
                } else {
                    self.builder.add_scope(PathBuf::from(trimmed));
                }
                None
            }
            Phase::Focus => {
                let trimmed = self.input.trim().to_string();
                self.input.clear();
                if trimmed.is_empty() {
                    // Empty line = done adding focus, execute observation.
                    self.builder.finish_focus();
                    if let Some(moment) = self.builder.moment() {
                        self.moment_lines = format_moment(moment);
                    }
                } else {
                    self.builder.add_focus(PathBuf::from(trimmed));
                }
                None
            }
            Phase::Observed => {
                self.builder.acknowledge_moment();
                self.input.clear();
                None
            }
            Phase::Position => {
                let text = self.input.trim().to_string();
                if text.is_empty() {
                    return None; // Reject empty position.
                }
                // Take ownership via std::mem::replace to call set_position(self).
                let builder = std::mem::replace(&mut self.builder, BearingBuilder::new());
                match builder.set_position(text) {
                    Ok(bearing) => Some(BearingResult { bearing }),
                    Err(_) => None,
                }
            }
        }
    }

    /// Handle scroll up in the moment view.
    pub fn on_scroll_up(&mut self) {
        if self.builder.phase() == Phase::Observed && self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// Handle scroll down in the moment view.
    pub fn on_scroll_down(&mut self) {
        if self.builder.phase() == Phase::Observed {
            self.scroll_offset += 1;
        }
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
        let step_label = match self.builder.phase() {
            Phase::Scope => "Take Bearing — Scope",
            Phase::Focus => "Take Bearing — Focus",
            Phase::Observed => "Take Bearing — Moment",
            Phase::Position => "Take Bearing — Position",
        };
        let header = Paragraph::new(Line::from(vec![Span::styled(step_label, highlight)]))
            .block(Block::default().padding(Padding::new(2, 0, 1, 0)));
        frame.render_widget(header, chunks[0]);

        // Content area.
        let content_area = chunks[1];
        let content_padding = Block::default().padding(Padding::new(2, 2, 0, 0));
        let inner = content_padding.inner(content_area);

        match self.builder.phase() {
            Phase::Scope => {
                let mut lines = vec![Line::from(Span::styled(
                    "Directories to survey (enter path, empty line when done):",
                    muted,
                ))];
                for p in self.builder.scope() {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", p.display()),
                        Style::default().fg(Color::Gray),
                    )));
                }
                let content = Paragraph::new(lines).block(content_padding);
                frame.render_widget(content, content_area);
            }
            Phase::Focus => {
                let mut lines = vec![Line::from(Span::styled(
                    "Files to inspect (enter path, empty line when done):",
                    muted,
                ))];
                for p in self.builder.focus() {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", p.display()),
                        Style::default().fg(Color::Gray),
                    )));
                }
                let content = Paragraph::new(lines).block(content_padding);
                frame.render_widget(content, content_area);
            }
            Phase::Observed => {
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
            Phase::Position => {
                let lines = vec![Line::from(Span::styled(
                    "Describe the state of the world (a sentence or two):",
                    muted,
                ))];
                let content = Paragraph::new(lines).block(content_padding);
                frame.render_widget(content, content_area);
            }
        }

        // Input line / help line.
        match self.builder.phase() {
            Phase::Scope | Phase::Focus | Phase::Position => {
                let prompt = Paragraph::new(Line::from(vec![
                    Span::styled(" › ", highlight),
                    Span::styled(&self.input, Style::default().fg(Color::White)),
                    Span::styled("█", Style::default().fg(Color::DarkGray)),
                ]));
                frame.render_widget(prompt, chunks[2]);
            }
            Phase::Observed => {
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
