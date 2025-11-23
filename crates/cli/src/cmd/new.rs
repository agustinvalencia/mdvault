use markadd_core::config::loader::{ConfigLoader, default_config_path};
use markadd_core::templates::discovery::TemplateInfo;
use markadd_core::templates::engine::{build_render_context, render};
use markadd_core::templates::repository::{TemplateRepoError, TemplateRepository};
use std::fs;
use std::path::Path;

pub fn run(
    config: Option<&Path>,
    profile: Option<&str>,
    template_name: &str,
    output: &Path,
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

    if output.exists() {
        eprintln!(
            "Refusing to overwrite existing file: {} (add --force later if needed)",
            output.display()
        );
        std::process::exit(1);
    }

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

    // We need a TemplateInfo to build context: reconstruct from LoadedTemplate.
    let info = TemplateInfo {
        logical_name: loaded.logical_name.clone(),
        path: loaded.path.clone(),
    };

    let ctx = build_render_context(&cfg, &info, output);

    let rendered = match render(&loaded, ctx) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to render template: {e}");
            std::process::exit(1);
        }
    };

    if let Some(parent) = output.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        eprintln!("Failed to create parent directory {}: {e}", parent.display());
        std::process::exit(1);
    }

    if let Err(e) = fs::write(output, rendered) {
        eprintln!("Failed to write output file {}: {e}", output.display());
        std::process::exit(1);
    }

    println!("OK   markadd new");
    println!("template: {}", template_name);
    println!("output:   {}", output.display());
}
