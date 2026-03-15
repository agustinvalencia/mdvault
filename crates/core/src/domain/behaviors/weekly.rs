//! Weekly note type behavior.
//!
//! Weekly notes have:
//! - Week-based identity (no ID, uses week)
//! - Output path: Journal/{year}/Weekly/{week}.md
//! - week field in frontmatter (YYYY-WXX format)

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{Local, NaiveDate, Weekday};

use crate::types::TypeDefinition;
use crate::vars::datemath::{is_date_expr, try_evaluate_date_expr};

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
        // Check Lua typedef for output template first
        if let Some(ref td) = self.typedef
            && let Some(ref output) = td.output
        {
            return super::render_output_template(output, ctx);
        }

        // Default: Journal/{year}/Weekly/YYYY-WXX.md
        let week = ctx
            .core_metadata
            .week
            .as_ref()
            .ok_or_else(|| DomainError::PathResolution("week not set".into()))?;
        let year = &week[..4];

        Ok(ctx.config.vault_root.join(format!("Journal/{}/Weekly/{}.md", year, week)))
    }

    fn core_fields(&self) -> Vec<&'static str> {
        vec!["type", "week"]
    }
}

impl NoteLifecycle for WeeklyBehavior {
    fn before_create(&self, ctx: &mut CreationContext) -> DomainResult<()> {
        // Determine the week: title takes priority (it's the user's intent), then
        // explicit --var week=..., then schema default, then current week.
        // Title is checked first because the schema default for `week` eagerly
        // evaluates to "today's week" and would shadow a date-based title.
        let week = if looks_like_week(&ctx.title) {
            // Title is already a week string (e.g. "2026-W13")
            ctx.title.clone()
        } else if is_date_expr(&ctx.title) {
            // Title is a date or date expression (e.g. "2026-03-23", "next week") —
            // evaluate it as a week. This takes priority over --var week= / schema
            // defaults because the title is the user's explicit intent.
            let expr_to_eval = if !ctx.title.contains('|') {
                format!("{} | %G-W%V", ctx.title)
            } else {
                ctx.title.clone()
            };

            try_evaluate_date_expr(&expr_to_eval)
                .unwrap_or_else(|| Local::now().format("%G-W%V").to_string())
        } else if let Some(provided) = ctx.get_var("week")
            && looks_like_week(provided)
        {
            // Explicit --var week=... or schema default
            provided.to_string()
        } else {
            // Fallback: current week
            Local::now().format("%G-W%V").to_string()
        };

        ctx.core_metadata.week = Some(week.clone());
        ctx.core_metadata.title = Some(week.clone());
        ctx.set_var("week", &week);

        // Set reference_date and core date to the Monday of this week so that
        // date format filters and template variables resolve to the correct week.
        if week.len() >= 7
            && week.contains("-W")
            && let Ok(year) = week[..4].parse::<i32>()
            && let Ok(wk) = week[6..].parse::<u32>()
            && let Some(monday) = NaiveDate::from_isoywd_opt(year, wk, Weekday::Mon)
        {
            ctx.reference_date = Some(monday);
            let date_str = monday.format("%Y-%m-%d").to_string();
            ctx.core_metadata.date = Some(date_str.clone());
            ctx.set_var("date", &date_str);
        }

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
    fn test_date_title_resolves_to_correct_week() {
        // Regression test for MDV-034: dates should resolve to their ISO week,
        // not the current week.
        use crate::vars::datemath::try_evaluate_date_expr;

        // Simulate what before_create does when title is a date
        let title = "2026-03-23";
        assert!(!looks_like_week(title));

        let expr = format!("{} | %G-W%V", title);
        let week = try_evaluate_date_expr(&expr).unwrap();
        assert_eq!(week, "2026-W13");

        // Also test a different date
        let expr2 = format!("{} | %G-W%V", "2026-03-16");
        let week2 = try_evaluate_date_expr(&expr2).unwrap();
        assert_eq!(week2, "2026-W12");
    }

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
