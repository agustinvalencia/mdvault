use std::fs;
use std::path::Path;

use anyhow::Result;
use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options, parse_document};

fn load_doc(s: String) -> Result<String> {
    let path = Path::new(&s);
    let input = fs::read_to_string(path)?;
    Ok(input)
}

fn iterate_nodes<'a>(node: &'a AstNode<'a>, depth: usize) {
    let indent = "  ".repeat(depth);
    match &node.data.borrow().value {
        NodeValue::Text(t) => println!("{}Text: {}", indent, t),
        NodeValue::Heading(h) => println!("{}Heading level: {}", indent, h.level),
        NodeValue::Paragraph => println!("{}Paragraph", indent),
        NodeValue::Document => println!("{}Document", indent),
        NodeValue::CodeBlock(c) => println!("{}", format!("{}CodeBlock \n{:?}", indent, c)),
        _ => println!(
            "{}",
            format!("{}Other node: {:?}", indent, node.data.borrow().value)
        ),
    }

    for child in node.children() {
        iterate_nodes(child, depth + 1);
    }
}

fn main() -> Result<()> {
    let arena = Arena::new();
    let options = Options::default();

    let file = String::from("./examples/test.md");
    let md = load_doc(file)?;

    let root = parse_document(&arena, &md, &options);
    iterate_nodes(root, 0);

    Ok(())
}
