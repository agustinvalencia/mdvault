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

#[test]
fn test_frontmatter_order_enforced() {
    let tmp = tempdir().unwrap();

    // XDG config
    let xdg = tmp.path().join("xdg");
    let cfg_dir = xdg.join("mdvault");
    let cfg_path = cfg_dir.join("config.toml");
    fs::create_dir_all(&cfg_dir).unwrap();

    // Vault and typedefs
    let vault = tmp.path().join("vault");
    let typedefs_dir = vault.join(".mdvault").join("typedefs");
    let templates_dir = vault.join(".mdvault").join("templates");

    // Create custom typedef with order
    let typedef_path = typedefs_dir.join("ordered.lua");
    write(
        &typedef_path,
        r#"return {{
    name = "ordered",
    schema = {{
        title = {{ type = "string" }},
        status = {{ type = "string" }},
        date = {{ type = "string" }},
        tags = {{ type = "list" }}
    }},
    -- Force specific order: date first, then status, then title
    frontmatter_order = {{ "date", "status", "title" }}
}}"#, // Corrected: Removed unnecessary escaping of curly braces within raw string literal
    );

    let toml = format!(
        r##"version = 1
profile = "default"

[profiles.default]
vault_root = "{}"
templates_dir = "{}"
typedefs_dir = "{}"
captures_dir  = "{{{{vault_root}}}}\n.mdvault/captures"
macros_dir    = "{{{{vault_root}}}}\n.mdvault/macros"
"##,
        vault.display(),
        templates_dir.display(),
        typedefs_dir.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    let output = vault.join("ordered.md");

    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("XDG_CONFIG_HOME", &xdg);
    cmd.env("NO_COLOR", "1");
    cmd.args([
        "--config",
        cfg_path.to_str().unwrap(),
        "--profile",
        "default",
        "new",
        "ordered",
        "My Title",
        "--var",
        "date=2024-01-01",
        "--var",
        "status=draft",
        "--output",
        output.to_str().unwrap(),
    ]);

    cmd.assert().success();

    let content = fs::read_to_string(&output).unwrap();
    println!("Content:\n{}", content);

    // Verify order
    // We expect date, then status, then title
    let date_pos = content.find("date:").unwrap();
    let status_pos = content.find("status:").unwrap();
    let title_pos = content.find("title:").unwrap();

    assert!(date_pos < status_pos, "date should come before status");
    assert!(status_pos < title_pos, "status should come before title");
}
