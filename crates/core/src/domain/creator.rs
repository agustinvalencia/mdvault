//! Note creation orchestrator.
//!
//! The `NoteCreator` provides a unified flow for creating notes of any type,
//! using polymorphic dispatch to handle type-specific behaviors.

use std::fs;
use std::path::PathBuf;

use super::NoteType;
use super::context::CreationContext;
use super::traits::{DomainError, DomainResult, NoteBehavior};
use crate::frontmatter::{Frontmatter, ParsedDocument, serialize_with_order};
use crate::templates::engine::render as render_template;
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
        let content = ensure_core_metadata(&content, ctx)?;

        // Step 6: Validation would happen here
        // (Deferred to integration phase)

        // Step 7: Write to disk
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(DomainError::Io)?;
        }
        fs::write(&output_path, &content).map_err(DomainError::Io)?;

        // Step 8: After create - logging, hooks, reindex
        behavior.after_create(ctx, &content)?;

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
        })
    }

    /// Generate the note content.
    ///
    /// If a template is provided in the context, renders it with variable substitution.
    /// Otherwise, generates scaffolding from the type definition.
    fn generate_content(&self, ctx: &CreationContext) -> DomainResult<String> {
        if let Some(ref template) = ctx.template {
            // Use template rendering
            render_template(template, &ctx.vars).map_err(|e| {
                DomainError::Other(format!("Failed to render template: {}", e))
            })
        } else {
            // Fall back to scaffolding generation
            Ok(generate_scaffolding(
                &ctx.type_name,
                ctx.typedef.as_deref(),
                &ctx.title,
                &ctx.vars,
            ))
        }
    }
}

/// Ensure core metadata fields are present in the content.
///
/// This function parses the frontmatter and ensures that core fields
/// (type, title, task-id, project-id, etc.) are set correctly,
/// overwriting any values that may have been modified by templates or hooks.
fn ensure_core_metadata(content: &str, ctx: &CreationContext) -> DomainResult<String> {
    use crate::frontmatter::parse;

    let parsed = parse(content)
        .map_err(|e| DomainError::Other(format!("Failed to parse frontmatter: {}", e)))?;

    let mut fields = parsed.frontmatter.map(|fm| fm.fields).unwrap_or_default();

    // Apply core metadata (these are authoritative)
    let core = &ctx.core_metadata;

    if let Some(ref t) = core.note_type {
        fields.insert("type".into(), serde_yaml::Value::String(t.clone()));
    }
    if let Some(ref t) = core.title {
        fields.insert("title".into(), serde_yaml::Value::String(t.clone()));
    }
    if let Some(ref id) = core.project_id {
        fields.insert("project-id".into(), serde_yaml::Value::String(id.clone()));
    }
    if let Some(ref id) = core.task_id {
        fields.insert("task-id".into(), serde_yaml::Value::String(id.clone()));
    }
    if let Some(counter) = core.task_counter {
        fields.insert("task_counter".into(), serde_yaml::Value::Number(counter.into()));
    }
    if let Some(ref p) = core.project {
        fields.insert("project".into(), serde_yaml::Value::String(p.clone()));
    }
    if let Some(ref d) = core.date {
        fields.insert("date".into(), serde_yaml::Value::String(d.clone()));
    }
    if let Some(ref w) = core.week {
        fields.insert("week".into(), serde_yaml::Value::String(w.clone()));
    }

    // Rebuild content using serializer with order
    let doc =
        ParsedDocument { frontmatter: Some(Frontmatter { fields }), body: parsed.body };

    let order = ctx.typedef.as_ref().and_then(|td| td.frontmatter_order.as_deref());
    Ok(serialize_with_order(&doc, order))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::ResolvedConfig;
    use crate::types::TypeRegistry;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_test_config(vault_root: PathBuf) -> ResolvedConfig {
        ResolvedConfig {
            active_profile: "test".into(),
            vault_root: vault_root.clone(),
            templates_dir: vault_root.join(".mdvault/templates"),
            captures_dir: vault_root.join(".mdvault/captures"),
            macros_dir: vault_root.join(".mdvault/macros"),
            typedefs_dir: vault_root.join(".mdvault/typedefs"),
            security: Default::default(),
            logging: Default::default(),
        }
    }

    #[test]
    fn test_ensure_core_metadata() {
        let content =
            "---\ntype: wrong\ntitle: Wrong Title\ncustom: value\n---\n# Body\n";

        let tmp = tempdir().unwrap();
        let config = make_test_config(tmp.path().to_path_buf());
        let registry = TypeRegistry::new();

        let mut ctx = CreationContext::new("task", "Correct Title", &config, &registry);
        ctx.core_metadata.task_id = Some("TST-001".into());
        ctx.core_metadata.project = Some("TST".into());

        let result = ensure_core_metadata(content, &ctx).unwrap();

        assert!(result.contains("type: task"));
        assert!(result.contains("title: Correct Title"));
        assert!(result.contains("task-id: TST-001"));
        assert!(result.contains("project: TST"));
        assert!(result.contains("custom: value")); // Non-core fields preserved
    }
}
