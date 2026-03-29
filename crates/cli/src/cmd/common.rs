//! Shared command utilities: config loading, index access, error helpers.

use std::path::Path;

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::config::types::ResolvedConfig;
use mdvault_core::index::IndexDb;

/// Load configuration, exiting with a message on failure.
pub fn load_config(config: Option<&Path>, profile: Option<&str>) -> ResolvedConfig {
    match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            std::process::exit(1);
        }
    }
}

/// Open the vault index database, exiting with a hint on failure.
pub fn open_index(vault_root: &Path) -> IndexDb {
    let index_path = vault_root.join(".mdvault/index.db");
    match IndexDb::open(&index_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open index: {e}");
            eprintln!("Hint: run 'mdv reindex' to build the index first.");
            std::process::exit(1);
        }
    }
}
