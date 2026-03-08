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
use mdvault_core::domain::{CreationContext, NoteType as DomainNoteType};
use mdvault_core::frontmatter::parse as parse_frontmatter;
use mdvault_core::frontmatter::{serialize_with_order, Frontmatter, ParsedDocument};
use mdvault_core::index::{IndexBuilder, IndexDb, NoteQuery, NoteType};
use mdvault_core::macros::MacroRepository;
use mdvault_core::scripting::{
    run_on_create_hook, HookResult, NoteContext, VaultContext,
};
use mdvault_core::templates::discovery::TemplateInfo;
use mdvault_core::templates::engine::{
    build_minimal_context, render_string, render_with_ref_date,
    resolve_template_output_path,
};
use mdvault_core::templates::repository::TemplateRepository;
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

    // Resolve effective name: --template flag OR positional note_type
    let effective_name = args
        .template
        .as_deref()
        .or(args.note_type.as_deref())
        .unwrap_or_else(|| {
            eprintln!("Error: either provide a type name or use --template");
            eprintln!("Usage: mdv new <type> [title] [--var field=value]");
            eprintln!("       mdv new --template <name> [--var key=value]");
            std::process::exit(1);
        })
        .to_string();

    run_unified(&cfg, &effective_name, &args);
}

/// Dispatch type-specific prompts to interactive widgets and collect values.
///
/// Pattern: caller does `let prompts = behavior.type_prompts(&ctx.to_prompt_context());`
/// (immutable borrow, returns owned Vec), then passes the owned Vec here with a mutable
/// reference to vars. This avoids borrow checker issues with CreationContext.
fn dispatch_type_prompts(
    prompts: Vec<mdvault_core::domain::FieldPrompt>,
    vars: &mut HashMap<String, String>,
    cfg: &ResolvedConfig,
    batch_mode: bool,
) {
    for prompt in prompts {
        // Skip if already provided
        if vars.contains_key(&prompt.field_name) {
            continue;
        }

        if batch_mode {
            // In batch mode, use default or skip
            if let Some(default) = prompt.default_value {
                vars.insert(prompt.field_name, default);
            }
        } else {
            // Interactive prompt (fall back to default if not interactive)
            match &prompt.prompt_type {
                mdvault_core::domain::PromptType::ProjectSelector => {
                    match prompt_project_selection(cfg) {
                        Some(project) => {
                            vars.insert("project".to_string(), project);
                        }
                        None => {
                            if let Some(default) = prompt.default_value {
                                vars.insert(prompt.field_name, default);
                            } else {
                                eprintln!("No project selected");
                                std::process::exit(1);
                            }
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
                            vars.insert(prompt.field_name, value);
                        }
                        Err(_) if prompt.default_value.is_some() => {
                            vars.insert(prompt.field_name, prompt.default_value.unwrap());
                        }
                        Err(e) => {
                            eprintln!("Error: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                mdvault_core::domain::PromptType::Multiline => {
                    if let Some(text) = Editor::new().edit("").ok().flatten() {
                        vars.insert(prompt.field_name, text);
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
                            vars.insert(prompt.field_name, value);
                        }
                        Err(_) if prompt.default_value.is_some() => {
                            vars.insert(prompt.field_name, prompt.default_value.unwrap());
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
}

/// Merge CreationContext vars and core_metadata fields into a template render HashMap.
///
/// After `behavior.before_create()` populates the CreationContext (vars + core_metadata),
/// this copies those values into the HashMap used by the template engine.
fn merge_context_to_render_vars(
    ctx: &CreationContext,
    render_ctx: &mut HashMap<String, String>,
) {
    // Copy all vars
    for (k, v) in &ctx.vars {
        render_ctx.insert(k.clone(), v.clone());
    }

    // Copy core_metadata fields
    if let Some(ref id) = ctx.core_metadata.task_id {
        render_ctx.insert("task-id".to_string(), id.clone());
    }
    if let Some(ref id) = ctx.core_metadata.project_id {
        render_ctx.insert("project-id".to_string(), id.clone());
    }
    if let Some(ref id) = ctx.core_metadata.meeting_id {
        render_ctx.insert("meeting-id".to_string(), id.clone());
    }
    if let Some(ref p) = ctx.core_metadata.project {
        render_ctx.insert("project".to_string(), p.clone());
    }
    if let Some(ref d) = ctx.core_metadata.date {
        render_ctx.insert("date".to_string(), d.clone());
    }
    if let Some(ref w) = ctx.core_metadata.week {
        render_ctx.insert("week".to_string(), w.clone());
    }
    if let Some(counter) = ctx.core_metadata.task_counter {
        render_ctx.insert("task_counter".to_string(), counter.to_string());
    }
}

/// Inject focused project into vars if creating a task and project not already set.
fn inject_focus_context(cfg: &ResolvedConfig, vars: &mut HashMap<String, String>) {
    if vars.contains_key("project") {
        return;
    }
    if let Ok(context_mgr) = ContextManager::load(&cfg.vault_root) {
        if let Some(focused_project) = context_mgr.active_project() {
            debug!("Using focused project: {}", focused_project);
            vars.insert("project".to_string(), focused_project.to_string());
        }
    }
}

/// Resolve output path from Lua typedef, or exit with an error.
fn resolve_lua_output_or_exit(
    lua_typedef: &Option<TypeDefinition>,
    cfg: &ResolvedConfig,
    render_ctx: &HashMap<String, String>,
) -> PathBuf {
    if let Some(ref typedef) = lua_typedef {
        if let Some(ref output_template) = typedef.output {
            match render_output_path(output_template, cfg, render_ctx) {
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
        eprintln!("Error: --output is required (template has no output in frontmatter)");
        std::process::exit(1);
    }
}

/// Resolve title when not explicitly provided.
///
/// When `required` is true (scaffolding path, no template), title must be resolved — check
/// schema default, prompt interactively, or error in batch mode. When false (template path),
/// return empty since title is just another template variable.
fn resolve_title_or_default(
    effective_name: &str,
    required: bool,
    batch_mode: bool,
    type_registry: &Option<TypeRegistry>,
) -> String {
    // Check schema for a default title value
    let title_default =
        type_registry.as_ref().and_then(|reg| reg.get(effective_name)).and_then(|td| {
            td.schema.get("title").and_then(|fs| {
                fs.default.as_ref().and_then(|v| match v {
                    serde_yaml::Value::String(s) => Some(s.clone()),
                    _ => None,
                })
            })
        });

    if let Some(default_title) = title_default {
        return default_title;
    }

    if !required {
        // Template-based creation — title is just another variable, not strictly required
        return String::new();
    }

    // Scaffolding path — title is required for the heading
    if batch_mode {
        eprintln!("Error: title is required in batch mode");
        eprintln!("Usage: mdv new {effective_name} \"Title\"");
        std::process::exit(1);
    }

    match prompt_for_field("title", "Note title", None, true) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

/// Post-write pipeline: hook execution, core_metadata protection, after_create, reindex, activity logging.
///
/// This is the shared tail of the creation flow, called after the note file has been written to disk.
/// It runs the on_create hook (if defined), re-applies core_metadata defensively, calls
/// `behavior.after_create()` at the correct point (after hooks), reindexes, and logs activity.
#[allow(clippy::too_many_arguments)]
fn post_write_pipeline(
    cfg: &ResolvedConfig,
    output_path: &Path,
    rendered: &str,
    note_type: Option<&DomainNoteType>,
    creation_ctx: Option<&CreationContext>,
    lua_typedef: Option<&TypeDefinition>,
    loaded_template: Option<&mdvault_core::templates::repository::LoadedTemplate>,
    render_ctx: &mut HashMap<String, String>,
    ref_date: Option<chrono::NaiveDate>,
    type_name: &str,
) {
    // 1. Hook execution
    match run_on_create_hook_if_exists(
        cfg,
        output_path,
        rendered,
        lua_typedef,
        render_ctx,
    ) {
        Ok(hook_result) => {
            if hook_result.modified {
                let final_content = if let Some(ref new_vars) = hook_result.variables {
                    if let serde_yaml::Value::Mapping(map) = new_vars {
                        for (k, v) in map {
                            if let serde_yaml::Value::String(ks) = k {
                                let vs = match v {
                                    serde_yaml::Value::String(s) => s.clone(),
                                    serde_yaml::Value::Number(n) => n.to_string(),
                                    serde_yaml::Value::Bool(b) => b.to_string(),
                                    _ => format!("{:?}", v),
                                };
                                render_ctx.insert(ks.clone(), vs);
                            }
                        }
                    }

                    // Re-render with updated variables if we have a template
                    if let Some(loaded) = loaded_template {
                        match render_with_ref_date(loaded, render_ctx, ref_date) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("Warning: failed to re-render template: {e}");
                                rendered.to_string()
                            }
                        }
                    } else {
                        rendered.to_string()
                    }
                } else {
                    rendered.to_string()
                };

                let order =
                    lua_typedef.as_ref().and_then(|td| td.frontmatter_order.as_deref());

                if let Err(e) = apply_hook_modifications(
                    output_path,
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

    // 2. Re-apply core_metadata after hooks (defensive — protects against hook tampering)
    if let Some(ctx) = creation_ctx {
        let order = lua_typedef.as_ref().and_then(|td| td.frontmatter_order.as_deref());
        if let Ok(current) = std::fs::read_to_string(output_path) {
            if let Ok(fixed) = ctx.core_metadata.apply_to_content(&current, order) {
                if let Err(e) = std::fs::write(output_path, fixed) {
                    eprintln!("Warning: failed to re-apply core metadata: {e}");
                }
            }
        }
    }

    // 3. Call after_create after hooks (correct ordering: hooks may modify content
    //    that after_create references, e.g. daily logging, project logging)
    if let (Some(nt), Some(ctx)) = (note_type, creation_ctx) {
        // Read current content from disk (may have been modified by hooks)
        let current = std::fs::read_to_string(output_path).unwrap_or_default();
        if let Err(e) = nt.behavior().after_create(ctx, &current) {
            eprintln!("Warning: after_create failed: {e}");
        }
    }

    // 4. Reindex vault
    reindex_vault(cfg);

    // 5. Activity logging
    if let Some(activity) = ActivityLogService::try_from_config(cfg) {
        let note_id = creation_ctx
            .and_then(|ctx| {
                ctx.core_metadata
                    .task_id
                    .as_ref()
                    .or(ctx.core_metadata.project_id.as_ref())
                    .or(ctx.core_metadata.meeting_id.as_ref())
            })
            .cloned()
            .or_else(|| {
                render_ctx
                    .get("task-id")
                    .or_else(|| render_ctx.get("project-id"))
                    .cloned()
            })
            .unwrap_or_default();
        let title_val = render_ctx.get("title").cloned();
        let _ = activity.log_new(type_name, &note_id, output_path, title_val.as_deref());
    }
}

/// Unified creation flow — handles both template-based and scaffolding-based note creation.
///
/// The `effective_name` is treated as both a template name (optional lookup) and a type name.
/// When a template file exists, it is used for content rendering. When no template exists,
/// content is generated via `generate_scaffolding()`. The behaviour lifecycle always runs
/// for known types regardless of content source.
fn run_unified(cfg: &ResolvedConfig, effective_name: &str, args: &NewArgs) {
    // 1. Load TypedefRepository + TypeRegistry (with fallback to default dir)
    let typedef_repo = match &cfg.typedefs_fallback_dir {
        Some(fallback) => {
            TypedefRepository::with_fallback(&cfg.typedefs_dir, fallback).ok()
        }
        None => TypedefRepository::new(&cfg.typedefs_dir).ok(),
    };
    let type_registry =
        typedef_repo.as_ref().and_then(|repo| TypeRegistry::from_repository(repo).ok());

    // 2. Try load template (optional — not fatal if missing)
    let template_repo = TemplateRepository::new(&cfg.templates_dir).ok();
    let loaded_template =
        template_repo.as_ref().and_then(|repo| repo.get_by_name(effective_name).ok());

    // 3. Load Lua typedef: from template frontmatter (if template has lua ref),
    //    or from the type registry (for scaffolding path without template)
    let lua_typedef: Option<TypeDefinition> = loaded_template
        .as_ref()
        .and_then(|loaded| loaded.frontmatter.as_ref())
        .and_then(|fm| fm.lua.as_ref())
        .and_then(|lua_path| {
            let lua_file = cfg.resolve_lua_path(lua_path);
            match load_typedef_from_file(&lua_file) {
                Ok(td) => Some(td),
                Err(e) => {
                    eprintln!("Warning: failed to load Lua script '{}': {}", lua_path, e);
                    None
                }
            }
        })
        .or_else(|| {
            // Fall back to registry typedef (supports both scaffolding and templates
            // that co-exist with a Lua typedef without explicit lua: reference)
            type_registry
                .as_ref()
                .and_then(|reg| reg.get(effective_name).map(|arc| (*arc).clone()))
        });

    // 4. Try NoteType::try_from_name (for behaviour lifecycle)
    let mut note_type = type_registry
        .as_ref()
        .and_then(|reg| DomainNoteType::try_from_name(effective_name, reg));

    // 5. Error if no template AND no known type
    if loaded_template.is_none() && note_type.is_none() {
        eprintln!("Unknown type or template: {effective_name}");
        if let Some(ref reg) = type_registry {
            eprintln!("Available types:");
            for t in reg.list_all_types() {
                eprintln!("  {t}");
            }
        }
        std::process::exit(1);
    }

    // 6. Parse CLI vars and title
    let mut provided_vars: HashMap<String, String> = args.vars.iter().cloned().collect();

    // Title handling: check CLI arg, positional arg, schema default, prompt/empty.
    // Title is strictly required only for scaffolding (no template) — it's needed for the
    // heading. When a template exists, title is just another variable and may be optional.
    let needs_title = loaded_template.is_none();

    let title = if let Some(ref t) = args.title {
        t.clone()
    } else if let Some(ref t) = args.note_type {
        // `mdv new --template X "Title"` puts title in note_type position
        // Only use if --template was explicitly set (otherwise note_type IS the type name)
        if args.template.is_some() {
            t.clone()
        } else {
            // note_type is the type name itself, no title provided
            resolve_title_or_default(
                effective_name,
                needs_title,
                args.batch,
                &type_registry,
            )
        }
    } else {
        resolve_title_or_default(effective_name, needs_title, args.batch, &type_registry)
    };
    if !title.is_empty() {
        provided_vars.entry("title".to_string()).or_insert(title.clone());
    }

    // 7–9. If behaviour: build CreationContext, inject focus, type prompts, sync vars
    let mut creation_ctx: Option<CreationContext> = None;
    if let (Some(ref mut _nt), Some(ref registry)) = (&mut note_type, &type_registry) {
        // Use title from provided_vars (e.g. --var title=X) if available,
        // since resolve_title_or_default may return empty for template paths.
        // Behaviours like CustomBehavior::output_path() use ctx.title for path rendering.
        let ctx_title =
            provided_vars.get("title").cloned().unwrap_or_else(|| title.clone());
        let mut ctx = CreationContext::new(effective_name, &ctx_title, cfg, registry)
            .with_vars(provided_vars.clone())
            .with_batch_mode(args.batch);

        // Track which vars exist before lifecycle steps
        let pre_lifecycle_keys: std::collections::HashSet<String> =
            ctx.vars.keys().cloned().collect();

        // 7. Inject focus context
        inject_focus_context(cfg, &mut ctx.vars);

        // 8. Run type-specific prompts
        let behavior = _nt.behavior();
        let prompts = behavior.type_prompts(&ctx.to_prompt_context());
        dispatch_type_prompts(prompts, &mut ctx.vars, cfg, args.batch);

        // 9. Sync only NEW vars back to provided_vars
        for (k, v) in &ctx.vars {
            if !pre_lifecycle_keys.contains(k) {
                provided_vars.insert(k.clone(), v.clone());
            }
        }

        creation_ctx = Some(ctx);
    }

    // 10. Collect schema variables
    let prompt_options = PromptOptions { batch_mode: args.batch };

    let collected = if let Some(ref typedef) = lua_typedef {
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
        CollectedVars {
            values: provided_vars.clone(),
            prompted: Vec::new(),
            defaulted: Vec::new(),
        }
    };

    // 11. Build render context + merge collected vars
    debug!("Collected variables: {:?}", collected.values);
    let mut render_ctx = if let Some(ref loaded) = loaded_template {
        let info = TemplateInfo {
            logical_name: loaded.logical_name.clone(),
            path: loaded.path.clone(),
        };
        build_minimal_context(cfg, &info)
    } else {
        // No template — build a basic context with date/time defaults
        let mut ctx = HashMap::new();
        let now = chrono::Local::now();
        ctx.insert("date".into(), now.format("%Y-%m-%d").to_string());
        ctx.insert("time".into(), now.format("%H:%M").to_string());
        ctx.insert("datetime".into(), now.to_rfc3339());
        ctx.insert("today".into(), now.format("%Y-%m-%d").to_string());
        ctx.insert("now".into(), now.to_rfc3339());
        ctx.insert("vault_root".into(), cfg.vault_root.to_string_lossy().to_string());
        ctx.insert(
            "templates_dir".into(),
            cfg.templates_dir.to_string_lossy().to_string(),
        );
        ctx
    };
    for (k, v) in &collected.values {
        render_ctx.insert(k.clone(), v.clone());
    }

    // 12–14. If behaviour: merge collected vars into CreationContext, call before_create
    let mut ref_date = None;
    if let Some(ref mut ctx) = creation_ctx {
        // 12. Merge collected vars into CreationContext
        for (k, v) in &collected.values {
            ctx.set_var(k, v);
        }

        // 13. Call before_create (ID gen, date eval, counters)
        if let Some(ref nt) = note_type {
            let behavior = nt.behavior();
            if let Err(e) = behavior.before_create(ctx) {
                eprintln!("FAIL mdv new");
                eprintln!("{e}");
                std::process::exit(1);
            }
        }

        ref_date = ctx.reference_date;

        // 14. Merge CreationContext vars + core_metadata into render_ctx
        merge_context_to_render_vars(ctx, &mut render_ctx);
    }

    // 15. Resolve output path: CLI > template FM > behaviour > Lua > error
    let output_path = if let Some(ref out) = args.output {
        out.clone()
    } else if let Some(ref loaded) = loaded_template {
        match resolve_template_output_path(loaded, cfg, &render_ctx) {
            Ok(Some(path)) => path,
            Ok(None) => {
                // Try behaviour output_path
                if let (Some(ref nt), Some(ref ctx)) = (&note_type, &creation_ctx) {
                    match nt.behavior().output_path(ctx) {
                        Ok(path) => path,
                        Err(_) => {
                            resolve_lua_output_or_exit(&lua_typedef, cfg, &render_ctx)
                        }
                    }
                } else {
                    resolve_lua_output_or_exit(&lua_typedef, cfg, &render_ctx)
                }
            }
            Err(e) => {
                eprintln!("Failed to resolve output path: {e}");
                std::process::exit(1);
            }
        }
    } else {
        // No template — try behaviour output_path, then Lua
        if let (Some(ref nt), Some(ref ctx)) = (&note_type, &creation_ctx) {
            match nt.behavior().output_path(ctx) {
                Ok(path) => path,
                Err(_) => resolve_lua_output_or_exit(&lua_typedef, cfg, &render_ctx),
            }
        } else {
            resolve_lua_output_or_exit(&lua_typedef, cfg, &render_ctx)
        }
    };

    // 16. Set ctx.output_path + update render context with output info
    if let Some(ref mut ctx) = creation_ctx {
        ctx.output_path = Some(output_path.clone());
    }

    let output_abs = if output_path.is_absolute() {
        output_path.clone()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(&output_path)
    };
    render_ctx
        .insert("output_path".to_string(), output_abs.to_string_lossy().to_string());
    if let Some(name) = output_abs.file_name().and_then(|s| s.to_str()) {
        render_ctx.insert("output_filename".to_string(), name.to_string());
    }
    if let Some(parent) = output_abs.parent() {
        render_ctx.insert("output_dir".to_string(), parent.to_string_lossy().to_string());
    }

    if output_path.exists() {
        eprintln!(
            "Refusing to overwrite existing file: {} (add --force later if needed)",
            output_path.display()
        );
        std::process::exit(1);
    }

    // 17. Generate content: template rendering or scaffolding
    let mut rendered = if let Some(ref loaded) = loaded_template {
        debug!("Render context: {:?}", render_ctx);
        match render_with_ref_date(loaded, &render_ctx, ref_date) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to render template: {e}");
                std::process::exit(1);
            }
        }
    } else {
        // No template — generate scaffolding content
        let title_for_scaffolding = creation_ctx
            .as_ref()
            .and_then(|ctx| ctx.core_metadata.title.as_ref())
            .unwrap_or(&title);
        mdvault_core::types::scaffolding::generate_scaffolding(
            effective_name,
            lua_typedef.as_ref(),
            title_for_scaffolding,
            &render_ctx,
        )
    };

    // 18. Apply core_metadata (before write — protects generated fields)
    if let Some(ref ctx) = creation_ctx {
        let order = lua_typedef.as_ref().and_then(|td| td.frontmatter_order.as_deref());
        match ctx.core_metadata.apply_to_content(&rendered, order) {
            Ok(fixed) => rendered = fixed,
            Err(e) => {
                eprintln!("Warning: failed to apply core metadata: {e}");
            }
        }
    }

    // 19. Validate before write
    if let Some(ref typedef) = lua_typedef {
        if let Some(ref registry) = type_registry {
            let note_type_name =
                extract_note_type(&rendered).unwrap_or_else(|| typedef.name.clone());

            match validate_before_write(
                registry,
                &note_type_name,
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

    // 20. Create dirs + write file
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

    // 21. Post-write pipeline (hooks, core_metadata protection, after_create, reindex, activity)
    post_write_pipeline(
        cfg,
        &output_path,
        &rendered,
        note_type.as_ref(),
        creation_ctx.as_ref(),
        lua_typedef.as_ref(),
        loaded_template.as_ref(),
        &mut render_ctx,
        ref_date,
        effective_name,
    );

    // 22. Print success
    println!("OK   mdv new");
    println!("type: {}", effective_name);
    if let Some(ref ctx) = creation_ctx {
        if let Some(ref id) = ctx
            .core_metadata
            .task_id
            .as_ref()
            .or(ctx.core_metadata.project_id.as_ref())
            .or(ctx.core_metadata.meeting_id.as_ref())
        {
            println!("id:   {}", id);
        }
    }
    println!("output: {}", output_path.display());
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
    let typedef_repo = match &cfg.typedefs_fallback_dir {
        Some(fallback) => TypedefRepository::with_fallback(&cfg.typedefs_dir, fallback),
        None => TypedefRepository::new(&cfg.typedefs_dir),
    }
    .map_err(|e| e.to_string())?;
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
    use mdvault_core::domain::CoreMetadata;
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
    fn test_apply_core_metadata() {
        let content = "---\nexisting: val\n---\nbody";
        let core = CoreMetadata {
            note_type: Some("task".into()),
            task_id: Some("TST-001".into()),
            ..Default::default()
        };

        let result = core.apply_to_content(content, None).unwrap();

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
