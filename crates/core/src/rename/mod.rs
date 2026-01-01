//! Rename and reference management for mdvault.
//!
//! This module provides safe note renaming with automatic reference updates.
//! It handles wikilinks, markdown links, and frontmatter references.

mod detector;
mod types;
mod updater;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub use types::*;

use crate::index::IndexDb;

use detector::find_references_in_content;
use updater::apply_updates;

/// Generate a preview of what a rename operation would do.
///
/// This does not modify any files - it only calculates what changes would be made.
pub fn generate_preview(
    db: &IndexDb,
    vault_root: &Path,
    old_path: &Path,
    new_path: &Path,
) -> Result<RenamePreview, RenameError> {
    // Validate paths
    let old_abs = if old_path.is_absolute() {
        old_path.to_path_buf()
    } else {
        vault_root.join(old_path)
    };

    let new_abs = if new_path.is_absolute() {
        new_path.to_path_buf()
    } else {
        vault_root.join(new_path)
    };

    if !old_abs.exists() {
        return Err(RenameError::SourceNotFound(old_abs));
    }

    if new_abs.exists() {
        return Err(RenameError::TargetExists(new_abs));
    }

    // Find the note in the index
    let old_rel = old_abs.strip_prefix(vault_root).unwrap_or(&old_abs);
    let note = db
        .get_note_by_path(old_rel)
        .map_err(|e| RenameError::IndexError(e.to_string()))?
        .ok_or_else(|| RenameError::NoteNotInIndex(old_abs.clone()))?;

    let note_id =
        note.id.ok_or_else(|| RenameError::IndexError("Note has no ID".to_string()))?;

    // Get backlinks from index to find files that reference this note
    let backlinks =
        db.get_backlinks(note_id).map_err(|e| RenameError::IndexError(e.to_string()))?;

    // Find all references by parsing the source files
    let mut all_references = Vec::new();
    let mut files_to_scan: HashMap<PathBuf, ()> = HashMap::new();

    for link in &backlinks {
        if let Some(source_note) = db
            .get_note_by_id(link.source_id)
            .map_err(|e| RenameError::IndexError(e.to_string()))?
        {
            let source_path = vault_root.join(&source_note.path);
            files_to_scan.insert(source_path, ());
        }
    }

    // Scan each file for exact reference positions
    for source_path in files_to_scan.keys() {
        let content = fs::read_to_string(source_path).map_err(|e| {
            RenameError::ReadError { path: source_path.clone(), source: e }
        })?;

        let refs =
            find_references_in_content(&content, source_path, &old_abs, vault_root);
        all_references.extend(refs);
    }

    // Get the new basename for reference updates
    let new_basename =
        new_abs.file_stem().and_then(|s| s.to_str()).unwrap_or("unnamed").to_string();

    // Generate file changes
    let mut changes = Vec::new();
    let mut warnings = Vec::new();

    // Group references by file
    let mut refs_by_file: HashMap<PathBuf, Vec<Reference>> = HashMap::new();
    for reference in &all_references {
        refs_by_file
            .entry(reference.source_path.clone())
            .or_default()
            .push(reference.clone());
    }

    for (source_path, refs) in refs_by_file {
        let content = fs::read_to_string(&source_path).map_err(|e| {
            RenameError::ReadError { path: source_path.clone(), source: e }
        })?;

        let new_content = apply_updates(&content, &refs, &new_basename);

        changes.push(FileChange {
            path: source_path,
            original_content: content,
            new_content,
            references: refs,
        });
    }

    // Check for potential ambiguity (multiple notes with same basename)
    let new_basename_lower = new_basename.to_lowercase();
    if let Ok(notes) = db.query_notes(&Default::default()) {
        let conflicts: Vec<_> = notes
            .iter()
            .filter(|n| {
                let basename = n.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                basename.to_lowercase() == new_basename_lower && n.path != old_rel
            })
            .collect();

        if !conflicts.is_empty() {
            warnings.push(format!(
                "Warning: {} existing note(s) have the same basename '{}'. \
                 This may cause ambiguous wikilink references.",
                conflicts.len(),
                new_basename
            ));
        }
    }

    Ok(RenamePreview {
        old_path: old_abs,
        new_path: new_abs,
        references: all_references,
        changes,
        warnings,
    })
}

/// Execute a rename operation.
///
/// This modifies files on disk and updates the index.
pub fn execute_rename(
    db: &IndexDb,
    vault_root: &Path,
    old_path: &Path,
    new_path: &Path,
) -> Result<RenameResult, RenameError> {
    // Generate preview first to get all the info
    let preview = generate_preview(db, vault_root, old_path, new_path)?;

    // Apply changes to all affected files
    let mut files_modified = Vec::new();
    let mut references_updated = 0;

    for change in &preview.changes {
        fs::write(&change.path, &change.new_content).map_err(|e| {
            RenameError::WriteError { path: change.path.clone(), source: e }
        })?;

        files_modified.push(change.path.clone());
        references_updated += change.references.len();
    }

    // Create parent directory for new path if needed
    if let Some(parent) = preview.new_path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|e| RenameError::WriteError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    // Rename the file itself
    fs::rename(&preview.old_path, &preview.new_path).map_err(RenameError::RenameError)?;

    // Update the index
    let old_rel = preview.old_path.strip_prefix(vault_root).unwrap_or(&preview.old_path);
    let new_rel = preview.new_path.strip_prefix(vault_root).unwrap_or(&preview.new_path);

    update_note_path(db, old_rel, new_rel)
        .map_err(|e| RenameError::IndexError(e.to_string()))?;

    // Re-resolve link targets after the rename
    db.resolve_link_targets().map_err(|e| RenameError::IndexError(e.to_string()))?;

    Ok(RenameResult {
        old_path: preview.old_path,
        new_path: preview.new_path,
        files_modified,
        references_updated,
        warnings: preview.warnings,
    })
}

