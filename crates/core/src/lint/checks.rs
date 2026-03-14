//! Individual lint check implementations.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::frontmatter::parse as parse_frontmatter;
use crate::index::{IndexBuilder, IndexDb, NoteQuery, NoteType};
use crate::types::{TypeRegistry, validate_note};

use super::result::{CategoryReport, LintIssue};

/// Check 1: Find broken references (outgoing links with no target).
pub fn check_broken_references(db: &IndexDb) -> CategoryReport {
    let mut report = CategoryReport::new("broken_references", "Broken References");

    let all_notes = match db.query_notes(&NoteQuery::default()) {
        Ok(notes) => notes,
        Err(e) => {
            report.errors.push(LintIssue {
                path: String::new(),
                line: None,
                message: format!("Failed to query notes: {e}"),
                suggestion: None,
                fixable: false,
            });
            return report;
        }
    };

    for note in &all_notes {
        let note_id = match note.id {
            Some(id) => id,
            None => continue,
        };

        let links = match db.get_outgoing_links(note_id) {
            Ok(links) => links,
            Err(_) => continue,
        };

        for link in links {
            if link.target_id.is_none() {
                let msg = match &link.link_text {
                    Some(text) => format!(
                        "broken {} link '{}' -> '{}' (target does not exist)",
                        link.link_type.as_str(),
                        text,
                        link.target_path,
                    ),
                    None => format!(
                        "broken {} link -> '{}' (target does not exist)",
                        link.link_type.as_str(),
                        link.target_path,
                    ),
                };

                report.errors.push(LintIssue {
                    path: note.path.to_string_lossy().to_string(),
                    line: link.line_number,
                    message: msg,
                    suggestion: None,
                    fixable: false,
                });
            }
        }
    }

    report
}

/// Check 2: Find malformed wikilinks (ID-pattern links without aliases).
pub fn check_malformed_wikilinks(db: &IndexDb, vault_root: &Path) -> CategoryReport {
    let mut report = CategoryReport::new("malformed_wikilinks", "Malformed Wikilinks");

    let id_patterns = [
        regex::Regex::new(r"^[A-Z]+-\d{3,}").unwrap(), // PROJ-001
        regex::Regex::new(r"^MTG-\d{4}-\d{2}-\d{2}-\d{3}").unwrap(), // MTG-2026-01-01-001
    ];

    let all_notes = match db.query_notes(&NoteQuery::default()) {
        Ok(notes) => notes,
        Err(_) => return report,
    };

    let wikilink_re = regex::Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").unwrap();

    for note in &all_notes {
        let full_path = vault_root.join(&note.path);
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Find all wikilinks in the content
        for (line_num, line) in content.lines().enumerate() {
            for cap in wikilink_re.captures_iter(line) {
                let target = cap.get(1).unwrap().as_str();
                let alias = cap.get(2).map(|m| m.as_str());

                // Check if target matches an ID pattern
                let is_id_pattern = id_patterns.iter().any(|re| re.is_match(target));

                if !is_id_pattern {
                    continue;
                }

                // ID-pattern link without alias — user sees the raw ID
                if alias.is_none() || alias == Some(target) {
                    report.warnings.push(LintIssue {
                        path: note.path.to_string_lossy().to_string(),
                        line: Some((line_num + 1) as u32),
                        message: format!(
                            "wikilink [[{}]] uses bare ID without alias",
                            target
                        ),
                        suggestion: Some(format!(
                            "use [[{}|descriptive alias]] instead",
                            target
                        )),
                        fixable: false,
                    });
                }
            }
        }
    }

    report
}

/// Check 3: Validate notes against their type schemas.
pub fn check_schema_violations(
    registry: &TypeRegistry,
    db: &IndexDb,
    vault_root: &Path,
) -> CategoryReport {
    let mut report = CategoryReport::new("schema_violations", "Schema Violations");

    let all_notes = match db.query_notes(&NoteQuery::default()) {
        Ok(notes) => notes,
        Err(_) => return report,
    };

    for note in &all_notes {
        let note_type = note.note_type.as_str();

        // Skip untyped notes without custom definitions
        if !registry.has_definition(note_type) && note_type == "none" {
            continue;
        }

        let full_path = vault_root.join(&note.path);
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Parse frontmatter
        let frontmatter: serde_yaml::Value = parse_frontmatter(&content)
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
            registry,
            note_type,
            &full_path.to_string_lossy(),
            &frontmatter,
            &content,
        );

        let path_str = note.path.to_string_lossy().to_string();

        for error in &result.errors {
            report.errors.push(LintIssue {
                path: path_str.clone(),
                line: None,
                message: error.to_string(),
                suggestion: None,
                fixable: true,
            });
        }

        for warning in &result.warnings {
            report.warnings.push(LintIssue {
                path: path_str.clone(),
                line: None,
                message: warning.clone(),
                suggestion: None,
                fixable: false,
            });
        }
    }

    report
}

