//! Reindex command implementation.

use std::io::Write;
use std::path::Path;

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::{IndexBuilder, IndexDb};

/// Run the reindex command.
pub fn run(config: Option<&Path>, profile: Option<&str>, verbose: bool) {
    // Load configuration
    let rc = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    };

    // Determine index path
    let index_dir = rc.vault_root.join(".mdvault");
    let index_path = index_dir.join("index.db");

    // Ensure .mdvault directory exists
    if let Err(e) = std::fs::create_dir_all(&index_dir) {
        eprintln!("Error creating index directory: {}", e);
        std::process::exit(1);
    }

    // Open database
    let db = match IndexDb::open(&index_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Error opening index database: {}", e);
            std::process::exit(1);
        }
    };

    println!("Indexing vault: {}", rc.vault_root.display());

    // Create progress callback
    let progress: Option<mdvault_core::index::ProgressCallback> = if verbose {
        Some(Box::new(|current, total, path| {
            println!("[{}/{}] {}", current, total, path);
        }))
    } else {
        Some(Box::new(|current, total, _path| {
            // Simple progress indicator
            if current % 50 == 0 || current == total {
                print!("\rIndexing... {}/{}", current, total);
                std::io::stdout().flush().ok();
            }
        }))
    };

    // Build index
    let builder = IndexBuilder::new(&db, &rc.vault_root);
    match builder.full_reindex(progress) {
        Ok(stats) => {
            if !verbose {
                println!(); // Newline after progress
            }
            println!();
            println!("Indexing complete:");
            println!("  Files found:    {}", stats.files_found);
            println!("  Notes indexed:  {}", stats.notes_indexed);
            if stats.notes_skipped > 0 {
                println!("  Notes skipped:  {}", stats.notes_skipped);
            }
            println!("  Links indexed:  {}", stats.links_indexed);
            println!("  Broken links:   {}", stats.broken_links);
            println!("  Duration:       {}ms", stats.duration_ms);
            println!();
            println!("Index stored at: {}", index_path.display());
        }
        Err(e) => {
            eprintln!("\nError during indexing: {}", e);
            std::process::exit(1);
        }
    }
}
