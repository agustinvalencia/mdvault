use markadd_core::config::loader::{ConfigLoader, default_config_path};
use std::path::PathBuf;

pub fn run(rest: Vec<String>) {
    let mut cfg_path: Option<PathBuf> = None;
    let mut profile: Option<String> = None;

    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--config" => {
                if let Some(p) = it.next() {
                    cfg_path = Some(PathBuf::from(p));
                } else {
                    eprintln!("--config expects a path");
                    std::process::exit(2);
                }
            }
            "--profile" => {
                if let Some(p) = it.next() {
                    profile = Some(p.to_string());
                } else {
                    eprintln!("--profile expects a name");
                    std::process::exit(2);
                }
            }
            _ => {
                eprintln!("unknown flag for doctor: {arg}");
                eprintln!("usage: markadd doctor [--config <path>] [--profile <name>]");
                std::process::exit(2);
            }
        }
    }

    let cfg_path_ref = cfg_path.as_deref();
    match ConfigLoader::load(cfg_path_ref, profile.as_deref()) {
        Ok(rc) => {
            println!("OK   markadd doctor");
            println!(
                "path: {}",
                cfg_path.map_or_else(
                    || default_config_path().display().to_string(),
                    |p| p.display().to_string()
                )
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
            if cfg_path_ref.is_none() {
                println!("looked for: {}", default_config_path().display());
            }
            std::process::exit(1);
        }
    }
}
