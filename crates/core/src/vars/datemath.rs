//! Date math expression parser and evaluator.
//!
//! Supports expressions like:
//! - `{{today}}`, `{{now}}`, `{{time}}`, `{{week}}`, `{{year}}`
//! - `{{today + 1d}}`, `{{today - 1w}}`, `{{now + 2h}}`
//! - `{{today | %Y-%m-%d}}` (with format specifier)
//! - `{{today - monday}}`, `{{today + friday}}` (relative weekday)
//! - `{{week}}` returns ISO week number (1-53), `{{week | %Y-W%V}}` for "2025-W51"

use chrono::{
    Datelike, Duration, IsoWeek, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike,
    Weekday,
};
use regex::Regex;
use thiserror::Error;

/// Error type for date math parsing and evaluation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DateMathError {
    #[error("invalid date math expression: {0}")]
    InvalidExpression(String),

    #[error("invalid duration unit: {0}")]
    InvalidUnit(String),

    #[error("invalid number in expression: {0}")]
    InvalidNumber(String),

    #[error("invalid weekday: {0}")]
    InvalidWeekday(String),
}

/// A parsed date/time base value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateBase {
    /// Current date (YYYY-MM-DD)
    Today,
    /// Current datetime (ISO 8601)
    Now,
    /// Current time (HH:MM)
    Time,
    /// Current date (alias for today)
    Date,
    /// Current ISO week number (1-53)
    Week,
    /// Current year (YYYY)
    Year,
}

/// A duration offset to apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateOffset {
    /// No offset
    None,
    /// Duration: +/- N units (days, weeks, months, hours, minutes)
    Duration { amount: i64, unit: DurationUnit },
    /// Relative weekday: previous/next Monday, Tuesday, etc.
    Weekday { weekday: Weekday, direction: Direction },
}

/// Units for duration offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationUnit {
    Minutes,
    Hours,
    Days,
    Weeks,
    Months,
    Years,
}

/// Direction for relative weekday.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Previous, // - (go back to previous weekday)
    Next,     // + (go forward to next weekday)
}

/// A fully parsed date math expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateExpr {
    pub base: DateBase,
    pub offset: DateOffset,
    pub format: Option<String>,
}

/// Parse a date math expression.
///
/// Examples:
/// - `today` -> DateExpr { base: Today, offset: None, format: None }
/// - `today + 1d` -> DateExpr { base: Today, offset: Duration { amount: 1, unit: Days }, format: None }
/// - `now | %H:%M` -> DateExpr { base: Now, offset: None, format: Some("%H:%M") }
/// - `today - monday` -> DateExpr { base: Today, offset: Weekday { weekday: Monday, direction: Previous }, format: None }
pub fn parse_date_expr(input: &str) -> Result<DateExpr, DateMathError> {
    let input = input.trim();

    // Split by format specifier first
    let (expr_part, format) = if let Some(idx) = input.find('|') {
        let (e, f) = input.split_at(idx);
        (e.trim(), Some(f[1..].trim().to_string()))
    } else {
        (input, None)
    };

    // Parse base and offset
    let re = Regex::new(r"^(\w+)\s*([+-])?\s*(\w+)?$").expect("valid regex");

    if let Some(caps) = re.captures(expr_part) {
        let base_str = &caps[1];
        let base = parse_base(base_str)?;

        let offset = if let (Some(op), Some(operand)) = (caps.get(2), caps.get(3)) {
            let op_str = op.as_str();
            let operand_str = operand.as_str();
            parse_offset(op_str, operand_str)?
        } else {
            DateOffset::None
        };

        Ok(DateExpr { base, offset, format })
    } else {
        Err(DateMathError::InvalidExpression(input.to_string()))
    }
}

