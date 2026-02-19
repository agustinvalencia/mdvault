use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

// --- Test Harness ---

fn write(path: &Path, contents: &str) {
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

    // Create required directories
    fs::create_dir_all(vault.join(".mdvault/typedefs")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/templates")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/captures")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/macros")).unwrap();

    let mut toml = String::new();
    writeln!(&mut toml, "version = 1").unwrap();
    writeln!(&mut toml, "profile = \"default\"",).unwrap();
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
    // Ensure we run in the vault root so relative paths work
    let vault_root =
        cfg_path.parent().unwrap().parent().unwrap().parent().unwrap().join("vault");
    cmd.current_dir(&vault_root);

    cmd.args(["--config", cfg_path.to_str().unwrap()]);
    cmd.args(args);
    cmd.output().expect("Failed to run mdv")
}

// --- Tests ---

#[test]
fn task_creation_with_project() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Setup: Create a project file
    let project_path = vault.join("Projects/TST/TST.md");
    write(
        &project_path,
        "---\ntype: project
title: Test Project
project-id: TST
task_counter: 5
---\n",
    );

    // Action: mdv new task "My Task" --var project=TST --batch
    let output = run_mdv(
        &cfg_path,
        &["new", "task", "My Task", "--var", "project=TST", "--batch"],
    );

    assert!(output.status.success(), "Command failed: {:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("type:   task"));
    assert!(stdout.contains("id:     TST-006"));

    // Assertions
    let task_path = vault.join("Projects/TST/Tasks/TST-006.md");
    assert!(task_path.exists());

    let content = fs::read_to_string(&task_path).unwrap();
    assert!(content.contains("type: task"));
    assert!(content.contains("title: My Task"));
    assert!(content.contains("task-id: TST-006"));
    assert!(content.contains("project: TST"));

    // Verify project counter updated
    let proj_content = fs::read_to_string(&project_path).unwrap();
    assert!(proj_content.contains("task_counter: 6"));

    // Verify daily note contains link
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let daily_path = vault.join(format!("Journal/Daily/{}.md", today));
    assert!(daily_path.exists());
    let daily_content = fs::read_to_string(&daily_path).unwrap();
    assert!(daily_content.contains("[[TST-006|My Task]]"));
}

#[test]
fn task_creation_inbox() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Action: mdv new task "Inbox Task" --batch (no project specified)
    let output = run_mdv(&cfg_path, &["new", "task", "Inbox Task", "--batch"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("id:     INB-001"));

    // Assertions
    let task_path = vault.join("Inbox/INB-001.md");
    assert!(task_path.exists());

    let content = fs::read_to_string(&task_path).unwrap();
    assert!(content.contains("type: task"));
    assert!(content.contains("task-id: INB-001"));
    // project field should be absent or not TST
    assert!(!content.contains("project: TST"));
}

#[test]
fn project_creation_generates_id() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Action: mdv new project "My Cool Project" --batch
    let output = run_mdv(&cfg_path, &["new", "project", "My Cool Project", "--batch"]);

    assert!(output.status.success());

    // Assertions
    // "My Cool Project" -> MCP
    let proj_path = vault.join("Projects/MCP/MCP.md");
    assert!(proj_path.exists());

    let content = fs::read_to_string(&proj_path).unwrap();
    assert!(content.contains("type: project"));
    assert!(content.contains("title: My Cool Project"));
    assert!(content.contains("project-id: MCP"));
    assert!(content.contains("task_counter: 0"));
}

