//! Capture lifecycle hooks execution.
//!
//! This module provides support for running before_insert and after_insert hooks
//! defined in Lua capture specifications.

use std::collections::HashMap;

use crate::scripting::{LuaEngine, ScriptingError};

use super::types::CaptureSpec;

/// Result of running a before_insert hook.
#[derive(Debug)]
pub struct BeforeInsertResult {
    /// Modified content to insert (or original if unchanged)
    pub content: String,
}

/// Run the before_insert hook if defined.
///
/// The hook receives:
/// - content: The rendered content template
/// - vars: Table of all variables
/// - target: Table with file, section, position
///
/// Returns: Modified content string
pub fn run_before_insert_hook(
    spec: &CaptureSpec,
    content: &str,
    vars: &HashMap<String, String>,
) -> Result<BeforeInsertResult, ScriptingError> {
    // If no hook defined, return content unchanged
    if !spec.has_before_insert {
        return Ok(BeforeInsertResult { content: content.to_string() });
    }

    // Get Lua source
    let lua_source = spec
        .lua_source
        .as_ref()
        .ok_or_else(|| ScriptingError::Other("Capture has no Lua source".to_string()))?;

    // Create Lua engine
    let engine = LuaEngine::sandboxed()?;
    let lua = engine.lua();

    // Execute the capture definition to get the table
    let capture_table: mlua::Table =
        lua.load(lua_source).eval().map_err(ScriptingError::Lua)?;

    // Get the before_insert function
    let hook_fn: mlua::Function = capture_table
        .get("before_insert")
        .map_err(ScriptingError::Lua)?;

    // Build vars table
    let vars_table = lua.create_table().map_err(ScriptingError::Lua)?;
    for (k, v) in vars {
        vars_table.set(k.as_str(), v.as_str()).map_err(ScriptingError::Lua)?;
    }

    // Build target table
    let target_table = lua.create_table().map_err(ScriptingError::Lua)?;
    target_table
        .set("file", spec.target.file.as_str())
        .map_err(ScriptingError::Lua)?;
    if let Some(section) = &spec.target.section {
        target_table.set("section", section.as_str()).map_err(ScriptingError::Lua)?;
    }
    let position_str = match spec.target.position {
        super::types::CapturePosition::Begin => "begin",
        super::types::CapturePosition::End => "end",
    };
    target_table.set("position", position_str).map_err(ScriptingError::Lua)?;

    // Call the hook: before_insert(content, vars, target)
    let result: mlua::Value = hook_fn
        .call((content, vars_table, target_table))
        .map_err(ScriptingError::Lua)?;

    // Extract result - should be a string (modified content)
    let modified_content = match result {
        mlua::Value::String(s) => s.to_str().map_err(ScriptingError::Lua)?.to_string(),
        mlua::Value::Nil => content.to_string(), // Hook returned nil, use original
        _ => {
            return Err(ScriptingError::Other(
                "before_insert hook must return a string or nil".to_string(),
            ));
        }
    };

    Ok(BeforeInsertResult { content: modified_content })
}

/// Result of running an after_insert hook.
#[derive(Debug)]
pub struct AfterInsertResult {
    /// Whether the hook ran successfully
    pub success: bool,
}

