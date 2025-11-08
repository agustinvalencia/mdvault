use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateInfo {
    pub logical_name: String,
    pub path: PathBuf,
}

#[derive(Debug, Error)]
pub enum TemplateDiscoveryError {
    #[error("templates directory does not exist: {0}")]
    MissingDir(String),

    #[error("failed to read templates directory {0} : {1}")]
    WalkError(String, #[source] walkdir::Error),
}

pub fn discover_templates(
    root: &Path,
) -> Result<Vec<TemplateInfo>, TemplateDiscoveryError> {
    let root = root
        .canonicalize()
        .map_err(|_| TemplateDiscoveryError::MissingDir(root.display().to_string()))?;

    if !root.exists() {
        return Err(TemplateDiscoveryError::MissingDir(root.display().to_string()));
    }

    let mut out = Vec::new();

    for entry in WalkDir::new(&root) {
        let entry = entry.map_err(|e| {
            TemplateDiscoveryError::WalkError(root.display().to_string(), e)
        })?;

        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !is_markdown_file(path) {
            continue;
        }

        let rel = path.strip_prefix(&root).unwrap_or(path);
        let logical = logical_name_from_relative(rel);

        out.push(TemplateInfo { logical_name: logical, path: path.to_path_buf() });
    }

    out.sort_by(|a, b| a.logical_name.cmp(&b.logical_name));
    Ok(out)
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()).map(|ext| ext == "md").unwrap_or(false)
}

fn logical_name_from_relative(rel: &Path) -> String {
    let s = rel.to_string_lossy();
    let suffix = ".md";
    if s.ends_with(suffix) {
        let cut = s.len() - suffix.len();
        return s[..cut].to_string();
    }
    s.to_string()
}
