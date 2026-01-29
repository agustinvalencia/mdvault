/// Integration tests for bug fixes related to template variables in frontmatter
///
/// Bug Report 1: mdv new fails when template has variables in frontmatter like `name: {{name}}`
/// Bug Report 2: Missing metadata after mdv new when template has schema variables
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn write(path: &PathBuf, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn make_config(vault_root: &str, templates_dir: &str, typedefs_dir: &str) -> String {
    format!(
        r#"
version = 1
profile = "test"

[profiles.test]
vault_root = "{vault_root}"
templates_dir = "{templates_dir}"
typedefs_dir = "{typedefs_dir}"
captures_dir = "{{{{vault_root}}}}/.mdvault/captures"
macros_dir = "{{{{vault_root}}}}/.mdvault/macros"
"#
    )
}

#[test]
fn bug_report_1_contact_template_with_vars_in_frontmatter() {
    // Replicates the exact scenario from Bug Report 1
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    let vault = root.join("vault");
    let templates = vault.join(".mdvault").join("templates");
    let typedefs = vault.join(".mdvault").join("typedefs");
    let config_path = root.join("config.toml");

    write(
        &config_path,
        &make_config(
            &vault.to_string_lossy(),
            &templates.to_string_lossy(),
            &typedefs.to_string_lossy(),
        ),
    );

    // Create the contact template with variables in frontmatter (exactly as in bug report)
    write(
        &templates.join("contact.md"),
        r#"---
type: contact
lua: contact.lua
name: {{name}}
email: {{email}}
phone: {{phone}}
position: {{position}}
organisation: {{organisation}}
tags: {{tags}}
---
# {{name}}
[[Contacts]]

## Contact Info
- **Email**: {{email}}
- **Phone**: {{phone}}
- **Position**: {{position}} at {{organisation}}

## Notes

## Interactions
"#,
    );

    // Create the Lua typedef
    write(
        &typedefs.join("contact.lua"),
        r#"local M = {
    name = "contact",
    description = "Contact for Person",
    output = "Contacts/{{name|slugify}}.md",
    schema = {
        type = { type = "string", core = true },
        title = { type = "string", core = true, prompt = "Project Name" },
        tags = {
            type = "string",
            prompt = "Tags",
            default = "",
        },
        name = {
            required = true,
            type = "string",
            prompt = "Name",
        },
        email = {
            type = "string",
            prompt = "Email",
        },
        phone = {
            type = "string",
            default = "-",
        },
        position = {
            required = true,
            type = "string",
            prompt = "Position",
        },
        organisation = {
            required = true,
            type = "string",
            prompt = "Organisation",
        },
    },
}

return M
"#,
    );

    // Run the command with batch mode and variables
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("XDG_CONFIG_HOME", root).env("NO_COLOR", "1").args([
        "--config",
        config_path.to_str().unwrap(),
        "new",
        "--template",
        "contact",
        "--batch",
        "--var",
        "name=John Doe",
        "--var",
        "email=john@example.com",
        "--var",
        "organisation=Acme Corp",
        "--var",
        "position=Engineer",
        "--var",
        "tags=test",
    ]);

    let output = cmd.output().unwrap();

    if !output.status.success() {
        eprintln!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));
        panic!("Command failed");
    }

    assert!(output.status.success(), "Command should succeed");

    // Verify the file was created
    let contact_file = vault.join("Contacts").join("john-doe.md");
    assert!(contact_file.exists(), "Contact file should be created");

    // Verify the content has variables substituted
    let content = fs::read_to_string(&contact_file).unwrap();
    println!("Generated content:\n{}", content);

    assert!(content.contains("name: John Doe"), "name should be substituted");
    assert!(content.contains("email: john@example.com"), "email should be substituted");
    assert!(content.contains("position: Engineer"), "position should be substituted");
    assert!(
        content.contains("organisation: Acme Corp"),
        "organisation should be substituted"
    );
    // Check that the default dash value is properly quoted to avoid YAML parsing errors
    assert!(
        content.contains("phone: \"-\""),
        "phone default '-' should be quoted, got: {}",
        content
    );
    assert!(content.contains("# John Doe"), "name in body should be substituted");
    assert!(
        content.contains("**Email**: john@example.com"),
        "email in body should be substituted"
    );
}

