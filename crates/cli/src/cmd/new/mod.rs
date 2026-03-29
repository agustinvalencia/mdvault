mod discovery;
mod hooks;
mod prompts;
mod writer;

use super::common::load_config;
use crate::prompt::{CollectedVars, PromptOptions};
use crate::NewArgs;
use mdvault_core::activity::ActivityLogService;
use mdvault_core::config::types::ResolvedConfig;
use mdvault_core::context::ContextManager;
use mdvault_core::domain::{CreationContext, NoteType as DomainNoteType};
use mdvault_core::templates::discovery::TemplateInfo;
use mdvault_core::templates::engine::{build_minimal_context, render_with_ref_date};
use mdvault_core::templates::repository::TemplateRepository;
use mdvault_core::types::{TypeRegistry, TypedefRepository};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

pub fn run(config: Option<&Path>, profile: Option<&str>, args: NewArgs) {
    debug!("Running create new");
    let cfg = load_config(config, profile);

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

/// Merge CreationContext vars and core_metadata fields into a template render HashMap.
fn merge_context_to_render_vars(
    ctx: &CreationContext,
    render_ctx: &mut HashMap<String, String>,
) {
    for (k, v) in &ctx.vars {
        render_ctx.insert(k.clone(), v.clone());
    }
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

/// Unified creation flow — handles both template-based and scaffolding-based note creation.
fn run_unified(cfg: &ResolvedConfig, effective_name: &str, args: &NewArgs) {
    // 1. Load TypedefRepository + TypeRegistry
    let typedef_repo = match &cfg.typedefs_fallback_dir {
        Some(fallback) => {
            TypedefRepository::with_fallback(&cfg.typedefs_dir, fallback).ok()
        }
        None => TypedefRepository::new(&cfg.typedefs_dir).ok(),
    };
    let type_registry =
        typedef_repo.as_ref().and_then(|repo| TypeRegistry::from_repository(repo).ok());

    // 2. Try load template
    let template_repo = TemplateRepository::new(&cfg.templates_dir).ok();
    let loaded_template =
        template_repo.as_ref().and_then(|repo| repo.get_by_name(effective_name).ok());

    // 3. Load Lua typedef
    let lua_typedef = discovery::resolve_lua_typedef(
        loaded_template.as_ref(),
        type_registry.as_ref(),
        cfg,
        effective_name,
    );

    // 4. Try NoteType::try_from_name
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
    let needs_title = loaded_template.is_none();

    let title = if let Some(ref t) = args.title {
        t.clone()
    } else if let Some(ref t) = args.note_type {
        if args.template.is_some() {
            t.clone()
        } else {
            prompts::resolve_title_or_default(
                effective_name,
                needs_title,
                args.batch,
                &type_registry,
            )
        }
    } else {
        prompts::resolve_title_or_default(
            effective_name,
            needs_title,
            args.batch,
            &type_registry,
        )
    };
    if !title.is_empty() {
        provided_vars.entry("title".to_string()).or_insert(title.clone());
    }

    // 7–9. If behaviour: build CreationContext, inject focus, type prompts, sync vars
    let mut creation_ctx: Option<CreationContext> = None;
    if let (Some(ref mut _nt), Some(ref registry)) = (&mut note_type, &type_registry) {
        let ctx_title =
            provided_vars.get("title").cloned().unwrap_or_else(|| title.clone());
        let mut ctx = CreationContext::new(effective_name, &ctx_title, cfg, registry)
            .with_vars(provided_vars.clone())
            .with_batch_mode(args.batch);

        let pre_lifecycle_keys: std::collections::HashSet<String> =
            ctx.vars.keys().cloned().collect();

        inject_focus_context(cfg, &mut ctx.vars);

        let behavior = _nt.behavior();
        let type_prompts = behavior.type_prompts(&ctx.to_prompt_context());
        prompts::dispatch_type_prompts(type_prompts, &mut ctx.vars, cfg, args.batch);

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
        match prompts::collect_schema_variables(
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

    // 11. Build render context
    debug!("Collected variables: {:?}", collected.values);
    let mut render_ctx = if let Some(ref loaded) = loaded_template {
        let info = TemplateInfo {
            logical_name: loaded.logical_name.clone(),
            path: loaded.path.clone(),
        };
        build_minimal_context(cfg, &info)
    } else {
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

    // 12–14. Merge collected vars into CreationContext, call before_create
    let mut ref_date = None;
    if let Some(ref mut ctx) = creation_ctx {
        for (k, v) in &collected.values {
            ctx.set_var(k, v);
        }
        if let Some(ref nt) = note_type {
            let behavior = nt.behavior();
            if let Err(e) = behavior.before_create(ctx) {
                eprintln!("FAIL mdv new");
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        ref_date = ctx.reference_date;
        merge_context_to_render_vars(ctx, &mut render_ctx);
    }

    // 15. Resolve output path
    let output_path = discovery::resolve_output_path(
        args.output.as_ref(),
        loaded_template.as_ref(),
        note_type.as_ref(),
        creation_ctx.as_ref(),
        &lua_typedef,
        cfg,
        &render_ctx,
    );

    // 16. Set ctx.output_path + update render context
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

    // 17. Generate content
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

    // 18. Apply core_metadata
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
            let note_type_name = discovery::extract_note_type(&rendered)
                .unwrap_or_else(|| typedef.name.clone());

            match writer::validate_before_write(
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

    // 21. Post-write pipeline
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

/// Post-write pipeline: hook execution, core_metadata protection, after_create, reindex, activity logging.
#[allow(clippy::too_many_arguments)]
fn post_write_pipeline(
    cfg: &ResolvedConfig,
    output_path: &Path,
    rendered: &str,
    note_type: Option<&DomainNoteType>,
    creation_ctx: Option<&CreationContext>,
    lua_typedef: Option<&mdvault_core::types::TypeDefinition>,
    loaded_template: Option<&mdvault_core::templates::repository::LoadedTemplate>,
    render_ctx: &mut HashMap<String, String>,
    ref_date: Option<chrono::NaiveDate>,
    type_name: &str,
) {
    match hooks::run_on_create_hook_if_exists(
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

                if let Err(e) = hooks::apply_hook_modifications(
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

    // Re-apply core_metadata after hooks
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

    // Call after_create after hooks
    if let (Some(nt), Some(ctx)) = (note_type, creation_ctx) {
        let current = std::fs::read_to_string(output_path).unwrap_or_default();
        if let Err(e) = nt.behavior().after_create(ctx, &current) {
            eprintln!("Warning: after_create failed: {e}");
        }
    }

    writer::reindex_vault(cfg);

    // Activity logging
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

#[cfg(test)]
mod tests {
    use mdvault_core::domain::CoreMetadata;
    use mdvault_core::frontmatter::parse as parse_frontmatter;

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
}
