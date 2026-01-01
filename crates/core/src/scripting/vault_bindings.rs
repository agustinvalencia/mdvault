//! Vault operation bindings for Lua.
//!
//! This module provides Lua bindings for vault operations:
//! - `mdv.template(name, vars?)` - Render a template by name
//! - `mdv.capture(name, vars?)` - Execute a capture workflow
//! - `mdv.macro(name, vars?)` - Execute a macro workflow
//! - `mdv.read_note(path)` - Read a note's content and frontmatter

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::Local;
use mlua::{Function, Lua, MultiValue, Result as LuaResult, Table, Value};

use super::vault_context::VaultContext;
use crate::captures::CaptureSpec;
use crate::config::types::ResolvedConfig;
use crate::frontmatter::{apply_ops, parse, serialize};
use crate::macros::runner::{MacroRunError, RunContext, RunOptions, StepExecutor};
use crate::macros::types::{CaptureStep, ShellStep, StepResult, TemplateStep};
use crate::markdown_ast::{MarkdownEditor, SectionMatch};
use crate::templates::engine::render_string;
use crate::types::validation::yaml_to_lua_table;

/// Register vault operation bindings on an existing mdv table.
///
/// This adds `mdv.template()`, `mdv.capture()`, and `mdv.macro()` functions
/// that have access to the vault context for executing operations.
pub fn register_vault_bindings(lua: &Lua, ctx: VaultContext) -> LuaResult<()> {
    // Store context in Lua app data
    lua.set_app_data(ctx);

    let mdv: Table = lua.globals().get("mdv")?;

    mdv.set("template", create_template_fn(lua)?)?;
    mdv.set("capture", create_capture_fn(lua)?)?;
    mdv.set("macro", create_macro_fn(lua)?)?;
    mdv.set("read_note", create_read_note_fn(lua)?)?;

    Ok(())
}

/// Create the `mdv.template(name, vars?)` function.
///
/// Returns: `(content, nil)` on success, `(nil, error)` on failure.
///
/// # Examples (in Lua)
///
/// ```lua
/// local content, err = mdv.template("meeting", { title = "Standup" })
/// if err then
///     print("Error: " .. err)
/// else
///     print(content)
/// end
/// ```
fn create_template_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, args: (String, Option<Table>)| {
        let (template_name, vars_table) = args;

        let ctx = lua
            .app_data_ref::<VaultContext>()
            .ok_or_else(|| mlua::Error::runtime("VaultContext not available"))?;

        // Load template
        let loaded = match ctx.template_repo.get_by_name(&template_name) {
            Ok(t) => t,
            Err(e) => {
                return Ok(MultiValue::from_vec(vec![
                    Value::Nil,
                    Value::String(lua.create_string(format!(
                        "template '{}' not found: {}",
                        template_name, e
                    ))?),
                ]));
            }
        };

        // Build render context
        let mut render_ctx = build_base_context(&ctx.config);
        if let Some(table) = vars_table {
            for pair in table.pairs::<String, Value>() {
                let (key, value) = pair?;
                let str_value = lua_value_to_string(&key, value)?;
                render_ctx.insert(key, str_value);
            }
        }

        // Render template body
        match render_string(&loaded.body, &render_ctx) {
            Ok(rendered) => Ok(MultiValue::from_vec(vec![
                Value::String(lua.create_string(&rendered)?),
                Value::Nil,
            ])),
            Err(e) => Ok(MultiValue::from_vec(vec![
                Value::Nil,
                Value::String(
                    lua.create_string(format!("template render error: {}", e))?,
                ),
            ])),
        }
    })
}

/// Create the `mdv.capture(name, vars?)` function.
///
/// Returns: `(true, nil)` on success, `(false, error)` on failure.
///
/// # Examples (in Lua)
///
/// ```lua
/// local ok, err = mdv.capture("log-to-daily", { text = "Created note" })
/// if not ok then
///     print("Error: " .. err)
/// end
/// ```
fn create_capture_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, args: (String, Option<Table>)| {
        let (capture_name, vars_table) = args;

        let ctx = lua
            .app_data_ref::<VaultContext>()
            .ok_or_else(|| mlua::Error::runtime("VaultContext not available"))?;

        // Load capture
        let loaded = match ctx.capture_repo.get_by_name(&capture_name) {
            Ok(c) => c,
            Err(e) => {
                return Ok(MultiValue::from_vec(vec![
                    Value::Boolean(false),
                    Value::String(lua.create_string(format!(
                        "capture '{}' not found: {}",
                        capture_name, e
                    ))?),
                ]));
            }
        };

        // Build context
        let mut vars = build_base_context(&ctx.config);
        if let Some(table) = vars_table {
            for pair in table.pairs::<String, Value>() {
                let (key, value) = pair?;
                let str_value = lua_value_to_string(&key, value)?;
                vars.insert(key, str_value);
            }
        }

        // Execute capture
        match execute_capture(&ctx.config, &loaded.spec, &vars) {
            Ok(_) => Ok(MultiValue::from_vec(vec![Value::Boolean(true), Value::Nil])),
            Err(e) => Ok(MultiValue::from_vec(vec![
                Value::Boolean(false),
                Value::String(lua.create_string(&e)?),
            ])),
        }
    })
}

