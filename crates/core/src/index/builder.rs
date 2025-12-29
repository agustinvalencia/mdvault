//! Index building orchestration.

use std::path::Path;

use chrono::{DateTime, Utc};
use thiserror::Error;

use super::db::{IndexDb, IndexError};
use super::types::{IndexedLink, IndexedNote};
use crate::vault::{
    VaultWalker, VaultWalkerError, WalkedFile, content_hash, extract_note,
};

#[derive(Debug, Error)]
pub enum BuilderError {
    #[error("Vault walker error: {0}")]
    Walker(#[from] VaultWalkerError),

    #[error("Index database error: {0}")]
    Index(#[from] IndexError),

    #[error("Failed to read file {path}: {source}")]
    FileRead {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Statistics from an indexing operation.
#[derive(Debug, Clone, Default)]
pub struct IndexStats {
    /// Number of files discovered.
    pub files_found: usize,
    /// Number of notes indexed (new or updated).
    pub notes_indexed: usize,
    /// Number of notes skipped due to errors.
    pub notes_skipped: usize,
    /// Number of links indexed.
    pub links_indexed: usize,
    /// Number of broken links found.
    pub broken_links: usize,
    /// Indexing duration in milliseconds.
    pub duration_ms: u64,
    /// Number of files unchanged (hash match) - incremental mode only.
    pub files_unchanged: usize,
    /// Number of files added (new to index) - incremental mode only.
    pub files_added: usize,
    /// Number of files updated (content changed) - incremental mode only.
    pub files_updated: usize,
    /// Number of files deleted (removed from vault) - incremental mode only.
    pub files_deleted: usize,
}

/// File change classification for incremental updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChange {
    /// File is new (not in index).
    Added,
    /// File content has changed.
    Modified,
    /// File content unchanged.
    Unchanged,
}

/// Progress callback for indexing operations.
/// Parameters: (current, total, current_path)
pub type ProgressCallback = Box<dyn Fn(usize, usize, &str)>;

/// Builder for populating the vault index.
pub struct IndexBuilder<'a> {
    db: &'a IndexDb,
    vault_root: &'a Path,
}

impl<'a> IndexBuilder<'a> {
    /// Create a new index builder.
    pub fn new(db: &'a IndexDb, vault_root: &'a Path) -> Self {
        Self { db, vault_root }
    }

    /// Perform a full reindex of the vault.
    /// Clears existing data and rebuilds from scratch.
    pub fn full_reindex(
        &self,
        progress: Option<ProgressCallback>,
    ) -> Result<IndexStats, BuilderError> {
        let start = std::time::Instant::now();
        let mut stats = IndexStats::default();

        // Walk the vault
        let walker = VaultWalker::new(self.vault_root)?;
        let files = walker.walk()?;
        stats.files_found = files.len();

        // Clear existing index
        self.db.clear_all()?;

        // Phase 1: Index all notes
        for (i, file) in files.iter().enumerate() {
            if let Some(ref cb) = progress {
                cb(i + 1, files.len(), &file.relative_path.to_string_lossy());
            }

            match self.index_note(file) {
                Ok(link_count) => {
                    stats.notes_indexed += 1;
                    stats.links_indexed += link_count;
                }
                Err(e) => {
                    // Log error but continue indexing
                    tracing::warn!(
                        "Failed to index {}: {}",
                        file.relative_path.display(),
                        e
                    );
                    stats.notes_skipped += 1;
                }
            }
        }

        // Phase 2: Resolve link targets
        self.db.resolve_link_targets()?;
        stats.broken_links = self.db.count_broken_links()? as usize;

        stats.duration_ms = start.elapsed().as_millis() as u64;
        Ok(stats)
    }

    /// Perform an incremental reindex of the vault.
    /// Only processes files that have changed since last index.
    pub fn incremental_reindex(
        &self,
        progress: Option<ProgressCallback>,
    ) -> Result<IndexStats, BuilderError> {
        let start = std::time::Instant::now();
        let mut stats = IndexStats::default();

        // Phase 1: Walk the vault and collect all current files
        let walker = VaultWalker::new(self.vault_root)?;
        let files = walker.walk()?;
        stats.files_found = files.len();

        // Phase 2: Get all currently indexed paths for deletion detection
        let indexed_paths: std::collections::HashSet<std::path::PathBuf> =
            self.db.get_all_paths()?.into_iter().collect();

        // Track which paths we've seen in the vault
        let mut seen_paths: std::collections::HashSet<std::path::PathBuf> =
            std::collections::HashSet::with_capacity(files.len());

        // Phase 3: Classify and process each file
        for (i, file) in files.iter().enumerate() {
            if let Some(ref cb) = progress {
                cb(i + 1, files.len(), &file.relative_path.to_string_lossy());
            }

            seen_paths.insert(file.relative_path.clone());

            // Classify the change
            let change = self.classify_change(file)?;

            match change {
                FileChange::Unchanged => {
                    stats.files_unchanged += 1;
                }
                FileChange::Added | FileChange::Modified => match self.index_note(file) {
                    Ok(link_count) => {
                        stats.notes_indexed += 1;
                        stats.links_indexed += link_count;
                        if change == FileChange::Added {
                            stats.files_added += 1;
                        } else {
                            stats.files_updated += 1;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to index {}: {}",
                            file.relative_path.display(),
                            e
                        );
                        stats.notes_skipped += 1;
                    }
                },
            }
        }

        // Phase 4: Detect and delete removed files
        for indexed_path in &indexed_paths {
            if !seen_paths.contains(indexed_path) && self.db.delete_note(indexed_path)? {
                stats.files_deleted += 1;
                tracing::debug!("Deleted from index: {}", indexed_path.display());
            }
        }

        // Phase 5: Resolve link targets (handles newly valid links)
        self.db.resolve_link_targets()?;
        stats.broken_links = self.db.count_broken_links()? as usize;

        stats.duration_ms = start.elapsed().as_millis() as u64;
        Ok(stats)
    }

    /// Classify a file's change status by comparing content hashes.
    fn classify_change(&self, file: &WalkedFile) -> Result<FileChange, BuilderError> {
        // Get stored hash (if any)
        let stored_hash = self.db.get_content_hash(&file.relative_path)?;

        match stored_hash {
            None => Ok(FileChange::Added),
            Some(stored) => {
                // Compute current hash
                let current = content_hash(&file.absolute_path).map_err(|e| {
                    BuilderError::FileRead {
                        path: file.absolute_path.display().to_string(),
                        source: e,
                    }
                })?;

                if current == stored {
                    Ok(FileChange::Unchanged)
                } else {
                    Ok(FileChange::Modified)
                }
            }
        }
    }

    /// Index a single note file.
    /// Returns the number of links indexed.
    fn index_note(&self, file: &WalkedFile) -> Result<usize, BuilderError> {
        // Read file content
        let content = std::fs::read_to_string(&file.absolute_path).map_err(|e| {
            BuilderError::FileRead {
                path: file.absolute_path.display().to_string(),
                source: e,
            }
        })?;

        // Compute content hash
        let hash =
            content_hash(&file.absolute_path).map_err(|e| BuilderError::FileRead {
                path: file.absolute_path.display().to_string(),
                source: e,
            })?;

        // Extract note metadata
        let extracted = extract_note(&content, &file.relative_path);

        // Convert modified time to DateTime<Utc>
        let modified: DateTime<Utc> = file.modified.into();

        // Create indexed note
        let note = IndexedNote {
            id: None,
            path: file.relative_path.clone(),
            note_type: extracted.note_type,
            title: extracted.title,
            created: None, // Could extract from frontmatter if present
            modified,
            frontmatter_json: extracted.frontmatter_json,
            content_hash: hash,
        };

        // Insert note and get ID
        let note_id = self.db.upsert_note(&note)?;

        // Delete existing links for this note (in case of update)
        self.db.delete_links_from(note_id)?;

        // Insert links
        let link_count = extracted.links.len();
        for link in extracted.links {
            let indexed_link = IndexedLink {
                id: None,
                source_id: note_id,
                target_id: None, // Resolved in phase 2
                target_path: link.target,
                link_text: link.text,
                link_type: link.link_type,
                context: link.context,
                line_number: Some(link.line_number),
            };
            self.db.insert_link(&indexed_link)?;
        }

        Ok(link_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_vault() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create some markdown files with links
        fs::write(
            root.join("note1.md"),
            r#"---
title: Note One
type: zettel
---
# Note One

This links to [[note2]] and [[missing-note]].
"#,
        )
        .unwrap();

        fs::write(
            root.join("note2.md"),
            r#"---
title: Note Two
type: task
project: note1
---
# Note Two

Back to [[note1]].
"#,
        )
        .unwrap();

        fs::create_dir(root.join("subdir")).unwrap();
        fs::write(
            root.join("subdir/note3.md"),
            r#"# Note Three

Links to [Note One](../note1.md).
"#,
        )
        .unwrap();

        dir
    }

    #[test]
    fn test_full_reindex() {
        let vault = create_test_vault();
        let db = IndexDb::open_in_memory().unwrap();

        let builder = IndexBuilder::new(&db, vault.path());
        let stats = builder.full_reindex(None).unwrap();

        assert_eq!(stats.files_found, 3);
        assert_eq!(stats.notes_indexed, 3);
        assert_eq!(stats.notes_skipped, 0);
        assert!(stats.links_indexed >= 4); // At least 4 links across all notes
    }

    #[test]
    fn test_notes_are_indexed_correctly() {
        let vault = create_test_vault();
        let db = IndexDb::open_in_memory().unwrap();

        let builder = IndexBuilder::new(&db, vault.path());
        builder.full_reindex(None).unwrap();

        // Check note1 is indexed
        let note1 = db
            .get_note_by_path(Path::new("note1.md"))
            .unwrap()
            .expect("note1 should exist");
        assert_eq!(note1.title, "Note One");
        assert_eq!(note1.note_type, crate::index::types::NoteType::Zettel);

        // Check note2 is indexed
        let note2 = db
            .get_note_by_path(Path::new("note2.md"))
            .unwrap()
            .expect("note2 should exist");
        assert_eq!(note2.title, "Note Two");
        assert_eq!(note2.note_type, crate::index::types::NoteType::Task);
    }

    #[test]
    fn test_links_are_indexed() {
        let vault = create_test_vault();
        let db = IndexDb::open_in_memory().unwrap();

        let builder = IndexBuilder::new(&db, vault.path());
        builder.full_reindex(None).unwrap();

        let note1 = db
            .get_note_by_path(Path::new("note1.md"))
            .unwrap()
            .expect("note1 should exist");

        let outgoing = db.get_outgoing_links(note1.id.unwrap()).unwrap();
        assert_eq!(outgoing.len(), 2); // [[note2]] and [[missing-note]]
    }

    #[test]
    fn test_link_targets_resolved() {
        let vault = create_test_vault();
        let db = IndexDb::open_in_memory().unwrap();

        let builder = IndexBuilder::new(&db, vault.path());
        let stats = builder.full_reindex(None).unwrap();

        // At least one broken link (missing-note)
        assert!(stats.broken_links >= 1);

        // Check that existing links have target_id resolved
        let note2 = db
            .get_note_by_path(Path::new("note2.md"))
            .unwrap()
            .expect("note2 should exist");

        let backlinks = db.get_backlinks(note2.id.unwrap()).unwrap();
        // note1 links to note2
        assert!(!backlinks.is_empty());
    }

    #[test]
    fn test_reindex_clears_old_data() {
        let vault = create_test_vault();
        let db = IndexDb::open_in_memory().unwrap();

        let builder = IndexBuilder::new(&db, vault.path());

        // Index twice
        builder.full_reindex(None).unwrap();
        let stats = builder.full_reindex(None).unwrap();

        // Should still have same counts (not doubled)
        assert_eq!(stats.notes_indexed, 3);
        assert_eq!(db.count_notes().unwrap(), 3);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Incremental reindex tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_incremental_first_run() {
        let vault = create_test_vault();
        let db = IndexDb::open_in_memory().unwrap();
        let builder = IndexBuilder::new(&db, vault.path());

        let stats = builder.incremental_reindex(None).unwrap();

        assert_eq!(stats.files_found, 3);
        assert_eq!(stats.files_added, 3);
        assert_eq!(stats.files_unchanged, 0);
        assert_eq!(stats.files_updated, 0);
        assert_eq!(stats.files_deleted, 0);
        assert_eq!(stats.notes_indexed, 3);
    }

    #[test]
    fn test_incremental_no_changes() {
        let vault = create_test_vault();
        let db = IndexDb::open_in_memory().unwrap();
        let builder = IndexBuilder::new(&db, vault.path());

        builder.incremental_reindex(None).unwrap();
        let stats = builder.incremental_reindex(None).unwrap();

        assert_eq!(stats.files_found, 3);
        assert_eq!(stats.files_unchanged, 3);
        assert_eq!(stats.files_added, 0);
        assert_eq!(stats.files_updated, 0);
        assert_eq!(stats.files_deleted, 0);
        assert_eq!(stats.notes_indexed, 0);
    }

    #[test]
    fn test_incremental_file_modified() {
        let vault = create_test_vault();
        let db = IndexDb::open_in_memory().unwrap();
        let builder = IndexBuilder::new(&db, vault.path());

        builder.incremental_reindex(None).unwrap();

        // Modify a file
        fs::write(vault.path().join("note1.md"), "# Note 1 Modified\n\nNew content.")
            .unwrap();

        let stats = builder.incremental_reindex(None).unwrap();

        assert_eq!(stats.files_unchanged, 2);
        assert_eq!(stats.files_updated, 1);
        assert_eq!(stats.files_added, 0);
        assert_eq!(stats.notes_indexed, 1);
    }

    #[test]
    fn test_incremental_file_added() {
        let vault = create_test_vault();
        let db = IndexDb::open_in_memory().unwrap();
        let builder = IndexBuilder::new(&db, vault.path());

        builder.incremental_reindex(None).unwrap();

        // Add a new file
        fs::write(vault.path().join("note4.md"), "# Note 4\n\nBrand new note.").unwrap();

        let stats = builder.incremental_reindex(None).unwrap();

        assert_eq!(stats.files_found, 4);
        assert_eq!(stats.files_unchanged, 3);
        assert_eq!(stats.files_added, 1);
        assert_eq!(stats.files_updated, 0);
        assert_eq!(stats.notes_indexed, 1);
    }

    #[test]
    fn test_incremental_file_deleted() {
        let vault = create_test_vault();
        let db = IndexDb::open_in_memory().unwrap();
        let builder = IndexBuilder::new(&db, vault.path());

        builder.incremental_reindex(None).unwrap();

        // Delete a file
        fs::remove_file(vault.path().join("note2.md")).unwrap();

        let stats = builder.incremental_reindex(None).unwrap();

        assert_eq!(stats.files_found, 2);
        assert_eq!(stats.files_deleted, 1);
        assert_eq!(stats.files_unchanged, 2);

        // Verify it's gone from the index
        assert!(db.get_note_by_path(Path::new("note2.md")).unwrap().is_none());
        assert_eq!(db.count_notes().unwrap(), 2);
    }

    #[test]
    fn test_incremental_links_updated_on_change() {
        let vault = create_test_vault();
        let db = IndexDb::open_in_memory().unwrap();
        let builder = IndexBuilder::new(&db, vault.path());

        builder.incremental_reindex(None).unwrap();

        let note1 = db.get_note_by_path(Path::new("note1.md")).unwrap().unwrap();
        let links_before = db.get_outgoing_links(note1.id.unwrap()).unwrap();
        assert_eq!(links_before.len(), 2); // [[note2]] and [[missing-note]]

        // Modify to have different links
        fs::write(vault.path().join("note1.md"), "# Note 1\n\n[[note3]] only now.")
            .unwrap();
        builder.incremental_reindex(None).unwrap();

        let note1 = db.get_note_by_path(Path::new("note1.md")).unwrap().unwrap();
        let links_after = db.get_outgoing_links(note1.id.unwrap()).unwrap();
        assert_eq!(links_after.len(), 1);
        assert_eq!(links_after[0].target_path, "note3");
    }

    #[test]
    fn test_incremental_broken_links_resolved() {
        let vault = create_test_vault();
        let db = IndexDb::open_in_memory().unwrap();
        let builder = IndexBuilder::new(&db, vault.path());

        // note1 links to note2 and missing-note
        let stats1 = builder.incremental_reindex(None).unwrap();
        assert!(stats1.broken_links > 0); // missing-note is broken

        // Now create the missing note
        fs::write(vault.path().join("missing-note.md"), "# Missing Note\n\nNow exists!")
            .unwrap();

        let stats2 = builder.incremental_reindex(None).unwrap();
        assert_eq!(stats2.files_added, 1);

        // The link to missing-note should now be resolved
        let missing = db.get_note_by_path(Path::new("missing-note.md")).unwrap().unwrap();
        let backlinks = db.get_backlinks(missing.id.unwrap()).unwrap();
        assert!(!backlinks.is_empty());
    }
}
