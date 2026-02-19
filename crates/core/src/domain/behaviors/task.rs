//! Task note type behavior.
//!
//! Tasks have:
//! - ID generated from project counter (TST-001) or inbox (INB-001)
//! - Project selector prompt
//! - Logging to daily note
//! - Output path: Projects/{project}/Tasks/{id}.md or Inbox/{id}.md

use std::path::{Path, PathBuf};
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
        // Check Lua typedef for output template first
        if let Some(ref td) = self.typedef
            && let Some(ref output) = td.output
        {
            return super::render_output_template(output, ctx);
        }

        // Default path
        let task_id = ctx
            .core_metadata
            .task_id
            .as_ref()
            .ok_or_else(|| DomainError::PathResolution("task-id not set".into()))?;

        let project = ctx.get_var("project").unwrap_or("inbox");

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
        let (task_id, project) = if project == "inbox" {
            (generate_inbox_task_id(&ctx.config.vault_root)?, project)
        } else {
            // Get project counter and canonical slug
            let (project_id, counter, slug) = get_project_info(ctx.config, &project)?;
            (format!("{}-{:03}", project_id, counter + 1), slug)
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

        // Log to daily note
        if let Some(ref output_path) = ctx.output_path {
            let task_id = ctx.core_metadata.task_id.as_deref().unwrap_or("");
            if let Err(e) = super::super::services::DailyLogService::log_creation(
                ctx.config,
                "task",
                &ctx.title,
                task_id,
                output_path,
            ) {
                // Log warning but don't fail the creation
                tracing::warn!("Failed to log to daily note: {}", e);
            }
        }

        // Log to project note
        if project != "inbox"
            && let Ok(project_file) = find_project_file(ctx.config, project)
        {
            let task_id = ctx.core_metadata.task_id.as_deref().unwrap_or("");
            let message = format!("Created task [[{}]]: {}", task_id, ctx.title);
            if let Err(e) = super::super::services::ProjectLogService::log_entry(
                &project_file,
                &message,
            ) {
                tracing::warn!("Failed to log to project note: {}", e);
            }
        }

        // TODO: Run Lua on_create hook if defined (requires VaultContext)

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

/// Get project info (project-id, task_counter, canonical slug) from project file.
fn get_project_info(
    config: &ResolvedConfig,
    project: &str,
) -> DomainResult<(String, u32, String)> {
    let project_file = find_project_file(config, project)?;
    let slug = extract_project_slug(&project_file, &config.vault_root);

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

    Ok((project_id, counter, slug))
}

/// Find the project file by project name/ID/title.
///
/// Searches in the following order:
/// 1. Direct path patterns (fast path)
/// 2. File named {project}.md in any Projects subfolder
/// 3. Any project file with matching project-id or title in frontmatter
fn find_project_file(config: &ResolvedConfig, project: &str) -> DomainResult<PathBuf> {
    // Try common patterns first (fast path)
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

    let projects_dir = config.vault_root.join("Projects");
    if !projects_dir.exists() {
        return Err(DomainError::Other(format!(
            "Project file not found for: {}",
            project
        )));
    }

    // Search for project file by name in any Projects subfolder
    // Handles structures like: Projects/my-project-folder/MDV.md
    if let Ok(entries) = fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                // Look for {project}.md in this folder
                let candidate = entry.path().join(format!("{}.md", project));
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }
    }

    // Search by frontmatter project-id or title
    // Handles structures where file is named differently (e.g., markdownvault-development.md with project-id: MDV)
    // Also resolves by human-readable title (e.g., "SEB Account" matches title field)
    if let Ok(entries) = fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Look for any .md file in this folder
                if let Ok(files) = fs::read_dir(&path) {
                    for file_entry in files.flatten() {
                        let file_path = file_entry.path();
                        if file_matches_project(&file_path, project) {
                            return Ok(file_path);
                        }
                    }
                }
            } else if file_matches_project(&path, project) {
                return Ok(path);
            }
        }
    }

    Err(DomainError::Other(format!("Project file not found for: {}", project)))
}

/// Check if a file matches a project by project-id (exact) or title (case-insensitive).
fn file_matches_project(path: &Path, project: &str) -> bool {
    if path.extension().map(|e| e == "md").unwrap_or(false)
        && let Ok(content) = fs::read_to_string(path)
        && let Ok(parsed) = crate::frontmatter::parse(&content)
        && let Some(fm) = parsed.frontmatter
    {
        // Check project-id (exact match)
        if let Some(pid) = fm.fields.get("project-id")
            && pid.as_str() == Some(project)
        {
            return true;
        }
        // Check title (case-insensitive)
        if let Some(title) = fm.fields.get("title")
            && title.as_str().map(|s| s.eq_ignore_ascii_case(project)).unwrap_or(false)
        {
            return true;
        }
    }
    false
}

