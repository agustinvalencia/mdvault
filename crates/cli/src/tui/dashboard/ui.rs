//! Dashboard UI rendering.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline},
};

use super::app::{DashboardApp, Panel};

/// Draw the entire dashboard.
pub fn draw(frame: &mut Frame, app: &DashboardApp) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Length(5), // Summary bar
            Constraint::Min(10),   // Body
            Constraint::Length(2), // Status / keybindings
        ])
        .split(frame.area());

    draw_header(frame, main_chunks[0], app);
    draw_summary(frame, main_chunks[1], app);
    draw_body(frame, main_chunks[2], app);
    draw_status(frame, main_chunks[3], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &DashboardApp) {
    let scope_text = match &app.report.scope {
        mdvault_core::report::ReportScope::Vault => "Vault Dashboard".to_string(),
        mdvault_core::report::ReportScope::Project { id, title } => {
            format!("{} [{}]", title, id)
        }
    };

    let line = Line::from(vec![
        Span::styled(" mdv dashboard ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("| ", Style::default().fg(Color::DarkGray)),
        Span::styled(scope_text, Style::default().fg(Color::White)),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}

fn draw_summary(frame: &mut Frame, area: Rect, app: &DashboardApp) {
    let s = &app.report.summary;

    let tasks_done = s.tasks_by_status.get("done").copied().unwrap_or(0);
    let tasks_todo = s.tasks_by_status.get("todo").copied().unwrap_or(0);
    let tasks_in_progress = s.tasks_by_status.get("in_progress").copied().unwrap_or(0);
    let tasks_blocked = s.tasks_by_status.get("blocked").copied().unwrap_or(0);

    let lines = vec![
        Line::from(vec![
            Span::styled("  Notes: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                s.total_notes.to_string(),
                Style::default().fg(Color::White).bold(),
            ),
            Span::styled("   Tasks: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                s.total_tasks.to_string(),
                Style::default().fg(Color::White).bold(),
            ),
            Span::styled("   Projects: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} active", s.active_projects),
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Done: ", Style::default().fg(Color::DarkGray)),
            Span::styled(tasks_done.to_string(), Style::default().fg(Color::Green)),
            Span::styled("  Todo: ", Style::default().fg(Color::DarkGray)),
            Span::styled(tasks_todo.to_string(), Style::default().fg(Color::Blue)),
            Span::styled("  In Progress: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                tasks_in_progress.to_string(),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled("  Blocked: ", Style::default().fg(Color::DarkGray)),
            Span::styled(tasks_blocked.to_string(), Style::default().fg(Color::Red)),
        ]),
    ];

    // Activity sparkline
    let activity_data: Vec<u64> = app
        .report
        .activity
        .daily_activity
        .iter()
        .map(|d| d.tasks_completed as u64)
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let summary_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Summary ");

    let paragraph = Paragraph::new(lines).block(summary_block);
    frame.render_widget(paragraph, chunks[0]);

    let sparkline_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(format!(" Activity ({}d) ", app.report.activity.period_days));

    let sparkline = Sparkline::default()
        .block(sparkline_block)
        .data(&activity_data)
        .style(Style::default().fg(Color::Green));
    frame.render_widget(sparkline, chunks[1]);
}

fn draw_body(frame: &mut Frame, area: Rect, app: &DashboardApp) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    draw_projects_panel(frame, chunks[0], app);
    draw_detail_panel(frame, chunks[1], app);
}

fn draw_projects_panel(frame: &mut Frame, area: Rect, app: &DashboardApp) {
    let border_color =
        if app.panel == Panel::Projects { Color::Cyan } else { Color::DarkGray };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(" Projects ");

    if app.report.projects.is_empty() {
        let p = Paragraph::new("  No projects found")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(p, area);
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Project list with inline progress gauges
    let items: Vec<ListItem> = app
        .report
        .projects
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let selected = i == app.project_index;
            let style = if selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let prefix = if selected { " > " } else { "   " };
            let bar_width: usize = 12;
            let filled =
                ((p.progress_percent / 100.0) * bar_width as f64).round() as usize;
            let empty = bar_width.saturating_sub(filled);
            let bar = format!("{}{}", "#".repeat(filled), ".".repeat(empty));

            let line = Line::from(vec![
                Span::raw(prefix),
                Span::styled(&p.id, Style::default().fg(Color::Cyan).bold()),
                Span::raw(" "),
                Span::styled(format!("[{}]", bar), Style::default().fg(Color::Green)),
                Span::styled(
                    format!(" {:.0}%", p.progress_percent),
                    Style::default().fg(Color::White),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{}/{}", p.tasks.done, p.tasks.total),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);
}

fn draw_detail_panel(frame: &mut Frame, area: Rect, app: &DashboardApp) {
    let border_color =
        if app.panel == Panel::Tasks { Color::Cyan } else { Color::DarkGray };

    let Some(project) = app.selected_project() else {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" Details ");
        let p = Paragraph::new("  Select a project")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(p, area);
        return;
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(format!(" {} [{}] ", project.title, project.id));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split detail area: progress gauge + task breakdown + recent completions
    let detail_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Progress gauge
            Constraint::Length(6), // Task breakdown
            Constraint::Min(3),    // Recent completions
        ])
        .split(inner);

    // Progress gauge
    let progress = (project.progress_percent / 100.0).clamp(0.0, 1.0);
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
        .ratio(progress)
        .label(format!(
            "{:.0}% ({}/{} done)",
            project.progress_percent, project.tasks.done, project.tasks.total
        ));
    let gauge_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Progress ");
    frame.render_widget(gauge.block(gauge_block), detail_chunks[0]);

    // Task breakdown
    let task_lines = vec![
        Line::from(vec![
            Span::styled("  Todo:        ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                project.tasks.todo.to_string(),
                Style::default().fg(Color::Blue),
            ),
        ]),
        Line::from(vec![
            Span::styled("  In Progress: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                project.tasks.in_progress.to_string(),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Blocked:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                project.tasks.blocked.to_string(),
                Style::default().fg(Color::Red),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Velocity:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(
                    "{:.1}/wk (4w)  {:.1}/wk (2w)",
                    project.velocity.tasks_per_week_4w,
                    project.velocity.tasks_per_week_2w
                ),
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    let tasks_paragraph = Paragraph::new(task_lines).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Tasks "),
    );
    frame.render_widget(tasks_paragraph, detail_chunks[1]);

    // Recent completions
    let completions_block = Block::default().title(" Recent Completions ");

    if project.recent_completions.is_empty() {
        let p = Paragraph::new("  (none in last 7 days)")
            .style(Style::default().fg(Color::DarkGray))
            .block(completions_block);
        frame.render_widget(p, detail_chunks[2]);
    } else {
        let items: Vec<ListItem> = project
            .recent_completions
            .iter()
            .skip(app.task_scroll)
            .map(|c| {
                let line = Line::from(vec![
                    Span::styled(
                        format!("  {} ", c.completed_at),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(&c.id, Style::default().fg(Color::Cyan)),
                    Span::raw(" "),
                    Span::styled(&c.title, Style::default().fg(Color::White)),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items).block(completions_block);
        frame.render_widget(list, detail_chunks[2]);
    }
}

fn draw_status(frame: &mut Frame, area: Rect, app: &DashboardApp) {
    let (left_text, right_content) = if let Some(status) = &app.status {
        let style = if status.is_error {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };
        (" [Enter] dismiss", Span::styled(&status.text, style))
    } else {
        (
            " [j/k] navigate  [Tab] panel  [v] png  [s] export  [r] refresh  [q] quit",
            Span::styled("Ready", Style::default().fg(Color::DarkGray)),
        )
    };

    let left = Span::styled(left_text, Style::default().fg(Color::DarkGray));
    let right_len = match &app.status {
        Some(s) => s.text.len(),
        None => 5, // "Ready"
    };
    let padding =
        area.width.saturating_sub(left_text.len() as u16 + right_len as u16 + 2) as usize;

    let line = Line::from(vec![left, Span::raw(" ".repeat(padding)), right_content]);

    let paragraph = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(paragraph, area);
}
