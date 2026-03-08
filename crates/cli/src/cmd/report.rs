//! Activity report generation commands.

use chrono::{Datelike, Duration, Local, NaiveDate, Utc};
use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::{IndexDb, IndexedNote, NoteQuery};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tabled::{settings::Style, Table, Tabled};

/// Report data for JSON output.
#[derive(Serialize)]
struct ReportData {
    period: String,
    period_type: String, // "month" or "week"
    start_date: String,
    end_date: String,
    generated_at: String,
    summary: ReportSummary,
    tasks_by_project: Vec<ProjectTaskSummary>,
    activity_heatmap: Vec<DayActivity>,
    top_completed: Vec<CompletedTask>,
}

#[derive(Serialize)]
struct ReportSummary {
    tasks_completed: usize,
    tasks_created: usize,
    projects_active: usize,
    daily_notes: usize,
    daily_notes_possible: usize,
}

#[derive(Serialize)]
struct ProjectTaskSummary {
    id: String,
    title: String,
    created: usize,
    completed: usize,
}

#[derive(Serialize)]
struct DayActivity {
    date: String,
    weekday: String,
    completed: usize,
}

#[derive(Serialize)]
struct CompletedTask {
    id: String,
    title: String,
    completed_at: String,
    project: String,
}

/// Row for project tasks table.
#[derive(Tabled)]
struct ProjectTaskRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Project")]
    title: String,
    #[tabled(rename = "Created")]
    created: usize,
    #[tabled(rename = "Done")]
    completed: usize,
}

/// Run the report command.
pub fn run(
    config: Option<&Path>,
    profile: Option<&str>,
    month: Option<&str>,
    week: Option<&str>,
    output: Option<&Path>,
    json_output: bool,
) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            std::process::exit(1);
        }
    };

    let index_path = cfg.vault_root.join(".mdvault/index.db");
    let db = match IndexDb::open(&index_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open index: {e}");
            eprintln!("Run 'mdv reindex' first.");
            std::process::exit(1);
        }
    };

    // Determine the time period
    let (start_date, end_date, period_str, period_type) = if let Some(m) = month {
        parse_month(m)
    } else if let Some(w) = week {
        parse_week(w)
    } else {
        // Default to current month
        let now = Local::now().date_naive();
        let start = NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap();
        let end = if now.month() == 12 {
            NaiveDate::from_ymd_opt(now.year() + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1).unwrap()
        } - Duration::days(1);
        let period = format!("{}", now.format("%Y-%m"));
        (start, end, period, "month".to_string())
    };

    // Generate report data
    let report = generate_report(&db, start_date, end_date, &period_str, &period_type);

    // Output the report
    if let Some(path) = output {
        let markdown = format_markdown_report(&report);
        if let Err(e) = fs::write(path, &markdown) {
            eprintln!("Failed to write report: {e}");
            std::process::exit(1);
        }
        println!("Report written to: {}", path.display());
    } else if json_output {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        print_terminal_report(&report);
    }
}

/// Parse a month string (YYYY-MM) into date range.
fn parse_month(month: &str) -> (NaiveDate, NaiveDate, String, String) {
    let parts: Vec<&str> = month.split('-').collect();
    if parts.len() != 2 {
        eprintln!("Invalid month format. Use YYYY-MM (e.g., 2025-01)");
        std::process::exit(1);
    }

    let year: i32 = parts[0].parse().unwrap_or_else(|_| {
        eprintln!("Invalid year in month");
        std::process::exit(1);
    });
    let month_num: u32 = parts[1].parse().unwrap_or_else(|_| {
        eprintln!("Invalid month number");
        std::process::exit(1);
    });

    let start = NaiveDate::from_ymd_opt(year, month_num, 1).unwrap_or_else(|| {
        eprintln!("Invalid month: {}", month);
        std::process::exit(1);
    });

    let end = if month_num == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month_num + 1, 1).unwrap()
    } - Duration::days(1);

    (start, end, month.to_string(), "month".to_string())
}

