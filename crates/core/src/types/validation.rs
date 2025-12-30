//! Note validation against type definitions.

use regex::Regex;

use super::definition::TypeDefinition;
use super::errors::{ValidationError, ValidationResult};
use super::registry::TypeRegistry;
use super::schema::{FieldSchema, FieldType};
use crate::scripting::LuaEngine;

/// Validate a note's frontmatter against its type definition.
///
/// Returns a ValidationResult with any errors and warnings found.
pub fn validate_note(
    registry: &TypeRegistry,
    note_type: &str,
    note_path: &str,
    frontmatter: &serde_yaml::Value,
    content: &str,
) -> ValidationResult {
    // Get type definition (if any)
    let typedef = match registry.get(note_type) {
        Some(td) => td,
        None => return ValidationResult::success(), // Unknown types pass by default
    };

    let mut result = ValidationResult::success();

    // Phase 1: Schema validation
    if let serde_yaml::Value::Mapping(map) = frontmatter {
        let schema_result = validate_schema(&typedef, map);
        result.merge(schema_result);
    }

    // Phase 2: Custom validate() function
    if typedef.has_validate_fn {
        match run_validate_hook(&typedef, note_type, note_path, frontmatter, content) {
            Ok((valid, message)) => {
                if !valid {
                    result.add_error(ValidationError::CustomValidation {
                        message: message.unwrap_or_else(|| "Custom validation failed".to_string()),
                    });
                }
            }
            Err(e) => result.add_error(e),
        }
    }

    result
}

/// Validate frontmatter against schema.
fn validate_schema(typedef: &TypeDefinition, frontmatter: &serde_yaml::Mapping) -> ValidationResult {
    let mut result = ValidationResult::success();

    for (field_name, schema) in &typedef.schema {
        let value = frontmatter.get(serde_yaml::Value::String(field_name.clone()));

        // Check required fields
        if schema.required && value.is_none() {
            result.add_error(ValidationError::MissingRequired {
                field: field_name.clone(),
            });
            continue;
        }

        // Validate value if present
        if let Some(val) = value {
            let field_result = validate_field(field_name, schema, val);
            result.merge(field_result);
        }
    }

    result
}

/// Validate a single field value against its schema.
fn validate_field(field: &str, schema: &FieldSchema, value: &serde_yaml::Value) -> ValidationResult {
    let mut result = ValidationResult::success();

    let expected_type = schema.effective_type();

    // Type checking
    let type_ok = match (&expected_type, value) {
        (FieldType::String, serde_yaml::Value::String(_)) => true,
        (FieldType::Number, serde_yaml::Value::Number(_)) => true,
        (FieldType::Boolean, serde_yaml::Value::Bool(_)) => true,
        (FieldType::List, serde_yaml::Value::Sequence(_)) => true,
        (FieldType::Date, serde_yaml::Value::String(s)) => is_valid_date(s),
        (FieldType::Datetime, serde_yaml::Value::String(s)) => is_valid_datetime(s),
        (FieldType::Reference, serde_yaml::Value::String(_)) => true,
        _ => false,
    };

    if !type_ok {
        result.add_error(ValidationError::TypeMismatch {
            field: field.to_string(),
            expected: expected_type.to_string(),
            actual: yaml_type_name(value),
        });
        return result;
    }

    // Enum constraint
    if let (Some(enum_values), serde_yaml::Value::String(s)) = (&schema.enum_values, value)
        && !enum_values.contains(s)
    {
        result.add_error(ValidationError::EnumViolation {
            field: field.to_string(),
            value: s.clone(),
            allowed: enum_values.clone(),
        });
    }

    // Number constraints
    if let serde_yaml::Value::Number(n) = value
        && let Some(f) = n.as_f64() {
            if let Some(min) = schema.min
                && f < min {
                    result.add_error(ValidationError::InvalidValue {
                        field: field.to_string(),
                        message: format!("value {} is less than minimum {}", f, min),
                    });
                }
            if let Some(max) = schema.max
                && f > max {
                    result.add_error(ValidationError::InvalidValue {
                        field: field.to_string(),
                        message: format!("value {} is greater than maximum {}", f, max),
                    });
                }
            if let Some(true) = schema.integer
                && f.fract() != 0.0 {
                    result.add_error(ValidationError::InvalidValue {
                        field: field.to_string(),
                        message: format!("value {} must be an integer", f),
                    });
                }
        }

    // String length constraints
    if let serde_yaml::Value::String(s) = value {
        if let Some(min) = schema.min_length
            && s.len() < min {
                result.add_error(ValidationError::InvalidValue {
                    field: field.to_string(),
                    message: format!("string length {} is less than minimum {}", s.len(), min),
                });
            }
        if let Some(max) = schema.max_length
            && s.len() > max {
                result.add_error(ValidationError::InvalidValue {
                    field: field.to_string(),
                    message: format!("string length {} is greater than maximum {}", s.len(), max),
                });
            }
        if let Some(pattern) = &schema.pattern
            && let Ok(re) = Regex::new(pattern)
                && !re.is_match(s) {
                    result.add_error(ValidationError::InvalidValue {
                        field: field.to_string(),
                        message: format!("value '{}' does not match pattern '{}'", s, pattern),
                    });
                }
    }

    // List constraints
    if let serde_yaml::Value::Sequence(seq) = value {
        if let Some(min) = schema.min_items
            && seq.len() < min {
                result.add_error(ValidationError::InvalidValue {
                    field: field.to_string(),
                    message: format!("list has {} items, minimum is {}", seq.len(), min),
                });
            }
        if let Some(max) = schema.max_items
            && seq.len() > max {
                result.add_error(ValidationError::InvalidValue {
                    field: field.to_string(),
                    message: format!("list has {} items, maximum is {}", seq.len(), max),
                });
            }

        // Validate items if schema provided
        if let Some(item_schema) = &schema.items {
            for (i, item) in seq.iter().enumerate() {
                let item_field = format!("{}[{}]", field, i);
                let item_result = validate_field(&item_field, item_schema, item);
                result.merge(item_result);
            }
        }
    }

    result
}

