//! Index query bindings for Lua.
//!
//! This module provides Lua bindings for querying the vault index:
//! - `mdv.current_note()` - Get the current note being processed
//! - `mdv.backlinks(path)` - Get notes linking to a path
//! - `mdv.outlinks(path)` - Get notes a path links to
//! - `mdv.query(opts)` - Query the vault index

use std::path::Path;

use mlua::{Function, Lua, Result as LuaResult, Table, Value};

use super::vault_context::VaultContext;
use crate::index::NoteQuery;
use crate::types::validation::yaml_to_lua_table;

/// Register index query bindings on an existing mdv table.
///
/// This adds `mdv.current_note()`, `mdv.backlinks()`, `mdv.outlinks()`, and
/// `mdv.query()` functions that have access to the vault index.
pub fn register_index_bindings(lua: &Lua) -> LuaResult<()> {
    let mdv: Table = lua.globals().get("mdv")?;

    mdv.set("current_note", create_current_note_fn(lua)?)?;
    mdv.set("backlinks", create_backlinks_fn(lua)?)?;
    mdv.set("outlinks", create_outlinks_fn(lua)?)?;
    mdv.set("query", create_query_fn(lua)?)?;

    Ok(())
}

/// Create the `mdv.current_note()` function.
///
/// Returns the current note being processed, or nil if not available.
///
/// # Examples (in Lua)
///
/// ```lua
/// local note = mdv.current_note()
/// if note then
///     print("Processing: " .. note.path)
///     print("Type: " .. note.type)
/// end
/// ```
fn create_current_note_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, ()| {
        let ctx = lua
            .app_data_ref::<VaultContext>()
            .ok_or_else(|| mlua::Error::runtime("VaultContext not available"))?;

        let current = match &ctx.current_note {
            Some(note) => note,
            None => return Ok(Value::Nil),
        };

        // Build note table
        let note_table = lua.create_table()?;
        note_table.set("path", current.path.as_str())?;
        note_table.set("type", current.note_type.as_str())?;
        note_table.set("content", current.content.as_str())?;

        if let Some(title) = &current.title {
            note_table.set("title", title.as_str())?;
        }

        if let Some(fm) = &current.frontmatter {
            let fm_table = yaml_to_lua_table(lua, fm)?;
            note_table.set("frontmatter", fm_table)?;
        }

        Ok(Value::Table(note_table))
    })
}

/// Create the `mdv.backlinks(path)` function.
///
/// Returns a list of notes that link to the specified path.
///
/// # Examples (in Lua)
///
/// ```lua
/// local links = mdv.backlinks("projects/my-project.md")
/// for _, link in ipairs(links) do
///     print(link.source_path .. " links to this note")
/// end
/// ```
fn create_backlinks_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, path: String| {
        let ctx = lua
            .app_data_ref::<VaultContext>()
            .ok_or_else(|| mlua::Error::runtime("VaultContext not available"))?;

        let db = match &ctx.index_db {
            Some(db) => db,
            None => {
                return Err(mlua::Error::runtime(
                    "Index database not available. Run 'mdv reindex' first.",
                ))
            }
        };

        // Resolve path
        let resolved_path = resolve_note_path(&ctx.vault_root, &path);

        // Get note ID
        let note = match db.get_note_by_path(Path::new(&resolved_path)) {
            Ok(Some(n)) => n,
            Ok(None) => {
                // Return empty table if note not found
                return Ok(Value::Table(lua.create_table()?));
            }
            Err(e) => return Err(mlua::Error::runtime(format!("Index error: {}", e))),
        };

        let note_id = match note.id {
            Some(id) => id,
            None => return Ok(Value::Table(lua.create_table()?)),
        };

        // Get backlinks
        let backlinks = db
            .get_backlinks(note_id)
            .map_err(|e| mlua::Error::runtime(format!("Index error: {}", e)))?;

        // Convert to Lua table
        let result = lua.create_table()?;
        for (i, link) in backlinks.iter().enumerate() {
            let link_table = lua.create_table()?;

            // Get source note path
            if let Ok(Some(source_note)) = db.get_note_by_id(link.source_id) {
                link_table.set("source_path", source_note.path.to_string_lossy().to_string())?;
                link_table.set("source_title", source_note.title)?;
                link_table.set("source_type", source_note.note_type.as_str())?;
            }

            if let Some(text) = &link.link_text {
                link_table.set("link_text", text.as_str())?;
            }
            if let Some(context) = &link.context {
                link_table.set("context", context.as_str())?;
            }
            link_table.set("link_type", link.link_type.as_str())?;

            result.set(i + 1, link_table)?;
        }

        Ok(Value::Table(result))
    })
}

