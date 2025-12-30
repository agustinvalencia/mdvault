//! Validate command implementation.

use std::path::Path;

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::IndexDb;
use mdvault_core::types::{validate_note, TypeRegistry, TypedefRepository};

use crate::{OutputFormat, ValidateArgs};

pub fn run(config: Option<&Path>, profile: Option<&str>, args: ValidateArgs) {
    // Load configuration
    let rc = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    };

    // Load type definitions
    let typedef_repo = match TypedefRepository::new(&rc.typedefs_dir) {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("Error loading type definitions: {}", e);
            std::process::exit(1);
        }
    };

    let registry = match TypeRegistry::from_repository(&typedef_repo) {
        Ok(reg) => reg,
        Err(e) => {
            eprintln!("Error building type registry: {}", e);
            std::process::exit(1);
        }
    };

    // If --list-types, just show available types
    if args.list_types {
        print_types(&registry);
        return;
    }

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

    // Query notes to validate
    let query = mdvault_core::index::NoteQuery {
        note_type: args.r#type.as_ref().map(|s| s.parse().unwrap_or_default()),
        path_prefix: None,
        modified_after: None,
        modified_before: None,
        limit: args.limit,
        offset: None,
    };

    let notes = match db.query_notes(&query) {
        Ok(notes) => notes,
        Err(e) => {
            eprintln!("Error querying notes: {}", e);
            std::process::exit(1);
        }
    };

    // Validate each note
    let mut total = 0;
    let mut valid_count = 0;
    let mut error_count = 0;
    let mut results = Vec::new();

    for note in &notes {
        total += 1;
        let note_type = note.note_type.as_str();

        // Skip notes without type definitions (unless we have a custom type for them)
        if !registry.has_definition(note_type) && note_type == "none" {
            valid_count += 1;
            continue;
        }

        // Parse frontmatter from JSON
        let frontmatter: serde_yaml::Value = note
            .frontmatter_json
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));

        // Read note content for custom validators
        let content =
            std::fs::read_to_string(rc.vault_root.join(&note.path)).unwrap_or_default();

        let result = validate_note(
            &registry,
            note_type,
            &note.path.to_string_lossy(),
            &frontmatter,
            &content,
        );

        if result.valid {
            valid_count += 1;
        } else {
            error_count += 1;
            results.push((note.path.clone(), note_type.to_string(), result));
        }
    }

    // Determine output format
    let format = resolve_format(args.output, args.json, args.quiet);

    // Output results
    match format {
        OutputFormat::Table => {
            print_results_table(&results, total, valid_count, error_count)
        }
        OutputFormat::Json => {
            print_results_json(&results, total, valid_count, error_count)
        }
        OutputFormat::Quiet => print_results_quiet(&results),
    }

    // Exit with error code if any validation failures
    if error_count > 0 {
        std::process::exit(1);
    }
}

fn print_types(registry: &TypeRegistry) {
    println!("Available note types:");
    println!();

    // Built-in types
    println!("Built-in types:");
    for name in ["daily", "weekly", "task", "project", "zettel"] {
        let has_override = registry.has_definition(name);
        if has_override {
            println!("  {} (with Lua override)", name);
        } else {
            println!("  {}", name);
        }
    }

    // Custom types
    let custom = registry.list_custom_types();
    if !custom.is_empty() {
        println!();
        println!("Custom types:");
        for name in custom {
            if let Some(td) = registry.get(name) {
                if let Some(desc) = &td.description {
                    println!("  {} - {}", name, desc);
                } else {
                    println!("  {}", name);
                }
            }
        }
    }
}

fn print_results_table(
    results: &[(std::path::PathBuf, String, mdvault_core::types::ValidationResult)],
    total: usize,
    valid: usize,
    errors: usize,
) {
    if results.is_empty() {
        println!("All {} notes validated successfully.", total);
        return;
    }

    println!(
        "Validation Results: {} valid, {} with errors (of {} total)",
        valid, errors, total
    );
    println!();

    for (path, note_type, result) in results {
        println!("{}  [type: {}]", path.display(), note_type);
        for error in &result.errors {
            println!("  - {}", error);
        }
        for warning in &result.warnings {
            println!("  ~ {}", warning);
        }
        println!();
    }
}

fn print_results_json(
    results: &[(std::path::PathBuf, String, mdvault_core::types::ValidationResult)],
    total: usize,
    valid: usize,
    errors: usize,
) {
    #[derive(serde::Serialize)]
    struct Output {
        total: usize,
        valid: usize,
        errors: usize,
        results: Vec<NoteResult>,
    }

    #[derive(serde::Serialize)]
    struct NoteResult {
        path: String,
        note_type: String,
        valid: bool,
        errors: Vec<String>,
        warnings: Vec<String>,
    }

    let output = Output {
        total,
        valid,
        errors,
        results: results
            .iter()
            .map(|(path, note_type, result)| NoteResult {
                path: path.to_string_lossy().to_string(),
                note_type: note_type.clone(),
                valid: result.valid,
                errors: result.errors.iter().map(|e| e.to_string()).collect(),
                warnings: result.warnings.clone(),
            })
            .collect(),
    };

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn print_results_quiet(
    results: &[(std::path::PathBuf, String, mdvault_core::types::ValidationResult)],
) {
    for (path, _, _) in results {
        println!("{}", path.display());
    }
}

fn resolve_format(output: OutputFormat, json: bool, quiet: bool) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else if quiet {
        OutputFormat::Quiet
    } else {
        output
    }
}