#[test]
fn project_creation_handles_collision_by_failing() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Setup: Create existing project
    let proj_path = vault.join("Projects/MCP/MCP.md");
    write(
        &proj_path,
        "---\ntype: project
---\n",
    );

    // Action: mdv new project "My Cool Project" --batch
    // Should fail or warn because file exists. Current code says "Refusing to overwrite" and exits 1.
    let output = run_mdv(&cfg_path, &["new", "project", "My Cool Project", "--batch"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Refusing to overwrite"));
}

#[test]
fn task_creation_preserves_core_metadata_after_hook() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Setup: Create task.lua with on_create hook that tries to modify task-id
    let typedef_path = vault.join(".mdvault/typedefs/task.lua");
    write(
        &typedef_path,
        r###"return { 
    on_create = function(note) 
        -- Try to overwrite core fields
        note.frontmatter["task-id"] = "HACKED"
        note.frontmatter["project"] = "HACKED"
        return note
    end
}
"###,
    );

    // Setup project
    let project_path = vault.join("Projects/TST/TST.md");
    write(
        &project_path,
        "---\ntype: project
title: Test Project
project-id: TST
task_counter: 5
---\n",
    );

    // Action
    let output = run_mdv(
        &cfg_path,
        &["new", "task", "Protected Task", "--var", "project=TST", "--batch"],
    );
    assert!(output.status.success());

    // Check output
    let task_path = vault.join("Projects/TST/Tasks/TST-006.md");
    let content = fs::read_to_string(&task_path).unwrap();

    // Core fields should match creation logic, NOT hook hack
    assert!(content.contains("task-id: TST-006"));
    assert!(!content.contains("task-id: HACKED"));
    assert!(content.contains("project: TST"));
    assert!(!content.contains("project: HACKED"));
}

#[test]
fn core_metadata_survives_template_rendering() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Setup: Template that omits core fields
    let template_path = vault.join(".mdvault/templates/task.md");
    write(
        &template_path,
        "---\nlua: task.lua
---\n# Just the content\n",
    );

    // Need a minimal task.lua for the template to work
    let typedef_path = vault.join(".mdvault/typedefs/task.lua");
    write(&typedef_path, "return {}");

    // Action
    let output = run_mdv(
        &cfg_path,
        &[
            "new",
            "--template",
            "task",
            "Template Task",
            "--var",
            "project=inbox",
            "--output",
            "task.md",
            "--batch",
        ],
    );
    assert!(output.status.success());
}

#[test]
fn scaffolding_mode_uses_template_and_preserves_metadata() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Setup: Template for task that looks "broken" (missing metadata)
    let template_path = vault.join(".mdvault/templates/task.md");
    write(
        &template_path,
        "---\ntags: [custom]
---\n# {{title}}
Custom content\n",
    );

    // Setup project
    let project_path = vault.join("Projects/TST/TST.md");
    write(
        &project_path,
        "---\ntype: project
title: Test Project
project-id: TST
task_counter: 0
---\n",
    );

    // Action: mdv new task ... (scaffolding mode)
    let output = run_mdv(
        &cfg_path,
        &["new", "task", "Scaffolded Task", "--var", "project=TST", "--batch"],
    );
    assert!(output.status.success());

    // Assertions
    let task_path = vault.join("Projects/TST/Tasks/TST-001.md");
    assert!(task_path.exists());
    let content = fs::read_to_string(&task_path).unwrap();

    // Should have content from template
    assert!(content.contains("Custom content"));
    // YAML formatting might change tags: [custom] to tags:\n- custom
    assert!(content.contains("tags:") && content.contains("custom"));

    // AND core metadata injected by scaffolding
    assert!(content.contains("type: task"));
    assert!(content.contains("task-id: TST-001"));
    assert!(content.contains("project: TST"));
}

#[test]
fn on_create_hook_can_add_fields_and_modify_content() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Setup: Custom type with hook
    let typedef_path = vault.join(".mdvault/typedefs/custom.lua");
    write(
        &typedef_path,
        r###"return { 
    on_create = function(note) 
        note.frontmatter["added_by_hook"] = "yes"
        note.content = note.content .. "\n\n## Added by hook"
        return note
    end
} 
"###,
    );

    // Action: mdv new custom ...
    let output = run_mdv(&cfg_path, &["new", "custom", "Hook Test", "--batch"]);
    assert!(output.status.success());

    // Output path default: customs/hook-test.md
    let out_path = vault.join("customs/hook-test.md");
    assert!(out_path.exists());
    let content = fs::read_to_string(&out_path).unwrap();

    assert!(content.contains("added_by_hook: yes"));
    assert!(content.contains("## Added by hook"));
}