fn parse_base(s: &str) -> Result<DateBase, DateMathError> {
    match s.to_lowercase().as_str() {
        "today" => Ok(DateBase::Today),
        "now" => Ok(DateBase::Now),
        "time" => Ok(DateBase::Time),
        "date" => Ok(DateBase::Date),
        "week" => Ok(DateBase::Week),
        "year" => Ok(DateBase::Year),
        _ => Err(DateMathError::InvalidExpression(format!("unknown base: {s}"))),
    }
}

fn parse_offset(op: &str, operand: &str) -> Result<DateOffset, DateMathError> {
    let direction = match op {
        "+" => Direction::Next,
        "-" => Direction::Previous,
        _ => {
            return Err(DateMathError::InvalidExpression(format!(
                "invalid operator: {op}"
            )));
        }
    };

    // Try parsing as weekday first
    if let Ok(weekday) = parse_weekday(operand) {
        return Ok(DateOffset::Weekday { weekday, direction });
    }

    // Try parsing as duration (e.g., "1d", "2w", "3M")
    let re = Regex::new(r"^(\d+)([dmMyhwY])$").expect("valid regex");
    if let Some(caps) = re.captures(operand) {
        let amount: i64 = caps[1]
            .parse()
            .map_err(|_| DateMathError::InvalidNumber(caps[1].to_string()))?;

        let unit = match &caps[2] {
            "m" => DurationUnit::Minutes,
            "h" => DurationUnit::Hours,
            "d" => DurationUnit::Days,
            "w" => DurationUnit::Weeks,
            "M" => DurationUnit::Months,
            "y" | "Y" => DurationUnit::Years,
            u => return Err(DateMathError::InvalidUnit(u.to_string())),
        };

        let signed_amount = match direction {
            Direction::Next => amount,
            Direction::Previous => -amount,
        };

        return Ok(DateOffset::Duration { amount: signed_amount, unit });
    }

    Err(DateMathError::InvalidExpression(format!("invalid offset: {operand}")))
}

fn parse_weekday(s: &str) -> Result<Weekday, DateMathError> {
    match s.to_lowercase().as_str() {
        "monday" | "mon" => Ok(Weekday::Mon),
        "tuesday" | "tue" => Ok(Weekday::Tue),
        "wednesday" | "wed" => Ok(Weekday::Wed),
        "thursday" | "thu" => Ok(Weekday::Thu),
        "friday" | "fri" => Ok(Weekday::Fri),
        "saturday" | "sat" => Ok(Weekday::Sat),
        "sunday" | "sun" => Ok(Weekday::Sun),
        _ => Err(DateMathError::InvalidWeekday(s.to_string())),
    }
}

/// Evaluate a date expression and return the formatted result.
pub fn evaluate_date_expr(expr: &DateExpr) -> String {
    let now = Local::now();
    let today = now.date_naive();
    let current_time = now.time();

    match expr.base {
        DateBase::Today | DateBase::Date => {
            let date = apply_date_offset(today, &expr.offset);
            format_date(date, expr.format.as_deref())
        }
        DateBase::Now => {
            let datetime = apply_datetime_offset(now.naive_local(), &expr.offset);
            format_datetime(datetime, expr.format.as_deref())
        }
        DateBase::Time => {
            let time = apply_time_offset(current_time, &expr.offset);
            format_time(time, expr.format.as_deref())
        }
        DateBase::Week => {
            let date = apply_date_offset(today, &expr.offset);
            format_week(date.iso_week(), expr.format.as_deref())
        }
        DateBase::Year => {
            let date = apply_date_offset(today, &expr.offset);
            format_year(date, expr.format.as_deref())
        }
    }
}

fn apply_date_offset(date: NaiveDate, offset: &DateOffset) -> NaiveDate {
    match offset {
        DateOffset::None => date,
        DateOffset::Duration { amount, unit } => match unit {
            DurationUnit::Days => date + Duration::days(*amount),
            DurationUnit::Weeks => date + Duration::weeks(*amount),
            DurationUnit::Months => add_months(date, *amount),
            DurationUnit::Years => add_months(date, amount * 12),
            DurationUnit::Hours | DurationUnit::Minutes => date, // hours/minutes don't affect date
        },
        DateOffset::Weekday { weekday, direction } => {
            find_relative_weekday(date, *weekday, *direction)
        }
    }
}

