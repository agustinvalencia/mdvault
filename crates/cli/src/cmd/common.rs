//! Shared command utilities: config loading, index access, error helpers.

use std::path::Path;

use color_eyre::eyre::{Result, WrapErr};
use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::config::types::ResolvedConfig;
use mdvault_core::index::IndexDb;
use mdvault_core::paths::PathResolver;

/// Load configuration.
pub fn load_config(
    config: Option<&Path>,
    profile: Option<&str>,
) -> Result<ResolvedConfig> {
    ConfigLoader::load(config, profile).wrap_err("Failed to load config")
}

/// Open the vault index database.
pub fn open_index(vault_root: &Path) -> Result<IndexDb> {
    let index_path = PathResolver::new(vault_root).index_db();
    IndexDb::open(&index_path)
        .wrap_err("Failed to open index. Run 'mdv reindex' to build it")
}
