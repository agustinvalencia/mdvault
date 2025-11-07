use std::path::PathBuf;
use walkdir::WalkDir;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateInfo {
    pub logical_name: String,
    pub path: PathBuf
}

#[derive(Debug, Error)]
pub enum TemplateDiscoveryError {
    #[error("templates directory does not exist: {0}")]
    MissingDir(String),

    #[error("failed to read templates directory {0} : {1}")]
    WalkError(String, #[source] walkdir::Error)
}

pub fn discover_templates(root: &Path){
    todo!()
}

fn is_template_file(path: &Path){
    todo!()
}

fn logical_name_from_relative(rel: &Path) -> String {
    todo!()
}
