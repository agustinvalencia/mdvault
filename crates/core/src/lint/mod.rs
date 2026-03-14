//! Vault lint — structural correctness checker for the markdown vault.

pub mod checks;
pub mod result;

use std::path::Path;

use crate::index::IndexDb;
use crate::types::TypeRegistry;

pub use result::{CategoryReport, LintIssue, LintReport, LintSummary};

/// Which lint categories to run.
const ALL_CATEGORIES: &[&str] = &[
    "broken_references",
    "malformed_wikilinks",
    "schema_violations",
    "structural_consistency",
    "orphaned_notes",
    "db_sync",
];

/// Run lint checks on the vault and produce a report.
pub fn run_lint(
    db: &IndexDb,
    registry: &TypeRegistry,
    vault_root: &Path,
    category_filter: Option<&str>,
    skip_reindex: bool,
) -> LintReport {
    let categories_to_run: Vec<&str> = match category_filter {
        Some(cat) => {
            if ALL_CATEGORIES.contains(&cat) {
                vec![cat]
            } else {
                // Unknown category — run nothing, report will be empty
                vec![]
            }
        }
        None => ALL_CATEGORIES.to_vec(),
    };

    let mut categories = Vec::new();
    let mut reindex_performed = false;

    for cat in &categories_to_run {
        let report = match *cat {
            "broken_references" => checks::check_broken_references(db),
            "malformed_wikilinks" => checks::check_malformed_wikilinks(db, vault_root),
            "schema_violations" => {
                checks::check_schema_violations(registry, db, vault_root)
            }
            "structural_consistency" => {
                checks::check_structural_consistency(db, vault_root)
            }
            "orphaned_notes" => checks::check_orphaned_notes(db),
            "db_sync" => {
                if skip_reindex {
                    CategoryReport::new("db_sync", "Index Sync")
                } else {
                    reindex_performed = true;
                    checks::check_db_sync(db, vault_root)
                }
            }
            _ => continue,
        };
        categories.push(report);
    }

    // Compute summary
    let total_errors: usize = categories.iter().map(|c| c.errors.len()).sum();
    let total_warnings: usize = categories.iter().map(|c| c.warnings.len()).sum();

    // Count total notes and notes with issues for health score
    let total_notes = db.count_notes().map(|c| c as usize).unwrap_or(0);

    // Collect unique paths with errors
    let notes_with_issues: HashSet<&str> = categories
        .iter()
        .flat_map(|c| {
            c.errors
                .iter()
                .chain(c.warnings.iter())
                .map(|i| i.path.as_str())
                .filter(|p| !p.is_empty())
        })
        .collect();

    let health_score = if total_notes == 0 {
        1.0
    } else {
        let clean_notes = total_notes.saturating_sub(notes_with_issues.len());
        clean_notes as f64 / total_notes as f64
    };

    LintReport {
        categories,
        summary: LintSummary {
            total_notes,
            total_errors,
            total_warnings,
            health_score,
            reindex_performed,
        },
    }
}

