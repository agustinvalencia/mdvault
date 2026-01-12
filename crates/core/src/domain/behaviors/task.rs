//! Task note type behavior.
//!
//! Tasks have:
//! - ID generated from project counter (TST-001) or inbox (INB-001)
//! - Project selector prompt
//! - Logging to daily note
//! - Output path: Projects/{project}/Tasks/{id}.md or Inbox/{id}.md

use std::path::PathBuf;
use std::sync::Arc;

use crate::types::TypeDefinition;

use super::super::context::{CreationContext, FieldPrompt, PromptContext, PromptType};
use super::super::traits::{
    DomainError, DomainResult, NoteBehavior, NoteIdentity, NoteLifecycle, NotePrompts,
};

/// Behavior implementation for task notes.
pub struct TaskBehavior {
    typedef: Option<Arc<TypeDefinition>>,
}

impl TaskBehavior {
    /// Create a new TaskBehavior, optionally wrapping a Lua typedef override.
    pub fn new(typedef: Option<Arc<TypeDefinition>>) -> Self {
        Self { typedef }
    }
}

impl NoteIdentity for TaskBehavior {
    fn generate_id(&self, ctx: &CreationContext) -> DomainResult<Option<String>> {
        // ID generation is handled in before_create after project is known
        // Return existing ID if already set
        if let Some(ref id) = ctx.core_metadata.task_id {
            return Ok(Some(id.clone()));
        }
        Ok(None)
    }

    fn output_path(&self, ctx: &CreationContext) -> DomainResult<PathBuf> {
        let task_id = ctx
            .core_metadata
            .task_id
            .as_ref()
            .ok_or_else(|| DomainError::PathResolution("task-id not set".into()))?;

        let project = ctx.get_var("project").unwrap_or("inbox");

        // Check Lua typedef for output template first
        if let Some(ref td) = self.typedef
            && let Some(ref _output) = td.output
        {
            // TODO: render_output_path(output, ctx)
        }

        // Default path
        if project == "inbox" {
            Ok(ctx.config.vault_root.join(format!("Inbox/{}.md", task_id)))
        } else {
            Ok(ctx
                .config
                .vault_root
                .join(format!("Projects/{}/Tasks/{}.md", project, task_id)))
        }
    }

    fn core_fields(&self) -> Vec<&'static str> {
        vec!["type", "title", "task-id", "project"]
    }
}

impl NoteLifecycle for TaskBehavior {
    fn before_create(&self, ctx: &mut CreationContext) -> DomainResult<()> {
        let project = ctx
            .get_var("project")
            .map(|s| s.to_string())
            .unwrap_or_else(|| "inbox".into());

        // Generate task ID based on project
        let task_id = if project == "inbox" {
            generate_inbox_task_id(&ctx.config.vault_root)?
        } else {
            // Get project counter and generate ID
            let (project_id, counter) = get_project_info(ctx.config, &project)?;
            format!("{}-{:03}", project_id, counter + 1)
        };

        // Set core metadata
        ctx.core_metadata.task_id = Some(task_id.clone());
        ctx.core_metadata.project =
            if project == "inbox" { None } else { Some(project.clone()) };
        ctx.set_var("task-id", &task_id);
        if project != "inbox" {
            ctx.set_var("project", &project);
        }

        Ok(())
    }

    fn after_create(&self, ctx: &CreationContext, _content: &str) -> DomainResult<()> {
        let project = ctx.get_var("project").unwrap_or("inbox");

        // Increment project counter if not inbox
        if project != "inbox" {
            increment_project_counter(ctx.config, project)?;
        }

        // TODO: Log to daily note
        // TODO: Run Lua on_create hook if defined
        // TODO: Reindex vault

        Ok(())
    }
}

