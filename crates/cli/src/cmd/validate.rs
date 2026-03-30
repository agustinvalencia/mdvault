//! Validate command implementation.

use std::path::Path;

use color_eyre::eyre::{Result, WrapErr, bail};
use mdvault_core::frontmatter::parse as parse_frontmatter;
use mdvault_core::index::IndexDb;
use mdvault_core::paths::PathResolver;
use mdvault_core::types::{
    TypeRegistry, TypedefRepository, ValidationResult, add_link_integrity_warnings,
    apply_fixes, try_fix_note, validate_note,
};

use super::common::load_config;
use super::output::resolve_format;
use crate::{OutputFormat, ValidateArgs};

pub fn run(
    config: Option<&Path>,
    profile: Option<&str>,
    args: ValidateArgs,
) -> Result<()> {
    // Load configuration
    let rc = load_config(config, profile)?;

    // Load type definitions (with fallback to default dir)
    let typedef_repo = match &rc.typedefs_fallback_dir {
        Some(fallback) => TypedefRepository::with_fallback(&rc.typedefs_dir, fallback),
        None => TypedefRepository::new(&rc.typedefs_dir),
    };
    let typedef_repo = typedef_repo
        .map_err(|e| color_eyre::eyre::eyre!("Error loading type definitions: {e}"))?;

    let registry = TypeRegistry::from_repository(&typedef_repo)
        .map_err(|e| color_eyre::eyre::eyre!("Error building type registry: {e}"))?;

    // If --list-types, just show available types
    if args.list_types {
        print_types(&registry);
        return Ok(());
    }

    // Open index database if needed (for querying notes or link checking)
    let index_path = PathResolver::new(&rc.vault_root).index_db();
    let index_db: Option<IndexDb> = if args.path.is_none() || args.check_links {
        match IndexDb::open(&index_path) {
            Ok(db) => Some(db),
            Err(e) => {
                if args.path.is_none() {
                    // Index is required for index-based mode
                    eprintln!("Hint: Run 'mdv reindex' to build the index first.");
                    return Err(e).wrap_err("Error opening index");
                } else if args.check_links {
                    // Index is optional for single-file mode with link checking
                    eprintln!(
                        "Warning: Cannot check links - index not available. Run 'mdv reindex' first."
                    );
                    None
                } else {
                    None
                }
            }
        }
    } else {
        None
    };

    // Check if we're validating a specific file or using the index
    let notes_to_validate = if let Some(ref path) = args.path {
        // Single file mode
        let full_path = if Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            rc.vault_root.join(path)
        };

        if !full_path.exists() {
            bail!("File not found: {}", full_path.display());
        }

        let content =
            std::fs::read_to_string(&full_path).wrap_err("Error reading file")?;

        // Extract note type from frontmatter
        let note_type = extract_note_type(&content);

        // Compute relative path for link checking
        let relative_path = full_path
            .strip_prefix(&rc.vault_root)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| full_path.clone());

        vec![NoteInfo { path: full_path, relative_path, note_type, content }]
    } else {
        // Index-based mode - index_db is guaranteed to be Some here
        let db = index_db.as_ref().unwrap();

        // Query notes to validate
        let query = mdvault_core::index::NoteQuery {
            note_type: args.r#type.as_ref().map(|s| s.parse().unwrap_or_default()),
            path_prefix: None,
            modified_after: None,
            modified_before: None,
            limit: args.limit,
            offset: None,
        };

        let notes = db.query_notes(&query).wrap_err("Error querying notes")?;

        // Convert to NoteInfo
        let note_infos: Vec<NoteInfo> = notes
            .into_iter()
            .map(|n| {
                let full_path = rc.vault_root.join(&n.path);
                let content = std::fs::read_to_string(&full_path).unwrap_or_default();
                NoteInfo {
                    path: full_path,
                    relative_path: n.path,
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

        // Run type-based validation (skip for untyped notes without custom definitions)
        let mut result = if !registry.has_definition(note_type) && note_type == "none" {
            ValidationResult::default()
        } else {
            validate_note(
                &registry,
                note_type,
                &note.path.to_string_lossy(),
                &frontmatter,
                &note.content,
            )
        };

        // Check link integrity if requested and index is available
        if args.check_links
            && let Some(ref db) = index_db
        {
            add_link_integrity_warnings(&mut result, db, &note.relative_path);
        }

        // Determine if note is valid (errors only, warnings don't count)
        let has_errors = !result.errors.is_empty();
        let has_warnings = !result.warnings.is_empty();

        if !has_errors && !has_warnings {
            valid_count += 1;
        } else if !has_errors {
            // Only warnings, still valid but add to results for display
            valid_count += 1;
            results.push((note.path.clone(), note_type.clone(), result, None));
        } else {
            // Has errors - try to fix if --fix is set
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
        bail!("{} note(s) failed validation", error_count);
    }
    Ok(())
}

/// Information about a note to validate.
struct NoteInfo {
    path: std::path::PathBuf,
    relative_path: std::path::PathBuf,
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
            if let Some(applied) = &fixes {
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
