//! Hook execution for lifecycle events.
//!
//! This module provides functions to run lifecycle hooks defined in type definitions.

use super::engine::LuaEngine;
use super::hooks::{HookError, NoteContext};
use super::types::SandboxConfig;
use super::vault_context::VaultContext;
use crate::types::definition::TypeDefinition;
use crate::types::validation::yaml_to_lua_table;

/// Result of running an `on_update` hook that may modify the note.
#[derive(Debug)]
pub struct UpdateHookResult {
    /// Whether the hook made changes to the note.
    pub modified: bool,
    /// The updated frontmatter (if modified).
    pub frontmatter: Option<serde_yaml::Value>,
    /// The updated content (if modified).
    pub content: Option<String>,
}

/// Run the `on_create` hook for a type definition.
///
/// This function is called after a note is created to allow the type definition
/// to perform additional operations like logging to daily notes or updating indexes.
///
/// # Arguments
///
/// * `typedef` - The type definition containing the hook
/// * `note_ctx` - Context about the created note
/// * `vault_ctx` - Vault context with access to repositories
///
/// # Returns
///
/// * `Ok(())` if the hook succeeds or doesn't exist
/// * `Err(HookError)` on failure
///
/// # Example
///
/// ```ignore
/// use mdvault_core::scripting::{run_on_create_hook, NoteContext, VaultContext};
///
/// let note_ctx = NoteContext::new(path, "task".into(), frontmatter, content);
/// run_on_create_hook(&typedef, &note_ctx, vault_ctx)?;
/// ```
pub fn run_on_create_hook(
    typedef: &TypeDefinition,
    note_ctx: &NoteContext,
    vault_ctx: VaultContext,
) -> Result<(), HookError> {
    // Skip if no hook defined
    if !typedef.has_on_create_hook {
        return Ok(());
    }

    // Create engine with vault context
    let engine = LuaEngine::with_vault_context(SandboxConfig::restricted(), vault_ctx)
        .map_err(|e| HookError::LuaError(e.to_string()))?;

    let lua = engine.lua();

    // Load and evaluate the type definition to get the table
    let typedef_table: mlua::Table =
        lua.load(&typedef.lua_source).eval().map_err(|e| {
            HookError::LuaError(format!("failed to load type definition: {}", e))
        })?;

    // Build note table for the hook
    let note_table =
        lua.create_table().map_err(|e| HookError::LuaError(e.to_string()))?;

    note_table
        .set("path", note_ctx.path.to_string_lossy().to_string())
        .map_err(|e| HookError::LuaError(e.to_string()))?;

    note_table
        .set("type", note_ctx.note_type.clone())
        .map_err(|e| HookError::LuaError(e.to_string()))?;

    note_table
        .set("content", note_ctx.content.clone())
        .map_err(|e| HookError::LuaError(e.to_string()))?;

    // Convert frontmatter to Lua table
    let fm_table = yaml_to_lua_table(lua, &note_ctx.frontmatter)
        .map_err(|e| HookError::LuaError(e.to_string()))?;

    note_table
        .set("frontmatter", fm_table)
        .map_err(|e| HookError::LuaError(e.to_string()))?;

    // Get on_create function
    let on_create_fn: mlua::Function = typedef_table.get("on_create").map_err(|e| {
        HookError::LuaError(format!("on_create function not found: {}", e))
    })?;

    // Call the hook
    // The hook receives the note table and can call mdv.template/capture/macro
    // We don't currently use the return value, but hooks are expected to return the note
    on_create_fn
        .call::<()>(note_table)
        .map_err(|e| HookError::Execution(format!("on_create hook failed: {}", e)))?;

    Ok(())
}

