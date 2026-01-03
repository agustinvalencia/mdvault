//! Shell completion support with dynamic value completers.
//!
//! This module provides intelligent tab completions that understand the user's vault
//! configuration, offering context-aware suggestions for types, templates, captures, etc.

use clap_complete::engine::CompletionCandidate;
use mdvault_core::captures::CaptureRepository;
use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::macros::MacroRepository;
use mdvault_core::templates::repository::TemplateRepository;
use mdvault_core::types::{TypeRegistry, TypedefRepository};
use std::ffi::OsStr;

/// Load the resolved config, returning None if it fails.
fn load_config() -> Option<mdvault_core::config::types::ResolvedConfig> {
    ConfigLoader::load(None, None).ok()
}

/// Complete note types (built-in + custom from TypeRegistry).
pub fn complete_types(current: &OsStr) -> Vec<CompletionCandidate> {
    let mut completions = vec![];
    let current_str = current.to_str().unwrap_or("");

    // Built-in types are always available
    let builtin_types = [
        ("daily", "Daily journal notes"),
        ("weekly", "Weekly overview notes"),
        ("task", "Individual actionable tasks"),
        ("project", "Collections of related tasks"),
        ("zettel", "Knowledge notes (Zettelkasten-style)"),
    ];

    for (name, help) in builtin_types {
        if name.starts_with(current_str) {
            completions.push(CompletionCandidate::new(name).help(Some(help.into())));
        }
    }

    // Try to load custom types from config
    if let Some(cfg) = load_config() {
        if let Ok(typedef_repo) = TypedefRepository::new(&cfg.typedefs_dir) {
            if let Ok(registry) = TypeRegistry::from_repository(&typedef_repo) {
                for type_name in registry.list_custom_types() {
                    if type_name.starts_with(current_str) {
                        completions.push(
                            CompletionCandidate::new(type_name)
                                .help(Some("Custom type".into())),
                        );
                    }
                }
            }
        }
    }

    completions
}

/// Complete template names from TemplateRepository.
pub fn complete_templates(current: &OsStr) -> Vec<CompletionCandidate> {
    let mut completions = vec![];
    let current_str = current.to_str().unwrap_or("");

    if let Some(cfg) = load_config() {
        if let Ok(repo) = TemplateRepository::new(&cfg.templates_dir) {
            for info in repo.list_all() {
                if info.logical_name.starts_with(current_str) {
                    completions.push(CompletionCandidate::new(&info.logical_name));
                }
            }
        }
    }

    completions
}

/// Complete capture names from CaptureRepository.
pub fn complete_captures(current: &OsStr) -> Vec<CompletionCandidate> {
    let mut completions = vec![];
    let current_str = current.to_str().unwrap_or("");

    if let Some(cfg) = load_config() {
        if let Ok(repo) = CaptureRepository::new(&cfg.captures_dir) {
            for info in repo.list_all() {
                if info.logical_name.starts_with(current_str) {
                    completions.push(CompletionCandidate::new(&info.logical_name));
                }
            }
        }
    }

    completions
}

/// Complete macro names from MacroRepository.
pub fn complete_macros(current: &OsStr) -> Vec<CompletionCandidate> {
    let mut completions = vec![];
    let current_str = current.to_str().unwrap_or("");

    if let Some(cfg) = load_config() {
        if let Ok(repo) = MacroRepository::new(&cfg.macros_dir) {
            for info in repo.list_all() {
                if info.logical_name.starts_with(current_str) {
                    completions.push(CompletionCandidate::new(&info.logical_name));
                }
            }
        }
    }

    completions
}

/// Complete note paths from the vault.
/// This walks the vault directory and returns markdown files.
pub fn complete_notes(current: &OsStr) -> Vec<CompletionCandidate> {
    let mut completions = vec![];
    let current_str = current.to_str().unwrap_or("");

    if let Some(cfg) = load_config() {
        // Walk and collect note paths relative to vault root
        for entry in walkdir::WalkDir::new(&cfg.vault_root)
            .min_depth(1)
            .max_depth(5) // Limit depth for performance
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
            .take(100)
        // Limit results for performance
        {
            if let Ok(rel_path) = entry.path().strip_prefix(&cfg.vault_root) {
                let path_str = rel_path.to_string_lossy();
                if path_str.starts_with(current_str) {
                    completions.push(CompletionCandidate::new(path_str.to_string()));
                }
            }
        }
    }

    completions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_types_builtin() {
        let completions = complete_types(OsStr::new(""));
        let names: Vec<_> =
            completions.iter().map(|c| c.get_value().to_str().unwrap()).collect();

        assert!(names.contains(&"daily"));
        assert!(names.contains(&"task"));
        assert!(names.contains(&"project"));
    }

    #[test]
    fn test_complete_types_prefix_filter() {
        let completions = complete_types(OsStr::new("da"));
        let names: Vec<_> =
            completions.iter().map(|c| c.get_value().to_str().unwrap()).collect();

        assert!(names.contains(&"daily"));
        assert!(!names.contains(&"task"));
    }
}