/// Check if a string is a valid date (YYYY-MM-DD format).
fn is_valid_date(s: &str) -> bool {
    // Simple validation: YYYY-MM-DD
    if s.len() != 10 {
        return false;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && parts[2].chars().all(|c| c.is_ascii_digit())
}

/// Check if a string is a valid datetime (ISO 8601 format).
fn is_valid_datetime(s: &str) -> bool {
    // Accept various ISO 8601 formats
    // YYYY-MM-DDTHH:MM:SS or YYYY-MM-DD HH:MM:SS or YYYY-MM-DDTHH:MM:SSZ
    chrono::DateTime::parse_from_rfc3339(s).is_ok()
        || chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").is_ok()
        || chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").is_ok()
}

/// Get a human-readable type name for a YAML value.
fn yaml_type_name(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::Null => "null".to_string(),
        serde_yaml::Value::Bool(_) => "boolean".to_string(),
        serde_yaml::Value::Number(_) => "number".to_string(),
        serde_yaml::Value::String(_) => "string".to_string(),
        serde_yaml::Value::Sequence(_) => "list".to_string(),
        serde_yaml::Value::Mapping(_) => "mapping".to_string(),
        serde_yaml::Value::Tagged(_) => "tagged".to_string(),
    }
}

/// Run custom validate() Lua hook.
fn run_validate_hook(
    typedef: &TypeDefinition,
    note_type: &str,
    note_path: &str,
    frontmatter: &serde_yaml::Value,
    content: &str,
) -> Result<(bool, Option<String>), ValidationError> {
    let engine = LuaEngine::sandboxed().map_err(|e| ValidationError::LuaError(e.to_string()))?;

    let lua = engine.lua();

    // Load the type definition
    lua.load(&typedef.lua_source)
        .exec()
        .map_err(|e| ValidationError::LuaError(e.to_string()))?;

    // Build note table for validation
    let note_table = lua
        .create_table()
        .map_err(|e| ValidationError::LuaError(e.to_string()))?;

    note_table
        .set("type", note_type)
        .map_err(|e| ValidationError::LuaError(e.to_string()))?;
    note_table
        .set("path", note_path)
        .map_err(|e| ValidationError::LuaError(e.to_string()))?;
    note_table
        .set("content", content)
        .map_err(|e| ValidationError::LuaError(e.to_string()))?;

    // Convert frontmatter to Lua table
    let fm_table = yaml_to_lua_table(lua, frontmatter)
        .map_err(|e| ValidationError::LuaError(e.to_string()))?;
    note_table
        .set("frontmatter", fm_table)
        .map_err(|e| ValidationError::LuaError(e.to_string()))?;

    // Get the type definition table by re-evaluating
    let typedef_table: mlua::Table = lua
        .load(&typedef.lua_source)
        .eval()
        .map_err(|e| ValidationError::LuaError(e.to_string()))?;

    // Call validate function
    let validate_fn: mlua::Function = typedef_table
        .get("validate")
        .map_err(|e| ValidationError::LuaError(e.to_string()))?;

    let result = validate_fn
        .call::<mlua::MultiValue>(note_table)
        .map_err(|e| ValidationError::LuaError(e.to_string()))?;

    // Parse result: (true) or (false, "error message")
    let values: Vec<mlua::Value> = result.into_iter().collect();
    match values.as_slice() {
        [mlua::Value::Boolean(true)] | [mlua::Value::Boolean(true), _] => Ok((true, None)),
        [mlua::Value::Boolean(false)] => Ok((false, None)),
        [mlua::Value::Boolean(false), mlua::Value::String(msg)] => {
            let msg_str = msg.to_str().map(|s| s.to_string()).unwrap_or_default();
            Ok((false, Some(msg_str)))
        }
        [mlua::Value::Nil] => Ok((true, None)), // nil treated as success
        [] => Ok((true, None)),                 // no return treated as success
        _ => Ok((true, None)),
    }
}