/// Run the after_insert hook if defined.
///
/// The hook receives:
/// - content: The content that was inserted
/// - vars: Table of all variables
/// - target: Table with file, section, position
/// - result: Table with target_file path and success status
///
/// Returns: Nothing (hook is for side effects only)
pub fn run_after_insert_hook(
    spec: &CaptureSpec,
    content: &str,
    vars: &HashMap<String, String>,
    target_file: &std::path::Path,
    section_matched: Option<(&str, u8)>,
) -> Result<AfterInsertResult, ScriptingError> {
    // If no hook defined, return success
    if !spec.has_after_insert {
        return Ok(AfterInsertResult { success: true });
    }

    // Get Lua source
    let lua_source = spec
        .lua_source
        .as_ref()
        .ok_or_else(|| ScriptingError::Other("Capture has no Lua source".to_string()))?;

    // Create Lua engine
    let engine = LuaEngine::sandboxed()?;
    let lua = engine.lua();

    // Execute the capture definition to get the table
    let capture_table: mlua::Table =
        lua.load(lua_source).eval().map_err(ScriptingError::Lua)?;

    // Get the after_insert function
    let hook_fn: mlua::Function = capture_table
        .get("after_insert")
        .map_err(ScriptingError::Lua)?;

    // Build vars table
    let vars_table = lua.create_table().map_err(ScriptingError::Lua)?;
    for (k, v) in vars {
        vars_table.set(k.as_str(), v.as_str()).map_err(ScriptingError::Lua)?;
    }

    // Build target table
    let target_table = lua.create_table().map_err(ScriptingError::Lua)?;
    target_table
        .set("file", spec.target.file.as_str())
        .map_err(ScriptingError::Lua)?;
    if let Some(section) = &spec.target.section {
        target_table.set("section", section.as_str()).map_err(ScriptingError::Lua)?;
    }
    let position_str = match spec.target.position {
        super::types::CapturePosition::Begin => "begin",
        super::types::CapturePosition::End => "end",
    };
    target_table.set("position", position_str).map_err(ScriptingError::Lua)?;

    // Build result table
    let result_table = lua.create_table().map_err(ScriptingError::Lua)?;
    result_table
        .set("target_file", target_file.to_string_lossy().as_ref())
        .map_err(ScriptingError::Lua)?;
    result_table.set("success", true).map_err(ScriptingError::Lua)?;
    if let Some((section_title, level)) = section_matched {
        result_table.set("section_title", section_title).map_err(ScriptingError::Lua)?;
        result_table.set("section_level", level).map_err(ScriptingError::Lua)?;
    }

    // Call the hook: after_insert(content, vars, target, result)
    let _: mlua::Value = hook_fn
        .call((content, vars_table, target_table, result_table))
        .map_err(ScriptingError::Lua)?;

    Ok(AfterInsertResult { success: true })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::captures::lua_loader::load_capture_from_lua;
    use std::fs;
    use tempfile::TempDir;

    fn write_lua_capture(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(format!("{}.lua", name));
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_before_insert_hook_modifies_content() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_capture(
            temp.path(),
            "test",
            r#"
return {
    name = "test",
    target = { file = "test.md", section = "Test" },
    content = "- {{text}}",
    before_insert = function(content, vars, target)
        return "[HOOK] " .. content
    end,
}
"#,
        );

        let spec = load_capture_from_lua(&path).unwrap();
        assert!(spec.has_before_insert);

        let vars: HashMap<String, String> = [("text".into(), "hello".into())].into();
        let result = run_before_insert_hook(&spec, "- hello", &vars).unwrap();

        assert_eq!(result.content, "[HOOK] - hello");
    }

    #[test]
    fn test_before_insert_hook_returns_nil() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_capture(
            temp.path(),
            "test",
            r#"
return {
    name = "test",
    target = { file = "test.md", section = "Test" },
    content = "- {{text}}",
    before_insert = function(content, vars, target)
        return nil -- Let original content through
    end,
}
"#,
        );

        let spec = load_capture_from_lua(&path).unwrap();
        let vars: HashMap<String, String> = [("text".into(), "hello".into())].into();
        let result = run_before_insert_hook(&spec, "- hello", &vars).unwrap();

        assert_eq!(result.content, "- hello");
    }

    #[test]
    fn test_no_hook_passes_through() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_capture(
            temp.path(),
            "test",
            r#"
return {
    name = "test",
    target = { file = "test.md", section = "Test" },
    content = "- {{text}}",
}
"#,
        );

        let spec = load_capture_from_lua(&path).unwrap();
        assert!(!spec.has_before_insert);

        let vars: HashMap<String, String> = [("text".into(), "hello".into())].into();
        let result = run_before_insert_hook(&spec, "- hello", &vars).unwrap();

        assert_eq!(result.content, "- hello");
    }

    #[test]
    fn test_after_insert_hook_runs() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_capture(
            temp.path(),
            "test",
            r#"
return {
    name = "test",
    target = { file = "test.md", section = "Test" },
    content = "- {{text}}",
    after_insert = function(content, vars, target, result)
        -- Side effect only, return value ignored
        print("Inserted: " .. content)
    end,
}
"#,
        );

        let spec = load_capture_from_lua(&path).unwrap();
        assert!(spec.has_after_insert);

        let vars: HashMap<String, String> = [("text".into(), "hello".into())].into();
        let result = run_after_insert_hook(
            &spec,
            "- hello",
            &vars,
            std::path::Path::new("/tmp/test.md"),
            Some(("Test", 2)),
        )
        .unwrap();

        assert!(result.success);
    }
}
