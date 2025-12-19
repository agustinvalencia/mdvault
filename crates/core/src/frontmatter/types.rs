//! Frontmatter types and data structures.

use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;

use crate::vars::VarsMap;

/// Represents parsed YAML frontmatter from a markdown document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frontmatter {
    /// Fields as key-value pairs.
    #[serde(flatten)]
    pub fields: HashMap<String, Value>,
}

/// Result of splitting frontmatter from markdown.
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    /// Parsed frontmatter (if present).
    pub frontmatter: Option<Frontmatter>,
    /// The markdown body (everything after frontmatter).
    pub body: String,
}

/// Template-specific frontmatter fields.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TemplateFrontmatter {
    /// Output path template (supports {{var}} placeholders).
    pub output: Option<String>,

    /// Variable specifications with prompts and defaults.
    #[serde(default)]
    pub vars: Option<VarsMap>,

    /// Other fields are passed through to output.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Frontmatter operations specification.
/// Supports both simple key-value pairs and explicit operations.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum FrontmatterOps {
    /// Simple map form: { field: value } implies "set".
    Simple(HashMap<String, Value>),
    /// List of explicit operations.
    Operations(Vec<FrontmatterOp>),
}

/// A single frontmatter modification operation.
#[derive(Debug, Clone, Deserialize)]
pub struct FrontmatterOp {
    /// Field name to modify.
    pub field: String,
    /// Operation type.
    pub op: FrontmatterOpType,
    /// Value for set/append operations (supports {{var}} in string values).
    #[serde(default)]
    pub value: Option<Value>,
}

/// Type of frontmatter operation.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrontmatterOpType {
    /// Set field to value (creates if missing).
    Set,
    /// Toggle boolean field.
    Toggle,
    /// Increment numeric field.
    Increment,
    /// Append to list field.
    Append,
}
