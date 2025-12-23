//! SQLite schema definition and migrations.

use rusqlite::Connection;
use thiserror::Error;

/// Current schema version.
pub const SCHEMA_VERSION: i32 = 1;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Schema version {found} is newer than supported {supported}")]
    VersionTooNew { found: i32, supported: i32 },

    #[error("Migration failed: {0}")]
    MigrationFailed(String),
}

/// Initialize or migrate the database schema.
pub fn init_schema(conn: &Connection) -> Result<(), SchemaError> {
    let version = get_schema_version(conn)?;

    if version == 0 {
        // Fresh database - create all tables
        create_schema_v1(conn)?;
        set_schema_version(conn, SCHEMA_VERSION)?;
    } else if version < SCHEMA_VERSION {
        // Run migrations
        migrate(conn, version)?;
    } else if version > SCHEMA_VERSION {
        return Err(SchemaError::VersionTooNew {
            found: version,
            supported: SCHEMA_VERSION,
        });
    }

    Ok(())
}

fn get_schema_version(conn: &Connection) -> Result<i32, SchemaError> {
    // Check if schema_version table exists
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='schema_version'",
        [],
        |row| row.get(0),
    )?;

    if !exists {
        return Ok(0);
    }

    let version: i32 =
        conn.query_row("SELECT version FROM schema_version", [], |row| row.get(0))?;

    Ok(version)
}

fn set_schema_version(conn: &Connection, version: i32) -> Result<(), SchemaError> {
    conn.execute(
        "INSERT OR REPLACE INTO schema_version (id, version) VALUES (1, ?1)",
        [version],
    )?;
    Ok(())
}

fn create_schema_v1(conn: &Connection) -> Result<(), SchemaError> {
    conn.execute_batch(
        r#"
        -- Schema version tracking
        CREATE TABLE schema_version (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            version INTEGER NOT NULL
        );

        -- Notes table: core metadata for each markdown file
        CREATE TABLE notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            note_type TEXT NOT NULL DEFAULT 'none',
            title TEXT NOT NULL,
            created_at TEXT,
            modified_at TEXT NOT NULL,
            frontmatter_json TEXT,
            content_hash TEXT NOT NULL
        );

        -- Index for common queries
        CREATE INDEX idx_notes_type ON notes(note_type);
        CREATE INDEX idx_notes_modified ON notes(modified_at);
        CREATE INDEX idx_notes_path ON notes(path);

        -- Links table: relationships between notes
        CREATE TABLE links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
            target_id INTEGER REFERENCES notes(id) ON DELETE SET NULL,
            target_path TEXT NOT NULL,
            link_text TEXT,
            link_type TEXT NOT NULL,
            context TEXT,
            line_number INTEGER
        );

        -- Indexes for link queries
        CREATE INDEX idx_links_source ON links(source_id);
        CREATE INDEX idx_links_target ON links(target_id);
        CREATE INDEX idx_links_target_path ON links(target_path);

        -- Temporal activity: when notes are referenced in dailies
        CREATE TABLE temporal_activity (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            note_id INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
            daily_id INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
            activity_date TEXT NOT NULL,
            context TEXT
        );

        CREATE INDEX idx_temporal_note ON temporal_activity(note_id);
        CREATE INDEX idx_temporal_daily ON temporal_activity(daily_id);
        CREATE INDEX idx_temporal_date ON temporal_activity(activity_date);

        -- Activity summary: cached aggregations (can be rebuilt)
        CREATE TABLE activity_summary (
            note_id INTEGER PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
            last_seen TEXT,
            access_count_30d INTEGER NOT NULL DEFAULT 0,
            access_count_90d INTEGER NOT NULL DEFAULT 0,
            staleness_score REAL NOT NULL DEFAULT 0.0
        );

        -- Note cooccurrence: notes appearing together in dailies
        CREATE TABLE note_cooccurrence (
            note_a_id INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
            note_b_id INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
            shared_daily_count INTEGER NOT NULL DEFAULT 0,
            most_recent TEXT,
            PRIMARY KEY (note_a_id, note_b_id)
        );

        CREATE INDEX idx_cooccurrence_a ON note_cooccurrence(note_a_id);
        CREATE INDEX idx_cooccurrence_b ON note_cooccurrence(note_b_id);

        -- Full-text search virtual table (optional, for content search)
        CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
            title,
            content,
            content_rowid='id'
        );
        "#,
    )?;

    Ok(())
}

fn migrate(_conn: &Connection, from_version: i32) -> Result<(), SchemaError> {
    // Add migration steps here as schema evolves
    // Example:
    // match from_version {
    //     1 => migrate_v1_to_v2(conn)?,
    //     2 => migrate_v2_to_v3(conn)?,
    //     _ => {}
    // }

    // For now, no migrations exist - we only have v1
    Err(SchemaError::MigrationFailed(format!(
        "No migration path from version {} to {}",
        from_version, SCHEMA_VERSION
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_init_fresh_database() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();

        // Verify schema version
        let version: i32 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);

        // Verify tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"notes".to_string()));
        assert!(tables.contains(&"links".to_string()));
        assert!(tables.contains(&"temporal_activity".to_string()));
    }

    #[test]
    fn test_init_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        init_schema(&conn).unwrap(); // Should not fail on second call
    }
}
