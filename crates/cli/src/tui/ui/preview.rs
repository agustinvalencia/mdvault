//! Preview pane rendering.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::tui::app::{App, Mode, Preview};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    // In input modes, show the input form instead of preview
    if matches!(app.mode, Mode::OutputPath | Mode::Input { .. }) {
        draw_input_form(frame, area, app);
        return;
    }

    let (title, content, style) = match &app.preview {
        Preview::None => (
            "Preview".to_string(),
            String::from("Select an item to preview"),
            Style::default().fg(Color::DarkGray),
        ),
        Preview::Template { content } => {
            ("Template Preview".to_string(), content.clone(), Style::default())
        }
        Preview::Capture { content } => {
            ("Capture Preview".to_string(), content.clone(), Style::default())
        }
        Preview::Macro { content, requires_trust } => {
            let title = if *requires_trust {
                "Macro Preview [requires --trust]".to_string()
            } else {
                "Macro Preview".to_string()
            };
            (title, content.clone(), Style::default())
        }
        Preview::Error(e) => {
            ("Error".to_string(), e.clone(), Style::default().fg(Color::Red))
        }
    };

    let paragraph = Paragraph::new(content)
        .style(style)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn draw_input_form(frame: &mut Frame, area: Rect, app: &App) {
    let label = app.current_input_label().unwrap_or_else(|| "Input".to_string());

    let title = match &app.mode {
        Mode::OutputPath => "Enter Output Path".to_string(),
        Mode::Input { var_index } => {
            if app.required_var_infos.len() > 1 {
                // Show progress
                format!("Variable {} of {}", var_index + 1, app.required_var_infos.len())
            } else {
                "Enter Variable".to_string()
            }
        }
        _ => "Input".to_string(),
    };

    let mut content = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("  {}: ", label),
            Style::default().fg(Color::Cyan).bold(),
        )]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(&app.input_buffer, Style::default().fg(Color::White)),
            Span::styled("_", Style::default().fg(Color::Gray).rapid_blink()),
        ]),
    ];

    // Show description if available
    if let Some(description) = app.current_input_description() {
        content.push(Line::from(""));
        content.push(Line::from(vec![Span::styled(
            format!("  {}", description),
            Style::default().fg(Color::DarkGray).italic(),
        )]));
    }

    content.push(Line::from(""));
    content.push(Line::from(vec![Span::styled(
        "  [Enter] submit  [Esc] cancel",
        Style::default().fg(Color::DarkGray),
    )]));

    let paragraph = Paragraph::new(content).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(paragraph, area);
}