impl NotePrompts for TaskBehavior {
    fn type_prompts(&self, ctx: &PromptContext) -> Vec<FieldPrompt> {
        let mut prompts = vec![];

        // Project selector (if not provided)
        if !ctx.provided_vars.contains_key("project") && !ctx.batch_mode {
            prompts.push(FieldPrompt {
                field_name: "project".into(),
                prompt_text: "Select project for this task".into(),
                prompt_type: PromptType::ProjectSelector,
                required: false, // Can be inbox
                default_value: Some("inbox".into()),
            });
        }

        prompts
    }
}

impl NoteBehavior for TaskBehavior {
    fn type_name(&self) -> &'static str {
        "task"
    }
}

// --- Helper functions (to be moved/refactored) ---

use crate::config::types::ResolvedConfig;
use std::fs;

/// Generate an inbox task ID by scanning the Inbox directory.
fn generate_inbox_task_id(vault_root: &std::path::Path) -> DomainResult<String> {
    let inbox_dir = vault_root.join("Inbox");

    let mut max_num = 0u32;

    if inbox_dir.exists() {
        for entry in fs::read_dir(&inbox_dir).map_err(DomainError::Io)? {
            let entry = entry.map_err(DomainError::Io)?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Parse INB-XXX.md pattern
            if let Some(stem) = name_str.strip_suffix(".md")
                && let Some(num_str) = stem.strip_prefix("INB-")
                && let Ok(num) = num_str.parse::<u32>()
            {
                max_num = max_num.max(num);
            }
        }
    }

    Ok(format!("INB-{:03}", max_num + 1))
}

/// Get project info (project-id and task_counter) from project file.
fn get_project_info(
    config: &ResolvedConfig,
    project: &str,
) -> DomainResult<(String, u32)> {
    let project_file = find_project_file(config, project)?;

    let content = fs::read_to_string(&project_file).map_err(DomainError::Io)?;

    // Parse frontmatter
    let parsed = crate::frontmatter::parse(&content).map_err(|e| {
        DomainError::Other(format!("Failed to parse project frontmatter: {}", e))
    })?;

    let fields = parsed.frontmatter.map(|fm| fm.fields).unwrap_or_default();

    let project_id = fields
        .get("project-id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| project.to_uppercase());

    let counter = fields
        .get("task_counter")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .unwrap_or(0);

    Ok((project_id, counter))
}

/// Find the project file by project name/ID.
fn find_project_file(config: &ResolvedConfig, project: &str) -> DomainResult<PathBuf> {
    // Try common patterns
    let patterns = [
        format!("Projects/{}/{}.md", project, project),
        format!("Projects/{}.md", project),
        format!("projects/{}/{}.md", project.to_lowercase(), project.to_lowercase()),
    ];

    for pattern in &patterns {
        let path = config.vault_root.join(pattern);
        if path.exists() {
            return Ok(path);
        }
    }

    Err(DomainError::Other(format!("Project file not found for: {}", project)))
}

/// Increment the task_counter in a project file.
fn increment_project_counter(config: &ResolvedConfig, project: &str) -> DomainResult<()> {
    let project_file = find_project_file(config, project)?;

    let content = fs::read_to_string(&project_file).map_err(DomainError::Io)?;

    // Parse frontmatter
    let parsed = crate::frontmatter::parse(&content).map_err(|e| {
        DomainError::Other(format!("Failed to parse project frontmatter: {}", e))
    })?;

    let mut fields = parsed.frontmatter.map(|fm| fm.fields).unwrap_or_default();

    let current = fields
        .get("task_counter")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .unwrap_or(0);

    fields.insert(
        "task_counter".to_string(),
        serde_yaml::Value::Number((current + 1).into()),
    );

    // Rebuild content with updated frontmatter
    let yaml = serde_yaml::to_string(&fields).map_err(|e| {
        DomainError::Other(format!("Failed to serialize frontmatter: {}", e))
    })?;

    let new_content = format!("---\n{}---\n{}", yaml, parsed.body);
    fs::write(&project_file, new_content).map_err(DomainError::Io)?;

    Ok(())
}
