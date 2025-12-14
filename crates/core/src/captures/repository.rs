use std::fs;
use std::path::{Path, PathBuf};

use super::discovery::discover_captures;
use super::types::{
    CaptureDiscoveryError, CaptureInfo, CaptureRepoError, CaptureSpec, LoadedCapture,
};

/// Repository for discovering and loading capture specifications
pub struct CaptureRepository {
    pub root: PathBuf,
    pub captures: Vec<CaptureInfo>,
}

impl CaptureRepository {
    /// Create a new repository by scanning the captures directory
    pub fn new(root: &Path) -> Result<Self, CaptureDiscoveryError> {
        let captures = discover_captures(root)?;
        Ok(Self { root: root.to_path_buf(), captures })
    }

    /// List all discovered captures
    pub fn list_all(&self) -> &[CaptureInfo] {
        &self.captures
    }

    /// Load a capture by its logical name
    pub fn get_by_name(&self, name: &str) -> Result<LoadedCapture, CaptureRepoError> {
        let info = self
            .captures
            .iter()
            .find(|c| c.logical_name == name)
            .ok_or_else(|| CaptureRepoError::NotFound(name.to_string()))?;

        let content = fs::read_to_string(&info.path)
            .map_err(|e| CaptureRepoError::Io { path: info.path.clone(), source: e })?;

        let spec: CaptureSpec = serde_yaml::from_str(&content).map_err(|e| {
            CaptureRepoError::Parse { path: info.path.clone(), source: e }
        })?;

        Ok(LoadedCapture {
            logical_name: info.logical_name.clone(),
            path: info.path.clone(),
            spec,
        })
    }
}
