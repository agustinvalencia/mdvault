//! Daily note type behavior.
//!
//! Daily notes have:
//! - Date-based identity (no ID, uses date)
//! - Output path: Journal/Daily/{date}.md
//! - date field in frontmatter

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Local;

use crate::types::TypeDefinition;

use super::super::context::{CreationContext, FieldPrompt, PromptContext};
use super::super::traits::{
    DomainError, DomainResult, NoteBehavior, NoteIdentity, NoteLifecycle, NotePrompts,
};

/// Behavior implementation for daily notes.
pub struct DailyBehavior {
    typedef: Option<Arc<TypeDefinition>>,
}

impl DailyBehavior {
    /// Create a new DailyBehavior, optionally wrapping a Lua typedef override.
    pub fn new(typedef: Option<Arc<TypeDefinition>>) -> Self {
        Self { typedef }
    }
}

impl NoteIdentity for DailyBehavior {
    fn generate_id(&self, _ctx: &CreationContext) -> DomainResult<Option<String>> {
        // Daily notes don't have IDs, they use dates
        Ok(None)
    }

    fn output_path(&self, ctx: &CreationContext) -> DomainResult<PathBuf> {
        let date = ctx
            .core_metadata
            .date
            .as_ref()
            .ok_or_else(|| DomainError::PathResolution("date not set".into()))?;

        // Check Lua typedef for output template
        if let Some(ref td) = self.typedef
            && let Some(ref _output) = td.output
        {
            // TODO: render_output_path(output, ctx)
        }

        // Default: Journal/Daily/YYYY-MM-DD.md
        Ok(ctx.config.vault_root.join(format!("Journal/Daily/{}.md", date)))
    }

    fn core_fields(&self) -> Vec<&'static str> {
        vec!["type", "date"]
    }
}

impl NoteLifecycle for DailyBehavior {
    fn before_create(&self, ctx: &mut CreationContext) -> DomainResult<()> {
        // Use title as date if it looks like a date, otherwise use today
        let date = if looks_like_date(&ctx.title) {
            ctx.title.clone()
        } else {
            Local::now().format("%Y-%m-%d").to_string()
        };

        ctx.core_metadata.date = Some(date.clone());
        ctx.core_metadata.title = Some(date.clone());
        ctx.set_var("date", &date);

        Ok(())
    }

    fn after_create(&self, _ctx: &CreationContext, _content: &str) -> DomainResult<()> {
        // TODO: Run Lua on_create hook if defined
        Ok(())
    }
}

impl NotePrompts for DailyBehavior {
    fn type_prompts(&self, _ctx: &PromptContext) -> Vec<FieldPrompt> {
        vec![] // No type-specific prompts for daily notes
    }

    fn should_prompt_schema(&self) -> bool {
        false // Daily notes typically don't need prompts
    }
}

impl NoteBehavior for DailyBehavior {
    fn type_name(&self) -> &'static str {
        "daily"
    }
}

/// Check if a string looks like a date (YYYY-MM-DD format).
fn looks_like_date(s: &str) -> bool {
    if s.len() != 10 {
        return false;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_date() {
        assert!(looks_like_date("2025-01-11"));
        assert!(looks_like_date("2024-12-31"));
        assert!(!looks_like_date("2025-1-11"));
        assert!(!looks_like_date("not a date"));
        assert!(!looks_like_date("01-11-2025"));
    }
}
