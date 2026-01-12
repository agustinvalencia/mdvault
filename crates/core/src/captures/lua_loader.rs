//! Lua-based capture specification loading.
//!
//! This module provides support for loading capture specifications from Lua files,
//! following the same pattern as type definitions.

use std::path::Path;

use crate::frontmatter::{FrontmatterOp, FrontmatterOpType, FrontmatterOps};
use crate::scripting::{LuaEngine, ScriptingError};
use crate::vars::{VarMetadata, VarSpec, VarsMap};

use super::types::{CapturePosition, CaptureRepoError, CaptureSpec, CaptureTarget};

/// Load and parse a capture specification from a Lua file.
pub fn load_capture_from_lua(path: &Path) -> Result<CaptureSpec, CaptureRepoError> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| CaptureRepoError::Io { path: path.to_path_buf(), source: e })?;

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    parse_capture_lua(&name, &source, path)
}

/// Parse a capture specification from Lua source.
fn parse_capture_lua(
    name: &str,
    source: &str,
    path: &Path,
) -> Result<CaptureSpec, CaptureRepoError> {
    let engine = LuaEngine::sandboxed().map_err(|e| CaptureRepoError::LuaParse {
        path: path.to_path_buf(),
        source: e,
    })?;

    let lua = engine.lua();

    // Execute the Lua file - it should return a table
    let value: mlua::Value =
        lua.load(source).eval().map_err(|e| CaptureRepoError::LuaParse {
            path: path.to_path_buf(),
            source: ScriptingError::Lua(e),
        })?;

    let table = match value {
        mlua::Value::Table(t) => t,
        _ => {
            return Err(CaptureRepoError::LuaInvalid {
                path: path.to_path_buf(),
                message: "Capture definition must return a table".to_string(),
            });
        }
    };

    // Extract name (optional, defaults to filename)
    let capture_name: String = table.get("name").unwrap_or_else(|_| name.to_string());

    // Extract description
    let description: String = table.get("description").unwrap_or_default();

    // Extract vars
    let vars = extract_vars(&table, path)?;

    // Extract target (required)
    let target = extract_target(&table, path)?;

    // Extract content (optional)
    let content: Option<String> = table.get("content").ok();

    // Extract frontmatter operations (optional)
    let frontmatter = extract_frontmatter(&table, path)?;

    Ok(CaptureSpec {
        name: capture_name,
        description,
        vars: if vars.is_empty() { None } else { Some(vars) },
        target,
        content,
        frontmatter,
    })
}

/// Extract vars from Lua table.
///
/// Supports two formats:
/// - Simple: `text = "What to capture?"` (prompt string)
/// - Full: `text = { prompt = "...", default = "...", required = true }`
fn extract_vars(table: &mlua::Table, path: &Path) -> Result<VarsMap, CaptureRepoError> {
    let mut vars = VarsMap::new();

    let vars_table: mlua::Table = match table.get("vars") {
        Ok(t) => t,
        Err(_) => return Ok(vars), // No vars defined is valid
    };

    for pair in vars_table.pairs::<String, mlua::Value>() {
        let (var_name, var_value) = pair.map_err(|e| CaptureRepoError::LuaParse {
            path: path.to_path_buf(),
            source: ScriptingError::Lua(e),
        })?;

        let var_spec = match var_value {
            // Simple form: variable = "prompt?"
            mlua::Value::String(s) => {
                let value = s.to_str().map_err(|e| CaptureRepoError::LuaParse {
                    path: path.to_path_buf(),
                    source: ScriptingError::Lua(e),
                })?;
                VarSpec::Simple(value.to_string())
            }
            // Full form: variable = { prompt = "...", default = "..." }
            mlua::Value::Table(t) => {
                let prompt: Option<String> = t.get("prompt").ok();
                let default: Option<String> = t.get("default").ok();
                let required: Option<bool> = t.get("required").ok();
                let description: Option<String> = t.get("description").ok();

                VarSpec::Full(VarMetadata { prompt, description, required, default })
            }
            _ => continue, // Skip invalid values
        };

        vars.insert(var_name, var_spec);
    }

    Ok(vars)
}

