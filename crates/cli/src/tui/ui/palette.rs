//! Palette list rendering (templates, captures, and macros).

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem},
};

use crate::tui::app::App;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let mut items: Vec<ListItem> = Vec::new();

    // Templates section
    if app.captures_start_index > 0 {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            " TEMPLATES",
            Style::default().fg(Color::Cyan).bold(),
        )])));

        for (i, item) in app.items.iter().enumerate().take(app.captures_start_index) {
            let style = if i == app.selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let prefix = if i == app.selected { " > " } else { "   " };
            items.push(ListItem::new(format!("{}{}", prefix, item.name())).style(style));
        }
    }

    // Captures section
    let has_captures = app.macros_start_index > app.captures_start_index;
    if has_captures {
        // Add spacing if we had templates
        if app.captures_start_index > 0 {
            items.push(ListItem::new(""));
        }

        items.push(ListItem::new(Line::from(vec![Span::styled(
            " CAPTURES",
            Style::default().fg(Color::Magenta).bold(),
        )])));

        for i in app.captures_start_index..app.macros_start_index {
            let item = &app.items[i];
            let style = if i == app.selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let prefix = if i == app.selected { " > " } else { "   " };
            items.push(ListItem::new(format!("{}{}", prefix, item.name())).style(style));
        }
    }

    // Macros section
    let has_macros = app.macros_start_index < app.items.len();
    if has_macros {
        // Add spacing if we had captures or templates
        if app.macros_start_index > 0 {
            items.push(ListItem::new(""));
        }

        items.push(ListItem::new(Line::from(vec![Span::styled(
            " MACROS",
            Style::default().fg(Color::Yellow).bold(),
        )])));

        for i in app.macros_start_index..app.items.len() {
            let item = &app.items[i];
            let style = if i == app.selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let prefix = if i == app.selected { " > " } else { "   " };
            items.push(ListItem::new(format!("{}{}", prefix, item.name())).style(style));
        }
    }

    // Empty state
    if app.items.is_empty() {
        items.push(ListItem::new(Span::styled(
            " (no items found)",
            Style::default().fg(Color::DarkGray).italic(),
        )));
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    // We don't use ListState for selection since we manually handle the styling
    frame.render_widget(list, area);
}
