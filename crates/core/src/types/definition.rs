//! Type definition structures.

use std::collections::HashMap;
use std::path::PathBuf;

use super::schema::FieldSchema;
use crate::vars::VarsMap;

/// A loaded type definition from a Lua file.
#[derive(Debug, Clone)]
pub struct TypeDefinition {
    /// Type name (from filename).
    pub name: String,

    /// Human-readable description.
    pub description: Option<String>,

    /// Path to the source .lua file.
    pub source_path: PathBuf,

    /// Field schemas for frontmatter validation.
    pub schema: HashMap<String, FieldSchema>,

    /// Output path template (supports {{var}} placeholders).
    pub output: Option<String>,

    /// Template variables with optional prompts and defaults.
    /// These are used for template body substitution, not frontmatter fields.
    pub variables: VarsMap,

    /// Whether this type has a custom validate() function.
    pub has_validate_fn: bool,

    /// Whether this type has an on_create() hook.
    pub has_on_create_hook: bool,

    /// Whether this type has an on_update() hook.
    pub has_on_update_hook: bool,

    /// Whether this overrides a built-in type.
    pub is_builtin_override: bool,

    /// Raw Lua source (for re-execution of hooks).
    pub lua_source: String,
}

impl TypeDefinition {
    /// Create an empty type definition (for testing).
    pub fn empty(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: None,
            source_path: PathBuf::new(),
            schema: HashMap::new(),
            output: None,
            variables: VarsMap::new(),
            has_validate_fn: false,
            has_on_create_hook: false,
            has_on_update_hook: false,
            is_builtin_override: false,
            lua_source: String::new(),
        }
    }

    /// Check if this type has any hooks.
    pub fn has_hooks(&self) -> bool {
        self.has_validate_fn || self.has_on_create_hook || self.has_on_update_hook
    }

    /// Get a list of required fields.
    pub fn required_fields(&self) -> Vec<&str> {
        self.schema
            .iter()
            .filter(|(_, schema)| schema.required)
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Check if the type has a schema for a given field.
    pub fn has_field(&self, name: &str) -> bool {
        self.schema.contains_key(name)
    }

    /// Get the schema for a field.
    pub fn get_field(&self, name: &str) -> Option<&FieldSchema> {
        self.schema.get(name)
    }
}

/// Information about a discovered type definition file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedefInfo {
    /// Type name (filename without .lua extension).
    pub name: String,

    /// Full path to the .lua file.
    pub path: PathBuf,
}

impl TypedefInfo {
    /// Create new typedef info.
    pub fn new(name: String, path: PathBuf) -> Self {
        Self { name, path }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::schema::FieldType;

    #[test]
    fn test_empty_typedef() {
        let td = TypeDefinition::empty("test");
        assert_eq!(td.name, "test");
        assert!(td.schema.is_empty());
        assert!(!td.has_hooks());
    }

    #[test]
    fn test_required_fields() {
        let mut td = TypeDefinition::empty("test");
        td.schema.insert(
            "title".to_string(),
            FieldSchema {
                field_type: Some(FieldType::String),
                required: true,
                ..Default::default()
            },
        );
        td.schema.insert(
            "description".to_string(),
            FieldSchema {
                field_type: Some(FieldType::String),
                required: false,
                ..Default::default()
            },
        );

        let required = td.required_fields();
        assert_eq!(required.len(), 1);
        assert!(required.contains(&"title"));
    }

    #[test]
    fn test_has_hooks() {
        let mut td = TypeDefinition::empty("test");
        assert!(!td.has_hooks());

        td.has_validate_fn = true;
        assert!(td.has_hooks());
    }

    #[test]
    fn test_typedef_info() {
        let info = TypedefInfo::new(
            "meeting".to_string(),
            PathBuf::from("/path/to/meeting.lua"),
        );
        assert_eq!(info.name, "meeting");
        assert_eq!(info.path, PathBuf::from("/path/to/meeting.lua"));
    }
}