/// Extract target configuration from Lua table.
fn extract_target(table: &mlua::Table, path: &Path) -> Result<CaptureTarget, CaptureRepoError> {
    let target_table: mlua::Table =
        table.get("target").map_err(|_| CaptureRepoError::LuaInvalid {
            path: path.to_path_buf(),
            message: "Capture must have a 'target' field".to_string(),
        })?;

    let file: String = target_table.get("file").map_err(|_| CaptureRepoError::LuaInvalid {
        path: path.to_path_buf(),
        message: "target.file is required".to_string(),
    })?;

    let section: Option<String> = target_table.get("section").ok();

    let position: CapturePosition = target_table
        .get::<String>("position")
        .ok()
        .map(|s| match s.to_lowercase().as_str() {
            "end" => CapturePosition::End,
            _ => CapturePosition::Begin,
        })
        .unwrap_or_default();

    let create_if_missing: bool = target_table.get("create_if_missing").unwrap_or(false);

    Ok(CaptureTarget { file, section, position, create_if_missing })
}

/// Extract frontmatter operations from Lua table.
///
/// Supports two formats:
/// - Simple map: `{ status = "pending", count = 1 }` (implicit set)
/// - Operations list: `{ { field = "count", op = "increment" } }`
/// - Mixed: allows simple set operations alongside explicit operations
fn extract_frontmatter(
    table: &mlua::Table,
    path: &Path,
) -> Result<Option<FrontmatterOps>, CaptureRepoError> {
    let fm_value: mlua::Value = match table.get("frontmatter") {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    let fm_table = match fm_value {
        mlua::Value::Table(t) => t,
        mlua::Value::Nil => return Ok(None),
        _ => {
            return Err(CaptureRepoError::LuaInvalid {
                path: path.to_path_buf(),
                message: "frontmatter must be a table".to_string(),
            });
        }
    };

    // Determine table type by checking for array-style entries (integer keys starting at 1)
    // In Lua, arrays use integer keys: { [1] = ..., [2] = ... }
    // Maps use string keys: { foo = ..., bar = ... }
    let first_entry: Option<mlua::Value> = fm_table.get(1i64).ok();
    let is_sequence = first_entry.is_some()
        && !matches!(first_entry, Some(mlua::Value::Nil));

    // Also check for any string keys (for map or mixed forms)
    let mut has_string_keys = false;
    for pair in fm_table.pairs::<mlua::Value, mlua::Value>() {
        if let Ok((k, _)) = pair {
            if matches!(k, mlua::Value::String(_)) {
                has_string_keys = true;
                break;
            }
        }
    }

    if is_sequence && !has_string_keys {
        // Pure operations list (array of tables)
        let ops = extract_operations_list(&fm_table, path)?;
        Ok(Some(FrontmatterOps::Operations(ops)))
    } else if has_string_keys && !is_sequence {
        // Pure simple map
        let map = extract_simple_map(&fm_table, path)?;
        Ok(Some(FrontmatterOps::Simple(map)))
    } else if is_sequence && has_string_keys {
        // Mixed - convert simple map entries to operations and combine
        let mut ops = Vec::new();

        // First extract simple string key-value pairs as set operations
        for pair in fm_table.pairs::<mlua::Value, mlua::Value>() {
            let (key, value) = pair.map_err(|e| CaptureRepoError::LuaParse {
                path: path.to_path_buf(),
                source: ScriptingError::Lua(e),
            })?;

            // Only process string keys (skip numeric keys which are operations)
            if let mlua::Value::String(key_str) = key {
                let key_name = key_str.to_str().map_err(|e| CaptureRepoError::LuaParse {
                    path: path.to_path_buf(),
                    source: ScriptingError::Lua(e),
                })?;
                if let Some(yaml_value) = lua_to_yaml_value(&value) {
                    ops.push(FrontmatterOp {
                        field: key_name.to_string(),
                        op: FrontmatterOpType::Set,
                        value: Some(yaml_value),
                    });
                }
            }
        }

        // Then extract explicit operations from numeric keys
        for pair in fm_table.pairs::<i64, mlua::Table>() {
            if let Ok((_, op_table)) = pair {
                if let Some(op) = parse_operation(&op_table, path)? {
                    ops.push(op);
                }
            }
        }

        Ok(Some(FrontmatterOps::Operations(ops)))
    } else {
        // Empty table
        Ok(None)
    }
}

/// Extract a simple key-value map from Lua table.
fn extract_simple_map(
    table: &mlua::Table,
    path: &Path,
) -> Result<std::collections::HashMap<String, serde_yaml::Value>, CaptureRepoError> {
    let mut map = std::collections::HashMap::new();

    for pair in table.pairs::<String, mlua::Value>() {
        let (key, value) = pair.map_err(|e| CaptureRepoError::LuaParse {
            path: path.to_path_buf(),
            source: ScriptingError::Lua(e),
        })?;

        if let Some(yaml_value) = lua_to_yaml_value(&value) {
            map.insert(key, yaml_value);
        }
    }

    Ok(map)
}

/// Extract a list of explicit operations from Lua table.
fn extract_operations_list(
    table: &mlua::Table,
    path: &Path,
) -> Result<Vec<FrontmatterOp>, CaptureRepoError> {
    let mut ops = Vec::new();

    for pair in table.pairs::<i64, mlua::Table>() {
        let (_, op_table) = pair.map_err(|e| CaptureRepoError::LuaParse {
            path: path.to_path_buf(),
            source: ScriptingError::Lua(e),
        })?;

        if let Some(op) = parse_operation(&op_table, path)? {
            ops.push(op);
        }
    }

    Ok(ops)
}

/// Parse a single frontmatter operation from a Lua table.
fn parse_operation(
    table: &mlua::Table,
    path: &Path,
) -> Result<Option<FrontmatterOp>, CaptureRepoError> {
    let field: String = match table.get("field") {
        Ok(f) => f,
        Err(_) => return Ok(None), // Skip entries without field
    };

    let op_str: String = table.get("op").unwrap_or_else(|_| "set".to_string());
    let op = match op_str.to_lowercase().as_str() {
        "set" => FrontmatterOpType::Set,
        "toggle" => FrontmatterOpType::Toggle,
        "increment" => FrontmatterOpType::Increment,
        "append" => FrontmatterOpType::Append,
        _ => {
            return Err(CaptureRepoError::LuaInvalid {
                path: path.to_path_buf(),
                message: format!("Unknown frontmatter operation: {}", op_str),
            });
        }
    };

    let value: Option<serde_yaml::Value> =
        table.get::<mlua::Value>("value").ok().and_then(|v| lua_to_yaml_value(&v));

    Ok(Some(FrontmatterOp { field, op, value }))
}

/// Convert a Lua value to a serde_yaml::Value.
fn lua_to_yaml_value(value: &mlua::Value) -> Option<serde_yaml::Value> {
    match value {
        mlua::Value::Nil => None,
        mlua::Value::Boolean(b) => Some(serde_yaml::Value::Bool(*b)),
        mlua::Value::Integer(i) => Some(serde_yaml::Value::Number((*i).into())),
        mlua::Value::Number(n) => {
            if n.fract() == 0.0 {
                Some(serde_yaml::Value::Number((*n as i64).into()))
            } else {
                Some(serde_yaml::Value::String(n.to_string()))
            }
        }
        mlua::Value::String(s) => {
            s.to_str().ok().map(|s| serde_yaml::Value::String(s.to_string()))
        }
        mlua::Value::Table(t) => {
            // Check if it's a sequence (array) or a map
            let is_sequence = t.pairs::<i64, mlua::Value>().next().is_some()
                && t.pairs::<String, mlua::Value>().next().is_none();

            if is_sequence {
                let mut seq = Vec::new();
                for pair in t.pairs::<i64, mlua::Value>() {
                    if let Ok((_, v)) = pair {
                        if let Some(yaml_v) = lua_to_yaml_value(&v) {
                            seq.push(yaml_v);
                        }
                    }
                }
                Some(serde_yaml::Value::Sequence(seq))
            } else {
                let mut map = serde_yaml::Mapping::new();
                for pair in t.pairs::<String, mlua::Value>() {
                    if let Ok((k, v)) = pair {
                        if let Some(yaml_v) = lua_to_yaml_value(&v) {
                            map.insert(serde_yaml::Value::String(k), yaml_v);
                        }
                    }
                }
                Some(serde_yaml::Value::Mapping(map))
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_lua_capture(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(format!("{}.lua", name));
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_load_simple_capture() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_capture(
            temp.path(),
            "inbox",
            r#"
return {
    name = "inbox",
    description = "Add to inbox",
    target = {
        file = "daily/{{date}}.md",
        section = "Inbox",
    },
    content = "- [ ] {{text}}",
}
"#,
        );

        let spec = load_capture_from_lua(&path).unwrap();

        assert_eq!(spec.name, "inbox");
        assert_eq!(spec.description, "Add to inbox");
        assert_eq!(spec.target.file, "daily/{{date}}.md");
        assert_eq!(spec.target.section, Some("Inbox".to_string()));
        assert_eq!(spec.content, Some("- [ ] {{text}}".to_string()));
    }

    #[test]
    fn test_load_capture_with_vars() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_capture(
            temp.path(),
            "todo",
            r#"
return {
    name = "todo",
    vars = {
        text = "What to add?",
        priority = {
            prompt = "Priority level?",
            default = "medium",
        },
    },
    target = {
        file = "tasks.md",
        section = "TODO",
        position = "end",
    },
    content = "- [ ] {{text}} ({{priority}})",
}
"#,
        );

        let spec = load_capture_from_lua(&path).unwrap();

        assert!(spec.vars.is_some());
        let vars = spec.vars.unwrap();
        assert_eq!(vars.len(), 2);

        match &vars["text"] {
            VarSpec::Simple(s) => assert_eq!(s, "What to add?"),
            _ => panic!("Expected simple var"),
        }

        match &vars["priority"] {
            VarSpec::Full(m) => {
                assert_eq!(m.prompt, Some("Priority level?".to_string()));
                assert_eq!(m.default, Some("medium".to_string()));
            }
            _ => panic!("Expected full var"),
        }

        assert!(matches!(spec.target.position, CapturePosition::End));
    }

    #[test]
    fn test_load_capture_with_frontmatter_simple() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_capture(
            temp.path(),
            "status",
            r#"
return {
    name = "status",
    target = {
        file = "project.md",
    },
    frontmatter = {
        status = "active",
        priority = 1,
    },
}
"#,
        );

        let spec = load_capture_from_lua(&path).unwrap();

        match &spec.frontmatter {
            Some(FrontmatterOps::Simple(map)) => {
                assert_eq!(map.len(), 2);
                assert_eq!(map.get("status"), Some(&serde_yaml::Value::String("active".to_string())));
            }
            _ => panic!("Expected simple frontmatter"),
        }
    }

    #[test]
    fn test_load_capture_with_frontmatter_operations() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_capture(
            temp.path(),
            "counter",
            r#"
return {
    name = "counter",
    target = {
        file = "project.md",
    },
    frontmatter = {
        { field = "count", op = "increment" },
        { field = "active", op = "toggle" },
        { field = "tags", op = "append", value = "new-tag" },
    },
}
"#,
        );

        let spec = load_capture_from_lua(&path).unwrap();

        match &spec.frontmatter {
            Some(FrontmatterOps::Operations(ops)) => {
                assert_eq!(ops.len(), 3);
                assert_eq!(ops[0].field, "count");
                assert!(matches!(ops[0].op, FrontmatterOpType::Increment));
                assert_eq!(ops[1].field, "active");
                assert!(matches!(ops[1].op, FrontmatterOpType::Toggle));
                assert_eq!(ops[2].field, "tags");
                assert!(matches!(ops[2].op, FrontmatterOpType::Append));
            }
            _ => panic!("Expected operations frontmatter"),
        }
    }

    #[test]
    fn test_load_capture_with_create_if_missing() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_capture(
            temp.path(),
            "daily",
            r#"
return {
    name = "daily",
    target = {
        file = "daily/{{date}}.md",
        section = "Log",
        create_if_missing = true,
    },
    content = "- {{text}}",
}
"#,
        );

        let spec = load_capture_from_lua(&path).unwrap();
        assert!(spec.target.create_if_missing);
    }

    #[test]
    fn test_load_capture_missing_target() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_capture(
            temp.path(),
            "invalid",
            r#"
return {
    name = "invalid",
    content = "- {{text}}",
}
"#,
        );

        let result = load_capture_from_lua(&path);
        assert!(matches!(result, Err(CaptureRepoError::LuaInvalid { .. })));
    }

    #[test]
    fn test_load_capture_invalid_return() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_capture(temp.path(), "invalid", r#"return "not a table""#);

        let result = load_capture_from_lua(&path);
        assert!(matches!(result, Err(CaptureRepoError::LuaInvalid { .. })));
    }
}