/// Convert a serde_yaml::Value to a Lua value.
fn yaml_to_lua_table(lua: &mlua::Lua, value: &serde_yaml::Value) -> mlua::Result<mlua::Value> {
    match value {
        serde_yaml::Value::Null => Ok(mlua::Value::Nil),
        serde_yaml::Value::Bool(b) => Ok(mlua::Value::Boolean(*b)),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(mlua::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(mlua::Value::Number(f))
            } else {
                Ok(mlua::Value::Nil)
            }
        }
        serde_yaml::Value::String(s) => Ok(mlua::Value::String(lua.create_string(s)?)),
        serde_yaml::Value::Sequence(seq) => {
            let table = lua.create_table()?;
            for (i, item) in seq.iter().enumerate() {
                table.set(i + 1, yaml_to_lua_table(lua, item)?)?;
            }
            Ok(mlua::Value::Table(table))
        }
        serde_yaml::Value::Mapping(map) => {
            let table = lua.create_table()?;
            for (k, v) in map {
                if let serde_yaml::Value::String(key) = k {
                    table.set(key.as_str(), yaml_to_lua_table(lua, v)?)?;
                }
            }
            Ok(mlua::Value::Table(table))
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_lua_table(lua, &tagged.value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::definition::TypeDefinition;
    use std::collections::HashMap;

    fn make_frontmatter(fields: &[(&str, serde_yaml::Value)]) -> serde_yaml::Value {
        let mut map = serde_yaml::Mapping::new();
        for (k, v) in fields {
            map.insert(serde_yaml::Value::String(k.to_string()), v.clone());
        }
        serde_yaml::Value::Mapping(map)
    }

    fn make_typedef_with_schema(schema: HashMap<String, FieldSchema>) -> TypeDefinition {
        TypeDefinition {
            name: "test".to_string(),
            description: None,
            source_path: std::path::PathBuf::new(),
            schema,
            has_validate_fn: false,
            has_on_create_hook: false,
            has_on_update_hook: false,
            is_builtin_override: false,
            lua_source: String::new(),
        }
    }

    #[test]
    fn test_validate_required_field_present() {
        let mut registry = TypeRegistry::new();
        let mut schema = HashMap::new();
        schema.insert(
            "title".to_string(),
            FieldSchema {
                field_type: Some(FieldType::String),
                required: true,
                ..Default::default()
            },
        );
        registry.register(make_typedef_with_schema(schema)).unwrap();

        let frontmatter = make_frontmatter(&[("title", serde_yaml::Value::String("Hello".into()))]);

        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_required_field_missing() {
        let mut registry = TypeRegistry::new();
        let mut schema = HashMap::new();
        schema.insert(
            "title".to_string(),
            FieldSchema {
                field_type: Some(FieldType::String),
                required: true,
                ..Default::default()
            },
        );
        registry.register(make_typedef_with_schema(schema)).unwrap();

        let frontmatter = make_frontmatter(&[]);

        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(&result.errors[0], ValidationError::MissingRequired { field } if field == "title"));
    }

    #[test]
    fn test_validate_type_mismatch() {
        let mut registry = TypeRegistry::new();
        let mut schema = HashMap::new();
        schema.insert(
            "count".to_string(),
            FieldSchema {
                field_type: Some(FieldType::Number),
                required: true,
                ..Default::default()
            },
        );
        registry.register(make_typedef_with_schema(schema)).unwrap();

        let frontmatter =
            make_frontmatter(&[("count", serde_yaml::Value::String("not a number".into()))]);

        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(!result.valid);
        assert!(matches!(&result.errors[0], ValidationError::TypeMismatch { .. }));
    }

    #[test]
    fn test_validate_enum() {
        let mut registry = TypeRegistry::new();
        let mut schema = HashMap::new();
        schema.insert(
            "status".to_string(),
            FieldSchema {
                field_type: Some(FieldType::String),
                required: true,
                enum_values: Some(vec!["open".to_string(), "done".to_string()]),
                ..Default::default()
            },
        );
        registry.register(make_typedef_with_schema(schema)).unwrap();

        // Valid enum value
        let frontmatter = make_frontmatter(&[("status", serde_yaml::Value::String("open".into()))]);
        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(result.valid);

        // Invalid enum value
        let frontmatter =
            make_frontmatter(&[("status", serde_yaml::Value::String("invalid".into()))]);
        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(!result.valid);
        assert!(matches!(&result.errors[0], ValidationError::EnumViolation { .. }));
    }

    #[test]
    fn test_validate_number_range() {
        let mut registry = TypeRegistry::new();
        let mut schema = HashMap::new();
        schema.insert(
            "priority".to_string(),
            FieldSchema {
                field_type: Some(FieldType::Number),
                required: true,
                min: Some(1.0),
                max: Some(5.0),
                ..Default::default()
            },
        );
        registry.register(make_typedef_with_schema(schema)).unwrap();

        // Valid range
        let frontmatter = make_frontmatter(&[("priority", serde_yaml::Value::Number(3.into()))]);
        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(result.valid);

        // Below minimum
        let frontmatter = make_frontmatter(&[("priority", serde_yaml::Value::Number(0.into()))]);
        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(!result.valid);

        // Above maximum
        let frontmatter = make_frontmatter(&[("priority", serde_yaml::Value::Number(10.into()))]);
        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(!result.valid);
    }

    #[test]
    fn test_validate_string_length() {
        let mut registry = TypeRegistry::new();
        let mut schema = HashMap::new();
        schema.insert(
            "code".to_string(),
            FieldSchema {
                field_type: Some(FieldType::String),
                required: true,
                min_length: Some(3),
                max_length: Some(10),
                ..Default::default()
            },
        );
        registry.register(make_typedef_with_schema(schema)).unwrap();

        // Valid length
        let frontmatter = make_frontmatter(&[("code", serde_yaml::Value::String("ABC123".into()))]);
        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(result.valid);

        // Too short
        let frontmatter = make_frontmatter(&[("code", serde_yaml::Value::String("AB".into()))]);
        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(!result.valid);

        // Too long
        let frontmatter =
            make_frontmatter(&[("code", serde_yaml::Value::String("ABCDEFGHIJK".into()))]);
        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(!result.valid);
    }

    #[test]
    fn test_validate_list_items() {
        let mut registry = TypeRegistry::new();
        let mut schema = HashMap::new();
        schema.insert(
            "tags".to_string(),
            FieldSchema {
                field_type: Some(FieldType::List),
                required: true,
                min_items: Some(1),
                max_items: Some(5),
                ..Default::default()
            },
        );
        registry.register(make_typedef_with_schema(schema)).unwrap();

        // Valid list
        let frontmatter = make_frontmatter(&[(
            "tags",
            serde_yaml::Value::Sequence(vec![
                serde_yaml::Value::String("a".into()),
                serde_yaml::Value::String("b".into()),
            ]),
        )]);
        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(result.valid);

        // Empty list (below minimum)
        let frontmatter = make_frontmatter(&[("tags", serde_yaml::Value::Sequence(vec![]))]);
        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(!result.valid);
    }

    #[test]
    fn test_validate_date_format() {
        let mut registry = TypeRegistry::new();
        let mut schema = HashMap::new();
        schema.insert(
            "due".to_string(),
            FieldSchema {
                field_type: Some(FieldType::Date),
                required: true,
                ..Default::default()
            },
        );
        registry.register(make_typedef_with_schema(schema)).unwrap();

        // Valid date
        let frontmatter =
            make_frontmatter(&[("due", serde_yaml::Value::String("2025-12-29".into()))]);
        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(result.valid);

        // Invalid date format
        let frontmatter =
            make_frontmatter(&[("due", serde_yaml::Value::String("29-12-2025".into()))]);
        let result = validate_note(&registry, "test", "/test.md", &frontmatter, "");
        assert!(!result.valid);
    }

    #[test]
    fn test_validate_unknown_type() {
        let registry = TypeRegistry::new();

        // Unknown types should pass
        let frontmatter = make_frontmatter(&[("anything", serde_yaml::Value::String("value".into()))]);
        let result = validate_note(&registry, "unknown", "/test.md", &frontmatter, "");
        assert!(result.valid);
    }

    #[test]
    fn test_is_valid_date() {
        assert!(is_valid_date("2025-12-29"));
        assert!(is_valid_date("2000-01-01"));
        assert!(!is_valid_date("2025-1-29")); // Month not zero-padded
        assert!(!is_valid_date("25-12-29")); // Year not 4 digits
        assert!(!is_valid_date("2025/12/29")); // Wrong separator
        assert!(!is_valid_date("not a date"));
    }

    #[test]
    fn test_is_valid_datetime() {
        assert!(is_valid_datetime("2025-12-29T14:30:00Z"));
        assert!(is_valid_datetime("2025-12-29T14:30:00+00:00"));
        assert!(is_valid_datetime("2025-12-29T14:30:00"));
        assert!(!is_valid_datetime("not a datetime"));
        assert!(!is_valid_datetime("2025-12-29")); // Just a date
    }
}