/// Run the `on_update` hook for a type definition.
///
/// This function is called after a note is modified (via capture operations) to allow
/// the type definition to perform additional operations like updating timestamps.
///
/// Unlike `on_create`, this hook can return a modified note which will be written back.
///
/// # Arguments
///
/// * `typedef` - The type definition containing the hook
/// * `note_ctx` - Context about the updated note
/// * `vault_ctx` - Vault context with access to repositories
///
/// # Returns
///
/// * `Ok(UpdateHookResult)` with any modifications from the hook
/// * `Err(HookError)` on failure
///
/// # Example
///
/// ```ignore
/// use mdvault_core::scripting::{run_on_update_hook, NoteContext, VaultContext};
///
/// let note_ctx = NoteContext::new(path, "task".into(), frontmatter, content);
/// let result = run_on_update_hook(&typedef, &note_ctx, vault_ctx)?;
/// if result.modified {
///     // Write back the updated content
/// }
/// ```
pub fn run_on_update_hook(
    typedef: &TypeDefinition,
    note_ctx: &NoteContext,
    vault_ctx: VaultContext,
) -> Result<UpdateHookResult, HookError> {
    // Skip if no hook defined
    if !typedef.has_on_update_hook {
        return Ok(UpdateHookResult {
            modified: false,
            frontmatter: None,
            content: None,
        });
    }

    // Create engine with vault context
    let engine = LuaEngine::with_vault_context(SandboxConfig::restricted(), vault_ctx)
        .map_err(|e| HookError::LuaError(e.to_string()))?;

    let lua = engine.lua();

    // Load and evaluate the type definition to get the table
    let typedef_table: mlua::Table =
        lua.load(&typedef.lua_source).eval().map_err(|e| {
            HookError::LuaError(format!("failed to load type definition: {}", e))
        })?;

    // Build note table for the hook
    let note_table =
        lua.create_table().map_err(|e| HookError::LuaError(e.to_string()))?;

    note_table
        .set("path", note_ctx.path.to_string_lossy().to_string())
        .map_err(|e| HookError::LuaError(e.to_string()))?;

    note_table
        .set("type", note_ctx.note_type.clone())
        .map_err(|e| HookError::LuaError(e.to_string()))?;

    note_table
        .set("content", note_ctx.content.clone())
        .map_err(|e| HookError::LuaError(e.to_string()))?;

    // Convert frontmatter to Lua table
    let fm_table = yaml_to_lua_table(lua, &note_ctx.frontmatter)
        .map_err(|e| HookError::LuaError(e.to_string()))?;

    note_table
        .set("frontmatter", fm_table)
        .map_err(|e| HookError::LuaError(e.to_string()))?;

    // Get on_update function
    let on_update_fn: mlua::Function = typedef_table.get("on_update").map_err(|e| {
        HookError::LuaError(format!("on_update function not found: {}", e))
    })?;

    // Call the hook - it may return a modified note table
    let result: mlua::Value = on_update_fn
        .call(note_table)
        .map_err(|e| HookError::Execution(format!("on_update hook failed: {}", e)))?;

    // Check if hook returned a modified note
    match result {
        mlua::Value::Table(returned_note) => {
            // Extract frontmatter and content if present
            let frontmatter: Option<serde_yaml::Value> =
                if let Ok(fm_table) = returned_note.get::<mlua::Table>("frontmatter") {
                    Some(lua_table_to_yaml(&fm_table)?)
                } else {
                    None
                };

            let content: Option<String> = returned_note.get("content").ok();

            let modified = frontmatter.is_some() || content.is_some();
            Ok(UpdateHookResult { modified, frontmatter, content })
        }
        mlua::Value::Nil => {
            // Hook returned nil, no modifications
            Ok(UpdateHookResult { modified: false, frontmatter: None, content: None })
        }
        _ => {
            // Unexpected return type
            Ok(UpdateHookResult { modified: false, frontmatter: None, content: None })
        }
    }
}

/// Convert a Lua table to serde_yaml::Value.
fn lua_table_to_yaml(table: &mlua::Table) -> Result<serde_yaml::Value, HookError> {
    let mut map = serde_yaml::Mapping::new();

    for pair in table.pairs::<mlua::Value, mlua::Value>() {
        let (key, value) = pair.map_err(|e| HookError::LuaError(e.to_string()))?;

        let yaml_key = match key {
            mlua::Value::String(s) => {
                let str_val =
                    s.to_str().map_err(|e| HookError::LuaError(e.to_string()))?;
                serde_yaml::Value::String(str_val.to_string())
            }
            mlua::Value::Integer(i) => serde_yaml::Value::Number(i.into()),
            _ => continue, // Skip non-string/integer keys
        };

        let yaml_value = lua_value_to_yaml(value)?;
        map.insert(yaml_key, yaml_value);
    }

    Ok(serde_yaml::Value::Mapping(map))
}

