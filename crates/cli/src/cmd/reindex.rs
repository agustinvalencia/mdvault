//! Reindex command implementation.

use std::io::Write;
use std::path::Path;

use super::common::{load_config, open_index};
use color_eyre::eyre::{Result, WrapErr};
use mdvault_core::index::{DerivedIndexBuilder, IndexBuilder};

/// Run the reindex command.
pub fn run(
    config: Option<&Path>,
    profile: Option<&str>,
    verbose: bool,
    force: bool,
) -> Result<()> {
    // Load configuration
    let rc = load_config(config, profile)?;

    // Determine index path
    let index_dir = rc.vault_root.join(".mdvault");
    let index_path = index_dir.join("index.db");

    // Ensure .mdvault directory exists
    std::fs::create_dir_all(&index_dir).wrap_err("Error creating index directory")?;

    // Open database
    let db = open_index(&rc.vault_root)?;

    let mode = if force { "full" } else { "incremental" };
    println!("Indexing vault ({} mode): {}", mode, rc.vault_root.display());

    // Create progress callback
    let progress: Option<mdvault_core::index::ProgressCallback> = if verbose {
        Some(Box::new(|current, total, path| {
            println!("[{}/{}] {}", current, total, path);
        }))
    } else {
        Some(Box::new(|current, total, _path| {
            // Simple progress indicator
            if current % 50 == 0 || current == total {
                print!("\rScanning... {}/{}", current, total);
                std::io::stdout().flush().ok();
            }
        }))
    };

    // Build index with exclusions
    let builder =
        IndexBuilder::with_exclusions(&db, &rc.vault_root, rc.excluded_folders.clone());
    let result = if force {
        builder.full_reindex(progress)
    } else {
        builder.incremental_reindex(progress)
    };

    let stats = result.wrap_err("Error during indexing")?;

    if !verbose {
        println!(); // Newline after progress
    }
    println!();
    println!("Indexing complete:");
    println!("  Files found:    {}", stats.files_found);

    if force {
        // Full reindex stats
        println!("  Notes indexed:  {}", stats.notes_indexed);
    } else {
        // Incremental stats
        println!("  Unchanged:      {}", stats.files_unchanged);
        println!("  Added:          {}", stats.files_added);
        println!("  Updated:        {}", stats.files_updated);
        println!("  Deleted:        {}", stats.files_deleted);
    }

    if stats.notes_skipped > 0 {
        println!("  Skipped:        {}", stats.notes_skipped);
    }
    println!("  Links indexed:  {}", stats.links_indexed);
    println!("  Broken links:   {}", stats.broken_links);
    println!("  Duration:       {}ms", stats.duration_ms);

    // Compute derived indices
    if verbose {
        println!();
        println!("Computing derived indices...");
    }
    let derived_builder = DerivedIndexBuilder::new(&db);
    match derived_builder.compute_all() {
        Ok(derived_stats) => {
            println!();
            println!("Derived indices:");
            println!("  Dailies processed:    {}", derived_stats.dailies_processed);
            println!("  Activity records:     {}", derived_stats.activity_records);
            println!("  Activity summaries:   {}", derived_stats.summaries_computed);
            println!("  Cooccurrence pairs:   {}", derived_stats.cooccurrence_pairs);
            println!("  Duration:             {}ms", derived_stats.duration_ms);
        }
        Err(e) => {
            eprintln!("Warning: Failed to compute derived indices: {}", e);
        }
    }

    println!();
    println!("Index stored at: {}", index_path.display());

    Ok(())
}
