use std::path::Path;
use walkdir::WalkDir;

use super::types::{CaptureDiscoveryError, CaptureInfo};

/// Discover all capture YAML files in the given directory
pub fn discover_captures(root: &Path) -> Result<Vec<CaptureInfo>, CaptureDiscoveryError> {
    let root = root
        .canonicalize()
        .map_err(|_| CaptureDiscoveryError::MissingDir(root.display().to_string()))?;

    if !root.exists() {
        return Err(CaptureDiscoveryError::MissingDir(root.display().to_string()));
    }

    let mut out = Vec::new();

    for entry in WalkDir::new(&root) {
        let entry = entry.map_err(|e| {
            CaptureDiscoveryError::WalkError(root.display().to_string(), e)
        })?;

        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !is_yaml_file(path) {
            continue;
        }

        let rel = path.strip_prefix(&root).unwrap_or(path);
        let logical = logical_name_from_relative(rel);

        out.push(CaptureInfo { logical_name: logical, path: path.to_path_buf() });
    }

    out.sort_by(|a, b| a.logical_name.cmp(&b.logical_name));
    Ok(out)
}

fn is_yaml_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    name.ends_with(".yaml") || name.ends_with(".yml")
}

fn logical_name_from_relative(rel: &Path) -> String {
    let s = rel.to_string_lossy();
    // Remove .yaml or .yml extension
    if let Some(stripped) = s.strip_suffix(".yaml") {
        return stripped.to_string();
    }
    if let Some(stripped) = s.strip_suffix(".yml") {
        return stripped.to_string();
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_discover_captures_simple() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create some files
        File::create(root.join("todo.yaml")).unwrap();
        File::create(root.join("ideas.yml")).unwrap();
        File::create(root.join("ignored.txt")).unwrap();
        File::create(root.join("README.md")).unwrap();

        let captures = discover_captures(root).unwrap();

        assert_eq!(captures.len(), 2);
        assert_eq!(captures[0].logical_name, "ideas");
        assert_eq!(captures[1].logical_name, "todo");
    }

    #[test]
    fn test_discover_captures_nested() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        std::fs::create_dir(root.join("subdir")).unwrap();
        File::create(root.join("subdir/nested.yaml")).unwrap();

        let captures = discover_captures(root).unwrap();

        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].logical_name, "subdir/nested");
    }

    #[test]
    fn test_discover_captures_missing_dir() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("missing");

        let result = discover_captures(&missing);
        assert!(result.is_err());
    }
}