fn apply_datetime_offset(dt: NaiveDateTime, offset: &DateOffset) -> NaiveDateTime {
    match offset {
        DateOffset::None => dt,
        DateOffset::Duration { amount, unit } => match unit {
            DurationUnit::Minutes => dt + Duration::minutes(*amount),
            DurationUnit::Hours => dt + Duration::hours(*amount),
            DurationUnit::Days => dt + Duration::days(*amount),
            DurationUnit::Weeks => dt + Duration::weeks(*amount),
            DurationUnit::Months => {
                let new_date = add_months(dt.date(), *amount);
                NaiveDateTime::new(new_date, dt.time())
            }
            DurationUnit::Years => {
                let new_date = add_months(dt.date(), amount * 12);
                NaiveDateTime::new(new_date, dt.time())
            }
        },
        DateOffset::Weekday { weekday, direction } => {
            let new_date = find_relative_weekday(dt.date(), *weekday, *direction);
            NaiveDateTime::new(new_date, dt.time())
        }
    }
}

fn apply_time_offset(time: NaiveTime, offset: &DateOffset) -> NaiveTime {
    match offset {
        DateOffset::None => time,
        DateOffset::Duration { amount, unit } => match unit {
            DurationUnit::Minutes => {
                let secs = time.num_seconds_from_midnight() as i64 + amount * 60;
                let normalized = secs.rem_euclid(86400) as u32;
                NaiveTime::from_num_seconds_from_midnight_opt(normalized, 0)
                    .unwrap_or(time)
            }
            DurationUnit::Hours => {
                let secs = time.num_seconds_from_midnight() as i64 + amount * 3600;
                let normalized = secs.rem_euclid(86400) as u32;
                NaiveTime::from_num_seconds_from_midnight_opt(normalized, 0)
                    .unwrap_or(time)
            }
            _ => time, // days/weeks/months don't affect time
        },
        DateOffset::Weekday { .. } => time, // weekdays don't affect time
    }
}

