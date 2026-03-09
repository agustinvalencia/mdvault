//! Note creation orchestrator.
//!
//! The `NoteCreator` provides a unified flow for creating notes of any type,
//! using polymorphic dispatch to handle type-specific behaviors.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use chrono::Local;

use super::NoteType;
use super::context::CreationContext;
use super::traits::{DomainError, DomainResult, NoteBehavior};
use crate::templates::engine::render_with_ref_date as render_template;
use crate::types::scaffolding::generate_scaffolding;

/// Result of a successful note creation.
#[derive(Debug)]
pub struct CreationResult {
    /// Path where the note was written.
    pub path: PathBuf,
    /// Generated ID (if applicable, e.g., task-id, project-id).
    pub generated_id: Option<String>,
    /// The type of note created.
    pub type_name: String,
    /// The final rendered content that was written to disk.
    pub content: String,
}

/// Orchestrates the note creation flow.
pub struct NoteCreator {
    note_type: NoteType,
}

impl NoteCreator {
    /// Create a new NoteCreator for the given note type.
    pub fn new(note_type: NoteType) -> Self {
        Self { note_type }
    }

    /// Get the underlying behavior.
    pub fn behavior(&self) -> &dyn NoteBehavior {
        self.note_type.behavior()
    }

    /// Execute the full note creation flow.
    ///
    /// Flow:
    /// 1. Collect type-specific prompts
    /// 2. Run before_create (sets IDs, counters, etc.)
    /// 3. Resolve output path
    /// 4. Generate content (template or scaffolding)
    /// 5. Ensure core metadata is present
    /// 6. Validate content
    /// 7. Write to disk
    /// 8. Run after_create (logging, hooks, reindex)
    pub fn create(&self, ctx: &mut CreationContext) -> DomainResult<CreationResult> {
        let behavior = self.note_type.behavior();

        // Step 1: Type-specific prompts would be collected here
        // (In practice, prompts are handled by the CLI layer)

        // Step 2: Before create - sets IDs, updates counters, etc.
        behavior.before_create(ctx)?;

        // Step 3: Resolve output path (use pre-set path if provided, otherwise resolve)
        let output_path = if let Some(ref path) = ctx.output_path {
            path.clone()
        } else {
            let path = behavior.output_path(ctx)?;
            ctx.output_path = Some(path.clone());
            path
        };

        // Check if file already exists
        if output_path.exists() {
            return Err(DomainError::Other(format!(
                "Refusing to overwrite existing file: {}",
                output_path.display()
            )));
        }

        // Step 4: Generate content
        let content = self.generate_content(ctx)?;

        // Step 5: Ensure core metadata is preserved
        let order = ctx.typedef.as_ref().and_then(|td| td.frontmatter_order.as_deref());
        let content =
            ctx.core_metadata.apply_to_content(&content, order).map_err(|e| {
                DomainError::Other(format!("Failed to apply core metadata: {}", e))
            })?;

        // Step 6: Validation would happen here
        // (Deferred to integration phase)

        // Step 7: Write to disk
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(DomainError::Io)?;
        }
        fs::write(&output_path, &content).map_err(DomainError::Io)?;

        // Set updated_at on the newly created note
        if let Err(e) = super::services::set_updated_at(&output_path) {
            tracing::warn!("Failed to set updated_at on new note: {}", e);
        }

        // Note: after_create is called by the CLI layer (after hooks)

        // Build result
        let generated_id = ctx
            .core_metadata
            .task_id
            .clone()
            .or_else(|| ctx.core_metadata.project_id.clone());

        Ok(CreationResult {
            path: output_path,
            generated_id,
            type_name: ctx.type_name.clone(),
            content,
        })
    }

    /// Generate the note content.
    ///
    /// If a template is provided in the context, renders it with variable substitution.
    /// Otherwise, generates scaffolding from the type definition.
    fn generate_content(&self, ctx: &CreationContext) -> DomainResult<String> {
        if let Some(ref template) = ctx.template {
            // Build render context with standard variables
            let render_ctx = self.build_render_context(ctx);
            render_template(template, &render_ctx, ctx.reference_date).map_err(|e| {
                DomainError::Other(format!("Failed to render template: {}", e))
            })
        } else {
            // Fall back to scaffolding generation
            // Use evaluated title from core_metadata if available (e.g., for daily/weekly
            // notes where date expressions like "today + 7d" are evaluated to actual dates)
            let title_for_scaffolding =
                ctx.core_metadata.title.as_ref().unwrap_or(&ctx.title);
            Ok(generate_scaffolding(
                &ctx.type_name,
                ctx.typedef.as_deref(),
                title_for_scaffolding,
                &ctx.vars,
            ))
        }
    }

    /// Build a render context with standard template variables.
    fn build_render_context(&self, ctx: &CreationContext) -> HashMap<String, String> {
        let mut render_ctx = HashMap::new();

        // Add date/time defaults FIRST (behaviours can override these)
        let now = Local::now();
        render_ctx.insert("date".into(), now.format("%Y-%m-%d").to_string());
        render_ctx.insert("time".into(), now.format("%H:%M").to_string());
        render_ctx.insert("datetime".into(), now.to_rfc3339());
        render_ctx.insert("today".into(), now.format("%Y-%m-%d").to_string());
        render_ctx.insert("now".into(), now.to_rfc3339());

        // Overlay user/behaviour vars — these WIN over defaults
        render_ctx.extend(ctx.vars.clone());

        // Add config paths
        render_ctx.insert(
            "vault_root".into(),
            ctx.config.vault_root.to_string_lossy().to_string(),
        );
        render_ctx.insert(
            "templates_dir".into(),
            ctx.config.templates_dir.to_string_lossy().to_string(),
        );

        // Add template info if available
        if let Some(ref template) = ctx.template {
            render_ctx.insert("template_name".into(), template.logical_name.clone());
            render_ctx.insert(
                "template_path".into(),
                template.path.to_string_lossy().to_string(),
            );
        }

        // Add output path info if available
        if let Some(ref output_path) = ctx.output_path {
            render_ctx
                .insert("output_path".into(), output_path.to_string_lossy().to_string());
            if let Some(name) = output_path.file_name().and_then(|s| s.to_str()) {
                render_ctx.insert("output_filename".into(), name.to_string());
            }
            if let Some(parent) = output_path.parent() {
                render_ctx
                    .insert("output_dir".into(), parent.to_string_lossy().to_string());
            }
        }

        // Add core metadata fields
        if let Some(ref id) = ctx.core_metadata.task_id {
            render_ctx.insert("task-id".into(), id.clone());
        }
        if let Some(ref id) = ctx.core_metadata.project_id {
            render_ctx.insert("project-id".into(), id.clone());
        }
        if let Some(ref project) = ctx.core_metadata.project {
            render_ctx.insert("project".into(), project.clone());
        }

        render_ctx
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::CoreMetadata;

    #[test]
    fn test_apply_core_metadata() {
        let content =
            "---\ntype: wrong\ntitle: Wrong Title\ncustom: value\n---\n# Body\n";

        let core = CoreMetadata {
            note_type: Some("task".into()),
            title: Some("Correct Title".into()),
            task_id: Some("TST-001".into()),
            project: Some("TST".into()),
            ..Default::default()
        };

        let result = core.apply_to_content(content, None).unwrap();

        assert!(result.contains("type: task"));
        assert!(result.contains("title: Correct Title"));
        assert!(result.contains("task-id: TST-001"));
        assert!(result.contains("project: TST"));
        assert!(result.contains("custom: value")); // Non-core fields preserved
    }
}
