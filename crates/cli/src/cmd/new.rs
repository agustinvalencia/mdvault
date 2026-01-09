use crate::prompt::{prompt_for_enum, prompt_for_field, CollectedVars, PromptOptions};
use crate::NewArgs;
use dialoguer::{theme::ColorfulTheme, Editor, Input, Select};
use mdvault_core::captures::CaptureRepository;
use mdvault_core::config::loader::{default_config_path, ConfigLoader};
use mdvault_core::config::types::ResolvedConfig;
use mdvault_core::frontmatter::parse as parse_frontmatter;
use mdvault_core::ids::{generate_project_id, generate_task_id};
use mdvault_core::index::{IndexBuilder, IndexDb, NoteQuery, NoteType};
use mdvault_core::macros::MacroRepository;
use mdvault_core::scripting::{
    run_on_create_hook, HookResult, NoteContext, VaultContext,
};
use mdvault_core::templates::discovery::TemplateInfo;
use mdvault_core::templates::engine::{
    build_minimal_context, render, render_string, resolve_template_output_path,
};
use mdvault_core::templates::repository::{TemplateRepoError, TemplateRepository};
use mdvault_core::types::try_fix_note;
use mdvault_core::types::{
    discovery::load_typedef_from_file, generate_scaffolding, get_missing_required_fields,
    validate_note, FieldType, TypeDefinition, TypeRegistry, TypedefRepository,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Core metadata that must be preserved in notes regardless of template/hook modifications.
/// These fields are managed by mdvault and should not be removed or overwritten by user code.
#[derive(Debug, Clone, Default)]
struct CoreMetadata {
    /// Note type (project, task, etc.)
    note_type: Option<String>,
    /// Title of the note
    title: Option<String>,
    /// Project ID (for projects)
    project_id: Option<String>,
    /// Task ID (for tasks)
    task_id: Option<String>,
    /// Task counter (for projects)
    task_counter: Option<u32>,
    /// Parent project (for tasks)
    project: Option<String>,
}

pub fn run(config: Option<&Path>, profile: Option<&str>, args: NewArgs) {
    debug!("Running create new");
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            println!("FAIL mdv new");
            println!("{e}");
            if config.is_none() {
                println!("looked for: {}", default_config_path().display());
            }
            std::process::exit(1);
        }
    };

    // Decide between template mode and type-based scaffolding
    if let Some(ref template_name) = args.template {
        // Template mode (existing behavior)
        run_template_mode(&cfg, template_name, &args);
    } else if let Some(ref type_name) = args.note_type {
        // Type-based scaffolding mode
        run_scaffolding_mode(&cfg, type_name, &args);
    } else {
        eprintln!("Error: either provide a type name or use --template");
        eprintln!("Usage: mdv new <type> [title] [--var field=value]");
        eprintln!("       mdv new --template <name> [--var key=value]");
        std::process::exit(1);
    }
}

