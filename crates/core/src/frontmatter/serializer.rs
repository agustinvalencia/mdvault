//! Frontmatter serialization back to markdown.

use super::types::{Frontmatter, ParsedDocument};
use serde_yaml::Value;
use std::collections::HashMap;

/// Serialize a parsed document back to markdown string.
pub fn serialize(doc: &ParsedDocument) -> String {
    if let Some(fm) = &doc.frontmatter
        && !fm.fields.is_empty()
    {
        let yaml = serialize_frontmatter(&fm.fields);
        return format!("---\n{}---\n\n{}", yaml, doc.body);
    }
    doc.body.clone()
}

/// Serialize frontmatter fields to YAML string.
fn serialize_frontmatter(fields: &HashMap<String, Value>) -> String {
    serde_yaml::to_string(fields).unwrap_or_default()
}

/// Serialize a Frontmatter struct to YAML string (without delimiters).
pub fn frontmatter_to_yaml(fm: &Frontmatter) -> String {
    serialize_frontmatter(&fm.fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::parser::parse;

    #[test]
    fn serialize_document_without_frontmatter() {
        let doc =
            ParsedDocument { frontmatter: None, body: "# Hello\n\nWorld".to_string() };
        assert_eq!(serialize(&doc), "# Hello\n\nWorld");
    }

    #[test]
    fn serialize_document_with_frontmatter() {
        let mut fields = HashMap::new();
        fields.insert("title".to_string(), Value::String("Test".to_string()));

        let doc = ParsedDocument {
            frontmatter: Some(Frontmatter { fields }),
            body: "# Content".to_string(),
        };

        let result = serialize(&doc);
        assert!(result.starts_with("---\n"));
        assert!(result.contains("title: Test"));
        assert!(result.ends_with("---\n\n# Content"));
    }

    #[test]
    fn roundtrip_frontmatter() {
        let original = "---\ntitle: Hello\ncount: 42\n---\n\n# Body";
        let parsed = parse(original).unwrap();
        let serialized = serialize(&parsed);

        // Re-parse and verify
        let reparsed = parse(&serialized).unwrap();
        assert!(reparsed.frontmatter.is_some());
        let fm = reparsed.frontmatter.unwrap();
        assert_eq!(fm.fields.get("title").and_then(|v| v.as_str()), Some("Hello"));
        assert_eq!(fm.fields.get("count").and_then(|v| v.as_i64()), Some(42));
        assert!(reparsed.body.contains("# Body"));
    }
}
