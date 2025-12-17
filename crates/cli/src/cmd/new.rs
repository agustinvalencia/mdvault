use markadd_core::config::loader::{default_config_path, ConfigLoader};
use markadd_core::templates::discovery::TemplateInfo;
use markadd_core::templates::engine::{
    build_minimal_context, build_render_context, render, resolve_template_output_path,
};
use markadd_core::templates::repository::{TemplateRepoError, TemplateRepository};
use std::fs;
use std::path::Path;

pub fn run(
    config: Option<&Path>,
    profile: Option<&str>,
    template_name: &str,
    output: Option<&Path>,
) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            println!("FAIL markadd new");
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
            println!("FAIL markadd new");
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

    // Resolve output path: CLI arg takes precedence, then frontmatter
    let output_path = if let Some(out) = output {
        out.to_path_buf()
    } else {
        // Try to get from template frontmatter
        let minimal_ctx = build_minimal_context(&cfg, &info);
        match resolve_template_output_path(&loaded, &cfg, &minimal_ctx) {
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

    if output_path.exists() {
        eprintln!(
            "Refusing to overwrite existing file: {} (add --force later if needed)",
            output_path.display()
        );
        std::process::exit(1);
    }

    let ctx = build_render_context(&cfg, &info, &output_path);

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

    println!("OK   markadd new");
    println!("template: {}", template_name);
    println!("output:   {}", output_path.display());
}
