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
fn new_renders_template_to_output_file() {
    let tmp = tempdir().unwrap();

    // XDG config
    let xdg = tmp.path().join("xdg");
    let cfg_dir = xdg.join("mdvault");
    let cfg_path = cfg_dir.join("config.toml");
    fs::create_dir_all(&cfg_dir).unwrap();

    // Vault and templates
    let vault = tmp.path().join("vault");
    let tpl_root = vault.join(".mdvault").join("templates");
    let tpl_daily = tpl_root.join("daily.md");

    write(
        &tpl_daily,
        "Title: {{template_name}}\nVault: {{vault_root}}\nFile: {{output_filename}}\n",
    );

    let toml = format!(
        r#"
version = 1
profile = "default"

[profiles.default]
vault_root = "{vault}"
templates_dir = "{tpl}"
captures_dir  = "{{{{vault_root}}}}/.mdvault/captures"
macros_dir    = "{{{{vault_root}}}}/.mdvault/macros"
"#,
        vault = vault.display(),
        tpl = tpl_root.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    let output = vault.join("rendered.md");

    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("XDG_CONFIG_HOME", &xdg);
    cmd.env("NO_COLOR", "1");
    cmd.args([
        "--config",
        cfg_path.to_str().unwrap(),
        "--profile",
        "default",
        "new",
        "--template",
        "daily",
        "--output",
        output.to_str().unwrap(),
    ]);

    cmd.assert()
        .success()
        .stdout(predicates::str::contains("OK   mdv new"))
        .stdout(predicates::str::contains("template: daily"));

    let rendered = fs::read_to_string(&output).unwrap();

    assert!(rendered.contains("Title: daily"));
    assert!(rendered.contains("Vault:"));
    assert!(rendered.contains("File: rendered.md"));
}
