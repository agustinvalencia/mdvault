//! Macro command implementation.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::prompt::{collect_variables, PromptOptions};
use mdvault_core::captures::CaptureRepository;
use mdvault_core::config::loader::{default_config_path, ConfigLoader};
use mdvault_core::config::types::ResolvedConfig;
use mdvault_core::frontmatter::{apply_ops, parse, serialize};
use mdvault_core::macros::{
    get_shell_commands, requires_trust, run_macro, CaptureStep, MacroRepoError,
    MacroRepository, MacroRunError, MacroSpec, RunContext, RunOptions, ShellStep,
    StepExecutor, StepResult, TemplateStep,
};
use mdvault_core::markdown_ast::{MarkdownEditor, SectionMatch};
use mdvault_core::templates::discovery::TemplateInfo;
use mdvault_core::templates::engine::{
    build_minimal_context, render_string, resolve_template_output_path,
};
use mdvault_core::templates::repository::TemplateRepository;

use chrono::Local;

/// List available macros.
pub fn run_list(config: Option<&Path>, profile: Option<&str>) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("FAIL mdv macro --list");
            eprintln!("{e}");
            if config.is_none() {
                eprintln!("looked for: {}", default_config_path().display());
            }
            std::process::exit(1);
        }
    };

    let repo = match MacroRepository::new(&cfg.macros_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("FAIL mdv macro --list");
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let macros = repo.list_all();
    if macros.is_empty() {
        println!("(no macros found)");
        return;
    }

    for info in macros {
        match repo.get_by_name(&info.logical_name) {
            Ok(loaded) => {
                let trust_marker =
                    if requires_trust(&loaded.spec) { " [requires --trust]" } else { "" };
                let desc = if loaded.spec.description.is_empty() {
                    String::new()
                } else {
                    format!(" - {}", loaded.spec.description)
                };
                println!(
                    "{}  ({} steps){trust_marker}{desc}",
                    info.logical_name,
                    loaded.spec.steps.len()
                );
            }
            Err(_) => {
                println!("{}  (error loading)", info.logical_name);
            }
        }
    }
    println!("-- {} macros --", macros.len());
}

/// Run a macro.
pub fn run(
    config: Option<&Path>,
    profile: Option<&str>,
    macro_name: &str,
    vars: &[(String, String)],
    batch: bool,
    trust: bool,
) {
    // 1. Load config
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("FAIL mdv macro");
            eprintln!("{e}");
            if config.is_none() {
                eprintln!("looked for: {}", default_config_path().display());
            }
            std::process::exit(1);
        }
    };

    // 2. Load macro repository
    let repo = match MacroRepository::new(&cfg.macros_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("FAIL mdv macro");
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    // 3. Get macro spec
    let loaded = match repo.get_by_name(macro_name) {
        Ok(m) => m,
        Err(e) => match e {
            MacroRepoError::NotFound(name) => {
                eprintln!("Macro not found: {name}");
                eprintln!("Available macros:");
                for m in repo.list_all() {
                    eprintln!("  - {}", m.logical_name);
                }
                std::process::exit(1);
            }
            other => {
                eprintln!("Failed to load macro: {other}");
                std::process::exit(1);
            }
        },
    };

    // 4. Check trust requirements
    if requires_trust(&loaded.spec) && !trust {
        eprintln!(
            "Error: This macro contains shell commands that require the --trust flag."
        );
        eprintln!("Shell commands:");
        for cmd in get_shell_commands(&loaded.spec) {
            eprintln!("  $ {cmd}");
        }
        eprintln!("\nRun with --trust to allow shell execution.");
        std::process::exit(1);
    }

    // 5. Build base context
    let base_ctx = build_macro_context(&cfg);

    // Convert provided vars to HashMap
    let provided_vars: HashMap<String, String> = vars.iter().cloned().collect();

    // Build content string for variable extraction from macro vars
    let content_for_vars = build_vars_content(&loaded.spec);

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
    let mut ctx_vars = base_ctx;
    for (k, v) in collected.values {
        ctx_vars.insert(k, v);
    }

    // 6. Create executor with loaded repositories
    let template_repo = match TemplateRepository::new(&cfg.templates_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to load templates: {e}");
            std::process::exit(1);
        }
    };

    let capture_repo = match CaptureRepository::new(&cfg.captures_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to load captures: {e}");
            std::process::exit(1);
        }
    };

    let executor = CliStepExecutor { config: cfg.clone(), template_repo, capture_repo };

    // 7. Create run context and options
    let run_options = RunOptions {
        trust,
        allow_shell: cfg.security.allow_shell || trust,
        dry_run: false,
    };

    let run_ctx = RunContext::new(ctx_vars, run_options);

    // 8. Run the macro
    let result = run_macro(&loaded, &executor, run_ctx);

    // 9. Print results
    if result.success {
        println!("OK   mdv macro");
        println!("macro: {}", macro_name);
        println!("steps: {} completed", result.step_results.len());
        for (i, step_result) in result.step_results.iter().enumerate() {
            let status = if step_result.success { "OK" } else { "FAIL" };
            println!("  [{status}] Step {}: {}", i + 1, step_result.message);
        }
    } else {
        eprintln!("FAIL mdv macro");
        eprintln!("macro: {}", macro_name);
        for (i, step_result) in result.step_results.iter().enumerate() {
            let status = if step_result.success { "OK" } else { "FAIL" };
            eprintln!("  [{status}] Step {}: {}", i + 1, step_result.message);
        }
        std::process::exit(1);
    }
}

