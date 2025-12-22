//! Main layout and frame composition.

use ratatui::{prelude::*, widgets::Paragraph};

use super::{palette, preview, status};
use crate::tui::app::App;

/// Draw the entire application UI.
pub fn draw(frame: &mut Frame, app: &App) {
    // Main layout: header, body, footer
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(5),    // Body
            Constraint::Length(2), // Status bar
        ])
        .split(frame.area());

    // Header
    draw_header(frame, main_chunks[0], app);

    // Body: palette | preview
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Palette
            Constraint::Percentage(70), // Preview
        ])
        .split(main_chunks[1]);

    palette::draw(frame, body_chunks[0], app);
    preview::draw(frame, body_chunks[1], app);

    // Status bar
    status::draw(frame, main_chunks[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let profile_text = format!("[{}]", app.config.active_profile);
    let title = "mdvault";

    // Calculate padding for right-alignment
    let padding =
        area.width.saturating_sub(title.len() as u16 + profile_text.len() as u16 + 2)
            as usize;

    let line = Line::from(vec![
        Span::styled(format!(" {}", title), Style::default().fg(Color::Cyan).bold()),
        Span::raw(" ".repeat(padding)),
        Span::styled(profile_text, Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
    ]);

    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
