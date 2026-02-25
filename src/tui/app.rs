//! Application loop and screen routing.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use uuid::Uuid;

use crate::model::{Voyage, VoyageKind, VoyageStatus};
use crate::storage::Storage;

use super::screens::{HomeScreen, VoyageScreen};

/// Which screen is currently displayed.
enum Screen {
    Home(HomeScreen),
    Voyage(VoyageScreen),
}

/// Runs the TUI event loop until the user quits.
pub fn run(storage: &Storage) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, storage);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut DefaultTerminal, storage: &Storage) -> io::Result<()> {
    let mut screen = Screen::Home(load_home_screen(storage)?);

    loop {
        terminal.draw(|frame| match &screen {
            Screen::Home(s) => s.render(frame),
            Screen::Voyage(s) => s.render(frame),
        })?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match &mut screen {
                Screen::Home(home) => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Up | KeyCode::Char('k') => home.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => home.move_down(),
                    KeyCode::Enter => {
                        if let Some(action) = home.select() {
                            match action {
                                HomeAction::OpenVoyage(id) => {
                                    let voyage =
                                        storage.load_voyage(id).map_err(io::Error::other)?;
                                    screen = Screen::Voyage(VoyageScreen::new(voyage));
                                }
                                HomeAction::NewOpenWaters => {
                                    let voyage = Voyage {
                                        id: Uuid::new_v4(),
                                        kind: VoyageKind::OpenWaters,
                                        intent: String::new(),
                                        created_at: jiff::Timestamp::now(),
                                        status: VoyageStatus::Active,
                                    };
                                    storage.create_voyage(&voyage).map_err(io::Error::other)?;
                                    screen = Screen::Voyage(VoyageScreen::new(voyage));
                                }
                            }
                        }
                    }
                    _ => {}
                },
                Screen::Voyage(v) => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Esc => {
                        screen = Screen::Home(load_home_screen(storage)?);
                    }
                    KeyCode::Up | KeyCode::Char('k') => v.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => v.move_down(),
                    KeyCode::Enter => v.select(),
                    _ => {}
                },
            }
        }
    }
}

fn load_home_screen(storage: &Storage) -> io::Result<HomeScreen> {
    let voyages = storage.list_voyages().map_err(io::Error::other)?;
    let active: Vec<Voyage> = voyages
        .into_iter()
        .filter(|v| matches!(v.status, VoyageStatus::Active))
        .collect();
    Ok(HomeScreen::new(active))
}

/// What the home screen wants to happen when the user presses Enter.
pub enum HomeAction {
    OpenVoyage(Uuid),
    NewOpenWaters,
}
