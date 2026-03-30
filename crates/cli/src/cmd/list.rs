//! List command implementation.

use std::path::Path;

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use color_eyre::eyre::{Result, WrapErr};
use mdvault_core::index::NoteQuery;
use mdvault_core::vars::try_evaluate_date_expr;

use super::common::{load_config, open_index};
use super::output::{
    print_notes_json, print_notes_quiet, print_notes_table, resolve_format,
};
use crate::{ListArgs, OutputFormat};

pub fn run(config: Option<&Path>, profile: Option<&str>, args: ListArgs) -> Result<()> {
    let rc = load_config(config, profile)?;
    let db = open_index(&rc.vault_root)?;

    // Build query
    let query = NoteQuery {
        note_type: args.r#type.map(|t| t.into()),
        path_prefix: None,
        modified_after: parse_date_arg(&args.modified_after, "modified-after"),
        modified_before: parse_date_arg(&args.modified_before, "modified-before"),
        limit: args.limit,
        offset: None,
    };

    // Execute query
    let notes = db.query_notes(&query).wrap_err("Error querying notes")?;

    // Determine output format
    let format = resolve_format(args.output, args.json, args.quiet);

    // Output results
    match format {
        OutputFormat::Table => print_notes_table(&notes),
        OutputFormat::Json => print_notes_json(&notes),
        OutputFormat::Quiet => print_notes_quiet(&notes),
    }

    Ok(())
}

/// Parse a date argument, supporting both YYYY-MM-DD and date math expressions.
fn parse_date_arg(arg: &Option<String>, name: &str) -> Option<DateTime<Utc>> {
    let s = arg.as_ref()?;

    // Try date math expression first (e.g., "today - 7d")
    if let Some(result) = try_evaluate_date_expr(s)
        && let Ok(date) = NaiveDate::parse_from_str(&result, "%Y-%m-%d")
    {
        let datetime = date.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        return Some(DateTime::from_naive_utc_and_offset(datetime, Utc));
    }

    // Try ISO date (YYYY-MM-DD)
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let datetime = date.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        return Some(DateTime::from_naive_utc_and_offset(datetime, Utc));
    }

    // Try ISO datetime (YYYY-MM-DDTHH:MM:SS)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    eprintln!(
        "Warning: Could not parse --{} '{}'. Expected YYYY-MM-DD or date expression.",
        name, s
    );
    None
}