/// Parse a week string (YYYY-WXX) into date range.
fn parse_week(week: &str) -> (NaiveDate, NaiveDate, String, String) {
    let parts: Vec<&str> = week.split("-W").collect();
    if parts.len() != 2 {
        eprintln!("Invalid week format. Use YYYY-WXX (e.g., 2025-W01)");
        std::process::exit(1);
    }

    let year: i32 = parts[0].parse().unwrap_or_else(|_| {
        eprintln!("Invalid year in week");
        std::process::exit(1);
    });
    let week_num: u32 = parts[1].parse().unwrap_or_else(|_| {
        eprintln!("Invalid week number");
        std::process::exit(1);
    });

    // ISO week: Week 1 is the first week with at least 4 days in the new year
    let jan4 = NaiveDate::from_ymd_opt(year, 1, 4).unwrap();
    let week1_monday =
        jan4 - Duration::days(jan4.weekday().num_days_from_monday() as i64);
    let start = week1_monday + Duration::weeks((week_num - 1) as i64);
    let end = start + Duration::days(6);

    (start, end, week.to_string(), "week".to_string())
}

/// Generate report data for the given period.
fn generate_report(
    db: &IndexDb,
    start_date: NaiveDate,
    end_date: NaiveDate,
    period: &str,
    period_type: &str,
) -> ReportData {
    // Query all notes
    let all_notes = db.query_notes(&NoteQuery::default()).unwrap_or_default();

    // Filter tasks
    let tasks: Vec<&IndexedNote> = all_notes
        .iter()
        .filter(|n| get_note_type(n) == Some("task".to_string()))
        .collect();

    // Filter projects
    let projects: Vec<&IndexedNote> = all_notes
        .iter()
        .filter(|n| get_note_type(n) == Some("project".to_string()))
        .collect();

    // Filter daily notes
    let daily_notes: Vec<&IndexedNote> = all_notes
        .iter()
        .filter(|n| get_note_type(n) == Some("daily".to_string()))
        .collect();

    // Count tasks completed in period
    let tasks_completed: Vec<&IndexedNote> = tasks
        .iter()
        .filter(|t| {
            get_completed_at(t).map(|d| d >= start_date && d <= end_date).unwrap_or(false)
        })
        .copied()
        .collect();

    // Count tasks created in period
    let tasks_created: Vec<&IndexedNote> = tasks
        .iter()
        .filter(|t| {
            get_created_at(t).map(|d| d >= start_date && d <= end_date).unwrap_or(false)
        })
        .copied()
        .collect();

    // Count daily notes in period
    let daily_notes_in_period: usize = daily_notes
        .iter()
        .filter(|n| {
            get_note_date(n).map(|d| d >= start_date && d <= end_date).unwrap_or(false)
        })
        .count();

    // Calculate days in period
    let days_in_period = (end_date - start_date).num_days() as usize + 1;

    // Group tasks by project
    let mut project_stats: HashMap<String, (String, usize, usize)> = HashMap::new();

    // Initialize with known projects
    for project in &projects {
        let (id, _) = extract_project_info(project);
        let title =
            if project.title.is_empty() { id.clone() } else { project.title.clone() };
        project_stats.insert(id, (title, 0, 0));
    }

    // Add inbox
    project_stats.insert("INB".to_string(), ("Inbox".to_string(), 0, 0));

    // Count created tasks by project
    for task in &tasks_created {
        let project = get_task_project(task).unwrap_or_else(|| "INB".to_string());
        if let Some(entry) = project_stats.get_mut(&project) {
            entry.1 += 1;
        } else {
            project_stats.insert(project.clone(), (project, 1, 0));
        }
    }

    // Count completed tasks by project
    for task in &tasks_completed {
        let project = get_task_project(task).unwrap_or_else(|| "INB".to_string());
        if let Some(entry) = project_stats.get_mut(&project) {
            entry.2 += 1;
        }
    }

    // Build project summary (only include projects with activity)
    let mut tasks_by_project: Vec<ProjectTaskSummary> = project_stats
        .into_iter()
        .filter(|(_, (_, created, completed))| *created > 0 || *completed > 0)
        .map(|(id, (title, created, completed))| ProjectTaskSummary {
            id,
            title,
            created,
            completed,
        })
        .collect();
    tasks_by_project.sort_by(|a, b| b.completed.cmp(&a.completed));

    // Build activity heatmap
    let mut activity_heatmap: Vec<DayActivity> = Vec::new();
    let mut current = start_date;
    while current <= end_date {
        let completed_on_day = tasks_completed
            .iter()
            .filter(|t| get_completed_at(t) == Some(current))
            .count();

        activity_heatmap.push(DayActivity {
            date: current.format("%Y-%m-%d").to_string(),
            weekday: format!("{:?}", current.weekday()),
            completed: completed_on_day,
        });

        current += Duration::days(1);
    }

    // Build top completed tasks
    let mut top_completed: Vec<CompletedTask> = tasks_completed
        .iter()
        .map(|t| {
            let id = get_task_id(t).unwrap_or_else(|| "-".to_string());
            let title = if t.title.is_empty() {
                t.path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled")
                    .to_string()
            } else {
                t.title.clone()
            };
            let completed_at = get_completed_at(t)
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default();
            let project = get_task_project(t).unwrap_or_else(|| "INB".to_string());

            CompletedTask { id, title, completed_at, project }
        })
        .collect();
    top_completed.sort_by(|a, b| b.completed_at.cmp(&a.completed_at));
    top_completed.truncate(10);

    // Count active projects (projects with any task activity)
    let projects_active = tasks_by_project.len();

    ReportData {
        period: period.to_string(),
        period_type: period_type.to_string(),
        start_date: start_date.format("%Y-%m-%d").to_string(),
        end_date: end_date.format("%Y-%m-%d").to_string(),
        generated_at: Utc::now().to_rfc3339(),
        summary: ReportSummary {
            tasks_completed: tasks_completed.len(),
            tasks_created: tasks_created.len(),
            projects_active,
            daily_notes: daily_notes_in_period,
            daily_notes_possible: days_in_period,
        },
        tasks_by_project,
        activity_heatmap,
        top_completed,
    }
}

