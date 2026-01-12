//! Context types for note creation.
//!
//! These types carry state through the note creation lifecycle.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::config::types::ResolvedConfig;
use crate::types::{TypeDefinition, TypeRegistry};

/// Core metadata fields managed by Rust.
/// These fields are authoritative and survive template/hook modifications.
#[derive(Debug, Clone, Default)]
pub struct CoreMetadata {
    pub note_type: Option<String>,
    pub title: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub task_counter: Option<u32>,
    pub project: Option<String>, // Parent project for tasks
    pub date: Option<String>,    // For daily notes
    pub week: Option<String>,    // For weekly notes
}

impl CoreMetadata {
    /// Convert to HashMap for merging into frontmatter.
    pub fn to_yaml_map(&self) -> HashMap<String, serde_yaml::Value> {
        let mut map = HashMap::new();
        if let Some(ref t) = self.note_type {
            map.insert("type".into(), serde_yaml::Value::String(t.clone()));
        }
        if let Some(ref t) = self.title {
            map.insert("title".into(), serde_yaml::Value::String(t.clone()));
        }
        if let Some(ref id) = self.project_id {
            map.insert("project-id".into(), serde_yaml::Value::String(id.clone()));
        }
        if let Some(ref id) = self.task_id {
            map.insert("task-id".into(), serde_yaml::Value::String(id.clone()));
        }
        if let Some(counter) = self.task_counter {
            map.insert("task_counter".into(), serde_yaml::Value::Number(counter.into()));
        }
        if let Some(ref p) = self.project {
            map.insert("project".into(), serde_yaml::Value::String(p.clone()));
        }
        if let Some(ref d) = self.date {
            map.insert("date".into(), serde_yaml::Value::String(d.clone()));
        }
        if let Some(ref w) = self.week {
            map.insert("week".into(), serde_yaml::Value::String(w.clone()));
        }
        map
    }
}

/// Context available during note creation.
pub struct CreationContext<'a> {
    // Core inputs
    pub title: String,
    pub type_name: String,

    // Configuration
    pub config: &'a ResolvedConfig,
    pub typedef: Option<Arc<TypeDefinition>>,
    pub registry: &'a TypeRegistry,

    // State (accumulated during creation)
    pub vars: HashMap<String, String>,
    pub core_metadata: CoreMetadata,

    // Output state
    pub output_path: Option<PathBuf>,

    // Mode flags
    pub batch_mode: bool,
}

impl<'a> CreationContext<'a> {
    /// Create a new creation context.
    pub fn new(
        type_name: &str,
        title: &str,
        config: &'a ResolvedConfig,
        registry: &'a TypeRegistry,
    ) -> Self {
        let typedef = registry.get(type_name);

        let core_metadata = CoreMetadata {
            note_type: Some(type_name.to_string()),
            title: Some(title.to_string()),
            ..Default::default()
        };

        let vars = HashMap::from([
            ("title".to_string(), title.to_string()),
            ("type".to_string(), type_name.to_string()),
        ]);

        Self {
            title: title.to_string(),
            type_name: type_name.to_string(),
            config,
            typedef,
            registry,
            vars,
            core_metadata,
            output_path: None,
            batch_mode: false,
        }
    }

    /// Add CLI-provided variables.
    pub fn with_vars(mut self, cli_vars: HashMap<String, String>) -> Self {
        self.vars.extend(cli_vars);
        self
    }

    /// Set batch mode flag.
    pub fn with_batch_mode(mut self, batch: bool) -> Self {
        self.batch_mode = batch;
        self
    }

    /// Get a variable value.
    pub fn get_var(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(|s| s.as_str())
    }

    /// Set a variable value.
    pub fn set_var(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.vars.insert(key.into(), value.into());
    }

    /// Create a PromptContext from this CreationContext.
    pub fn to_prompt_context(&self) -> PromptContext<'_> {
        PromptContext {
            config: self.config,
            type_name: &self.type_name,
            title: &self.title,
            provided_vars: &self.vars,
            batch_mode: self.batch_mode,
        }
    }
}

/// Context for determining prompts.
pub struct PromptContext<'a> {
    pub config: &'a ResolvedConfig,
    pub type_name: &'a str,
    pub title: &'a str,
    pub provided_vars: &'a HashMap<String, String>,
    pub batch_mode: bool,
}

/// A single field prompt specification.
#[derive(Debug, Clone)]
pub struct FieldPrompt {
    pub field_name: String,
    pub prompt_text: String,
    pub prompt_type: PromptType,
    pub required: bool,
    pub default_value: Option<String>,
}

/// Type of prompt to display.
#[derive(Debug, Clone)]
pub enum PromptType {
    /// Single-line text input.
    Text,
    /// Multi-line text input.
    Multiline,
    /// Selection from a list of options.
    Select(Vec<String>),
    /// Special: pick from indexed projects.
    ProjectSelector,
}
