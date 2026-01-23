use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ConfigFile {
    pub version: u32,
    pub profile: Option<String>,
    pub profiles: HashMap<String, Profile>,
    #[serde(default)]
    pub security: SecurityPolicy,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub activity: ActivityConfig,
}

#[derive(Debug, Deserialize)]
pub struct Profile {
    pub vault_root: String,
    pub templates_dir: String,
    pub captures_dir: String,
    pub macros_dir: String,
    /// Optional override for typedefs directory (defaults to global ~/.config/mdvault/types/)
    pub typedefs_dir: Option<String>,
    /// Folders to exclude from vault operations (relative to vault_root).
    /// These folders and their contents will be ignored by indexing, validation, etc.
    #[serde(default)]
    pub excluded_folders: Vec<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct SecurityPolicy {
    #[serde(default)]
    pub allow_shell: bool,
    #[serde(default)]
    pub allow_http: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub file_level: Option<String>,
    #[serde(default)]
    pub file: Option<PathBuf>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self { level: default_log_level(), file_level: None, file: None }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

/// Configuration for activity logging.
#[derive(Debug, Deserialize, Clone)]
pub struct ActivityConfig {
    /// Whether activity logging is enabled (default: true)
    #[serde(default = "default_activity_enabled")]
    pub enabled: bool,
    /// Number of days to retain logs before rotation (default: 90)
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    /// Which operations to log (empty = all operations)
    #[serde(default)]
    pub log_operations: Vec<String>,
}

impl Default for ActivityConfig {
    fn default() -> Self {
        Self {
            enabled: default_activity_enabled(),
            retention_days: default_retention_days(),
            log_operations: Vec::new(),
        }
    }
}

fn default_activity_enabled() -> bool {
    true
}

fn default_retention_days() -> u32 {
    90
}

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub active_profile: String,
    pub vault_root: PathBuf,
    pub templates_dir: PathBuf,
    pub captures_dir: PathBuf,
    pub macros_dir: PathBuf,
    /// Directory for Lua type definitions (global, not per-profile).
    pub typedefs_dir: PathBuf,
    /// Folders to exclude from vault operations (resolved to absolute paths).
    pub excluded_folders: Vec<PathBuf>,
    pub security: SecurityPolicy,
    pub logging: LoggingConfig,
    pub activity: ActivityConfig,
}
