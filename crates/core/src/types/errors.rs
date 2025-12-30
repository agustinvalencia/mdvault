//! Error types for type definitions and validation.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur when loading type definitions.
#[derive(Debug, Error)]
pub enum TypedefError {
    /// Type definitions directory does not exist.
    #[error("type definitions directory does not exist: {0}")]
    MissingDir(String),

    /// Error walking the type definitions directory.
    #[error("failed to read types directory {0}: {1}")]
    WalkError(String, #[source] walkdir::Error),

    /// Error reading a type definition file.
    #[error("failed to read type definition file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Error parsing a Lua type definition.
    #[error("failed to parse type definition {path}: {source}")]
    LuaParse {
        path: PathBuf,
        #[source]
        source: crate::scripting::ScriptingError,
    },

    /// Invalid type definition structure.
    #[error("invalid type definition in {path}: {message}")]
    InvalidDefinition { path: PathBuf, message: String },

    /// Type not found.
    #[error("type not found: {0}")]
    NotFound(String),

    /// Duplicate type definition.
    #[error("duplicate type definition: {0}")]
    Duplicate(String),
}

/// Errors that occur during note validation.
#[derive(Debug, Clone, Error)]
pub enum ValidationError {
    /// A required field is missing.
    #[error("missing required field: {field}")]
    MissingRequired { field: String },

    /// Field value has wrong type.
    #[error("invalid type for field '{field}': expected {expected}, got {actual}")]
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },

    /// Field value is invalid.
    #[error("invalid value for field '{field}': {message}")]
    InvalidValue { field: String, message: String },

    /// Enum constraint violated.
    #[error("enum constraint violated for '{field}': '{value}' not in {allowed:?}")]
    EnumViolation {
        field: String,
        value: String,
        allowed: Vec<String>,
    },

    /// Custom validation function failed.
    #[error("custom validation failed: {message}")]
    CustomValidation { message: String },

    /// Lua execution error during validation.
    #[error("Lua error during validation: {0}")]
    LuaError(String),
}

/// Result of validating a note against its type definition.
#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    /// Whether the note is valid.
    pub valid: bool,
    /// Validation errors (empty if valid).
    pub errors: Vec<ValidationError>,
    /// Non-fatal warnings.
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Create a successful validation result.
    pub fn success() -> Self {
        Self {
            valid: true,
            errors: vec![],
            warnings: vec![],
        }
    }

    /// Create a failed validation result.
    pub fn failure(errors: Vec<ValidationError>) -> Self {
        Self {
            valid: false,
            errors,
            warnings: vec![],
        }
    }

    /// Create a failed validation result with a single error.
    pub fn single_error(error: ValidationError) -> Self {
        Self::failure(vec![error])
    }

    /// Add an error to the result.
    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
        self.valid = false;
    }

    /// Add a warning to the result.
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    /// Merge another validation result into this one.
    pub fn merge(&mut self, other: ValidationResult) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
        if !other.valid {
            self.valid = false;
        }
    }
}
