//! Frontmatter modification operations.

use super::types::{
    Frontmatter, FrontmatterOp, FrontmatterOpType, FrontmatterOps, ParsedDocument,
};
use regex::Regex;
use serde_yaml::Value;
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during frontmatter modification.
#[derive(Debug, Error)]
pub enum FrontmatterModifyError {
    #[error("field '{0}' is not a boolean, cannot toggle")]
    NotBoolean(String),
    #[error("field '{0}' is not a number, cannot increment")]
    NotNumber(String),
    #[error("field '{0}' is not a list, cannot append")]
    NotList(String),
}

/// Apply frontmatter operations to a document.
pub fn apply_ops(
    mut doc: ParsedDocument,
    ops: &FrontmatterOps,
    render_ctx: &HashMap<String, String>,
) -> Result<ParsedDocument, FrontmatterModifyError> {
    // Ensure frontmatter exists
    if doc.frontmatter.is_none() {
        doc.frontmatter = Some(Frontmatter::default());
    }
    let fm = doc.frontmatter.as_mut().unwrap();

    match ops {
        FrontmatterOps::Simple(map) => {
            for (field, value) in map {
                let rendered_value = render_value(value, render_ctx);
                fm.fields.insert(field.clone(), rendered_value);
            }
        }
        FrontmatterOps::Operations(op_list) => {
            for op in op_list {
                apply_single_op(fm, op, render_ctx)?;
            }
        }
    }

    Ok(doc)
}

/// Apply a single frontmatter operation.
fn apply_single_op(
    fm: &mut Frontmatter,
    op: &FrontmatterOp,
    render_ctx: &HashMap<String, String>,
) -> Result<(), FrontmatterModifyError> {
    match &op.op {
        FrontmatterOpType::Set => {
            if let Some(value) = &op.value {
                let rendered = render_value(value, render_ctx);
                fm.fields.insert(op.field.clone(), rendered);
            }
        }
        FrontmatterOpType::Toggle => {
            let current = fm.fields.get(&op.field);
            match current {
                Some(Value::Bool(b)) => {
                    fm.fields.insert(op.field.clone(), Value::Bool(!b));
                }
                None => {
                    // Default: toggle from false to true
                    fm.fields.insert(op.field.clone(), Value::Bool(true));
                }
                _ => return Err(FrontmatterModifyError::NotBoolean(op.field.clone())),
            }
        }
        FrontmatterOpType::Increment => {
            let current = fm.fields.get(&op.field).cloned();
            let increment = op.value.as_ref().and_then(|v| v.as_i64()).unwrap_or(1);

            match current {
                Some(Value::Number(n)) => {
                    let new_val = n.as_i64().unwrap_or(0) + increment;
                    fm.fields.insert(op.field.clone(), Value::Number(new_val.into()));
                }
                None => {
                    fm.fields.insert(op.field.clone(), Value::Number(increment.into()));
                }
                _ => return Err(FrontmatterModifyError::NotNumber(op.field.clone())),
            }
        }
        FrontmatterOpType::Append => {
            let current = fm.fields.get(&op.field).cloned();
            let append_val = op
                .value
                .as_ref()
                .map(|v| render_value(v, render_ctx))
                .unwrap_or(Value::Null);

            match current {
                Some(Value::Sequence(mut seq)) => {
                    seq.push(append_val);
                    fm.fields.insert(op.field.clone(), Value::Sequence(seq));
                }
                None => {
                    fm.fields.insert(op.field.clone(), Value::Sequence(vec![append_val]));
                }
                _ => return Err(FrontmatterModifyError::NotList(op.field.clone())),
            }
        }
    }
    Ok(())
}

/// Render {{var}} placeholders in YAML values.
fn render_value(value: &Value, ctx: &HashMap<String, String>) -> Value {
    match value {
        Value::String(s) => {
            let rendered = render_string(s, ctx);
            Value::String(rendered)
        }
        // Recursively handle nested structures
        Value::Mapping(map) => {
            let rendered_map: serde_yaml::Mapping =
                map.iter().map(|(k, v)| (k.clone(), render_value(v, ctx))).collect();
            Value::Mapping(rendered_map)
        }
        Value::Sequence(seq) => {
            Value::Sequence(seq.iter().map(|v| render_value(v, ctx)).collect())
        }
        _ => value.clone(),
    }
}