/// Create the `mdv.macro(name, vars?)` function.
///
/// Returns: `(true, nil)` on success, `(false, error)` on failure.
///
/// Note: Shell steps in macros are NOT executed from hooks (no --trust context).
///
/// # Examples (in Lua)
///
/// ```lua
/// local ok, err = mdv.macro("on-task-created", { task_path = note.path })
/// if not ok then
///     print("Error: " .. err)
/// end
/// ```
fn create_macro_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, args: (String, Option<Table>)| {
        let (macro_name, vars_table) = args;

        let ctx = lua
            .app_data_ref::<VaultContext>()
            .ok_or_else(|| mlua::Error::runtime("VaultContext not available"))?;

        // Load macro
        let loaded = match ctx.macro_repo.get_by_name(&macro_name) {
            Ok(m) => m,
            Err(e) => {
                return Ok(MultiValue::from_vec(vec![
                    Value::Boolean(false),
                    Value::String(lua.create_string(format!(
                        "macro '{}' not found: {}",
                        macro_name, e
                    ))?),
                ]));
            }
        };

        // Build context
        let mut vars = build_base_context(&ctx.config);
        if let Some(table) = vars_table {
            for pair in table.pairs::<String, Value>() {
                let (key, value) = pair?;
                let str_value = lua_value_to_string(&key, value)?;
                vars.insert(key, str_value);
            }
        }

        // Create a hook step executor (no shell support)
        let executor = HookStepExecutor {
            config: ctx.config.clone(),
            template_repo: ctx.template_repo.clone(),
            capture_repo: ctx.capture_repo.clone(),
        };

        // Run macro with shell disabled (no --trust in hooks)
        let run_ctx = RunContext::new(
            vars,
            RunOptions { trust: false, allow_shell: false, dry_run: false },
        );

        let result = crate::macros::runner::run_macro(&loaded, &executor, run_ctx);

        if result.success {
            Ok(MultiValue::from_vec(vec![Value::Boolean(true), Value::Nil]))
        } else {
            Ok(MultiValue::from_vec(vec![
                Value::Boolean(false),
                Value::String(lua.create_string(&result.message)?),
            ]))
        }
    })
}

/// Create the `mdv.read_note(path)` function.
///
/// Reads a note from the vault and returns its content and frontmatter.
///
/// Returns: `(note_table, nil)` on success, `(nil, error)` on failure.
///
/// The note table contains:
/// - `path`: The resolved path to the note
/// - `content`: The full file content including frontmatter
/// - `body`: The note body without frontmatter
/// - `frontmatter`: A table with frontmatter fields (if present)
/// - `title`: The title from frontmatter (if present)
/// - `type`: The note type from frontmatter (if present)
///
/// # Examples (in Lua)
///
/// ```lua
/// local note, err = mdv.read_note("projects/my-project.md")
/// if err then
///     print("Error: " .. err)
/// else
///     print("Title: " .. (note.title or "untitled"))
///     if note.frontmatter then
///         print("Status: " .. (note.frontmatter.status or "unknown"))
///     end
/// end
/// ```
fn create_read_note_fn(lua: &Lua) -> LuaResult<Function> {
    lua.create_function(|lua, path: String| {
        let ctx = lua
            .app_data_ref::<VaultContext>()
            .ok_or_else(|| mlua::Error::runtime("VaultContext not available"))?;

        // Resolve path relative to vault root
        let resolved_path =
            if path.ends_with(".md") { path.clone() } else { format!("{}.md", path) };

        let full_path = if Path::new(&resolved_path).is_absolute() {
            std::path::PathBuf::from(&resolved_path)
        } else {
            ctx.vault_root.join(&resolved_path)
        };

        // Read file content
        let content = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(MultiValue::from_vec(vec![
                    Value::Nil,
                    Value::String(lua.create_string(format!(
                        "failed to read '{}': {}",
                        full_path.display(),
                        e
                    ))?),
                ]));
            }
        };

        // Parse frontmatter
        let parsed = match parse(&content) {
            Ok(p) => p,
            Err(e) => {
                return Ok(MultiValue::from_vec(vec![
                    Value::Nil,
                    Value::String(
                        lua.create_string(format!("failed to parse frontmatter: {}", e))?,
                    ),
                ]));
            }
        };

        // Build note table
        let note_table = lua.create_table()?;
        note_table.set("path", resolved_path)?;
        note_table.set("content", content)?;
        note_table.set("body", parsed.body.clone())?;

        // Add frontmatter if present
        if let Some(ref fm) = parsed.frontmatter {
            // Convert frontmatter to serde_yaml::Value for yaml_to_lua_table
            let fm_yaml = serde_yaml::to_value(fm).map_err(|e| {
                mlua::Error::runtime(format!("failed to serialize frontmatter: {}", e))
            })?;

            let fm_table = yaml_to_lua_table(lua, &fm_yaml)?;
            note_table.set("frontmatter", fm_table)?;

            // Extract common fields for convenience
            if let Some(title) = fm.fields.get("title").and_then(|v| v.as_str()) {
                note_table.set("title", title)?;
            }
            if let Some(note_type) = fm.fields.get("type").and_then(|v| v.as_str()) {
                note_table.set("type", note_type)?;
            }
        }

        Ok(MultiValue::from_vec(vec![Value::Table(note_table), Value::Nil]))
    })
}

