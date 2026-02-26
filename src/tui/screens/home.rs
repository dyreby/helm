//! Home screen: active voyages and new voyage option.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Padding, Paragraph};

use crate::model::{Voyage, VoyageStatus};
use crate::tui::app::HomeAction;

/// An item in the home screen list — either an existing voyage or the "new" option.
enum HomeItem {
    Voyage(Voyage),
    NewOpenWaters,
}

pub struct HomeScreen {
    items: Vec<HomeItem>,
    selected: usize,
}

impl HomeScreen {
    pub fn new(active_voyages: Vec<Voyage>) -> Self {
        let mut items: Vec<HomeItem> = active_voyages.into_iter().map(HomeItem::Voyage).collect();
        items.push(HomeItem::NewOpenWaters);
        Self { items, selected: 0 }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }

    pub fn select(&self) -> Option<HomeAction> {
        self.items.get(self.selected).map(|item| match item {
            HomeItem::Voyage(v) => HomeAction::OpenVoyage(v.id),
            HomeItem::NewOpenWaters => HomeAction::NewOpenWaters,
        })
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::vertical([
            Constraint::Length(3), // title
            Constraint::Min(0),    // list
            Constraint::Length(1), // help
        ])
        .split(area);

        // Title.
        let title = Paragraph::new(Line::from(vec![Span::styled(
            "Helm",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )]))
        .block(Block::default().padding(Padding::new(2, 0, 1, 0)));
        frame.render_widget(title, chunks[0]);

        // Build list items.
        let muted = Style::default().fg(Color::DarkGray);
        let normal = Style::default().fg(Color::Gray);
        let highlight = Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);

        let list_items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if i == self.selected {
                    highlight
                } else {
                    normal
                };
                let pointer = if i == self.selected { "› " } else { "  " };

                match item {
                    HomeItem::Voyage(v) => {
                        let status = match v.status {
                            VoyageStatus::Active => "active",
                            VoyageStatus::Completed { .. } => "done",
                        };
                        let label = if v.intent.is_empty() {
                            "Open Waters".to_string()
                        } else {
                            v.intent.clone()
                        };
                        ListItem::new(Line::from(vec![
                            Span::styled(pointer, style),
                            Span::styled(label, style),
                            Span::styled(format!("  [{status}]"), muted),
                        ]))
                    }
                    HomeItem::NewOpenWaters => ListItem::new(Line::from(vec![
                        Span::styled(pointer, style),
                        Span::styled("New Voyage — Open Waters", style),
                    ])),
                }
            })
            .collect();

        let list = List::new(list_items).block(Block::default().padding(Padding::new(2, 2, 0, 0)));
        frame.render_widget(list, chunks[1]);

        // Help line.
        let help = Paragraph::new(Line::from(vec![Span::styled(
            " ↑↓ navigate  ⏎ select  q quit",
            muted,
        )]));
        frame.render_widget(help, chunks[2]);
    }
}
