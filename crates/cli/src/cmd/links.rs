//! Links command implementation.

use std::path::Path;

use super::common::{load_config, open_index};
use super::output::{
    print_links_json, print_links_quiet, print_links_table, resolve_format, LinkOutput,
};
use crate::{LinksArgs, OutputFormat};
use color_eyre::eyre::{Result, WrapErr};

pub fn run(config: Option<&Path>, profile: Option<&str>, args: LinksArgs) -> Result<()> {
    // Load configuration
    let rc = load_config(config, profile)?;

    // Open database
    let db = open_index(&rc.vault_root)?;

    // Normalize the note path (strip leading ./)
    let note_path = normalize_path(&args.note);

    // Look up the note
    let note = db
        .get_note_by_path(Path::new(&note_path))
        .wrap_err("Error looking up note")?
        .ok_or_else(|| {
            color_eyre::eyre::eyre!(
                "Note not found in index: {}\nHint: Check the path or run 'mdv reindex'.",
                note_path
            )
        })?;

    let note_id = note.id.expect("indexed note should have ID");

    // Determine what to show (both shown by default)
    let show_backlinks = args.backlinks || !args.outlinks;
    let show_outlinks = args.outlinks || !args.backlinks;

    // Resolve output format
    let format = resolve_format(args.output, args.json, args.quiet);

    // Get and display backlinks
    if show_backlinks {
        let links = db.get_backlinks(note_id).wrap_err("Error getting backlinks")?;
        let outputs: Vec<LinkOutput> = links
            .iter()
            .map(|l| {
                // Look up source note path
                let source_path = db
                    .get_note_by_id(l.source_id)
                    .ok()
                    .flatten()
                    .map(|n| n.path.to_string_lossy().to_string());
                LinkOutput::from_link(l, source_path.as_deref())
            })
            .collect();

        if show_outlinks && !matches!(format, OutputFormat::Json) {
            println!("=== Backlinks (notes linking to {}) ===", note_path);
            println!();
        }
        match format {
            OutputFormat::Table => print_links_table(&outputs, "backlinks"),
            OutputFormat::Json => print_links_json(&outputs),
            OutputFormat::Quiet => print_links_quiet(&outputs, true),
        }
    }

    // Get and display outgoing links
    if show_outlinks {
        let links =
            db.get_outgoing_links(note_id).wrap_err("Error getting outgoing links")?;
        let outputs: Vec<LinkOutput> =
            links.iter().map(|l| LinkOutput::from_link(l, Some(&note_path))).collect();

        if show_backlinks && !matches!(format, OutputFormat::Json) {
            println!();
            println!("=== Outgoing links (notes {} links to) ===", note_path);
            println!();
        }
        match format {
            OutputFormat::Table => print_links_table(&outputs, "outgoing links"),
            OutputFormat::Json => print_links_json(&outputs),
            OutputFormat::Quiet => print_links_quiet(&outputs, false),
        }
    }

    Ok(())
}

/// Normalize note path by removing leading ./.
fn normalize_path(path: &str) -> String {
    path.strip_prefix("./").unwrap_or(path).to_string()
}
