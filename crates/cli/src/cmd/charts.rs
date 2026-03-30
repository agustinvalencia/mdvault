//! PNG dashboard chart generation using charts-rs.
//!
//! Consumes `DashboardReport` from mdvault-core and produces multi-panel
//! PNG images suitable for embedding in markdown notes.

use charts_rs::{
    BarChart, ChildChart, LineChart, MultiChart, PieChart, Series, THEME_GRAFANA,
    svg_to_png,
};
use mdvault_core::report::{DashboardReport, ProjectReport};
use std::path::Path;

/// Generate a dashboard PNG from a report and write it to `output_path`.
pub fn generate_dashboard_png(
    report: &DashboardReport,
    output_path: &Path,
) -> Result<(), String> {
    let mut multi = MultiChart::new();
    multi.margin = (10.0).into();
    multi.background_color = Some((31, 29, 29, 255).into());

    // Panel 1: Task status pie (aggregate across all projects in scope)
    let pie = build_task_status_pie(report);
    multi.add(ChildChart::Pie(pie, None));

    // Panel 2: Project progress bar chart (if multiple projects)
    if report.projects.len() > 1 {
        let bar = build_project_progress_bar(report);
        multi.add(ChildChart::Bar(bar, None));
    }

    // Panel 3: Activity timeline (tasks completed + created per day)
    if !report.activity.daily_activity.is_empty() {
        let line = build_activity_timeline(report);
        multi.add(ChildChart::Line(line, None));
    }

    // Panel 4: Velocity comparison across projects
    if report.projects.len() > 1 {
        let bar = build_velocity_bar(report);
        multi.add(ChildChart::HorizontalBar(bar, None));
    }

    let svg = multi.svg().map_err(|e| format!("SVG generation failed: {e}"))?;
    let png = svg_to_png(&svg).map_err(|e| format!("PNG conversion failed: {e}"))?;

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create output directory: {e}"))?;
    }

    std::fs::write(output_path, png).map_err(|e| format!("Failed to write PNG: {e}"))?;

    Ok(())
}

/// Generate a dashboard PNG for a single project.
pub fn generate_project_dashboard_png(
    report: &DashboardReport,
    output_path: &Path,
) -> Result<(), String> {
    let project = report.projects.first().ok_or("No project data in report")?;

    let mut multi = MultiChart::new();
    multi.margin = (10.0).into();
    multi.background_color = Some((31, 29, 29, 255).into());

    // Panel 1: Task status pie for this project
    let pie = build_single_project_pie(project);
    multi.add(ChildChart::Pie(pie, None));

    // Panel 2: Activity timeline
    if !report.activity.daily_activity.is_empty() {
        let line = build_activity_timeline(report);
        multi.add(ChildChart::Line(line, None));
    }

    let svg = multi.svg().map_err(|e| format!("SVG generation failed: {e}"))?;
    let png = svg_to_png(&svg).map_err(|e| format!("PNG conversion failed: {e}"))?;

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create output directory: {e}"))?;
    }

    std::fs::write(output_path, png).map_err(|e| format!("Failed to write PNG: {e}"))?;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Chart builders
// ─────────────────────────────────────────────────────────────────────────────

