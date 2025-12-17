use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Local;
use thiserror::Error;

use crate::config::types::ResolvedConfig;

use super::discovery::TemplateInfo;
use super::repository::LoadedTemplate;

#[derive(Debug, Error)]
pub enum TemplateRenderError {
    #[error("invalid regex for template placeholder: {0}")]
    Regex(String),
}

pub type RenderContext = HashMap<String, String>;

/// Build a minimal render context with date/time and config variables.
///
/// This is useful for resolving template output paths from frontmatter
/// before the actual output path is known.
pub fn build_minimal_context(
    cfg: &ResolvedConfig,
    template: &TemplateInfo,
) -> RenderContext {
    let mut ctx = RenderContext::new();

    // Date/time
    let now = Local::now();
    ctx.insert("date".into(), now.format("%Y-%m-%d").to_string());
    ctx.insert("time".into(), now.format("%H:%M").to_string());
    ctx.insert("datetime".into(), now.to_rfc3339());

    // From config
    ctx.insert("vault_root".into(), cfg.vault_root.to_string_lossy().to_string());
    ctx.insert("templates_dir".into(), cfg.templates_dir.to_string_lossy().to_string());
    ctx.insert("captures_dir".into(), cfg.captures_dir.to_string_lossy().to_string());
    ctx.insert("macros_dir".into(), cfg.macros_dir.to_string_lossy().to_string());

    // Template info
    ctx.insert("template_name".into(), template.logical_name.clone());
    ctx.insert("template_path".into(), template.path.to_string_lossy().to_string());

    ctx
}

pub fn build_render_context(
    cfg: &ResolvedConfig,
    template: &TemplateInfo,
    output_path: &Path,
) -> RenderContext {
    let mut ctx = build_minimal_context(cfg, template);

    // Output info
    let output_abs = absolutize(output_path);
    ctx.insert("output_path".into(), output_abs.to_string_lossy().to_string());
    if let Some(name) = output_abs.file_name().and_then(|s| s.to_str()) {
        ctx.insert("output_filename".into(), name.to_string());
    }
    if let Some(parent) = output_abs.parent() {
        ctx.insert("output_dir".into(), parent.to_string_lossy().to_string());
    }

    ctx
}

fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
    }
}

pub fn render(
    template: &LoadedTemplate,
    ctx: &RenderContext,
) -> Result<String, TemplateRenderError> {
    render_string(&template.body, ctx)
}

/// Render a string template with variable substitution.
pub fn render_string(
    template: &str,
    ctx: &RenderContext,
) -> Result<String, TemplateRenderError> {
    let re = Regex::new(r"\{\{([a-zA-Z0-9_]+)\}\}")
        .map_err(|e| TemplateRenderError::Regex(e.to_string()))?;

    let result = re.replace_all(template, |caps: &regex::Captures<'_>| {
        let key = &caps[1];
        ctx.get(key).cloned().unwrap_or_else(|| caps[0].to_string())
    });

    Ok(result.into_owned())
}

/// Resolve the output path for a template.
///
/// If the template has frontmatter with an `output` field, render it with the context.
/// Otherwise, return None.
pub fn resolve_template_output_path(
    template: &LoadedTemplate,
    cfg: &ResolvedConfig,
    ctx: &RenderContext,
) -> Result<Option<PathBuf>, TemplateRenderError> {
    if let Some(ref fm) = template.frontmatter
        && let Some(ref output) = fm.output
    {
        let rendered = render_string(output, ctx)?;
        let path = cfg.vault_root.join(&rendered);
        return Ok(Some(path));
    }
    Ok(None)
}
