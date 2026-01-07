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
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    name.ends_with(".md") && !(name.ends_with(".tpl.md") || name.ends_with(".tmpl.md"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_discover_templates_simple() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        File::create(root.join("daily.md")).unwrap();
        File::create(root.join("meeting.md")).unwrap();
        File::create(root.join("readme.txt")).unwrap(); // Should be ignored

        let templates = discover_templates(root).unwrap();

        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].logical_name, "daily");
        assert_eq!(templates[1].logical_name, "meeting");
    }

    #[test]
    fn test_discover_templates_ignores_partials() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        File::create(root.join("valid.md")).unwrap();
        File::create(root.join("partial.tpl.md")).unwrap();
        File::create(root.join("other.tmpl.md")).unwrap();

        let templates = discover_templates(root).unwrap();

        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].logical_name, "valid");
    }

    #[test]
    fn test_discover_templates_nested() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        std::fs::create_dir(root.join("work")).unwrap();
        File::create(root.join("work/report.md")).unwrap();

        let templates = discover_templates(root).unwrap();

        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].logical_name, "work/report");
    }

    #[test]
    fn test_discover_templates_missing_dir() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("missing");

        let result = discover_templates(&missing);
        assert!(matches!(result, Err(TemplateDiscoveryError::MissingDir(_))));
    }
}