/// Build content string for variable extraction from macro spec.
fn build_vars_content(spec: &MacroSpec) -> String {
    let mut content = String::new();

    // Add macro-level vars
    if let Some(vars) = &spec.vars {
        for (name, spec) in vars {
            content.push_str(&format!("{{{{{name}}}}}"));
            if let Some(default) = spec.default() {
                content.push_str(default);
            }
        }
    }

    // Add vars from step overrides
    for step in &spec.steps {
        match step {
            mdvault_core::macros::MacroStep::Template(t) => {
                for v in t.vars_with.values() {
                    content.push_str(v);
                }
                if let Some(output) = &t.output {
                    content.push_str(output);
                }
            }
            mdvault_core::macros::MacroStep::Capture(c) => {
                for v in c.vars_with.values() {
                    content.push_str(v);
                }
            }
            mdvault_core::macros::MacroStep::Shell(s) => {
                content.push_str(&s.shell);
            }
        }
    }

    content
}

fn build_macro_context(cfg: &ResolvedConfig) -> HashMap<String, String> {
    let mut ctx = HashMap::new();

    // Date/time
    let now = Local::now();
    ctx.insert("date".into(), now.format("%Y-%m-%d").to_string());
    ctx.insert("time".into(), now.format("%H:%M").to_string());
    ctx.insert("datetime".into(), now.to_rfc3339());
    ctx.insert("today".into(), now.format("%Y-%m-%d").to_string());
    ctx.insert("now".into(), now.to_rfc3339());

    // Config paths
    ctx.insert("vault_root".into(), cfg.vault_root.to_string_lossy().to_string());
    ctx.insert("templates_dir".into(), cfg.templates_dir.to_string_lossy().to_string());
    ctx.insert("captures_dir".into(), cfg.captures_dir.to_string_lossy().to_string());
    ctx.insert("macros_dir".into(), cfg.macros_dir.to_string_lossy().to_string());

    ctx
}

/// CLI step executor that uses template and capture repositories.
struct CliStepExecutor {
    config: ResolvedConfig,
    template_repo: TemplateRepository,
    capture_repo: CaptureRepository,
}

