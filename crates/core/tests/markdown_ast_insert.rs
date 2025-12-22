use mdvault_core::markdown_ast::{
    InsertPosition, MarkdownAstError, MarkdownEditor, SectionMatch,
};

// === Basic insertion tests ===

#[test]
fn insert_at_begin_of_section() {
    let input = r#"# Changelog

## Unreleased

### Added

- Feature A

## 1.0.0
"#;

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Added"),
        "- Feature B\n",
        InsertPosition::Begin,
    )
    .unwrap();

    // Feature B should appear before Feature A
    let feature_b_pos = result.content.find("Feature B").unwrap();
    let feature_a_pos = result.content.find("Feature A").unwrap();
    assert!(feature_b_pos < feature_a_pos);
}

#[test]
fn insert_at_end_of_section() {
    let input = r#"# Inbox

- Task 1
- Task 2

# Done
"#;

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Inbox"),
        "- Task 3\n",
        InsertPosition::End,
    )
    .unwrap();

    // Task 3 should appear after Task 2 but before Done
    let task2_pos = result.content.find("Task 2").unwrap();
    let task3_pos = result.content.find("Task 3").unwrap();
    let done_pos = result.content.find("# Done").unwrap();
    assert!(task2_pos < task3_pos);
    assert!(task3_pos < done_pos);
}

// === Section matching tests ===

#[test]
fn case_insensitive_match_default() {
    let input = "# INBOX\n\nContent\n";

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("inbox"),
        "New item\n",
        InsertPosition::Begin,
    );

    assert!(result.is_ok());
}

#[test]
fn case_sensitive_match_fails_when_case_differs() {
    let input = "# INBOX\n\nContent\n";

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("inbox").case_sensitive(true),
        "New item\n",
        InsertPosition::Begin,
    );

    assert!(matches!(result, Err(MarkdownAstError::SectionNotFound(_))));
}

#[test]
fn case_sensitive_match_succeeds_when_case_matches() {
    let input = "# INBOX\n\nContent\n";

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("INBOX").case_sensitive(true),
        "New item\n",
        InsertPosition::Begin,
    );

    assert!(result.is_ok());
}

#[test]
fn trimmed_title_matching() {
    let input = "# Inbox   \n\nContent\n"; // Trailing spaces in heading

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Inbox"),
        "New\n",
        InsertPosition::Begin,
    );

    assert!(result.is_ok());
}

// === Edge cases ===

#[test]
fn section_not_found_error() {
    let input = "# Existing\n\nContent\n";

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("NonExistent"),
        "Fragment\n",
        InsertPosition::Begin,
    );

    match result {
        Err(MarkdownAstError::SectionNotFound(s)) => assert_eq!(s, "NonExistent"),
        _ => panic!("Expected SectionNotFound error"),
    }
}

#[test]
fn empty_document_error() {
    let result = MarkdownEditor::insert_into_section(
        "",
        &SectionMatch::new("Any"),
        "Fragment\n",
        InsertPosition::Begin,
    );

    assert!(matches!(result, Err(MarkdownAstError::EmptyDocument)));
}

#[test]
fn whitespace_only_document_error() {
    let result = MarkdownEditor::insert_into_section(
        "   \n\n   ",
        &SectionMatch::new("Any"),
        "Fragment\n",
        InsertPosition::Begin,
    );

    assert!(matches!(result, Err(MarkdownAstError::EmptyDocument)));
}

#[test]
fn empty_fragment_is_noop() {
    let input = "# Section\n\nOriginal\n";

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Section"),
        "",
        InsertPosition::Begin,
    )
    .unwrap();

    // Content should be unchanged
    assert!(result.content.contains("Original"));
}

#[test]
fn code_block_with_hash_not_matched_as_heading() {
    let input = r#"# Real Heading

```bash
# This is a comment, not a heading
echo "hello"
```

# Another Heading
"#;

    let headings = MarkdownEditor::find_headings(input);
    assert_eq!(headings.len(), 2);
    assert_eq!(headings[0].title, "Real Heading");
    assert_eq!(headings[1].title, "Another Heading");
}

#[test]
fn last_section_extends_to_eof() {
    let input = "# Only Section\n\nLine 1\n\nLine 2\n";

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Only Section"),
        "New line\n",
        InsertPosition::End,
    )
    .unwrap();

    // New line should appear after Line 2
    let line2_pos = result.content.find("Line 2").unwrap();
    let newline_pos = result.content.find("New line").unwrap();
    assert!(newline_pos > line2_pos);
}

#[test]
fn insert_into_empty_section() {
    let input = "# Empty Section\n\n# Next Section\n\nContent\n";

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Empty Section"),
        "New content\n",
        InsertPosition::Begin,
    )
    .unwrap();

    // New content should appear after Empty Section but before Next Section
    let empty_section_pos = result.content.find("# Empty Section").unwrap();
    let new_content_pos = result.content.find("New content").unwrap();
    let next_section_pos = result.content.find("# Next Section").unwrap();
    assert!(new_content_pos > empty_section_pos);
    assert!(new_content_pos < next_section_pos);
}

