//! Daily note type behavior.
//!
//! Daily notes have:
//! - Date-based identity (no ID, uses date)
//! - Output path: Journal/{year}/Daily/{date}.md
//! - date field in frontmatter

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{Datelike, Local, NaiveDate};

use crate::paths::PathResolver;
use crate::types::TypeDefinition;
use crate::vars::datemath::try_evaluate_date_expr;

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
        // Check Lua typedef for output template first
        if let Some(ref td) = self.typedef
            && let Some(ref output) = td.output
        {
            return super::render_output_template(output, ctx);
        }

        // Default: Journal/{year}/Daily/YYYY-MM-DD.md
        let date = ctx
            .core_metadata
            .date
            .as_ref()
            .ok_or_else(|| DomainError::PathResolution("date not set".into()))?;
        Ok(PathResolver::new(&ctx.config.vault_root).daily_note(date))
    }

    fn core_fields(&self) -> Vec<&'static str> {
        vec!["type", "date"]
    }
}

impl NoteLifecycle for DailyBehavior {
    fn before_create(&self, ctx: &mut CreationContext) -> DomainResult<()> {
        // Check for a date provided via --var date=... first, then try title, then today
        let date = if let Some(provided) = ctx.get_var("date")
            && looks_like_date(provided)
        {
            provided.to_string()
        } else if looks_like_date(&ctx.title) {
            ctx.title.clone()
        } else if let Some(evaluated) = try_evaluate_date_expr(&ctx.title) {
            evaluated
        } else {
            Local::now().format("%Y-%m-%d").to_string()
        };

        ctx.core_metadata.date = Some(date.clone());
        ctx.core_metadata.title = Some(date.clone());
        ctx.set_var("date", &date);

        // Parse target date and set as reference for all date expressions
        if let Ok(target) = NaiveDate::parse_from_str(&date, "%Y-%m-%d") {
            ctx.reference_date = Some(target);
            // Set week var to override schema default (which was evaluated at load time)
            let week = format!(
                "[[{}-W{:02}]]",
                target.iso_week().year(),
                target.iso_week().week()
            );
            ctx.set_var("week", &week);
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
        let mut ctx = CreationContext::new("daily", "2026-03-15", config, registry);

        let behavior = DailyBehavior::new(None);
        behavior.before_create(&mut ctx).unwrap();

        let path = behavior.output_path(&ctx).unwrap();
        let expected = dir.path().join("Journal/2026/Daily/2026-03-15.md");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_before_create_with_date_var() {
        let dir = tempfile::tempdir().unwrap();
        let config = Box::leak(Box::new(make_test_config(dir.path())));
        let registry = Box::leak(Box::new(TypeRegistry::new()));
        let mut vars = HashMap::new();
        vars.insert("date".into(), "2026-03-15".into());
        let mut ctx = CreationContext::new("daily", "placeholder", config, registry)
            .with_vars(vars);

        let behavior = DailyBehavior::new(None);
        behavior.before_create(&mut ctx).unwrap();

        assert_eq!(ctx.core_metadata.date.as_deref(), Some("2026-03-15"));
    }

    #[test]
    fn test_before_create_with_date_title() {
        let dir = tempfile::tempdir().unwrap();
        let config = Box::leak(Box::new(make_test_config(dir.path())));
        let registry = Box::leak(Box::new(TypeRegistry::new()));
        let mut ctx = CreationContext::new("daily", "2026-03-15", config, registry);

        let behavior = DailyBehavior::new(None);
        behavior.before_create(&mut ctx).unwrap();

        assert_eq!(ctx.core_metadata.date.as_deref(), Some("2026-03-15"));
        assert_eq!(ctx.core_metadata.title.as_deref(), Some("2026-03-15"));
    }

    #[test]
    fn test_before_create_sets_week_var() {
        let dir = tempfile::tempdir().unwrap();
        let config = Box::leak(Box::new(make_test_config(dir.path())));
        let registry = Box::leak(Box::new(TypeRegistry::new()));
        let mut ctx = CreationContext::new("daily", "2026-03-15", config, registry);

        let behavior = DailyBehavior::new(None);
        behavior.before_create(&mut ctx).unwrap();

        // 2026-03-15 is in ISO week 11
        assert_eq!(ctx.vars.get("week").map(|s| s.as_str()), Some("[[2026-W11]]"));
    }

    use crate::domain::context::HookRunner;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Mock HookRunner that records whether it was called.
    struct MockHookRunner {
        called: AtomicBool,
    }

    impl MockHookRunner {
        fn new() -> Self {
            Self { called: AtomicBool::new(false) }
        }

        fn was_called(&self) -> bool {
            self.called.load(Ordering::SeqCst)
        }
    }

    impl HookRunner for MockHookRunner {
        fn run_on_create(
            &self,
            _output_path: &std::path::Path,
            _content: &str,
        ) -> Result<(), String> {
            self.called.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn test_after_create_calls_hook_runner() {
        let dir = tempfile::tempdir().unwrap();
        let config = Box::leak(Box::new(make_test_config(dir.path())));
        let registry = Box::leak(Box::new(TypeRegistry::new()));
        let runner = MockHookRunner::new();

        let mut ctx = CreationContext::new("daily", "2026-03-15", config, registry)
            .with_hook_runner(&runner);

        let behavior = DailyBehavior::new(None);
        behavior.before_create(&mut ctx).unwrap();
        ctx.output_path = Some(dir.path().join("Journal/2026/Daily/2026-03-15.md"));

        behavior.after_create(&ctx, "---\ndate: 2026-03-15\n---\n").unwrap();

        assert!(runner.was_called(), "HookRunner should have been called");
    }

    #[test]
    fn test_after_create_without_hook_runner() {
        let dir = tempfile::tempdir().unwrap();
        let config = Box::leak(Box::new(make_test_config(dir.path())));
        let registry = Box::leak(Box::new(TypeRegistry::new()));

        let mut ctx = CreationContext::new("daily", "2026-03-15", config, registry);

        let behavior = DailyBehavior::new(None);
        behavior.before_create(&mut ctx).unwrap();
        ctx.output_path = Some(dir.path().join("Journal/2026/Daily/2026-03-15.md"));

        // Should not fail even without a hook runner
        behavior.after_create(&ctx, "---\ndate: 2026-03-15\n---\n").unwrap();
    }

    #[test]
    fn test_after_create_hook_error_is_non_fatal() {
        struct FailingHookRunner;
        impl HookRunner for FailingHookRunner {
            fn run_on_create(
                &self,
                _output_path: &std::path::Path,
                _content: &str,
            ) -> Result<(), String> {
                Err("hook exploded".into())
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let config = Box::leak(Box::new(make_test_config(dir.path())));
        let registry = Box::leak(Box::new(TypeRegistry::new()));
        let runner = FailingHookRunner;

        let mut ctx = CreationContext::new("daily", "2026-03-15", config, registry)
            .with_hook_runner(&runner);

        let behavior = DailyBehavior::new(None);
        behavior.before_create(&mut ctx).unwrap();
        ctx.output_path = Some(dir.path().join("Journal/2026/Daily/2026-03-15.md"));

        // Should succeed even when the hook fails (errors are warnings, not fatal)
        let result = behavior.after_create(&ctx, "---\ndate: 2026-03-15\n---\n");
        assert!(result.is_ok(), "Hook failure should not propagate as error");
    }
}