/// Run template-based note creation (existing behavior).
fn run_template_mode(cfg: &ResolvedConfig, template_name: &str, args: &NewArgs) {
    let repo = match TemplateRepository::new(&cfg.templates_dir) {
        Ok(r) => r,
        Err(e) => {
            println!("FAIL mdv new");
            println!("{e}");
            std::process::exit(1);
        }
    };

    let loaded = match repo.get_by_name(template_name) {
        Ok(t) => t,
        Err(e) => match e {
            TemplateRepoError::NotFound(name) => {
                eprintln!("Template not found: {name}");
                std::process::exit(1);
            }
            other => {
                eprintln!("Failed to load template: {other}");
                std::process::exit(1);
            }
        },
    };

    // Build TemplateInfo for context building
    let info = TemplateInfo {
        logical_name: loaded.logical_name.clone(),
        path: loaded.path.clone(),
    };

    // Check if template links to a Lua script
    let lua_typedef: Option<TypeDefinition> =
        loaded.frontmatter.as_ref().and_then(|fm| fm.lua.as_ref()).and_then(|lua_path| {
            // Resolve lua path relative to typedefs directory
            let lua_file = cfg.typedefs_dir.join(lua_path);
            match load_typedef_from_file(&lua_file) {
                Ok(td) => Some(td),
                Err(e) => {
                    eprintln!("Warning: failed to load Lua script '{}': {}", lua_path, e);
                    None
                }
            }
        });

    // Convert provided vars to HashMap
    let mut provided_vars: HashMap<String, String> = args.vars.iter().cloned().collect();

    // Handle title: In template mode, the first positional arg (note_type) is actually the title
    // since --template replaces the type name. Also check args.title for completeness.
    let title = args.title.clone().or_else(|| args.note_type.clone());
    if let Some(ref t) = title {
        provided_vars.entry("title".to_string()).or_insert(t.clone());
    }

    // For task templates: show project picker if project not already provided
    if template_name == "task" && !provided_vars.contains_key("project") && !args.batch {
        if let Some(project) = prompt_project_selection(cfg) {
            provided_vars.insert("project".to_string(), project);
        }
    }

    // Build minimal context for variable resolution
    let minimal_ctx = build_minimal_context(cfg, &info);

    // Collect variables using Lua schema prompts
    let prompt_options = PromptOptions { batch_mode: args.batch };

    let collected = if let Some(ref typedef) = lua_typedef {
        // Use Lua schema for prompting - fields with `prompt` set will be prompted
        match collect_schema_variables(typedef, &provided_vars, &prompt_options) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        // No Lua script - just use provided vars directly
        CollectedVars {
            values: provided_vars.clone(),
            prompted: Vec::new(),
            defaulted: Vec::new(),
        }
    };

    // Merge collected variables into context
    debug!("Collected variables: {:?}", collected.values);
    let mut ctx = minimal_ctx;
    for (k, v) in collected.values {
        ctx.insert(k, v);
    }

    // Resolve output path: CLI arg > template frontmatter > Lua typedef output
    let output_path = if let Some(ref out) = args.output {
        out.clone()
    } else {
        // Try to get from template frontmatter first
        match resolve_template_output_path(&loaded, cfg, &ctx) {
            Ok(Some(path)) => path,
            Ok(None) => {
                // Fall back to Lua typedef output if available
                if let Some(ref typedef) = lua_typedef {
                    if let Some(ref output_template) = typedef.output {
                        // Render the output template with current context
                        match render_output_path(output_template, cfg, &ctx) {
                            Ok(path) => path,
                            Err(e) => {
                                eprintln!("Failed to resolve Lua output path: {e}");
                                std::process::exit(1);
                            }
                        }
                    } else {
                        eprintln!(
                            "Error: --output is required (neither template nor Lua script has output)"
                        );
                        std::process::exit(1);
                    }
                } else {
                    eprintln!(
                        "Error: --output is required (template has no output in frontmatter)"
                    );
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("Failed to resolve output path: {e}");
                std::process::exit(1);
            }
        }
    };

    // Update context with output info
    let output_abs = if output_path.is_absolute() {
        output_path.clone()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(&output_path)
    };
    ctx.insert("output_path".to_string(), output_abs.to_string_lossy().to_string());
    if let Some(name) = output_abs.file_name().and_then(|s| s.to_str()) {
        ctx.insert("output_filename".to_string(), name.to_string());
    }
    if let Some(parent) = output_abs.parent() {
        ctx.insert("output_dir".to_string(), parent.to_string_lossy().to_string());
    }

    if output_path.exists() {
        eprintln!(
            "Refusing to overwrite existing file: {} (add --force later if needed)",
            output_path.display()
        );
        std::process::exit(1);
    }

    debug!("Render context: {:?}", ctx);
    let mut rendered = match render(&loaded, &ctx) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to render template: {e}");
            std::process::exit(1);
        }
    };

    // Phase 3: Validate content before writing
    // Load type registry for validation (only if we have a Lua typedef)
    if let Some(ref typedef) = lua_typedef {
        // Try to load type registry, skip validation if it fails
        if let Ok(typedef_repo) = TypedefRepository::new(&cfg.typedefs_dir) {
            if let Ok(type_registry) = TypeRegistry::from_repository(&typedef_repo) {
                // Extract note type from rendered content for validation
                let note_type =
                    extract_note_type(&rendered).unwrap_or_else(|| typedef.name.clone());

                match validate_before_write(
                    &type_registry,
                    &note_type,
                    &output_path,
                    &rendered,
                ) {
                    Ok(Some(fixed)) => rendered = fixed,
                    Ok(None) => {}
                    Err(errors) => {
                        eprintln!("Validation failed:");
                        for err in &errors {
                            eprintln!("  - {}", err);
                        }
                        std::process::exit(1);
                    }
                }
            }
        }
    }

    if let Some(parent) = output_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("Failed to create parent directory {}: {e}", parent.display());
            std::process::exit(1);
        }
    }

    if let Err(e) = fs::write(&output_path, &rendered) {
        eprintln!("Failed to write output file {}: {e}", output_path.display());
        std::process::exit(1);
    }

    // Execute on_create hook if type definition exists

    match run_on_create_hook_if_exists(
        cfg,
        &output_path,
        &rendered,
        lua_typedef.as_ref(),
        &ctx,
    ) {
        Ok(hook_result) => {
            if hook_result.modified {
                // Check if variables were updated by the hook

                let final_content = if let Some(ref new_vars) = hook_result.variables {
                    // Update context with new variables

                    if let serde_yaml::Value::Mapping(map) = new_vars {
                        for (k, v) in map {
                            if let serde_yaml::Value::String(ks) = k {
                                // Convert value to string for RenderContext

                                let vs = match v {
                                    serde_yaml::Value::String(s) => s.clone(),

                                    serde_yaml::Value::Number(n) => n.to_string(),

                                    serde_yaml::Value::Bool(b) => b.to_string(),

                                    _ => format!("{:?}", v),
                                };

                                ctx.insert(ks.clone(), vs);
                            }
                        }
                    }

                    // Re-render template with new context

                    match render(&loaded, &ctx) {
                        Ok(s) => s,

                        Err(e) => {
                            eprintln!("Warning: failed to re-render template: {e}");

                            rendered.clone()
                        }
                    }
                } else {
                    rendered.clone()
                };

                // Apply other modifications (frontmatter/content)

                if let Err(e) =
                    apply_hook_modifications(&output_path, &final_content, &hook_result)
                {
                    eprintln!(
                        "Warning: failed to apply on_create hook modifications: {e}"
                    );
                }
            }
        }

        Err(e) => {
            eprintln!("Warning: on_create hook failed: {e}");
        }
    }

    // Log to daily note for tasks and projects
    // Note: In template mode, we don't auto-generate IDs (user should use scaffolding mode for that)
    // But we still log to daily with whatever ID might be in the context
    if template_name == "task" || template_name == "project" {
        let title = ctx.get("title").cloned().unwrap_or_else(|| "Untitled".to_string());
        let note_id = ctx
            .get("task-id")
            .or_else(|| ctx.get("project-id"))
            .cloned()
            .unwrap_or_default();
        log_to_daily(cfg, template_name, &title, &note_id, &output_path);

        // Force reindex so the new note appears in queries
        reindex_vault(cfg);
    }

    println!("OK   mdv new");
    println!("template: {}", template_name);
    println!("output:   {}", output_path.display());
}