/// Print report to terminal.
fn print_terminal_report(report: &ReportData) {
    let title = if report.period_type == "month" {
        format_month_title(&report.period)
    } else {
        format!("Weekly Report: {}", report.period)
    };

    println!();
    println!("{}", "═".repeat(65));
    println!("{:^65}", title);
    println!("{}", "═".repeat(65));
    println!();

    // Summary
    println!("SUMMARY");
    println!("  Tasks Completed:    {}", report.summary.tasks_completed);
    println!("  Tasks Created:      {}", report.summary.tasks_created);
    println!("  Projects Active:    {}", report.summary.projects_active);
    println!(
        "  Daily Notes:        {}/{} days",
        report.summary.daily_notes, report.summary.daily_notes_possible
    );
    println!();

    // Tasks by project
    if !report.tasks_by_project.is_empty() {
        println!("TASKS BY PROJECT");
        let rows: Vec<ProjectTaskRow> = report
            .tasks_by_project
            .iter()
            .map(|p| ProjectTaskRow {
                id: p.id.clone(),
                title: if p.title.len() > 20 {
                    format!("{}...", &p.title[..17])
                } else {
                    p.title.clone()
                },
                created: p.created,
                completed: p.completed,
            })
            .collect();
        let table = Table::new(&rows).with(Style::rounded()).to_string();
        println!("{}", table);
        println!();
    }

    // Activity heatmap (simplified for terminal)
    println!("ACTIVITY (tasks completed per day)");
    print_activity_heatmap(&report.activity_heatmap);
    println!();

    // Top completed
    if !report.top_completed.is_empty() {
        println!("TOP COMPLETED TASKS");
        for (i, task) in report.top_completed.iter().take(5).enumerate() {
            println!("  {}. {}: {}", i + 1, task.id, task.title);
        }
        println!();
    }
}

