//! Context query commands: day and week.

use std::path::Path;

use chrono::{Datelike, Duration, Local, NaiveDate};
use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::context::ContextQueryService;
use mdvault_core::vars::datemath::{parse_date_expr, DateBase};

/// Get context for a specific day.
pub fn day(
    config: Option<&Path>,
    profile: Option<&str>,
    date_arg: Option<&str>,
    format: &str,
    lookback: bool,
) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Configuration error: {e}");
            std::process::exit(1);
        }
    };

    // Parse date argument
    let date = match parse_date_arg(date_arg) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Invalid date: {e}");
            std::process::exit(1);
        }
    };

    let service = ContextQueryService::new(&cfg);

    // Get context
    let context = match service.day_context(date) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to get context: {e}");
            std::process::exit(1);
        }
    };

    // Handle lookback: if no activity and lookback is enabled, try previous days
    let context = if lookback && is_empty_context(&context) {
        find_last_active_day(&service, date).unwrap_or(context)
    } else {
        context
    };

    // Output based on format
    match format {
        "json" => {
            match serde_json::to_string_pretty(&context) {
                Ok(json) => println!("{}", json),
                Err(e) => {
                    eprintln!("Failed to serialize context: {e}");
                    std::process::exit(1);
                }
            }
        }
        "summary" => {
            println!("{}", context.to_summary());
        }
        _ => {
            // Default: markdown
            println!("{}", context.to_markdown());
        }
    }
}

/// Get context for a specific week.
pub fn week(
    config: Option<&Path>,
    profile: Option<&str>,
    week_arg: Option<&str>,
    format: &str,
) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Configuration error: {e}");
            std::process::exit(1);
        }
    };

    // Parse week argument
    let date = match parse_week_arg(week_arg) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Invalid week: {e}");
            std::process::exit(1);
        }
    };

    let service = ContextQueryService::new(&cfg);

    // Get context
    let context = match service.week_context(date) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to get context: {e}");
            std::process::exit(1);
        }
    };

    // Output based on format
    match format {
        "json" => {
            match serde_json::to_string_pretty(&context) {
                Ok(json) => println!("{}", json),
                Err(e) => {
                    eprintln!("Failed to serialize context: {e}");
                    std::process::exit(1);
                }
            }
        }
        "summary" => {
            println!("{}", context.to_summary());
        }
        _ => {
            // Default: markdown
            println!("{}", context.to_markdown());
        }
    }
}

/// Parse a date argument into NaiveDate.
fn parse_date_arg(arg: Option<&str>) -> Result<NaiveDate, String> {
    let arg = arg.unwrap_or("today");

    // Handle special keywords
    match arg.to_lowercase().as_str() {
        "today" => return Ok(Local::now().date_naive()),
        "yesterday" => return Ok(Local::now().date_naive() - Duration::days(1)),
        _ => {}
    }

    // Try ISO date format first (YYYY-MM-DD)
    if let Ok(date) = NaiveDate::parse_from_str(arg, "%Y-%m-%d") {
        return Ok(date);
    }

    // Try date expression
    match parse_date_expr(arg) {
        Ok(expr) => {
            // Evaluate the expression to get a date string
            let today = Local::now().date_naive();

            let base_date = match expr.base {
                DateBase::Today | DateBase::Date | DateBase::Now => today,
                DateBase::Yesterday => today - Duration::days(1),
                DateBase::Tomorrow => today + Duration::days(1),
                DateBase::WeekStart => {
                    let days = today.weekday().num_days_from_monday();
                    today - Duration::days(days as i64)
                }
                DateBase::WeekEnd => {
                    let days = 6 - today.weekday().num_days_from_monday();
                    today + Duration::days(days as i64)
                }
                DateBase::Literal(date) => date,
                DateBase::IsoWeek { year, week } => {
                    // Find Monday of the specified ISO week
                    NaiveDate::from_isoywd_opt(year, week, chrono::Weekday::Mon)
                        .ok_or_else(|| format!("Invalid ISO week: {}-W{:02}", year, week))?
                }
                _ => today,
            };

            // Apply offset
            let result = match expr.offset {
                mdvault_core::vars::datemath::DateOffset::None => base_date,
                mdvault_core::vars::datemath::DateOffset::Duration { amount, unit } => {
                    use mdvault_core::vars::datemath::DurationUnit;
                    match unit {
                        DurationUnit::Days => base_date + Duration::days(amount),
                        DurationUnit::Weeks => base_date + Duration::weeks(amount),
                        DurationUnit::Months => {
                            // Approximate: 30 days per month
                            base_date + Duration::days(amount * 30)
                        }
                        DurationUnit::Years => {
                            // Approximate: 365 days per year
                            base_date + Duration::days(amount * 365)
                        }
                        _ => base_date,
                    }
                }
                mdvault_core::vars::datemath::DateOffset::Weekday { weekday, direction } => {
                    use mdvault_core::vars::datemath::Direction;
                    let target_day = weekday.num_days_from_monday();
                    let current_day = base_date.weekday().num_days_from_monday();

                    let diff = match direction {
                        Direction::Previous => {
                            if current_day >= target_day {
                                current_day - target_day
                            } else {
                                7 - (target_day - current_day)
                            }
                        }
                        Direction::Next => {
                            if target_day > current_day {
                                target_day - current_day
                            } else {
                                7 - (current_day - target_day)
                            }
                        }
                    };

                    match direction {
                        Direction::Previous => base_date - Duration::days(diff as i64),
                        Direction::Next => base_date + Duration::days(diff as i64),
                    }
                }
            };

            Ok(result)
        }
        Err(e) => Err(format!("Invalid date expression: {}", e)),
    }
}

