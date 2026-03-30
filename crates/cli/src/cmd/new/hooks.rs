use crate::prompt::create_fuzzy_selector_callback;
use mdvault_core::captures::CaptureRepository;
use mdvault_core::config::types::ResolvedConfig;
use mdvault_core::frontmatter::{
    Frontmatter, ParsedDocument, parse as parse_frontmatter, serialize_with_order,
};
use mdvault_core::index::IndexDb;
use mdvault_core::macros::MacroRepository;
use mdvault_core::paths::PathResolver;
use mdvault_core::scripting::{
    HookResult, NoteContext, VaultContext, run_on_create_hook,
};
use mdvault_core::templates::repository::TemplateRepository;
use mdvault_core::types::{TypeDefinition, TypeRegistry, TypedefRepository};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::discovery::extract_note_type;

/// Run on_create hook if the note type has one defined.
/// Returns the HookResult which may contain modifications to apply.
pub(super) fn run_on_create_hook_if_exists(
    cfg: &ResolvedConfig,
    output_path: &Path,
    content: &str,
    explicit_typedef: Option<&TypeDefinition>,
    variables: &HashMap<String, String>,
) -> Result<HookResult, String> {
    let typedef_repo = match &cfg.typedefs_fallback_dir {
        Some(fallback) => TypedefRepository::with_fallback(&cfg.typedefs_dir, fallback),
        None => TypedefRepository::new(&cfg.typedefs_dir),
    }
    .map_err(|e| e.to_string())?;
    let type_registry =
        TypeRegistry::from_repository(&typedef_repo).map_err(|e| e.to_string())?;

    let typedef = if let Some(td) = explicit_typedef {
        if !td.has_on_create_hook {
            return Ok(HookResult {
                modified: false,
                frontmatter: None,
                content: None,
                variables: None,
            });
        }
        td.clone()
    } else {
        let note_type = match extract_note_type(content) {
            Some(t) => t,
            None => {
                return Ok(HookResult {
                    modified: false,
                    frontmatter: None,
                    content: None,
                    variables: None,
                });
            }
        };

        match type_registry.get(&note_type) {
            Some(td) if td.has_on_create_hook => (*td).clone(),
            _ => {
                return Ok(HookResult {
                    modified: false,
                    frontmatter: None,
                    content: None,
                    variables: None,
                });
            }
        }
    };

    let template_repo =
        TemplateRepository::new(&cfg.templates_dir).map_err(|e| e.to_string())?;
    let capture_repo =
        CaptureRepository::new(&cfg.captures_dir).map_err(|e| e.to_string())?;
    let macro_repo = MacroRepository::new(&cfg.macros_dir).map_err(|e| e.to_string())?;

    let index_db = IndexDb::open(&PathResolver::new(&cfg.vault_root).index_db())
        .ok()
        .map(std::sync::Arc::new);

    let mut vault_ctx = VaultContext::new(
        cfg.clone(),
        template_repo,
        capture_repo,
        macro_repo,
        type_registry,
    )
    .with_selector(create_fuzzy_selector_callback());

    if let Some(db) = index_db {
        vault_ctx = vault_ctx.with_index(db);
    }

    let parsed = parse_frontmatter(content).map_err(|e| e.to_string())?;

    let frontmatter = match parsed.frontmatter {
        Some(fm) => {
            let mut mapping = serde_yaml::Mapping::new();
            for (k, v) in fm.fields {
                mapping.insert(serde_yaml::Value::String(k), v);
            }
            serde_yaml::Value::Mapping(mapping)
        }
        None => serde_yaml::Value::Null,
    };

    let mut vars_mapping = serde_yaml::Mapping::new();
    for (k, v) in variables {
        vars_mapping.insert(
            serde_yaml::Value::String(k.clone()),
            serde_yaml::Value::String(v.clone()),
        );
    }
    let vars_value = serde_yaml::Value::Mapping(vars_mapping);

    let note_ctx = NoteContext::new(
        output_path.to_path_buf(),
        typedef.name.clone(),
        frontmatter,
        content.to_string(),
        vars_value,
    );

    run_on_create_hook(&typedef, &note_ctx, vault_ctx).map_err(|e| e.to_string())
}

/// Apply hook modifications to the output file.
pub(super) fn apply_hook_modifications(
    output_path: &Path,
    original_content: &str,
    hook_result: &HookResult,
    order: Option<&[String]>,
) -> Result<(), String> {
    if !hook_result.modified {
        return Ok(());
    }

    let original_parsed =
        parse_frontmatter(original_content).map_err(|e| e.to_string())?;

    let mut final_fields = if let Some(fm) = original_parsed.frontmatter {
        fm.fields
    } else {
        HashMap::new()
    };

    if let Some(serde_yaml::Value::Mapping(map)) = hook_result.frontmatter.as_ref() {
        for (k, v) in map {
            if let serde_yaml::Value::String(ks) = k {
                final_fields.insert(ks.clone(), v.clone());
            }
        }
    }

    let final_body = if let Some(ref new_content) = hook_result.content {
        let content_parsed = parse_frontmatter(new_content).map_err(|e| e.to_string())?;
        content_parsed.body
    } else {
        original_parsed.body
    };

    let doc = ParsedDocument {
        frontmatter: Some(Frontmatter { fields: final_fields }),
        body: final_body,
    };

    let final_content = serialize_with_order(&doc, order);

    fs::write(output_path, final_content).map_err(|e| e.to_string())
}