/// Print activity heatmap in a compact format.
fn print_activity_heatmap(heatmap: &[DayActivity]) {
    if heatmap.is_empty() {
        return;
    }

    // Group by week
    let mut weeks: Vec<Vec<&DayActivity>> = Vec::new();
    let mut current_week: Vec<&DayActivity> = Vec::new();

    for day in heatmap {
        current_week.push(day);
        if day.weekday == "Sun" {
            weeks.push(current_week);
            current_week = Vec::new();
        }
    }
    if !current_week.is_empty() {
        weeks.push(current_week);
    }

    // Print header
    println!("      Mon Tue Wed Thu Fri Sat Sun");

    // Print each week
    for (i, week) in weeks.iter().enumerate() {
        // Use the ISO week number from the first day of this week
        let week_num = if let Some(first_day) = week.first() {
            NaiveDate::parse_from_str(&first_day.date, "%Y-%m-%d")
                .map(|d| d.iso_week().week())
                .unwrap_or(0)
        } else {
            0
        };
        print!("  W{:02} ", week_num);

        // Pad beginning of first week if needed
        if i == 0 && !week.is_empty() {
            let first_weekday = &week[0].weekday;
            let padding = match first_weekday.as_str() {
                "Mon" => 0,
                "Tue" => 1,
                "Wed" => 2,
                "Thu" => 3,
                "Fri" => 4,
                "Sat" => 5,
                "Sun" => 6,
                _ => 0,
            };
            for _ in 0..padding {
                print!("    ");
            }
        }

        for day in week {
            print!(" {:>2} ", day.completed);
        }
        println!();
    }
}

/// Format month title (e.g., "2025-01" -> "Monthly Report: January 2025").
fn format_month_title(period: &str) -> String {
    let parts: Vec<&str> = period.split('-').collect();
    if parts.len() != 2 {
        return format!("Monthly Report: {}", period);
    }

    let month_names = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];

    let month_num: usize = parts[1].parse().unwrap_or(1);
    let month_name = month_names.get(month_num.saturating_sub(1)).unwrap_or(&"Unknown");

    format!("Monthly Report: {} {}", month_name, parts[0])
}

/// Format report as markdown.
fn format_markdown_report(report: &ReportData) -> String {
    let title = if report.period_type == "month" {
        format_month_title(&report.period)
    } else {
        format!("Weekly Report: {}", report.period)
    };

    let mut md = String::new();

    // Frontmatter
    md.push_str("---\n");
    md.push_str("type: report\n");
    md.push_str(&format!("period: {}\n", report.period));
    md.push_str(&format!("period_type: {}\n", report.period_type));
    md.push_str(&format!("generated: {}\n", report.generated_at));
    md.push_str("---\n\n");

    // Title
    md.push_str(&format!("# {}\n\n", title));

    // Summary
    md.push_str("## Summary\n\n");
    md.push_str("| Metric | Value |\n");
    md.push_str("|--------|-------|\n");
    md.push_str(&format!("| Tasks Completed | {} |\n", report.summary.tasks_completed));
    md.push_str(&format!("| Tasks Created | {} |\n", report.summary.tasks_created));
    md.push_str(&format!("| Projects Active | {} |\n", report.summary.projects_active));
    md.push_str(&format!(
        "| Daily Notes | {}/{} days |\n",
        report.summary.daily_notes, report.summary.daily_notes_possible
    ));
    md.push('\n');

    // Tasks by project
    if !report.tasks_by_project.is_empty() {
        md.push_str("## Tasks by Project\n\n");
        md.push_str("| ID | Project | Created | Done |\n");
        md.push_str("|----|---------|---------|------|\n");
        for p in &report.tasks_by_project {
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                p.id, p.title, p.created, p.completed
            ));
        }
        md.push('\n');
    }

    // Top completed
    if !report.top_completed.is_empty() {
        md.push_str("## Top Completed Tasks\n\n");
        for (i, task) in report.top_completed.iter().enumerate() {
            md.push_str(&format!(
                "{}. **{}**: {} ({})\n",
                i + 1,
                task.id,
                task.title,
                task.completed_at
            ));
        }
        md.push('\n');
    }

    md
}