/// Build base context with date/time and config paths.
fn build_base_context(config: &ResolvedConfig) -> HashMap<String, String> {
    let mut ctx = HashMap::new();
    let now = Local::now();

    // Date/time
    ctx.insert("date".into(), now.format("%Y-%m-%d").to_string());
    ctx.insert("time".into(), now.format("%H:%M").to_string());
    ctx.insert("datetime".into(), now.to_rfc3339());
    ctx.insert("today".into(), now.format("%Y-%m-%d").to_string());
    ctx.insert("now".into(), now.format("%Y-%m-%dT%H:%M:%S").to_string());

    // Config paths
    ctx.insert("vault_root".into(), config.vault_root.to_string_lossy().to_string());
    ctx.insert(
        "templates_dir".into(),
        config.templates_dir.to_string_lossy().to_string(),
    );
    ctx.insert("captures_dir".into(), config.captures_dir.to_string_lossy().to_string());
    ctx.insert("macros_dir".into(), config.macros_dir.to_string_lossy().to_string());

    ctx
}

/// Convert a Lua value to a string.
fn lua_value_to_string(key: &str, value: Value) -> LuaResult<String> {
    match value {
        Value::String(s) => Ok(s.to_str()?.to_string()),
        Value::Integer(i) => Ok(i.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Boolean(b) => Ok(b.to_string()),
        Value::Nil => Ok(String::new()),
        _ => Err(mlua::Error::runtime(format!(
            "context value for '{}' must be string, number, boolean, or nil",
            key
        ))),
    }
}

/// Execute a capture operation.
fn execute_capture(
    config: &ResolvedConfig,
    spec: &CaptureSpec,
    vars: &HashMap<String, String>,
) -> Result<(), String> {
    // Render target file path
    let target_file_raw =
        render_string(&spec.target.file, vars).map_err(|e| e.to_string())?;
    let target_file = resolve_target_path(&config.vault_root, &target_file_raw);

    // Read existing file or create if missing
    let existing_content = match fs::read_to_string(&target_file) {
        Ok(content) => content,
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                && spec.target.create_if_missing =>
        {
            // Create the file with minimal structure
            let content = create_minimal_note(vars, spec.target.section.as_deref());

            // Ensure parent directory exists
            if let Some(parent) = target_file.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!("failed to create directory {}: {}", parent.display(), e)
                })?;
            }

            // Write the new file
            fs::write(&target_file, &content).map_err(|e| {
                format!("failed to create target file {}: {}", target_file.display(), e)
            })?;

            content
        }
        Err(e) => {
            return Err(format!(
                "failed to read target file {}: {}",
                target_file.display(),
                e
            ));
        }
    };

    // Execute capture operations
    let (result_content, _section_info) =
        execute_capture_operations(&existing_content, spec, vars)?;

    // Write back to file
    fs::write(&target_file, &result_content)
        .map_err(|e| format!("failed to write to {}: {}", target_file.display(), e))?;

    Ok(())
}

/// Create a minimal note structure for auto-created files.
fn create_minimal_note(vars: &HashMap<String, String>, section: Option<&str>) -> String {
    let date = vars.get("date").map(|s| s.as_str()).unwrap_or("unknown");
    let title = vars.get("title").map(|s| s.as_str()).unwrap_or(date);

    let mut content = format!("---\ntype: daily\ndate: {}\n---\n\n# {}\n", date, title);

    // Add the target section if specified
    if let Some(section_name) = section {
        content.push_str(&format!("\n## {}\n", section_name));
    }

    content
}