#[test]
fn nested_sections_respects_level() {
    let input = r#"# Level 1

## Level 2 A

Content A

### Level 3

Deep content

## Level 2 B

Content B
"#;

    // Insert into Level 2 A - should only affect until Level 2 B
    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Level 2 A"),
        "New A content\n",
        InsertPosition::End,
    )
    .unwrap();

    // Verify insertion is before "## Level 2 B"
    let level2b_pos = result.content.find("## Level 2 B").unwrap();
    let new_content_pos = result.content.find("New A content").unwrap();
    assert!(new_content_pos < level2b_pos);
}

#[test]
fn multiple_same_name_sections_matches_first() {
    let input = r#"# Inbox

First inbox content

# Other

# Inbox

Second inbox content
"#;

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Inbox"),
        "New item\n",
        InsertPosition::Begin,
    )
    .unwrap();

    // Should insert after FIRST "# Inbox"
    let first_inbox = result.content.find("# Inbox").unwrap();
    let new_item = result.content.find("New item").unwrap();

    // Find second inbox after the first
    let after_first = &result.content[first_inbox + 7..];
    let second_inbox = after_first.find("# Inbox").unwrap() + first_inbox + 7;

    assert!(new_item > first_inbox && new_item < second_inbox);
}

// === find_headings tests ===

#[test]
fn find_headings_returns_all_levels() {
    let input = r#"# H1

## H2

### H3

#### H4

##### H5

###### H6
"#;

    let headings = MarkdownEditor::find_headings(input);
    assert_eq!(headings.len(), 6);
    assert_eq!(headings[0].level, 1);
    assert_eq!(headings[1].level, 2);
    assert_eq!(headings[2].level, 3);
    assert_eq!(headings[3].level, 4);
    assert_eq!(headings[4].level, 5);
    assert_eq!(headings[5].level, 6);
}

#[test]
fn find_headings_preserves_order() {
    let input = r#"# First

## Second

# Third
"#;

    let headings = MarkdownEditor::find_headings(input);
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0].title, "First");
    assert_eq!(headings[1].title, "Second");
    assert_eq!(headings[2].title, "Third");
}

// === section_exists tests ===

#[test]
fn section_exists_returns_true_for_existing() {
    let input = "# Existing\n\nContent\n";
    assert!(MarkdownEditor::section_exists(input, &SectionMatch::new("Existing")));
}

#[test]
fn section_exists_returns_false_for_missing() {
    let input = "# Existing\n\nContent\n";
    assert!(!MarkdownEditor::section_exists(input, &SectionMatch::new("Missing")));
}

// === Setext headings ===

#[test]
fn setext_headings_level_1() {
    let input = r#"Main Title
===========

Content under main.
"#;

    let headings = MarkdownEditor::find_headings(input);
    assert_eq!(headings.len(), 1);
    assert_eq!(headings[0].title, "Main Title");
    assert_eq!(headings[0].level, 1);
}

#[test]
fn setext_headings_level_2() {
    let input = r#"Subtitle
--------

Subtitle content.
"#;

    let headings = MarkdownEditor::find_headings(input);
    assert_eq!(headings.len(), 1);
    assert_eq!(headings[0].title, "Subtitle");
    assert_eq!(headings[0].level, 2);
}

#[test]
fn insert_into_setext_heading() {
    let input = r#"My Section
===========

Original content.
"#;

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("My Section"),
        "New content\n",
        InsertPosition::Begin,
    )
    .unwrap();

    assert!(result.content.contains("New content"));
}

// === Wikilinks and special character preservation ===

#[test]
fn preserves_wikilinks_in_existing_content() {
    let input = r#"# Notes

- [[wikilink]]
- [[page#section]]
- Regular text

# Other
"#;

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Notes"),
        "- New item\n",
        InsertPosition::End,
    )
    .unwrap();

    // Wikilinks should NOT be escaped
    assert!(
        result.content.contains("[[wikilink]]"),
        "Wikilinks should be preserved, got: {}",
        result.content
    );
    assert!(
        result.content.contains("[[page#section]]"),
        "Wikilinks with sections should be preserved, got: {}",
        result.content
    );
}

#[test]
fn preserves_wikilinks_in_inserted_content() {
    let input = r#"# Notes

- Existing item

# Other
"#;

    let result = MarkdownEditor::insert_into_section(
        input,
        &SectionMatch::new("Notes"),
        "- [[new wikilink]]\n",
        InsertPosition::End,
    )
    .unwrap();

    // Inserted wikilinks should NOT be escaped
    assert!(
        result.content.contains("[[new wikilink]]"),
        "Inserted wikilinks should be preserved, got: {}",
        result.content
    );
}
