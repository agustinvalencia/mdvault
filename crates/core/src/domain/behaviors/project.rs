//! Project note type behavior.
//!
//! Projects have:
//! - 3-letter ID generated from title
//! - task_counter initialized to 0
//! - Logging to daily note
//! - Output path: Projects/{id}/{id}.md

use std::path::PathBuf;
use std::sync::Arc;

use crate::paths::PathResolver;
use crate::types::TypeDefinition;

use super::super::context::{CreationContext, FieldPrompt, PromptContext, PromptType};
use super::super::traits::{
    DomainError, DomainResult, NoteBehavior, NoteIdentity, NoteLifecycle, NotePrompts,
};

/// Behavior implementation for project notes.
pub struct ProjectBehavior {
    typedef: Option<Arc<TypeDefinition>>,
}

impl ProjectBehavior {
    /// Create a new ProjectBehavior, optionally wrapping a Lua typedef override.
    pub fn new(typedef: Option<Arc<TypeDefinition>>) -> Self {
        Self { typedef }
    }
}

impl NoteIdentity for ProjectBehavior {
    fn generate_id(&self, ctx: &CreationContext) -> DomainResult<Option<String>> {
        // Check if already provided via vars
        if let Some(id) = ctx.get_var("project-id") {
            return Ok(Some(id.to_string()));
        }

        // Generate 3-letter ID from title
        let computed = generate_project_id(&ctx.title);
        Ok(Some(computed))
    }

    fn output_path(&self, ctx: &CreationContext) -> DomainResult<PathBuf> {
        // Check Lua typedef for output template first
        if let Some(ref td) = self.typedef
            && let Some(ref output) = td.output
        {
            return super::render_output_template(output, ctx);
        }

        // Default: Projects/{id}/{id}.md
        let project_id =
            ctx.core_metadata.project_id.as_ref().ok_or_else(|| {
                DomainError::PathResolution("project-id not set".into())
            })?;

        Ok(PathResolver::new(&ctx.config.vault_root).project_note(project_id))
    }

    fn core_fields(&self) -> Vec<&'static str> {
        vec!["type", "title", "project-id", "task_counter"]
    }
}

impl NoteLifecycle for ProjectBehavior {
    fn before_create(&self, ctx: &mut CreationContext) -> DomainResult<()> {
        // Generate or use provided project ID
        let project_id = ctx
            .get_var("project-id")
            .map(|s| s.to_string())
            .or_else(|| self.generate_id(ctx).ok().flatten())
            .ok_or_else(|| {
                DomainError::IdGeneration("could not generate project-id".into())
            })?;

        // Set core metadata
        ctx.core_metadata.project_id = Some(project_id.clone());
        ctx.core_metadata.task_counter = Some(0);
        ctx.set_var("project-id", &project_id);
        ctx.set_var("task_counter", "0");

        Ok(())
    }

    fn after_create(&self, ctx: &CreationContext, content: &str) -> DomainResult<()> {
        // Log to daily note
        if let Some(ref output_path) = ctx.output_path {
            let project_id = ctx.core_metadata.project_id.as_deref().unwrap_or("");
            if let Err(e) = super::super::services::DailyLogService::log_creation(
                ctx.config,
                "project",
                &ctx.title,
                project_id,
                output_path,
            ) {
                // Log warning but don't fail the creation
                tracing::warn!("Failed to log to daily note: {}", e);
            }
        }

        if let (Some(runner), Some(output_path)) = (ctx.hook_runner, &ctx.output_path)
            && let Err(e) = runner.run_on_create(output_path, content)
        {
            tracing::warn!("on_create hook failed: {e}");
        }

        Ok(())
    }
}

impl NotePrompts for ProjectBehavior {
    fn type_prompts(&self, ctx: &PromptContext) -> Vec<FieldPrompt> {
        let mut prompts = vec![];

        // Project ID prompt with computed default
        if !ctx.provided_vars.contains_key("project-id") && !ctx.batch_mode {
            let computed = generate_project_id(ctx.title);
            prompts.push(FieldPrompt {
                field_name: "project-id".into(),
                prompt_text: "Project ID (3-letter code)".into(),
                prompt_type: PromptType::Text,
                required: true,
                default_value: Some(computed),
            });
        }

        prompts
    }
}

