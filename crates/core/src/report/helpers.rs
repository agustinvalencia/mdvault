use chrono::NaiveDate;

use crate::index::IndexedNote;

pub(super) fn get_frontmatter_str(note: &IndexedNote, key: &str) -> Option<String> {
    note.frontmatter_json
        .as_ref()
        .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok())
        .and_then(|fm| fm.get(key).and_then(|v| v.as_str()).map(String::from))
}

pub(super) fn get_frontmatter_date(note: &IndexedNote, key: &str) -> Option<NaiveDate> {
    let date_str = get_frontmatter_str(note, key)?;
    NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
        .ok()
        .or_else(|| {
            chrono::DateTime::parse_from_rfc3339(&date_str).ok().map(|dt| dt.date_naive())
        })
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|dt| dt.date())
        })
        .or_else(|| {
            let trimmed = date_str.split('.').next().unwrap_or(&date_str);
            chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|dt| dt.date())
        })
}

/// Parse a duration string like "1w", "2w", "30d", "1m" into days.
pub(super) fn parse_review_interval(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (num_str, suffix) = s.split_at(s.len() - 1);
    let num: i64 = num_str.parse().ok()?;
    match suffix {
        "d" => Some(num),
        "w" => Some(num * 7),
        "m" => Some(num * 30),
        _ => None,
    }
}

pub(super) fn extract_project_info(project: &IndexedNote) -> (String, String, String) {
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

    let kind = fm
        .as_ref()
        .and_then(|fm| fm.get("kind").and_then(|v| v.as_str()))
        .map(String::from)
        .unwrap_or_else(|| "project".to_string());

    (id, status, kind)
}

pub(super) fn task_matches_project(task: &IndexedNote, project_folder: &str) -> bool {
    if let Some(project) = get_frontmatter_str(task, "project")
        && project.eq_ignore_ascii_case(project_folder)
    {
        return true;
    }

    let path_str = task.path.to_string_lossy();
    crate::domain::task_belongs_to_project(&path_str, project_folder)
}

pub(super) fn normalise_status(status: &str) -> String {
    match status {
        "todo" | "open" => "todo".to_string(),
        "in-progress" | "in_progress" | "doing" => "in_progress".to_string(),
        "blocked" | "waiting" => "blocked".to_string(),
        "done" | "completed" => "done".to_string(),
        "cancelled" | "canceled" => "cancelled".to_string(),
        other => other.to_string(),
    }
}