/// Create the `mdv.outlinks(path)` function.
///
/// Returns a list of notes that the specified path links to.
///
/// # Examples (in Lua)
///
/// ```lua
/// local links = mdv.outlinks("projects/my-project.md")
/// for _, link in ipairs(links) do
///     print("Links to: " .. link.target_path)
/// end
/// ```
fn create_outlinks_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, path: String| {
        let ctx = lua
            .app_data_ref::<VaultContext>()
            .ok_or_else(|| mlua::Error::runtime("VaultContext not available"))?;

        let db = match &ctx.index_db {
            Some(db) => db,
            None => {
                return Err(mlua::Error::runtime(
                    "Index database not available. Run 'mdv reindex' first.",
                ))
            }
        };

        // Resolve path
        let resolved_path = resolve_note_path(&ctx.vault_root, &path);

        // Get note ID
        let note = match db.get_note_by_path(Path::new(&resolved_path)) {
            Ok(Some(n)) => n,
            Ok(None) => {
                // Return empty table if note not found
                return Ok(Value::Table(lua.create_table()?));
            }
            Err(e) => return Err(mlua::Error::runtime(format!("Index error: {}", e))),
        };

        let note_id = match note.id {
            Some(id) => id,
            None => return Ok(Value::Table(lua.create_table()?)),
        };

        // Get outgoing links
        let outlinks = db
            .get_outgoing_links(note_id)
            .map_err(|e| mlua::Error::runtime(format!("Index error: {}", e)))?;

        // Convert to Lua table
        let result = lua.create_table()?;
        for (i, link) in outlinks.iter().enumerate() {
            let link_table = lua.create_table()?;

            link_table.set("target_path", link.target_path.as_str())?;

            // Get target note info if resolved
            if let Some(target_id) = link.target_id {
                if let Ok(Some(target_note)) = db.get_note_by_id(target_id) {
                    link_table.set("target_title", target_note.title)?;
                    link_table.set("target_type", target_note.note_type.as_str())?;
                    link_table.set("resolved", true)?;
                } else {
                    link_table.set("resolved", false)?;
                }
            } else {
                link_table.set("resolved", false)?;
            }

            if let Some(text) = &link.link_text {
                link_table.set("link_text", text.as_str())?;
            }
            link_table.set("link_type", link.link_type.as_str())?;

            result.set(i + 1, link_table)?;
        }

        Ok(Value::Table(result))
    })
}

