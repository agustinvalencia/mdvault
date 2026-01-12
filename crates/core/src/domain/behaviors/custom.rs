//! Custom (Lua-driven) note type behavior.
//!
//! Custom types delegate everything to their Lua typedef:
//! - Output path from typedef.output template
//! - Prompts from typedef.schema
//! - Hooks from typedef.on_create

use std::path::PathBuf;
use std::sync::Arc;

use crate::types::TypeDefinition;

use super::super::context::{CreationContext, FieldPrompt, PromptContext};
use super::super::traits::{
    DomainResult, NoteBehavior, NoteIdentity, NoteLifecycle, NotePrompts,
};

/// Behavior implementation for custom (Lua-defined) note types.
pub struct CustomBehavior {
    typedef: Arc<TypeDefinition>,
    type_name: String,
}

impl CustomBehavior {
    /// Create a new CustomBehavior wrapping a Lua typedef.
    pub fn new(typedef: Arc<TypeDefinition>) -> Self {
        let type_name = typedef.name.clone();
        Self { typedef, type_name }
    }

    /// Get the underlying typedef.
    pub fn typedef(&self) -> &TypeDefinition {
        &self.typedef
    }
}

impl NoteIdentity for CustomBehavior {
    fn generate_id(&self, _ctx: &CreationContext) -> DomainResult<Option<String>> {
        // Custom types don't generate IDs in Rust
        // Lua hooks can set them
        Ok(None)
    }

    fn output_path(&self, ctx: &CreationContext) -> DomainResult<PathBuf> {
        // Use Lua typedef output template if available
        if let Some(ref _output) = self.typedef.output {
            // TODO: render_output_path(output, ctx)
        }

        // Default: {type}s/{slug}.md
        let slug = slugify(&ctx.title);
        Ok(ctx.config.vault_root.join(format!("{}s/{}.md", self.type_name, slug)))
    }

    fn core_fields(&self) -> Vec<&'static str> {
        vec!["type", "title"]
    }
}

impl NoteLifecycle for CustomBehavior {
    fn before_create(&self, _ctx: &mut CreationContext) -> DomainResult<()> {
        // No Rust-side before_create for custom types
        Ok(())
    }

    fn after_create(&self, _ctx: &CreationContext, _content: &str) -> DomainResult<()> {
        // TODO: Run Lua on_create hook if defined
        Ok(())
    }
}

impl NotePrompts for CustomBehavior {
    fn type_prompts(&self, _ctx: &PromptContext) -> Vec<FieldPrompt> {
        vec![] // Custom types only use schema-based prompts
    }
}

impl NoteBehavior for CustomBehavior {
    fn type_name(&self) -> &'static str {
        // This is a bit awkward because we need 'static lifetime
        // In practice, we'll use the type_name from the typedef
        // For now, return a placeholder - the actual type name is in self.type_name
        "custom"
    }
}

/// Convert a title to a URL-friendly slug.
fn slugify(s: &str) -> String {
    let mut result = String::with_capacity(s.len());

    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            result.push(c.to_ascii_lowercase());
        } else if (c == ' ' || c == '_' || c == '-') && !result.ends_with('-') {
            result.push('-');
        }
    }

    result.trim_matches('-').to_string()
}
