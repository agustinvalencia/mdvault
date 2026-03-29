//! Links command implementation.

use std::path::Path;

use super::common::{load_config, open_index};
use super::output::{
    print_links_json, print_links_quiet, print_links_table, resolve_format, LinkOutput,
};
use crate::{LinksArgs, OutputFormat};

pub fn run(config: Option<&Path>, profile: Option<&str>, args: LinksArgs) {
    // Load configuration
    let rc = load_config(config, profile);

    // Open database
    let db = open_index(&rc.vault_root);

    // Normalize the note path (strip leading ./)
    let note_path = normalize_path(&args.note);

    // Look up the note
    let note = match db.get_note_by_path(Path::new(&note_path)) {
        Ok(Some(n)) => n,
        Ok(None) => {
            eprintln!("Note not found in index: {}", note_path);
            eprintln!("Hint: Check the path or run 'mdv reindex'.");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error looking up note: {}", e);
            std::process::exit(1);
        }
    };

    let note_id = note.id.expect("indexed note should have ID");

    // Determine what to show (both shown by default)
    let show_backlinks = args.backlinks || !args.outlinks;
    let show_outlinks = args.outlinks || !args.backlinks;

    // Resolve output format
    let format = resolve_format(args.output, args.json, args.quiet);

    // Get and display backlinks
    if show_backlinks {
        match db.get_backlinks(note_id) {
            Ok(links) => {
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
            Err(e) => {
                eprintln!("Error getting backlinks: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Get and display outgoing links
    if show_outlinks {
        match db.get_outgoing_links(note_id) {
            Ok(links) => {
                let outputs: Vec<LinkOutput> = links
                    .iter()
                    .map(|l| LinkOutput::from_link(l, Some(&note_path)))
                    .collect();

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
            Err(e) => {
                eprintln!("Error getting outgoing links: {}", e);
                std::process::exit(1);
            }
        }
    }
}

/// Normalize note path by removing leading ./.
fn normalize_path(path: &str) -> String {
    path.strip_prefix("./").unwrap_or(path).to_string()
}
