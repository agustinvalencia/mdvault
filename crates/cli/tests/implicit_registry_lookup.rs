use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

// --- Test Harness ---

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn setup_vault() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let cfg_path = setup_config(&tmp, &vault);
    (tmp, vault, cfg_path)
}

fn setup_config(tmp: &tempfile::TempDir, vault: &Path) -> PathBuf {
    let xdg = tmp.path().join("xdg");
    let cfg_dir = xdg.join("mdvault");
    let cfg_path = cfg_dir.join("config.toml");
    fs::create_dir_all(&cfg_dir).unwrap();

    fs::create_dir_all(vault.join(".mdvault/typedefs")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/templates")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/captures")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/macros")).unwrap();

    let mut toml = String::new();
    writeln!(&mut toml, "version = 1").unwrap();
    writeln!(&mut toml, "profile = \"default\"").unwrap();
    writeln!(&mut toml).unwrap();
    writeln!(&mut toml, "[profiles.default]").unwrap();
    writeln!(&mut toml, "vault_root = \"{}\"", vault.display()).unwrap();
    writeln!(&mut toml, "typedefs_dir = \"{}/.mdvault/typedefs\"", vault.display())
        .unwrap();
    writeln!(&mut toml, "templates_dir = \"{}/.mdvault/templates\"", vault.display())
        .unwrap();
    writeln!(&mut toml, "captures_dir = \"{}/.mdvault/captures\"", vault.display())
        .unwrap();
    writeln!(&mut toml, "macros_dir = \"{}/.mdvault/macros\"", vault.display()).unwrap();

    fs::write(&cfg_path, toml).unwrap();
    cfg_path
}

fn run_mdv(cfg_path: &Path, args: &[&str]) -> std::process::Output {
    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("NO_COLOR", "1");
    let vault_root =
        cfg_path.parent().unwrap().parent().unwrap().parent().unwrap().join("vault");
    cmd.current_dir(&vault_root);

    cmd.args(["--config", cfg_path.to_str().unwrap()]);
    cmd.args(args);
    cmd.output().expect("Failed to run mdv")
}

// --- Tests ---

/// Template exists but has NO `lua:` field; typedef loaded by name match from registry.
/// Verifies: schema defaults applied, output path from typedef used.
#[test]
fn template_without_lua_field_loads_typedef_by_name() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Typedef: defines output path and a schema default
    write_file(
        &vault.join(".mdvault/typedefs/briefing.lua"),
        r#"return {
    name = "briefing",
    output = "briefings/{{title | slugify}}.md",
    schema = {
        title = { type = "string", required = true },
        status = { type = "string", default = "draft" },
    }
}"#,
    );

    // Template: NO `lua:` field — typedef must be found by registry name match
    write_file(
        &vault.join(".mdvault/templates/briefing.md"),
        "---\ntype: briefing\ntitle: {{title}}\nstatus: {{status}}\n---\n# {{title}}\n",
    );

    let output = run_mdv(&cfg_path, &["new", "briefing", "Budget Review", "--batch"]);

    assert!(
        output.status.success(),
        "Failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Output path should come from the typedef (not the default slug)
    let expected = vault.join("briefings/budget-review.md");
    assert!(expected.exists(), "File should exist at {}", expected.display());

    let content = fs::read_to_string(&expected).unwrap();
    // Schema default applied (proves typedef was loaded)
    assert!(
        content.contains("status: draft"),
        "Schema default missing. Content:\n{content}"
    );
    assert!(content.contains("title: Budget Review"));
}

/// Title provided via `--var title=X` is used for output path rendering
/// when the typedef is loaded via implicit registry lookup (no `lua:` field).
#[test]
fn title_from_var_used_for_output_path_via_registry() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Typedef: output path uses {{title | slugify}}
    write_file(
        &vault.join(".mdvault/typedefs/resource.lua"),
        r#"return {
    name = "resource",
    output = "resources/{{title | slugify}}.md",
    schema = {
        title = { type = "string", required = true },
    }
}"#,
    );

    // Template: NO `lua:` field
    write_file(
        &vault.join(".mdvault/templates/resource.md"),
        "---\ntype: resource\ntitle: {{title}}\n---\n# {{title}}\n",
    );

    // Provide title via --var (not positional arg)
    let output = run_mdv(
        &cfg_path,
        &["new", "resource", "--var", "title=API Documentation", "--batch"],
    );

    assert!(
        output.status.success(),
        "Failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Path should use slugified title from --var, not be empty
    let expected = vault.join("resources/api-documentation.md");
    assert!(
        expected.exists(),
        "File should exist at {} (title from --var should be used for path)",
        expected.display()
    );

    let content = fs::read_to_string(&expected).unwrap();
    assert!(content.contains("title: API Documentation"));
}

/// Task creation without a template file — typedef and behaviour drive everything.
/// Verifies: scaffolding content generated, output path from behaviour, daily log created.
#[test]
fn task_creation_without_template_file() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // NO task.md template — only the typedef exists
    // (don't write anything to .mdvault/templates/)

    // Setup project
    write_file(
        &vault.join("Projects/TST/TST.md"),
        "---\ntype: project\ntitle: Test Project\nproject-id: TST\ntask_counter: 0\n---\n",
    );

    let output = run_mdv(
        &cfg_path,
        &["new", "task", "No Template Task", "--var", "project=TST", "--batch"],
    );

    assert!(
        output.status.success(),
        "Failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("id:   TST-001"),
        "Should show generated ID. Stdout:\n{stdout}"
    );

    // Task file should exist at behaviour-defined path
    let task_path = vault.join("Projects/TST/Tasks/TST-001.md");
    assert!(task_path.exists(), "Task file should exist at {}", task_path.display());

    let content = fs::read_to_string(&task_path).unwrap();
    assert!(content.contains("type: task"));
    assert!(content.contains("task-id: TST-001"));
    assert!(content.contains("project: TST"));
    assert!(content.contains("title: No Template Task"));

    // Project counter should be incremented (proves after_create ran)
    let proj = fs::read_to_string(vault.join("Projects/TST/TST.md")).unwrap();
    assert!(proj.contains("task_counter: 1"), "Counter should be 1. Project:\n{proj}");

    // Daily note should contain link (proves after_create lifecycle completed)
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let year = &today[..4];
    let daily_path = vault.join(format!("Journal/{}/Daily/{}.md", year, today));
    assert!(daily_path.exists(), "Daily note should exist");
    let daily = fs::read_to_string(&daily_path).unwrap();
    assert!(
        daily.contains("[[TST-001|No Template Task]]"),
        "Daily should contain task link. Daily:\n{daily}"
    );
}