impl StepExecutor for CliStepExecutor {
    fn execute_template(
        &self,
        step: &TemplateStep,
        ctx: &RunContext,
    ) -> Result<StepResult, MacroRunError> {
        let step_vars = ctx.with_step_vars(&step.vars_with);

        // Load template
        let loaded = self
            .template_repo
            .get_by_name(&step.template)
            .map_err(|e| MacroRunError::TemplateError(e.to_string()))?;

        // Build template info
        let info = TemplateInfo {
            logical_name: loaded.logical_name.clone(),
            path: loaded.path.clone(),
        };

        // Resolve output path
        let output_path = if let Some(ref output) = step.output {
            let rendered = render_string(output, &step_vars)
                .map_err(|e| MacroRunError::TemplateError(e.to_string()))?;
            self.config.vault_root.join(&rendered)
        } else {
            let minimal_ctx = build_minimal_context(&self.config, &info);
            let mut merged_ctx = minimal_ctx;
            for (k, v) in &step_vars {
                merged_ctx.insert(k.clone(), v.clone());
            }
            resolve_template_output_path(&loaded, &self.config, &merged_ctx)
                .map_err(|e| MacroRunError::TemplateError(e.to_string()))?
                .ok_or_else(|| {
                    MacroRunError::TemplateError(
                        "Template has no output path and none specified in macro"
                            .to_string(),
                    )
                })?
        };

        // Check if file exists
        if output_path.exists() {
            return Err(MacroRunError::TemplateError(format!(
                "File already exists: {}",
                output_path.display()
            )));
        }

        // Render template
        let rendered = render_string(&loaded.body, &step_vars)
            .map_err(|e| MacroRunError::TemplateError(e.to_string()))?;

        // Create parent directories
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| MacroRunError::TemplateError(e.to_string()))?;
        }

        // Write file
        fs::write(&output_path, &rendered)
            .map_err(|e| MacroRunError::TemplateError(e.to_string()))?;

        Ok(StepResult {
            step_index: 0, // Will be set by runner
            success: true,
            message: format!("Created {}", output_path.display()),
            output_path: Some(output_path),
        })
    }

    fn execute_capture(
        &self,
        step: &CaptureStep,
        ctx: &RunContext,
    ) -> Result<StepResult, MacroRunError> {
        let step_vars = ctx.with_step_vars(&step.vars_with);

        // Load capture
        let loaded = self
            .capture_repo
            .get_by_name(&step.capture)
            .map_err(|e| MacroRunError::CaptureError(e.to_string()))?;

        // Render target file path
        let target_file_raw = render_string(&loaded.spec.target.file, &step_vars)
            .map_err(|e| MacroRunError::CaptureError(e.to_string()))?;
        let target_file = if Path::new(&target_file_raw).is_absolute() {
            PathBuf::from(&target_file_raw)
        } else {
            self.config.vault_root.join(&target_file_raw)
        };

        // Read existing file
        let existing_content = fs::read_to_string(&target_file).map_err(|e| {
            MacroRunError::CaptureError(format!(
                "Failed to read {}: {e}",
                target_file.display()
            ))
        })?;

        // Parse frontmatter
        let mut parsed = parse(&existing_content)
            .map_err(|e| MacroRunError::CaptureError(e.to_string()))?;

        // Apply frontmatter operations
        if let Some(fm_ops) = &loaded.spec.frontmatter {
            parsed = apply_ops(parsed, fm_ops, &step_vars)
                .map_err(|e| MacroRunError::CaptureError(e.to_string()))?;
        }

        // Insert content if specified
        if let Some(content_template) = &loaded.spec.content {
            let section = loaded.spec.target.section.as_ref().ok_or_else(|| {
                MacroRunError::CaptureError(
                    "Capture has content but no target section".to_string(),
                )
            })?;

            let rendered_content = render_string(content_template, &step_vars)
                .map_err(|e| MacroRunError::CaptureError(e.to_string()))?;
            let section_match = SectionMatch::new(section);
            let position = loaded.spec.target.position.clone().into();

            let result = MarkdownEditor::insert_into_section(
                &parsed.body,
                &section_match,
                &rendered_content,
                position,
            )
            .map_err(|e| MacroRunError::CaptureError(e.to_string()))?;

            parsed.body = result.content;
        }

        // Serialize and write
        let final_content = serialize(&parsed);
        fs::write(&target_file, &final_content)
            .map_err(|e| MacroRunError::CaptureError(e.to_string()))?;

        Ok(StepResult {
            step_index: 0,
            success: true,
            message: format!("Updated {}", target_file.display()),
            output_path: Some(target_file),
        })
    }

    fn execute_shell(
        &self,
        step: &ShellStep,
        ctx: &RunContext,
    ) -> Result<StepResult, MacroRunError> {
        let rendered_cmd = render_string(&step.shell, &ctx.vars)
            .map_err(|e| MacroRunError::ShellError(e.to_string()))?;

        // Execute the command
        let output = Command::new("sh")
            .arg("-c")
            .arg(&rendered_cmd)
            .current_dir(&self.config.vault_root)
            .output()
            .map_err(|e| MacroRunError::ShellError(e.to_string()))?;

        if output.status.success() {
            Ok(StepResult {
                step_index: 0,
                success: true,
                message: format!("Executed: {rendered_cmd}"),
                output_path: None,
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(MacroRunError::ShellError(format!(
                "Command failed: {rendered_cmd}\n{stderr}"
            )))
        }
    }
}
