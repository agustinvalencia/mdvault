use super::common::load_config;
use color_eyre::eyre::{Result, WrapErr};
use mdvault_core::templates::discovery::discover_templates;
use std::path::Path;

pub fn run(config: Option<&Path>, profile: Option<&str>) -> Result<()> {
    let rc = load_config(config, profile)?;

    let list =
        discover_templates(&rc.templates_dir).wrap_err("Failed to discover templates")?;

    if list.is_empty() {
        println!("(no templates found)");
        return Ok(());
    }
    for t in &list {
        println!("{}", t.logical_name);
    }
    println!("-- {} templates --", list.len());

    Ok(())
}