/// Execute capture operations: frontmatter modification and/or content insertion.
fn execute_capture_operations(
    existing_content: &str,
    spec: &CaptureSpec,
    ctx: &HashMap<String, String>,
) -> Result<(String, Option<(String, u8)>), String> {
    // Parse frontmatter from existing content
    let mut parsed = parse(existing_content)
        .map_err(|e| format!("failed to parse frontmatter: {}", e))?;
    let mut section_info = None;

    // Apply frontmatter operations if specified
    if let Some(fm_ops) = &spec.frontmatter {
        parsed = apply_ops(parsed, fm_ops, ctx)
            .map_err(|e| format!("failed to apply frontmatter ops: {}", e))?;
    }

    // Insert content if specified
    if let Some(content_template) = &spec.content {
        let section = spec.target.section.as_ref().ok_or_else(|| {
            "capture has content but no target section specified".to_string()
        })?;

        let rendered_section = render_string(section, ctx).map_err(|e| e.to_string())?;
        let rendered_content =
            render_string(content_template, ctx).map_err(|e| e.to_string())?;

        let section_match = SectionMatch::new(&rendered_section);
        let position = spec.target.position.clone().into();

        let result = MarkdownEditor::insert_into_section(
            &parsed.body,
            &section_match,
            &rendered_content,
            position,
        )
        .map_err(|e| format!("section insertion failed: {}", e))?;

        section_info = Some((result.matched_heading.title, result.matched_heading.level));
        parsed.body = result.content;
    }

    // Serialize the document
    let final_content = serialize(&parsed);
    Ok((final_content, section_info))
}

fn resolve_target_path(vault_root: &Path, target: &str) -> std::path::PathBuf {
    let path = Path::new(target);
    if path.is_absolute() { path.to_path_buf() } else { vault_root.join(path) }
}

/// Step executor for hooks (no shell support).
struct HookStepExecutor {
    config: std::sync::Arc<ResolvedConfig>,
    template_repo: std::sync::Arc<crate::templates::repository::TemplateRepository>,
    capture_repo: std::sync::Arc<crate::captures::CaptureRepository>,
}

impl StepExecutor for HookStepExecutor {
    fn execute_template(
        &self,
        step: &TemplateStep,
        ctx: &RunContext,
    ) -> Result<StepResult, MacroRunError> {
        // Load template
        let loaded = self
            .template_repo
            .get_by_name(&step.template)
            .map_err(|e| MacroRunError::TemplateError(e.to_string()))?;

        // Merge step vars
        let vars = ctx.with_step_vars(&step.vars_with);

        // Resolve output path
        let output_path = if let Some(output) = step.output.as_ref() {
            let rendered = render_string(output, &vars)
                .map_err(|e| MacroRunError::TemplateError(e.to_string()))?;
            resolve_target_path(&self.config.vault_root, &rendered)
        } else if let Some(fm) = loaded.frontmatter.as_ref() {
            if let Some(output) = fm.output.as_ref() {
                let rendered = render_string(output, &vars)
                    .map_err(|e| MacroRunError::TemplateError(e.to_string()))?;
                resolve_target_path(&self.config.vault_root, &rendered)
            } else {
                return Err(MacroRunError::TemplateError(
                    "template has no output path and none specified in step".to_string(),
                ));
            }
        } else {
            return Err(MacroRunError::TemplateError(
                "template has no output path and none specified in step".to_string(),
            ));
        };

        // Render template
        let rendered = render_string(&loaded.body, &vars)
            .map_err(|e| MacroRunError::TemplateError(e.to_string()))?;

        // Create parent directories if needed
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                MacroRunError::TemplateError(format!("failed to create directory: {}", e))
            })?;
        }

        // Write file
        fs::write(&output_path, &rendered).map_err(|e| {
            MacroRunError::TemplateError(format!(
                "failed to write {}: {}",
                output_path.display(),
                e
            ))
        })?;

        Ok(StepResult {
            step_index: 0,
            success: true,
            message: format!("Created {}", output_path.display()),
            output_path: Some(output_path),
        })
    }

    fn execute_capture(
        &self,
        step: &CaptureStep,
        ctx: &RunContext,
    ) -> Result<StepResult, MacroRunError> {
        // Load capture
        let loaded = self
            .capture_repo
            .get_by_name(&step.capture)
            .map_err(|e| MacroRunError::CaptureError(e.to_string()))?;

        // Merge step vars
        let vars = ctx.with_step_vars(&step.vars_with);

        // Execute capture
        execute_capture(&self.config, &loaded.spec, &vars)
            .map_err(MacroRunError::CaptureError)?;

        Ok(StepResult {
            step_index: 0,
            success: true,
            message: format!("Executed capture: {}", step.capture),
            output_path: None,
        })
    }

    fn execute_shell(
        &self,
        _step: &ShellStep,
        _ctx: &RunContext,
    ) -> Result<StepResult, MacroRunError> {
        // Shell steps are not supported in hooks
        Err(MacroRunError::TrustRequired)
    }
}
