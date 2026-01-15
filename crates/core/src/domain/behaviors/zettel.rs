//! Zettel (knowledge note) type behavior.
//!
//! Zettels have:
//! - Minimal Rust behavior, mostly Lua-driven
//! - Output path: zettels/{slug}.md or Lua-defined

use std::path::PathBuf;
use std::sync::Arc;

use crate::types::TypeDefinition;

use super::super::context::{CreationContext, FieldPrompt, PromptContext};
use super::super::traits::{
    DomainResult, NoteBehavior, NoteIdentity, NoteLifecycle, NotePrompts,
};

/// Behavior implementation for zettel (knowledge) notes.
pub struct ZettelBehavior {
    typedef: Option<Arc<TypeDefinition>>,
}

impl ZettelBehavior {
    /// Create a new ZettelBehavior, optionally wrapping a Lua typedef override.
    pub fn new(typedef: Option<Arc<TypeDefinition>>) -> Self {
        Self { typedef }
    }
}

impl NoteIdentity for ZettelBehavior {
    fn generate_id(&self, _ctx: &CreationContext) -> DomainResult<Option<String>> {
        // Zettels don't have special IDs
        Ok(None)
    }

    fn output_path(&self, ctx: &CreationContext) -> DomainResult<PathBuf> {
        // Check Lua typedef for output template first
        if let Some(ref td) = self.typedef
            && let Some(ref output) = td.output
        {
            return super::render_output_template(output, ctx);
        }

        // Default: zettels/{slug}.md
        let slug = slugify(&ctx.title);
        Ok(ctx.config.vault_root.join(format!("zettels/{}.md", slug)))
    }

    fn core_fields(&self) -> Vec<&'static str> {
        vec!["type", "title"]
    }
}

impl NoteLifecycle for ZettelBehavior {
    fn before_create(&self, _ctx: &mut CreationContext) -> DomainResult<()> {
        // No special before_create logic for zettels
        Ok(())
    }

    fn after_create(&self, _ctx: &CreationContext, _content: &str) -> DomainResult<()> {
        // TODO: Run Lua on_create hook if defined
        Ok(())
    }
}

impl NotePrompts for ZettelBehavior {
    fn type_prompts(&self, _ctx: &PromptContext) -> Vec<FieldPrompt> {
        vec![] // Zettels use schema-based prompts only
    }
}

impl NoteBehavior for ZettelBehavior {
    fn type_name(&self) -> &'static str {
        "zettel"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("My Cool Note!"), "my-cool-note");
        assert_eq!(slugify("  spaced  out  "), "spaced-out");
        assert_eq!(slugify("under_score"), "under-score");
    }
}