/// Update a note's path in the index.
fn update_note_path(
    db: &IndexDb,
    old_path: &Path,
    new_path: &Path,
) -> Result<(), crate::index::IndexError> {
    let conn = db.connection();

    // Update the notes table
    conn.execute(
        "UPDATE notes SET path = ?1 WHERE path = ?2",
        rusqlite::params![new_path.to_string_lossy(), old_path.to_string_lossy(),],
    )?;

    // Update target_path in links table where it matches the old path
    let old_basename = old_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let new_basename = new_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    // Update exact matches
    conn.execute(
        "UPDATE links SET target_path = ?1 WHERE target_path = ?2",
        rusqlite::params![new_path.to_string_lossy(), old_path.to_string_lossy(),],
    )?;

    // Update basename-only matches
    conn.execute(
        "UPDATE links SET target_path = ?1 WHERE target_path = ?2",
        rusqlite::params![new_basename, old_basename],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::types::{IndexedNote, NoteType};
    use chrono::Utc;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_test_vault() -> (TempDir, IndexDb) {
        let temp_dir = TempDir::new().unwrap();
        let db = IndexDb::open_in_memory().unwrap();
        (temp_dir, db)
    }

    fn create_note(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

    fn sample_note(path: &str) -> IndexedNote {
        IndexedNote {
            id: None,
            path: PathBuf::from(path),
            note_type: NoteType::None,
            title: "Test".to_string(),
            created: Some(Utc::now()),
            modified: Utc::now(),
            frontmatter_json: None,
            content_hash: "hash".to_string(),
        }
    }

    #[test]
    fn test_generate_preview_source_not_found() {
        let (temp_dir, db) = setup_test_vault();
        let result = generate_preview(
            &db,
            temp_dir.path(),
            Path::new("nonexistent.md"),
            Path::new("new.md"),
        );

        assert!(matches!(result, Err(RenameError::SourceNotFound(_))));
    }

    #[test]
    fn test_generate_preview_target_exists() {
        let (temp_dir, db) = setup_test_vault();

        create_note(temp_dir.path(), "old.md", "# Old");
        create_note(temp_dir.path(), "new.md", "# New");

        db.insert_note(&sample_note("old.md")).unwrap();

        let result = generate_preview(
            &db,
            temp_dir.path(),
            Path::new("old.md"),
            Path::new("new.md"),
        );

        assert!(matches!(result, Err(RenameError::TargetExists(_))));
    }

    #[test]
    fn test_generate_preview_no_references() {
        let (temp_dir, db) = setup_test_vault();

        create_note(temp_dir.path(), "old.md", "# Old Note\n\nContent here.");
        db.insert_note(&sample_note("old.md")).unwrap();

        let preview = generate_preview(
            &db,
            temp_dir.path(),
            Path::new("old.md"),
            Path::new("new.md"),
        )
        .unwrap();

        assert_eq!(preview.references.len(), 0);
        assert_eq!(preview.changes.len(), 0);
    }

    #[test]
    fn test_execute_rename_simple() {
        let (temp_dir, db) = setup_test_vault();

        create_note(temp_dir.path(), "old.md", "# Old Note\n\nContent.");
        db.insert_note(&sample_note("old.md")).unwrap();

        let result = execute_rename(
            &db,
            temp_dir.path(),
            Path::new("old.md"),
            Path::new("new.md"),
        )
        .unwrap();

        // Old file should not exist
        assert!(!temp_dir.path().join("old.md").exists());

        // New file should exist
        assert!(temp_dir.path().join("new.md").exists());

        // Index should be updated
        assert!(db.get_note_by_path(Path::new("old.md")).unwrap().is_none());
        assert!(db.get_note_by_path(Path::new("new.md")).unwrap().is_some());

        assert_eq!(result.references_updated, 0);
    }

    #[test]
    fn test_execute_rename_with_references() {
        let (temp_dir, db) = setup_test_vault();

        // Create target note
        create_note(temp_dir.path(), "old.md", "# Old Note\n\nContent.");
        let old_id = db.insert_note(&sample_note("old.md")).unwrap();

        // Create source note with reference
        create_note(temp_dir.path(), "source.md", "# Source\n\nSee [[old]] for details.");
        let source_id = db.insert_note(&sample_note("source.md")).unwrap();

        // Add link in index
        db.insert_link(&crate::index::types::IndexedLink {
            id: None,
            source_id,
            target_id: Some(old_id),
            target_path: "old".to_string(),
            link_text: None,
            link_type: crate::index::types::LinkType::Wikilink,
            context: None,
            line_number: Some(3),
        })
        .unwrap();

        // Execute rename
        let result = execute_rename(
            &db,
            temp_dir.path(),
            Path::new("old.md"),
            Path::new("new.md"),
        )
        .unwrap();

        // Check reference was updated
        assert_eq!(result.references_updated, 1);
        assert_eq!(result.files_modified.len(), 1);

        // Verify file content was updated
        let source_content =
            fs::read_to_string(temp_dir.path().join("source.md")).unwrap();
        assert!(source_content.contains("[[new]]"));
        assert!(!source_content.contains("[[old]]"));
    }
}
