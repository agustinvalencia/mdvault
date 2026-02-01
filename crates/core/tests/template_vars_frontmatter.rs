use mdvault_core::frontmatter::parse_template_frontmatter;
use mdvault_core::templates::engine::{RenderContext, render};
use mdvault_core::templates::repository::LoadedTemplate;
use std::path::PathBuf;

#[test]
fn template_with_vars_in_frontmatter_parses_successfully() {
    // This is the bug from Report 1 - template with {{var}} in frontmatter should not fail to parse
    let content = r#"---
type: contact
lua: contact.lua
name: {{name}}
email: {{email}}
phone: {{phone}}
position: {{position}}
organisation: {{organisation}}
tags: {{tags}}
---
# {{name}}
[[Contacts]]

## Contact Info
- **Email**: {{email}}
- **Phone**: {{phone}}
- **Position**: {{position}} at {{organisation}}
"#;

    // Should parse without error
    let (fm, raw_fm, body) =
        parse_template_frontmatter(content).expect("should parse successfully");

    // Should have frontmatter (even if not fully parsed due to template vars)
    assert!(fm.is_some());

    // Should have raw frontmatter text
    assert!(raw_fm.is_some());
    let raw = raw_fm.unwrap();
    assert!(raw.contains("name: {{name}}"));

    // Should have body
    assert!(body.contains("# {{name}}"));
}

#[test]
fn template_with_vars_renders_correctly() {
    // This is the bug from Report 2 - variables in frontmatter should be substituted
    let content = r#"---
lua: task.lua
type: task
status: {{status}}
priority: {{priority}}
due_date: {{due_date}}
---
# {{title}}
"#;

    let (fm, raw_fm, body) = parse_template_frontmatter(content).expect("should parse");

    let template = LoadedTemplate {
        logical_name: "task".into(),
        path: PathBuf::from("task.md"),
        content: content.to_string(),
        frontmatter: fm,
        raw_frontmatter: raw_fm,
        body,
    };

    // Create render context with variables
    let mut ctx = RenderContext::new();
    ctx.insert("title".into(), "Test Task".into());
    ctx.insert("status".into(), "todo".into());
    ctx.insert("priority".into(), "high".into());
    ctx.insert("due_date".into(), "2026-02-15".into());

    // Render the template
    let rendered = render(&template, &ctx).expect("should render");

    // Verify frontmatter variables were substituted
    assert!(
        rendered.contains("status: todo"),
        "status should be substituted, got: {}",
        rendered
    );
    assert!(rendered.contains("priority: high"), "priority should be substituted");
    assert!(rendered.contains("due_date: 2026-02-15"), "due_date should be substituted");

    // Verify template-specific fields are filtered out
    assert!(!rendered.contains("lua:"), "lua field should be filtered out");

    // Verify body variables were substituted
    assert!(rendered.contains("# Test Task"), "title in body should be substituted");
}

#[test]
fn template_boolean_values_are_not_quoted() {
    // Regression test: boolean values like true/false should NOT be quoted
    // as strings. They should remain as valid YAML booleans.
    let content = r#"---
type: daily
exercise: false
meds: true
closed: false
intentions: true
---
# Daily note
"#;

    let (fm, raw_fm, body) = parse_template_frontmatter(content).expect("should parse");

    let template = LoadedTemplate {
        logical_name: "daily".into(),
        path: PathBuf::from("daily.md"),
        content: content.to_string(),
        frontmatter: fm,
        raw_frontmatter: raw_fm,
        body,
    };

    let ctx = RenderContext::new();
    let rendered = render(&template, &ctx).expect("should render");

    // Boolean values should NOT be quoted - they should remain as YAML booleans
    assert!(
        rendered.contains("exercise: false"),
        "exercise should be 'false' not '\"false\"', got: {}",
        rendered
    );
    assert!(
        rendered.contains("meds: true"),
        "meds should be 'true' not '\"true\"', got: {}",
        rendered
    );
    assert!(!rendered.contains("exercise: \"false\""), "exercise should NOT be quoted");
    assert!(!rendered.contains("meds: \"true\""), "meds should NOT be quoted");
}