#[test]
fn bug_report_2_task_template_missing_metadata() {
    // Replicates the exact scenario from Bug Report 2
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    let vault = root.join("vault");
    let templates = vault.join(".mdvault").join("templates");
    let typedefs = vault.join(".mdvault").join("typedefs");
    let config_path = root.join("config.toml");

    write(
        &config_path,
        &make_config(
            &vault.to_string_lossy(),
            &templates.to_string_lossy(),
            &typedefs.to_string_lossy(),
        ),
    );

    // Create the task template with schema variables in frontmatter
    write(
        &templates.join("task.md"),
        r#"---
lua: task.lua
type: task
status: {{status}}
priority: {{priority}}
due_date: {{due_date}}
planned_for: {{planned_for}}
created_at: {{created_at}}
updated_at: {{updated_at}}
---
# {{title}}
[[{{project}}]]

## Description
{{description}}

## Acceptance Criteria
- [ ] {{criteria}}

## Notes

## Logs
- [[{{date}}]] {{time}} : Task created
"#,
    );

    // Create a simplified task typedef
    write(
        &typedefs.join("task.lua"),
        r#"local M = {
    name = "task",
    description = "Create new task",
    output = "Tasks/{{title|slugify}}.md",
    schema = {
        type = { type = "string", core = true },
        title = { type = "string", core = true },
        created_at = { type = "datetime", required = true, default = "2026-01-28T18:00:00" },
        updated_at = { type = "datetime", required = true, default = "2026-01-28T18:00:00" },
        status = {
            required = true,
            type = "string",
            enum = { "todo", "in-progress", "done" },
            default = "todo",
        },
        priority = {
            required = true,
            type = "string",
            enum = { "high", "medium", "low" },
            default = "medium",
        },
        due_date = {
            type = "date",
            required = false,
        },
        planned_for = {
            type = "date",
            required = false,
        },
        project = {
            type = "string",
            default = "inbox",
        },
    },
    variables = {
        description = {
            type = "string",
            prompt = "Task description",
            default = "No description",
        },
        criteria = {
            type = "string",
            prompt = "Completion criteria",
            default = "Task completed",
        },
    },
}

return M
"#,
    );

    // Run the command in batch mode
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("XDG_CONFIG_HOME", root).env("NO_COLOR", "1").args([
        "--config",
        config_path.to_str().unwrap(),
        "new",
        "--template",
        "task",
        "--batch",
        "Test Task",
        "--var",
        "due_date=2026-02-15",
        "--var",
        "project=myproject",
    ]);

    let output = cmd.output().unwrap();

    if !output.status.success() {
        eprintln!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));
        panic!("Command failed");
    }

    assert!(output.status.success(), "Command should succeed");

    // Verify the file was created
    let task_file = vault.join("Tasks").join("test-task.md");
    assert!(task_file.exists(), "Task file should be created");

    // Verify ALL metadata fields are present (not just type, title)
    let content = fs::read_to_string(&task_file).unwrap();
    println!("Generated content:\n{}", content);

    // These are the fields that were missing in the bug report
    assert!(content.contains("status: todo"), "status should be present");
    assert!(content.contains("priority: medium"), "priority should be present");
    assert!(content.contains("due_date: 2026-02-15"), "due_date should be present");
    assert!(
        content.contains("created_at: 2026-01-28T18:00:00"),
        "created_at should be present"
    );
    assert!(
        content.contains("updated_at: 2026-01-28T18:00:00"),
        "updated_at should be present"
    );

    // Verify template-specific fields are filtered out
    assert!(!content.contains("lua:"), "lua field should be filtered out");

    // Verify body substitutions
    assert!(content.contains("# Test Task"), "title should be in body");
    assert!(content.contains("[[myproject]]"), "project should be in body");
}
