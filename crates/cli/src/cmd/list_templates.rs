// crates/cli/src/cmd/list_templates.rs
use markadd_core::config::loader::{default_config_path, ConfigLoader};
use markadd_core::templates::discovery::discover_templates;
use std::path::PathBuf;

/// ultra-light flag parser for this single command
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
                eprintln!("unknown flag for list-templates: {arg}");
                eprintln!("usage: markadd list-templates [--config <path>] [--profile <name>]");
                std::process::exit(2);
            }
        }
    }

    let cfg_path_ref = cfg_path.as_deref();
    let rc = match ConfigLoader::load(cfg_path_ref, profile.as_deref()) {
        Ok(rc) => rc,
        Err(e) => {
            println!("FAIL markadd list-templates");
            println!("{e}");
            if cfg_path_ref.is_none() {
                println!("looked for: {}", default_config_path().display());
            }
            std::process::exit(1);
        }
    };

    match discover_templates(&rc.templates_dir) {
        Ok(list) => {
            if list.is_empty() {
                println!("(no templates found)");
                return;
            }
            for t in &list {
                println!("{}", t.logical_name);
            }
            println!("-- {} templates --", list.len());
        }
        Err(e) => {
            println!("FAIL markadd list-templates");
            println!("{e}");
            std::process::exit(1);
        }
    }
}
