//! Project note type behavior.
//!
//! Projects have:
//! - 3-letter ID generated from title
//! - task_counter initialized to 0
//! - Logging to daily note
//! - Output path: Projects/{id}/{id}.md

use std::path::PathBuf;
use std::sync::Arc;

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
        let project_id =
            ctx.core_metadata.project_id.as_ref().ok_or_else(|| {
                DomainError::PathResolution("project-id not set".into())
            })?;

        // Check Lua typedef for output template first
        if let Some(ref td) = self.typedef
            && let Some(ref _output) = td.output
        {
            // TODO: render_output_path(output, ctx)
        }

        // Default: Projects/{id}/{id}.md
        Ok(ctx
            .config
            .vault_root
            .join(format!("Projects/{}/{}.md", project_id, project_id)))
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

    fn after_create(&self, _ctx: &CreationContext, _content: &str) -> DomainResult<()> {
        // TODO: Log to daily note
        // TODO: Run Lua on_create hook if defined
        // TODO: Reindex vault

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
}