/// Run the dashboard report command.
pub fn run_dashboard(
    config: Option<&Path>,
    profile: Option<&str>,
    project: Option<&str>,
    activity_days: u32,
    json_output: bool,
    output: Option<&Path>,
    visual: bool,
) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            std::process::exit(1);
        }
    };

    let index_path = cfg.vault_root.join(".mdvault/index.db");
    let db = match IndexDb::open(&index_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open index: {e}");
            eprintln!("Run 'mdv reindex' first.");
            std::process::exit(1);
        }
    };

    let options = mdvault_core::report::DashboardOptions {
        project: project.map(String::from),
        activity_days,
        ..Default::default()
    };

    let report = match mdvault_core::report::build_dashboard(&db, &options) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to generate dashboard: {e}");
            std::process::exit(1);
        }
    };

    if visual {
        let png_path = if let Some(path) = output {
            path.to_path_buf()
        } else {
            // Default: write to vault assets directory
            let filename = match &report.scope {
                mdvault_core::report::ReportScope::Project { id, .. } => {
                    format!("dashboard-{}.png", id.to_lowercase())
                }
                mdvault_core::report::ReportScope::Vault => {
                    "dashboard-vault.png".to_string()
                }
            };
            cfg.vault_root.join("assets").join("dashboards").join(filename)
        };

        let gen_fn = if project.is_some() {
            super::charts::generate_project_dashboard_png
        } else {
            super::charts::generate_dashboard_png
        };

        match gen_fn(&report, &png_path) {
            Ok(()) => {
                let rel_path =
                    png_path.strip_prefix(&cfg.vault_root).unwrap_or(&png_path);
                println!("Dashboard PNG written to: {}", png_path.display());
                println!("Embed in markdown:  ![dashboard]({})", rel_path.display());
            }
            Err(e) => {
                eprintln!("Failed to generate dashboard PNG: {e}");
                std::process::exit(1);
            }
        }

        if json_output {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
        }
    } else if json_output {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else if let Some(path) = output {
        let json_str = serde_json::to_string_pretty(&report).unwrap();
        if let Err(e) = std::fs::write(path, &json_str) {
            eprintln!("Failed to write report: {e}");
            std::process::exit(1);
        }
        println!("Dashboard written to: {}", path.display());
    } else {
        print_dashboard_terminal(&report);
    }
}

/// Print dashboard report to terminal in human-readable format.
fn print_dashboard_terminal(report: &mdvault_core::report::DashboardReport) {
    println!();
    println!("{}", "═".repeat(65));
    match &report.scope {
        mdvault_core::report::ReportScope::Vault => {
            println!("{:^65}", "Vault Dashboard");
        }
        mdvault_core::report::ReportScope::Project { id, title } => {
            println!("{:^65}", format!("Dashboard: {} [{}]", title, id));
        }
    }
    println!("{}", "═".repeat(65));
    println!();

    // Summary
    println!("SUMMARY");
    println!("  Total Notes:     {}", report.summary.total_notes);
    println!("  Total Tasks:     {}", report.summary.total_tasks);
    println!("  Total Projects:  {}", report.summary.total_projects);
    println!("  Active Projects: {}", report.summary.active_projects);

    if !report.summary.tasks_by_status.is_empty() {
        print!("  Tasks by Status:");
        let mut statuses: Vec<_> = report.summary.tasks_by_status.iter().collect();
        statuses.sort_by_key(|(k, _)| (*k).clone());
        for (status, count) in &statuses {
            print!("  {}={}", status, count);
        }
        println!();
    }
    println!();

    // Projects
    if !report.projects.is_empty() {
        println!("PROJECTS");
        for p in &report.projects {
            let bar_width: usize = 20;
            let filled =
                ((p.progress_percent / 100.0) * bar_width as f64).round() as usize;
            let empty = bar_width.saturating_sub(filled);
            let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

            println!(
                "  {} [{}]  {} {:.0}%  ({}/{} done)  vel: {:.1}/wk",
                p.title,
                p.id,
                bar,
                p.progress_percent,
                p.tasks.done,
                p.tasks.total,
                p.velocity.tasks_per_week_4w
            );
        }
        println!();
    }

    // Recent completions (across all projects, deduplicated)
    let all_completions: Vec<_> =
        report.projects.iter().flat_map(|p| p.recent_completions.iter()).collect();
    if !all_completions.is_empty() {
        println!("RECENT COMPLETIONS (7 days)");
        for c in all_completions.iter().take(10) {
            println!("  {} {} ({})", c.completed_at, c.id, c.title);
        }
        println!();
    }

    // Stale notes
    if !report.activity.stale_notes.is_empty() {
        println!("STALE NOTES (top {})", report.activity.stale_notes.len());
        for s in &report.activity.stale_notes {
            let last = s.last_seen.as_deref().unwrap_or("never");
            println!(
                "  [{:.2}] {} ({}, last seen: {})",
                s.staleness_score, s.title, s.note_type, last
            );
        }
        println!();
    }
}

