//! Stale notes command implementation.

use std::path::Path;

use color_eyre::eyre::{Result, WrapErr};
use mdvault_core::index::IndexedNote;
use serde::Serialize;

use super::common::{load_config, open_index};
use super::output::{
    print_notes_json, print_notes_quiet, print_notes_table, resolve_format, truncate,
};
use crate::{OutputFormat, StaleArgs};

/// Stale note output for JSON.
#[derive(Debug, Serialize)]
struct StaleNoteOutput {
    path: String,
    #[serde(rename = "type")]
    note_type: String,
    title: String,
    staleness: f64,
    last_seen: Option<String>,
}

pub fn run(config: Option<&Path>, profile: Option<&str>, args: StaleArgs) -> Result<()> {
    // Load configuration
    let rc = load_config(config, profile)?;

    // Open database
    let db = open_index(&rc.vault_root)?;

    // Determine output format
    let format = resolve_format(args.output, args.json, args.quiet);

    // --orphans mode: find notes with no incoming links
    if args.orphans {
        let orphans = db.find_orphans().wrap_err("Error finding orphans")?;

        match format {
            OutputFormat::Table => print_notes_table(&orphans),
            OutputFormat::Json => print_notes_json(&orphans),
            OutputFormat::Quiet => print_notes_quiet(&orphans),
        }
        return Ok(());
    }

    // Get note type filter
    let note_type_str = args.r#type.map(|t| {
        use mdvault_core::index::NoteType;
        let nt: NoteType = t.into();
        nt.as_str().to_string()
    });

    // Query stale notes
    let results: Vec<StaleNote> = if let Some(days) = args.days {
        // Query by days not seen
        db.get_notes_not_seen_in_days(days, note_type_str.as_deref(), args.limit)
            .wrap_err("Error querying stale notes")?
            .into_iter()
            .map(|(note, last_seen)| StaleNote {
                note,
                staleness: 1.0, // Max staleness for day-based query
                last_seen,
            })
            .collect()
    } else {
        // Query by staleness threshold
        db.get_stale_notes(args.threshold, note_type_str.as_deref(), args.limit)
            .wrap_err("Error querying stale notes")?
            .into_iter()
            .map(|(note, staleness)| StaleNote {
                note,
                staleness,
                last_seen: None, // Not available in staleness query
            })
            .collect()
    };

    // Output results
    match format {
        OutputFormat::Table => print_stale_table(&results),
        OutputFormat::Json => print_stale_json(&results),
        OutputFormat::Quiet => print_stale_quiet(&results),
    }

    Ok(())
}

/// Internal stale note representation.
struct StaleNote {
    note: IndexedNote,
    staleness: f64,
    last_seen: Option<String>,
}

/// Print stale notes as a table.
fn print_stale_table(notes: &[StaleNote]) {
    if notes.is_empty() {
        println!("(no stale notes found)");
        return;
    }

    // Calculate column widths
    let path_width = notes
        .iter()
        .map(|n| n.note.path.to_string_lossy().len())
        .max()
        .unwrap_or(4)
        .clamp(4, 45);
    let title_width =
        notes.iter().map(|n| n.note.title.len()).max().unwrap_or(5).clamp(5, 30);

    // Header
    println!(
        "{:<path_width$}  {:<title_width$}  STALENESS  LAST_SEEN",
        "PATH",
        "TITLE",
        path_width = path_width,
        title_width = title_width,
    );
    println!(
        "{:-<path_width$}  {:-<title_width$}  {:-<9}  {:-<10}",
        "",
        "",
        "",
        "",
        path_width = path_width,
        title_width = title_width,
    );

    // Rows
    for stale in notes {
        let path = truncate(&stale.note.path.to_string_lossy(), path_width);
        let title = truncate(&stale.note.title, title_width);
        let last_seen = stale.last_seen.as_deref().unwrap_or("-");

        println!(
            "{:<path_width$}  {:<title_width$}  {:9.2}  {}",
            path,
            title,
            stale.staleness,
            last_seen,
            path_width = path_width,
            title_width = title_width,
        );
    }

    println!();
    println!("-- {} stale notes --", notes.len());
}

/// Print stale notes as JSON.
fn print_stale_json(notes: &[StaleNote]) {
    let output: Vec<StaleNoteOutput> = notes
        .iter()
        .map(|stale| StaleNoteOutput {
            path: stale.note.path.to_string_lossy().to_string(),
            note_type: stale.note.note_type.as_str().to_string(),
            title: stale.note.title.clone(),
            staleness: stale.staleness,
            last_seen: stale.last_seen.clone(),
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
}

/// Print stale notes as paths only.
fn print_stale_quiet(notes: &[StaleNote]) {
    for stale in notes {
        println!("{}", stale.note.path.display());
    }
}
