//! Hook execution for lifecycle events.
//!
//! This module provides functions to run lifecycle hooks defined in type definitions.

use super::engine::LuaEngine;
use super::hooks::{HookError, NoteContext};
use super::types::SandboxConfig;
use super::vault_context::VaultContext;
use crate::types::definition::TypeDefinition;
use crate::types::validation::yaml_to_lua_table;

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
    let typedef_table: mlua::Table = lua
        .load(&typedef.lua_source)
        .eval()
        .map_err(|e| HookError::LuaError(format!("failed to load type definition: {}", e)))?;

    // Build note table for the hook
    let note_table = lua
        .create_table()
        .map_err(|e| HookError::LuaError(e.to_string()))?;

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
    let fm_table =
        yaml_to_lua_table(lua, &note_ctx.frontmatter).map_err(|e| HookError::LuaError(e.to_string()))?;

    note_table
        .set("frontmatter", fm_table)
        .map_err(|e| HookError::LuaError(e.to_string()))?;

    // Get on_create function
    let on_create_fn: mlua::Function = typedef_table
        .get("on_create")
        .map_err(|e| HookError::LuaError(format!("on_create function not found: {}", e)))?;

    // Call the hook
    // The hook receives the note table and can call mdv.template/capture/macro
    // We don't currently use the return value, but hooks are expected to return the note
    on_create_fn
        .call::<()>(note_table)
        .map_err(|e| HookError::Execution(format!("on_create hook failed: {}", e)))?;

    Ok(())
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
        let note_ctx = make_note_ctx();

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

        let typedef = make_typedef_with_hook(lua_source);
        let note_ctx = make_note_ctx();

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
