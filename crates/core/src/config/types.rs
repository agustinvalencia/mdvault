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
}

#[derive(Debug, Deserialize)]
pub struct Profile {
    pub vault_root: String,
    pub templates_dir: String,
    pub captures_dir: String,
    pub macros_dir: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct SecurityPolicy {
    #[serde(default)]
    pub allow_shell: bool,
    #[serde(default)]
    pub allow_http: bool,
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
    pub security: SecurityPolicy,
}
