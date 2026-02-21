//! Stale notes command implementation.

use std::path::Path;

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::{IndexDb, IndexedNote};
use serde::Serialize;

use super::output::{print_notes_json, print_notes_quiet, print_notes_table};
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

pub fn run(config: Option<&Path>, profile: Option<&str>, args: StaleArgs) {
    // Load configuration
    let rc = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    };

    // Open database
    let index_path = rc.vault_root.join(".mdvault/index.db");
    let db = match IndexDb::open(&index_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Error opening index: {}", e);
            eprintln!("Hint: Run 'mdv reindex' to build the index first.");
            std::process::exit(1);
        }
    };

    // Determine output format
    let format = resolve_format(args.output, args.json, args.quiet);

    // --orphans mode: find notes with no incoming links
    if args.orphans {
        let orphans = match db.find_orphans() {
            Ok(notes) => notes,
            Err(e) => {
                eprintln!("Error finding orphans: {}", e);
                std::process::exit(1);
            }
        };

        match format {
            OutputFormat::Table => print_notes_table(&orphans),
            OutputFormat::Json => print_notes_json(&orphans),
            OutputFormat::Quiet => print_notes_quiet(&orphans),
        }
        return;
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
        match db.get_notes_not_seen_in_days(days, note_type_str.as_deref(), args.limit) {
            Ok(notes) => notes
                .into_iter()
                .map(|(note, last_seen)| StaleNote {
                    note,
                    staleness: 1.0, // Max staleness for day-based query
                    last_seen,
                })
                .collect(),
            Err(e) => {
                eprintln!("Error querying stale notes: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        // Query by staleness threshold
        match db.get_stale_notes(args.threshold, note_type_str.as_deref(), args.limit) {
            Ok(notes) => notes
                .into_iter()
                .map(|(note, staleness)| StaleNote {
                    note,
                    staleness,
                    last_seen: None, // Not available in staleness query
                })
                .collect(),
            Err(e) => {
                eprintln!("Error querying stale notes: {}", e);
                std::process::exit(1);
            }
        }
    };

    // Output results
    match format {
        OutputFormat::Table => print_stale_table(&results),
        OutputFormat::Json => print_stale_json(&results),
        OutputFormat::Quiet => print_stale_quiet(&results),
    }
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

/// Truncate string with ellipsis if needed.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}

/// Resolve the output format from flags.
fn resolve_format(output: OutputFormat, json: bool, quiet: bool) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else if quiet {
        OutputFormat::Quiet
    } else {
        output
    }
}
