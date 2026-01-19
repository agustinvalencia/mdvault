//! Vault context for Lua scripting.
//!
//! This module provides the `VaultContext` struct which holds references
//! to all vault repositories needed for executing vault operations from Lua.

use std::path::PathBuf;
use std::sync::Arc;

use crate::captures::CaptureRepository;
use crate::config::types::ResolvedConfig;
use crate::index::IndexDb;
use crate::macros::MacroRepository;
use crate::templates::repository::TemplateRepository;
use crate::types::TypeRegistry;

use super::selector::SelectorCallback;

/// Information about the current note being processed.
///
/// This is set when validating or processing a specific note,
/// allowing Lua hooks to access note metadata.
#[derive(Clone, Debug)]
pub struct CurrentNote {
    /// Path to the note relative to vault root.
    pub path: String,
    /// Note type from frontmatter.
    pub note_type: String,
    /// Note title.
    pub title: Option<String>,
    /// Frontmatter as YAML value.
    pub frontmatter: Option<serde_yaml::Value>,
    /// Note content.
    pub content: String,
}

/// Context for vault operations accessible from Lua hooks.
///
/// This struct holds Arc references to avoid cloning large repositories.
/// It's designed to be passed to Lua bindings for template/capture/macro execution.
#[derive(Clone)]
pub struct VaultContext {
    /// Resolved configuration with paths.
    pub config: Arc<ResolvedConfig>,
    /// Template repository for loading templates.
    pub template_repo: Arc<TemplateRepository>,
    /// Capture repository for loading captures.
    pub capture_repo: Arc<CaptureRepository>,
    /// Macro repository for loading macros.
    pub macro_repo: Arc<MacroRepository>,
    /// Type registry for type definitions.
    pub type_registry: Arc<TypeRegistry>,
    /// Optional index database for query operations.
    pub index_db: Option<Arc<IndexDb>>,
    /// Optional current note being processed.
    pub current_note: Option<CurrentNote>,
    /// Vault root path for resolving relative paths.
    pub vault_root: PathBuf,
    /// Optional selector callback for interactive prompts.
    pub selector_callback: Option<SelectorCallback>,
}

impl VaultContext {
    /// Create a new VaultContext from owned values.
    pub fn new(
        config: ResolvedConfig,
        template_repo: TemplateRepository,
        capture_repo: CaptureRepository,
        macro_repo: MacroRepository,
        type_registry: TypeRegistry,
    ) -> Self {
        let vault_root = config.vault_root.clone();
        Self {
            config: Arc::new(config),
            template_repo: Arc::new(template_repo),
            capture_repo: Arc::new(capture_repo),
            macro_repo: Arc::new(macro_repo),
            type_registry: Arc::new(type_registry),
            index_db: None,
            current_note: None,
            vault_root,
            selector_callback: None,
        }
    }

    /// Create a new VaultContext from Arc references.
    pub fn from_arcs(
        config: Arc<ResolvedConfig>,
        template_repo: Arc<TemplateRepository>,
        capture_repo: Arc<CaptureRepository>,
        macro_repo: Arc<MacroRepository>,
        type_registry: Arc<TypeRegistry>,
    ) -> Self {
        let vault_root = config.vault_root.clone();
        Self {
            config,
            template_repo,
            capture_repo,
            macro_repo,
            type_registry,
            index_db: None,
            current_note: None,
            vault_root,
            selector_callback: None,
        }
    }

    /// Set the index database for query operations.
    pub fn with_index(mut self, index_db: Arc<IndexDb>) -> Self {
        self.index_db = Some(index_db);
        self
    }

    /// Set the current note being processed.
    pub fn with_current_note(mut self, note: CurrentNote) -> Self {
        self.current_note = Some(note);
        self
    }

    /// Set the selector callback for interactive prompts.
    ///
    /// The selector callback is called when Lua scripts invoke `mdv.selector()`.
    /// It should display the items to the user and return the selected value.
    pub fn with_selector(mut self, callback: SelectorCallback) -> Self {
        self.selector_callback = Some(callback);
        self
    }
}
