use crate::config::types::{
    ConfigFile, LoggingConfig, Profile, ResolvedConfig, SecurityPolicy,
};
use shellexpand::full;
use std::path::{Path, PathBuf};
use std::{env, fs};

use dirs::home_dir;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config file not found at {0}")]
    NotFound(String),

    #[error("failed to read config file {0}: {1}")]
    ReadError(String, #[source] std::io::Error),

    #[error("failed to parse TOML in {0}: {1}")]
    ParseError(String, #[source] toml::de::Error),

    #[error("profile '{0}' not found")]
    ProfileNotFound(String),

    #[error("no profiles defined in config")]
    NoProfiles,

    #[error("version {0} is unsupported (expected 1)")]
    BadVersion(u32),

    #[error("home directory not available to expand '~'")]
    NoHome,
}

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load(
        config_path: Option<&Path>,
        profile_override: Option<&str>,
    ) -> Result<ResolvedConfig, ConfigError> {
        let path = match config_path {
            Some(p) => p.to_path_buf(),
            None => default_config_path(),
        };

        if !path.exists() {
            return Err(ConfigError::NotFound(path.display().to_string()));
        }

        let s = fs::read_to_string(&path)
            .map_err(|e| ConfigError::ReadError(path.display().to_string(), e))?;

        let cf: ConfigFile = toml::from_str(&s)
            .map_err(|e| ConfigError::ParseError(path.display().to_string(), e))?;

        if cf.version != 1 {
            return Err(ConfigError::BadVersion(cf.version));
        }
        if cf.profiles.is_empty() {
            return Err(ConfigError::NoProfiles);
        }

        let active = profile_override
            .map(ToOwned::to_owned)
            .or(cf.profile.clone())
            .unwrap_or_else(|| "default".to_string());

        let prof = cf
            .profiles
            .get(&active)
            .ok_or_else(|| ConfigError::ProfileNotFound(active.clone()))?;

        let resolved = Self::resolve_profile(&active, prof, &cf.security, &cf.logging)?;
        Ok(resolved)
    }

    fn resolve_profile(
        active: &str,
        prof: &Profile,
        sec: &SecurityPolicy,
        log_cfg: &LoggingConfig,
    ) -> Result<ResolvedConfig, ConfigError> {
        let vault_root = expand_path(&prof.vault_root)?;
        let sub = |s: &str| s.replace("{{vault_root}}", &vault_root.to_string_lossy());

        let templates_dir = expand_path(&sub(&prof.templates_dir))?;
        let captures_dir = expand_path(&sub(&prof.captures_dir))?;
        let macros_dir = expand_path(&sub(&prof.macros_dir))?;
        let typedefs_dir = match &prof.typedefs_dir {
            Some(dir) => expand_path(&sub(dir))?,
            None => default_typedefs_dir(),
        };

        // Resolve log file path if present
        let logging = if let Some(ref file) = log_cfg.file {
            let expanded_file = expand_path(&sub(&file.to_string_lossy()))?;
            LoggingConfig {
                level: log_cfg.level.clone(),
                file_level: log_cfg.file_level.clone(),
                file: Some(expanded_file),
            }
        } else {
            log_cfg.clone()
        };

        Ok(ResolvedConfig {
            active_profile: active.to_string(),
            vault_root,
            templates_dir,
            captures_dir,
            macros_dir,
            typedefs_dir,
            security: sec.clone(),
            logging,
        })
    }
}

pub fn default_config_path() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        return Path::new(&xdg).join("mdvault").join("config.toml");
    }
    let home = home_dir().unwrap_or_else(|| PathBuf::from("~"));
    home.join(".config").join("mdvault").join("config.toml")
}

/// Default directory for Lua type definitions.
/// Global location: ~/.config/mdvault/types/
pub fn default_typedefs_dir() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        return Path::new(&xdg).join("mdvault").join("types");
    }
    let home = home_dir().unwrap_or_else(|| PathBuf::from("~"));
    home.join(".config").join("mdvault").join("types")
}

fn expand_path(input: &str) -> Result<PathBuf, ConfigError> {
    let expanded = full(input).map_err(|_| ConfigError::NoHome)?;
    Ok(PathBuf::from(expanded.to_string()))
}
