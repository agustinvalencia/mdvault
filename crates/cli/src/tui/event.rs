//! Event handling: maps keyboard events to application messages.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{App, Message, Mode};

/// Map a key event to an optional message based on current app mode.
pub fn map_key_event(app: &App, key: KeyEvent) -> Option<Message> {
    // Global bindings (work in any mode)
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Message::Quit);
    }

    // Mode-specific bindings
    match &app.mode {
        Mode::Browse => map_browse_keys(key),
        Mode::OutputPath | Mode::Input { .. } => map_input_keys(key),
        Mode::Result => map_result_keys(key),
    }
}

fn map_browse_keys(key: KeyEvent) -> Option<Message> {
    match key.code {
        // Vim-style navigation
        KeyCode::Char('j') | KeyCode::Down => Some(Message::SelectNext),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::SelectPrev),

        // Actions
        KeyCode::Enter => Some(Message::Execute),
        KeyCode::Char('q') | KeyCode::Esc => Some(Message::Quit),

        _ => None,
    }
}

fn map_input_keys(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char(c) => Some(Message::InputChar(c)),
        KeyCode::Backspace => Some(Message::InputBackspace),
        KeyCode::Enter => Some(Message::InputSubmit),
        KeyCode::Esc => Some(Message::Cancel),
        _ => None,
    }
}

fn map_result_keys(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Char('q') => {
            Some(Message::ClearStatus)
        }
        _ => None,
    }
}
