use std::collections::HashMap;
use std::fs;
use std::path::Path;

use markadd_core::captures::{CaptureRepoError, CaptureRepository};
use markadd_core::config::loader::{ConfigLoader, default_config_path};
use markadd_core::markdown_ast::{MarkdownAstError, MarkdownEditor, SectionMatch};

use chrono::Local;
use regex::Regex;

pub fn run(
    config: Option<&Path>,
    profile: Option<&str>,
    capture_name: &str,
    vars: &[(String, String)],
) {
    // 1. Load config
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("FAIL markadd capture");
            eprintln!("{e}");
            if config.is_none() {
                eprintln!("looked for: {}", default_config_path().display());
            }
            std::process::exit(1);
        }
    };

    // 2. Load capture repository
    let repo = match CaptureRepository::new(&cfg.captures_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("FAIL markadd capture");
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    // 3. Get capture spec
    let loaded = match repo.get_by_name(capture_name) {
        Ok(c) => c,
        Err(e) => match e {
            CaptureRepoError::NotFound(name) => {
                eprintln!("Capture not found: {name}");
                eprintln!("Available captures:");
                for c in repo.list_all() {
                    eprintln!("  - {}", c.logical_name);
                }
                std::process::exit(1);
            }
            other => {
                eprintln!("Failed to load capture: {other}");
                std::process::exit(1);
            }
        },
    };

    // 4. Build render context
    let mut ctx = build_capture_context(&cfg);

    // Add user-provided variables
    for (key, value) in vars {
        ctx.insert(key.clone(), value.clone());
    }

    // 5. Render target file path
    let target_file_raw = render_string(&loaded.spec.target.file, &ctx);
    let target_file = resolve_target_path(&cfg.vault_root, &target_file_raw);

    // 6. Render content
    let rendered_content = render_string(&loaded.spec.content, &ctx);

    // 7. Read existing file
    let existing_content = match fs::read_to_string(&target_file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Failed to read target file {}: {e}", target_file.display());
            eprintln!("Hint: The target file must exist before capturing to it.");
            std::process::exit(1);
        }
    };

    // 8. Insert content using MarkdownEditor
    let section = SectionMatch::new(&loaded.spec.target.section);
    let position = loaded.spec.target.position.clone().into();

    let result = match MarkdownEditor::insert_into_section(
        &existing_content,
        &section,
        &rendered_content,
        position,
    ) {
        Ok(r) => r,
        Err(e) => {
            match &e {
                MarkdownAstError::SectionNotFound(s) => {
                    eprintln!("Section not found: '{s}'");
                    eprintln!("Available sections in {}:", target_file.display());
                    for h in MarkdownEditor::find_headings(&existing_content) {
                        eprintln!("  - {} (level {})", h.title, h.level);
                    }
                }
                MarkdownAstError::EmptyDocument => {
                    eprintln!("Target file is empty: {}", target_file.display());
                }
                MarkdownAstError::RenderError(msg) => {
                    eprintln!("Markdown render error: {msg}");
                }
            }
            std::process::exit(1);
        }
    };

    // 9. Write back to file
    if let Err(e) = fs::write(&target_file, &result.content) {
        eprintln!("Failed to write to {}: {e}", target_file.display());
        std::process::exit(1);
    }

    println!("OK   markadd capture");
    println!("capture: {}", capture_name);
    println!("target:  {}", target_file.display());
    println!(
        "section: {} (level {})",
        result.matched_heading.title, result.matched_heading.level
    );
}

fn build_capture_context(
    cfg: &markadd_core::config::types::ResolvedConfig,
) -> HashMap<String, String> {
    let mut ctx = HashMap::new();

    // Date/time
    let now = Local::now();
    ctx.insert("date".into(), now.format("%Y-%m-%d").to_string());
    ctx.insert("time".into(), now.format("%H:%M").to_string());
    ctx.insert("datetime".into(), now.to_rfc3339());

    // Config paths
    ctx.insert("vault_root".into(), cfg.vault_root.to_string_lossy().to_string());
    ctx.insert("templates_dir".into(), cfg.templates_dir.to_string_lossy().to_string());
    ctx.insert("captures_dir".into(), cfg.captures_dir.to_string_lossy().to_string());
    ctx.insert("macros_dir".into(), cfg.macros_dir.to_string_lossy().to_string());

    ctx
}

fn render_string(template: &str, ctx: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\{\{([a-zA-Z0-9_]+)\}\}").unwrap();
    re.replace_all(template, |caps: &regex::Captures<'_>| {
        let key = &caps[1];
        ctx.get(key).cloned().unwrap_or_else(|| caps[0].to_string())
    })
    .into_owned()
}

fn resolve_target_path(vault_root: &Path, target: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(target);
    if path.is_absolute() { path.to_path_buf() } else { vault_root.join(path) }
}
