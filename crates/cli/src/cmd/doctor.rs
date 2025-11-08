use markadd_core::config::loader::{ConfigLoader, default_config_path};
use std::path::Path;

pub fn run(config: Option<&Path>, profile: Option<&str>) {
    match ConfigLoader::load(config, profile) {
        Ok(rc) => {
            println!("OK   markadd doctor");
            println!(
                "path: {}",
                config
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| default_config_path().display().to_string())
            );
            println!("profile: {}", rc.active_profile);
            println!("vault_root: {}", rc.vault_root.display());
            println!("templates_dir: {}", rc.templates_dir.display());
            println!("captures_dir: {}", rc.captures_dir.display());
            println!("macros_dir: {}", rc.macros_dir.display());
            println!("security.allow_shell: {}", rc.security.allow_shell);
            println!("security.allow_http:  {}", rc.security.allow_http);
        }
        Err(e) => {
            println!("FAIL markadd doctor");
            println!("{e}");
            if config.is_none() {
                println!("looked for: {}", default_config_path().display());
            }
            std::process::exit(1);
        }
    }
}
