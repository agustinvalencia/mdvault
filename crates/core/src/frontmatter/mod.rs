//! Frontmatter parsing, modification, and serialization.
//!
//! This module provides functionality to:
//! - Parse YAML frontmatter from markdown documents
//! - Modify frontmatter fields (set, toggle, increment, append)
//! - Serialize documents back to markdown with frontmatter

pub mod modifier;
pub mod parser;
pub mod serializer;
pub mod types;

pub use modifier::apply_ops;
pub use parser::{FrontmatterParseError, parse, parse_template_frontmatter};
pub use serializer::{serialize, serialize_with_order};
pub use types::{
    Frontmatter, FrontmatterOp, FrontmatterOpType, FrontmatterOps, ParsedDocument,
    TemplateFrontmatter,
};
