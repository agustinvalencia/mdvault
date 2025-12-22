use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::prompt::{collect_variables, PromptOptions};
use mdvault_core::captures::{CaptureRepoError, CaptureRepository, CaptureSpec};
use mdvault_core::config::loader::{default_config_path, ConfigLoader};
use mdvault_core::config::types::ResolvedConfig;
use mdvault_core::frontmatter::{apply_ops, parse, serialize};
use mdvault_core::markdown_ast::{MarkdownAstError, MarkdownEditor, SectionMatch};
use mdvault_core::templates::engine::render_string as engine_render_string;

use chrono::Local;
use regex::Regex;

/// Built-in variables that are automatically provided
const BUILTIN_VARS: &[&str] = &[
    "date",
    "time",
    "datetime",
    "vault_root",
    "templates_dir",
    "captures_dir",
    "macros_dir",
];

pub fn run_list(config: Option<&Path>, profile: Option<&str>) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("FAIL mdv capture --list");
            eprintln!("{e}");
            if config.is_none() {
                eprintln!("looked for: {}", default_config_path().display());
            }
            std::process::exit(1);
        }
    };

    let repo = match CaptureRepository::new(&cfg.captures_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("FAIL mdv capture --list");
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let captures = repo.list_all();
    if captures.is_empty() {
        println!("(no captures found)");
        return;
    }

    for info in captures {
        // Try to load the capture to get its variables
        match repo.get_by_name(&info.logical_name) {
            Ok(loaded) => {
                let user_vars = extract_user_variables(&loaded.spec);
                if user_vars.is_empty() {
                    println!("{}", info.logical_name);
                } else {
                    let vars_str = user_vars.join(", ");
                    println!("{}  [{}]", info.logical_name, vars_str);
                }
            }
            Err(_) => {
                // If we can't load it, just show the name
                println!("{}  (error loading)", info.logical_name);
            }
        }
    }
    println!("-- {} captures --", captures.len());
}

/// Extract user-defined variables from a capture spec (excludes built-ins)
fn extract_user_variables(spec: &CaptureSpec) -> Vec<String> {
    let re = Regex::new(r"\{\{([a-zA-Z0-9_]+)\}\}").unwrap();
    let builtin: HashSet<&str> = BUILTIN_VARS.iter().copied().collect();

    let mut vars = HashSet::new();

    // Extract from content (if present)
    if let Some(content) = &spec.content {
        for cap in re.captures_iter(content) {
            let var = cap.get(1).unwrap().as_str();
            if !builtin.contains(var) {
                vars.insert(var.to_string());
            }
        }
    }

    // Extract from target file path
    for cap in re.captures_iter(&spec.target.file) {
        let var = cap.get(1).unwrap().as_str();
        if !builtin.contains(var) {
            vars.insert(var.to_string());
        }
    }

    // Extract from section (if present)
    if let Some(section) = &spec.target.section {
        for cap in re.captures_iter(section) {
            let var = cap.get(1).unwrap().as_str();
            if !builtin.contains(var) {
                vars.insert(var.to_string());
            }
        }
    }

    let mut sorted: Vec<_> = vars.into_iter().collect();
    sorted.sort();
    sorted
}

