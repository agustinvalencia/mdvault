use markadd_core::config::loader::{ConfigLoader, default_config_path};
use markadd_core::templates::discovery::discover_templates;
use std::path::Path;

pub fn run(config: Option<&Path>, profile: Option<&str>) {
    let rc = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            println!("FAIL markadd list-templates");
            println!("{e}");
            if config.is_none() {
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
