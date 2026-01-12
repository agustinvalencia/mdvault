//! Lua-based macro specification loading.
//!
//! This module provides support for loading macro specifications from Lua files,
//! following the same pattern as captures and type definitions.

use std::collections::HashMap;
use std::path::Path;

use crate::scripting::{LuaEngine, ScriptingError};
use crate::vars::{VarMetadata, VarSpec, VarsMap};

use super::discovery::MacroRepoError;
use super::types::{CaptureStep, ErrorPolicy, MacroSpec, MacroStep, ShellStep, TemplateStep};

/// Load and parse a macro specification from a Lua file.
pub fn load_macro_from_lua(path: &Path) -> Result<MacroSpec, MacroRepoError> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| MacroRepoError::Io { path: path.to_path_buf(), source: e })?;

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    parse_macro_lua(&name, &source, path)
}

/// Parse a macro specification from Lua source.
fn parse_macro_lua(name: &str, source: &str, path: &Path) -> Result<MacroSpec, MacroRepoError> {
    let engine = LuaEngine::sandboxed().map_err(|e| MacroRepoError::LuaParse {
        path: path.to_path_buf(),
        source: e,
    })?;

    let lua = engine.lua();

    // Execute the Lua file - it should return a table
    let value: mlua::Value =
        lua.load(source).eval().map_err(|e| MacroRepoError::LuaParse {
            path: path.to_path_buf(),
            source: ScriptingError::Lua(e),
        })?;

    let table = match value {
        mlua::Value::Table(t) => t,
        _ => {
            return Err(MacroRepoError::LuaInvalid {
                path: path.to_path_buf(),
                message: "Macro definition must return a table".to_string(),
            });
        }
    };

    // Extract name (optional, defaults to filename)
    let macro_name: String = table.get("name").unwrap_or_else(|_| name.to_string());

    // Extract description
    let description: String = table.get("description").unwrap_or_default();

    // Extract vars
    let vars = extract_vars(&table, path)?;

    // Extract steps (required)
    let steps = extract_steps(&table, path)?;

    // Extract on_error policy
    let on_error = extract_error_policy(&table);

    Ok(MacroSpec {
        name: macro_name,
        description,
        vars: if vars.is_empty() { None } else { Some(vars) },
        steps,
        on_error,
    })
}