pub fn run(
    config: Option<&Path>,
    profile: Option<&str>,
    capture_name: &str,
    vars: &[(String, String)],
    batch: bool,
) {
    // 1. Load config
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("FAIL mdv capture");
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
            eprintln!("FAIL mdv capture");
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
    let base_ctx = build_capture_context(&cfg);

    // Convert provided vars to HashMap
    let provided_vars: HashMap<String, String> = vars.iter().cloned().collect();

    // Build content string for variable extraction (combine all templated fields)
    let mut content_for_vars = String::new();
    if let Some(content) = &loaded.spec.content {
        content_for_vars.push_str(content);
    }
    content_for_vars.push_str(&loaded.spec.target.file);
    if let Some(section) = &loaded.spec.target.section {
        content_for_vars.push_str(section);
    }

    // Collect variables (prompt for missing ones if interactive)
    let vars_map = loaded.spec.vars.as_ref();
    let prompt_options = PromptOptions { batch_mode: batch };

    let collected = match collect_variables(
        vars_map,
        &content_for_vars,
        &provided_vars,
        &base_ctx,
        &prompt_options,
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    // Merge collected variables into context
    let mut ctx = base_ctx;
    for (k, v) in collected.values {
        ctx.insert(k, v);
    }

    // 5. Render target file path
    let target_file_raw = render_string(&loaded.spec.target.file, &ctx);
    let target_file = resolve_target_path(&cfg.vault_root, &target_file_raw);

    // 6. Read existing file
    let existing_content = match fs::read_to_string(&target_file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Failed to read target file {}: {e}", target_file.display());
            eprintln!("Hint: The target file must exist before capturing to it.");
            std::process::exit(1);
        }
    };

    // 7. Execute capture (frontmatter + content insertion)
    let (result_content, section_info) =
        match execute_capture_operations(&existing_content, &loaded.spec, &ctx) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        };

    // 8. Write back to file
    if let Err(e) = fs::write(&target_file, &result_content) {
        eprintln!("Failed to write to {}: {e}", target_file.display());
        std::process::exit(1);
    }

    println!("OK   mdv capture");
    println!("capture: {}", capture_name);
    println!("target:  {}", target_file.display());
    if let Some((title, level)) = section_info {
        println!("section: {} (level {})", title, level);
    }
    if loaded.spec.frontmatter.is_some() {
        println!("frontmatter: modified");
    }
}

/// Execute capture operations: frontmatter modification and/or content insertion.
/// Returns the modified content and optional section info (title, level).
fn execute_capture_operations(
    existing_content: &str,
    spec: &CaptureSpec,
    ctx: &HashMap<String, String>,
) -> Result<(String, Option<(String, u8)>), String> {
    // Parse frontmatter from existing content first
    let mut parsed = parse(existing_content)
        .map_err(|e| format!("Failed to parse frontmatter: {e}"))?;
    let mut section_info = None;

    // Apply frontmatter operations if specified
    if let Some(fm_ops) = &spec.frontmatter {
        parsed = apply_ops(parsed, fm_ops, ctx)
            .map_err(|e| format!("Failed to apply frontmatter ops: {e}"))?;
    }

    // Insert content if specified - operate on body only to preserve frontmatter
    if let Some(content_template) = &spec.content {
        let section = spec.target.section.as_ref().ok_or_else(|| {
            "Capture has content but no target section specified".to_string()
        })?;

        let rendered_content = render_string(content_template, ctx);
        let section_match = SectionMatch::new(section);
        let position = spec.target.position.clone().into();

        let result = MarkdownEditor::insert_into_section(
            &parsed.body,
            &section_match,
            &rendered_content,
            position,
        )
        .map_err(|e| match &e {
            MarkdownAstError::SectionNotFound(s) => {
                let headings = MarkdownEditor::find_headings(&parsed.body);
                let mut msg = format!("Section not found: '{s}'\nAvailable sections:\n");
                for h in headings {
                    msg.push_str(&format!("  - {} (level {})\n", h.title, h.level));
                }
                msg
            }
            MarkdownAstError::EmptyDocument => "Target file is empty".to_string(),
            MarkdownAstError::RenderError(msg) => format!("Markdown render error: {msg}"),
        })?;

        section_info = Some((result.matched_heading.title, result.matched_heading.level));
        parsed.body = result.content;
    }

    // Serialize the document (frontmatter + body)
    let final_content = serialize(&parsed);
    Ok((final_content, section_info))
}

fn build_capture_context(cfg: &ResolvedConfig) -> HashMap<String, String> {
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
    // Use the engine's render_string which supports date math expressions
    engine_render_string(template, ctx).unwrap_or_else(|_| template.to_string())
}

fn resolve_target_path(vault_root: &Path, target: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(target);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        vault_root.join(path)
    }
}