/// Parse a week argument into a date within that week.
fn parse_week_arg(arg: Option<&str>) -> Result<NaiveDate, String> {
    let arg = arg.unwrap_or("current");

    // Handle special keywords
    match arg.to_lowercase().as_str() {
        "current" | "this" => return Ok(Local::now().date_naive()),
        "last" | "previous" => return Ok(Local::now().date_naive() - Duration::weeks(1)),
        "next" => return Ok(Local::now().date_naive() + Duration::weeks(1)),
        _ => {}
    }

    // Try ISO week format (YYYY-Wxx)
    if arg.contains("-W") || arg.contains("-w") {
        let parts: Vec<&str> = arg.split(['-', 'W', 'w']).collect();
        if parts.len() >= 2 {
            let year: i32 = parts[0].parse().map_err(|_| "Invalid year in ISO week")?;
            let week: u32 = parts.last().unwrap().parse().map_err(|_| "Invalid week number")?;

            return NaiveDate::from_isoywd_opt(year, week, chrono::Weekday::Mon)
                .ok_or_else(|| format!("Invalid ISO week: {}-W{:02}", year, week));
        }
    }

    // Try as a date (get week containing that date)
    parse_date_arg(Some(arg))
}

/// Check if a day context has no activity.
fn is_empty_context(context: &mdvault_core::context::DayContext) -> bool {
    context.summary.tasks_completed == 0
        && context.summary.tasks_created == 0
        && context.summary.notes_modified == 0
}

/// Find the last day with activity within the past 30 days.
fn find_last_active_day(
    service: &ContextQueryService,
    start_date: NaiveDate,
) -> Option<mdvault_core::context::DayContext> {
    for i in 1..=30 {
        let date = start_date - Duration::days(i);
        if let Ok(ctx) = service.day_context(date) {
            if !is_empty_context(&ctx) {
                return Some(ctx);
            }
        }
    }
    None
}

/// Get context for a specific note.
pub fn note(
    config: Option<&Path>,
    profile: Option<&str>,
    note_path: &str,
    format: &str,
    activity_days: u32,
) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Configuration error: {e}");
            std::process::exit(1);
        }
    };

    let service = ContextQueryService::new(&cfg);

    // Normalize path (remove leading ./ if present)
    let note_path = note_path.trim_start_matches("./");
    let path = std::path::Path::new(note_path);

    // Get context
    let context = match service.note_context(path, activity_days) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to get note context: {e}");
            std::process::exit(1);
        }
    };

    // Output based on format
    match format {
        "json" => {
            match serde_json::to_string_pretty(&context) {
                Ok(json) => println!("{}", json),
                Err(e) => {
                    eprintln!("Failed to serialize context: {e}");
                    std::process::exit(1);
                }
            }
        }
        "summary" => {
            println!("{}", context.to_summary());
        }
        _ => {
            // Default: markdown
            println!("{}", context.to_markdown());
        }
    }
}

/// Get context for the focused project.
pub fn focus(
    config: Option<&Path>,
    profile: Option<&str>,
    format: &str,
    _with_tasks: bool, // TODO: implement with_tasks option
) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Configuration error: {e}");
            std::process::exit(1);
        }
    };

    let service = ContextQueryService::new(&cfg);

    // Get focus context
    let context = match service.focus_context() {
        Ok(ctx) => ctx,
        Err(e) => {
            // Check if it's just "no focus set"
            if e.to_string().contains("No focus set") {
                println!("No focus set. Use `mdv focus <project>` to set focus.");
                return;
            }
            eprintln!("Failed to get focus context: {e}");
            std::process::exit(1);
        }
    };

    // Output based on format
    match format {
        "json" => {
            match serde_json::to_string_pretty(&context) {
                Ok(json) => println!("{}", json),
                Err(e) => {
                    eprintln!("Failed to serialize context: {e}");
                    std::process::exit(1);
                }
            }
        }
        "summary" => {
            println!("{}", context.to_summary());
        }
        _ => {
            // Default: markdown
            println!("{}", context.to_markdown());
        }
    }
}
