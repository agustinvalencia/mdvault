//! Shared output formatting for query commands.

use mdvault_core::index::{IndexedLink, IndexedNote};
use serde::Serialize;

/// Formatted note for JSON output.
#[derive(Debug, Serialize)]
pub struct NoteOutput {
    pub path: String,
    #[serde(rename = "type")]
    pub note_type: String,
    pub title: String,
    pub modified: String,
}

impl From<&IndexedNote> for NoteOutput {
    fn from(note: &IndexedNote) -> Self {
        Self {
            path: note.path.to_string_lossy().to_string(),
            note_type: note.note_type.as_str().to_string(),
            title: note.title.clone(),
            modified: note.modified.format("%Y-%m-%d %H:%M").to_string(),
        }
    }
}

/// Formatted link for JSON output.
#[derive(Debug, Serialize)]
pub struct LinkOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    pub target_path: String,
    pub link_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u32>,
}

impl LinkOutput {
    pub fn from_link(link: &IndexedLink, source_path: Option<&str>) -> Self {
        Self {
            source_path: source_path.map(|s| s.to_string()),
            target_path: link.target_path.clone(),
            link_type: link.link_type.as_str().to_string(),
            link_text: link.link_text.clone(),
            line_number: link.line_number,
        }
    }
}

/// Print notes as a table.
pub fn print_notes_table(notes: &[IndexedNote]) {
    if notes.is_empty() {
        println!("(no notes found)");
        return;
    }

    // Calculate column widths
    let path_width = notes
        .iter()
        .map(|n| n.path.to_string_lossy().len())
        .max()
        .unwrap_or(4)
        .clamp(4, 50);
    let type_width = 8; // "project" is longest
    let title_width = notes.iter().map(|n| n.title.len()).max().unwrap_or(5).clamp(5, 40);

    // Header
    println!(
        "{:<path_width$}  {:<type_width$}  {:<title_width$}  MODIFIED",
        "PATH",
        "TYPE",
        "TITLE",
        path_width = path_width,
        type_width = type_width,
        title_width = title_width,
    );
    println!(
        "{:-<path_width$}  {:-<type_width$}  {:-<title_width$}  {:-<16}",
        "",
        "",
        "",
        "",
        path_width = path_width,
        type_width = type_width,
        title_width = title_width,
    );

    // Rows
    for note in notes {
        let path = truncate(&note.path.to_string_lossy(), path_width);
        let title = truncate(&note.title, title_width);
        let modified = note.modified.format("%Y-%m-%d %H:%M").to_string();

        println!(
            "{:<path_width$}  {:<type_width$}  {:<title_width$}  {}",
            path,
            note.note_type.as_str(),
            title,
            modified,
            path_width = path_width,
            type_width = type_width,
            title_width = title_width,
        );
    }

    println!();
    println!("-- {} notes --", notes.len());
}

/// Print notes as JSON.
pub fn print_notes_json(notes: &[IndexedNote]) {
    let output: Vec<NoteOutput> = notes.iter().map(NoteOutput::from).collect();
    println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
}

/// Print notes as paths only (quiet mode).
pub fn print_notes_quiet(notes: &[IndexedNote]) {
    for note in notes {
        println!("{}", note.path.display());
    }
}

/// Print links as a table.
pub fn print_links_table(links: &[LinkOutput], direction: &str) {
    if links.is_empty() {
        println!("(no {} found)", direction);
        return;
    }

    let path_width = links
        .iter()
        .map(|l| {
            l.target_path.len().max(l.source_path.as_ref().map(|s| s.len()).unwrap_or(0))
        })
        .max()
        .unwrap_or(4)
        .clamp(4, 50);
    let type_width = 10;

    println!(
        "{:<path_width$}  {:<type_width$}  LINE",
        "PATH",
        "LINK_TYPE",
        path_width = path_width,
        type_width = type_width
    );
    println!(
        "{:-<path_width$}  {:-<type_width$}  {:-<6}",
        "",
        "",
        "",
        path_width = path_width,
        type_width = type_width
    );

    for link in links {
        let path = if direction == "backlinks" {
            link.source_path.as_deref().unwrap_or(&link.target_path)
        } else {
            &link.target_path
        };
        let path = truncate(path, path_width);
        let line =
            link.line_number.map(|n| n.to_string()).unwrap_or_else(|| "-".to_string());

        println!(
            "{:<path_width$}  {:<type_width$}  {}",
            path,
            link.link_type,
            line,
            path_width = path_width,
            type_width = type_width,
        );
    }

    println!();
    println!("-- {} {} --", links.len(), direction);
}

/// Print links as JSON.
pub fn print_links_json(links: &[LinkOutput]) {
    println!("{}", serde_json::to_string_pretty(&links).unwrap_or_default());
}

/// Print links as paths only (quiet mode).
pub fn print_links_quiet(links: &[LinkOutput], use_source: bool) {
    for link in links {
        if use_source {
            if let Some(ref source) = link.source_path {
                println!("{}", source);
            }
        } else {
            println!("{}", link.target_path);
        }
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