/// Render {{var}} placeholders in a string.
fn render_string(template: &str, ctx: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\{\{([a-zA-Z0-9_]+)\}\}").unwrap();
    re.replace_all(template, |caps: &regex::Captures<'_>| {
        let key = &caps[1];
        ctx.get(key).cloned().unwrap_or_else(|| caps[0].to_string())
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::parser::parse;

    fn make_ctx() -> HashMap<String, String> {
        let mut ctx = HashMap::new();
        ctx.insert("date".to_string(), "2024-01-15".to_string());
        ctx.insert("time".to_string(), "14:30".to_string());
        ctx
    }

    #[test]
    fn test_simple_set() {
        let content = "---\ntitle: Old\n---\n# Content";
        let doc = parse(content).unwrap();

        let mut ops_map = HashMap::new();
        ops_map.insert("title".to_string(), Value::String("New".to_string()));
        ops_map.insert("added".to_string(), Value::Bool(true));

        let ops = FrontmatterOps::Simple(ops_map);
        let result = apply_ops(doc, &ops, &make_ctx()).unwrap();

        let fm = result.frontmatter.unwrap();
        assert_eq!(fm.fields.get("title").and_then(|v| v.as_str()), Some("New"));
        assert_eq!(fm.fields.get("added").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn test_toggle_existing_true() {
        let content = "---\nflag: true\n---\n# Content";
        let doc = parse(content).unwrap();

        let ops = FrontmatterOps::Operations(vec![FrontmatterOp {
            field: "flag".to_string(),
            op: FrontmatterOpType::Toggle,
            value: None,
        }]);

        let result = apply_ops(doc, &ops, &make_ctx()).unwrap();
        let fm = result.frontmatter.unwrap();
        assert_eq!(fm.fields.get("flag").and_then(|v| v.as_bool()), Some(false));
    }

    #[test]
    fn test_toggle_missing_field() {
        let content = "---\nother: value\n---\n# Content";
        let doc = parse(content).unwrap();

        let ops = FrontmatterOps::Operations(vec![FrontmatterOp {
            field: "flag".to_string(),
            op: FrontmatterOpType::Toggle,
            value: None,
        }]);

        let result = apply_ops(doc, &ops, &make_ctx()).unwrap();
        let fm = result.frontmatter.unwrap();
        assert_eq!(fm.fields.get("flag").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn test_increment() {
        let content = "---\ncount: 5\n---\n# Content";
        let doc = parse(content).unwrap();

        let ops = FrontmatterOps::Operations(vec![FrontmatterOp {
            field: "count".to_string(),
            op: FrontmatterOpType::Increment,
            value: None, // Default increment of 1
        }]);

        let result = apply_ops(doc, &ops, &make_ctx()).unwrap();
        let fm = result.frontmatter.unwrap();
        assert_eq!(fm.fields.get("count").and_then(|v| v.as_i64()), Some(6));
    }

    #[test]
    fn test_increment_with_value() {
        let content = "---\ncount: 10\n---\n# Content";
        let doc = parse(content).unwrap();

        let ops = FrontmatterOps::Operations(vec![FrontmatterOp {
            field: "count".to_string(),
            op: FrontmatterOpType::Increment,
            value: Some(Value::Number(5.into())),
        }]);

        let result = apply_ops(doc, &ops, &make_ctx()).unwrap();
        let fm = result.frontmatter.unwrap();
        assert_eq!(fm.fields.get("count").and_then(|v| v.as_i64()), Some(15));
    }

    #[test]
    fn test_append_to_existing_list() {
        let content = "---\nitems:\n  - one\n  - two\n---\n# Content";
        let doc = parse(content).unwrap();

        let ops = FrontmatterOps::Operations(vec![FrontmatterOp {
            field: "items".to_string(),
            op: FrontmatterOpType::Append,
            value: Some(Value::String("three".to_string())),
        }]);

        let result = apply_ops(doc, &ops, &make_ctx()).unwrap();
        let fm = result.frontmatter.unwrap();
        let items = fm.fields.get("items").unwrap().as_sequence().unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[2].as_str(), Some("three"));
    }

    #[test]
    fn test_append_to_new_list() {
        let content = "---\nother: value\n---\n# Content";
        let doc = parse(content).unwrap();

        let ops = FrontmatterOps::Operations(vec![FrontmatterOp {
            field: "items".to_string(),
            op: FrontmatterOpType::Append,
            value: Some(Value::String("first".to_string())),
        }]);

        let result = apply_ops(doc, &ops, &make_ctx()).unwrap();
        let fm = result.frontmatter.unwrap();
        let items = fm.fields.get("items").unwrap().as_sequence().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].as_str(), Some("first"));
    }

    #[test]
    fn test_variable_substitution() {
        let content = "---\n---\n# Content";
        let doc = parse(content).unwrap();

        let mut ops_map = HashMap::new();
        ops_map.insert(
            "modified".to_string(),
            Value::String("{{date}} at {{time}}".to_string()),
        );

        let ops = FrontmatterOps::Simple(ops_map);
        let result = apply_ops(doc, &ops, &make_ctx()).unwrap();

        let fm = result.frontmatter.unwrap();
        assert_eq!(
            fm.fields.get("modified").and_then(|v| v.as_str()),
            Some("2024-01-15 at 14:30")
        );
    }

    #[test]
    fn test_creates_frontmatter_if_missing() {
        let doc =
            ParsedDocument { frontmatter: None, body: "# No frontmatter".to_string() };

        let mut ops_map = HashMap::new();
        ops_map.insert("new_field".to_string(), Value::Bool(true));

        let ops = FrontmatterOps::Simple(ops_map);
        let result = apply_ops(doc, &ops, &make_ctx()).unwrap();

        assert!(result.frontmatter.is_some());
        let fm = result.frontmatter.unwrap();
        assert_eq!(fm.fields.get("new_field").and_then(|v| v.as_bool()), Some(true));
    }
}
