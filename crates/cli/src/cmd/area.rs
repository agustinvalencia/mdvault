//! Area management commands.

use chrono::{Datelike, Local, NaiveDate};
use color_eyre::eyre::{bail, Result};
use mdvault_core::index::{IndexedNote, NoteQuery, NoteType};
use serde::Serialize;
use std::path::Path;
use tabled::{settings::Style, Table, Tabled};

use super::common::{load_config, open_index};

// ── Helpers ──────────────────────────────────────────────────────────────

fn get_fm_json(note: &IndexedNote) -> Option<serde_json::Value> {
    note.frontmatter_json.as_ref().and_then(|fm| serde_json::from_str(fm).ok())
}

fn get_fm_str(fm: &serde_json::Value, key: &str) -> Option<String> {
    fm.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn get_fm_bool(fm: &serde_json::Value, key: &str) -> Option<bool> {
    fm.get(key).and_then(|v| {
        // Handle both bool and string "true"/"false"
        v.as_bool().or_else(|| v.as_str().map(|s| s == "true"))
    })
}

fn get_note_date(note: &IndexedNote) -> Option<NaiveDate> {
    let fm = get_fm_json(note)?;
    let date_str = get_fm_str(&fm, "date")?;
    NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()
}

// ── Types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
struct Criterion {
    field: String,
    target: u32,
    period: String,
    label: String,
}

#[derive(Debug, Serialize)]
struct CriterionResult {
    label: String,
    field: String,
    actual: u32,
    target: u32,
    met: bool,
}

#[derive(Debug, Serialize)]
struct AreaReport {
    area: String,
    area_id: String,
    period: String,
    criteria: Vec<CriterionResult>,
}

#[derive(Tabled)]
struct CriterionRow {
    #[tabled(rename = "Standard")]
    label: String,
    #[tabled(rename = "Actual")]
    actual: u32,
    #[tabled(rename = "Target")]
    target: u32,
    #[tabled(rename = "Met")]
    met: String,
}

// ── Parsing ──────────────────────────────────────────────────────────────

fn parse_health_criteria(fm: &serde_json::Value) -> Vec<Criterion> {
    let Some(criteria_val) = fm.get("health_criteria") else {
        return Vec::new();
    };
    let Some(arr) = criteria_val.as_array() else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|c| {
            let field = c.get("field")?.as_str()?.to_string();
            let target = c.get("target")?.as_u64()? as u32;
            let period =
                c.get("period").and_then(|v| v.as_str()).unwrap_or("week").to_string();
            let label =
                c.get("label").and_then(|v| v.as_str()).unwrap_or(&field).to_string();
            Some(Criterion { field, target, period, label })
        })
        .collect()
}

/// Parse a period string into a date range (start, end) inclusive.
fn parse_period(period: &str) -> (NaiveDate, NaiveDate, String) {
    let today = Local::now().date_naive();

    match period {
        "week" => {
            let iso = today.iso_week();
            let start =
                NaiveDate::from_isoywd_opt(iso.year(), iso.week(), chrono::Weekday::Mon)
                    .unwrap_or(today);
            let end = start + chrono::Duration::days(6);
            let label = format!("{}-W{:02}", iso.year(), iso.week());
            (start, end, label)
        }
        "month" => {
            let start =
                NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
            let end = if today.month() == 12 {
                NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1)
            }
            .map(|d| d - chrono::Duration::days(1))
            .unwrap_or(today);
            let label = format!("{}-{:02}", today.year(), today.month());
            (start, end, label)
        }
        s if s.contains("-W") => {
            // e.g. "2026-W11"
            let parts: Vec<&str> = s.split("-W").collect();
            if let (Some(year), Some(week)) = (
                parts.first().and_then(|y| y.parse::<i32>().ok()),
                parts.get(1).and_then(|w| w.parse::<u32>().ok()),
            ) {
                let start = NaiveDate::from_isoywd_opt(year, week, chrono::Weekday::Mon)
                    .unwrap_or(today);
                let end = start + chrono::Duration::days(6);
                (start, end, s.to_string())
            } else {
                (today, today, s.to_string())
            }
        }
        s if s.len() == 7 && s.contains('-') => {
            // e.g. "2026-03"
            let parts: Vec<&str> = s.split('-').collect();
            if let (Some(year), Some(month)) = (
                parts.first().and_then(|y| y.parse::<i32>().ok()),
                parts.get(1).and_then(|m| m.parse::<u32>().ok()),
            ) {
                let start = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(today);
                let end = if month == 12 {
                    NaiveDate::from_ymd_opt(year + 1, 1, 1)
                } else {
                    NaiveDate::from_ymd_opt(year, month + 1, 1)
                }
                .map(|d| d - chrono::Duration::days(1))
                .unwrap_or(today);
                (start, end, s.to_string())
            } else {
                (today, today, s.to_string())
            }
        }
        _ => {
            // Fallback: current week
            let iso = today.iso_week();
            let start =
                NaiveDate::from_isoywd_opt(iso.year(), iso.week(), chrono::Weekday::Mon)
                    .unwrap_or(today);
            let end = start + chrono::Duration::days(6);
            let label = format!("{}-W{:02}", iso.year(), iso.week());
            (start, end, label)
        }
    }
}

