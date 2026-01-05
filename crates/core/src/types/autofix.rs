//! Auto-fix functionality for note validation issues.
//!
//! Provides safe auto-corrections for common validation errors:
//! - Adding missing required fields with default values
//! - Normalizing enum value case
//! - Adding missing frontmatter block

use std::collections::HashMap;
use std::path::Path;

use super::definition::TypeDefinition;
use super::errors::ValidationError;
use super::registry::TypeRegistry;
use crate::frontmatter::{Frontmatter, ParsedDocument, parse as parse_frontmatter};

/// Result of attempting to fix a note.
#[derive(Debug)]
pub struct FixResult {
    /// Whether any fixes were applied.
    pub fixed: bool,
    /// Description of fixes applied.
    pub fixes: Vec<String>,
    /// The corrected content (if fixes were applied).
    pub content: Option<String>,
}

impl FixResult {
    pub fn no_fix() -> Self {
        Self { fixed: false, fixes: Vec::new(), content: None }
    }
}

/// Attempt to auto-fix validation errors in a note.
///
/// Returns a FixResult with the corrected content if fixes were applied.
pub fn try_fix_note(
    registry: &TypeRegistry,
    note_type: &str,
    content: &str,
    errors: &[ValidationError],
) -> FixResult {
    let typedef = match registry.get(note_type) {
        Some(td) => td,
        None => return FixResult::no_fix(),
    };

    // Parse existing frontmatter
    let parsed = match parse_frontmatter(content) {
        Ok(p) => p,
        Err(_) => return FixResult::no_fix(),
    };

    let mut frontmatter = parsed.frontmatter.map(|fm| fm.fields).unwrap_or_default();
    let mut fixes = Vec::new();

    for error in errors {
        match error {
            ValidationError::MissingRequired { field } => {
                if let Some(fix) = fix_missing_required(&typedef, field, &mut frontmatter)
                {
                    fixes.push(fix);
                }
            }
            ValidationError::EnumViolation { field, value, allowed } => {
                if let Some(fix) = fix_enum_case(field, value, allowed, &mut frontmatter)
                {
                    fixes.push(fix);
                }
            }
            _ => {} // Other errors can't be auto-fixed
        }
    }

    if fixes.is_empty() {
        return FixResult::no_fix();
    }

    // Reconstruct the document with fixed frontmatter
    let new_doc = ParsedDocument {
        frontmatter: Some(Frontmatter { fields: frontmatter }),
        body: parsed.body,
    };
    let new_content = crate::frontmatter::serialize(&new_doc);

    FixResult { fixed: true, fixes, content: Some(new_content) }
}

/// Fix a missing required field by adding its default value.
fn fix_missing_required(
    typedef: &TypeDefinition,
    field: &str,
    frontmatter: &mut HashMap<String, serde_yaml::Value>,
) -> Option<String> {
    let schema = typedef.schema.get(field)?;

    // Only fix if there's a default value
    let default = schema.default.as_ref()?;

    frontmatter.insert(field.to_string(), default.clone());

    let default_str = match default {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        _ => format!("{:?}", default),
    };

    Some(format!("Added missing field '{}' with default '{}'", field, default_str))
}

/// Fix enum case mismatch by normalizing to the correct case.
fn fix_enum_case(
    field: &str,
    value: &str,
    allowed: &[String],
    frontmatter: &mut HashMap<String, serde_yaml::Value>,
) -> Option<String> {
    // Find a case-insensitive match
    let lowercase_value = value.to_lowercase();
    let correct_value = allowed.iter().find(|v| v.to_lowercase() == lowercase_value)?;

    if correct_value == value {
        return None; // No fix needed
    }

    frontmatter
        .insert(field.to_string(), serde_yaml::Value::String(correct_value.clone()));

    Some(format!("Fixed case for '{}': '{}' -> '{}'", field, value, correct_value))
}

/// Apply fixes to a note file.
pub fn apply_fixes(path: &Path, content: &str) -> Result<(), String> {
    std::fs::write(path, content)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::schema::{FieldSchema, FieldType};

    fn make_typedef_with_defaults() -> TypeDefinition {
        let mut schema = HashMap::new();
        schema.insert(
            "status".to_string(),
            FieldSchema {
                field_type: Some(FieldType::String),
                required: true,
                enum_values: Some(vec!["open".to_string(), "done".to_string()]),
                default: Some(serde_yaml::Value::String("open".to_string())),
                ..Default::default()
            },
        );
        schema.insert(
            "priority".to_string(),
            FieldSchema {
                field_type: Some(FieldType::String),
                required: false,
                enum_values: Some(vec![
                    "low".to_string(),
                    "medium".to_string(),
                    "high".to_string(),
                ]),
                default: Some(serde_yaml::Value::String("medium".to_string())),
                ..Default::default()
            },
        );

        TypeDefinition {
            name: "task".to_string(),
            description: None,
            source_path: std::path::PathBuf::new(),
            schema,
            output: None,
            has_validate_fn: false,
            has_on_create_hook: false,
            has_on_update_hook: false,
            is_builtin_override: false,
            lua_source: String::new(),
        }
    }

    #[test]
    fn test_fix_missing_required_with_default() {
        let mut registry = TypeRegistry::new();
        registry.register(make_typedef_with_defaults()).unwrap();

        let content = "---\ntype: task\ntitle: Test\n---\n\n# Test\n";
        let errors =
            vec![ValidationError::MissingRequired { field: "status".to_string() }];

        let result = try_fix_note(&registry, "task", content, &errors);
        assert!(result.fixed);
        assert_eq!(result.fixes.len(), 1);
        assert!(result.fixes[0].contains("status"));
        assert!(result.content.unwrap().contains("status: open"));
    }

    #[test]
    fn test_fix_enum_case() {
        let mut registry = TypeRegistry::new();
        registry.register(make_typedef_with_defaults()).unwrap();

        let content = "---\ntype: task\nstatus: OPEN\n---\n\n# Test\n";
        let errors = vec![ValidationError::EnumViolation {
            field: "status".to_string(),
            value: "OPEN".to_string(),
            allowed: vec!["open".to_string(), "done".to_string()],
        }];

        let result = try_fix_note(&registry, "task", content, &errors);
        assert!(result.fixed);
        assert!(result.fixes[0].contains("OPEN"));
        assert!(result.fixes[0].contains("open"));
    }

    #[test]
    fn test_no_fix_without_default() {
        let mut schema = HashMap::new();
        schema.insert(
            "project".to_string(),
            FieldSchema {
                field_type: Some(FieldType::String),
                required: true,
                default: None, // No default
                ..Default::default()
            },
        );

        let typedef = TypeDefinition {
            name: "task".to_string(),
            description: None,
            source_path: std::path::PathBuf::new(),
            schema,
            output: None,
            has_validate_fn: false,
            has_on_create_hook: false,
            has_on_update_hook: false,
            is_builtin_override: false,
            lua_source: String::new(),
        };

        let mut registry = TypeRegistry::new();
        registry.register(typedef).unwrap();

        let content = "---\ntype: task\n---\n\n# Test\n";
        let errors =
            vec![ValidationError::MissingRequired { field: "project".to_string() }];

        let result = try_fix_note(&registry, "task", content, &errors);
        assert!(!result.fixed);
    }
}
