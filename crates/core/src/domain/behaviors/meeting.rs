//! Meeting note type behavior.
//!
//! Meetings have:
//! - ID generated from date and counter (MTG-2025-01-15-001)
//! - Date prompt (defaults to today)
//! - Attendees prompt
//! - Logging to daily note
//! - Output path: Meetings/{year}/{id}.md

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Local;

use crate::types::TypeDefinition;

use super::super::context::{CreationContext, FieldPrompt, PromptContext, PromptType};
use super::super::traits::{
    DomainError, DomainResult, NoteBehavior, NoteIdentity, NoteLifecycle, NotePrompts,
};

/// Behavior implementation for meeting notes.
pub struct MeetingBehavior {
    typedef: Option<Arc<TypeDefinition>>,
}

impl MeetingBehavior {
    /// Create a new MeetingBehavior, optionally wrapping a Lua typedef override.
    pub fn new(typedef: Option<Arc<TypeDefinition>>) -> Self {
        Self { typedef }
    }
}

impl NoteIdentity for MeetingBehavior {
    fn generate_id(&self, ctx: &CreationContext) -> DomainResult<Option<String>> {
        // ID generation is handled in before_create
        // Return existing ID if already set
        if let Some(ref id) = ctx.core_metadata.meeting_id {
            return Ok(Some(id.clone()));
        }
        Ok(None)
    }

    fn output_path(&self, ctx: &CreationContext) -> DomainResult<PathBuf> {
        // Check Lua typedef for output template first
        if let Some(ref td) = self.typedef
            && let Some(ref output) = td.output
        {
            return super::render_output_template(output, ctx);
        }

        // Default path: Meetings/{year}/{meeting-id}.md
        let meeting_id =
            ctx.core_metadata.meeting_id.as_ref().ok_or_else(|| {
                DomainError::PathResolution("meeting-id not set".into())
            })?;
        let date = ctx
            .core_metadata
            .date
            .as_ref()
            .ok_or_else(|| DomainError::PathResolution("date not set".into()))?;
        let year = &date[..4];

        Ok(ctx.config.vault_root.join(format!("Meetings/{}/{}.md", year, meeting_id)))
    }

    fn core_fields(&self) -> Vec<&'static str> {
        vec!["type", "title", "meeting-id", "date", "attendees"]
    }
}

impl NoteLifecycle for MeetingBehavior {
    fn before_create(&self, ctx: &mut CreationContext) -> DomainResult<()> {
        // Get or default date to today
        let date = ctx
            .get_var("date")
            .map(|s| s.to_string())
            .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());

        // Generate meeting ID: MTG-YYYY-MM-DD-NNN
        let meeting_id = generate_meeting_id(&ctx.config.vault_root, &date)?;

        // Set core metadata
        ctx.core_metadata.meeting_id = Some(meeting_id.clone());
        ctx.core_metadata.date = Some(date.clone());
        ctx.set_var("meeting-id", &meeting_id);
        ctx.set_var("date", &date);

        Ok(())
    }

    fn after_create(&self, ctx: &CreationContext, _content: &str) -> DomainResult<()> {
        // Log to daily note
        if let Some(ref output_path) = ctx.output_path {
            let meeting_id = ctx.core_metadata.meeting_id.as_deref().unwrap_or("");
            if let Err(e) = super::super::services::DailyLogService::log_creation(
                ctx.config,
                "meeting",
                &ctx.title,
                meeting_id,
                output_path,
            ) {
                // Log warning but don't fail the creation
                tracing::warn!("Failed to log to daily note: {}", e);
            }
        }

        Ok(())
    }
}

impl NotePrompts for MeetingBehavior {
    fn type_prompts(&self, ctx: &PromptContext) -> Vec<FieldPrompt> {
        let mut prompts = vec![];

        // Date prompt (if not provided)
        if !ctx.provided_vars.contains_key("date") && !ctx.batch_mode {
            prompts.push(FieldPrompt {
                field_name: "date".into(),
                prompt_text: "Meeting date".into(),
                prompt_type: PromptType::Text,
                required: false,
                default_value: Some(Local::now().format("%Y-%m-%d").to_string()),
            });
        }

        // Attendees prompt (if not provided)
        if !ctx.provided_vars.contains_key("attendees") && !ctx.batch_mode {
            prompts.push(FieldPrompt {
                field_name: "attendees".into(),
                prompt_text: "Who's attending?".into(),
                prompt_type: PromptType::Text,
                required: false,
                default_value: None,
            });
        }

        prompts
    }
}

impl NoteBehavior for MeetingBehavior {
    fn type_name(&self) -> &'static str {
        "meeting"
    }
}

// --- Helper functions ---

use std::fs;

/// Generate a meeting ID by scanning the Meetings directory for the given date.
fn generate_meeting_id(vault_root: &std::path::Path, date: &str) -> DomainResult<String> {
    let year = &date[..4];
    let meetings_dir = vault_root.join("Meetings").join(year);
    let prefix = format!("MTG-{}-", date);

    let mut max_num = 0u32;

    if meetings_dir.exists() {
        for entry in fs::read_dir(&meetings_dir).map_err(DomainError::Io)? {
            let entry = entry.map_err(DomainError::Io)?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Parse MTG-YYYY-MM-DD-XXX.md pattern
            if let Some(stem) = name_str.strip_suffix(".md")
                && stem.starts_with(&prefix)
                && let Some(num_str) = stem.strip_prefix(&prefix)
                && let Ok(num) = num_str.parse::<u32>()
            {
                max_num = max_num.max(num);
            }
        }
    }

    Ok(format!("{}{:03}", prefix, max_num + 1))
}