// ── Find area ────────────────────────────────────────────────────────────

fn find_area<'a>(
    projects: &'a [IndexedNote],
    area_name: &str,
) -> Option<&'a IndexedNote> {
    projects.iter().find(|p| {
        let fm = get_fm_json(p);
        let kind = fm
            .as_ref()
            .and_then(|f| get_fm_str(f, "kind"))
            .unwrap_or_else(|| "project".to_string());

        if kind != "area" {
            return false;
        }

        let folder = p.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let id =
            fm.as_ref().and_then(|f| get_fm_str(f, "project-id")).unwrap_or_default();

        folder.eq_ignore_ascii_case(area_name) || id.eq_ignore_ascii_case(area_name)
    })
}

// ── Report command ───────────────────────────────────────────────────────

pub fn report(
    config: Option<&Path>,
    profile: Option<&str>,
    area_name: &str,
    period: &str,
    json_output: bool,
) -> Result<()> {
    let cfg = load_config(config, profile)?;
    let db = open_index(&cfg.vault_root)?;

    // Find the area
    let projects = db
        .query_notes(&NoteQuery {
            note_type: Some(NoteType::Project),
            ..Default::default()
        })
        .unwrap_or_default();

    let area = match find_area(&projects, area_name) {
        Some(a) => a,
        None => {
            eprintln!("Run 'mdv project list --kind area' to see available areas.");
            bail!("Area not found: {}", area_name);
        }
    };

    let fm = get_fm_json(area).unwrap_or(serde_json::Value::Null);
    let area_id = get_fm_str(&fm, "project-id").unwrap_or_else(|| "???".to_string());
    let area_title = if area.title.is_empty() {
        area.path.file_stem().and_then(|s| s.to_str()).unwrap_or("???").to_string()
    } else {
        area.title.clone()
    };

    let criteria = parse_health_criteria(&fm);
    if criteria.is_empty() {
        if json_output {
            let report = AreaReport {
                area: area_title,
                area_id,
                period: period.to_string(),
                criteria: Vec::new(),
            };
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
        } else {
            println!("Area: {} [{}]", area_title, area_id);
            println!("No health_criteria defined. Add them to the area frontmatter.");
        }
        return Ok(());
    }

    // Parse period
    let (start, end, period_label) = parse_period(period);

    // Query daily notes
    let daily_notes = db
        .query_notes(&NoteQuery {
            note_type: Some(NoteType::Daily),
            ..Default::default()
        })
        .unwrap_or_default();

    // Filter to date range
    let daily_in_range: Vec<&IndexedNote> = daily_notes
        .iter()
        .filter(|n| {
            if let Some(date) = get_note_date(n) {
                date >= start && date <= end
            } else {
                false
            }
        })
        .collect();

    // Evaluate each criterion
    let results: Vec<CriterionResult> = criteria
        .iter()
        .map(|c| {
            let actual = daily_in_range
                .iter()
                .filter(|n| {
                    if let Some(fm) = get_fm_json(n) {
                        get_fm_bool(&fm, &c.field).unwrap_or(false)
                    } else {
                        false
                    }
                })
                .count() as u32;

            CriterionResult {
                label: c.label.clone(),
                field: c.field.clone(),
                actual,
                target: c.target,
                met: actual >= c.target,
            }
        })
        .collect();

    if json_output {
        let report = AreaReport {
            area: area_title,
            area_id,
            period: period_label,
            criteria: results,
        };
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        println!("Area: {} ({})  ", area_title, period_label);
        println!();

        let rows: Vec<CriterionRow> = results
            .iter()
            .map(|r| CriterionRow {
                label: r.label.clone(),
                actual: r.actual,
                target: r.target,
                met: if r.met { "✓".into() } else { "✗".into() },
            })
            .collect();

        let table = Table::new(&rows).with(Style::rounded()).to_string();
        println!("{table}");
    }
    Ok(())
}

// ── Export command ────────────────────────────────────────────────────────

