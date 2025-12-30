//! Scaffolding generation for note creation.
//!
//! This module provides functions to generate note content based on type schemas.

use std::collections::HashMap;

use chrono::Local;

use super::definition::TypeDefinition;
use super::schema::FieldType;

/// Generate scaffolding content for a new note.
///
/// Creates frontmatter with:
/// - `type` field set to the type name
/// - `title` field if provided
/// - `created` field with current date
/// - Fields from schema with provided values or defaults
///
/// # Arguments
///
/// * `type_name` - The note type (e.g., "task", "project")
/// * `typedef` - Optional type definition with schema
/// * `title` - The note title
/// * `vars` - User-provided field values
///
/// # Returns
///
/// The complete note content with frontmatter and body.
pub fn generate_scaffolding(
    type_name: &str,
    typedef: Option<&TypeDefinition>,
    title: &str,
    vars: &HashMap<String, String>,
) -> String {
    let mut frontmatter = serde_yaml::Mapping::new();

    // Always include type
    frontmatter.insert(
        serde_yaml::Value::String("type".to_string()),
        serde_yaml::Value::String(type_name.to_string()),
    );

    // Always include title
    frontmatter.insert(
        serde_yaml::Value::String("title".to_string()),
        serde_yaml::Value::String(title.to_string()),
    );

    // Add fields from schema
    if let Some(td) = typedef {
        for (field, schema) in &td.schema {
            // Skip if already set (type, title)
            if field == "type" || field == "title" {
                continue;
            }

            let value = if let Some(v) = vars.get(field) {
                // User provided value
                Some(string_to_yaml_value(v, schema.field_type))
            } else {
                // Schema default
                schema.default.clone()
            };

            if let Some(v) = value {
                frontmatter.insert(serde_yaml::Value::String(field.clone()), v);
            }
        }
    }

    // Add any extra vars not in schema
    for (key, value) in vars {
        if key == "type" || key == "title" {
            continue;
        }
        // Only add if not already in frontmatter
        let key_value = serde_yaml::Value::String(key.clone());
        if !frontmatter.contains_key(&key_value) {
            frontmatter.insert(key_value, serde_yaml::Value::String(value.clone()));
        }
    }

    // Add created date
    let today = Local::now().format("%Y-%m-%d").to_string();
    frontmatter.insert(
        serde_yaml::Value::String("created".to_string()),
        serde_yaml::Value::String(today),
    );

    // Serialize frontmatter
    let yaml = serde_yaml::to_string(&frontmatter).unwrap_or_default();

    format!("---\n{}---\n\n# {}\n\n", yaml, title)
}

/// Convert a string value to appropriate YAML type based on field type.
fn string_to_yaml_value(s: &str, field_type: Option<FieldType>) -> serde_yaml::Value {
    match field_type {
        Some(FieldType::Number) => {
            if let Ok(n) = s.parse::<i64>() {
                serde_yaml::Value::Number(n.into())
            } else if let Ok(n) = s.parse::<f64>() {
                serde_yaml::Value::Number(serde_yaml::Number::from(n))
            } else {
                serde_yaml::Value::String(s.to_string())
            }
        }
        Some(FieldType::Boolean) => {
            serde_yaml::Value::Bool(s.eq_ignore_ascii_case("true") || s == "1")
        }
        Some(FieldType::List) => {
            // Parse comma-separated values
            let items: Vec<serde_yaml::Value> = s
                .split(',')
                .map(|item| serde_yaml::Value::String(item.trim().to_string()))
                .collect();
            serde_yaml::Value::Sequence(items)
        }
        _ => serde_yaml::Value::String(s.to_string()),
    }
}

/// Get required fields that are missing from the provided vars.
///
/// Returns a list of (field_name, field_schema) for fields that:
/// - Are marked as required in the schema
/// - Have no default value
/// - Are not provided in vars
pub fn get_missing_required_fields<'a>(
    typedef: &'a TypeDefinition,
    vars: &HashMap<String, String>,
) -> Vec<(&'a String, &'a super::schema::FieldSchema)> {
    typedef
        .schema
        .iter()
        .filter(|(field, schema)| {
            schema.required
                && schema.default.is_none()
                && !vars.contains_key(*field)
                && *field != "title"
                && *field != "type"
        })
        .collect()
}

/// Generate default output path for a note.
///
/// Pattern: `<type>s/<title-slugified>.md`
/// Examples:
/// - task, "Fix bug" -> "tasks/fix-bug.md"
/// - project, "My Project" -> "projects/my-project.md"
pub fn default_output_path(type_name: &str, title: &str) -> String {
    let slug = slugify(title);
    format!("{}s/{}.md", type_name, slug)
}

/// Convert a string to a URL-friendly slug.
fn slugify(s: &str) -> String {
    let mut result = String::with_capacity(s.len());

    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            result.push(c.to_ascii_lowercase());
        } else if (c == ' ' || c == '_' || c == '-') && !result.ends_with('-') {
            result.push('-');
        }
    }

    result.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_scaffolding_minimal() {
        let content = generate_scaffolding("task", None, "My Task", &HashMap::new());

        assert!(content.contains("type: task"));
        assert!(content.contains("title: My Task"));
        assert!(content.contains("# My Task"));
        assert!(content.contains("created:"));
    }

    #[test]
    fn test_generate_scaffolding_with_vars() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), "open".to_string());
        vars.insert("project".to_string(), "myproject".to_string());

        let content = generate_scaffolding("task", None, "My Task", &vars);

        assert!(content.contains("status: open"));
        assert!(content.contains("project: myproject"));
    }

    #[test]
    fn test_default_output_path() {
        assert_eq!(default_output_path("task", "Fix bug"), "tasks/fix-bug.md");
        assert_eq!(
            default_output_path("project", "My New Project"),
            "projects/my-new-project.md"
        );
        assert_eq!(
            default_output_path("zettel", "Random Thought!"),
            "zettels/random-thought.md"
        );
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("My Task: Do Something!"), "my-task-do-something");
        assert_eq!(slugify("  spaced  out  "), "spaced-out");
    }

    #[test]
    fn test_string_to_yaml_value_number() {
        let v = string_to_yaml_value("42", Some(FieldType::Number));
        assert_eq!(v, serde_yaml::Value::Number(42.into()));
    }

    #[test]
    fn test_string_to_yaml_value_boolean() {
        let v = string_to_yaml_value("true", Some(FieldType::Boolean));
        assert_eq!(v, serde_yaml::Value::Bool(true));

        let v = string_to_yaml_value("false", Some(FieldType::Boolean));
        assert_eq!(v, serde_yaml::Value::Bool(false));
    }

    #[test]
    fn test_string_to_yaml_value_list() {
        let v = string_to_yaml_value("a, b, c", Some(FieldType::List));
        if let serde_yaml::Value::Sequence(items) = v {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], serde_yaml::Value::String("a".to_string()));
            assert_eq!(items[1], serde_yaml::Value::String("b".to_string()));
            assert_eq!(items[2], serde_yaml::Value::String("c".to_string()));
        } else {
            panic!("Expected sequence");
        }
    }
}
