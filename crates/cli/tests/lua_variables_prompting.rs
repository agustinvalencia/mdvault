//! Regression tests for Lua typedef `variables` section prompting.
//!
//! This test ensures that variables defined in the `variables` section of Lua
//! type definitions are:
//! 1. Collected with their defaults in batch mode
//! 2. Rendered into the template body
//!
//! Bug regression: https://github.com/agustinvalencia/mdvault/issues/XX
//! The `collect_schema_variables` function was only processing `schema` fields
//! (frontmatter) but ignoring `variables` (template body placeholders).

use assert_cmd::prelude::*;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn write(path: &PathBuf, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

/// Test that variables defined in Lua `variables` section are used with defaults
/// in batch mode and rendered into the template.
#[test]
fn lua_variables_section_collected_in_batch_mode() {
    let tmp = tempdir().unwrap();

    // XDG config
    let xdg = tmp.path().join("xdg");
    let cfg_dir = xdg.join("mdvault");
    let cfg_path = cfg_dir.join("config.toml");
    fs::create_dir_all(&cfg_dir).unwrap();

    // Vault
    let vault = tmp.path().join("vault");

    // Type definitions
    let typedefs_dir = vault.join(".mdvault/types");
    let lua_path = typedefs_dir.join("note.lua");

    // Templates
    let tpl_root = vault.join(".mdvault/templates");
    let tpl_note = tpl_root.join("note.md");

    // Create required directories
    fs::create_dir_all(vault.join(".mdvault/captures")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/macros")).unwrap();

    // 1. Create Template that uses variables
    write(
        &tpl_note,
        r#"---
type: note
---
# {{title}}

## Description
{{description}}

## Notes
{{notes}}
"#,
    );

    // 2. Create Lua Type Definition with variables section
    let lua_source = r#"
return {
    name = "note",
    description = "Simple note",
    schema = {
        type = { type = "string", core = true },
        title = { type = "string", core = true },
    },
    variables = {
        description = {
            type = "string",
            prompt = "Enter description",
            default = "Default description text",
        },
        notes = {
            type = "string",
            prompt = "Enter notes",
            default = "Default notes content",
        },
    },
}
"#;
    write(&lua_path, lua_source);

    // 3. Create Config
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
        tpl = tpl_root.display(),
        typedefs = typedefs_dir.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    let output = vault.join("test-note.md");

    // 4. Run mdv new in batch mode (uses defaults)
    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("XDG_CONFIG_HOME", &xdg);
    cmd.env("NO_COLOR", "1");
    cmd.args([
        "--config",
        cfg_path.to_str().unwrap(),
        "new",
        "note",
        "Test Note",
        "--batch",
        "--output",
        output.to_str().unwrap(),
    ]);

    cmd.assert().success();

    // 5. Verify Output contains the default values from variables section
    let rendered = fs::read_to_string(&output).unwrap();

    assert!(
        rendered.contains("Default description text"),
        "Expected 'Default description text' from variables.description.default, found:\n{}",
        rendered
    );
    assert!(
        rendered.contains("Default notes content"),
        "Expected 'Default notes content' from variables.notes.default, found:\n{}",
        rendered
    );
}

/// Test that variables can be overridden via --var flag
#[test]
fn lua_variables_section_overridden_by_var_flag() {
    let tmp = tempdir().unwrap();

    // XDG config
    let xdg = tmp.path().join("xdg");
    let cfg_dir = xdg.join("mdvault");
    let cfg_path = cfg_dir.join("config.toml");
    fs::create_dir_all(&cfg_dir).unwrap();

    // Vault
    let vault = tmp.path().join("vault");

    // Type definitions
    let typedefs_dir = vault.join(".mdvault/types");
    let lua_path = typedefs_dir.join("note.lua");

    // Templates
    let tpl_root = vault.join(".mdvault/templates");
    let tpl_note = tpl_root.join("note.md");

    // Create required directories
    fs::create_dir_all(vault.join(".mdvault/captures")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/macros")).unwrap();

    // 1. Create Template
    write(
        &tpl_note,
        r#"---
type: note
---
# {{title}}

## Description
{{description}}
"#,
    );

    // 2. Create Lua Type Definition with variables section
    let lua_source = r#"
return {
    name = "note",
    description = "Simple note",
    schema = {
        type = { type = "string", core = true },
        title = { type = "string", core = true },
    },
    variables = {
        description = {
            type = "string",
            prompt = "Enter description",
            default = "Default description",
        },
    },
}
"#;
    write(&lua_path, lua_source);

    // 3. Create Config
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
        tpl = tpl_root.display(),
        typedefs = typedefs_dir.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    let output = vault.join("test-note.md");

    // 4. Run mdv new with --var to override the default
    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("XDG_CONFIG_HOME", &xdg);
    cmd.env("NO_COLOR", "1");
    cmd.args([
        "--config",
        cfg_path.to_str().unwrap(),
        "new",
        "note",
        "Test Note",
        "--var",
        "description=Custom description provided",
        "--output",
        output.to_str().unwrap(),
    ]);

    cmd.assert().success();

    // 5. Verify Output contains the custom value, not the default
    let rendered = fs::read_to_string(&output).unwrap();

    assert!(
        rendered.contains("Custom description provided"),
        "Expected 'Custom description provided' from --var flag, found:\n{}",
        rendered
    );
    assert!(
        !rendered.contains("Default description"),
        "Should not contain 'Default description', found:\n{}",
        rendered
    );
}

/// Test that required variables without defaults fail in batch mode
#[test]
fn lua_variables_required_without_default_fails_batch() {
    let tmp = tempdir().unwrap();

    // XDG config
    let xdg = tmp.path().join("xdg");
    let cfg_dir = xdg.join("mdvault");
    let cfg_path = cfg_dir.join("config.toml");
    fs::create_dir_all(&cfg_dir).unwrap();

    // Vault
    let vault = tmp.path().join("vault");

    // Type definitions
    let typedefs_dir = vault.join(".mdvault/types");
    let lua_path = typedefs_dir.join("note.lua");

    // Templates
    let tpl_root = vault.join(".mdvault/templates");
    let tpl_note = tpl_root.join("note.md");

    // Create required directories
    fs::create_dir_all(vault.join(".mdvault/captures")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/macros")).unwrap();

    // 1. Create Template
    write(
        &tpl_note,
        r#"---
type: note
---
# {{title}}

{{required_field}}
"#,
    );

    // 2. Create Lua Type Definition with required variable (no default)
    let lua_source = r#"
return {
    name = "note",
    description = "Simple note",
    schema = {
        type = { type = "string", core = true },
        title = { type = "string", core = true },
    },
    variables = {
        required_field = {
            type = "string",
            prompt = "This is required",
            required = true,
        },
    },
}
"#;
    write(&lua_path, lua_source);

    // 3. Create Config
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
        tpl = tpl_root.display(),
        typedefs = typedefs_dir.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    let output = vault.join("test-note.md");

    // 4. Run mdv new in batch mode - should fail because required variable has no default
    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("XDG_CONFIG_HOME", &xdg);
    cmd.env("NO_COLOR", "1");
    cmd.args([
        "--config",
        cfg_path.to_str().unwrap(),
        "new",
        "note",
        "Test Note",
        "--batch",
        "--output",
        output.to_str().unwrap(),
    ]);

    // Should fail with an error about missing required variable
    cmd.assert().failure();
}
