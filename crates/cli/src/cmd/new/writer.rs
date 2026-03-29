use mdvault_core::config::types::ResolvedConfig;
use mdvault_core::frontmatter::parse as parse_frontmatter;
use mdvault_core::index::{IndexBuilder, IndexDb};
use mdvault_core::types::{try_fix_note, validate_note_for_creation, TypeRegistry};
use std::fs;
use std::path::Path;

/// Force a vault reindex to include newly created notes.
pub(super) fn reindex_vault(cfg: &ResolvedConfig) {
    let index_path = cfg.vault_root.join(".mdvault/index.db");

    if let Some(parent) = index_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    match IndexDb::open(&index_path) {
        Ok(db) => {
            let builder = IndexBuilder::with_exclusions(
                &db,
                &cfg.vault_root,
                cfg.excluded_folders.clone(),
            );
            if let Err(e) = builder.incremental_reindex(None) {
                eprintln!("Warning: reindex failed: {e}");
            }
        }
        Err(e) => {
            eprintln!("Warning: could not open index for reindex: {e}");
        }
    }
}

/// Validate note content before writing.
///
/// Returns Ok(None) if valid, Ok(Some(content)) if valid after auto-fixing,
/// or Err with error messages if validation fails.
pub(super) fn validate_before_write(
    registry: &TypeRegistry,
    note_type: &str,
    output_path: &Path,
    content: &str,
) -> Result<Option<String>, Vec<String>> {
    let parsed = match parse_frontmatter(content) {
        Ok(p) => p,
        Err(e) => return Err(vec![format!("Failed to parse frontmatter: {}", e)]),
    };

    let frontmatter = match parsed.frontmatter {
        Some(fm) => {
            let mut mapping = serde_yaml::Mapping::new();
            for (k, v) in fm.fields {
                mapping.insert(serde_yaml::Value::String(k), v);
            }
            serde_yaml::Value::Mapping(mapping)
        }
        None => serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
    };

    let path_str = output_path.to_string_lossy();
    let result = validate_note_for_creation(
        registry,
        note_type,
        &path_str,
        &frontmatter,
        &parsed.body,
    );

    if result.valid {
        Ok(None)
    } else {
        let fix_result = try_fix_note(registry, note_type, content, &result.errors);
        if fix_result.fixed {
            if let Some(new_content) = fix_result.content {
                println!("Auto-fixed validation errors:");
                for fix in fix_result.fixes {
                    println!("  - {}", fix);
                }
                Ok(Some(new_content))
            } else {
                let errors: Vec<String> =
                    result.errors.iter().map(|e| e.to_string()).collect();
                Err(errors)
            }
        } else {
            let errors: Vec<String> =
                result.errors.iter().map(|e| e.to_string()).collect();
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_before_write_bad_yaml() {
        let registry = TypeRegistry::new();
        let path = Path::new("foo.md");
        let content = "---\n: invalid\n---\nbody";
        let result = validate_before_write(&registry, "task", path, content);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(!errs.is_empty());
        assert!(errs[0].contains("Failed to parse frontmatter"));
    }
}
