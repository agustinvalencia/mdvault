use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::frontmatter::{
    FrontmatterParseError, TemplateFrontmatter, parse_template_frontmatter,
};
use crate::templates::discovery::{
    TemplateDiscoveryError, TemplateInfo, discover_templates,
};

#[derive(Debug, Error)]
pub enum TemplateRepoError {
    #[error(transparent)]
    Discovery(#[from] TemplateDiscoveryError),

    #[error("template not found: {0}")]
    NotFound(String),

    #[error("failed to read template file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse frontmatter in {path}: {source}")]
    FrontmatterParse {
        path: PathBuf,
        #[source]
        source: FrontmatterParseError,
    },
}

#[derive(Debug, Clone)]
pub struct LoadedTemplate {
    pub logical_name: String,
    pub path: PathBuf,
    /// Raw content (includes frontmatter if present).
    pub content: String,
    /// Parsed template frontmatter (if present).
    pub frontmatter: Option<TemplateFrontmatter>,
    /// Body content (excludes frontmatter).
    pub body: String,
}

pub struct TemplateRepository {
    pub root: PathBuf,
    pub templates: Vec<TemplateInfo>,
}

impl TemplateRepository {
    pub fn new(root: &Path) -> Result<Self, TemplateDiscoveryError> {
        let templates = discover_templates(root)?;
        Ok(Self { root: root.to_path_buf(), templates })
    }

    pub fn list_all(&self) -> &[TemplateInfo] {
        &self.templates
    }

    pub fn get_by_name(&self, name: &str) -> Result<LoadedTemplate, TemplateRepoError> {
        let info = self
            .templates
            .iter()
            .find(|t| t.logical_name == name)
            .ok_or_else(|| TemplateRepoError::NotFound(name.to_lowercase()))?;

        let content = fs::read_to_string(&info.path)
            .map_err(|e| TemplateRepoError::Io { path: info.path.clone(), source: e })?;

        let (frontmatter, body) = parse_template_frontmatter(&content).map_err(|e| {
            TemplateRepoError::FrontmatterParse { path: info.path.clone(), source: e }
        })?;

        Ok(LoadedTemplate {
            logical_name: info.logical_name.clone(),
            path: info.path.clone(),
            content,
            frontmatter,
            body,
        })
    }
}
