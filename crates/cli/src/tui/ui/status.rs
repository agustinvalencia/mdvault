//! Status bar rendering.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use crate::tui::app::{App, Mode};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let (left_text, right_text) = match &app.mode {
        Mode::Browse => (" [j/k] navigate  [Enter] execute  [q] quit", "Ready"),
        Mode::OutputPath | Mode::Input { .. } => {
            (" [Enter] submit  [Esc] cancel", "Input Mode")
        }
        Mode::Result => (" [Enter] continue", "Done"),
    };

    // If there's a status message, show it on the right
    let right_content = if let Some(status) = &app.status {
        let style = if status.is_error {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };
        Span::styled(&status.text, style)
    } else {
        Span::styled(right_text, Style::default().fg(Color::DarkGray))
    };

    let left = Span::styled(left_text, Style::default().fg(Color::DarkGray));

    // Calculate padding for right-alignment
    let left_len = left_text.len();
    let right_len = if app.status.is_some() {
        app.status.as_ref().unwrap().text.len()
    } else {
        right_text.len()
    };
    let padding =
        area.width.saturating_sub(left_len as u16 + right_len as u16 + 2) as usize;

    let line = Line::from(vec![left, Span::raw(" ".repeat(padding)), right_content]);

    let paragraph = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(paragraph, area);
}
