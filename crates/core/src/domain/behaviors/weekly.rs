//! Weekly note type behavior.
//!
//! Weekly notes have:
//! - Week-based identity (no ID, uses week)
//! - Output path: Journal/Weekly/{week}.md
//! - week field in frontmatter (YYYY-WXX format)

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Local;

use crate::types::TypeDefinition;

use super::super::context::{CreationContext, FieldPrompt, PromptContext};
use super::super::traits::{
    DomainError, DomainResult, NoteBehavior, NoteIdentity, NoteLifecycle, NotePrompts,
};

/// Behavior implementation for weekly notes.
pub struct WeeklyBehavior {
    typedef: Option<Arc<TypeDefinition>>,
}

impl WeeklyBehavior {
    /// Create a new WeeklyBehavior, optionally wrapping a Lua typedef override.
    pub fn new(typedef: Option<Arc<TypeDefinition>>) -> Self {
        Self { typedef }
    }
}

impl NoteIdentity for WeeklyBehavior {
    fn generate_id(&self, _ctx: &CreationContext) -> DomainResult<Option<String>> {
        // Weekly notes don't have IDs, they use week numbers
        Ok(None)
    }

    fn output_path(&self, ctx: &CreationContext) -> DomainResult<PathBuf> {
        let week = ctx
            .core_metadata
            .week
            .as_ref()
            .ok_or_else(|| DomainError::PathResolution("week not set".into()))?;

        // Check Lua typedef for output template
        if let Some(ref td) = self.typedef
            && let Some(ref _output) = td.output
        {
            // TODO: render_output_path(output, ctx)
        }

        // Default: Journal/Weekly/YYYY-WXX.md
        Ok(ctx.config.vault_root.join(format!("Journal/Weekly/{}.md", week)))
    }

    fn core_fields(&self) -> Vec<&'static str> {
        vec!["type", "week"]
    }
}

impl NoteLifecycle for WeeklyBehavior {
    fn before_create(&self, ctx: &mut CreationContext) -> DomainResult<()> {
        // Use title as week if it looks like a week, otherwise use current week
        let week = if looks_like_week(&ctx.title) {
            ctx.title.clone()
        } else {
            Local::now().format("%Y-W%W").to_string()
        };

        ctx.core_metadata.week = Some(week.clone());
        ctx.core_metadata.title = Some(week.clone());
        ctx.set_var("week", &week);

        Ok(())
    }

    fn after_create(&self, _ctx: &CreationContext, _content: &str) -> DomainResult<()> {
        // TODO: Run Lua on_create hook if defined
        Ok(())
    }
}

impl NotePrompts for WeeklyBehavior {
    fn type_prompts(&self, _ctx: &PromptContext) -> Vec<FieldPrompt> {
        vec![] // No type-specific prompts for weekly notes
    }

    fn should_prompt_schema(&self) -> bool {
        false // Weekly notes typically don't need prompts
    }
}

impl NoteBehavior for WeeklyBehavior {
    fn type_name(&self) -> &'static str {
        "weekly"
    }
}

/// Check if a string looks like a week (YYYY-WXX format).
fn looks_like_week(s: &str) -> bool {
    if s.len() < 7 || s.len() > 8 {
        return false;
    }
    let parts: Vec<&str> = s.split("-W").collect();
    if parts.len() != 2 {
        return false;
    }
    parts[0].len() == 4
        && (parts[1].len() == 1 || parts[1].len() == 2)
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_week() {
        assert!(looks_like_week("2025-W01"));
        assert!(looks_like_week("2025-W52"));
        assert!(looks_like_week("2024-W1"));
        assert!(!looks_like_week("2025-01"));
        assert!(!looks_like_week("not a week"));
        assert!(!looks_like_week("W01-2025"));
    }
}
