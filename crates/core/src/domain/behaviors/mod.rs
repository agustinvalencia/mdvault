//! Behavior implementations for first-class note types.

mod custom;
mod daily;
mod project;
mod task;
mod weekly;
mod zettel;

pub use custom::CustomBehavior;
pub use daily::DailyBehavior;
pub use project::ProjectBehavior;
pub use task::TaskBehavior;
pub use weekly::WeeklyBehavior;
pub use zettel::ZettelBehavior;

use std::path::PathBuf;

use chrono::Local;

use crate::templates::engine::render_string;

use super::context::CreationContext;
use super::traits::{DomainError, DomainResult};

/// Render a Lua typedef output template to a concrete path.
///
/// Adds standard context variables (date, time, title, etc.) and renders
/// the template string. Returns an absolute path (relative paths are joined
/// with vault_root).
pub fn render_output_template(
    template: &str,
    ctx: &CreationContext,
) -> DomainResult<PathBuf> {
    let mut render_ctx = ctx.vars.clone();

    // Add standard context variables
    let now = Local::now();
    render_ctx.insert("date".into(), now.format("%Y-%m-%d").to_string());
    render_ctx.insert("time".into(), now.format("%H:%M").to_string());
    render_ctx.insert("datetime".into(), now.to_rfc3339());
    render_ctx.insert("today".into(), now.format("%Y-%m-%d").to_string());
    render_ctx.insert("now".into(), now.to_rfc3339());

    render_ctx
        .insert("vault_root".into(), ctx.config.vault_root.to_string_lossy().to_string());
    render_ctx.insert("type".into(), ctx.type_name.clone());
    // Use evaluated title from core_metadata if available (e.g., for daily/weekly notes
    // where date expressions like "today + 7d" are evaluated to actual dates)
    let title = ctx.core_metadata.title.as_ref().unwrap_or(&ctx.title);
    render_ctx.insert("title".into(), title.clone());

    // Add core metadata fields if available
    if let Some(ref id) = ctx.core_metadata.project_id {
        render_ctx.insert("project-id".into(), id.clone());
    }
    if let Some(ref id) = ctx.core_metadata.task_id {
        render_ctx.insert("task-id".into(), id.clone());
    }
    if let Some(ref project) = ctx.core_metadata.project {
        render_ctx.insert("project".into(), project.clone());
    }
    if let Some(ref date) = ctx.core_metadata.date {
        render_ctx.insert("date".into(), date.clone());
    }
    if let Some(ref week) = ctx.core_metadata.week {
        render_ctx.insert("week".into(), week.clone());
    }

    let rendered = render_string(template, &render_ctx).map_err(|e| {
        DomainError::Other(format!("Failed to render output path: {}", e))
    })?;

    let path = PathBuf::from(&rendered);
    if path.is_absolute() { Ok(path) } else { Ok(ctx.config.vault_root.join(path)) }
}