fn build_task_status_pie(report: &DashboardReport) -> PieChart {
    let status = &report.summary.tasks_by_status;

    let series: Vec<Series> = [
        ("Done", status.get("done").copied().unwrap_or(0)),
        ("Todo", status.get("todo").copied().unwrap_or(0)),
        ("In Progress", status.get("in_progress").copied().unwrap_or(0)),
        ("Blocked", status.get("blocked").copied().unwrap_or(0)),
        ("Cancelled", status.get("cancelled").copied().unwrap_or(0)),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
    .map(|(name, count)| (name, vec![count as f32]).into())
    .collect();

    let mut pie = PieChart::new_with_theme(series, THEME_GRAFANA);
    pie.title_text = "Task Status".to_string();
    pie.width = 500.0;
    pie.height = 350.0;
    pie
}

fn build_single_project_pie(project: &ProjectReport) -> PieChart {
    let t = &project.tasks;

    let series: Vec<Series> = [
        ("Done", t.done),
        ("Todo", t.todo),
        ("In Progress", t.in_progress),
        ("Blocked", t.blocked),
        ("Cancelled", t.cancelled),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
    .map(|(name, count)| (name, vec![count as f32]).into())
    .collect();

    let mut pie = PieChart::new_with_theme(series, THEME_GRAFANA);
    pie.title_text = format!("{} [{}] — Tasks", project.title, project.id);
    pie.width = 500.0;
    pie.height = 350.0;
    pie
}

fn build_project_progress_bar(report: &DashboardReport) -> BarChart {
    // Filter to projects with tasks, sorted by progress
    let mut projects: Vec<&ProjectReport> =
        report.projects.iter().filter(|p| p.tasks.total > 0).collect();
    projects.sort_by(|a, b| b.progress_percent.partial_cmp(&a.progress_percent).unwrap());

    let labels: Vec<String> = projects.iter().map(|p| p.id.clone()).collect();

    let done_data: Vec<f32> = projects.iter().map(|p| p.tasks.done as f32).collect();
    let open_data: Vec<f32> = projects
        .iter()
        .map(|p| (p.tasks.todo + p.tasks.in_progress + p.tasks.blocked) as f32)
        .collect();

    let series = vec![("Done", done_data).into(), ("Open", open_data).into()];

    let mut bar = BarChart::new_with_theme(series, labels, THEME_GRAFANA);
    bar.title_text = "Tasks by Project".to_string();
    bar.title_margin = Some((10.0).into());
    bar.legend_margin = Some((30.0).into());
    bar.width = 600.0;
    bar.height = 400.0;
    bar.series_list[0].label_show = true;
    bar
}

fn build_activity_timeline(report: &DashboardReport) -> LineChart {
    let activity = &report.activity.daily_activity;

    // Sample to avoid overcrowding — show at most ~30 data points
    let step = (activity.len() / 30).max(1);
    let sampled: Vec<_> = activity
        .iter()
        .enumerate()
        .filter(|(i, _)| i % step == 0 || *i == activity.len() - 1)
        .map(|(_, d)| d)
        .collect();

    let labels: Vec<String> = sampled
        .iter()
        .map(|d| {
            // Show just MM-DD for readability
            d.date.get(5..).unwrap_or(&d.date).to_string()
        })
        .collect();

    let completed: Vec<f32> = sampled.iter().map(|d| d.tasks_completed as f32).collect();
    let created: Vec<f32> = sampled.iter().map(|d| d.tasks_created as f32).collect();

    let series = vec![("Completed", completed).into(), ("Created", created).into()];

    let mut line = LineChart::new_with_theme(series, labels, THEME_GRAFANA);
    line.title_text = "Activity Timeline".to_string();
    line.title_margin = Some((10.0).into());
    line.legend_margin = Some((30.0).into());
    line.width = 700.0;
    line.height = 350.0;
    line.series_smooth = true;
    line
}

fn build_velocity_bar(report: &DashboardReport) -> charts_rs::HorizontalBarChart {
    let mut projects: Vec<&ProjectReport> =
        report.projects.iter().filter(|p| p.velocity.tasks_per_week_4w > 0.0).collect();
    projects.sort_by(|a, b| {
        b.velocity.tasks_per_week_4w.partial_cmp(&a.velocity.tasks_per_week_4w).unwrap()
    });

    let labels: Vec<String> = projects.iter().map(|p| p.id.clone()).collect();

    let vel_4w: Vec<f32> =
        projects.iter().map(|p| p.velocity.tasks_per_week_4w as f32).collect();
    let vel_2w: Vec<f32> =
        projects.iter().map(|p| p.velocity.tasks_per_week_2w as f32).collect();

    let series = vec![("4-week avg", vel_4w).into(), ("2-week avg", vel_2w).into()];

    let mut bar =
        charts_rs::HorizontalBarChart::new_with_theme(series, labels, THEME_GRAFANA);
    bar.title_text = "Velocity (tasks/week)".to_string();
    bar.title_margin = Some((10.0).into());
    bar.legend_margin = Some((30.0).into());
    bar.width = 600.0;
    bar.height = 350.0;
    bar
}
