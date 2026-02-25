//! Voyage screen: core loop menu.

use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Padding, Paragraph};
use ratatui::Frame;

use crate::model::Voyage;

/// The four phases of the core loop.
const MENU_ITEMS: &[&str] = &[
    "Take Bearing",
    "Correct Position",
    "Correct Course",
    "Take Action",
];

pub struct VoyageScreen {
    voyage: Voyage,
    selected: usize,
}

impl VoyageScreen {
    pub fn new(voyage: Voyage) -> Self {
        Self {
            voyage,
            selected: 0,
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < MENU_ITEMS.len() {
            self.selected += 1;
        }
    }

    #[allow(clippy::unused_self)] // Menu items don't do anything yet — navigation skeleton only.
    pub fn select(&self) {}

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::vertical([
            Constraint::Length(3), // header
            Constraint::Length(1), // separator
            Constraint::Min(0),   // menu
            Constraint::Length(1), // help
        ])
        .split(area);

        let muted = Style::default().fg(Color::DarkGray);
        let normal = Style::default().fg(Color::Gray);
        let highlight = Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);

        // Header: voyage intent or kind.
        let label = if self.voyage.intent.is_empty() {
            "Open Waters".to_string()
        } else {
            self.voyage.intent.clone()
        };
        let header = Paragraph::new(Line::from(vec![
            Span::styled(&label, highlight),
        ]))
        .block(Block::default().padding(Padding::new(2, 0, 1, 0)));
        frame.render_widget(header, chunks[0]);

        // Thin separator.
        let sep = Paragraph::new(Line::from(vec![Span::styled(
            "─".repeat(area.width.saturating_sub(4) as usize),
            muted,
        )]))
        .block(Block::default().padding(Padding::new(2, 2, 0, 0)));
        frame.render_widget(sep, chunks[1]);

        // Core loop menu.
        let items: Vec<ListItem> = MENU_ITEMS
            .iter()
            .enumerate()
            .map(|(i, &name)| {
                let style = if i == self.selected {
                    highlight
                } else {
                    normal
                };
                let pointer = if i == self.selected { "› " } else { "  " };
                ListItem::new(Line::from(vec![
                    Span::styled(pointer, style),
                    Span::styled(name, style),
                ]))
            })
            .collect();

        let menu = List::new(items)
            .block(Block::default().padding(Padding::new(2, 2, 1, 0)));
        frame.render_widget(menu, chunks[2]);

        // Help line.
        let help = Paragraph::new(Line::from(vec![Span::styled(
            " ↑↓ navigate  ⏎ select  esc back  q quit",
            muted,
        )]));
        frame.render_widget(help, chunks[3]);
    }
}