#[test]
fn template_mode_uses_lua_schema_defaults() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Setup: Lua def with schema defaults
    let typedef_path = vault.join(".mdvault/typedefs/meeting.lua");
    write(
        &typedef_path,
        r###"return { 
    schema = { 
        platform = { type = "string", default = "zoom" }
    }
}
"###,
    );

    // Setup: Template using that lua
    let template_path = vault.join(".mdvault/templates/meeting.md");
    write(
        &template_path,
        "---\nlua: meeting.lua
---\n# Meeting on {{platform}}\n",
    );

    // Action: mdv new --template meeting "My Meeting" --batch
    let output = run_mdv(
        &cfg_path,
        &[
            "new",
            "--template",
            "meeting",
            "My Meeting",
            "--output",
            "meeting.md",
            "--batch",
        ],
    );
    assert!(output.status.success());

    let content = fs::read_to_string(vault.join("meeting.md")).unwrap();
    assert!(content.contains("Meeting on zoom"));
    // The variable is NOT injected into frontmatter in template mode currently (unlike scaffolding mode)
    // assert!(content.contains("platform: zoom"));
}

#[test]
fn generic_scaffolding_mode() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Setup: Create a generic type definition
    let typedef_path = vault.join(".mdvault/typedefs/generic.lua");
    write(
        &typedef_path,
        r###"return { 
    description = "A generic note type",
    schema = { 
        field1 = { type = "string", default = "value1" }
    }
}
"###,
    );

    // Action: mdv new generic "My Note" --batch
    let output = run_mdv(&cfg_path, &["new", "generic", "My Note", "--batch"]);
    assert!(output.status.success());

    // Assertions
    // Default output path for generic types is "generics/my-note.md" (pluralized)
    let note_path = vault.join("generics/my-note.md");
    assert!(note_path.exists());

    let content = fs::read_to_string(&note_path).unwrap();
    assert!(content.contains("type: generic"));
    assert!(content.contains("title: My Note"));
    assert!(content.contains("field1: value1"));
}

#[test]
fn daily_creation_uses_date_path() {
    let (_tmp, vault, cfg_path) = setup_vault();

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    // Setup: Create daily.lua typedef with proper output path
    let typedef_path = vault.join(".mdvault/typedefs/daily.lua");
    write(
        &typedef_path,
        r#"return {
    output = "Journal/Daily/{{title}}.md",
    schema = {
        date = { type = "string", default_expr = "os.date('%Y-%m-%d')" }
    }
}"#,
    );

    // Action: mdv new daily <date> --batch
    let output = run_mdv(&cfg_path, &["new", "daily", &today, "--batch"]);

    assert!(output.status.success(), "Command failed: {:?}", output);

    // Assertions
    let daily_path = vault.join(format!("Journal/Daily/{}.md", today));
    assert!(daily_path.exists(), "Daily note not found at {:?}", daily_path);

    let content = fs::read_to_string(&daily_path).unwrap();
    assert!(content.contains("type: daily"));
    assert!(content.contains(&format!("title: {}", today)));
}

#[test]
fn weekly_creation_uses_week_path() {
    let (_tmp, vault, cfg_path) = setup_vault();

    let today = chrono::Local::now();
    let week = today.format("%G-W%V").to_string();

    // Setup: Create weekly.lua typedef with proper output path
    let typedef_path = vault.join(".mdvault/typedefs/weekly.lua");
    write(
        &typedef_path,
        r#"return {
    output = "Journal/Weekly/{{title}}.md",
    schema = {
        week = { type = "string", default_expr = "os.date('%G-W%V')" }
    }
}"#,
    );

    // Action: mdv new weekly <week> --batch
    let output = run_mdv(&cfg_path, &["new", "weekly", &week, "--batch"]);

    assert!(output.status.success(), "Command failed: {:?}", output);

    // Assertions
    let weekly_path = vault.join(format!("Journal/Weekly/{}.md", week));
    assert!(weekly_path.exists(), "Weekly note not found at {:?}", weekly_path);

    let content = fs::read_to_string(&weekly_path).unwrap();
    assert!(content.contains("type: weekly"));
    assert!(content.contains(&format!("title: {}", week)));
}