/// Check 4: Structural consistency (duplicate IDs, wrong directories).
pub fn check_structural_consistency(db: &IndexDb, _vault_root: &Path) -> CategoryReport {
    let mut report =
        CategoryReport::new("structural_consistency", "Structural Consistency");

    let all_notes = match db.query_notes(&NoteQuery::default()) {
        Ok(notes) => notes,
        Err(_) => return report,
    };

    // Check for duplicate IDs within typed notes
    let mut seen_ids: HashMap<String, Vec<String>> = HashMap::new();

    for note in &all_notes {
        // Extract ID from frontmatter
        if let Some(ref fm_json) = note.frontmatter_json
            && let Ok(fm) = serde_json::from_str::<serde_json::Value>(fm_json)
        {
            // Check task-id, project-id, meeting-id
            for id_field in &["task-id", "project-id", "meeting-id"] {
                if let Some(id_val) = fm.get(id_field).and_then(|v| v.as_str()) {
                    let key = format!("{}:{}", id_field, id_val);
                    seen_ids
                        .entry(key)
                        .or_default()
                        .push(note.path.to_string_lossy().to_string());
                }
            }
        }
    }

    // Report duplicates
    for (key, paths) in &seen_ids {
        if paths.len() > 1 {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            let (field, id) = (parts[0], parts[1]);
            for path in paths {
                report.errors.push(LintIssue {
                    path: path.clone(),
                    line: None,
                    message: format!(
                        "duplicate {} '{}' (shared by {} notes)",
                        field,
                        id,
                        paths.len()
                    ),
                    suggestion: None,
                    fixable: false,
                });
            }
        }
    }

    // Check notes in wrong directories for their type
    for note in &all_notes {
        let path_str = note.path.to_string_lossy().to_string();
        let expected_prefix = match note.note_type {
            NoteType::Daily => Some("Journal/"),
            NoteType::Weekly => Some("Journal/"),
            NoteType::Task => None, // Tasks can be in project subdirs
            NoteType::Project => Some("Projects/"),
            NoteType::Zettel => Some("Zettelkasten/"),
            NoteType::None => None,
        };

        if let Some(prefix) = expected_prefix
            && !path_str.starts_with(prefix)
        {
            report.warnings.push(LintIssue {
                path: path_str,
                line: None,
                message: format!(
                    "{} note is outside expected directory '{}'",
                    note.note_type.as_str(),
                    prefix
                ),
                suggestion: None,
                fixable: false,
            });
        }
    }

    report
}

/// Check 5: Find orphaned notes (no incoming links), excluding entry-point types.
pub fn check_orphaned_notes(db: &IndexDb) -> CategoryReport {
    let mut report = CategoryReport::new("orphaned_notes", "Orphaned Notes");

    let orphans = match db.find_orphans() {
        Ok(notes) => notes,
        Err(e) => {
            report.errors.push(LintIssue {
                path: String::new(),
                line: None,
                message: format!("Failed to find orphans: {e}"),
                suggestion: None,
                fixable: false,
            });
            return report;
        }
    };

    // Entry-point types that are naturally orphaned
    let entry_types: HashSet<NoteType> =
        [NoteType::Daily, NoteType::Weekly].into_iter().collect();

    for note in orphans {
        if entry_types.contains(&note.note_type) {
            continue;
        }

        report.warnings.push(LintIssue {
            path: note.path.to_string_lossy().to_string(),
            line: None,
            message: format!("{} note has no incoming links", note.note_type.as_str()),
            suggestion: Some(
                "link to this note from a daily note, project, or other note".to_string(),
            ),
            fixable: false,
        });
    }

    report
}