/// Extract the canonical project directory slug from a resolved project file path.
///
/// Given `Projects/seb-account/seb-account.md`, returns `"seb-account"`.
/// Given `Projects/seb-account.md`, returns `"seb-account"`.
fn extract_project_slug(project_file: &Path, vault_root: &Path) -> String {
    let rel = project_file.strip_prefix(vault_root).unwrap_or(project_file);
    // Projects/seb-account/seb-account.md → parent dir name = "seb-account"
    // Projects/seb-account.md → file stem = "seb-account"
    if let Some(parent) = rel.parent()
        && let Some(dir_name) = parent.file_name()
    {
        let name = dir_name.to_string_lossy();
        if !name.eq_ignore_ascii_case("projects") {
            return name.to_string();
        }
    }
    project_file.file_stem().unwrap_or_default().to_string_lossy().to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_file_matches_project_by_project_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-project.md");
        fs::write(
            &path,
            "---\ntype: project\ntitle: Test Project\nproject-id: TST\ntask_counter: 0\n---\n",
        )
        .unwrap();

        assert!(file_matches_project(&path, "TST"));
        assert!(!file_matches_project(&path, "tst")); // project-id is exact match
        assert!(!file_matches_project(&path, "NOPE"));
    }

    #[test]
    fn test_file_matches_project_by_title() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-project.md");
        fs::write(
            &path,
            "---\ntype: project\ntitle: SEB Account\nproject-id: SAE\ntask_counter: 0\n---\n",
        )
        .unwrap();

        assert!(file_matches_project(&path, "SEB Account"));
        assert!(file_matches_project(&path, "seb account")); // case-insensitive
        assert!(file_matches_project(&path, "SEB ACCOUNT")); // case-insensitive
        assert!(!file_matches_project(&path, "Other Project"));
    }

    #[test]
    fn test_file_matches_project_no_match() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-project.md");
        fs::write(&path, "---\ntype: project\ntitle: My Project\nproject-id: MPR\n---\n")
            .unwrap();

        assert!(!file_matches_project(&path, "Other"));
        assert!(!file_matches_project(&path, ""));
    }

    #[test]
    fn test_file_matches_project_non_md_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("readme.txt");
        fs::write(&path, "---\ntitle: Test\nproject-id: TST\n---\n").unwrap();

        assert!(!file_matches_project(&path, "TST"));
    }

    #[test]
    fn test_extract_project_slug_subfolder() {
        let vault_root = Path::new("/vault");
        let project_file = Path::new("/vault/Projects/seb-account/seb-account.md");
        assert_eq!(extract_project_slug(project_file, vault_root), "seb-account");
    }

    #[test]
    fn test_extract_project_slug_flat() {
        let vault_root = Path::new("/vault");
        let project_file = Path::new("/vault/Projects/seb-account.md");
        assert_eq!(extract_project_slug(project_file, vault_root), "seb-account");
    }

    #[test]
    fn test_extract_project_slug_nested_deeply() {
        // Edge case: Tasks subfolder shouldn't happen for project files, but test robustness
        let vault_root = Path::new("/vault");
        let project_file = Path::new("/vault/Projects/my-proj/my-proj.md");
        assert_eq!(extract_project_slug(project_file, vault_root), "my-proj");
    }

    #[test]
    fn test_find_project_file_by_title() {
        let dir = tempfile::tempdir().unwrap();
        let vault_root = dir.path();

        // Create project structure: Projects/seb-account/seb-account.md
        let project_dir = vault_root.join("Projects/seb-account");
        fs::create_dir_all(&project_dir).unwrap();
        let project_file = project_dir.join("seb-account.md");
        fs::write(
            &project_file,
            "---\ntype: project\ntitle: SEB Account\nproject-id: SAE\ntask_counter: 3\n---\n",
        )
        .unwrap();

        let config = ResolvedConfig {
            vault_root: vault_root.to_path_buf(),
            ..make_test_config(vault_root)
        };

        // Should resolve by slug (fast path)
        let result = find_project_file(&config, "seb-account");
        assert!(result.is_ok(), "Should resolve by slug");

        // Should resolve by title
        let result = find_project_file(&config, "SEB Account");
        assert!(result.is_ok(), "Should resolve by title");
        assert_eq!(result.unwrap(), project_file);

        // Should resolve by title case-insensitively
        let result = find_project_file(&config, "seb account");
        assert!(result.is_ok(), "Should resolve by title case-insensitively");

        // Should resolve by project-id
        let result = find_project_file(&config, "SAE");
        assert!(result.is_ok(), "Should resolve by project-id");

        // Should fail for unknown
        let result = find_project_file(&config, "Unknown Project");
        assert!(result.is_err(), "Should fail for unknown project");
    }

    fn make_test_config(vault_root: &Path) -> ResolvedConfig {
        ResolvedConfig {
            active_profile: "test".into(),
            vault_root: vault_root.to_path_buf(),
            templates_dir: vault_root.join(".mdvault/templates"),
            captures_dir: vault_root.join(".mdvault/captures"),
            macros_dir: vault_root.join(".mdvault/macros"),
            typedefs_dir: vault_root.join(".mdvault/typedefs"),
            excluded_folders: vec![],
            security: Default::default(),
            logging: Default::default(),
            activity: Default::default(),
        }
    }
}
