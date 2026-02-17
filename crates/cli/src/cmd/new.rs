use crate::prompt::{
    create_fuzzy_selector_callback, prompt_for_enum, prompt_for_field, CollectedVars,
    PromptOptions,
};
use crate::NewArgs;
use dialoguer::{theme::ColorfulTheme, Editor, Input, Select};
use mdvault_core::activity::ActivityLogService;
use mdvault_core::captures::CaptureRepository;
use mdvault_core::config::loader::{default_config_path, ConfigLoader};
use mdvault_core::config::types::ResolvedConfig;
use mdvault_core::context::ContextManager;
use mdvault_core::domain::{
    CoreMetadata, CreationContext, DailyLogService, NoteCreator,
    NoteType as DomainNoteType,
};
use mdvault_core::frontmatter::parse as parse_frontmatter;
use mdvault_core::frontmatter::{serialize_with_order, Frontmatter, ParsedDocument};
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
    discovery::load_typedef_from_file, validate_note_for_creation, TypeDefinition,
    TypeRegistry, TypedefRepository,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

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
    // If title is not provided here, collect_schema_variables will prompt for it
    // when the schema has title with `prompt` or `default` set.
    let title = args.title.clone().or_else(|| args.note_type.clone());
    if let Some(ref t) = title {
        provided_vars.entry("title".to_string()).or_insert(t.clone());
    }

    // For task templates: use focus context or show project picker if project not already provided
    if template_name == "task" && !provided_vars.contains_key("project") {
        // Check for active focus context first
        if let Ok(context_mgr) = ContextManager::load(&cfg.vault_root) {
            if let Some(focused_project) = context_mgr.active_project() {
                debug!("Using focused project: {}", focused_project);
                provided_vars.insert("project".to_string(), focused_project.to_string());
            }
        }

        // If still no project and not batch mode, prompt for selection
        if !provided_vars.contains_key("project") && !args.batch {
            if let Some(project) = prompt_project_selection(cfg) {
                provided_vars.insert("project".to_string(), project);
            }
        }
    }

    // Build minimal context for variable resolution
    let minimal_ctx = build_minimal_context(cfg, &info);

    // Collect variables using Lua schema prompts
    let prompt_options = PromptOptions { batch_mode: args.batch };

    let collected = if let Some(ref typedef) = lua_typedef {
        // Use Lua schema for prompting - fields with `prompt` set will be prompted
        match collect_schema_variables(
            typedef,
            &provided_vars,
            &prompt_options,
            Some(cfg),
        ) {
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
                    Ok(None) => {} // Valid
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

                let order =
                    lua_typedef.as_ref().and_then(|td| td.frontmatter_order.as_deref());

                if let Err(e) = apply_hook_modifications(
                    &output_path,
                    &final_content,
                    &hook_result,
                    order,
                ) {
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
        if let Err(e) = DailyLogService::log_creation(
            cfg,
            template_name,
            &title,
            &note_id,
            &output_path,
        ) {
            eprintln!("Warning: failed to log to daily note: {e}");
        }

        // Force reindex so the new note appears in queries
        reindex_vault(cfg);
    }

    // Log to activity log
    if let Some(activity) = ActivityLogService::try_from_config(cfg) {
        let note_id = ctx
            .get("task-id")
            .or_else(|| ctx.get("project-id"))
            .cloned()
            .unwrap_or_default();
        let title = ctx.get("title").cloned();
        let _ = activity.log_new(template_name, &note_id, &output_path, title.as_deref());
    }

    println!("OK   mdv new");
    println!("template: {}", template_name);
    println!("output:   {}", output_path.display());
}

/// Run type-based scaffolding mode using the domain module.
///
/// This implementation uses trait-based dispatch for note type behaviors,
/// replacing the previous scattered if/else checks.
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

    // Check for template
    let template_repo = TemplateRepository::new(&cfg.templates_dir).ok();
    let loaded_template =
        template_repo.as_ref().and_then(|repo| repo.get_by_name(type_name).ok());

    // Get title - check for schema default before prompting
    let title = match &args.title {
        Some(t) => t.clone(),
        None => {
            // Check if the type's schema has a default for title
            let title_default = type_registry.get(type_name).and_then(|td| {
                td.schema.get("title").and_then(|fs| fs.default.as_ref()).and_then(|v| {
                    match v {
                        serde_yaml::Value::String(s) => Some(s.clone()),
                        _ => None,
                    }
                })
            });

            if let Some(default_title) = title_default {
                // Use the schema default (e.g., today's date for daily notes)
                default_title
            } else if args.batch {
                eprintln!("Error: title is required in batch mode");
                eprintln!("Usage: mdv new {type_name} \"Title\"");
                std::process::exit(1);
            } else {
                // Prompt for title
                match prompt_for_field("title", "Note title", None, true) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        std::process::exit(1);
                    }
                }
            }
        }
    };

    // Create domain note type
    let note_type = match DomainNoteType::from_name(type_name, &type_registry) {
        Ok(nt) => nt,
        Err(e) => {
            eprintln!("Failed to create note type: {e}");
            std::process::exit(1);
        }
    };

    // Build creation context
    let cli_vars: HashMap<String, String> = args.vars.iter().cloned().collect();
    let mut ctx = CreationContext::new(type_name, &title, cfg, &type_registry)
        .with_vars(cli_vars)
        .with_batch_mode(args.batch);

    // For task types: inject focused project if not already provided via --var
    if type_name == "task" && !ctx.vars.contains_key("project") {
        if let Ok(context_mgr) = ContextManager::load(&cfg.vault_root) {
            if let Some(focused_project) = context_mgr.active_project() {
                debug!("Using focused project for task: {}", focused_project);
                ctx.set_var("project", focused_project);
            }
        }
    }

    // Handle type-specific prompts
    let behavior = note_type.behavior();
    let prompts = behavior.type_prompts(&ctx.to_prompt_context());

    for prompt in prompts {
        // Skip if already provided
        if ctx.vars.contains_key(&prompt.field_name) {
            continue;
        }

        if args.batch {
            // In batch mode, use default or skip
            if let Some(default) = prompt.default_value {
                ctx.set_var(&prompt.field_name, default);
            }
        } else {
            // Interactive prompt
            match &prompt.prompt_type {
                mdvault_core::domain::PromptType::ProjectSelector => {
                    // Use existing project selection logic
                    match prompt_project_selection(cfg) {
                        Some(project) => {
                            ctx.set_var("project", &project);
                        }
                        None => {
                            eprintln!("No project selected");
                            std::process::exit(1);
                        }
                    }
                }
                mdvault_core::domain::PromptType::Text => {
                    match prompt_for_field(
                        &prompt.field_name,
                        &prompt.prompt_text,
                        prompt.default_value.as_deref(),
                        prompt.required,
                    ) {
                        Ok(value) => {
                            ctx.set_var(&prompt.field_name, value);
                        }
                        Err(e) => {
                            eprintln!("Error: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                mdvault_core::domain::PromptType::Multiline => {
                    // For multiline, use editor
                    if let Some(text) = Editor::new().edit("").ok().flatten() {
                        ctx.set_var(&prompt.field_name, text);
                    }
                }
                mdvault_core::domain::PromptType::Select(options) => {
                    match prompt_for_enum(
                        &prompt.field_name,
                        &prompt.prompt_text,
                        options,
                        prompt.default_value.as_deref(),
                    ) {
                        Ok(value) => {
                            ctx.set_var(&prompt.field_name, value);
                        }
                        Err(e) => {
                            eprintln!("Error: {e}");
                            std::process::exit(1);
                        }
                    }
                }
            }
        }
    }

    // Collect schema-based prompts from Lua typedef (if available)
    // This handles fields with `prompt` attribute that aren't type-specific
    if let Some(ref typedef) = ctx.typedef {
        let provided_vars: HashMap<String, String> = ctx.vars.clone();
        let prompt_options = PromptOptions { batch_mode: args.batch };

        match collect_schema_variables(
            typedef,
            &provided_vars,
            &prompt_options,
            Some(cfg),
        ) {
            Ok(collected) => {
                // Merge collected variables into context
                for (k, v) in collected.values {
                    ctx.set_var(&k, v);
                }
            }
            Err(e) => {
                eprintln!("Error collecting schema variables: {e}");
                std::process::exit(1);
            }
        }
    }

    // Create the note using NoteCreator (handles both template and scaffolding)
    // Set template in context if available
    if let Some(loaded) = loaded_template {
        ctx.template = Some(loaded);
    }

    // Set output path override if provided via CLI
    if let Some(ref out) = args.output {
        ctx.output_path = Some(out.clone());
    }

    let creator = NoteCreator::new(note_type);
    match creator.create(&mut ctx) {
        Ok(result) => {
            // Read the created content for validation and hooks
            let mut content = match std::fs::read_to_string(&result.path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("FAIL mdv new");
                    eprintln!("Failed to read created file: {e}");
                    std::process::exit(1);
                }
            };

            // Validate and auto-fix if needed
            match validate_before_write(&type_registry, type_name, &result.path, &content)
            {
                Ok(Some(fixed)) => {
                    println!("Auto-fixed validation errors");
                    content = fixed;
                    // Write the fixed content back
                    if let Err(e) = fs::write(&result.path, &content) {
                        eprintln!("Warning: failed to write fixed content: {e}");
                    }
                }
                Ok(None) => {} // Valid
                Err(errors) => {
                    // Validation failed - remove the created file and exit
                    let _ = fs::remove_file(&result.path);
                    eprintln!("Validation failed:");
                    for err in &errors {
                        eprintln!("  - {}", err);
                    }
                    std::process::exit(1);
                }
            }

            match run_on_create_hook_if_exists(
                cfg,
                &result.path,
                &content,
                ctx.typedef.as_deref(),
                &ctx.vars,
            ) {
                Ok(hook_result) if hook_result.modified => {
                    let order = ctx
                        .typedef
                        .as_deref()
                        .and_then(|td| td.frontmatter_order.clone());

                    // Check if hook modified variables - if so and we have a template, re-render
                    if let Some(ref new_vars) = hook_result.variables {
                        if let serde_yaml::Value::Mapping(ref vars_map) = new_vars {
                            // Update ctx.vars with new values from hook
                            for (k, v) in vars_map {
                                if let serde_yaml::Value::String(ks) = k {
                                    let vs = match v {
                                        serde_yaml::Value::String(s) => s.clone(),
                                        serde_yaml::Value::Number(n) => n.to_string(),
                                        serde_yaml::Value::Bool(b) => b.to_string(),
                                        _ => format!("{:?}", v),
                                    };
                                    ctx.set_var(ks, vs);
                                }
                            }
                        }

                        // Re-render template with updated variables if we had a template
                        if let Some(ref loaded) = ctx.template {
                            let info = TemplateInfo {
                                logical_name: loaded.logical_name.clone(),
                                path: loaded.path.clone(),
                            };
                            let mut template_ctx = build_minimal_context(cfg, &info);
                            template_ctx.insert("title".to_string(), ctx.title.clone());
                            for (k, v) in &ctx.vars {
                                template_ctx.insert(k.clone(), v.clone());
                            }
                            template_ctx.insert(
                                "output_path".to_string(),
                                result.path.to_string_lossy().to_string(),
                            );
                            if let Some(name) =
                                result.path.file_name().and_then(|s| s.to_str())
                            {
                                template_ctx.insert(
                                    "output_filename".to_string(),
                                    name.to_string(),
                                );
                            }

                            let regenerated = match render(loaded, &template_ctx) {
                                Ok(rendered) => rendered,
                                Err(e) => {
                                    eprintln!(
                                        "Warning: failed to re-render template: {e}"
                                    );
                                    content.clone()
                                }
                            };

                            // Re-apply core metadata and write
                            let final_content = match ensure_core_metadata(
                                &regenerated,
                                &ctx.core_metadata,
                                order.as_deref(),
                            ) {
                                Ok(fixed) => fixed,
                                Err(_) => regenerated,
                            };

                            if let Err(e) = fs::write(&result.path, &final_content) {
                                eprintln!(
                                    "Warning: failed to write re-rendered content: {e}"
                                );
                            }
                        } else {
                            // No template, just apply hook modifications
                            if let Err(e) = apply_hook_modifications(
                                &result.path,
                                &content,
                                &hook_result,
                                order.as_deref(),
                            ) {
                                eprintln!(
                                    "Warning: failed to apply hook modifications: {e}"
                                );
                            }
                        }
                    } else {
                        // No variable changes, just apply hook modifications
                        if let Err(e) = apply_hook_modifications(
                            &result.path,
                            &content,
                            &hook_result,
                            order.as_deref(),
                        ) {
                            eprintln!("Warning: failed to apply hook modifications: {e}");
                        }
                    }

                    // Re-apply core metadata to protect against hook tampering
                    if let Ok(current) = std::fs::read_to_string(&result.path) {
                        if let Ok(fixed) = ensure_core_metadata(
                            &current,
                            &ctx.core_metadata,
                            order.as_deref(),
                        ) {
                            if let Err(e) = std::fs::write(&result.path, fixed) {
                                eprintln!(
                                    "Warning: failed to re-apply core metadata: {e}"
                                );
                            }
                        }
                    }
                }
                Ok(_) => {} // No modifications
                Err(e) => {
                    eprintln!("Warning: hook execution failed: {e}");
                }
            }

            // Note: Daily logging is handled by behavior.after_create() via DailyLogService

            // Log to activity log
            if let Some(activity) = ActivityLogService::try_from_config(cfg) {
                let note_id = result.generated_id.as_deref().unwrap_or("");
                let title = ctx.vars.get("title").map(|s| s.as_str());
                let _ = activity.log_new(type_name, note_id, &result.path, title);
            }

            println!("OK   mdv new");
            println!("type:   {}", result.type_name);
            if let Some(ref id) = result.generated_id {
                println!("id:     {}", id);
            }
            println!("output: {}", result.path.display());
        }
        Err(e) => {
            eprintln!("FAIL mdv new");
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

/// Ensure core metadata fields are present in the note content.
///
/// This function is called after template rendering and hook execution to guarantee
/// that required fields managed by mdvault are not removed or corrupted by user code.
/// Templates and hooks can ADD fields but cannot REMOVE core fields.
fn ensure_core_metadata(
    content: &str,
    core: &CoreMetadata,
    order: Option<&[String]>,
) -> Result<String, String> {
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

    if let Some(ref date) = core.date {
        fields.insert("date".to_string(), serde_yaml::Value::String(date.clone()));
    }

    if let Some(ref week) = core.week {
        fields.insert("week".to_string(), serde_yaml::Value::String(week.clone()));
    }

    // Rebuild the document
    let doc =
        ParsedDocument { frontmatter: Some(Frontmatter { fields }), body: parsed.body };

    Ok(serialize_with_order(&doc, order))
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
            let builder = IndexBuilder::with_exclusions(
                &db,
                &cfg.vault_root,
                cfg.excluded_folders.clone(),
            );
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

    // Try to open index
    let index_db = IndexDb::open(&cfg.vault_root.join(".mdvault/index.db"))
        .ok()
        .map(std::sync::Arc::new);

    // Build VaultContext with selector callback for interactive prompts
    let mut vault_ctx = VaultContext::new(
        cfg.clone(),
        template_repo,
        capture_repo,
        macro_repo,
        type_registry,
    )
    .with_selector(create_fuzzy_selector_callback());

    if let Some(db) = index_db {
        vault_ctx = vault_ctx.with_index(db);
    }

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
    order: Option<&[String]>,
) -> Result<(), String> {
    if !hook_result.modified {
        return Ok(());
    }

    // Parse original content to get structure
    let original_parsed =
        parse_frontmatter(original_content).map_err(|e| e.to_string())?;

    // Start with original frontmatter (includes schema defaults)
    let mut final_fields = if let Some(fm) = original_parsed.frontmatter {
        fm.fields
    } else {
        HashMap::new()
    };

    // Merge hook's frontmatter on top (hook values win on conflict)
    if let Some(serde_yaml::Value::Mapping(map)) = hook_result.frontmatter.as_ref() {
        for (k, v) in map {
            if let serde_yaml::Value::String(ks) = k {
                final_fields.insert(ks.clone(), v.clone());
            }
        }
    }

    // Determine final content body
    let final_body = if let Some(ref new_content) = hook_result.content {
        let content_parsed = parse_frontmatter(new_content).map_err(|e| e.to_string())?;
        content_parsed.body
    } else {
        original_parsed.body
    };

    // Rebuild the document
    let doc = ParsedDocument {
        frontmatter: Some(Frontmatter { fields: final_fields }),
        body: final_body,
    };

    let final_content = serialize_with_order(&doc, order);

    // Write back to file
    fs::write(output_path, final_content).map_err(|e| e.to_string())
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
///
/// If a field has `selector` set, shows a fuzzy note selector instead of text input.
fn collect_schema_variables(
    typedef: &TypeDefinition,
    provided_vars: &HashMap<String, String>,
    options: &PromptOptions,
    cfg: Option<&ResolvedConfig>,
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

        // Skip core fields that have no prompt and no default (auto-managed by Rust)
        // Core fields with prompt OR default should still be processed
        if schema.core && schema.prompt.is_none() && schema.default.is_none() {
            continue;
        }

        // Check if field has a selector - use fuzzy note picker
        if let Some(ref selector_type) = schema.selector {
            if options.batch_mode {
                // In batch mode, use default or fail if required
                if let Some(ref default) = schema.default {
                    let value = yaml_value_to_string(default);
                    result.values.insert(field_name.clone(), value);
                    result.defaulted.push(field_name.clone());
                } else if schema.required {
                    return Err(format!(
                        "Missing required field '{}' in batch mode (selector field)",
                        field_name
                    ));
                }
            } else if let Some(config) = cfg {
                // Interactive: show fuzzy selector for notes of the specified type
                let prompt_text = schema.prompt.as_deref().unwrap_or(field_name.as_str());

                match prompt_with_note_selector(config, selector_type, prompt_text) {
                    Ok(Some(value)) => {
                        result.values.insert(field_name.clone(), value);
                        result.prompted.push(field_name.clone());
                    }
                    Ok(None) => {
                        // User cancelled - use default if available
                        if let Some(ref default) = schema.default {
                            result.values.insert(
                                field_name.clone(),
                                yaml_value_to_string(default),
                            );
                            result.defaulted.push(field_name.clone());
                        } else if schema.required {
                            return Err(format!(
                                "Required field '{}' was cancelled",
                                field_name
                            ));
                        }
                    }
                    Err(e) => return Err(e),
                }
            } else {
                // No config available, fall through to default handling
                if let Some(ref default) = schema.default {
                    result
                        .values
                        .insert(field_name.clone(), yaml_value_to_string(default));
                    result.defaulted.push(field_name.clone());
                }
            }
        } else if let Some(ref prompt_text) = schema.prompt {
            // If field has a prompt, ask the user
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

    // Process template variables (for body substitution, not frontmatter)
    // These are defined in the `variables` section of Lua typedefs
    let mut vars: Vec<_> = typedef.variables.iter().collect();
    vars.sort_by(|a, b| a.0.cmp(b.0));

    for (var_name, var_spec) in vars {
        // Skip if already provided
        if result.values.contains_key(var_name) {
            continue;
        }

        let prompt_text = var_spec.prompt();
        let default_value = var_spec.default();
        let is_required = var_spec.is_required();

        // Only prompt if there's a prompt text defined
        if !prompt_text.is_empty() {
            if options.batch_mode {
                // In batch mode, use default or fail if required
                if let Some(default) = default_value {
                    result.values.insert(var_name.clone(), default.to_string());
                    result.defaulted.push(var_name.clone());
                } else if is_required {
                    return Err(format!(
                        "Missing required variable '{}' in batch mode",
                        var_name
                    ));
                }
            } else {
                // Interactive: prompt for variable
                match prompt_for_variable(
                    var_name,
                    prompt_text,
                    default_value,
                    is_required,
                ) {
                    Ok(value) if !value.is_empty() => {
                        result.values.insert(var_name.clone(), value);
                        result.prompted.push(var_name.clone());
                    }
                    Ok(_) => {
                        // Empty value - use default if available
                        if let Some(default) = default_value {
                            result.values.insert(var_name.clone(), default.to_string());
                            result.defaulted.push(var_name.clone());
                        }
                        result.prompted.push(var_name.clone());
                    }
                    Err(e) => return Err(e),
                }
            }
        } else if let Some(default) = default_value {
            // No prompt but has default - use it
            result.values.insert(var_name.clone(), default.to_string());
            result.defaulted.push(var_name.clone());
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
            .map_err(|e| format!("Editor error for '{}': {}", field_name, e))?;
        return Ok(content.unwrap_or_else(|| initial.to_string()));
    }

    // Default: use Input widget
    let mut input = Input::<String>::with_theme(&theme);
    input = input.with_prompt(prompt_text);
    input = input.allow_empty(!required);

    if let Some(def) = default {
        input = input.with_initial_text(def);
    }

    input
        .interact_text()
        .map_err(|e| format!("Failed to read input for '{}': {}", field_name, e))
}

/// Prompt using a fuzzy note selector.
///
/// Opens the index database, queries notes of the specified type,
/// and shows a fuzzy selector for the user to pick from.
///
/// Returns:
/// - `Ok(Some(value))` - User selected a note, value is the note's name (file stem)
/// - `Ok(None)` - User cancelled selection
/// - `Err(msg)` - Error occurred
fn prompt_with_note_selector(
    cfg: &ResolvedConfig,
    note_type: &str,
    prompt_text: &str,
) -> Result<Option<String>, String> {
    use dialoguer::FuzzySelect;

    // Open the index database
    let index_path = cfg.vault_root.join(".mdvault/index.db");
    let db = IndexDb::open(&index_path).map_err(|e| {
        format!("Failed to open index for selector (run 'mdv reindex' first): {}", e)
    })?;

    // Query notes of the specified type
    let query = NoteQuery {
        note_type: Some(note_type.parse().unwrap_or_default()),
        ..Default::default()
    };

    let notes = db.query_notes(&query).map_err(|e| format!("Query error: {}", e))?;

    if notes.is_empty() {
        return Ok(None);
    }

    // Build selection items: display title, return file stem (name without .md)
    let items: Vec<String> = notes.iter().map(|n| n.title.clone()).collect();

    // Show fuzzy selector
    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt_text)
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|e| format!("Selector error: {}", e))?;

    Ok(selection.map(|idx| {
        // Return the file stem (note name without .md extension)
        notes[idx]
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    }))
}

/// Prompt for a template variable value.
///
/// Template variables are simpler than schema fields - they're always strings
/// and don't support enum or multiline options.
fn prompt_for_variable(
    var_name: &str,
    prompt_text: &str,
    default: Option<&str>,
    required: bool,
) -> Result<String, String> {
    let theme = ColorfulTheme::default();

    let mut input = Input::<String>::with_theme(&theme);
    input = input.with_prompt(prompt_text);
    input = input.allow_empty(!required);

    if let Some(def) = default {
        input = input.with_initial_text(def);
    }

    input
        .interact_text()
        .map_err(|e| format!("Failed to read input for '{}': {}", var_name, e))
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

    // Run validation (use creation variant to skip inherited fields)
    let path_str = output_path.to_string_lossy();
    let result = validate_note_for_creation(
        registry,
        note_type,
        &path_str,
        &frontmatter,
        &parsed.body,
    );

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

#[cfg(test)]
mod tests {
    use super::*;
    use mdvault_core::types::FieldSchema;
    use serde_yaml::Value;

    #[test]
    fn test_yaml_value_to_string() {
        assert_eq!(yaml_value_to_string(&Value::String("foo".into())), "foo");
        assert_eq!(yaml_value_to_string(&Value::Number(42.into())), "42");
        assert_eq!(yaml_value_to_string(&Value::Bool(true)), "true");
        assert_eq!(yaml_value_to_string(&Value::Null), "");
    }

    #[test]
    fn test_ensure_core_metadata() {
        let content = "---\nexisting: val\n---\nbody";
        let core = CoreMetadata {
            note_type: Some("task".into()),
            task_id: Some("TST-001".into()),
            ..Default::default()
        };

        let result = ensure_core_metadata(content, &core, None).unwrap();

        let parsed = parse_frontmatter(&result).unwrap();
        let fm = parsed.frontmatter.unwrap();

        assert_eq!(fm.fields.get("type").unwrap().as_str(), Some("task"));
        assert_eq!(fm.fields.get("task-id").unwrap().as_str(), Some("TST-001"));
        assert_eq!(fm.fields.get("existing").unwrap().as_str(), Some("val"));
        assert_eq!(parsed.body.trim(), "body");
    }

    #[test]
    fn test_extract_note_type() {
        let content = "---\ntype: project\n---\nbody";
        assert_eq!(extract_note_type(content), Some("project".into()));

        let content_no_type = "---\ntitle: foo\n---\nbody";
        assert_eq!(extract_note_type(content_no_type), None);
    }

    #[test]
    fn test_collect_schema_variables_batch_missing_required() {
        let mut schema = HashMap::new();
        schema.insert(
            "req".to_string(),
            FieldSchema {
                required: true,
                prompt: Some("Required field?".to_string()),
                ..Default::default()
            },
        );

        let typedef = TypeDefinition { schema, ..TypeDefinition::empty("test") };

        let provided = HashMap::new();
        let options = PromptOptions { batch_mode: true };

        let result = collect_schema_variables(&typedef, &provided, &options, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required field"));
    }

    #[test]
    fn test_collect_schema_variables_selector_batch_with_default() {
        let mut schema = HashMap::new();
        schema.insert(
            "project".to_string(),
            FieldSchema {
                selector: Some("project".to_string()),
                prompt: Some("Select project".to_string()),
                default: Some(Value::String("inbox".to_string())),
                ..Default::default()
            },
        );

        let typedef = TypeDefinition { schema, ..TypeDefinition::empty("test") };

        let provided = HashMap::new();
        let options = PromptOptions { batch_mode: true };

        // In batch mode with a default, selector field should use the default
        let result =
            collect_schema_variables(&typedef, &provided, &options, None).unwrap();
        assert_eq!(result.values.get("project"), Some(&"inbox".to_string()));
        assert!(result.defaulted.contains(&"project".to_string()));
    }

    #[test]
    fn test_collect_schema_variables_selector_batch_required_no_default() {
        let mut schema = HashMap::new();
        schema.insert(
            "project".to_string(),
            FieldSchema {
                selector: Some("project".to_string()),
                prompt: Some("Select project".to_string()),
                required: true,
                ..Default::default()
            },
        );

        let typedef = TypeDefinition { schema, ..TypeDefinition::empty("test") };

        let provided = HashMap::new();
        let options = PromptOptions { batch_mode: true };

        // In batch mode without a default, required selector field should error
        let result = collect_schema_variables(&typedef, &provided, &options, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("selector field"));
    }

    #[test]
    fn test_collect_schema_variables_selector_provided() {
        let mut schema = HashMap::new();
        schema.insert(
            "project".to_string(),
            FieldSchema {
                selector: Some("project".to_string()),
                prompt: Some("Select project".to_string()),
                ..Default::default()
            },
        );

        let typedef = TypeDefinition { schema, ..TypeDefinition::empty("test") };

        let mut provided = HashMap::new();
        provided.insert("project".to_string(), "my-project".to_string());
        let options = PromptOptions { batch_mode: false };

        // When value is already provided, selector should not be invoked
        let result =
            collect_schema_variables(&typedef, &provided, &options, None).unwrap();
        assert_eq!(result.values.get("project"), Some(&"my-project".to_string()));
    }

    #[test]
    fn test_validate_before_write_bad_yaml() {
        let registry = TypeRegistry::new();
        let path = Path::new("foo.md");
        let content = "---\n: invalid\n---\nbody";
        let result = validate_before_write(&registry, "task", path, content);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(!errs.is_empty());
        assert!(errs[0].contains("Failed to parse frontmatter"));
    }
}
