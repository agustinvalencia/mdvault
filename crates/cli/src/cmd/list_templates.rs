use super::common::load_config;
use mdvault_core::templates::discovery::discover_templates;
use std::path::Path;

pub fn run(config: Option<&Path>, profile: Option<&str>) {
    let rc = load_config(config, profile);

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
            println!("FAIL mdv list-templates");
            println!("{e}");
            std::process::exit(1);
        }
    }
}
