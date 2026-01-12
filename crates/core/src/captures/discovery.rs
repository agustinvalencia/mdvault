use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

use super::types::{CaptureDiscoveryError, CaptureFormat, CaptureInfo};

/// Discover all capture files (Lua and YAML) in the given directory.
///
/// Lua files take precedence over YAML files with the same name.
pub fn discover_captures(root: &Path) -> Result<Vec<CaptureInfo>, CaptureDiscoveryError> {
    let root = root
        .canonicalize()
        .map_err(|_| CaptureDiscoveryError::MissingDir(root.display().to_string()))?;

    if !root.exists() {
        return Err(CaptureDiscoveryError::MissingDir(root.display().to_string()));
    }

    // Use a map to handle Lua/YAML precedence (Lua wins)
    let mut captures: HashMap<String, CaptureInfo> = HashMap::new();

    for entry in WalkDir::new(&root) {
        let entry = entry.map_err(|e| {
            CaptureDiscoveryError::WalkError(root.display().to_string(), e)
        })?;

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let (logical, format) = match get_capture_format(path) {
            Some((l, f)) => (l, f),
            None => continue,
        };

        let rel = path.strip_prefix(&root).unwrap_or(path);
        let full_logical = if rel.parent().map(|p| p.as_os_str().is_empty()).unwrap_or(true) {
            logical.clone()
        } else {
            let parent = rel.parent().unwrap().to_string_lossy();
            format!("{}/{}", parent, logical)
        };

        // Lua takes precedence over YAML
        match captures.get(&full_logical) {
            Some(existing) if existing.format == CaptureFormat::Lua => {
                // Keep existing Lua file
                continue;
            }
            _ => {
                captures.insert(
                    full_logical.clone(),
                    CaptureInfo {
                        logical_name: full_logical,
                        path: path.to_path_buf(),
                        format,
                    },
                );
            }
        }
    }

    let mut out: Vec<CaptureInfo> = captures.into_values().collect();
    out.sort_by(|a, b| a.logical_name.cmp(&b.logical_name));
    Ok(out)
}

/// Get the capture format and logical name from a file path.
/// Returns None if the file is not a capture file.
fn get_capture_format(path: &Path) -> Option<(String, CaptureFormat)> {
    let name = path.file_name().and_then(|s| s.to_str())?;

    if name.ends_with(".lua") {
        let logical = name.strip_suffix(".lua")?.to_string();
        Some((logical, CaptureFormat::Lua))
    } else if name.ends_with(".yaml") {
        let logical = name.strip_suffix(".yaml")?.to_string();
        Some((logical, CaptureFormat::Yaml))
    } else if name.ends_with(".yml") {
        let logical = name.strip_suffix(".yml")?.to_string();
        Some((logical, CaptureFormat::Yaml))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_discover_captures_yaml() {
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
        assert_eq!(captures[0].format, CaptureFormat::Yaml);
        assert_eq!(captures[1].logical_name, "todo");
        assert_eq!(captures[1].format, CaptureFormat::Yaml);
    }

    #[test]
    fn test_discover_captures_lua() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        File::create(root.join("inbox.lua")).unwrap();
        File::create(root.join("todo.lua")).unwrap();

        let captures = discover_captures(root).unwrap();

        assert_eq!(captures.len(), 2);
        assert_eq!(captures[0].logical_name, "inbox");
        assert_eq!(captures[0].format, CaptureFormat::Lua);
        assert_eq!(captures[1].logical_name, "todo");
        assert_eq!(captures[1].format, CaptureFormat::Lua);
    }

    #[test]
    fn test_discover_captures_lua_precedence() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Both Lua and YAML with same name - Lua should win
        File::create(root.join("inbox.lua")).unwrap();
        File::create(root.join("inbox.yaml")).unwrap();
        File::create(root.join("todo.yaml")).unwrap(); // Only YAML

        let captures = discover_captures(root).unwrap();

        assert_eq!(captures.len(), 2);

        let inbox = captures.iter().find(|c| c.logical_name == "inbox").unwrap();
        assert_eq!(inbox.format, CaptureFormat::Lua);

        let todo = captures.iter().find(|c| c.logical_name == "todo").unwrap();
        assert_eq!(todo.format, CaptureFormat::Yaml);
    }

    #[test]
    fn test_discover_captures_nested() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        std::fs::create_dir(root.join("subdir")).unwrap();
        File::create(root.join("subdir/nested.yaml")).unwrap();
        File::create(root.join("subdir/inbox.lua")).unwrap();

        let captures = discover_captures(root).unwrap();

        assert_eq!(captures.len(), 2);
        assert_eq!(captures[0].logical_name, "subdir/inbox");
        assert_eq!(captures[0].format, CaptureFormat::Lua);
        assert_eq!(captures[1].logical_name, "subdir/nested");
        assert_eq!(captures[1].format, CaptureFormat::Yaml);
    }

    #[test]
    fn test_discover_captures_missing_dir() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("missing");

        let result = discover_captures(&missing);
        assert!(result.is_err());
    }
}
