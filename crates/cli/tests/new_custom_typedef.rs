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
fn new_custom_type_respects_output_path() {
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

    // Create custom typedef
    let typedef_path = typedefs_dir.join("briefing.lua");
    write(
        &typedef_path,
        r#"return {
    name = "briefing",
    output = "briefings/{{title | slugify}}.md",
    schema = {
        title = { type = "string", required = true },
    }
}"#,
    );

    // Create template (optional but good practice)
    let template_path = templates_dir.join("briefing.md");
    write(
        &template_path,
        "---\ntype: briefing\ntitle: {{title}}\n---\n# Meeting: {{title}}",
    );

    let toml = format!(
        "version = 1\n\
        profile = \"default\"\n\
        \n\
        [profiles.default]\n\
        vault_root = \"{}\"\n\
        templates_dir = \"{}\"\n\
        typedefs_dir = \"{}\"\n\
        captures_dir  = \"{{{{vault_root}}}}/.mdvault/captures\"\n\
        macros_dir    = \"{{{{vault_root}}}}/.mdvault/macros\"\n",
        vault.display(),
        templates_dir.display(),
        typedefs_dir.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    // Expected output path based on typedef
    let expected_output = vault.join("briefings").join("team-sync.md");

    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("XDG_CONFIG_HOME", &xdg);
    cmd.env("NO_COLOR", "1");
    cmd.args([
        "--config",
        cfg_path.to_str().unwrap(),
        "--profile",
        "default",
        "new",
        "briefing",
        "Team Sync",
    ]);

    let output = cmd.output().unwrap();

    // Print stdout/stderr for debugging if it fails
    if !output.status.success() {
        println!("Stdout: {}", String::from_utf8_lossy(&output.stdout));
        println!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    assert!(output.status.success());
    assert!(
        expected_output.exists(),
        "File should exist at {}",
        expected_output.display()
    );
}

#[test]
fn new_custom_type_validation_fails() {
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

    // Create custom typedef with strict validation (custom validator in lua)
    let typedef_path = typedefs_dir.join("report.lua");
    write(
        &typedef_path,
        r#"return {
    name = "report",
    output = "reports/{{title | slugify}}.md",
    schema = {
        title = { type = "string", required = true },
        status = { type = "string", required = true, enum = {"draft", "final"} }
    },
    validate = function(note)
        if note.frontmatter.status == "draft" and not string.match(note.body, "DRAFT") then
            return "Draft reports must contain DRAFT marker in body"
        end
        return nil
    end
}"#,
    );

    // Create template that produces invalid content (missing DRAFT marker)
    let template_path = templates_dir.join("report.md");
    write(&template_path, "---\ntype: report\ntitle: {{title}}\nstatus: draft\n---\n# Report: {{title}}\n\nSome content.");

    let toml = format!(
        "version = 1\n\
        profile = \"default\"\n\
        \n\
        [profiles.default]\n\
        vault_root = \"{}\"\n\
        templates_dir = \"{}\"\n\
        typedefs_dir = \"{}\"\n\
        captures_dir  = \"{{{{vault_root}}}}/.mdvault/captures\"\n\
        macros_dir    = \"{{{{vault_root}}}}/.mdvault/macros\"\n",
        vault.display(),
        templates_dir.display(),
        typedefs_dir.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("XDG_CONFIG_HOME", &xdg);
    cmd.env("NO_COLOR", "1");
    cmd.args([
        "--config",
        cfg_path.to_str().unwrap(),
        "--profile",
        "default",
        "new",
        "report",
        "My Report",
    ]);

    let output = cmd.output().unwrap();

    // Should fail validation
    if output.status.success() {
        println!(
            "Unexpected success! Stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    } else {
        println!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Validation failed"));
    assert!(stderr.contains("Draft reports must contain DRAFT marker"));
}
