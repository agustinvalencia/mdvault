use crate::prompt::{collect_variables, PromptOptions};
use mdvault_core::config::loader::{default_config_path, ConfigLoader};
use mdvault_core::templates::discovery::TemplateInfo;
use mdvault_core::templates::engine::{
    build_minimal_context, render, resolve_template_output_path,
};
use mdvault_core::templates::repository::{TemplateRepoError, TemplateRepository};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn run(
    config: Option<&Path>,
    profile: Option<&str>,
    template_name: &str,
    output: Option<&Path>,
    vars: &[(String, String)],
    batch: bool,
) {
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
    let provided_vars: HashMap<String, String> = vars.iter().cloned().collect();

    // Build minimal context for variable resolution
    let minimal_ctx = build_minimal_context(&cfg, &info);

    // Collect variables (prompt for missing ones if interactive)
    let vars_map = loaded.frontmatter.as_ref().and_then(|fm| fm.vars.as_ref());
    let prompt_options = PromptOptions { batch_mode: batch };

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
    let output_path = if let Some(out) = output {
        out.to_path_buf()
    } else {
        // Try to get from template frontmatter
        match resolve_template_output_path(&loaded, &cfg, &ctx) {
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

    if let Err(e) = fs::write(&output_path, rendered) {
        eprintln!("Failed to write output file {}: {e}", output_path.display());
        std::process::exit(1);
    }

    println!("OK   mdv new");
    println!("template: {}", template_name);
    println!("output:   {}", output_path.display());
}
