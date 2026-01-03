use crate::prompt::{collect_variables, prompt_for_field, PromptOptions};
use crate::NewArgs;
use mdvault_core::captures::CaptureRepository;
use mdvault_core::config::loader::{default_config_path, ConfigLoader};
use mdvault_core::config::types::ResolvedConfig;
use mdvault_core::frontmatter::parse as parse_frontmatter;
use mdvault_core::macros::MacroRepository;
use mdvault_core::scripting::{
    run_on_create_hook, HookResult, NoteContext, VaultContext,
};
use mdvault_core::templates::discovery::TemplateInfo;
use mdvault_core::templates::engine::{
    build_minimal_context, render, resolve_template_output_path,
};
use mdvault_core::templates::repository::{TemplateRepoError, TemplateRepository};
use mdvault_core::types::{
    default_output_path, generate_scaffolding, get_missing_required_fields, TypeRegistry,
    TypedefRepository,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn run(config: Option<&Path>, profile: Option<&str>, args: NewArgs) {
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

    // Convert provided vars to HashMap
    let mut provided_vars: HashMap<String, String> = args.vars.iter().cloned().collect();

    // If title was provided as positional arg, add it to vars
    if let Some(ref title) = args.title {
        provided_vars.entry("title".to_string()).or_insert(title.clone());
    }

    // Build minimal context for variable resolution
    let minimal_ctx = build_minimal_context(cfg, &info);

    // Collect variables (prompt for missing ones if interactive)
    let vars_map = loaded.frontmatter.as_ref().and_then(|fm| fm.vars.as_ref());
    let prompt_options = PromptOptions { batch_mode: args.batch };

    let collected = match collect_variables(
        vars_map,
        &loaded.body,
        &provided_vars,
        &minimal_ctx,
        &prompt_options,
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    // Merge collected variables into context
    let mut ctx = minimal_ctx;
    for (k, v) in collected.values {
        ctx.insert(k, v);
    }

    // Resolve output path: CLI arg takes precedence, then frontmatter
    let output_path = if let Some(ref out) = args.output {
        out.clone()
    } else {
        // Try to get from template frontmatter
        match resolve_template_output_path(&loaded, cfg, &ctx) {
            Ok(Some(path)) => path,
            Ok(None) => {
                eprintln!(
                    "Error: --output is required (template has no output in frontmatter)"
                );
                std::process::exit(1);
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

    let rendered = match render(&loaded, &ctx) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to render template: {e}");
            std::process::exit(1);
        }
    };

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
    match run_on_create_hook_if_exists(cfg, &output_path, &rendered) {
        Ok(hook_result) => {
            if hook_result.modified {
                if let Err(e) =
                    apply_hook_modifications(&output_path, &rendered, &hook_result)
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
    let has_template = template_repo
        .as_ref()
        .and_then(|repo| repo.get_by_name(type_name).ok())
        .is_some();

    // If there's a matching template, use template mode instead
    if has_template {
        run_template_mode(cfg, type_name, args);
        return;
    }

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

                // For enums, show available values
                let enum_hint = schema.enum_values.as_ref().map(|v| v.join("/"));

                match prompt_for_field(field, &prompt, enum_hint.as_deref(), true) {
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
    }

    // Generate output path
    let output_path = if let Some(ref out) = args.output {
        out.clone()
    } else {
        let rel_path = default_output_path(type_name, &title);
        cfg.vault_root.join(rel_path)
    };

    if output_path.exists() {
        eprintln!("Refusing to overwrite existing file: {}", output_path.display());
        std::process::exit(1);
    }

    // Generate scaffolding content
    let content = generate_scaffolding(type_name, typedef.as_deref(), &title, &vars);

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
    match run_on_create_hook_if_exists(cfg, &output_path, &content) {
        Ok(hook_result) => {
            if hook_result.modified {
                if let Err(e) =
                    apply_hook_modifications(&output_path, &content, &hook_result)
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

    println!("OK   mdv new");
    println!("type:   {}", type_name);
    println!("output: {}", output_path.display());
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
) -> Result<HookResult, String> {
    // Extract note type from frontmatter
    let note_type = match extract_note_type(content) {
        Some(t) => t,
        None => {
            return Ok(HookResult { modified: false, frontmatter: None, content: None })
        }
    };

    // Load type registry
    let typedef_repo =
        TypedefRepository::new(&cfg.typedefs_dir).map_err(|e| e.to_string())?;
    let type_registry =
        TypeRegistry::from_repository(&typedef_repo).map_err(|e| e.to_string())?;

    // Check if type has on_create hook
    let typedef = match type_registry.get(&note_type) {
        Some(td) if td.has_on_create_hook => td,
        _ => return Ok(HookResult { modified: false, frontmatter: None, content: None }),
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

    // Build NoteContext
    let note_ctx = NoteContext::new(
        output_path.to_path_buf(),
        note_type,
        frontmatter,
        content.to_string(),
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