pub fn export(
    config: Option<&Path>,
    profile: Option<&str>,
    area_name: &str,
    from: Option<&str>,
    to: Option<&str>,
    format: &str,
) -> Result<()> {
    let cfg = load_config(config, profile)?;
    let db = open_index(&cfg.vault_root)?;

    // Find the area
    let projects = db
        .query_notes(&NoteQuery {
            note_type: Some(NoteType::Project),
            ..Default::default()
        })
        .unwrap_or_default();

    let area = match find_area(&projects, area_name) {
        Some(a) => a,
        None => {
            bail!("Area not found: {}", area_name);
        }
    };

    let fm = get_fm_json(area).unwrap_or(serde_json::Value::Null);
    let criteria = parse_health_criteria(&fm);
    if criteria.is_empty() {
        bail!("No health_criteria defined for this area.");
    }

    // Parse date range
    let today = Local::now().date_naive();
    let start = from
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| today - chrono::Duration::days(30));
    let end =
        to.and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()).unwrap_or(today);

    // Query daily notes
    let daily_notes = db
        .query_notes(&NoteQuery {
            note_type: Some(NoteType::Daily),
            ..Default::default()
        })
        .unwrap_or_default();

    // Filter and sort by date
    let mut daily_in_range: Vec<(&IndexedNote, NaiveDate)> = daily_notes
        .iter()
        .filter_map(|n| {
            let date = get_note_date(n)?;
            if date >= start && date <= end {
                Some((n, date))
            } else {
                None
            }
        })
        .collect();
    daily_in_range.sort_by_key(|(_, d)| *d);

    let fields: Vec<&str> = criteria.iter().map(|c| c.field.as_str()).collect();

    match format {
        "json" => {
            let data: Vec<serde_json::Value> = daily_in_range
                .iter()
                .map(|(note, date)| {
                    let fm = get_fm_json(note).unwrap_or(serde_json::Value::Null);
                    let mut row = serde_json::Map::new();
                    row.insert(
                        "date".into(),
                        serde_json::Value::String(date.to_string()),
                    );
                    for field in &fields {
                        let val = get_fm_bool(&fm, field).unwrap_or(false);
                        row.insert(field.to_string(), serde_json::Value::Bool(val));
                    }
                    serde_json::Value::Object(row)
                })
                .collect();

            let output = serde_json::json!({ "data": data });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        _ => {
            // CSV
            // Header
            print!("date");
            for field in &fields {
                print!(",{field}");
            }
            println!();

            // Rows
            for (note, date) in &daily_in_range {
                let fm = get_fm_json(note).unwrap_or(serde_json::Value::Null);
                print!("{date}");
                for field in &fields {
                    let val = get_fm_bool(&fm, field).unwrap_or(false);
                    print!(",{val}");
                }
                println!();
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_health_criteria_basic() {
        let fm = json!({
            "health_criteria": [
                {"field": "exercise", "target": 3, "period": "week", "label": "Move 3x/week"},
                {"field": "meds", "target": 7, "period": "week", "label": "Daily meds"}
            ]
        });
        let criteria = parse_health_criteria(&fm);
        assert_eq!(criteria.len(), 2);
        assert_eq!(criteria[0].field, "exercise");
        assert_eq!(criteria[0].target, 3);
        assert_eq!(criteria[0].label, "Move 3x/week");
        assert_eq!(criteria[1].field, "meds");
        assert_eq!(criteria[1].target, 7);
    }

    #[test]
    fn parse_health_criteria_missing_field() {
        let fm = json!({"title": "Health"});
        let criteria = parse_health_criteria(&fm);
        assert!(criteria.is_empty());
    }

    #[test]
    fn parse_health_criteria_defaults() {
        let fm = json!({
            "health_criteria": [
                {"field": "exercise", "target": 3}
            ]
        });
        let criteria = parse_health_criteria(&fm);
        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].period, "week");
        assert_eq!(criteria[0].label, "exercise"); // defaults to field name
    }

    #[test]
    fn get_fm_bool_handles_bool() {
        let fm = json!({"exercise": true, "meds": false});
        assert_eq!(get_fm_bool(&fm, "exercise"), Some(true));
        assert_eq!(get_fm_bool(&fm, "meds"), Some(false));
    }

    #[test]
    fn get_fm_bool_handles_string() {
        let fm = json!({"exercise": "true", "meds": "false"});
        assert_eq!(get_fm_bool(&fm, "exercise"), Some(true));
        assert_eq!(get_fm_bool(&fm, "meds"), Some(false));
    }

    #[test]
    fn get_fm_bool_missing_key() {
        let fm = json!({"title": "test"});
        assert_eq!(get_fm_bool(&fm, "exercise"), None);
    }

    #[test]
    fn parse_period_specific_week() {
        let (start, end, label) = parse_period("2026-W11");
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 3, 9).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 3, 15).unwrap());
        assert_eq!(label, "2026-W11");
    }

    #[test]
    fn parse_period_specific_month() {
        let (start, end, label) = parse_period("2026-03");
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 3, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 3, 31).unwrap());
        assert_eq!(label, "2026-03");
    }

    #[test]
    fn parse_period_december_month() {
        let (start, end, _) = parse_period("2026-12");
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 12, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
    }

    #[test]
    fn parse_period_week_is_7_days() {
        let (start, end, _) = parse_period("2026-W01");
        assert_eq!((end - start).num_days(), 6); // Mon-Sun inclusive
    }
}
