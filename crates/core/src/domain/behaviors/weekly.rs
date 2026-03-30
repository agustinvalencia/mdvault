//! Weekly note type behavior.
//!
//! Weekly notes have:
//! - Week-based identity (no ID, uses week)
//! - Output path: Journal/{year}/Weekly/{week}.md
//! - week field in frontmatter (YYYY-WXX format)

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{Local, NaiveDate, Weekday};

use crate::paths::PathResolver;
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
        Ok(PathResolver::new(&ctx.config.vault_root).weekly_note(week))
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

    fn after_create(&self, ctx: &CreationContext, content: &str) -> DomainResult<()> {
        if let (Some(runner), Some(output_path)) = (ctx.hook_runner, &ctx.output_path)
            && let Err(e) = runner.run_on_create(output_path, content)
        {
            tracing::warn!("on_create hook failed: {e}");
        }
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
    use crate::config::types::{
        ActivityConfig, LoggingConfig, ResolvedConfig, SecurityPolicy,
    };
    use crate::domain::context::CreationContext;
    use crate::domain::traits::NoteLifecycle;
    use crate::types::TypeRegistry;
    use chrono::Datelike;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn test_config() -> ResolvedConfig {
        ResolvedConfig {
            active_profile: "test".into(),
            vault_root: PathBuf::from("/tmp/test-vault"),
            templates_dir: PathBuf::from("/tmp/test-vault/.mdvault/templates"),
            captures_dir: PathBuf::from("/tmp/test-vault/.mdvault/captures"),
            macros_dir: PathBuf::from("/tmp/test-vault/.mdvault/macros"),
            typedefs_dir: PathBuf::from("/tmp/test-vault/.mdvault/types"),
            typedefs_fallback_dir: None,
            excluded_folders: vec![],
            security: SecurityPolicy::default(),
            logging: LoggingConfig::default(),
            activity: ActivityConfig::default(),
        }
    }

    fn run_before_create(
        title: &str,
        vars: HashMap<String, String>,
    ) -> CreationContext<'_> {
        // Leak to get 'static lifetime — fine for tests
        let cfg = Box::leak(Box::new(test_config()));
        let registry = Box::leak(Box::new(TypeRegistry::new()));
        let mut ctx =
            CreationContext::new("weekly", title, cfg, registry).with_vars(vars);
        let behavior = WeeklyBehavior::new(None);
        behavior.before_create(&mut ctx).unwrap();
        ctx
    }

    // ── before_create tests (MDV-034 regression) ─────────────────────────

    #[test]
    fn before_create_date_title_resolves_correct_week() {
        // MDV-034: "2026-03-23" should resolve to W13, not the current week
        let ctx = run_before_create("2026-03-23", HashMap::new());
        assert_eq!(ctx.core_metadata.week.as_deref(), Some("2026-W13"));
        assert_eq!(ctx.core_metadata.date.as_deref(), Some("2026-03-23")); // Monday of W13
        assert_eq!(ctx.vars.get("week").map(|s| s.as_str()), Some("2026-W13"));
        assert!(ctx.reference_date.is_some());
    }

    #[test]
    fn before_create_date_title_beats_schema_default() {
        // The core bug: schema default sets week="2026-W11" (today's week),
        // but title "2026-03-23" should override it to W13
        let mut vars = HashMap::new();
        vars.insert("week".into(), "2026-W11".into()); // Simulates schema default
        let ctx = run_before_create("2026-03-23", vars);
        assert_eq!(ctx.core_metadata.week.as_deref(), Some("2026-W13"));
    }

    #[test]
    fn before_create_explicit_week_title() {
        let ctx = run_before_create("2026-W05", HashMap::new());
        assert_eq!(ctx.core_metadata.week.as_deref(), Some("2026-W05"));
        assert_eq!(ctx.core_metadata.date.as_deref(), Some("2026-01-26")); // Monday of W05
    }

    #[test]
    fn before_create_next_week_expr() {
        let ctx = run_before_create("next week", HashMap::new());
        let week = ctx.core_metadata.week.as_deref().unwrap();
        assert!(looks_like_week(week), "Expected week format, got: {week}");
    }

    #[test]
    fn before_create_placeholder_title_uses_var_week() {
        // When title is not a date or week, fall back to --var week=
        let mut vars = HashMap::new();
        vars.insert("week".into(), "2026-W30".into());
        let ctx = run_before_create("placeholder", vars);
        assert_eq!(ctx.core_metadata.week.as_deref(), Some("2026-W30"));
    }

    #[test]
    fn before_create_no_title_no_var_falls_back_to_now() {
        let ctx = run_before_create("", HashMap::new());
        let week = ctx.core_metadata.week.as_deref().unwrap();
        assert!(looks_like_week(week), "Expected week format, got: {week}");
    }

    #[test]
    fn before_create_sets_reference_date_to_monday() {
        let ctx = run_before_create("2026-W13", HashMap::new());
        let monday = ctx.reference_date.unwrap();
        assert_eq!(monday.format("%Y-%m-%d").to_string(), "2026-03-23");
        assert_eq!(monday.weekday(), Weekday::Mon);
    }

    // ── looks_like_week ──────────────────────────────────────────────────

    #[test]
    fn test_looks_like_week() {
        assert!(looks_like_week("2025-W01"));
        assert!(looks_like_week("2025-W52"));
        assert!(looks_like_week("2024-W1"));
        assert!(!looks_like_week("2025-01"));
        assert!(!looks_like_week("not a week"));
        assert!(!looks_like_week("W01-2025"));
        assert!(!looks_like_week("2026-03-23"));
    }

    #[test]
    fn test_output_path_default() {
        let ctx = run_before_create("2026-W13", HashMap::new());
        let behavior = WeeklyBehavior::new(None);
        let path = behavior.output_path(&ctx).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/tmp/test-vault/Journal/2026/Weekly/2026-W13.md")
        );
    }
}
