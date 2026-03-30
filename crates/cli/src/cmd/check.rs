//! Vault lint check command implementation.

use std::path::Path;

use super::common::{load_config, open_index};
use color_eyre::eyre::{Result, bail};
use mdvault_core::lint::{CategoryReport, LintReport, run_lint};
use mdvault_core::types::{TypeRegistry, TypedefRepository};

use crate::CheckArgs;

pub fn run(config: Option<&Path>, profile: Option<&str>, args: CheckArgs) -> Result<()> {
    // Load configuration
    let rc = load_config(config, profile)?;
    let db = open_index(&rc.vault_root)?;

    // Load type registry
    let typedef_repo = match &rc.typedefs_fallback_dir {
        Some(fallback) => TypedefRepository::with_fallback(&rc.typedefs_dir, fallback),
        None => TypedefRepository::new(&rc.typedefs_dir),
    };
    let typedef_repo = typedef_repo
        .map_err(|e| color_eyre::eyre::eyre!("Error loading type definitions: {e}"))?;
    let registry = TypeRegistry::from_repository(&typedef_repo)
        .map_err(|e| color_eyre::eyre::eyre!("Error building type registry: {e}"))?;

    // Run lint
    let report = run_lint(
        &db,
        &registry,
        &rc.vault_root,
        args.category.as_deref(),
        args.no_reindex,
    );

    // Output
    if args.json {
        print_json(&report);
    } else if args.quiet {
        print_quiet(&report);
    } else {
        print_table(&report);
    }

    // Exit code: 1 if errors found
    if report.has_errors() {
        bail!("Vault check found errors");
    }

    Ok(())
}

fn print_table(report: &LintReport) {
    let s = &report.summary;

    // Health score banner
    let score_pct = (s.health_score * 100.0).round() as u32;
    println!("Vault Health: {}% ({} notes)", score_pct, s.total_notes);
    println!();

    // Clean categories first
    let clean: Vec<&CategoryReport> =
        report.categories.iter().filter(|c| c.is_clean()).collect();
    if !clean.is_empty() {
        let names: Vec<&str> = clean.iter().map(|c| c.label.as_str()).collect();
        println!("Clean: {}", names.join(", "));
        println!();
    }

    // Categories with issues
    for cat in &report.categories {
        if cat.is_clean() {
            continue;
        }

        println!(
            "{} ({} error(s), {} warning(s))",
            cat.label,
            cat.errors.len(),
            cat.warnings.len()
        );

        for issue in &cat.errors {
            let loc = format_location(&issue.path, issue.line);
            print!("  ERROR {}: {}", loc, issue.message);
            if let Some(ref sug) = issue.suggestion {
                print!(" — {sug}");
            }
            println!();
        }

        for issue in &cat.warnings {
            let loc = format_location(&issue.path, issue.line);
            print!("  WARN  {}: {}", loc, issue.message);
            if let Some(ref sug) = issue.suggestion {
                print!(" — {sug}");
            }
            println!();
        }

        println!();
    }

    // Summary line
    if s.total_errors == 0 && s.total_warnings == 0 {
        println!("No issues found.");
    } else {
        println!("Total: {} error(s), {} warning(s)", s.total_errors, s.total_warnings);
    }

    if s.reindex_performed {
        println!("(index was updated during check)");
    }
}

fn print_json(report: &LintReport) {
    println!(
        "{}",
        serde_json::to_string_pretty(report)
            .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}")),
    );
}

fn print_quiet(report: &LintReport) {
    for cat in &report.categories {
        for issue in cat.errors.iter().chain(cat.warnings.iter()) {
            if !issue.path.is_empty() {
                println!("{}", issue.path);
            }
        }
    }
}

fn format_location(path: &str, line: Option<u32>) -> String {
    if path.is_empty() {
        return "(vault)".to_string();
    }
    match line {
        Some(l) => format!("{path}:{l}"),
        None => path.to_string(),
    }
}