/// Run type-based scaffolding mode.
fn run_scaffolding_mode(cfg: &ResolvedConfig, type_name: &str, args: &NewArgs) {
    // Load type registry
    let typedef_repo = match TypedefRepository::new(&cfg.typedefs_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to load type definitions: {e}");
            std::process::exit(1);
        }
    };

    let type_registry = match TypeRegistry::from_repository(&typedef_repo) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to build type registry: {e}");
            std::process::exit(1);
        }
    };

    // Check if type is known
    if !type_registry.is_known_type(type_name) {
        eprintln!("Unknown type: {type_name}");
        eprintln!("Available types:");
        for t in type_registry.list_all_types() {
            eprintln!("  {t}");
        }
        std::process::exit(1);
    }

    // Get type definition (may be None for built-in types without Lua override)
    let typedef = type_registry.get(type_name);

    // Check if there's a matching template
    let template_repo = TemplateRepository::new(&cfg.templates_dir).ok();
    let loaded_template =
        template_repo.as_ref().and_then(|repo| repo.get_by_name(type_name).ok());

    // Note: We no longer delegate to template mode for non-project/task types.
    // Scaffolding mode handles template rendering AND validation with the type registry,
    // which is required for autofix to work correctly.

    // Get title (required for scaffolding)
    let title = match &args.title {
        Some(t) => t.clone(),
        None => {
            if args.batch {
                eprintln!("Error: title is required in batch mode");
                eprintln!("Usage: mdv new {type_name} \"Title\"");
                std::process::exit(1);
            }
            // Prompt for title
            match prompt_for_field("title", "Note title", None, true) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }
    };

    // Collect vars from command line
    let mut vars: HashMap<String, String> = args.vars.iter().cloned().collect();

    // Handle project creation with ID generation
    let (output_path, note_id) = if type_name == "project" {
        // Compute default project ID from title
        let computed_id = generate_project_id(&title);

        // Prompt for project ID with computed value as default (unless batch mode)
        let project_id = if args.batch {
            computed_id
        } else {
            match prompt_for_field(
                "project-id",
                "Project ID (3-letter code)",
                Some(&computed_id),
                true,
            ) {
                Ok(id) if !id.is_empty() => id.to_uppercase(),
                Ok(_) => computed_id, // Empty input uses computed default
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        };

        vars.insert("project-id".to_string(), project_id.clone());
        vars.insert("task_counter".to_string(), "0".to_string());
        vars.insert("title".to_string(), title.clone());

        let path = if let Some(ref out) = args.output {
            out.clone()
        } else if let Some(ref td) = typedef {
            // Use Lua typedef's output template if available
            if let Some(ref output_template) = td.output {
                match render_output_path(output_template, cfg, &vars) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Warning: failed to render Lua output path: {e}");
                        // Fall back to default
                        cfg.vault_root
                            .join(format!("Projects/{}/{}.md", project_id, project_id))
                    }
                }
            } else {
                // No output template in Lua, use default
                cfg.vault_root.join(format!("Projects/{}/{}.md", project_id, project_id))
            }
        } else {
            // No typedef, use default
            cfg.vault_root.join(format!("Projects/{}/{}.md", project_id, project_id))
        };
        (path, project_id)
    } else if type_name == "task" {
        // For tasks: prompt for project selection if not already provided
        let project_folder = if let Some(proj) = vars.get("project").cloned() {
            proj
        } else if !args.batch {
            match prompt_project_selection(cfg) {
                Some(proj) => {
                    vars.insert("project".to_string(), proj.clone());
                    proj
                }
                None => "inbox".to_string(),
            }
        } else {
            "inbox".to_string()
        };

        // Add title to vars for output path rendering
        vars.insert("title".to_string(), title.clone());

        // Get project info and generate task ID
        let (task_id, output_path) = if project_folder == "inbox" {
            // Inbox tasks get a simple incremental ID
            let task_id = generate_inbox_task_id(cfg);
            vars.insert("task-id".to_string(), task_id.clone());
            let path = if let Some(ref out) = args.output {
                out.clone()
            } else if let Some(ref td) = typedef {
                // Use Lua typedef's output template if available
                if let Some(ref output_template) = td.output {
                    match render_output_path(output_template, cfg, &vars) {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("Warning: failed to render Lua output path: {e}");
                            cfg.vault_root.join(format!("Inbox/{}.md", task_id))
                        }
                    }
                } else {
                    cfg.vault_root.join(format!("Inbox/{}.md", task_id))
                }
            } else {
                cfg.vault_root.join(format!("Inbox/{}.md", task_id))
            };
            (task_id, path)
        } else {
            // Get project's task counter and increment it
            match get_and_increment_project_counter(cfg, &project_folder) {
                Ok((project_id, counter)) => {
                    let task_id = generate_task_id(&project_id, counter);
                    vars.insert("task-id".to_string(), task_id.clone());
                    vars.insert("project-id".to_string(), project_id.clone());
                    let path = if let Some(ref out) = args.output {
                        out.clone()
                    } else if let Some(ref td) = typedef {
                        // Use Lua typedef's output template if available
                        if let Some(ref output_template) = td.output {
                            match render_output_path(output_template, cfg, &vars) {
                                Ok(p) => p,
                                Err(e) => {
                                    eprintln!(
                                        "Warning: failed to render Lua output path: {e}"
                                    );
                                    cfg.vault_root.join(format!(
                                        "Projects/{}/Tasks/{}.md",
                                        project_folder, task_id
                                    ))
                                }
                            }
                        } else {
                            cfg.vault_root.join(format!(
                                "Projects/{}/Tasks/{}.md",
                                project_folder, task_id
                            ))
                        }
                    } else {
                        cfg.vault_root.join(format!(
                            "Projects/{}/Tasks/{}.md",
                            project_folder, task_id
                        ))
                    };
                    (task_id, path)
                }
                Err(e) => {
                    eprintln!("Warning: could not get project info: {e}");
                    // Fall back to inbox-style ID
                    let task_id = generate_inbox_task_id(cfg);
                    vars.insert("task-id".to_string(), task_id.clone());
                    let path = if let Some(ref td) = typedef {
                        if let Some(ref output_template) = td.output {
                            match render_output_path(output_template, cfg, &vars) {
                                Ok(p) => p,
                                Err(_) => cfg.vault_root.join(format!(
                                    "Projects/{}/Tasks/{}.md",
                                    project_folder, task_id
                                )),
                            }
                        } else {
                            cfg.vault_root.join(format!(
                                "Projects/{}/Tasks/{}.md",
                                project_folder, task_id
                            ))
                        }
                    } else {
                        cfg.vault_root.join(format!(
                            "Projects/{}/Tasks/{}.md",
                            project_folder, task_id
                        ))
                    };
                    (task_id, path)
                }
            }
        };
        (output_path, task_id)
    } else {
        // Other types: check CLI arg, then typedef output template, then default
        vars.insert("title".to_string(), title.clone());
        let path = if let Some(ref out) = args.output {
            out.clone()
        } else if let Some(ref td) = typedef {
            // Use Lua typedef's output template if available
            if let Some(ref output_template) = td.output {
                match render_output_path(output_template, cfg, &vars) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Warning: failed to render Lua output path: {e}");
                        cfg.vault_root.join(format!(
                            "{}s/{}.md",
                            type_name,
                            slugify(&title)
                        ))
                    }
                }
            } else {
                cfg.vault_root.join(format!("{}s/{}.md", type_name, slugify(&title)))
            }
        } else {
            cfg.vault_root.join(format!("{}s/{}.md", type_name, slugify(&title)))
        };
        (path, String::new())
    };

    // Build core metadata for projects and tasks
    // This will be used to ensure these fields survive template/hook modifications
    let core_metadata = if type_name == "project" {
        CoreMetadata {
            note_type: Some("project".to_string()),
            title: Some(title.clone()),
            project_id: vars.get("project-id").cloned(),
            task_counter: Some(0),
            ..Default::default()
        }
    } else if type_name == "task" {
        CoreMetadata {
            note_type: Some("task".to_string()),
            title: Some(title.clone()),
            task_id: vars.get("task-id").cloned(),
            project: vars.get("project").cloned(),
            ..Default::default()
        }
    } else {
        CoreMetadata::default()
    };

    // Prompt for missing required fields
    if let Some(ref td) = typedef {
        let missing = get_missing_required_fields(td, &vars);

        if !missing.is_empty() {
            if args.batch {
                eprintln!("Error: missing required fields:");
                for (field, schema) in &missing {
                    let type_hint = schema
                        .field_type
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "string".to_string());
                    eprintln!("  {} ({})", field, type_hint);
                }
                std::process::exit(1);
            }

            // Prompt for each missing field
            for (field, schema) in missing {
                let type_hint = schema
                    .field_type
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "string".to_string());

                let prompt = if let Some(ref desc) = schema.description {
                    format!("{} ({})", desc, type_hint)
                } else {
                    format!("{} ({})", field, type_hint)
                };

                // Use picker for enum fields, text input for others
                let result = if let Some(ref enum_values) = schema.enum_values {
                    if !enum_values.is_empty() {
                        // Use picker widget for enum selection
                        prompt_for_enum(
                            field,
                            &prompt,
                            enum_values,
                            schema.default.as_ref().and_then(|v| v.as_str()),
                        )
                    } else {
                        prompt_for_field(field, &prompt, None, true)
                    }
                } else {
                    prompt_for_field(field, &prompt, None, true)
                };

                match result {
                    Ok(value) => {
                        vars.insert(field.clone(), value);
                    }
                    Err(e) => {
                        eprintln!("Error: {e}");
                        std::process::exit(1);
                    }
                }
            }
        }

        // Prompt for template variables defined in the typedef
        for (var_name, var_spec) in &td.variables {
            // Skip if already provided via CLI
            if vars.contains_key(var_name) {
                continue;
            }

            // Get the prompt text (if any)
            let prompt_text = var_spec.prompt();
            let default_value = var_spec.default();

            // Only prompt if there's a prompt defined or variable is required
            if !prompt_text.is_empty() || var_spec.is_required() {
                if args.batch {
                    // In batch mode, use default if available
                    if let Some(default) = default_value {
                        vars.insert(var_name.clone(), default.to_string());
                    } else if var_spec.is_required() {
                        eprintln!("Error: missing required variable: {}", var_name);
                        std::process::exit(1);
                    }
                } else {
                    // Interactive mode: prompt the user
                    let prompt_label = if prompt_text.is_empty() {
                        var_name.to_string()
                    } else {
                        prompt_text.to_string()
                    };

                    match prompt_for_field(
                        var_name,
                        &prompt_label,
                        default_value,
                        var_spec.is_required(),
                    ) {
                        Ok(value) => {
                            // Use provided value or fall back to default
                            if value.is_empty() {
                                if let Some(default) = default_value {
                                    vars.insert(var_name.clone(), default.to_string());
                                }
                            } else {
                                vars.insert(var_name.clone(), value);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {e}");
                            std::process::exit(1);
                        }
                    }
                }
            } else if let Some(default) = default_value {
                // Variable has a default but no prompt - just use the default
                vars.insert(var_name.clone(), default.to_string());
            }
        }
    }

    if output_path.exists() {
        eprintln!("Refusing to overwrite existing file: {}", output_path.display());
        std::process::exit(1);
    }

    debug!("Scaffolding vars: {:?}", vars);

    // Generate content - use template if available, otherwise scaffolding
    // For projects/tasks, we'll ensure core metadata is preserved either way
    let content = if let Some(ref loaded) = loaded_template {
        // Build context for template rendering
        let info = TemplateInfo {
            logical_name: loaded.logical_name.clone(),
            path: loaded.path.clone(),
        };
        let mut ctx = build_minimal_context(cfg, &info);

        // Add all vars to context
        ctx.insert("title".to_string(), title.clone());
        for (k, v) in &vars {
            ctx.insert(k.clone(), v.clone());
        }

        // Update context with output info
        ctx.insert("output_path".to_string(), output_path.to_string_lossy().to_string());
        if let Some(name) = output_path.file_name().and_then(|s| s.to_str()) {
            ctx.insert("output_filename".to_string(), name.to_string());
        }

        // Render template
        match render(loaded, &ctx) {
            Ok(rendered) => {
                inject_vars_into_frontmatter(&rendered, &vars, typedef.as_deref())
                    .unwrap_or(rendered)
            }
            Err(e) => {
                eprintln!("Failed to render template: {e}");
                eprintln!("Falling back to scaffolding...");
                generate_scaffolding(type_name, typedef.as_deref(), &title, &vars)
            }
        }
    } else {
        generate_scaffolding(type_name, typedef.as_deref(), &title, &vars)
    };

    // Apply core metadata immediately after content generation (before writing)
    // This ensures template output has the required fields
    let mut content = if type_name == "project" || type_name == "task" {
        match ensure_core_metadata(&content, &core_metadata) {
            Ok(fixed) => fixed,
            Err(e) => {
                eprintln!("Warning: failed to apply core metadata: {e}");
                content
            }
        }
    } else {
        content
    };

    // Phase 3: Validate content before writing
    match validate_before_write(&type_registry, type_name, &output_path, &content) {
        Ok(Some(fixed)) => content = fixed,
        Ok(None) => {}
        Err(errors) => {
            eprintln!("Validation failed:");
            for err in &errors {
                eprintln!("  - {}", err);
            }
            std::process::exit(1);
        }
    }

    // Create parent directories
    if let Some(parent) = output_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("Failed to create parent directory {}: {e}", parent.display());
            std::process::exit(1);
        }
    }

    // Write file
    if let Err(e) = fs::write(&output_path, &content) {
        eprintln!("Failed to write output file {}: {e}", output_path.display());
        std::process::exit(1);
    }

    // Execute on_create hook if defined
    match run_on_create_hook_if_exists(
        cfg,
        &output_path,
        &content,
        typedef.as_deref(),
        &vars,
    ) {
        Ok(mut hook_result) => {
            if hook_result.modified {
                let mut final_content = content.clone();

                if let Some(ref new_vars) = hook_result.variables {
                    // Sync updated variables with frontmatter if both exist
                    if let (
                        Some(serde_yaml::Value::Mapping(ref mut fm)),
                        serde_yaml::Value::Mapping(ref vars_map),
                    ) = (&mut hook_result.frontmatter, new_vars)
                    {
                        for (k, v) in vars_map {
                            if k.as_str() != Some("type") && k.as_str() != Some("title") {
                                fm.insert(k.clone(), v.clone());
                            }
                        }
                    }

                    // Update vars
                    if let serde_yaml::Value::Mapping(map) = new_vars {
                        for (k, v) in map {
                            if let serde_yaml::Value::String(ks) = k {
                                let vs = match v {
                                    serde_yaml::Value::String(s) => s.clone(),
                                    serde_yaml::Value::Number(n) => n.to_string(),
                                    serde_yaml::Value::Bool(b) => b.to_string(),
                                    _ => format!("{:?}", v),
                                };
                                vars.insert(ks.clone(), vs);
                            }
                        }
                    }
                    // Re-render template or scaffolding with updated vars
                    let regenerated = if let Some(ref loaded) = loaded_template {
                        // Re-render template with updated context
                        let info = TemplateInfo {
                            logical_name: loaded.logical_name.clone(),
                            path: loaded.path.clone(),
                        };
                        let mut ctx = build_minimal_context(cfg, &info);
                        ctx.insert("title".to_string(), title.clone());
                        for (k, v) in &vars {
                            ctx.insert(k.clone(), v.clone());
                        }
                        ctx.insert(
                            "output_path".to_string(),
                            output_path.to_string_lossy().to_string(),
                        );
                        if let Some(name) =
                            output_path.file_name().and_then(|s| s.to_str())
                        {
                            ctx.insert("output_filename".to_string(), name.to_string());
                        }
                        match render(loaded, &ctx) {
                            Ok(rendered) => inject_vars_into_frontmatter(
                                &rendered,
                                &vars,
                                typedef.as_deref(),
                            )
                            .unwrap_or(rendered),
                            Err(_) => generate_scaffolding(
                                type_name,
                                typedef.as_deref(),
                                &title,
                                &vars,
                            ),
                        }
                    } else {
                        generate_scaffolding(type_name, typedef.as_deref(), &title, &vars)
                    };
                    // Re-apply core metadata
                    final_content = if type_name == "project" || type_name == "task" {
                        match ensure_core_metadata(&regenerated, &core_metadata) {
                            Ok(c) => c,
                            Err(_) => regenerated,
                        }
                    } else {
                        regenerated
                    };
                }

                if let Err(e) =
                    apply_hook_modifications(&output_path, &final_content, &hook_result)
                {
                    eprintln!(
                        "Warning: failed to apply on_create hook modifications: {e}"
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: on_create hook failed: {e}");
        }
    }

    // Ensure core metadata is preserved after template/hook modifications
    // This guarantees that projects have project-id and tasks have task-id
    if type_name == "project" || type_name == "task" {
        match fs::read_to_string(&output_path) {
            Ok(current_content) => {
                match ensure_core_metadata(&current_content, &core_metadata) {
                    Ok(fixed_content) => {
                        if let Err(e) = fs::write(&output_path, fixed_content) {
                            eprintln!("Warning: failed to write core metadata: {e}");
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: failed to ensure core metadata: {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: failed to read file for metadata check: {e}");
            }
        }
    }

    // Log to daily note for tasks and projects
    if type_name == "task" || type_name == "project" {
        log_to_daily(cfg, type_name, &title, &note_id, &output_path);
    }

    // Force reindex so the new note appears in queries
    reindex_vault(cfg);

    println!("OK   mdv new");
    println!("type:   {}", type_name);
    if !note_id.is_empty() {
        println!("id:     {}", note_id);
    }
    println!("output: {}", output_path.display());
}

/// Slugify a string for use in paths.
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

/// Ensure core metadata fields are present in the note content.
///
/// This function is called after template rendering and hook execution to guarantee
/// that required fields managed by mdvault are not removed or corrupted by user code.
/// Templates and hooks can ADD fields but cannot REMOVE core fields.
fn ensure_core_metadata(content: &str, core: &CoreMetadata) -> Result<String, String> {
    let parsed = parse_frontmatter(content).map_err(|e| e.to_string())?;

    // Start with existing frontmatter or create new
    let mut fields: HashMap<String, serde_yaml::Value> =
        if let Some(fm) = parsed.frontmatter { fm.fields } else { HashMap::new() };

    // Inject/overwrite core fields - these are authoritative from Rust
    if let Some(ref t) = core.note_type {
        fields.insert("type".to_string(), serde_yaml::Value::String(t.clone()));
    }

    if let Some(ref t) = core.title {
        fields.insert("title".to_string(), serde_yaml::Value::String(t.clone()));
    }

    if let Some(ref id) = core.project_id {
        fields.insert("project-id".to_string(), serde_yaml::Value::String(id.clone()));
    }

    if let Some(ref id) = core.task_id {
        fields.insert("task-id".to_string(), serde_yaml::Value::String(id.clone()));
    }

    if let Some(counter) = core.task_counter {
        fields.insert(
            "task_counter".to_string(),
            serde_yaml::Value::Number(serde_yaml::Number::from(counter)),
        );
    }

    if let Some(ref proj) = core.project {
        fields.insert("project".to_string(), serde_yaml::Value::String(proj.clone()));
    }

    // Rebuild the document
    let mut mapping = serde_yaml::Mapping::new();
    for (k, v) in fields {
        mapping.insert(serde_yaml::Value::String(k), v);
    }

    let yaml_str = serde_yaml::to_string(&serde_yaml::Value::Mapping(mapping))
        .map_err(|e| e.to_string())?;

    Ok(format!("---\n{}---\n{}", yaml_str, parsed.body))
}

/// Generate a task ID for inbox tasks (no project).
fn generate_inbox_task_id(cfg: &ResolvedConfig) -> String {
    let inbox_path = cfg.vault_root.join("Inbox");
    let mut max_counter = 0u32;

    if inbox_path.exists() {
        if let Ok(entries) = fs::read_dir(&inbox_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                // Parse INB-XXX pattern
                if name_str.starts_with("INB-") {
                    if let Some(num_str) =
                        name_str.strip_prefix("INB-").and_then(|s| s.strip_suffix(".md"))
                    {
                        if let Ok(n) = num_str.parse::<u32>() {
                            max_counter = max_counter.max(n);
                        }
                    }
                }
            }
        }
    }

    generate_task_id("INB", max_counter + 1)
}

/// Get project's ID and increment its task counter.
/// Returns (project_id, new_counter) on success.
fn get_and_increment_project_counter(
    cfg: &ResolvedConfig,
    project_folder: &str,
) -> Result<(String, u32), String> {
    // Find the project file - try both <folder>/<folder>.md and <folder>.md patterns
    let project_path = find_project_file(cfg, project_folder)?;

    // Read and parse the project file
    let content = fs::read_to_string(&project_path)
        .map_err(|e| format!("Failed to read project file: {e}"))?;

    let parsed = parse_frontmatter(&content)
        .map_err(|e| format!("Failed to parse project frontmatter: {e}"))?;

    let fm = parsed.frontmatter.ok_or("Project has no frontmatter")?;

    // Get project-id
    let project_id = fm
        .fields
        .get("project-id")
        .and_then(|v| match v {
            serde_yaml::Value::String(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_else(|| generate_project_id(project_folder));

    // Get current task counter
    let current_counter = fm
        .fields
        .get("task_counter")
        .and_then(|v| match v {
            serde_yaml::Value::Number(n) => n.as_u64().map(|n| n as u32),
            serde_yaml::Value::String(s) => s.parse::<u32>().ok(),
            _ => None,
        })
        .unwrap_or(0);

    let new_counter = current_counter + 1;

    // Update the project file with new counter
    let mut new_fm = fm.fields.clone();
    new_fm.insert(
        "task_counter".to_string(),
        serde_yaml::Value::Number(serde_yaml::Number::from(new_counter)),
    );

    // Rebuild the document
    let mut mapping = serde_yaml::Mapping::new();
    for (k, v) in new_fm {
        mapping.insert(serde_yaml::Value::String(k), v);
    }
    let yaml_str = serde_yaml::to_string(&serde_yaml::Value::Mapping(mapping))
        .map_err(|e| format!("Failed to serialize frontmatter: {e}"))?;

    let new_content = format!("---\n{}---\n{}", yaml_str, parsed.body);

    fs::write(&project_path, new_content)
        .map_err(|e| format!("Failed to update project file: {e}"))?;

    Ok((project_id, new_counter))
}

/// Find the project file for a given project folder name.
fn find_project_file(
    cfg: &ResolvedConfig,
    project_folder: &str,
) -> Result<PathBuf, String> {
    // Try Projects/<folder>/<folder>.md
    let path1 =
        cfg.vault_root.join(format!("Projects/{}/{}.md", project_folder, project_folder));
    if path1.exists() {
        return Ok(path1);
    }

    // Try Projects/<folder>.md
    let path2 = cfg.vault_root.join(format!("Projects/{}.md", project_folder));
    if path2.exists() {
        return Ok(path2);
    }

    // Try scanning the Projects/<folder>/ directory for any .md file
    let folder_path = cfg.vault_root.join(format!("Projects/{}", project_folder));
    if folder_path.is_dir() {
        if let Ok(entries) = fs::read_dir(&folder_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    // Check if it's a project file (not in Tasks subdirectory)
                    if !path.to_string_lossy().contains("/Tasks/") {
                        return Ok(path);
                    }
                }
            }
        }
    }

    Err(format!("Project file not found for: {}", project_folder))
}

/// Force a vault reindex to include newly created notes.
fn reindex_vault(cfg: &ResolvedConfig) {
    let index_path = cfg.vault_root.join(".mdvault/index.db");

    // Ensure index directory exists
    if let Some(parent) = index_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Open the database and run incremental reindex
    match IndexDb::open(&index_path) {
        Ok(db) => {
            let builder = IndexBuilder::new(&db, &cfg.vault_root);
            if let Err(e) = builder.incremental_reindex(None) {
                eprintln!("Warning: reindex failed: {e}");
            }
        }
        Err(e) => {
            eprintln!("Warning: could not open index for reindex: {e}");
        }
    }
}

/// Extract note type from rendered content's frontmatter.
fn extract_note_type(content: &str) -> Option<String> {
    let parsed = parse_frontmatter(content).ok()?;
    let fm = parsed.frontmatter?;

    if let Some(serde_yaml::Value::String(t)) = fm.fields.get("type") {
        return Some(t.clone());
    }
    None
}

/// Run on_create hook if the note type has one defined.
/// Returns the HookResult which may contain modifications to apply.
fn run_on_create_hook_if_exists(
    cfg: &ResolvedConfig,
    output_path: &Path,
    content: &str,
    explicit_typedef: Option<&TypeDefinition>,
    variables: &HashMap<String, String>,
) -> Result<HookResult, String> {
    // Load type registry first, as we need it for VaultContext anyway
    let typedef_repo =
        TypedefRepository::new(&cfg.typedefs_dir).map_err(|e| e.to_string())?;
    let type_registry =
        TypeRegistry::from_repository(&typedef_repo).map_err(|e| e.to_string())?;

    // Determine which typedef to use
    let typedef = if let Some(td) = explicit_typedef {
        if !td.has_on_create_hook {
            return Ok(HookResult {
                modified: false,
                frontmatter: None,
                content: None,
                variables: None,
            });
        }
        td.clone()
    } else {
        // Extract note type from frontmatter
        let note_type = match extract_note_type(content) {
            Some(t) => t,
            None => {
                return Ok(HookResult {
                    modified: false,
                    frontmatter: None,
                    content: None,
                    variables: None,
                })
            }
        };

        // Check if type has on_create hook
        match type_registry.get(&note_type) {
            Some(td) if td.has_on_create_hook => (*td).clone(),
            _ => {
                return Ok(HookResult {
                    modified: false,
                    frontmatter: None,
                    content: None,
                    variables: None,
                })
            }
        }
    };

    // Load all repositories for VaultContext
    let template_repo =
        TemplateRepository::new(&cfg.templates_dir).map_err(|e| e.to_string())?;
    let capture_repo =
        CaptureRepository::new(&cfg.captures_dir).map_err(|e| e.to_string())?;
    let macro_repo = MacroRepository::new(&cfg.macros_dir).map_err(|e| e.to_string())?;

    // Build VaultContext
    let vault_ctx = VaultContext::new(
        cfg.clone(),
        template_repo,
        capture_repo,
        macro_repo,
        type_registry,
    );

    // Parse frontmatter for NoteContext
    let parsed = parse_frontmatter(content).map_err(|e| e.to_string())?;

    // Convert Frontmatter to serde_yaml::Value
    let frontmatter = match parsed.frontmatter {
        Some(fm) => {
            let mut mapping = serde_yaml::Mapping::new();
            for (k, v) in fm.fields {
                mapping.insert(serde_yaml::Value::String(k), v);
            }
            serde_yaml::Value::Mapping(mapping)
        }
        None => serde_yaml::Value::Null,
    };

    // Convert variables to serde_yaml::Value
    let mut vars_mapping = serde_yaml::Mapping::new();
    for (k, v) in variables {
        vars_mapping.insert(
            serde_yaml::Value::String(k.clone()),
            serde_yaml::Value::String(v.clone()),
        );
    }
    let vars_value = serde_yaml::Value::Mapping(vars_mapping);

    // Build NoteContext
    let note_ctx = NoteContext::new(
        output_path.to_path_buf(),
        typedef.name.clone(),
        frontmatter,
        content.to_string(),
        vars_value,
    );

    // Run the hook and return its result
    run_on_create_hook(&typedef, &note_ctx, vault_ctx).map_err(|e| e.to_string())
}

/// Apply hook modifications to the output file.
fn apply_hook_modifications(
    output_path: &Path,
    original_content: &str,
    hook_result: &HookResult,
) -> Result<(), String> {
    if !hook_result.modified {
        return Ok(());
    }

    // Parse original content to get structure
    let original_parsed =
        parse_frontmatter(original_content).map_err(|e| e.to_string())?;

    // Determine final frontmatter
    let final_frontmatter = if let Some(ref new_fm) = hook_result.frontmatter {
        new_fm.clone()
    } else if let Some(fm) = original_parsed.frontmatter {
        let mut mapping = serde_yaml::Mapping::new();
        for (k, v) in fm.fields {
            mapping.insert(serde_yaml::Value::String(k), v);
        }
        serde_yaml::Value::Mapping(mapping)
    } else {
        serde_yaml::Value::Null
    };

    // Determine final content body
    // If hook returned content, it might contain frontmatter, so parse it to get just the body
    let final_body = if let Some(ref new_content) = hook_result.content {
        // Parse the hook's content to extract just the body (in case it includes frontmatter)
        let content_parsed = parse_frontmatter(new_content).map_err(|e| e.to_string())?;
        content_parsed.body
    } else {
        original_parsed.body
    };

    // Rebuild the document
    let final_content = if final_frontmatter.is_null() {
        final_body
    } else {
        let yaml_str =
            serde_yaml::to_string(&final_frontmatter).map_err(|e| e.to_string())?;
        format!("---\n{}---\n{}", yaml_str, final_body)
    };

    // Write back to file
    fs::write(output_path, final_content).map_err(|e| e.to_string())
}

/// Log a creation event to today's daily note.
/// Creates the daily note if it doesn't exist.
fn log_to_daily(
    cfg: &ResolvedConfig,
    note_type: &str,
    title: &str,
    note_id: &str,
    output_path: &Path,
) {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let time = chrono::Local::now().format("%H:%M").to_string();

    // Build daily note path (default pattern: Journal/Daily/YYYY-MM-DD.md)
    let daily_path = cfg.vault_root.join(format!("Journal/Daily/{}.md", today));

    // Ensure parent directory exists
    if let Some(parent) = daily_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("Warning: could not create daily directory: {e}");
            return;
        }
    }

    // Read or create daily note
    let mut content = match fs::read_to_string(&daily_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Create minimal daily note
            let content = format!(
                "---\ntype: daily\ndate: {}\n---\n\n# {}\n\n## Log\n",
                today, today
            );
            if let Err(e) = fs::write(&daily_path, &content) {
                eprintln!("Warning: could not create daily note: {e}");
                return;
            }
            println!("Created daily note: {}", daily_path.display());
            content
        }
        Err(e) => {
            eprintln!("Warning: could not read daily note: {e}");
            return;
        }
    };

    // Build the log entry with link to the note
    let rel_path = output_path.strip_prefix(&cfg.vault_root).unwrap_or(output_path);
    let link = rel_path.file_stem().and_then(|s| s.to_str()).unwrap_or("note");

    // Format: "- **HH:MM** Created task [MCP-001]: [[MCP-001|Title]]"
    let id_display =
        if note_id.is_empty() { String::new() } else { format!(" {}", note_id) };

    let log_entry = format!(
        "- **{}** Created {}{}: [[{}|{}]]\n",
        time, note_type, id_display, link, title
    );

    // Find the Log section and append, or append at end
    if let Some(log_pos) = content.find("## Log") {
        // Find the end of the Log section (next ## or end of file)
        let after_log = &content[log_pos + 6..]; // Skip "## Log"
        let insert_pos = if let Some(next_section) = after_log.find("\n## ") {
            log_pos + 6 + next_section
        } else {
            content.len()
        };

        // Insert the log entry
        content.insert_str(insert_pos, &format!("\n{}", log_entry));
    } else {
        // No Log section, add one
        content.push_str(&format!("\n## Log\n{}", log_entry));
    }

    // Write back
    if let Err(e) = fs::write(&daily_path, &content) {
        eprintln!("Warning: could not update daily note: {e}");
    }
}

/// Query existing projects from the index and prompt user to select one.
/// Returns None if user cancels, Some("inbox") for inbox, or Some(project_name) for a project.
fn prompt_project_selection(cfg: &ResolvedConfig) -> Option<String> {
    // Open the index database
    let index_path = cfg.vault_root.join(".mdvault/index.db");
    let db = match IndexDb::open(&index_path) {
        Ok(db) => db,
        Err(_) => {
            // No index yet, default to inbox
            println!("No index found. Task will go to inbox.");
            return Some("inbox".to_string());
        }
    };

    // Query all projects
    let query = NoteQuery { note_type: Some(NoteType::Project), ..Default::default() };

    let projects = match db.query_notes(&query) {
        Ok(p) => p,
        Err(_) => return Some("inbox".to_string()),
    };

    // Build selection items: inbox first, then projects
    let mut items: Vec<String> = vec!["Inbox (no project - for triage)".to_string()];

    for p in &projects {
        let title = if p.title.is_empty() { "Untitled" } else { &p.title };
        items.push(title.to_string());
    }

    // Show selector
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select project for this task")
        .items(&items)
        .default(0)
        .interact_opt()
        .ok()?;

    // Handle selection
    selection.map(|idx| {
        if idx == 0 {
            // Inbox selected
            "inbox".to_string()
        } else {
            // Project selected (idx - 1 because inbox is at 0)
            let project = &projects[idx - 1];
            project
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("project")
                .to_string()
        }
    })
}

/// Collect variables from Lua schema fields that have `prompt` set.
/// Prompts for fields that:
/// - Have `prompt` defined (the prompt text to show)
/// - Are not already provided in `provided_vars`
/// - Are not marked as `core` (managed by Rust)
fn collect_schema_variables(
    typedef: &TypeDefinition,
    provided_vars: &HashMap<String, String>,
    options: &PromptOptions,
) -> Result<CollectedVars, String> {
    let mut result = CollectedVars {
        values: HashMap::new(),
        prompted: Vec::new(),
        defaulted: Vec::new(),
    };

    // Start with provided vars
    for (k, v) in provided_vars {
        result.values.insert(k.clone(), v.clone());
    }

    // Process schema fields in alphabetical order for consistency
    let mut fields: Vec<_> = typedef.schema.iter().collect();
    fields.sort_by(|a, b| a.0.cmp(b.0));

    for (field_name, schema) in fields {
        // Skip if already provided
        if result.values.contains_key(field_name) {
            continue;
        }

        // Skip core fields (managed by Rust)
        if schema.core {
            continue;
        }

        // If field has a prompt, ask the user
        if let Some(ref prompt_text) = schema.prompt {
            if options.batch_mode {
                // In batch mode, use default or fail if required
                if let Some(ref default) = schema.default {
                    let value = yaml_value_to_string(default);
                    result.values.insert(field_name.clone(), value);
                    result.defaulted.push(field_name.clone());
                } else if schema.required {
                    return Err(format!(
                        "Missing required field '{}' in batch mode",
                        field_name
                    ));
                }
            } else {
                // Interactive: prompt for field
                let enum_values = schema.enum_values.as_deref();
                let default_str = schema.default.as_ref().map(yaml_value_to_string);

                match prompt_for_schema_field(
                    field_name,
                    prompt_text,
                    enum_values,
                    default_str.as_deref(),
                    schema.required,
                    schema.multiline,
                ) {
                    Ok(value) if !value.is_empty() => {
                        result.values.insert(field_name.clone(), value);
                        result.prompted.push(field_name.clone());
                    }
                    Ok(_) => {
                        // Empty value - use default if available
                        if let Some(ref default) = schema.default {
                            result.values.insert(
                                field_name.clone(),
                                yaml_value_to_string(default),
                            );
                            result.defaulted.push(field_name.clone());
                        }
                        result.prompted.push(field_name.clone());
                    }
                    Err(e) => return Err(e),
                }
            }
        } else if let Some(ref default) = schema.default {
            // No prompt but has default - use it
            result.values.insert(field_name.clone(), yaml_value_to_string(default));
            result.defaulted.push(field_name.clone());
        }
    }

    Ok(result)
}

/// Prompt for a single schema field value.
///
/// Uses different widgets based on field type:
/// - Enum fields: Select widget for choosing from options
/// - Multiline fields: Editor widget for multi-line text
/// - Other fields: Input widget for single-line text
fn prompt_for_schema_field(
    field_name: &str,
    prompt_text: &str,
    enum_values: Option<&[String]>,
    default: Option<&str>,
    required: bool,
    multiline: bool,
) -> Result<String, String> {
    let theme = ColorfulTheme::default();

    // If enum values provided, use Select widget
    if let Some(values) = enum_values {
        let default_idx =
            default.and_then(|d| values.iter().position(|v| v == d)).unwrap_or(0);

        let selection = Select::with_theme(&theme)
            .with_prompt(prompt_text)
            .items(values)
            .default(default_idx)
            .interact_opt()
            .map_err(|e| {
                format!("Failed to read selection for '{}': {}", field_name, e)
            })?;

        return match selection {
            Some(idx) => Ok(values[idx].clone()),
            None => {
                // User cancelled - use default if available, else empty
                Ok(default.unwrap_or("").to_string())
            }
        };
    }

    // If multiline, use Editor widget
    if multiline {
        let initial = default.unwrap_or("");
        let content = Editor::new()
            .edit(initial)
            .map_err(|e| format!("Editor error for '{}': {}", field_name, e))?
            .unwrap_or_else(|| initial.to_string());
        return Ok(content);
    }

    // Default: use Input widget
    let mut builder = Input::<String>::with_theme(&theme).with_prompt(prompt_text);

    if let Some(def) = default {
        builder = builder.default(def.to_string());
    }

    builder = builder.allow_empty(!required);

    builder
        .interact_text()
        .map_err(|e| format!("Failed to read input for '{}': {}", field_name, e))
}

/// Convert a serde_yaml::Value to a string for template context.
fn yaml_value_to_string(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Null => String::new(),
        other => serde_yaml::to_string(other).unwrap_or_default().trim().to_string(),
    }
}

/// Render an output path template with variable substitution.
/// Uses the template engine to support filters like `{{title | slugify}}`.
fn render_output_path(
    template: &str,
    cfg: &ResolvedConfig,
    ctx: &HashMap<String, String>,
) -> Result<PathBuf, String> {
    // Use the template engine to render with filter support
    let rendered = render_string(template, ctx).map_err(|e| e.to_string())?;

    // Make path absolute relative to vault root
    let path = PathBuf::from(&rendered);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(cfg.vault_root.join(path))
    }
}

/// Validate note content before writing.
///
/// This runs schema validation and custom Lua validate() function (if defined).
/// Returns Ok(None) if valid, Ok(Some(content)) if valid after auto-fixing,
/// or Err with error messages if validation fails.
fn validate_before_write(
    registry: &TypeRegistry,
    note_type: &str,
    output_path: &Path,
    content: &str,
) -> Result<Option<String>, Vec<String>> {
    // Parse frontmatter from rendered content
    let parsed = match parse_frontmatter(content) {
        Ok(p) => p,
        Err(e) => return Err(vec![format!("Failed to parse frontmatter: {}", e)]),
    };

    // Convert frontmatter to serde_yaml::Value for validation
    let frontmatter = match parsed.frontmatter {
        Some(fm) => {
            let mut mapping = serde_yaml::Mapping::new();
            for (k, v) in fm.fields {
                mapping.insert(serde_yaml::Value::String(k), v);
            }
            serde_yaml::Value::Mapping(mapping)
        }
        None => serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
    };

    // Run validation
    let path_str = output_path.to_string_lossy();
    let result =
        validate_note(registry, note_type, &path_str, &frontmatter, &parsed.body);

    if result.valid {
        Ok(None)
    } else {
        // Try to auto-fix validation errors
        let fix_result = try_fix_note(registry, note_type, content, &result.errors);
        if fix_result.fixed {
            if let Some(new_content) = fix_result.content {
                println!("Auto-fixed validation errors:");
                for fix in fix_result.fixes {
                    println!("  - {}", fix);
                }
                Ok(Some(new_content))
            } else {
                // Should not happen if fixed is true
                let errors: Vec<String> =
                    result.errors.iter().map(|e| e.to_string()).collect();
                Err(errors)
            }
        } else {
            // Collect all error messages
            let errors: Vec<String> =
                result.errors.iter().map(|e| e.to_string()).collect();
            Err(errors)
        }
    }
}

/// Helper to convert string to yaml value based on type hint
fn string_to_yaml_value(s: &str, field_type: Option<FieldType>) -> serde_yaml::Value {
    match field_type {
        Some(FieldType::Number) => {
            if let Ok(n) = s.parse::<i64>() {
                serde_yaml::Value::Number(n.into())
            } else if let Ok(n) = s.parse::<f64>() {
                serde_yaml::Value::Number(serde_yaml::Number::from(n))
            } else {
                serde_yaml::Value::String(s.to_string())
            }
        }
        Some(FieldType::Boolean) => {
            serde_yaml::Value::Bool(s.eq_ignore_ascii_case("true") || s == "1")
        }
        Some(FieldType::List) => {
            let items: Vec<serde_yaml::Value> = s
                .split(',')
                .map(|item| serde_yaml::Value::String(item.trim().to_string()))
                .collect();
            serde_yaml::Value::Sequence(items)
        }
        _ => serde_yaml::Value::String(s.to_string()),
    }
}

/// Inject user-provided variables into frontmatter if not already present.
///
/// This ensures that variables prompted for (or passed via CLI) appear in the output note
/// even if the template didn't explicitly include them.
fn inject_vars_into_frontmatter(
    content: &str,
    vars: &HashMap<String, String>,
    typedef: Option<&TypeDefinition>,
) -> Result<String, String> {
    let parsed = parse_frontmatter(content).map_err(|e| e.to_string())?;

    let mut fields: HashMap<String, serde_yaml::Value> =
        if let Some(fm) = parsed.frontmatter { fm.fields } else { HashMap::new() };

    let mut modified = false;

    for (k, v) in vars {
        // Skip keys that are usually handled specially or shouldn't be overridden blindly
        if k == "type" || k == "title" {
            continue;
        }

        // If field already exists, don't overwrite
        if fields.contains_key(k) {
            continue;
        }

        // Only inject if the variable is defined in the schema
        // Variables not in schema are considered ephemeral template inputs
        if let Some(td) = typedef {
            if let Some(schema) = td.schema.get(k) {
                let val = string_to_yaml_value(v, schema.field_type);
                fields.insert(k.clone(), val);
                modified = true;
            }
        }
    }

    if !modified {
        return Ok(content.to_string());
    }

    // Rebuild
    let mut mapping = serde_yaml::Mapping::new();
    for (k, v) in fields {
        mapping.insert(serde_yaml::Value::String(k), v);
    }
    let yaml_str = serde_yaml::to_string(&serde_yaml::Value::Mapping(mapping))
        .map_err(|e| e.to_string())?;

    Ok(format!("---\n{}---\n{}", yaml_str, parsed.body))
}