/// Create the `mdv.query(opts)` function.
///
/// Query the vault index with filters.
///
/// # Examples (in Lua)
///
/// ```lua
/// -- Find all open tasks
/// local tasks = mdv.query({ type = "task" })
/// for _, note in ipairs(tasks) do
///     print(note.path .. ": " .. note.title)
/// end
///
/// -- Find recent notes
/// local recent = mdv.query({ limit = 10 })
/// ```
fn create_query_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, opts: Option<Table>| {
        let ctx = lua
            .app_data_ref::<VaultContext>()
            .ok_or_else(|| mlua::Error::runtime("VaultContext not available"))?;

        let db = match &ctx.index_db {
            Some(db) => db,
            None => {
                return Err(mlua::Error::runtime(
                    "Index database not available. Run 'mdv reindex' first.",
                ))
            }
        };

        // Build query from options
        let mut query = NoteQuery::default();

        if let Some(opts) = opts {
            // Type filter
            if let Ok(type_str) = opts.get::<String>("type") {
                query.note_type = Some(type_str.parse().unwrap_or_default());
            }

            // Path prefix filter
            if let Ok(prefix) = opts.get::<String>("path_prefix") {
                query.path_prefix = Some(std::path::PathBuf::from(prefix));
            }

            // Limit
            if let Ok(limit) = opts.get::<i64>("limit") {
                query.limit = Some(limit as u32);
            }

            // Offset
            if let Ok(offset) = opts.get::<i64>("offset") {
                query.offset = Some(offset as u32);
            }
        }

        // Execute query
        let notes = db
            .query_notes(&query)
            .map_err(|e| mlua::Error::runtime(format!("Query error: {}", e)))?;

        // Convert to Lua table
        let result = lua.create_table()?;
        for (i, note) in notes.iter().enumerate() {
            let note_table = lua.create_table()?;
            note_table.set("path", note.path.to_string_lossy().to_string())?;
            note_table.set("type", note.note_type.as_str())?;
            note_table.set("title", note.title.clone())?;
            note_table.set("modified", note.modified.to_rfc3339())?;

            if let Some(created) = note.created {
                note_table.set("created", created.to_rfc3339())?;
            }

            // Parse and include frontmatter if available
            if let Some(fm_json) = &note.frontmatter_json
                && let Ok(fm) = serde_json::from_str::<serde_json::Value>(fm_json)
            {
                let fm_yaml = json_to_yaml(&fm);
                let fm_lua = yaml_to_lua_table(lua, &fm_yaml)?;
                note_table.set("frontmatter", fm_lua)?;
            }

            result.set(i + 1, note_table)?;
        }

        Ok(Value::Table(result))
    })
}

/// Resolve a note path relative to vault root.
fn resolve_note_path(_vault_root: &std::path::Path, path: &str) -> String {
    // If path doesn't end with .md, append it
    if path.ends_with(".md") { path.to_string() } else { format!("{}.md", path) }
}

/// Convert serde_json::Value to serde_yaml::Value.
fn json_to_yaml(json: &serde_json::Value) -> serde_yaml::Value {
    match json {
        serde_json::Value::Null => serde_yaml::Value::Null,
        serde_json::Value::Bool(b) => serde_yaml::Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_yaml::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_yaml::Value::Number(serde_yaml::Number::from(f))
            } else {
                serde_yaml::Value::Null
            }
        }
        serde_json::Value::String(s) => serde_yaml::Value::String(s.clone()),
        serde_json::Value::Array(arr) => {
            serde_yaml::Value::Sequence(arr.iter().map(json_to_yaml).collect())
        }
        serde_json::Value::Object(obj) => {
            let mut map = serde_yaml::Mapping::new();
            for (k, v) in obj {
                map.insert(serde_yaml::Value::String(k.clone()), json_to_yaml(v));
            }
            serde_yaml::Value::Mapping(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_note_path_with_extension() {
        let vault_root = std::path::Path::new("/vault");
        let result = resolve_note_path(vault_root, "notes/test.md");
        assert_eq!(result, "notes/test.md");
    }

    #[test]
    fn test_resolve_note_path_without_extension() {
        let vault_root = std::path::Path::new("/vault");
        let result = resolve_note_path(vault_root, "notes/test");
        assert_eq!(result, "notes/test.md");
    }

    #[test]
    fn test_json_to_yaml() {
        let json = serde_json::json!({
            "string": "value",
            "number": 42,
            "bool": true,
            "array": [1, 2, 3]
        });

        let yaml = json_to_yaml(&json);

        if let serde_yaml::Value::Mapping(map) = yaml {
            assert!(map.contains_key(&serde_yaml::Value::String("string".into())));
            assert!(map.contains_key(&serde_yaml::Value::String("number".into())));
        } else {
            panic!("Expected mapping");
        }
    }
}