/// Extract vars from Lua table.
fn extract_vars(table: &mlua::Table, path: &Path) -> Result<VarsMap, MacroRepoError> {
    let mut vars = VarsMap::new();

    let vars_table: mlua::Table = match table.get("vars") {
        Ok(t) => t,
        Err(_) => return Ok(vars), // No vars defined is valid
    };

    for pair in vars_table.pairs::<String, mlua::Value>() {
        let (var_name, var_value) = pair.map_err(|e| MacroRepoError::LuaParse {
            path: path.to_path_buf(),
            source: ScriptingError::Lua(e),
        })?;

        let var_spec = match var_value {
            // Simple form: variable = "prompt?"
            mlua::Value::String(s) => {
                let value = s.to_str().map_err(|e| MacroRepoError::LuaParse {
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

/// Extract steps from Lua table.
fn extract_steps(table: &mlua::Table, path: &Path) -> Result<Vec<MacroStep>, MacroRepoError> {
    let steps_table: mlua::Table =
        table.get("steps").map_err(|_| MacroRepoError::LuaInvalid {
            path: path.to_path_buf(),
            message: "Macro must have a 'steps' field".to_string(),
        })?;

    let mut steps = Vec::new();

    for pair in steps_table.pairs::<i64, mlua::Table>() {
        let (_, step_table) = pair.map_err(|e| MacroRepoError::LuaParse {
            path: path.to_path_buf(),
            source: ScriptingError::Lua(e),
        })?;

        let step = parse_step(&step_table, path)?;
        steps.push(step);
    }

    Ok(steps)
}

/// Parse a single step from a Lua table.
fn parse_step(table: &mlua::Table, path: &Path) -> Result<MacroStep, MacroRepoError> {
    // Check for step type field
    let step_type: Option<String> = table.get("type").ok();

    // If type is specified, use it
    if let Some(t) = step_type {
        return match t.as_str() {
            "template" => parse_template_step(table, path),
            "capture" => parse_capture_step(table, path),
            "shell" => parse_shell_step(table, path),
            _ => Err(MacroRepoError::LuaInvalid {
                path: path.to_path_buf(),
                message: format!("Unknown step type: '{}'", t),
            }),
        };
    }

    // Otherwise, try to detect step type from fields (for simpler syntax)
    if table.get::<String>("template").is_ok() {
        return parse_template_step(table, path);
    }
    if table.get::<String>("capture").is_ok() {
        return parse_capture_step(table, path);
    }
    if table.get::<String>("shell").is_ok() {
        return parse_shell_step(table, path);
    }

    Err(MacroRepoError::LuaInvalid {
        path: path.to_path_buf(),
        message: "Step must have 'type' field or 'template'/'capture'/'shell' field".to_string(),
    })
}

/// Parse a template step.
fn parse_template_step(table: &mlua::Table, path: &Path) -> Result<MacroStep, MacroRepoError> {
    let template: String = table.get("template").map_err(|_| MacroRepoError::LuaInvalid {
        path: path.to_path_buf(),
        message: "Template step must have 'template' field".to_string(),
    })?;

    let output: Option<String> = table.get("output").ok();

    let vars_with = extract_with_vars(table, path)?;

    Ok(MacroStep::Template(TemplateStep { template, output, vars_with }))
}

/// Parse a capture step.
fn parse_capture_step(table: &mlua::Table, path: &Path) -> Result<MacroStep, MacroRepoError> {
    let capture: String = table.get("capture").map_err(|_| MacroRepoError::LuaInvalid {
        path: path.to_path_buf(),
        message: "Capture step must have 'capture' field".to_string(),
    })?;

    let vars_with = extract_with_vars(table, path)?;

    Ok(MacroStep::Capture(CaptureStep { capture, vars_with }))
}

/// Parse a shell step.
fn parse_shell_step(table: &mlua::Table, path: &Path) -> Result<MacroStep, MacroRepoError> {
    let shell: String = table.get("shell").map_err(|_| MacroRepoError::LuaInvalid {
        path: path.to_path_buf(),
        message: "Shell step must have 'shell' field".to_string(),
    })?;

    let description: String = table.get("description").unwrap_or_default();

    Ok(MacroStep::Shell(ShellStep { shell, description }))
}

/// Extract `with` vars from a step table.
fn extract_with_vars(
    table: &mlua::Table,
    path: &Path,
) -> Result<HashMap<String, String>, MacroRepoError> {
    let mut vars = HashMap::new();

    let with_table: mlua::Table = match table.get("with") {
        Ok(t) => t,
        Err(_) => return Ok(vars),
    };

    for pair in with_table.pairs::<String, mlua::Value>() {
        let (key, value) = pair.map_err(|e| MacroRepoError::LuaParse {
            path: path.to_path_buf(),
            source: ScriptingError::Lua(e),
        })?;

        let str_value = match value {
            mlua::Value::String(s) => s
                .to_str()
                .map_err(|e| MacroRepoError::LuaParse {
                    path: path.to_path_buf(),
                    source: ScriptingError::Lua(e),
                })?
                .to_string(),
            mlua::Value::Integer(n) => n.to_string(),
            mlua::Value::Number(n) => n.to_string(),
            mlua::Value::Boolean(b) => b.to_string(),
            _ => continue,
        };

        vars.insert(key, str_value);
    }

    Ok(vars)
}

/// Extract error policy from Lua table.
fn extract_error_policy(table: &mlua::Table) -> ErrorPolicy {
    let policy: Option<String> = table.get("on_error").ok();
    match policy.as_deref() {
        Some("continue") => ErrorPolicy::Continue,
        _ => ErrorPolicy::Abort,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_lua_macro(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(format!("{}.lua", name));
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_load_simple_macro() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_macro(
            temp.path(),
            "test",
            r#"
return {
    name = "test",
    description = "A test macro",
    steps = {
        { type = "template", template = "meeting" },
    },
}
"#,
        );

        let spec = load_macro_from_lua(&path).unwrap();

        assert_eq!(spec.name, "test");
        assert_eq!(spec.description, "A test macro");
        assert_eq!(spec.steps.len(), 1);
        assert!(matches!(&spec.steps[0], MacroStep::Template(t) if t.template == "meeting"));
    }

    #[test]
    fn test_load_macro_with_vars() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_macro(
            temp.path(),
            "weekly",
            r#"
return {
    name = "weekly",
    vars = {
        focus = "What's your focus?",
        week_of = {
            prompt = "Week date",
            default = "{{today}}",
        },
    },
    steps = {
        { template = "weekly-summary" },
    },
}
"#,
        );

        let spec = load_macro_from_lua(&path).unwrap();

        assert!(spec.vars.is_some());
        let vars = spec.vars.unwrap();
        assert_eq!(vars.len(), 2);

        match &vars["focus"] {
            VarSpec::Simple(s) => assert_eq!(s, "What's your focus?"),
            _ => panic!("Expected simple var"),
        }

        match &vars["week_of"] {
            VarSpec::Full(m) => {
                assert_eq!(m.prompt, Some("Week date".to_string()));
                assert_eq!(m.default, Some("{{today}}".to_string()));
            }
            _ => panic!("Expected full var"),
        }
    }

    #[test]
    fn test_load_macro_template_step() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_macro(
            temp.path(),
            "template-test",
            r#"
return {
    name = "template-test",
    steps = {
        {
            type = "template",
            template = "meeting",
            output = "meetings/{{date}}.md",
            with = {
                title = "Weekly sync",
                attendees = "Team A",
            },
        },
    },
}
"#,
        );

        let spec = load_macro_from_lua(&path).unwrap();

        assert_eq!(spec.steps.len(), 1);
        match &spec.steps[0] {
            MacroStep::Template(t) => {
                assert_eq!(t.template, "meeting");
                assert_eq!(t.output, Some("meetings/{{date}}.md".to_string()));
                assert_eq!(t.vars_with.get("title"), Some(&"Weekly sync".to_string()));
                assert_eq!(t.vars_with.get("attendees"), Some(&"Team A".to_string()));
            }
            _ => panic!("Expected template step"),
        }
    }

    #[test]
    fn test_load_macro_capture_step() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_macro(
            temp.path(),
            "capture-test",
            r#"
return {
    name = "capture-test",
    steps = {
        {
            type = "capture",
            capture = "inbox",
            with = {
                text = "New item",
            },
        },
    },
}
"#,
        );

        let spec = load_macro_from_lua(&path).unwrap();

        assert_eq!(spec.steps.len(), 1);
        match &spec.steps[0] {
            MacroStep::Capture(c) => {
                assert_eq!(c.capture, "inbox");
                assert_eq!(c.vars_with.get("text"), Some(&"New item".to_string()));
            }
            _ => panic!("Expected capture step"),
        }
    }

    #[test]
    fn test_load_macro_shell_step() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_macro(
            temp.path(),
            "shell-test",
            r#"
return {
    name = "shell-test",
    steps = {
        {
            type = "shell",
            shell = "git add .",
            description = "Stage changes",
        },
    },
}
"#,
        );

        let spec = load_macro_from_lua(&path).unwrap();

        assert_eq!(spec.steps.len(), 1);
        match &spec.steps[0] {
            MacroStep::Shell(s) => {
                assert_eq!(s.shell, "git add .");
                assert_eq!(s.description, "Stage changes");
            }
            _ => panic!("Expected shell step"),
        }
    }

    #[test]
    fn test_load_macro_multiple_steps() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_macro(
            temp.path(),
            "multi",
            r#"
return {
    name = "multi",
    steps = {
        { template = "daily" },
        { capture = "inbox", with = { text = "test" } },
        { shell = "echo done", description = "Print done" },
    },
}
"#,
        );

        let spec = load_macro_from_lua(&path).unwrap();

        assert_eq!(spec.steps.len(), 3);
        assert!(matches!(&spec.steps[0], MacroStep::Template(_)));
        assert!(matches!(&spec.steps[1], MacroStep::Capture(_)));
        assert!(matches!(&spec.steps[2], MacroStep::Shell(_)));
    }

    #[test]
    fn test_load_macro_on_error_continue() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_macro(
            temp.path(),
            "continue",
            r#"
return {
    name = "continue",
    on_error = "continue",
    steps = {
        { template = "test" },
    },
}
"#,
        );

        let spec = load_macro_from_lua(&path).unwrap();
        assert_eq!(spec.on_error, ErrorPolicy::Continue);
    }

    #[test]
    fn test_load_macro_on_error_default() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_macro(
            temp.path(),
            "default",
            r#"
return {
    name = "default",
    steps = {
        { template = "test" },
    },
}
"#,
        );

        let spec = load_macro_from_lua(&path).unwrap();
        assert_eq!(spec.on_error, ErrorPolicy::Abort);
    }

    #[test]
    fn test_load_macro_missing_steps() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_macro(
            temp.path(),
            "invalid",
            r#"
return {
    name = "invalid",
}
"#,
        );

        let result = load_macro_from_lua(&path);
        assert!(matches!(result, Err(MacroRepoError::LuaInvalid { .. })));
    }

    #[test]
    fn test_load_macro_invalid_return() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_macro(temp.path(), "invalid", r#"return "not a table""#);

        let result = load_macro_from_lua(&path);
        assert!(matches!(result, Err(MacroRepoError::LuaInvalid { .. })));
    }

    #[test]
    fn test_load_macro_invalid_step_type() {
        let temp = TempDir::new().unwrap();
        let path = write_lua_macro(
            temp.path(),
            "invalid-step",
            r#"
return {
    name = "invalid-step",
    steps = {
        { type = "unknown" },
    },
}
"#,
        );

        let result = load_macro_from_lua(&path);
        assert!(matches!(result, Err(MacroRepoError::LuaInvalid { .. })));
    }
}
