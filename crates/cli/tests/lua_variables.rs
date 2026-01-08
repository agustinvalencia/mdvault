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
fn lua_hook_can_modify_variables_and_rerender() {
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
    let lua_path = typedefs_dir.join("meeting.lua");
    
    // Templates
    let tpl_root = vault.join(".mdvault/templates");
    let tpl_meeting = tpl_root.join("meeting.md");

    // 1. Create Template
    write(
        &tpl_meeting,
        "---\ntype: meeting\n---\n# Meeting with {{ host }}\n",
    );

    // Create captures and macros dirs to satisfy VaultContext
    fs::create_dir_all(vault.join(".mdvault/captures")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/macros")).unwrap();

    // 2. Create Lua Type Definition with Hook
    let lua_source = r#" 
return {
    name = "meeting",
    description = "Meeting notes",
    on_create = function(note)
        -- Override the 'host' variable
        note.variables.host = "LuaHost"
        return note
    end
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
typedefs_dir  = "{typedefs}"
captures_dir  = "{{{{vault_root}}}}/.mdvault/captures"
macros_dir    = "{{{{vault_root}}}}/.mdvault/macros"
"#,
        vault = vault.display(),
        tpl = tpl_root.display(),
        typedefs = typedefs_dir.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    let output = vault.join("meeting-note.md");

    // 4. Run mdv new
    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("XDG_CONFIG_HOME", &xdg);
    cmd.env("NO_COLOR", "1");
    cmd.args([
        "--config",
        cfg_path.to_str().unwrap(),
        "new",
        "meeting",
        "My Meeting",
        "--var",
        "host=OriginalHost",
        "--output",
        output.to_str().unwrap(),
    ]);

    cmd.assert()
        .success();

    // 5. Verify Output
    let rendered = fs::read_to_string(&output).unwrap();
    
    // Should contain "Meeting with LuaHost" not "Meeting with OriginalHost"
    assert!(rendered.contains("Meeting with LuaHost"), "Expected 'Meeting with LuaHost', found:\n{}", rendered);
    assert!(!rendered.contains("OriginalHost"), "Should not contain 'OriginalHost'");
}
