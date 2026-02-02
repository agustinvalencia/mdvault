use std::path::Path;
use walkdir::WalkDir;

use super::types::{CaptureDiscoveryError, CaptureFormat, CaptureInfo};

/// Discover all Lua capture files in the given directory.
pub fn discover_captures(root: &Path) -> Result<Vec<CaptureInfo>, CaptureDiscoveryError> {
    let root = root
        .canonicalize()
        .map_err(|_| CaptureDiscoveryError::MissingDir(root.display().to_string()))?;

    if !root.exists() {
        return Err(CaptureDiscoveryError::MissingDir(root.display().to_string()));
    }

    let mut captures: Vec<CaptureInfo> = Vec::new();

    for entry in WalkDir::new(&root) {
        let entry = entry.map_err(|e| {
            CaptureDiscoveryError::WalkError(root.display().to_string(), e)
        })?;

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Only process .lua files
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) if n.ends_with(".lua") => n,
            _ => continue,
        };

        let logical = name.strip_suffix(".lua").unwrap().to_string();

        let rel = path.strip_prefix(&root).unwrap_or(path);
        let full_logical =
            if rel.parent().map(|p| p.as_os_str().is_empty()).unwrap_or(true) {
                logical
            } else {
                let parent = rel.parent().unwrap().to_string_lossy();
                format!("{}/{}", parent, logical)
            };

        captures.push(CaptureInfo {
            logical_name: full_logical,
            path: path.to_path_buf(),
            format: CaptureFormat::Lua,
        });
    }

    captures.sort_by(|a, b| a.logical_name.cmp(&b.logical_name));
    Ok(captures)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_discover_captures_lua() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        File::create(root.join("inbox.lua")).unwrap();
        File::create(root.join("todo.lua")).unwrap();
        // Non-lua files should be ignored
        File::create(root.join("ignored.txt")).unwrap();
        File::create(root.join("README.md")).unwrap();
        File::create(root.join("old.yaml")).unwrap(); // YAML no longer supported

        let captures = discover_captures(root).unwrap();

        assert_eq!(captures.len(), 2);
        assert_eq!(captures[0].logical_name, "inbox");
        assert_eq!(captures[0].format, CaptureFormat::Lua);
        assert_eq!(captures[1].logical_name, "todo");
        assert_eq!(captures[1].format, CaptureFormat::Lua);
    }

    #[test]
    fn test_discover_captures_nested() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        std::fs::create_dir(root.join("subdir")).unwrap();
        File::create(root.join("subdir/inbox.lua")).unwrap();
        File::create(root.join("subdir/todo.lua")).unwrap();

        let captures = discover_captures(root).unwrap();

        assert_eq!(captures.len(), 2);
        assert_eq!(captures[0].logical_name, "subdir/inbox");
        assert_eq!(captures[0].format, CaptureFormat::Lua);
        assert_eq!(captures[1].logical_name, "subdir/todo");
        assert_eq!(captures[1].format, CaptureFormat::Lua);
    }

    #[test]
    fn test_discover_captures_missing_dir() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("missing");

        let result = discover_captures(&missing);
        assert!(result.is_err());
    }
}
