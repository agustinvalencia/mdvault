use serde::Deserialize;
use std::path::PathBuf;
use thiserror::Error;

use crate::frontmatter::FrontmatterOps;
use crate::markdown_ast::InsertPosition;
use crate::vars::VarsMap;

/// A capture specification loaded from a YAML file
#[derive(Debug, Clone, Deserialize)]
pub struct CaptureSpec {
    /// Logical name of the capture
    pub name: String,

    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// Variable specifications with prompts and defaults.
    #[serde(default)]
    pub vars: Option<VarsMap>,

    /// Target file and section configuration
    pub target: CaptureTarget,

    /// Content template to insert (supports {{var}} placeholders)
    /// Optional: capture may only modify frontmatter without adding content
    #[serde(default)]
    pub content: Option<String>,

    /// Frontmatter operations to apply to the target file
    #[serde(default)]
    pub frontmatter: Option<FrontmatterOps>,
}

/// Target configuration for where to insert captured content
#[derive(Debug, Clone, Deserialize)]
pub struct CaptureTarget {
    /// Path to the target file (supports {{var}} placeholders)
    pub file: String,

    /// Section heading to insert into (optional: not needed for frontmatter-only captures)
    #[serde(default)]
    pub section: Option<String>,

    /// Where in the section to insert (begin or end)
    #[serde(default)]
    pub position: CapturePosition,
}

/// Position within a section (maps to InsertPosition)
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CapturePosition {
    #[default]
    Begin,
    End,
}

impl From<CapturePosition> for InsertPosition {
    fn from(pos: CapturePosition) -> Self {
        match pos {
            CapturePosition::Begin => InsertPosition::Begin,
            CapturePosition::End => InsertPosition::End,
        }
    }
}

/// Information about a discovered capture file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureInfo {
    /// Logical name (filename without .yaml extension)
    pub logical_name: String,
    /// Full path to the YAML file
    pub path: PathBuf,
}

/// A fully loaded capture ready for execution
#[derive(Debug, Clone)]
pub struct LoadedCapture {
    pub logical_name: String,
    pub path: PathBuf,
    pub spec: CaptureSpec,
}

#[derive(Debug, Error)]
pub enum CaptureDiscoveryError {
    #[error("captures directory does not exist: {0}")]
    MissingDir(String),

    #[error("failed to read captures directory {0}: {1}")]
    WalkError(String, #[source] walkdir::Error),
}

#[derive(Debug, Error)]
pub enum CaptureRepoError {
    #[error(transparent)]
    Discovery(#[from] CaptureDiscoveryError),

    #[error("capture not found: {0}")]
    NotFound(String),

    #[error("failed to read capture file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse capture YAML {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
}
