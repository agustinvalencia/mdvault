use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn write(path: &PathBuf, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

#[test]
fn new_autofix_adds_missing_default_field_in_scaffolding_mode_with_template() {
    let tmp = tempdir().unwrap();

    // XDG config
    let xdg = tmp.path().join("xdg");
    let cfg_dir = xdg.join("mdvault");
    let cfg_path = cfg_dir.join("config.toml");
    fs::create_dir_all(&cfg_dir).unwrap();

    // Vault
    let vault = tmp.path().join("vault");
    let typedefs_dir = vault.join(".mdvault").join("typedefs");
    let templates_dir = vault.join(".mdvault").join("templates");

    // 1. Define a type "bug" with a required field "status" defaulting to "open"
    let typedef_path = typedefs_dir.join("bug.lua");
    write(
        &typedef_path,
        r#"
return {
    schema = {
        status = {
            type = "string",
            required = true,
            default = "open"
        }
    }
}
"#,
    );

    // 2. Define a template for "bug" that OMITS "status" from frontmatter
    // This forces the generated note to lack "status" initially, triggering validation failure
    // which should be caught and fixed by autovalidation.
    let template_path = templates_dir.join("bug.md");
    write(
        &template_path,
        r#"---
type: bug
title: {{title}}
---

# {{title}}

Description here.
"#,
    );

    let toml = format!(
        r#"
version = 1
profile = "default"

[profiles.default]
vault_root = "{vault}"
typedefs_dir = "{typedefs}"
templates_dir = "{templates}"
captures_dir = "{vault}/.mdvault/captures"
macros_dir = "{vault}/.mdvault/macros"
"#,
        vault = vault.display(),
        typedefs = typedefs_dir.display(),
        templates = templates_dir.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    let output = vault.join("bugs/test-bug.md");

    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("XDG_CONFIG_HOME", &xdg);
    cmd.env("NO_COLOR", "1");
    // We use scaffolding mode (providing type name)
    cmd.args([
        "--config",
        cfg_path.to_str().unwrap(),
        "--profile",
        "default",
        "new",
        "bug",
        "Test Bug",
        "--batch",
        "--output",
        output.to_str().unwrap(),
    ]);

    let output_result = cmd.output().expect("Failed to run command");
    println!("STDOUT:\n{}", String::from_utf8_lossy(&output_result.stdout));
    println!("STDERR:\n{}", String::from_utf8_lossy(&output_result.stderr));
    assert!(output_result.status.success(), "Command should succeed");
    let stdout = String::from_utf8_lossy(&output_result.stdout);
    assert!(stdout.contains("OK   mdv new"), "Should print OK message");
    assert!(
        stdout.contains("Auto-fixed validation errors"),
        "Should print auto-fix message"
    );

    // Check if the file contains the default value added by autofix
    let content = fs::read_to_string(&output).unwrap();
    println!("Generated content:\n{}", content);

    // Autofix should add the missing status field
    assert!(content.contains("status: open"), "autofix should add status: open");

    // Template-rendered fields should be preserved
    assert!(content.contains("type: bug"), "type should be preserved from template");
    assert!(
        content.contains("title: Test Bug"),
        "title should be preserved from template"
    );

    // Template body should be preserved
    assert!(content.contains("# Test Bug"), "heading should be rendered from template");
    assert!(
        content.contains("Description here."),
        "body should be rendered from template"
    );
}