/// Check 6: Database sync -- run incremental reindex and report out-of-sync files.
pub fn check_db_sync(db: &IndexDb, vault_root: &Path) -> CategoryReport {
    let mut report = CategoryReport::new("db_sync", "Index Sync");

    let builder = IndexBuilder::new(db, vault_root);
    match builder.incremental_reindex(None) {
        Ok(stats) => {
            if stats.files_added > 0 {
                report.warnings.push(LintIssue {
                    path: String::new(),
                    line: None,
                    message: format!(
                        "{} new file(s) were not in the index",
                        stats.files_added
                    ),
                    suggestion: Some("run 'mdv reindex' to update".to_string()),
                    fixable: false,
                });
            }
            if stats.files_updated > 0 {
                report.warnings.push(LintIssue {
                    path: String::new(),
                    line: None,
                    message: format!(
                        "{} file(s) had changed since last index",
                        stats.files_updated
                    ),
                    suggestion: None,
                    fixable: false,
                });
            }
            if stats.files_deleted > 0 {
                report.warnings.push(LintIssue {
                    path: String::new(),
                    line: None,
                    message: format!(
                        "{} file(s) in the index no longer exist on disk",
                        stats.files_deleted
                    ),
                    suggestion: Some("run 'mdv reindex' to clean up".to_string()),
                    fixable: false,
                });
            }
        }
        Err(e) => {
            report.errors.push(LintIssue {
                path: String::new(),
                line: None,
                message: format!("Failed to check index sync: {e}"),
                suggestion: Some("run 'mdv reindex --force' to rebuild".to_string()),
                fixable: false,
            });
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::types::{IndexedLink, IndexedNote, LinkType};
    use crate::types::definition::TypeDefinition;
    use crate::types::schema::{FieldSchema, FieldType};
    use chrono::Utc;
    use std::path::PathBuf;

    fn test_db() -> IndexDb {
        IndexDb::open_in_memory().unwrap()
    }

    fn insert_test_note(db: &IndexDb, path: &str, note_type: NoteType) -> i64 {
        insert_test_note_with_fm(db, path, note_type, None)
    }

    fn insert_test_note_with_fm(
        db: &IndexDb,
        path: &str,
        note_type: NoteType,
        fm_json: Option<&str>,
    ) -> i64 {
        let note = IndexedNote {
            id: None,
            path: PathBuf::from(path),
            note_type,
            title: path.to_string(),
            created: Some(Utc::now()),
            modified: Utc::now(),
            frontmatter_json: fm_json.map(String::from),
            content_hash: format!("hash-{path}"),
        };
        db.insert_note(&note).unwrap()
    }

    fn insert_test_link(
        db: &IndexDb,
        source_id: i64,
        target_id: Option<i64>,
        target_path: &str,
        link_text: Option<&str>,
        line: Option<u32>,
    ) {
        let link = IndexedLink {
            id: None,
            source_id,
            target_id,
            target_path: target_path.to_string(),
            link_text: link_text.map(String::from),
            link_type: LinkType::Wikilink,
            context: None,
            line_number: line,
        };
        db.insert_link(&link).unwrap();
    }

    // ── check_broken_references ──────────────────────────────────────────

    #[test]
    fn broken_refs_empty_db() {
        let db = test_db();
        let report = check_broken_references(&db);
        assert!(report.is_clean());
        assert_eq!(report.name, "broken_references");
    }

    #[test]
    fn broken_refs_no_broken_links() {
        let db = test_db();
        let src = insert_test_note(&db, "notes/a.md", NoteType::None);
        let tgt = insert_test_note(&db, "notes/b.md", NoteType::None);
        insert_test_link(&db, src, Some(tgt), "notes/b.md", Some("B"), None);

        let report = check_broken_references(&db);
        assert!(report.is_clean());
    }

    #[test]
    fn broken_refs_detects_broken_link() {
        let db = test_db();
        let src = insert_test_note(&db, "notes/a.md", NoteType::None);
        insert_test_link(&db, src, None, "notes/missing.md", Some("Missing"), Some(5));

        let report = check_broken_references(&db);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].message.contains("missing.md"));
        assert!(report.errors[0].message.contains("target does not exist"));
        assert_eq!(report.errors[0].line, Some(5));
        assert_eq!(report.errors[0].path, "notes/a.md");
    }

    #[test]
    fn broken_refs_link_without_text() {
        let db = test_db();
        let src = insert_test_note(&db, "notes/a.md", NoteType::None);
        insert_test_link(&db, src, None, "gone.md", None, None);

        let report = check_broken_references(&db);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].message.contains("gone.md"));
    }

    #[test]
    fn broken_refs_multiple_broken_from_one_note() {
        let db = test_db();
        let src = insert_test_note(&db, "notes/a.md", NoteType::None);
        insert_test_link(&db, src, None, "x.md", None, Some(1));
        insert_test_link(&db, src, None, "y.md", None, Some(3));

        let report = check_broken_references(&db);
        assert_eq!(report.errors.len(), 2);
    }

    // ── check_malformed_wikilinks ────────────────────────────────────────

    #[test]
    fn malformed_wikilinks_clean_with_alias() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();

        let note_path = tmp.path().join("notes/a.md");
        std::fs::create_dir_all(note_path.parent().unwrap()).unwrap();
        std::fs::write(&note_path, "See [[MCP-001|MCP task]]").unwrap();
        insert_test_note(&db, "notes/a.md", NoteType::None);

        let report = check_malformed_wikilinks(&db, tmp.path());
        assert!(report.is_clean());
    }

    #[test]
    fn malformed_wikilinks_bare_id() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();

        let note_path = tmp.path().join("notes/a.md");
        std::fs::create_dir_all(note_path.parent().unwrap()).unwrap();
        std::fs::write(&note_path, "See [[MCP-001]] and [[MTG-2026-01-15-001]]").unwrap();
        insert_test_note(&db, "notes/a.md", NoteType::None);

        let report = check_malformed_wikilinks(&db, tmp.path());
        assert_eq!(report.warnings.len(), 2);
        assert!(report.warnings[0].message.contains("MCP-001"));
        assert!(report.warnings[0].suggestion.is_some());
        assert_eq!(report.warnings[0].line, Some(1));
    }

    #[test]
    fn malformed_wikilinks_ignores_non_id_links() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();

        let note_path = tmp.path().join("notes/a.md");
        std::fs::create_dir_all(note_path.parent().unwrap()).unwrap();
        std::fs::write(&note_path, "See [[some-note]] and [[2026-01-15]]").unwrap();
        insert_test_note(&db, "notes/a.md", NoteType::None);

        let report = check_malformed_wikilinks(&db, tmp.path());
        assert!(report.is_clean());
    }

    #[test]
    fn malformed_wikilinks_on_different_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();

        let note_path = tmp.path().join("notes/a.md");
        std::fs::create_dir_all(note_path.parent().unwrap()).unwrap();
        std::fs::write(&note_path, "Line one\n[[PROJ-123]]\nLine three\n[[PROJ-456]]")
            .unwrap();
        insert_test_note(&db, "notes/a.md", NoteType::None);

        let report = check_malformed_wikilinks(&db, tmp.path());
        assert_eq!(report.warnings.len(), 2);
        assert_eq!(report.warnings[0].line, Some(2));
        assert_eq!(report.warnings[1].line, Some(4));
    }

    // ── check_schema_violations ──────────────────────────────────────────

    #[test]
    fn schema_violations_skips_untyped() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();
        let registry = TypeRegistry::new();

        let note_path = tmp.path().join("notes/a.md");
        std::fs::create_dir_all(note_path.parent().unwrap()).unwrap();
        std::fs::write(&note_path, "---\ntype: none\n---\nHello").unwrap();
        insert_test_note(&db, "notes/a.md", NoteType::None);

        let report = check_schema_violations(&registry, &db, tmp.path());
        assert!(report.is_clean());
    }

    #[test]
    fn schema_violations_detects_missing_required() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();
        let mut registry = TypeRegistry::new();

        let mut td = TypeDefinition::empty("task");
        td.schema.insert(
            "status".to_string(),
            FieldSchema {
                field_type: Some(FieldType::String),
                required: true,
                ..Default::default()
            },
        );
        registry.register(td).unwrap();

        let note_path = tmp.path().join("tasks/t.md");
        std::fs::create_dir_all(note_path.parent().unwrap()).unwrap();
        std::fs::write(&note_path, "---\ntype: task\ntitle: Do thing\n---\nContent")
            .unwrap();
        insert_test_note(&db, "tasks/t.md", NoteType::Task);

        let report = check_schema_violations(&registry, &db, tmp.path());
        assert!(!report.errors.is_empty());
        assert!(report.errors[0].message.contains("status"));
    }

    // ── check_structural_consistency ─────────────────────────────────────

    #[test]
    fn structural_consistency_clean() {
        let db = test_db();
        insert_test_note(&db, "Journal/2026/Daily/2026-01-15.md", NoteType::Daily);
        insert_test_note(&db, "Projects/MCP/MCP.md", NoteType::Project);

        let report = check_structural_consistency(&db, Path::new("/tmp"));
        assert!(report.errors.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn structural_consistency_wrong_directory() {
        let db = test_db();
        insert_test_note(&db, "random/daily.md", NoteType::Daily);

        let report = check_structural_consistency(&db, Path::new("/tmp"));
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].message.contains("outside expected directory"));
    }

    #[test]
    fn structural_consistency_duplicate_ids() {
        let db = test_db();
        let fm = r#"{"task-id":"MCP-001"}"#;
        insert_test_note_with_fm(&db, "tasks/a.md", NoteType::Task, Some(fm));
        insert_test_note_with_fm(&db, "tasks/b.md", NoteType::Task, Some(fm));

        let report = check_structural_consistency(&db, Path::new("/tmp"));
        assert_eq!(report.errors.len(), 2);
        assert!(report.errors[0].message.contains("duplicate"));
        assert!(report.errors[0].message.contains("MCP-001"));
    }

    #[test]
    fn structural_consistency_unique_ids_ok() {
        let db = test_db();
        insert_test_note_with_fm(
            &db,
            "tasks/a.md",
            NoteType::Task,
            Some(r#"{"task-id":"MCP-001"}"#),
        );
        insert_test_note_with_fm(
            &db,
            "tasks/b.md",
            NoteType::Task,
            Some(r#"{"task-id":"MCP-002"}"#),
        );

        let report = check_structural_consistency(&db, Path::new("/tmp"));
        assert!(report.errors.is_empty());
    }

    #[test]
    fn structural_consistency_tasks_anywhere() {
        let db = test_db();
        insert_test_note(&db, "Projects/MCP/tasks/MCP-001.md", NoteType::Task);

        let report = check_structural_consistency(&db, Path::new("/tmp"));
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn structural_consistency_zettel_wrong_dir() {
        let db = test_db();
        insert_test_note(&db, "random/thought.md", NoteType::Zettel);

        let report = check_structural_consistency(&db, Path::new("/tmp"));
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].message.contains("zettel"));
    }

    // ── check_orphaned_notes ─────────────────────────────────────────────

    #[test]
    fn orphaned_notes_empty_db() {
        let db = test_db();
        let report = check_orphaned_notes(&db);
        assert!(report.is_clean());
    }

    #[test]
    fn orphaned_notes_excludes_daily_weekly() {
        let db = test_db();
        insert_test_note(&db, "Journal/2026/Daily/2026-01-15.md", NoteType::Daily);
        insert_test_note(&db, "Journal/2026/Weekly/2026-W03.md", NoteType::Weekly);

        let report = check_orphaned_notes(&db);
        assert!(report.is_clean());
    }

    #[test]
    fn orphaned_notes_reports_task_orphan() {
        let db = test_db();
        insert_test_note(&db, "tasks/lonely.md", NoteType::Task);

        let report = check_orphaned_notes(&db);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].message.contains("no incoming links"));
    }

    #[test]
    fn orphaned_notes_linked_note_not_reported() {
        let db = test_db();
        let src =
            insert_test_note(&db, "Journal/2026/Daily/2026-01-15.md", NoteType::Daily);
        let tgt = insert_test_note(&db, "tasks/linked.md", NoteType::Task);
        insert_test_link(&db, src, Some(tgt), "tasks/linked.md", None, None);

        let report = check_orphaned_notes(&db);
        assert!(report.is_clean());
    }

    #[test]
    fn orphaned_notes_project_reported() {
        let db = test_db();
        insert_test_note(&db, "Projects/orphan/orphan.md", NoteType::Project);

        let report = check_orphaned_notes(&db);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].message.contains("project"));
    }

    // ── check_db_sync ────────────────────────────────────────────────────

    #[test]
    fn db_sync_empty_vault() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();

        let report = check_db_sync(&db, tmp.path());
        assert!(report.is_clean());
    }

    #[test]
    fn db_sync_detects_new_files() {
        let tmp = tempfile::tempdir().unwrap();
        let db = test_db();

        let note = tmp.path().join("hello.md");
        std::fs::write(&note, "---\ntitle: Hello\n---\nContent").unwrap();

        let report = check_db_sync(&db, tmp.path());
        assert!(!report.is_clean());
        assert!(report.warnings.iter().any(|w| w.message.contains("new file")));
    }
}
