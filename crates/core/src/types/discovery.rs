//! Type definition discovery and loading.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use super::definition::{TypeDefinition, TypedefInfo};
use super::errors::TypedefError;
use super::schema::{FieldSchema, FieldType};
use crate::scripting::LuaEngine;

/// Built-in type names that can be overridden by Lua definitions.
const BUILTIN_TYPES: &[&str] = &["daily", "weekly", "task", "project", "zettel"];

/// Discover type definition files in a directory.
///
/// Finds all `.lua` files in the given directory (non-recursive).
/// Returns an empty list if the directory doesn't exist.
pub fn discover_typedefs(root: &Path) -> Result<Vec<TypedefInfo>, TypedefError> {
    // Gracefully handle missing directory
    if !root.exists() {
        return Ok(vec![]);
    }

    let root = root
        .canonicalize()
        .map_err(|_| TypedefError::MissingDir(root.display().to_string()))?;

    let mut out = Vec::new();

    // Only look at direct children (max_depth = 1)
    for entry in WalkDir::new(&root).max_depth(1) {
        let entry =
            entry.map_err(|e| TypedefError::WalkError(root.display().to_string(), e))?;

        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !is_lua_file(path) {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        if !name.is_empty() {
            out.push(TypedefInfo::new(name, path.to_path_buf()));
        }
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn is_lua_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("lua")
}

/// Repository for discovering and loading type definitions.
pub struct TypedefRepository {
    /// Root directory for type definitions.
    pub root: PathBuf,
    /// Discovered type definition files.
    pub typedefs: Vec<TypedefInfo>,
}

impl TypedefRepository {
    /// Create a new repository from a directory.
    ///
    /// Returns an empty repository if the directory doesn't exist.
    pub fn new(root: &Path) -> Result<Self, TypedefError> {
        let typedefs = discover_typedefs(root)?;
        Ok(Self {
            root: root.to_path_buf(),
            typedefs,
        })
    }

    /// List all discovered type definitions.
    pub fn list_all(&self) -> &[TypedefInfo] {
        &self.typedefs
    }

    /// Check if a type definition exists.
    pub fn has_typedef(&self, name: &str) -> bool {
        self.typedefs.iter().any(|t| t.name == name)
    }

    /// Load a type definition by name.
    pub fn load_typedef(&self, name: &str) -> Result<TypeDefinition, TypedefError> {
        let info = self
            .typedefs
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| TypedefError::NotFound(name.to_string()))?;

        load_typedef_from_file(&info.path)
    }

    /// Load all type definitions.
    pub fn load_all(&self) -> Result<Vec<TypeDefinition>, TypedefError> {
        let mut result = Vec::new();
        for info in &self.typedefs {
            result.push(load_typedef_from_file(&info.path)?);
        }
        Ok(result)
    }
}

/// Load and parse a type definition from a Lua file.
pub fn load_typedef_from_file(path: &Path) -> Result<TypeDefinition, TypedefError> {
    let source = fs::read_to_string(path).map_err(|e| TypedefError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    parse_typedef(&name, &source, path)
}

/// Parse a type definition from Lua source.
fn parse_typedef(name: &str, source: &str, path: &Path) -> Result<TypeDefinition, TypedefError> {
    let engine = LuaEngine::sandboxed().map_err(|e| TypedefError::LuaParse {
        path: path.to_path_buf(),
        source: e,
    })?;

    let lua = engine.lua();

    // Execute the Lua file - it should return a table
    let value: mlua::Value = lua.load(source).eval().map_err(|e| TypedefError::LuaParse {
        path: path.to_path_buf(),
        source: crate::scripting::ScriptingError::Lua(e),
    })?;

    let table = match value {
        mlua::Value::Table(t) => t,
        _ => {
            return Err(TypedefError::InvalidDefinition {
                path: path.to_path_buf(),
                message: "Type definition must return a table".to_string(),
            })
        }
    };

    // Extract optional description
    let description: Option<String> = table.get("description").ok();

    // Extract schema
    let schema = extract_schema(&table, path)?;

    // Check for hook functions
    let has_validate_fn = table.get::<mlua::Function>("validate").is_ok();
    let has_on_create_hook = table.get::<mlua::Function>("on_create").is_ok();
    let has_on_update_hook = table.get::<mlua::Function>("on_update").is_ok();

    // Check if this overrides a built-in
    let is_builtin_override = BUILTIN_TYPES.contains(&name);

    Ok(TypeDefinition {
        name: name.to_string(),
        description,
        source_path: path.to_path_buf(),
        schema,
        has_validate_fn,
        has_on_create_hook,
        has_on_update_hook,
        is_builtin_override,
        lua_source: source.to_string(),
    })
}

/// Extract schema from Lua table.
fn extract_schema(
    table: &mlua::Table,
    path: &Path,
) -> Result<HashMap<String, FieldSchema>, TypedefError> {
    let mut schema = HashMap::new();

    let schema_table: mlua::Table = match table.get("schema") {
        Ok(t) => t,
        Err(_) => return Ok(schema), // No schema defined is valid
    };

    for pair in schema_table.pairs::<String, mlua::Table>() {
        let (field_name, field_def) =
            pair.map_err(|e| TypedefError::LuaParse {
                path: path.to_path_buf(),
                source: crate::scripting::ScriptingError::Lua(e),
            })?;

        let field_schema = parse_field_schema(&field_def, &field_name, path)?;
        schema.insert(field_name, field_schema);
    }

    Ok(schema)
}

/// Parse a field schema from a Lua table.
fn parse_field_schema(
    table: &mlua::Table,
    field_name: &str,
    _path: &Path,
) -> Result<FieldSchema, TypedefError> {
    // Get field type
    let field_type: Option<FieldType> = table
        .get::<String>("type")
        .ok()
        .and_then(|s| s.parse().ok());

    // Get required flag
    let required: bool = table.get("required").unwrap_or(false);

    // Get description
    let description: Option<String> = table.get("description").ok();

    // Get default value (convert Lua value to serde_yaml::Value)
    let default: Option<serde_yaml::Value> = table
        .get::<mlua::Value>("default")
        .ok()
        .and_then(|v| lua_to_yaml_value(&v));

    // Get enum values
    let enum_values: Option<Vec<String>> = table.get::<mlua::Table>("enum").ok().map(|t| {
        t.pairs::<i64, String>()
            .filter_map(|r| r.ok())
            .map(|(_, v)| v)
            .collect()
    });

    // Get string constraints
    let pattern: Option<String> = table.get("pattern").ok();
    let min_length: Option<usize> = table.get::<i64>("min_length").ok().map(|v| v as usize);
    let max_length: Option<usize> = table.get::<i64>("max_length").ok().map(|v| v as usize);

    // Get number constraints
    let min: Option<f64> = table.get("min").ok();
    let max: Option<f64> = table.get("max").ok();
    let integer: Option<bool> = table.get("integer").ok();

    // Get list constraints
    let min_items: Option<usize> = table.get::<i64>("min_items").ok().map(|v| v as usize);
    let max_items: Option<usize> = table.get::<i64>("max_items").ok().map(|v| v as usize);

    // Get nested items schema (for lists)
    let items: Option<Box<FieldSchema>> = table
        .get::<mlua::Table>("items")
        .ok()
        .map(|t| parse_field_schema(&t, &format!("{}[]", field_name), _path))
        .transpose()?
        .map(Box::new);

    // Get reference constraint
    let note_type: Option<String> = table.get("note_type").ok();

    Ok(FieldSchema {
        field_type,
        required,
        description,
        default,
        enum_values,
        pattern,
        min_length,
        max_length,
        min,
        max,
        integer,
        items,
        min_items,
        max_items,
        note_type,
    })
}

/// Convert a Lua value to a serde_yaml::Value.
fn lua_to_yaml_value(value: &mlua::Value) -> Option<serde_yaml::Value> {
    match value {
        mlua::Value::Nil => Some(serde_yaml::Value::Null),
        mlua::Value::Boolean(b) => Some(serde_yaml::Value::Bool(*b)),
        mlua::Value::Integer(i) => Some(serde_yaml::Value::Number((*i).into())),
        mlua::Value::Number(n) => {
            // Convert float to integer if it's a whole number, otherwise use string representation
            if n.fract() == 0.0 {
                Some(serde_yaml::Value::Number((*n as i64).into()))
            } else {
                Some(serde_yaml::Value::String(n.to_string()))
            }
        }
        mlua::Value::String(s) => s.to_str().ok().map(|s| serde_yaml::Value::String(s.to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_discover_typedefs_empty_dir() {
        let temp = TempDir::new().unwrap();
        let types_dir = temp.path().join("types");
        fs::create_dir_all(&types_dir).unwrap();

        let typedefs = discover_typedefs(&types_dir).unwrap();
        assert!(typedefs.is_empty());
    }

    #[test]
    fn test_discover_typedefs_missing_dir() {
        let temp = TempDir::new().unwrap();
        let types_dir = temp.path().join("nonexistent");

        // Should return empty list, not error
        let typedefs = discover_typedefs(&types_dir).unwrap();
        assert!(typedefs.is_empty());
    }

    #[test]
    fn test_discover_typedefs() {
        let temp = TempDir::new().unwrap();
        let types_dir = temp.path().join("types");
        fs::create_dir_all(&types_dir).unwrap();

        // Create some type definition files
        fs::write(types_dir.join("meeting.lua"), "return { schema = {} }").unwrap();
        fs::write(types_dir.join("project.lua"), "return { schema = {} }").unwrap();

        // Create a non-lua file (should be ignored)
        fs::write(types_dir.join("readme.md"), "# Types").unwrap();

        let typedefs = discover_typedefs(&types_dir).unwrap();

        assert_eq!(typedefs.len(), 2);
        assert!(typedefs.iter().any(|t| t.name == "meeting"));
        assert!(typedefs.iter().any(|t| t.name == "project"));
    }

    #[test]
    fn test_load_simple_typedef() {
        let temp = TempDir::new().unwrap();
        let types_dir = temp.path().join("types");
        fs::create_dir_all(&types_dir).unwrap();

        fs::write(
            types_dir.join("meeting.lua"),
            r#"
return {
    description = "Meeting notes",
    schema = {
        title = { type = "string", required = true },
        date = { type = "date", required = true },
        attendees = { type = "list" },
    }
}
"#,
        )
        .unwrap();

        let repo = TypedefRepository::new(&types_dir).unwrap();
        let typedef = repo.load_typedef("meeting").unwrap();

        assert_eq!(typedef.name, "meeting");
        assert_eq!(typedef.description, Some("Meeting notes".to_string()));
        assert_eq!(typedef.schema.len(), 3);
        assert!(typedef.schema.contains_key("title"));
        assert!(typedef.schema.get("title").unwrap().required);
        assert!(!typedef.has_validate_fn);
        assert!(!typedef.has_on_create_hook);
    }

    #[test]
    fn test_load_typedef_with_hooks() {
        let temp = TempDir::new().unwrap();
        let types_dir = temp.path().join("types");
        fs::create_dir_all(&types_dir).unwrap();

        fs::write(
            types_dir.join("task.lua"),
            r#"
return {
    schema = {
        status = { type = "string", enum = { "open", "done" } },
    },
    validate = function(note)
        return true
    end,
    on_create = function(note)
        return note
    end,
    on_update = function(note, previous)
        return note
    end
}
"#,
        )
        .unwrap();

        let repo = TypedefRepository::new(&types_dir).unwrap();
        let typedef = repo.load_typedef("task").unwrap();

        assert!(typedef.has_validate_fn);
        assert!(typedef.has_on_create_hook);
        assert!(typedef.has_on_update_hook);
        assert!(typedef.is_builtin_override); // "task" is a built-in
    }

    #[test]
    fn test_load_typedef_with_enum() {
        let temp = TempDir::new().unwrap();
        let types_dir = temp.path().join("types");
        fs::create_dir_all(&types_dir).unwrap();

        fs::write(
            types_dir.join("status.lua"),
            r#"
return {
    schema = {
        priority = {
            type = "string",
            enum = { "low", "medium", "high" },
            default = "medium"
        },
    }
}
"#,
        )
        .unwrap();

        let repo = TypedefRepository::new(&types_dir).unwrap();
        let typedef = repo.load_typedef("status").unwrap();

        let priority = typedef.schema.get("priority").unwrap();
        assert_eq!(
            priority.enum_values,
            Some(vec!["low".to_string(), "medium".to_string(), "high".to_string()])
        );
        assert_eq!(
            priority.default,
            Some(serde_yaml::Value::String("medium".to_string()))
        );
    }

    #[test]
    fn test_load_typedef_with_number_constraints() {
        let temp = TempDir::new().unwrap();
        let types_dir = temp.path().join("types");
        fs::create_dir_all(&types_dir).unwrap();

        fs::write(
            types_dir.join("meeting.lua"),
            r#"
return {
    schema = {
        duration_minutes = {
            type = "number",
            min = 1,
            max = 480,
            integer = true
        },
    }
}
"#,
        )
        .unwrap();

        let repo = TypedefRepository::new(&types_dir).unwrap();
        let typedef = repo.load_typedef("meeting").unwrap();

        let duration = typedef.schema.get("duration_minutes").unwrap();
        assert_eq!(duration.min, Some(1.0));
        assert_eq!(duration.max, Some(480.0));
        assert_eq!(duration.integer, Some(true));
    }

    #[test]
    fn test_typedef_not_found() {
        let temp = TempDir::new().unwrap();
        let types_dir = temp.path().join("types");
        fs::create_dir_all(&types_dir).unwrap();

        let repo = TypedefRepository::new(&types_dir).unwrap();
        let result = repo.load_typedef("nonexistent");

        assert!(matches!(result, Err(TypedefError::NotFound(_))));
    }

    #[test]
    fn test_invalid_typedef_not_table() {
        let temp = TempDir::new().unwrap();
        let types_dir = temp.path().join("types");
        fs::create_dir_all(&types_dir).unwrap();

        fs::write(types_dir.join("invalid.lua"), r#"return "not a table""#).unwrap();

        let repo = TypedefRepository::new(&types_dir).unwrap();
        let result = repo.load_typedef("invalid");

        assert!(matches!(result, Err(TypedefError::InvalidDefinition { .. })));
    }
}
