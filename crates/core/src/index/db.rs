//! Database connection and operations.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;

use super::schema::{SchemaError, init_schema};
use super::types::{IndexedLink, IndexedNote, LinkType, NoteQuery, NoteType};

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Schema error: {0}")]
    Schema(#[from] SchemaError),

    #[error("Note not found: {0}")]
    NoteNotFound(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),
}

/// Vault index database handle.
pub struct IndexDb {
    conn: Connection,
}

impl IndexDb {
    /// Open or create an index database at the given path.
    pub fn open(path: &Path) -> Result<Self, IndexError> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;
        init_schema(&conn)?;
        Ok(Self { conn })
    }

    /// Create an in-memory database (for testing).
    pub fn open_in_memory() -> Result<Self, IndexError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        init_schema(&conn)?;
        Ok(Self { conn })
    }

    /// Get the underlying connection (for transactions).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Notes CRUD
    // ─────────────────────────────────────────────────────────────────────────

    /// Insert a new note into the index.
    pub fn insert_note(&self, note: &IndexedNote) -> Result<i64, IndexError> {
        self.conn.execute(
            "INSERT INTO notes (path, note_type, title, created_at, modified_at, frontmatter_json, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                note.path.to_string_lossy(),
                note.note_type.as_str(),
                note.title,
                note.created.map(|d| d.to_rfc3339()),
                note.modified.to_rfc3339(),
                note.frontmatter_json,
                note.content_hash,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Update an existing note in the index.
    pub fn update_note(&self, note: &IndexedNote) -> Result<(), IndexError> {
        let id = note.id.ok_or_else(|| {
            IndexError::InvalidData("Note must have an ID for update".to_string())
        })?;

        let rows = self.conn.execute(
            "UPDATE notes SET
                path = ?1, note_type = ?2, title = ?3,
                created_at = ?4, modified_at = ?5,
                frontmatter_json = ?6, content_hash = ?7
             WHERE id = ?8",
            params![
                note.path.to_string_lossy(),
                note.note_type.as_str(),
                note.title,
                note.created.map(|d| d.to_rfc3339()),
                note.modified.to_rfc3339(),
                note.frontmatter_json,
                note.content_hash,
                id,
            ],
        )?;

        if rows == 0 {
            return Err(IndexError::NoteNotFound(format!("ID {}", id)));
        }
        Ok(())
    }

    /// Upsert a note (insert or update based on path).
    pub fn upsert_note(&self, note: &IndexedNote) -> Result<i64, IndexError> {
        self.conn.execute(
            "INSERT INTO notes (path, note_type, title, created_at, modified_at, frontmatter_json, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(path) DO UPDATE SET
                note_type = excluded.note_type,
                title = excluded.title,
                created_at = excluded.created_at,
                modified_at = excluded.modified_at,
                frontmatter_json = excluded.frontmatter_json,
                content_hash = excluded.content_hash",
            params![
                note.path.to_string_lossy(),
                note.note_type.as_str(),
                note.title,
                note.created.map(|d| d.to_rfc3339()),
                note.modified.to_rfc3339(),
                note.frontmatter_json,
                note.content_hash,
            ],
        )?;

        // Get the ID (either new or existing)
        let id: i64 = self.conn.query_row(
            "SELECT id FROM notes WHERE path = ?1",
            [note.path.to_string_lossy()],
            |row| row.get(0),
        )?;

        Ok(id)
    }

    /// Get a note by its path.
    pub fn get_note_by_path(
        &self,
        path: &Path,
    ) -> Result<Option<IndexedNote>, IndexError> {
        self.conn
            .query_row(
                "SELECT id, path, note_type, title, created_at, modified_at, frontmatter_json, content_hash
                 FROM notes WHERE path = ?1",
                [path.to_string_lossy()],
                Self::row_to_note,
            )
            .optional()
            .map_err(Into::into)
    }

    /// Get a note by its ID.
    pub fn get_note_by_id(&self, id: i64) -> Result<Option<IndexedNote>, IndexError> {
        self.conn
            .query_row(
                "SELECT id, path, note_type, title, created_at, modified_at, frontmatter_json, content_hash
                 FROM notes WHERE id = ?1",
                [id],
                Self::row_to_note,
            )
            .optional()
            .map_err(Into::into)
    }

    /// Query notes with filters.
    pub fn query_notes(&self, query: &NoteQuery) -> Result<Vec<IndexedNote>, IndexError> {
        let mut sql = String::from(
            "SELECT id, path, note_type, title, created_at, modified_at, frontmatter_json, content_hash
             FROM notes WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(note_type) = &query.note_type {
            sql.push_str(" AND note_type = ?");
            params_vec.push(Box::new(note_type.as_str().to_string()));
        }

        if let Some(prefix) = &query.path_prefix {
            sql.push_str(" AND path LIKE ?");
            params_vec.push(Box::new(format!("{}%", prefix.to_string_lossy())));
        }

        if let Some(after) = &query.modified_after {
            sql.push_str(" AND modified_at >= ?");
            params_vec.push(Box::new(after.to_rfc3339()));
        }

        if let Some(before) = &query.modified_before {
            sql.push_str(" AND modified_at <= ?");
            params_vec.push(Box::new(before.to_rfc3339()));
        }

        sql.push_str(" ORDER BY modified_at DESC");

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = query.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let notes = stmt
            .query_map(params_refs.as_slice(), Self::row_to_note)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(notes)
    }

    /// Delete a note by path (also deletes associated links via CASCADE).
    pub fn delete_note(&self, path: &Path) -> Result<bool, IndexError> {
        let rows = self
            .conn
            .execute("DELETE FROM notes WHERE path = ?1", [path.to_string_lossy()])?;
        Ok(rows > 0)
    }

    /// Get content hash for a note path (for change detection).
    pub fn get_content_hash(&self, path: &Path) -> Result<Option<String>, IndexError> {
        self.conn
            .query_row(
                "SELECT content_hash FROM notes WHERE path = ?1",
                [path.to_string_lossy()],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    fn row_to_note(row: &rusqlite::Row) -> Result<IndexedNote, rusqlite::Error> {
        let path_str: String = row.get(1)?;
        let type_str: String = row.get(2)?;
        let created_str: Option<String> = row.get(4)?;
        let modified_str: String = row.get(5)?;

        Ok(IndexedNote {
            id: Some(row.get(0)?),
            path: path_str.into(),
            note_type: type_str.parse().unwrap(),
            title: row.get(3)?,
            created: created_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&chrono::Utc))
            }),
            modified: chrono::DateTime::parse_from_rfc3339(&modified_str)
                .map(|d| d.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            frontmatter_json: row.get(6)?,
            content_hash: row.get(7)?,
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Links CRUD
    // ─────────────────────────────────────────────────────────────────────────

    /// Insert a link between notes.
    pub fn insert_link(&self, link: &IndexedLink) -> Result<i64, IndexError> {
        self.conn.execute(
            "INSERT INTO links (source_id, target_id, target_path, link_text, link_type, context, line_number)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                link.source_id,
                link.target_id,
                link.target_path,
                link.link_text,
                link.link_type.as_str(),
                link.context,
                link.line_number,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Delete all links from a source note.
    pub fn delete_links_from(&self, source_id: i64) -> Result<usize, IndexError> {
        let rows =
            self.conn.execute("DELETE FROM links WHERE source_id = ?1", [source_id])?;
        Ok(rows)
    }

    /// Get outgoing links from a note.
    pub fn get_outgoing_links(
        &self,
        source_id: i64,
    ) -> Result<Vec<IndexedLink>, IndexError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, target_id, target_path, link_text, link_type, context, line_number
             FROM links WHERE source_id = ?1",
        )?;

        let links = stmt
            .query_map([source_id], Self::row_to_link)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(links)
    }

    /// Get incoming links (backlinks) to a note.
    pub fn get_backlinks(&self, target_id: i64) -> Result<Vec<IndexedLink>, IndexError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, target_id, target_path, link_text, link_type, context, line_number
             FROM links WHERE target_id = ?1",
        )?;

        let links = stmt
            .query_map([target_id], Self::row_to_link)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(links)
    }

    /// Find orphan notes (no incoming links).
    pub fn find_orphans(&self) -> Result<Vec<IndexedNote>, IndexError> {
        let mut stmt = self.conn.prepare(
            "SELECT n.id, n.path, n.note_type, n.title, n.created_at, n.modified_at, n.frontmatter_json, n.content_hash
             FROM notes n
             LEFT JOIN links l ON l.target_id = n.id
             WHERE l.id IS NULL",
        )?;

        let notes =
            stmt.query_map([], Self::row_to_note)?.filter_map(|r| r.ok()).collect();

        Ok(notes)
    }

    /// Resolve target_id for links by matching target_path to notes.
    pub fn resolve_link_targets(&self) -> Result<usize, IndexError> {
        let rows = self.conn.execute(
            "UPDATE links SET target_id = (
                SELECT n.id FROM notes n
                WHERE links.target_path = n.path
                   OR links.target_path || '.md' = n.path
                   OR links.target_path = REPLACE(n.path, '.md', '')
             )
             WHERE target_id IS NULL",
            [],
        )?;
        Ok(rows)
    }

    fn row_to_link(row: &rusqlite::Row) -> Result<IndexedLink, rusqlite::Error> {
        let type_str: String = row.get(5)?;
        Ok(IndexedLink {
            id: Some(row.get(0)?),
            source_id: row.get(1)?,
            target_id: row.get(2)?,
            target_path: row.get(3)?,
            link_text: row.get(4)?,
            link_type: LinkType::parse(&type_str).unwrap_or(LinkType::Wikilink),
            context: row.get(6)?,
            line_number: row.get(7)?,
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Statistics
    // ─────────────────────────────────────────────────────────────────────────

    /// Get count of notes by type.
    pub fn count_by_type(&self) -> Result<Vec<(NoteType, i64)>, IndexError> {
        let mut stmt = self
            .conn
            .prepare("SELECT note_type, COUNT(*) FROM notes GROUP BY note_type")?;

        let counts = stmt
            .query_map([], |row| {
                let type_str: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((type_str.parse().unwrap(), count))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(counts)
    }

    /// Get total note count.
    pub fn count_notes(&self) -> Result<i64, IndexError> {
        let count: i64 =
            self.conn.query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Get total link count.
    pub fn count_links(&self) -> Result<i64, IndexError> {
        let count: i64 =
            self.conn.query_row("SELECT COUNT(*) FROM links", [], |row| row.get(0))?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::path::PathBuf;

    fn sample_note(path: &str) -> IndexedNote {
        IndexedNote {
            id: None,
            path: PathBuf::from(path),
            note_type: NoteType::Zettel,
            title: "Test Note".to_string(),
            created: Some(Utc::now()),
            modified: Utc::now(),
            frontmatter_json: Some(r#"{"tags": ["test"]}"#.to_string()),
            content_hash: "abc123".to_string(),
        }
    }

    #[test]
    fn test_insert_and_get_note() {
        let db = IndexDb::open_in_memory().unwrap();
        let note = sample_note("test/note.md");

        let id = db.insert_note(&note).unwrap();
        assert!(id > 0);

        let retrieved = db.get_note_by_path(Path::new("test/note.md")).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title, "Test Note");
        assert_eq!(retrieved.note_type, NoteType::Zettel);
    }

    #[test]
    fn test_upsert_note() {
        let db = IndexDb::open_in_memory().unwrap();
        let mut note = sample_note("test/note.md");

        let id1 = db.upsert_note(&note).unwrap();
        note.title = "Updated Title".to_string();
        let id2 = db.upsert_note(&note).unwrap();

        assert_eq!(id1, id2); // Same ID after upsert

        let retrieved = db.get_note_by_id(id1).unwrap().unwrap();
        assert_eq!(retrieved.title, "Updated Title");
    }

    #[test]
    fn test_query_by_type() {
        let db = IndexDb::open_in_memory().unwrap();

        let mut zettel = sample_note("knowledge/note1.md");
        zettel.note_type = NoteType::Zettel;
        db.insert_note(&zettel).unwrap();

        let mut task = sample_note("tasks/task1.md");
        task.note_type = NoteType::Task;
        db.insert_note(&task).unwrap();

        let query = NoteQuery { note_type: Some(NoteType::Zettel), ..Default::default() };
        let results = db.query_notes(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_type, NoteType::Zettel);
    }

    #[test]
    fn test_links() {
        let db = IndexDb::open_in_memory().unwrap();

        let note1 = sample_note("note1.md");
        let note2 = sample_note("note2.md");
        let id1 = db.insert_note(&note1).unwrap();
        let id2 = db.insert_note(&note2).unwrap();

        let link = IndexedLink {
            id: None,
            source_id: id1,
            target_id: Some(id2),
            target_path: "note2.md".to_string(),
            link_text: Some("Note 2".to_string()),
            link_type: LinkType::Wikilink,
            context: None,
            line_number: Some(10),
        };
        db.insert_link(&link).unwrap();

        let outgoing = db.get_outgoing_links(id1).unwrap();
        assert_eq!(outgoing.len(), 1);

        let backlinks = db.get_backlinks(id2).unwrap();
        assert_eq!(backlinks.len(), 1);
    }

    #[test]
    fn test_orphans() {
        let db = IndexDb::open_in_memory().unwrap();

        let note1 = sample_note("note1.md");
        let note2 = sample_note("note2.md");
        let id1 = db.insert_note(&note1).unwrap();
        let id2 = db.insert_note(&note2).unwrap();

        // Link note1 -> note2, so note1 is orphan (no incoming)
        let link = IndexedLink {
            id: None,
            source_id: id1,
            target_id: Some(id2),
            target_path: "note2.md".to_string(),
            link_text: None,
            link_type: LinkType::Wikilink,
            context: None,
            line_number: None,
        };
        db.insert_link(&link).unwrap();

        let orphans = db.find_orphans().unwrap();
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].path, PathBuf::from("note1.md"));
    }
}
