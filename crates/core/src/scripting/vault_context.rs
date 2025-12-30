//! Vault context for Lua scripting.
//!
//! This module provides the `VaultContext` struct which holds references
//! to all vault repositories needed for executing vault operations from Lua.

use std::sync::Arc;

use crate::captures::CaptureRepository;
use crate::config::types::ResolvedConfig;
use crate::macros::MacroRepository;
use crate::templates::repository::TemplateRepository;
use crate::types::TypeRegistry;

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
        Self {
            config: Arc::new(config),
            template_repo: Arc::new(template_repo),
            capture_repo: Arc::new(capture_repo),
            macro_repo: Arc::new(macro_repo),
            type_registry: Arc::new(type_registry),
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
        Self {
            config,
            template_repo,
            capture_repo,
            macro_repo,
            type_registry,
        }
    }
}
