//! Validate command implementation.

use std::path::Path;

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::frontmatter::parse as parse_frontmatter;
use mdvault_core::index::IndexDb;
use mdvault_core::types::{
    apply_fixes, try_fix_note, validate_note, TypeRegistry, TypedefRepository,
    ValidationResult,
};

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

    // Check if we're validating a specific file or using the index
    let notes_to_validate = if let Some(ref path) = args.path {
        // Single file mode
        let full_path = if Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            rc.vault_root.join(path)
        };

        if !full_path.exists() {
            eprintln!("Error: File not found: {}", full_path.display());
            std::process::exit(1);
        }

        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error reading file: {}", e);
                std::process::exit(1);
            }
        };

        // Extract note type from frontmatter
        let note_type = extract_note_type(&content);

        vec![NoteInfo { path: full_path, note_type, content }]
    } else {
        // Index-based mode
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

        // Convert to NoteInfo
        let note_infos: Vec<NoteInfo> = notes
            .into_iter()
            .map(|n| {
                let full_path = rc.vault_root.join(&n.path);
                let content = std::fs::read_to_string(&full_path).unwrap_or_default();
                NoteInfo {
                    path: full_path,
                    note_type: n.note_type.as_str().to_string(),
                    content,
                }
            })
            .collect();

        note_infos
    };

    // Validate each note
    let mut total = 0;
    let mut valid_count = 0;
    let mut error_count = 0;
    let mut fixed_count = 0;
    let mut results: Vec<(
        std::path::PathBuf,
        String,
        ValidationResult,
        Option<Vec<String>>,
    )> = Vec::new();

    for note in &notes_to_validate {
        total += 1;
        let note_type = &note.note_type;

        // Skip notes without type definitions (unless we have a custom type for them)
        if !registry.has_definition(note_type) && note_type == "none" {
            valid_count += 1;
            continue;
        }

        // Parse frontmatter
        let frontmatter: serde_yaml::Value = parse_frontmatter(&note.content)
            .ok()
            .and_then(|p| p.frontmatter)
            .map(|fm| {
                let mut map = serde_yaml::Mapping::new();
                for (k, v) in fm.fields {
                    map.insert(serde_yaml::Value::String(k), v);
                }
                serde_yaml::Value::Mapping(map)
            })
            .unwrap_or(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));

        let result = validate_note(
            &registry,
            note_type,
            &note.path.to_string_lossy(),
            &frontmatter,
            &note.content,
        );

        if result.valid {
            valid_count += 1;
        } else {
            // Try to fix if --fix is set
            let fixes = if args.fix {
                let fix_result =
                    try_fix_note(&registry, note_type, &note.content, &result.errors);
                if fix_result.fixed {
                    if let Some(new_content) = fix_result.content {
                        if let Err(e) = apply_fixes(&note.path, &new_content) {
                            eprintln!(
                                "Warning: Failed to apply fixes to {}: {}",
                                note.path.display(),
                                e
                            );
                            None
                        } else {
                            fixed_count += 1;
                            Some(fix_result.fixes)
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            // Only count as error if not fully fixed
            if fixes.is_none()
                || result.errors.len() > fixes.as_ref().map_or(0, |f| f.len())
            {
                error_count += 1;
            }
            results.push((note.path.clone(), note_type.clone(), result, fixes));
        }
    }

    // Determine output format
    let format = resolve_format(args.output, args.json, args.quiet);

    // Output results
    match format {
        OutputFormat::Table => print_results_table(
            &results,
            total,
            valid_count,
            error_count,
            fixed_count,
            args.fix,
        ),
        OutputFormat::Json => {
            print_results_json(&results, total, valid_count, error_count, fixed_count)
        }
        OutputFormat::Quiet => print_results_quiet(&results),
    }

    // Exit with error code if any validation failures remain unfixed
    if error_count > 0 {
        std::process::exit(1);
    }
}

/// Information about a note to validate.
struct NoteInfo {
    path: std::path::PathBuf,
    note_type: String,
    content: String,
}

/// Extract note type from content's frontmatter.
fn extract_note_type(content: &str) -> String {
    parse_frontmatter(content)
        .ok()
        .and_then(|p| p.frontmatter)
        .and_then(|fm| fm.fields.get("type").cloned())
        .and_then(|v| match v {
            serde_yaml::Value::String(s) => Some(s),
            _ => None,
        })
        .unwrap_or_else(|| "none".to_string())
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
    results: &[(std::path::PathBuf, String, ValidationResult, Option<Vec<String>>)],
    total: usize,
    valid: usize,
    errors: usize,
    fixed: usize,
    fix_mode: bool,
) {
    if results.is_empty() {
        println!("All {} notes validated successfully.", total);
        return;
    }

    if fix_mode && fixed > 0 {
        println!(
            "Validation Results: {} valid, {} fixed, {} with errors (of {} total)",
            valid, fixed, errors, total
        );
    } else {
        println!(
            "Validation Results: {} valid, {} with errors (of {} total)",
            valid, errors, total
        );
    }
    println!();

    for (path, note_type, result, fixes) in results {
        println!("{}  [type: {}]", path.display(), note_type);

        // Show fixes if any
        if let Some(applied_fixes) = fixes {
            for fix in applied_fixes {
                println!("  + {}", fix);
            }
        }

        // Show remaining errors
        for error in &result.errors {
            // Skip errors that were fixed
            if let Some(ref applied) = fixes {
                let error_str = error.to_string();
                if applied
                    .iter()
                    .any(|f| error_str.contains(f.split('\'').nth(1).unwrap_or("")))
                {
                    continue;
                }
            }
            println!("  - {}", error);
        }

        for warning in &result.warnings {
            println!("  ~ {}", warning);
        }
        println!();
    }
}

fn print_results_json(
    results: &[(std::path::PathBuf, String, ValidationResult, Option<Vec<String>>)],
    total: usize,
    valid: usize,
    errors: usize,
    fixed: usize,
) {
    #[derive(serde::Serialize)]
    struct Output {
        total: usize,
        valid: usize,
        errors: usize,
        fixed: usize,
        results: Vec<NoteResult>,
    }

    #[derive(serde::Serialize)]
    struct NoteResult {
        path: String,
        note_type: String,
        valid: bool,
        errors: Vec<String>,
        warnings: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fixes_applied: Option<Vec<String>>,
    }

    let output = Output {
        total,
        valid,
        errors,
        fixed,
        results: results
            .iter()
            .map(|(path, note_type, result, fixes)| NoteResult {
                path: path.to_string_lossy().to_string(),
                note_type: note_type.clone(),
                valid: result.valid,
                errors: result.errors.iter().map(|e| e.to_string()).collect(),
                warnings: result.warnings.clone(),
                fixes_applied: fixes.clone(),
            })
            .collect(),
    };

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn print_results_quiet(
    results: &[(std::path::PathBuf, String, ValidationResult, Option<Vec<String>>)],
) {
    for (path, _, _, _) in results {
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