impl NoteBehavior for ProjectBehavior {
    fn type_name(&self) -> &'static str {
        "project"
    }
}

// --- Helper functions ---

/// Generate a 3-letter project ID from a title.
///
/// Takes the first letter of the first 3 words, uppercased.
/// Examples:
/// - "My Cool Project" -> "MCP"
/// - "Test" -> "TES"
/// - "A B" -> "AB"
fn generate_project_id(title: &str) -> String {
    let words: Vec<&str> = title.split_whitespace().collect();

    let mut id = String::with_capacity(3);

    for word in words.iter().take(3) {
        if let Some(c) = word.chars().next() {
            id.push(c.to_ascii_uppercase());
        }
    }

    // Pad with characters from the first word if needed
    if id.len() < 3
        && let Some(first_word) = words.first()
    {
        for c in first_word.chars().skip(1) {
            if id.len() >= 3 {
                break;
            }
            id.push(c.to_ascii_uppercase());
        }
    }

    // Ensure at least 3 characters
    while id.len() < 3 {
        id.push('X');
    }

    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_project_id() {
        assert_eq!(generate_project_id("My Cool Project"), "MCP");
        assert_eq!(generate_project_id("Test"), "TES");
        assert_eq!(generate_project_id("A B"), "ABX"); // A + B + X (padding)
        assert_eq!(generate_project_id("Hello World"), "HWE"); // H + W + E (from Hello)
        assert_eq!(generate_project_id("One Two Three Four"), "OTT");
    }

    use crate::config::types::ResolvedConfig;
    use crate::domain::context::CreationContext;
    use crate::domain::traits::{NoteIdentity, NoteLifecycle};
    use crate::types::TypeRegistry;
    use std::collections::HashMap;

    fn make_test_config(vault_root: &std::path::Path) -> ResolvedConfig {
        ResolvedConfig {
            active_profile: "test".into(),
            vault_root: vault_root.to_path_buf(),
            templates_dir: vault_root.join(".mdvault/templates"),
            captures_dir: vault_root.join(".mdvault/captures"),
            macros_dir: vault_root.join(".mdvault/macros"),
            typedefs_dir: vault_root.join(".mdvault/typedefs"),
            typedefs_fallback_dir: None,
            excluded_folders: vec![],
            security: Default::default(),
            logging: Default::default(),
            activity: Default::default(),
        }
    }

    #[test]
    fn test_output_path_default() {
        let dir = tempfile::tempdir().unwrap();
        let config = Box::leak(Box::new(make_test_config(dir.path())));
        let registry = Box::leak(Box::new(TypeRegistry::new()));
        let mut ctx =
            CreationContext::new("project", "My Cool Project", config, registry);

        let behavior = ProjectBehavior::new(None);
        behavior.before_create(&mut ctx).unwrap();

        let path = behavior.output_path(&ctx).unwrap();
        let expected = dir.path().join("Projects/MCP/MCP.md");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_before_create_sets_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let config = Box::leak(Box::new(make_test_config(dir.path())));
        let registry = Box::leak(Box::new(TypeRegistry::new()));
        let mut ctx =
            CreationContext::new("project", "My Cool Project", config, registry);

        let behavior = ProjectBehavior::new(None);
        behavior.before_create(&mut ctx).unwrap();

        assert_eq!(ctx.core_metadata.project_id.as_deref(), Some("MCP"));
        assert_eq!(ctx.core_metadata.task_counter, Some(0));
        assert_eq!(ctx.vars.get("project-id").map(|s| s.as_str()), Some("MCP"));
        assert_eq!(ctx.vars.get("task_counter").map(|s| s.as_str()), Some("0"));
    }

    #[test]
    fn test_before_create_uses_provided_id() {
        let dir = tempfile::tempdir().unwrap();
        let config = Box::leak(Box::new(make_test_config(dir.path())));
        let registry = Box::leak(Box::new(TypeRegistry::new()));
        let mut vars = HashMap::new();
        vars.insert("project-id".into(), "CUS".into());
        let mut ctx =
            CreationContext::new("project", "My Cool Project", config, registry)
                .with_vars(vars);

        let behavior = ProjectBehavior::new(None);
        behavior.before_create(&mut ctx).unwrap();

        assert_eq!(ctx.core_metadata.project_id.as_deref(), Some("CUS"));
    }
}