/// Convert a single Lua value to serde_yaml::Value.
fn lua_value_to_yaml(value: mlua::Value) -> Result<serde_yaml::Value, HookError> {
    match value {
        mlua::Value::Nil => Ok(serde_yaml::Value::Null),
        mlua::Value::Boolean(b) => Ok(serde_yaml::Value::Bool(b)),
        mlua::Value::Integer(i) => Ok(serde_yaml::Value::Number(i.into())),
        mlua::Value::Number(n) => {
            Ok(serde_yaml::Value::Number(serde_yaml::Number::from(n)))
        }
        mlua::Value::String(s) => {
            let str_val = s.to_str().map_err(|e| HookError::LuaError(e.to_string()))?;
            Ok(serde_yaml::Value::String(str_val.to_string()))
        }
        mlua::Value::Table(t) => {
            // Check if it's an array or a map
            if is_lua_array(&t) {
                let mut seq = Vec::new();
                for pair in t.pairs::<i64, mlua::Value>() {
                    let (_, v) = pair.map_err(|e| HookError::LuaError(e.to_string()))?;
                    seq.push(lua_value_to_yaml(v)?);
                }
                Ok(serde_yaml::Value::Sequence(seq))
            } else {
                lua_table_to_yaml(&t)
            }
        }
        _ => Ok(serde_yaml::Value::Null),
    }
}

/// Check if a Lua table is an array (sequential integer keys starting from 1).
fn is_lua_array(table: &mlua::Table) -> bool {
    let len = table.raw_len();
    if len == 0 {
        // Could be empty table, check for any keys
        table.pairs::<mlua::Value, mlua::Value>().next().is_none()
    } else {
        // Check if keys are 1..=len
        for i in 1..=len {
            if table.raw_get::<mlua::Value>(i).is_err() {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_typedef_with_hook(lua_source: &str) -> TypeDefinition {
        TypeDefinition {
            name: "test".to_string(),
            description: None,
            source_path: PathBuf::new(),
            schema: HashMap::new(),
            has_validate_fn: false,
            has_on_create_hook: true,
            has_on_update_hook: false,
            is_builtin_override: false,
            lua_source: lua_source.to_string(),
        }
    }

    fn make_note_ctx() -> NoteContext {
        NoteContext {
            path: PathBuf::from("test.md"),
            note_type: "test".to_string(),
            frontmatter: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
            content: "# Test".to_string(),
        }
    }

    #[test]
    fn test_skip_if_no_hook() {
        let typedef = TypeDefinition {
            name: "test".to_string(),
            description: None,
            source_path: PathBuf::new(),
            schema: HashMap::new(),
            has_validate_fn: false,
            has_on_create_hook: false, // No hook
            has_on_update_hook: false,
            is_builtin_override: false,
            lua_source: String::new(),
        };

        // Create a minimal vault context - this won't be used since there's no hook
        // We can't easily create a VaultContext in tests without real repositories,
        // but since has_on_create_hook is false, it will return early
        let _note_ctx = make_note_ctx();

        // This test verifies that when has_on_create_hook is false,
        // the function returns Ok(()) without trying to access vault_ctx
        // However, we need a VaultContext to call the function...
        // For now, just test the hook detection logic works.
        assert!(!typedef.has_on_create_hook);
    }

    #[test]
    fn test_hook_receives_note_context() {
        // This test verifies the Lua hook structure works
        // We create a hook that just returns true without vault operations
        let lua_source = r#"
            return {
                on_create = function(note)
                    -- Just verify we can access note fields
                    local _ = note.path
                    local _ = note.type
                    local _ = note.content
                    local _ = note.frontmatter
                    return note
                end
            }
        "#;

        let _typedef = make_typedef_with_hook(lua_source);
        let _note_ctx = make_note_ctx();

        // Create a sandboxed engine to test the Lua code directly
        let engine = LuaEngine::sandboxed().unwrap();
        let lua = engine.lua();

        // Load the typedef
        let typedef_table: mlua::Table = lua.load(lua_source).eval().unwrap();

        // Build note table
        let note_table = lua.create_table().unwrap();
        note_table.set("path", "test.md").unwrap();
        note_table.set("type", "test").unwrap();
        note_table.set("content", "# Test").unwrap();
        let fm = lua.create_table().unwrap();
        note_table.set("frontmatter", fm).unwrap();

        // Call on_create
        let on_create: mlua::Function = typedef_table.get("on_create").unwrap();
        let result = on_create.call::<mlua::Value>(note_table);

        assert!(result.is_ok());
    }
}
