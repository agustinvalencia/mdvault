//! Key event mapping for the dashboard TUI.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{DashboardApp, Message, Mode};

/// Map a key event to an optional message based on current mode.
pub fn map_key_event(app: &DashboardApp, key: KeyEvent) -> Option<Message> {
    // Global: Ctrl+C always quits
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Message::Quit);
    }

    match &app.mode {
        Mode::Browse => map_browse_keys(key),
        Mode::Status => map_status_keys(key),
    }
}

fn map_browse_keys(key: KeyEvent) -> Option<Message> {
    match key.code {
        // Navigation
        KeyCode::Char('j') | KeyCode::Down => Some(Message::SelectNext),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::SelectPrev),
        KeyCode::Tab => Some(Message::SwitchPanel),

        // Actions
        KeyCode::Char('v') => Some(Message::GeneratePng),
        KeyCode::Char('s') => Some(Message::ExportToNote),
        KeyCode::Char('r') => Some(Message::Refresh),

        // Quit
        KeyCode::Char('q') | KeyCode::Esc => Some(Message::Quit),

        _ => None,
    }
}

fn map_status_keys(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Char('q') => {
            Some(Message::DismissStatus)
        }
        _ => None,
    }
}