fn add_months(date: NaiveDate, months: i64) -> NaiveDate {
    let year = date.year() as i64;
    let month = date.month() as i64;
    let day = date.day();

    let total_months = year * 12 + month - 1 + months;
    let new_year = (total_months / 12) as i32;
    let new_month = (total_months % 12 + 1) as u32;

    // Handle day overflow (e.g., Jan 31 + 1 month = Feb 28/29)
    let max_day = days_in_month(new_year, new_month);
    let new_day = day.min(max_day);

    NaiveDate::from_ymd_opt(new_year, new_month, new_day).unwrap_or(date)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn find_relative_weekday(
    date: NaiveDate,
    target: Weekday,
    direction: Direction,
) -> NaiveDate {
    let current = date.weekday();

    match direction {
        Direction::Previous => {
            // Find the previous occurrence (or today if it's the target)
            let days_diff = (current.num_days_from_monday() as i64
                - target.num_days_from_monday() as i64
                + 7)
                % 7;
            let days_back = if days_diff == 0 { 7 } else { days_diff };
            date - Duration::days(days_back)
        }
        Direction::Next => {
            // Find the next occurrence (or today if it's the target)
            let days_diff = (target.num_days_from_monday() as i64
                - current.num_days_from_monday() as i64
                + 7)
                % 7;
            let days_forward = if days_diff == 0 { 7 } else { days_diff };
            date + Duration::days(days_forward)
        }
    }
}

fn format_date(date: NaiveDate, format: Option<&str>) -> String {
    let fmt = format.unwrap_or("%Y-%m-%d");
    date.format(fmt).to_string()
}

fn format_datetime(dt: NaiveDateTime, format: Option<&str>) -> String {
    let fmt = format.unwrap_or("%Y-%m-%dT%H:%M:%S");
    dt.format(fmt).to_string()
}

fn format_time(time: NaiveTime, format: Option<&str>) -> String {
    let fmt = format.unwrap_or("%H:%M");
    time.format(fmt).to_string()
}

fn format_week(week: IsoWeek, format: Option<&str>) -> String {
    match format {
        // If a format is provided, apply it to a date in that week
        // This allows formats like "%Y-W%V" to produce "2025-W51"
        Some(fmt) => {
            // Get a date in this week (Monday)
            let date = NaiveDate::from_isoywd_opt(week.year(), week.week(), Weekday::Mon)
                .unwrap_or_else(|| Local::now().date_naive());
            date.format(fmt).to_string()
        }
        // Default: just the week number
        None => week.week().to_string(),
    }
}

fn format_year(date: NaiveDate, format: Option<&str>) -> String {
    let fmt = format.unwrap_or("%Y");
    date.format(fmt).to_string()
}

/// Check if a string looks like a date math expression.
///
/// Returns true for strings like "today", "now + 1d", "time - 2h", "week", "year", etc.
pub fn is_date_expr(s: &str) -> bool {
    let s = s.trim().to_lowercase();
    s.starts_with("today")
        || s.starts_with("now")
        || s.starts_with("time")
        || s.starts_with("date")
        || s.starts_with("week")
        || s.starts_with("year")
}

/// Evaluate a date expression string if it is one, otherwise return None.
pub fn try_evaluate_date_expr(s: &str) -> Option<String> {
    if is_date_expr(s) {
        parse_date_expr(s).ok().map(|e| evaluate_date_expr(&e))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_today() {
        let expr = parse_date_expr("today").unwrap();
        assert_eq!(expr.base, DateBase::Today);
        assert_eq!(expr.offset, DateOffset::None);
        assert!(expr.format.is_none());
    }

    #[test]
    fn test_parse_today_plus_days() {
        let expr = parse_date_expr("today + 1d").unwrap();
        assert_eq!(expr.base, DateBase::Today);
        assert_eq!(
            expr.offset,
            DateOffset::Duration { amount: 1, unit: DurationUnit::Days }
        );
    }

    #[test]
    fn test_parse_today_minus_weeks() {
        let expr = parse_date_expr("today - 2w").unwrap();
        assert_eq!(expr.base, DateBase::Today);
        assert_eq!(
            expr.offset,
            DateOffset::Duration { amount: -2, unit: DurationUnit::Weeks }
        );
    }

    #[test]
    fn test_parse_now_with_format() {
        let expr = parse_date_expr("now | %H:%M").unwrap();
        assert_eq!(expr.base, DateBase::Now);
        assert_eq!(expr.format, Some("%H:%M".to_string()));
    }

    #[test]
    fn test_parse_weekday_previous() {
        let expr = parse_date_expr("today - monday").unwrap();
        assert_eq!(expr.base, DateBase::Today);
        assert_eq!(
            expr.offset,
            DateOffset::Weekday { weekday: Weekday::Mon, direction: Direction::Previous }
        );
    }

    #[test]
    fn test_parse_weekday_next() {
        let expr = parse_date_expr("today + friday").unwrap();
        assert_eq!(expr.base, DateBase::Today);
        assert_eq!(
            expr.offset,
            DateOffset::Weekday { weekday: Weekday::Fri, direction: Direction::Next }
        );
    }

    #[test]
    fn test_parse_months() {
        let expr = parse_date_expr("today + 3M").unwrap();
        assert_eq!(
            expr.offset,
            DateOffset::Duration { amount: 3, unit: DurationUnit::Months }
        );
    }

    #[test]
    fn test_parse_hours() {
        let expr = parse_date_expr("now + 2h").unwrap();
        assert_eq!(
            expr.offset,
            DateOffset::Duration { amount: 2, unit: DurationUnit::Hours }
        );
    }

    #[test]
    fn test_evaluate_today() {
        let expr =
            DateExpr { base: DateBase::Today, offset: DateOffset::None, format: None };
        let result = evaluate_date_expr(&expr);
        // Should be in YYYY-MM-DD format
        assert!(result.len() == 10);
        assert!(result.chars().nth(4) == Some('-'));
    }

    #[test]
    fn test_evaluate_today_plus_one_day() {
        let expr = parse_date_expr("today + 1d").unwrap();
        let result = evaluate_date_expr(&expr);

        let today = Local::now().date_naive();
        let tomorrow = today + Duration::days(1);
        assert_eq!(result, tomorrow.format("%Y-%m-%d").to_string());
    }

    #[test]
    fn test_evaluate_with_format() {
        let expr = parse_date_expr("today | %A").unwrap();
        let result = evaluate_date_expr(&expr);
        // Should be a day name like "Monday", "Tuesday", etc.
        let valid_days = [
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday",
        ];
        assert!(valid_days.contains(&result.as_str()));
    }

    #[test]
    fn test_add_months_overflow() {
        // Jan 31 + 1 month should be Feb 28 (non-leap year)
        let date = NaiveDate::from_ymd_opt(2023, 1, 31).unwrap();
        let result = add_months(date, 1);
        assert_eq!(result, NaiveDate::from_ymd_opt(2023, 2, 28).unwrap());
    }

    #[test]
    fn test_add_months_leap_year() {
        // Jan 31 + 1 month in leap year should be Feb 29
        let date = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();
        let result = add_months(date, 1);
        assert_eq!(result, NaiveDate::from_ymd_opt(2024, 2, 29).unwrap());
    }

    #[test]
    fn test_is_date_expr() {
        assert!(is_date_expr("today"));
        assert!(is_date_expr("TODAY"));
        assert!(is_date_expr("today + 1d"));
        assert!(is_date_expr("now"));
        assert!(is_date_expr("time - 2h"));
        assert!(!is_date_expr("some_var"));
        assert!(!is_date_expr("{{today}}"));
    }

    #[test]
    fn test_try_evaluate() {
        assert!(try_evaluate_date_expr("today").is_some());
        assert!(try_evaluate_date_expr("not_a_date").is_none());
    }

    #[test]
    fn test_parse_week() {
        let expr = parse_date_expr("week").unwrap();
        assert_eq!(expr.base, DateBase::Week);
        assert_eq!(expr.offset, DateOffset::None);
    }

    #[test]
    fn test_evaluate_week() {
        let expr = parse_date_expr("week").unwrap();
        let result = evaluate_date_expr(&expr);
        // Should be a number between 1 and 53
        let week_num: u32 = result.parse().unwrap();
        assert!((1..=53).contains(&week_num));
    }

    #[test]
    fn test_evaluate_week_with_format() {
        let expr = parse_date_expr("week | %Y-W%V").unwrap();
        let result = evaluate_date_expr(&expr);
        // Should be like "2025-W51"
        assert!(result.contains("-W"));
        assert!(result.len() >= 8); // "YYYY-WNN"
    }

    #[test]
    fn test_week_with_offset() {
        let expr = parse_date_expr("week + 1w").unwrap();
        let result = evaluate_date_expr(&expr);
        // Should be a valid week number
        let week_num: u32 = result.parse().unwrap();
        assert!((1..=53).contains(&week_num));
    }

    #[test]
    fn test_parse_year() {
        let expr = parse_date_expr("year").unwrap();
        assert_eq!(expr.base, DateBase::Year);
    }

    #[test]
    fn test_evaluate_year() {
        let expr = parse_date_expr("year").unwrap();
        let result = evaluate_date_expr(&expr);
        // Should be a 4-digit year
        assert_eq!(result.len(), 4);
        let year: i32 = result.parse().unwrap();
        assert!((2020..=2100).contains(&year));
    }

    #[test]
    fn test_is_date_expr_week_year() {
        assert!(is_date_expr("week"));
        assert!(is_date_expr("WEEK"));
        assert!(is_date_expr("week + 1w"));
        assert!(is_date_expr("year"));
        assert!(is_date_expr("year - 1y"));
    }
}
