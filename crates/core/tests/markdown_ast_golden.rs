use insta::assert_snapshot;
use mdvault_core::markdown_ast::{InsertPosition, MarkdownEditor, SectionMatch};

#[test]
fn golden_changelog_insert_unreleased_begin() {
    let input = include_str!("fixtures/changelog_simple.md");

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Unreleased"),
        "### Added\n\n- New feature from test\n",
        InsertPosition::Begin,
    )
    .unwrap();

    assert_snapshot!(result.content);
}

#[test]
fn golden_changelog_insert_added_end() {
    let input = include_str!("fixtures/changelog_simple.md");

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Added"),
        "- Another new feature\n",
        InsertPosition::End,
    )
    .unwrap();

    assert_snapshot!(result.content);
}

#[test]
fn golden_complex_document_preserves_formatting() {
    let input = include_str!("fixtures/changelog_complex.md");

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Added"),
        "- Complex item with `code` and **bold**\n",
        InsertPosition::End,
    )
    .unwrap();

    assert_snapshot!(result.content);
}

#[test]
fn golden_insert_into_changed_with_code_block() {
    let input = include_str!("fixtures/changelog_complex.md");

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Changed"),
        "- New change entry\n",
        InsertPosition::End,
    )
    .unwrap();

    assert_snapshot!(result.content);
}

#[test]
fn golden_find_headings_complex() {
    let input = include_str!("fixtures/changelog_complex.md");

    let headings = MarkdownEditor::find_headings(input);
    let headings_str: Vec<String> =
        headings.iter().map(|h| format!("L{}: {}", h.level, h.title)).collect();

    assert_snapshot!(headings_str.join("\n"));
}