// --- Helper functions ---

/// Get note type from frontmatter.
fn get_note_type(note: &IndexedNote) -> Option<String> {
    note.frontmatter_json
        .as_ref()
        .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok())
        .and_then(|fm| fm.get("type").and_then(|v| v.as_str()).map(String::from))
}

/// Parse a date string that may be in various formats:
/// - "YYYY-MM-DD"
/// - "YYYY-MM-DDThh:mm:ss" (no timezone)
/// - "YYYY-MM-DD hh:mm:ss" (space separator)
/// - "YYYY-MM-DD hh:mm:ss.fff" (fractional seconds)
/// - RFC3339 ("YYYY-MM-DDThh:mm:ss+00:00")
fn parse_flexible_date(date_str: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .ok()
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|dt| dt.date())
        })
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|dt| dt.date())
        })
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S%.f")
                .ok()
                .map(|dt| dt.date())
        })
        .or_else(|| {
            chrono::DateTime::parse_from_rfc3339(date_str).ok().map(|dt| dt.date_naive())
        })
}

/// Get completed_at date from frontmatter.
fn get_completed_at(note: &IndexedNote) -> Option<NaiveDate> {
    let fm_json = note.frontmatter_json.as_ref()?;
    let fm: serde_json::Value = serde_json::from_str(fm_json).ok()?;
    let date_str = fm.get("completed_at")?.as_str()?;
    parse_flexible_date(date_str)
}

/// Get created_at date from frontmatter.
fn get_created_at(note: &IndexedNote) -> Option<NaiveDate> {
    let fm_json = note.frontmatter_json.as_ref()?;
    let fm: serde_json::Value = serde_json::from_str(fm_json).ok()?;
    let date_str = fm.get("created_at")?.as_str()?;
    parse_flexible_date(date_str)
}

/// Get date from daily note frontmatter.
fn get_note_date(note: &IndexedNote) -> Option<NaiveDate> {
    let fm_json = note.frontmatter_json.as_ref()?;
    let fm: serde_json::Value = serde_json::from_str(fm_json).ok()?;
    let date_str = fm.get("date")?.as_str()?;

    NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()
}

/// Get task ID from frontmatter.
fn get_task_id(note: &IndexedNote) -> Option<String> {
    note.frontmatter_json
        .as_ref()
        .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok())
        .and_then(|fm| fm.get("task-id").and_then(|v| v.as_str()).map(String::from))
}

/// Get project from task frontmatter or path.
fn get_task_project(note: &IndexedNote) -> Option<String> {
    // Try frontmatter first
    if let Some(fm_json) = &note.frontmatter_json {
        if let Ok(fm) = serde_json::from_str::<serde_json::Value>(fm_json) {
            if let Some(project) = fm.get("project").and_then(|v| v.as_str()) {
                return Some(project.to_string());
            }
        }
    }

    // Try to extract from path (Projects/{project}/Tasks/...)
    let path_str = note.path.to_string_lossy();
    if path_str.contains("Projects/") {
        let parts: Vec<&str> = path_str.split("Projects/").collect();
        if parts.len() > 1 {
            let after_projects = parts[1];
            if let Some(project_name) = after_projects.split('/').next() {
                // Get project ID from folder name
                return Some(project_name.to_string());
            }
        }
    }

    None
}

/// Extract project ID and status from frontmatter.
fn extract_project_info(project: &IndexedNote) -> (String, String) {
    let fm = project
        .frontmatter_json
        .as_ref()
        .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok());

    let id = fm
        .as_ref()
        .and_then(|fm| fm.get("project-id").and_then(|v| v.as_str()))
        .map(String::from)
        .unwrap_or_else(|| {
            project.path.file_stem().and_then(|s| s.to_str()).unwrap_or("???").to_string()
        });

    let status = fm
        .as_ref()
        .and_then(|fm| fm.get("status").and_then(|v| v.as_str()))
        .map(String::from)
        .unwrap_or_else(|| "unknown".to_string());

    (id, status)
}
