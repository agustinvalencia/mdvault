//! Test for bug fix: template mode with Lua typedef should prompt for title.
//!
//! Bug: When running `mdv new --template <template>` where the template has
//! a Lua typedef with `title = { core = true, prompt = "..." }`, the title
//! was not being prompted because core fields were unconditionally skipped.

use assert_cmd::prelude::*;
use predicates::prelude::*;
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

/// Test that template mode with Lua typedef prompts for title when not provided.
/// In batch mode without title, this should error.
#[test]
fn template_lua_typedef_requires_title_in_batch_mode() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let typedefs_dir = vault.join(".mdvault/types");
    let templates_dir = vault.join(".mdvault/templates");
    let cfg_path = tmp.path().join("config.toml");

    // Create Lua typedef with title field (core = true, prompt set, required = true)
    write(
        &typedefs_dir.join("project_resource.lua"),
        r#"return {
    name = "project_resource",
    description = "Resource specific to one project",
    output = "Projects/{{project}}/resources/{{title|slugify}}.md",
    schema = {
        title = { type = "string", core = true, prompt = "Note Title", required = true },
        project = { type = "string", prompt = "Project Folder", required = true },
    }
}
"#,
    );

    // Create template that references the Lua typedef
    write(
        &templates_dir.join("project_resource.md"),
        r#"---
type: project_resource
lua: project_resource.lua
---

# {{title}}

Project: {{project}}
"#,
    );

    // Create config
    let toml = format!(
        r#"
version = 1
profile = "default"

[profiles.default]
vault_root = "{vault}"
templates_dir = "{tpl}"
typedefs_dir = "{typedefs}"
captures_dir = "{{{{vault_root}}}}/.mdvault/captures"
macros_dir = "{{{{vault_root}}}}/.mdvault/macros"
"#,
        vault = vault.display(),
        tpl = templates_dir.display(),
        typedefs = typedefs_dir.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    // Create necessary directories
    fs::create_dir_all(vault.join(".mdvault/captures")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/macros")).unwrap();

    // Run in batch mode without providing title - should fail
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(&cfg_path)
        .arg("new")
        .arg("--template")
        .arg("project_resource")
        .arg("--batch")
        .arg("--var")
        .arg("project=test-project");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Missing required field 'title' in batch mode"));
}

/// Test that template mode with Lua typedef works when title is provided via CLI.
#[test]
fn template_lua_typedef_title_provided_works() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let typedefs_dir = vault.join(".mdvault/types");
    let templates_dir = vault.join(".mdvault/templates");
    let cfg_path = tmp.path().join("config.toml");

    // Create Lua typedef with title field (core = true, prompt set, required = true)
    write(
        &typedefs_dir.join("project_resource.lua"),
        r#"return {
    name = "project_resource",
    description = "Resource specific to one project",
    output = "Projects/{{project}}/resources/{{title|slugify}}.md",
    schema = {
        title = { type = "string", core = true, prompt = "Note Title", required = true },
        project = { type = "string", prompt = "Project Folder", required = true },
    }
}
"#,
    );

    // Create template that references the Lua typedef
    write(
        &templates_dir.join("project_resource.md"),
        r#"---
type: project_resource
lua: project_resource.lua
---

# {{title}}

Project: {{project}}
"#,
    );

    // Create config
    let toml = format!(
        r#"
version = 1
profile = "default"

[profiles.default]
vault_root = "{vault}"
templates_dir = "{tpl}"
typedefs_dir = "{typedefs}"
captures_dir = "{{{{vault_root}}}}/.mdvault/captures"
macros_dir = "{{{{vault_root}}}}/.mdvault/macros"
"#,
        vault = vault.display(),
        tpl = templates_dir.display(),
        typedefs = typedefs_dir.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    // Create necessary directories
    fs::create_dir_all(vault.join(".mdvault/captures")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/macros")).unwrap();

    // Run with title provided as positional arg - should succeed
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(&cfg_path)
        .arg("new")
        .arg("--template")
        .arg("project_resource")
        .arg("My Test Resource") // Title as positional arg
        .arg("--batch")
        .arg("--var")
        .arg("project=test-project");

    cmd.assert().success();

    // Verify the file was created with correct path and content
    let expected_path = vault.join("Projects/test-project/resources/my-test-resource.md");
    assert!(expected_path.exists(), "Output file should exist at {:?}", expected_path);

    let content = fs::read_to_string(&expected_path).unwrap();
    assert!(content.contains("# My Test Resource"), "Title should be in content");
    assert!(content.contains("Project: test-project"), "Project should be in content");
}

/// Test that template mode with Lua typedef works when title is provided via --var.
#[test]
fn template_lua_typedef_title_via_var_works() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let typedefs_dir = vault.join(".mdvault/types");
    let templates_dir = vault.join(".mdvault/templates");
    let cfg_path = tmp.path().join("config.toml");

    // Create Lua typedef with title field (core = true, prompt set, required = true)
    write(
        &typedefs_dir.join("resource.lua"),
        r#"return {
    name = "resource",
    output = "resources/{{title|slugify}}.md",
    schema = {
        title = { type = "string", core = true, prompt = "Resource Title", required = true },
    }
}
"#,
    );

    // Create template
    write(
        &templates_dir.join("resource.md"),
        r#"---
type: resource
lua: resource.lua
---

# {{title}}

Content here.
"#,
    );

    // Create config
    let toml = format!(
        r#"
version = 1
profile = "default"

[profiles.default]
vault_root = "{vault}"
templates_dir = "{tpl}"
typedefs_dir = "{typedefs}"
captures_dir = "{{{{vault_root}}}}/.mdvault/captures"
macros_dir = "{{{{vault_root}}}}/.mdvault/macros"
"#,
        vault = vault.display(),
        tpl = templates_dir.display(),
        typedefs = typedefs_dir.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    fs::create_dir_all(vault.join(".mdvault/captures")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/macros")).unwrap();

    // Run with title provided via --var
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(&cfg_path)
        .arg("new")
        .arg("--template")
        .arg("resource")
        .arg("--batch")
        .arg("--var")
        .arg("title=API Documentation");

    cmd.assert().success();

    // Verify the file was created
    let expected_path = vault.join("resources/api-documentation.md");
    assert!(expected_path.exists(), "Output file should exist");

    let content = fs::read_to_string(&expected_path).unwrap();
    assert!(content.contains("# API Documentation"), "Title should be substituted");
}

/// Test that title with default value works without prompting.
#[test]
fn template_lua_typedef_title_with_default() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let typedefs_dir = vault.join(".mdvault/types");
    let templates_dir = vault.join(".mdvault/templates");
    let cfg_path = tmp.path().join("config.toml");

    // Create Lua typedef with title field that has a default
    write(
        &typedefs_dir.join("daily.lua"),
        r#"return {
    name = "daily",
    output = "daily/{{title}}.md",
    schema = {
        title = {
            type = "string",
            core = true,
            default = mdv.date("today")
        },
    }
}
"#,
    );

    // Create template
    write(
        &templates_dir.join("daily.md"),
        r#"---
type: daily
lua: daily.lua
---

# Daily Note: {{title}}

## Tasks

## Notes
"#,
    );

    // Create config
    let toml = format!(
        r#"
version = 1
profile = "default"

[profiles.default]
vault_root = "{vault}"
templates_dir = "{tpl}"
typedefs_dir = "{typedefs}"
captures_dir = "{{{{vault_root}}}}/.mdvault/captures"
macros_dir = "{{{{vault_root}}}}/.mdvault/macros"
"#,
        vault = vault.display(),
        tpl = templates_dir.display(),
        typedefs = typedefs_dir.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    fs::create_dir_all(vault.join(".mdvault/captures")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/macros")).unwrap();

    // Run without providing title - should use default
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(&cfg_path)
        .arg("new")
        .arg("--template")
        .arg("daily")
        .arg("--batch");

    cmd.assert().success();

    // Verify a file was created in the daily directory
    let daily_dir = vault.join("daily");
    assert!(daily_dir.exists(), "daily directory should exist");

    let files: Vec<_> = fs::read_dir(&daily_dir).unwrap().collect();
    assert_eq!(files.len(), 1, "Should have one daily note");

    // Filename should be a date (starts with 20)
    let filename = files[0].as_ref().unwrap().file_name();
    let filename_str = filename.to_string_lossy();
    assert!(
        filename_str.starts_with("20"),
        "Filename should start with year: {}",
        filename_str
    );
}