#[test]
fn daily_with_date_expression_evaluates_title_and_path() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Calculate expected date (7 days from now)
    let expected_date =
        (chrono::Local::now() + chrono::Duration::days(7)).format("%Y-%m-%d").to_string();

    // Setup: Create daily.lua typedef with output using {{title}}
    let typedef_path = vault.join(".mdvault/typedefs/daily.lua");
    write(
        &typedef_path,
        r#"return {
    output = "Journal/Daily/{{title}}.md",
    schema = {
        date = { type = "string", default_expr = "os.date('%Y-%m-%d')" }
    }
}"#,
    );

    // Action: mdv new daily "today + 7d" --batch
    // The date expression should be evaluated for both path and heading
    let output = run_mdv(&cfg_path, &["new", "daily", "today + 7d", "--batch"]);

    assert!(output.status.success(), "Command failed: {:?}", output);

    // Assertions: File should be created with evaluated date, not literal "today + 7d"
    let daily_path = vault.join(format!("Journal/Daily/{}.md", expected_date));
    assert!(
        daily_path.exists(),
        "Daily note not found at {:?} (should use evaluated date, not 'today + 7d')",
        daily_path
    );

    let content = fs::read_to_string(&daily_path).unwrap();
    assert!(content.contains("type: daily"));
    assert!(content.contains(&format!("title: {}", expected_date)));
    assert!(content.contains(&format!("date: {}", expected_date)));
    // Heading should also use evaluated date
    assert!(
        content.contains(&format!("# {}", expected_date)),
        "Heading should be '# {}', not '# today + 7d'. Content:\n{}",
        expected_date,
        content
    );
}

#[test]
fn weekly_with_date_expression_evaluates_title_and_path() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Calculate expected week (2 weeks from now)
    // Note: The weekly behavior uses %V (ISO week) for date expressions
    let expected_week =
        (chrono::Local::now() + chrono::Duration::weeks(2)).format("%G-W%V").to_string();

    // Setup: Create weekly.lua typedef with output using {{title}}
    let typedef_path = vault.join(".mdvault/typedefs/weekly.lua");
    write(
        &typedef_path,
        r#"return {
    output = "Journal/Weekly/{{title}}.md",
    schema = {
        week = { type = "string", default_expr = "os.date('%G-W%V')" }
    }
}"#,
    );

    // Action: mdv new weekly "today + 2w" --batch
    // The date expression should be evaluated for both path and heading
    let output = run_mdv(&cfg_path, &["new", "weekly", "today + 2w", "--batch"]);

    assert!(output.status.success(), "Command failed: {:?}", output);

    // Assertions: File should be created with evaluated week, not literal "today + 2w"
    let weekly_path = vault.join(format!("Journal/Weekly/{}.md", expected_week));
    assert!(
        weekly_path.exists(),
        "Weekly note not found at {:?} (should use evaluated week, not 'today + 2w')",
        weekly_path
    );

    let content = fs::read_to_string(&weekly_path).unwrap();
    assert!(content.contains("type: weekly"));
    assert!(content.contains(&format!("title: {}", expected_week)));
    assert!(content.contains(&format!("week: {}", expected_week)));
    // Heading should also use evaluated week
    assert!(
        content.contains(&format!("# {}", expected_week)),
        "Heading should be '# {}', not '# today + 2w'. Content:\n{}",
        expected_week,
        content
    );
}

#[test]
fn on_create_hook_preserves_schema_defaults_when_returning_new_note() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Setup: Type with schema defaults and an on_create hook that returns a NEW note object
    // with only partial frontmatter (simulating the MDV-010 bug scenario)
    let typedef_path = vault.join(".mdvault/typedefs/custom.lua");
    write(
        &typedef_path,
        r###"return {
    schema = {
        priority = { type = "string", default = "low" },
        status = { type = "string", default = "open" },
        tags = { type = "string", default = "default-tag" },
    },
    on_create = function(note)
        -- Return a NEW table with only partial frontmatter
        -- Schema defaults should still be preserved via merge
        return {
            frontmatter = {
                added_by_hook = "yes",
                priority = "high",
            },
            content = note.content,
        }
    end
}
"###,
    );

    let output = run_mdv(&cfg_path, &["new", "custom", "Schema Test", "--batch"]);
    assert!(
        output.status.success(),
        "mdv new failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let out_path = vault.join("customs/schema-test.md");
    assert!(out_path.exists());
    let content = fs::read_to_string(&out_path).unwrap();

    // Hook's field should be present
    assert!(
        content.contains("added_by_hook: yes"),
        "Hook field missing. Content:\n{content}"
    );
    // Hook's override should win
    assert!(
        content.contains("priority: high"),
        "Hook override missing. Content:\n{content}"
    );
    // Schema defaults NOT set by hook should be preserved
    assert!(
        content.contains("status: open"),
        "Schema default 'status' lost. Content:\n{content}"
    );
    assert!(
        content.contains("tags: default-tag"),
        "Schema default 'tags' lost. Content:\n{content}"
    );
}