use std::collections::HashSet;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::types::{IndexedLink, IndexedNote, LinkType, NoteType};
    use chrono::Utc;
    use std::path::PathBuf;

    fn test_db() -> IndexDb {
        IndexDb::open_in_memory().unwrap()
    }

    fn insert_test_note(db: &IndexDb, path: &str, note_type: NoteType) -> i64 {
        let note = IndexedNote {
            id: None,
            path: PathBuf::from(path),
            note_type,
            title: path.to_string(),
            created: Some(Utc::now()),
            modified: Utc::now(),
            frontmatter_json: None,
            content_hash: format!("hash-{path}"),
        };
        db.insert_note(&note).unwrap()
    }

    fn insert_test_link(
        db: &IndexDb,
        source_id: i64,
        target_id: Option<i64>,
        target_path: &str,
    ) {
        let link = IndexedLink {
            id: None,
            source_id,
            target_id,
            target_path: target_path.to_string(),
            link_text: None,
            link_type: LinkType::Wikilink,
            context: None,
            line_number: None,
        };
        db.insert_link(&link).unwrap();
    }

    // ── run_lint orchestrator ────────────────────────────────────────────

    #[test]
    fn run_lint_empty_vault() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();
        let registry = TypeRegistry::new();

        let report = run_lint(&db, &registry, tmp.path(), None, true);

        assert!(report.is_clean());
        assert!(!report.has_errors());
        assert_eq!(report.summary.total_notes, 0);
        assert_eq!(report.summary.health_score, 1.0);
        assert!(!report.summary.reindex_performed);
        assert_eq!(report.categories.len(), 6);
    }

    #[test]
    fn run_lint_category_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();
        let registry = TypeRegistry::new();

        let report =
            run_lint(&db, &registry, tmp.path(), Some("broken_references"), true);
        assert_eq!(report.categories.len(), 1);
        assert_eq!(report.categories[0].name, "broken_references");
    }

    #[test]
    fn run_lint_unknown_category_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();
        let registry = TypeRegistry::new();

        let report = run_lint(&db, &registry, tmp.path(), Some("nonexistent"), true);
        assert_eq!(report.categories.len(), 0);
        assert!(report.is_clean());
    }

    #[test]
    fn run_lint_skip_reindex() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();
        let registry = TypeRegistry::new();

        let report = run_lint(&db, &registry, tmp.path(), Some("db_sync"), true);
        assert!(!report.summary.reindex_performed);
        assert!(report.categories[0].is_clean());
    }

    #[test]
    fn run_lint_with_reindex() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();
        let registry = TypeRegistry::new();

        let report = run_lint(&db, &registry, tmp.path(), Some("db_sync"), false);
        assert!(report.summary.reindex_performed);
    }

    #[test]
    fn run_lint_health_score_with_issues() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();
        let registry = TypeRegistry::new();

        // Insert 4 notes, 1 with a broken link
        let src = insert_test_note(&db, "notes/a.md", NoteType::None);
        insert_test_note(&db, "notes/b.md", NoteType::None);
        insert_test_note(&db, "notes/c.md", NoteType::None);
        insert_test_note(&db, "notes/d.md", NoteType::None);
        insert_test_link(&db, src, None, "missing.md");

        let report =
            run_lint(&db, &registry, tmp.path(), Some("broken_references"), true);

        assert_eq!(report.summary.total_notes, 4);
        assert!(report.has_errors());
        // 1 out of 4 notes has issues → 3/4 = 0.75
        assert!((report.summary.health_score - 0.75).abs() < 0.01);
    }

    #[test]
    fn run_lint_aggregates_errors_and_warnings() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();
        let registry = TypeRegistry::new();

        // Broken link → error
        let src = insert_test_note(&db, "notes/a.md", NoteType::None);
        insert_test_link(&db, src, None, "missing.md");
        // Orphaned project → warning
        insert_test_note(&db, "Projects/lonely/lonely.md", NoteType::Project);

        let report = run_lint(&db, &registry, tmp.path(), None, true);

        assert!(report.summary.total_errors >= 1);
        assert!(report.summary.total_warnings >= 1);
        assert!(report.has_errors());
    }

    // ── Result type tests ────────────────────────────────────────────────

    #[test]
    fn category_report_is_clean() {
        let report = CategoryReport::new("test", "Test");
        assert!(report.is_clean());
        assert_eq!(report.issue_count(), 0);
    }

    #[test]
    fn category_report_with_issues() {
        let mut report = CategoryReport::new("test", "Test");
        report.errors.push(LintIssue {
            path: "a.md".to_string(),
            line: None,
            message: "bad".to_string(),
            suggestion: None,
            fixable: false,
        });
        report.warnings.push(LintIssue {
            path: "b.md".to_string(),
            line: Some(5),
            message: "meh".to_string(),
            suggestion: Some("fix it".to_string()),
            fixable: true,
        });
        assert!(!report.is_clean());
        assert_eq!(report.issue_count(), 2);
    }

    #[test]
    fn lint_report_serialises_to_json() {
        let report = LintReport {
            categories: vec![CategoryReport::new("test", "Test")],
            summary: LintSummary {
                total_notes: 10,
                total_errors: 0,
                total_warnings: 0,
                health_score: 1.0,
                reindex_performed: false,
            },
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("health_score"));
        assert!(json.contains("\"total_notes\":10"));
    }
}
