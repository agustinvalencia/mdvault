//! Execution logic for templates and captures.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use chrono::Local;
use regex::Regex;

use markadd_core::captures::{CaptureRepository, CaptureSpec};
use markadd_core::config::types::ResolvedConfig;
use markadd_core::frontmatter::{apply_ops, parse, serialize};
use markadd_core::markdown_ast::{MarkdownAstError, MarkdownEditor, SectionMatch};
use markadd_core::templates::discovery::TemplateInfo;
use markadd_core::templates::engine::{build_render_context, render};
use markadd_core::templates::repository::TemplateRepository;

/// Built-in variables that are automatically provided.
const BUILTIN_VARS: &[&str] = &[
    "date",
    "time",
    "datetime",
    "vault_root",
    "templates_dir",
    "captures_dir",
    "macros_dir",
];

/// Extract user-defined variables from a capture spec (excludes built-ins).
pub fn extract_user_variables(spec: &CaptureSpec) -> Vec<String> {
    let re = Regex::new(r"\{\{([a-zA-Z0-9_]+)\}\}").unwrap();
    let builtin: HashSet<&str> = BUILTIN_VARS.iter().copied().collect();

    let mut vars = HashSet::new();

    // Extract from content (if present)
    if let Some(content) = &spec.content {
        for cap in re.captures_iter(content) {
            let var = cap.get(1).unwrap().as_str();
            if !builtin.contains(var) {
                vars.insert(var.to_string());
            }
        }
    }

    // Extract from target file path
    for cap in re.captures_iter(&spec.target.file) {
        let var = cap.get(1).unwrap().as_str();
        if !builtin.contains(var) {
            vars.insert(var.to_string());
        }
    }

    // Extract from section (if present)
    if let Some(section) = &spec.target.section {
        for cap in re.captures_iter(section) {
            let var = cap.get(1).unwrap().as_str();
            if !builtin.contains(var) {
                vars.insert(var.to_string());
            }
        }
    }

    let mut sorted: Vec<_> = vars.into_iter().collect();
    sorted.sort();
    sorted
}

/// Execute template creation.
pub fn execute_template(
    config: &ResolvedConfig,
    template_name: &str,
    output_path: &Path,
) -> Result<String, String> {
    // Check output doesn't exist
    if output_path.exists() {
        return Err(format!("File already exists: {}", output_path.display()));
    }

    // Load template
    let repo = TemplateRepository::new(&config.templates_dir)
        .map_err(|e| format!("Failed to load templates: {e}"))?;

    let loaded =
        repo.get_by_name(template_name).map_err(|e| format!("Template error: {e}"))?;

    // Build context and render
    let info = TemplateInfo {
        logical_name: loaded.logical_name.clone(),
        path: loaded.path.clone(),
    };

    let ctx = build_render_context(config, &info, output_path);

    let rendered = render(&loaded, &ctx).map_err(|e| format!("Render error: {e}"))?;

    // Create parent dirs and write
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {e}"))?;
    }

    fs::write(output_path, rendered).map_err(|e| format!("Write failed: {e}"))?;

    Ok(format!("Created: {}", output_path.display()))
}

/// Execute capture insertion.
pub fn execute_capture(
    config: &ResolvedConfig,
    capture_name: &str,
    vars: &HashMap<String, String>,
) -> Result<String, String> {
    // Load capture
    let repo = CaptureRepository::new(&config.captures_dir)
        .map_err(|e| format!("Failed to load captures: {e}"))?;

    let loaded =
        repo.get_by_name(capture_name).map_err(|e| format!("Capture error: {e}"))?;

    // Build full context (builtins + user vars)
    let mut ctx = build_capture_context(config);
    for (k, v) in vars {
        ctx.insert(k.clone(), v.clone());
    }

    // Resolve target file
    let target_path = render_string(&loaded.spec.target.file, &ctx);
    let target_path = resolve_target_path(&config.vault_root, &target_path);

    // Read existing file
    let existing = fs::read_to_string(&target_path)
        .map_err(|e| format!("Failed to read {}: {e}", target_path.display()))?;

    // Execute capture operations
    let (result_content, section_info) =
        execute_capture_operations(&existing, &loaded.spec, &ctx)?;

    // Write back
    fs::write(&target_path, &result_content).map_err(|e| format!("Write failed: {e}"))?;

    let mut msg = format!("Captured to: {}", target_path.display());
    if let Some((title, _level)) = section_info {
        msg.push_str(&format!(" (section: {})", title));
    }
    if loaded.spec.frontmatter.is_some() {
        msg.push_str(" [frontmatter updated]");
    }

    Ok(msg)
}

/// Execute capture operations: frontmatter modification and/or content insertion.
fn execute_capture_operations(
    existing_content: &str,
    spec: &CaptureSpec,
    ctx: &HashMap<String, String>,
) -> Result<(String, Option<(String, u8)>), String> {
    // Parse frontmatter from existing content first
    let mut parsed = parse(existing_content)
        .map_err(|e| format!("Failed to parse frontmatter: {e}"))?;
    let mut section_info = None;

    // Apply frontmatter operations if specified
    if let Some(fm_ops) = &spec.frontmatter {
        parsed = apply_ops(parsed, fm_ops, ctx)
            .map_err(|e| format!("Frontmatter error: {e}"))?;
    }

    // Insert content if specified - operate on body only to preserve frontmatter
    if let Some(content_template) = &spec.content {
        let section = spec.target.section.as_ref().ok_or_else(|| {
            "Capture has content but no target section specified".to_string()
        })?;

        let rendered_content = render_string(content_template, ctx);
        let section_match = SectionMatch::new(section);
        let position = spec.target.position.clone().into();

        let result = MarkdownEditor::insert_into_section(
            &parsed.body,
            &section_match,
            &rendered_content,
            position,
        )
        .map_err(|e| match &e {
            MarkdownAstError::SectionNotFound(s) => format!("Section not found: '{s}'"),
            MarkdownAstError::EmptyDocument => "Target file is empty".to_string(),
            MarkdownAstError::RenderError(msg) => format!("Render error: {msg}"),
        })?;

        section_info = Some((result.matched_heading.title, result.matched_heading.level));
        parsed.body = result.content;
    }

    // Serialize the document (frontmatter + body)
    let final_content = serialize(&parsed);
    Ok((final_content, section_info))
}

fn build_capture_context(cfg: &ResolvedConfig) -> HashMap<String, String> {
    let mut ctx = HashMap::new();

    // Date/time
    let now = Local::now();
    ctx.insert("date".into(), now.format("%Y-%m-%d").to_string());
    ctx.insert("time".into(), now.format("%H:%M").to_string());
    ctx.insert("datetime".into(), now.to_rfc3339());

    // Config paths
    ctx.insert("vault_root".into(), cfg.vault_root.to_string_lossy().to_string());
    ctx.insert("templates_dir".into(), cfg.templates_dir.to_string_lossy().to_string());
    ctx.insert("captures_dir".into(), cfg.captures_dir.to_string_lossy().to_string());
    ctx.insert("macros_dir".into(), cfg.macros_dir.to_string_lossy().to_string());

    ctx
}

fn render_string(template: &str, ctx: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\{\{([a-zA-Z0-9_]+)\}\}").unwrap();
    re.replace_all(template, |caps: &regex::Captures<'_>| {
        let key = &caps[1];
        ctx.get(key).cloned().unwrap_or_else(|| caps[0].to_string())
    })
    .into_owned()
}

fn resolve_target_path(vault_root: &Path, target: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(target);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        vault_root.join(path)
    }
}
