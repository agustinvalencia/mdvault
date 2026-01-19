//! Field type and schema definitions for type definitions.

use serde::{Deserialize, Serialize};

/// Type of a frontmatter field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    /// String value.
    String,
    /// Numeric value (integer or float).
    Number,
    /// Boolean value.
    Boolean,
    /// Date in YYYY-MM-DD format.
    Date,
    /// ISO 8601 datetime.
    Datetime,
    /// Array of values.
    List,
    /// Link to another note.
    Reference,
}

impl FieldType {
    /// Get the display name for this field type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Date => "date",
            Self::Datetime => "datetime",
            Self::List => "list",
            Self::Reference => "reference",
        }
    }
}

impl std::fmt::Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for FieldType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "string" | "str" => Ok(Self::String),
            "number" | "num" | "int" | "integer" | "float" => Ok(Self::Number),
            "boolean" | "bool" => Ok(Self::Boolean),
            "date" => Ok(Self::Date),
            "datetime" => Ok(Self::Datetime),
            "list" | "array" => Ok(Self::List),
            "reference" | "ref" | "link" | "wikilink" => Ok(Self::Reference),
            _ => Err(format!("unknown field type: {}", s)),
        }
    }
}

/// Schema definition for a single frontmatter field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FieldSchema {
    /// Field type.
    #[serde(rename = "type")]
    pub field_type: Option<FieldType>,

    /// Whether the field is required.
    #[serde(default)]
    pub required: bool,

    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,

    /// Default value (as serde_yaml::Value for flexibility).
    #[serde(default)]
    pub default: Option<serde_yaml::Value>,

    /// Prompt text for interactive input.
    /// If set, user will be prompted for this field during note creation.
    #[serde(default)]
    pub prompt: Option<String>,

    /// Whether this is a core field managed by Rust (not user-modifiable).
    #[serde(default)]
    pub core: bool,

    /// Whether to allow multiline input for string fields.
    #[serde(default)]
    pub multiline: bool,

    /// Whether this field's value will be inherited/set by an on_create hook.
    /// When true:
    /// - The field will NOT be prompted for during note creation
    /// - Validation will skip required checks during creation (pre-hook)
    /// - The on_create hook is responsible for setting the value
    #[serde(default)]
    pub inherited: bool,

    // String constraints
    /// Allowed values for enum fields.
    #[serde(default, rename = "enum")]
    pub enum_values: Option<Vec<String>>,

    /// Regex pattern for validation.
    #[serde(default)]
    pub pattern: Option<String>,

    /// Minimum string length.
    #[serde(default)]
    pub min_length: Option<usize>,

    /// Maximum string length.
    #[serde(default)]
    pub max_length: Option<usize>,

    // Number constraints
    /// Minimum numeric value.
    #[serde(default)]
    pub min: Option<f64>,

    /// Maximum numeric value.
    #[serde(default)]
    pub max: Option<f64>,

    /// Whether the number must be an integer.
    #[serde(default)]
    pub integer: Option<bool>,

    // List constraints
    /// Schema for list items.
    #[serde(default)]
    pub items: Option<Box<FieldSchema>>,

    /// Minimum number of items.
    #[serde(default)]
    pub min_items: Option<usize>,

    /// Maximum number of items.
    #[serde(default)]
    pub max_items: Option<usize>,

    // Reference constraints
    /// Restrict to notes of a specific type.
    #[serde(default)]
    pub note_type: Option<String>,

    // Interactive selection
    /// If set, show a fuzzy selector for notes of this type during prompting.
    /// The selector shows all notes of the specified type and returns the selected
    /// note's path (without .md extension) or a custom field from frontmatter.
    ///
    /// Example in Lua typedef:
    /// ```lua
    /// project = { selector = "project", prompt = "Select project" }
    /// ```
    #[serde(default)]
    pub selector: Option<String>,
}

impl FieldSchema {
    /// Create a new required string field.
    pub fn required_string() -> Self {
        Self { field_type: Some(FieldType::String), required: true, ..Default::default() }
    }

    /// Create a new optional string field.
    pub fn optional_string() -> Self {
        Self {
            field_type: Some(FieldType::String),
            required: false,
            ..Default::default()
        }
    }

    /// Create a new required field with an enum constraint.
    pub fn required_enum(values: Vec<String>) -> Self {
        Self {
            field_type: Some(FieldType::String),
            required: true,
            enum_values: Some(values),
            ..Default::default()
        }
    }

    /// Get the effective field type, defaulting to String if not specified.
    pub fn effective_type(&self) -> FieldType {
        self.field_type.unwrap_or(FieldType::String)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_type_from_str() {
        assert_eq!("string".parse::<FieldType>().unwrap(), FieldType::String);
        assert_eq!("number".parse::<FieldType>().unwrap(), FieldType::Number);
        assert_eq!("boolean".parse::<FieldType>().unwrap(), FieldType::Boolean);
        assert_eq!("date".parse::<FieldType>().unwrap(), FieldType::Date);
        assert_eq!("datetime".parse::<FieldType>().unwrap(), FieldType::Datetime);
        assert_eq!("list".parse::<FieldType>().unwrap(), FieldType::List);
        assert_eq!("reference".parse::<FieldType>().unwrap(), FieldType::Reference);
        // Aliases
        assert_eq!("bool".parse::<FieldType>().unwrap(), FieldType::Boolean);
        assert_eq!("array".parse::<FieldType>().unwrap(), FieldType::List);
        assert_eq!("wikilink".parse::<FieldType>().unwrap(), FieldType::Reference);
    }

    #[test]
    fn test_field_type_display() {
        assert_eq!(FieldType::String.to_string(), "string");
        assert_eq!(FieldType::Number.to_string(), "number");
    }

    #[test]
    fn test_field_schema_defaults() {
        let schema = FieldSchema::default();
        assert!(!schema.required);
        assert!(schema.field_type.is_none());
        assert!(schema.enum_values.is_none());
    }

    #[test]
    fn test_required_string() {
        let schema = FieldSchema::required_string();
        assert!(schema.required);
        assert_eq!(schema.effective_type(), FieldType::String);
    }

    #[test]
    fn test_required_enum() {
        let schema = FieldSchema::required_enum(vec!["a".to_string(), "b".to_string()]);
        assert!(schema.required);
        assert_eq!(schema.enum_values, Some(vec!["a".to_string(), "b".to_string()]));
    }

    #[test]
    fn test_field_schema_with_selector() {
        let schema = FieldSchema {
            field_type: Some(FieldType::String),
            selector: Some("project".to_string()),
            prompt: Some("Select project".to_string()),
            ..Default::default()
        };
        assert_eq!(schema.selector, Some("project".to_string()));
        assert_eq!(schema.prompt, Some("Select project".to_string()));
    }

    #[test]
    fn test_field_schema_selector_deserialization() {
        let yaml = r#"
            type: string
            selector: project
            prompt: "Select project"
            default: inbox
        "#;
        let schema: FieldSchema = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(schema.selector, Some("project".to_string()));
        assert_eq!(schema.prompt, Some("Select project".to_string()));
        assert_eq!(schema.default, Some(serde_yaml::Value::String("inbox".to_string())));
    }
}
