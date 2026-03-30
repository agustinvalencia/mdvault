use color_eyre::eyre::{Result, WrapErr, bail};

use crate::prompt::{CollectedVars, PromptOptions, prompt_for_enum, prompt_for_field};
use dialoguer::{Editor, FuzzySelect, Input, Select, theme::ColorfulTheme};
use mdvault_core::config::types::ResolvedConfig;
use mdvault_core::index::{IndexDb, NoteQuery, NoteType};
use mdvault_core::paths::PathResolver;
use mdvault_core::types::{TypeDefinition, TypeRegistry};
use std::collections::HashMap;

/// Dispatch type-specific prompts to interactive widgets and collect values.
pub(super) fn dispatch_type_prompts(
    prompts: Vec<mdvault_core::domain::FieldPrompt>,
    vars: &mut HashMap<String, String>,
    cfg: &ResolvedConfig,
    batch_mode: bool,
) -> Result<()> {
    for prompt in prompts {
        if vars.contains_key(&prompt.field_name) {
            continue;
        }

        if batch_mode {
            if let Some(default) = prompt.default_value {
                vars.insert(prompt.field_name, default);
            }
        } else {
            match &prompt.prompt_type {
                mdvault_core::domain::PromptType::ProjectSelector => {
                    match prompt_project_selection(cfg) {
                        Some(project) => {
                            vars.insert("project".to_string(), project);
                        }
                        None => {
                            if let Some(default) = prompt.default_value {
                                vars.insert(prompt.field_name, default);
                            } else {
                                bail!("No project selected");
                            }
                        }
                    }
                }
                mdvault_core::domain::PromptType::Text => {
                    match prompt_for_field(
                        &prompt.field_name,
                        &prompt.prompt_text,
                        prompt.default_value.as_deref(),
                        prompt.required,
                    ) {
                        Ok(value) => {
                            vars.insert(prompt.field_name, value);
                        }
                        Err(_) if prompt.default_value.is_some() => {
                            vars.insert(prompt.field_name, prompt.default_value.unwrap());
                        }
                        Err(e) => {
                            bail!("Error: {e}");
                        }
                    }
                }
                mdvault_core::domain::PromptType::Multiline => {
                    if let Some(text) = Editor::new().edit("").ok().flatten() {
                        vars.insert(prompt.field_name, text);
                    }
                }
                mdvault_core::domain::PromptType::Select(options) => {
                    match prompt_for_enum(
                        &prompt.field_name,
                        &prompt.prompt_text,
                        options,
                        prompt.default_value.as_deref(),
                    ) {
                        Ok(value) => {
                            vars.insert(prompt.field_name, value);
                        }
                        Err(_) if prompt.default_value.is_some() => {
                            vars.insert(prompt.field_name, prompt.default_value.unwrap());
                        }
                        Err(e) => {
                            bail!("Error: {e}");
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Resolve title when not explicitly provided.
pub(super) fn resolve_title_or_default(
    effective_name: &str,
    required: bool,
    batch_mode: bool,
    type_registry: &Option<TypeRegistry>,
) -> Result<String> {
    let title_default =
        type_registry.as_ref().and_then(|reg| reg.get(effective_name)).and_then(|td| {
            td.schema.get("title").and_then(|fs| {
                fs.default.as_ref().and_then(|v| match v {
                    serde_yaml::Value::String(s) => Some(s.clone()),
                    _ => None,
                })
            })
        });

    if let Some(default_title) = title_default {
        return Ok(default_title);
    }

    if !required {
        return Ok(String::new());
    }

    if batch_mode {
        bail!(
            "Title is required in batch mode\nUsage: mdv new {effective_name} \"Title\""
        );
    }

    prompt_for_field("title", "Note title", None, true).wrap_err("Failed to read title")
}

/// Collect variables from Lua schema fields that have `prompt` set.
pub(super) fn collect_schema_variables(
    typedef: &TypeDefinition,
    provided_vars: &HashMap<String, String>,
    options: &PromptOptions,
    cfg: Option<&ResolvedConfig>,
) -> Result<CollectedVars> {
    let mut result = CollectedVars {
        values: HashMap::new(),
        prompted: Vec::new(),
        defaulted: Vec::new(),
    };

    for (k, v) in provided_vars {
        result.values.insert(k.clone(), v.clone());
    }

    let mut fields: Vec<_> = typedef.schema.iter().collect();
    fields.sort_by(|a, b| a.0.cmp(b.0));

    for (field_name, schema) in fields {
        if result.values.contains_key(field_name) {
            continue;
        }

        if schema.core && schema.prompt.is_none() && schema.default.is_none() {
            continue;
        }

        if let Some(ref selector_type) = schema.selector {
            if options.batch_mode {
                if let Some(ref default) = schema.default {
                    let value = yaml_value_to_string(default);
                    result.values.insert(field_name.clone(), value);
                    result.defaulted.push(field_name.clone());
                } else if schema.required {
                    bail!(
                        "Missing required field '{}' in batch mode (selector field)",
                        field_name
                    );
                }
            } else if let Some(config) = cfg {
                let prompt_text = schema.prompt.as_deref().unwrap_or(field_name.as_str());
                match prompt_with_note_selector(config, selector_type, prompt_text) {
                    Ok(Some(value)) => {
                        result.values.insert(field_name.clone(), value);
                        result.prompted.push(field_name.clone());
                    }
                    Ok(None) => {
                        if let Some(ref default) = schema.default {
                            result.values.insert(
                                field_name.clone(),
                                yaml_value_to_string(default),
                            );
                            result.defaulted.push(field_name.clone());
                        } else if schema.required {
                            bail!("Required field '{}' was cancelled", field_name);
                        }
                    }
                    Err(e) => bail!("{e}"),
                }
            } else if let Some(ref default) = schema.default {
                result.values.insert(field_name.clone(), yaml_value_to_string(default));
                result.defaulted.push(field_name.clone());
            }
        } else if let Some(ref prompt_text) = schema.prompt {
            if options.batch_mode {
                if let Some(ref default) = schema.default {
                    let value = yaml_value_to_string(default);
                    result.values.insert(field_name.clone(), value);
                    result.defaulted.push(field_name.clone());
                } else if schema.required {
                    bail!("Missing required field '{}' in batch mode", field_name);
                }
            } else {
                let enum_values = schema.enum_values.as_deref();
                let default_str = schema.default.as_ref().map(yaml_value_to_string);

                match prompt_for_schema_field(
                    field_name,
                    prompt_text,
                    enum_values,
                    default_str.as_deref(),
                    schema.required,
                    schema.multiline,
                ) {
                    Ok(value) if !value.is_empty() => {
                        result.values.insert(field_name.clone(), value);
                        result.prompted.push(field_name.clone());
                    }
                    Ok(_) => {
                        if let Some(ref default) = schema.default {
                            result.values.insert(
                                field_name.clone(),
                                yaml_value_to_string(default),
                            );
                            result.defaulted.push(field_name.clone());
                        }
                        result.prompted.push(field_name.clone());
                    }
                    Err(e) => bail!("{e}"),
                }
            }
        } else if let Some(ref default) = schema.default {
            result.values.insert(field_name.clone(), yaml_value_to_string(default));
            result.defaulted.push(field_name.clone());
        }
    }

    // Process template variables
    let mut vars: Vec<_> = typedef.variables.iter().collect();
    vars.sort_by(|a, b| a.0.cmp(b.0));

    for (var_name, var_spec) in vars {
        if result.values.contains_key(var_name) {
            continue;
        }

        let prompt_text = var_spec.prompt();
        let default_value = var_spec.default();
        let is_required = var_spec.is_required();

        if !prompt_text.is_empty() {
            if options.batch_mode {
                if let Some(default) = default_value {
                    result.values.insert(var_name.clone(), default.to_string());
                    result.defaulted.push(var_name.clone());
                } else if is_required {
                    bail!("Missing required variable '{}' in batch mode", var_name);
                }
            } else {
                match prompt_for_variable(
                    var_name,
                    prompt_text,
                    default_value,
                    is_required,
                ) {
                    Ok(value) if !value.is_empty() => {
                        result.values.insert(var_name.clone(), value);
                        result.prompted.push(var_name.clone());
                    }
                    Ok(_) => {
                        if let Some(default) = default_value {
                            result.values.insert(var_name.clone(), default.to_string());
                            result.defaulted.push(var_name.clone());
                        }
                        result.prompted.push(var_name.clone());
                    }
                    Err(e) => bail!("{e}"),
                }
            }
        } else if let Some(default) = default_value {
            result.values.insert(var_name.clone(), default.to_string());
            result.defaulted.push(var_name.clone());
        }
    }

    Ok(result)
}

/// Convert a serde_yaml::Value to a string for template context.
pub(super) fn yaml_value_to_string(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Null => String::new(),
        other => serde_yaml::to_string(other).unwrap_or_default().trim().to_string(),
    }
}

/// Query existing projects from the index and prompt user to select one.
fn prompt_project_selection(cfg: &ResolvedConfig) -> Option<String> {
    let index_path = PathResolver::new(&cfg.vault_root).index_db();
    let db = match IndexDb::open(&index_path) {
        Ok(db) => db,
        Err(_) => {
            println!("No index found. Task will go to inbox.");
            return Some("inbox".to_string());
        }
    };

    let query = NoteQuery { note_type: Some(NoteType::Project), ..Default::default() };

    let projects = match db.query_notes(&query) {
        Ok(p) => p,
        Err(_) => return Some("inbox".to_string()),
    };

    let mut items: Vec<String> = vec!["Inbox (no project - for triage)".to_string()];
    for p in &projects {
        let title = if p.title.is_empty() { "Untitled" } else { &p.title };
        items.push(title.to_string());
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select project for this task")
        .items(&items)
        .default(0)
        .interact_opt()
        .ok()?;

    selection.map(|idx| {
        if idx == 0 {
            "inbox".to_string()
        } else {
            let project = &projects[idx - 1];
            project
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("project")
                .to_string()
        }
    })
}

/// Prompt for a single schema field value.
fn prompt_for_schema_field(
    field_name: &str,
    prompt_text: &str,
    enum_values: Option<&[String]>,
    default: Option<&str>,
    required: bool,
    multiline: bool,
) -> Result<String, String> {
    let theme = ColorfulTheme::default();

    if let Some(values) = enum_values {
        let default_idx =
            default.and_then(|d| values.iter().position(|v| v == d)).unwrap_or(0);

        let selection = Select::with_theme(&theme)
            .with_prompt(prompt_text)
            .items(values)
            .default(default_idx)
            .interact_opt()
            .map_err(|e| {
                format!("Failed to read selection for '{}': {}", field_name, e)
            })?;

        return match selection {
            Some(idx) => Ok(values[idx].clone()),
            None => Ok(default.unwrap_or("").to_string()),
        };
    }

    if multiline {
        let initial = default.unwrap_or("");
        let content = Editor::new()
            .edit(initial)
            .map_err(|e| format!("Editor error for '{}': {}", field_name, e))?;
        return Ok(content.unwrap_or_else(|| initial.to_string()));
    }

    let mut input = Input::<String>::with_theme(&theme);
    input = input.with_prompt(prompt_text);
    input = input.allow_empty(!required);

    if let Some(def) = default {
        input = input.with_initial_text(def);
    }

    input
        .interact_text()
        .map_err(|e| format!("Failed to read input for '{}': {}", field_name, e))
}

/// Prompt using a fuzzy note selector.
fn prompt_with_note_selector(
    cfg: &ResolvedConfig,
    note_type: &str,
    prompt_text: &str,
) -> Result<Option<String>, String> {
    let index_path = PathResolver::new(&cfg.vault_root).index_db();
    let db = IndexDb::open(&index_path).map_err(|e| {
        format!("Failed to open index for selector (run 'mdv reindex' first): {}", e)
    })?;

    let query = NoteQuery {
        note_type: Some(note_type.parse().unwrap_or_default()),
        ..Default::default()
    };

    let notes = db.query_notes(&query).map_err(|e| format!("Query error: {}", e))?;

    if notes.is_empty() {
        return Ok(None);
    }

    let items: Vec<String> = notes.iter().map(|n| n.title.clone()).collect();

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt_text)
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|e| format!("Selector error: {}", e))?;

    Ok(selection.map(|idx| {
        notes[idx]
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    }))
}

/// Prompt for a template variable value.
fn prompt_for_variable(
    var_name: &str,
    prompt_text: &str,
    default: Option<&str>,
    required: bool,
) -> Result<String, String> {
    let theme = ColorfulTheme::default();

    let mut input = Input::<String>::with_theme(&theme);
    input = input.with_prompt(prompt_text);
    input = input.allow_empty(!required);

    if let Some(def) = default {
        input = input.with_initial_text(def);
    }

    input
        .interact_text()
        .map_err(|e| format!("Failed to read input for '{}': {}", var_name, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdvault_core::types::FieldSchema;
    use serde_yaml::Value;

    #[test]
    fn test_yaml_value_to_string() {
        assert_eq!(yaml_value_to_string(&Value::String("foo".into())), "foo");
        assert_eq!(yaml_value_to_string(&Value::Number(42.into())), "42");
        assert_eq!(yaml_value_to_string(&Value::Bool(true)), "true");
        assert_eq!(yaml_value_to_string(&Value::Null), "");
    }

    #[test]
    fn test_collect_schema_variables_batch_missing_required() {
        let mut schema = HashMap::new();
        schema.insert(
            "req".to_string(),
            FieldSchema {
                required: true,
                prompt: Some("Required field?".to_string()),
                ..Default::default()
            },
        );

        let typedef = TypeDefinition { schema, ..TypeDefinition::empty("test") };

        let provided = HashMap::new();
        let options = PromptOptions { batch_mode: true };

        let result = collect_schema_variables(&typedef, &provided, &options, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing required field"));
    }

    #[test]
    fn test_collect_schema_variables_selector_batch_with_default() {
        let mut schema = HashMap::new();
        schema.insert(
            "project".to_string(),
            FieldSchema {
                selector: Some("project".to_string()),
                prompt: Some("Select project".to_string()),
                default: Some(Value::String("inbox".to_string())),
                ..Default::default()
            },
        );

        let typedef = TypeDefinition { schema, ..TypeDefinition::empty("test") };

        let provided = HashMap::new();
        let options = PromptOptions { batch_mode: true };

        let result =
            collect_schema_variables(&typedef, &provided, &options, None).unwrap();
        assert_eq!(result.values.get("project"), Some(&"inbox".to_string()));
        assert!(result.defaulted.contains(&"project".to_string()));
    }

    #[test]
    fn test_collect_schema_variables_selector_batch_required_no_default() {
        let mut schema = HashMap::new();
        schema.insert(
            "project".to_string(),
            FieldSchema {
                selector: Some("project".to_string()),
                prompt: Some("Select project".to_string()),
                required: true,
                ..Default::default()
            },
        );

        let typedef = TypeDefinition { schema, ..TypeDefinition::empty("test") };

        let provided = HashMap::new();
        let options = PromptOptions { batch_mode: true };

        let result = collect_schema_variables(&typedef, &provided, &options, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("selector field"));
    }

    #[test]
    fn test_collect_schema_variables_selector_provided() {
        let mut schema = HashMap::new();
        schema.insert(
            "project".to_string(),
            FieldSchema {
                selector: Some("project".to_string()),
                prompt: Some("Select project".to_string()),
                ..Default::default()
            },
        );

        let typedef = TypeDefinition { schema, ..TypeDefinition::empty("test") };

        let mut provided = HashMap::new();
        provided.insert("project".to_string(), "my-project".to_string());
        let options = PromptOptions { batch_mode: false };

        let result =
            collect_schema_variables(&typedef, &provided, &options, None).unwrap();
        assert_eq!(result.values.get("project"), Some(&"my-project".to_string()));
    }
}
