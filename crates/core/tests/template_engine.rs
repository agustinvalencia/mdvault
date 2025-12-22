use mdvault_core::templates::engine::{RenderContext, render};
use mdvault_core::templates::repository::LoadedTemplate;
use std::path::PathBuf;

fn loaded(contents: &str) -> LoadedTemplate {
    LoadedTemplate {
        logical_name: "test".into(),
        path: PathBuf::from("test.md"),
        content: contents.to_string(),
        frontmatter: None,
        body: contents.to_string(),
    }
}

#[test]
fn render_replaces_known_variables() {
    let tpl = loaded("Hello {{name}}!");
    let mut ctx = RenderContext::new();
    ctx.insert("name".into(), "Agustin".into());

    let out = render(&tpl, &ctx).expect("render ok");
    assert_eq!(out, "Hello Agustin!");
}

#[test]
fn render_leaves_unknown_variables_intact() {
    let tpl = loaded("Hello {{name}} and {{unknown}}!");
    let mut ctx = RenderContext::new();
    ctx.insert("name".into(), "Agustin".into());

    let out = render(&tpl, &ctx).expect("render ok");
    assert_eq!(out, "Hello Agustin and {{unknown}}!");
}
