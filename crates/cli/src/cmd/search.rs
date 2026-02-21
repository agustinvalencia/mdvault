//! Search command implementation.

use std::path::Path;

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::{
    IndexDb, MatchSource, SearchEngine, SearchMode, SearchQuery, SearchResult,
};
use serde::Serialize;

use super::output::truncate;
use crate::{OutputFormat, SearchArgs, SearchModeArg};

/// Search result for JSON output.
#[derive(Debug, Serialize)]
struct SearchResultOutput {
    path: String,
    #[serde(rename = "type")]
    note_type: String,
    title: String,
    score: f64,
    match_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    staleness: Option<f64>,
}

impl From<&SearchResult> for SearchResultOutput {
    fn from(result: &SearchResult) -> Self {
        Self {
            path: result.note.path.to_string_lossy().to_string(),
            note_type: result.note.note_type.as_str().to_string(),
            title: result.note.title.clone(),
            score: result.score,
            match_source: format_match_source(&result.match_source),
            staleness: result.staleness,
        }
    }
}

fn format_match_source(source: &MatchSource) -> String {
    match source {
        MatchSource::Direct => "direct".to_string(),
        MatchSource::Linked { hops } => format!("linked({})", hops),
        MatchSource::Temporal { daily_path } => format!("temporal({})", daily_path),
        MatchSource::Cooccurrence { shared_dailies } => {
            format!("cooccur({})", shared_dailies)
        }
    }
}

pub fn run(config: Option<&Path>, profile: Option<&str>, args: SearchArgs) {
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

    // Convert search mode
    let mode = match args.mode {
        SearchModeArg::Direct => SearchMode::Direct,
        SearchModeArg::Neighbourhood => SearchMode::Neighbourhood { hops: 2 },
        SearchModeArg::Temporal => SearchMode::Temporal { days: 30 },
        SearchModeArg::Cooccurrence => SearchMode::Cooccurrence { min_shared: 2 },
        SearchModeArg::Full => SearchMode::Full,
    };

    // Build search query
    let query = SearchQuery {
        text: args.query,
        note_type: args.r#type.map(|t| t.into()),
        path_prefix: None,
        mode,
        limit: args.limit,
        temporal_boost: args.boost,
    };

    // Execute search
    let engine = SearchEngine::new(&db);
    let results = match engine.search(&query) {
        Ok(results) => results,
        Err(e) => {
            eprintln!("Error searching: {}", e);
            std::process::exit(1);
        }
    };

    // Determine output format
    let format = resolve_format(args.output, args.json, args.quiet);

    // Output results
    match format {
        OutputFormat::Table => print_results_table(&results),
        OutputFormat::Json => print_results_json(&results),
        OutputFormat::Quiet => print_results_quiet(&results),
    }
}

/// Print search results as a table.
fn print_results_table(results: &[SearchResult]) {
    if results.is_empty() {
        println!("(no results found)");
        return;
    }

    // Calculate column widths
    let path_width = results
        .iter()
        .map(|r| r.note.path.to_string_lossy().len())
        .max()
        .unwrap_or(4)
        .clamp(4, 40);
    let title_width =
        results.iter().map(|r| r.note.title.len()).max().unwrap_or(5).clamp(5, 30);
    let source_width = 15;

    // Header
    println!(
        "{:<path_width$}  {:<title_width$}  SCORE  {:<source_width$}",
        "PATH",
        "TITLE",
        "SOURCE",
        path_width = path_width,
        title_width = title_width,
        source_width = source_width,
    );
    println!(
        "{:-<path_width$}  {:-<title_width$}  {:-<5}  {:-<source_width$}",
        "",
        "",
        "",
        "",
        path_width = path_width,
        title_width = title_width,
        source_width = source_width,
    );

    // Rows
    for result in results {
        let path = truncate(&result.note.path.to_string_lossy(), path_width);
        let title = truncate(&result.note.title, title_width);
        let source = format_match_source(&result.match_source);
        let source = truncate(&source, source_width);

        println!(
            "{:<path_width$}  {:<title_width$}  {:5.2}  {:<source_width$}",
            path,
            title,
            result.score,
            source,
            path_width = path_width,
            title_width = title_width,
            source_width = source_width,
        );
    }

    println!();
    println!("-- {} results --", results.len());
}

/// Print search results as JSON.
fn print_results_json(results: &[SearchResult]) {
    let output: Vec<SearchResultOutput> =
        results.iter().map(SearchResultOutput::from).collect();
    println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
}

/// Print search results as paths only.
fn print_results_quiet(results: &[SearchResult]) {
    for result in results {
        println!("{}", result.note.path.display());
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
