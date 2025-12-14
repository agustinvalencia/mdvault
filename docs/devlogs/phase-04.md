# Phase 4 — Markdown AST Insertions

**Status**: Complete
**Branch**: `phase-03` (continuation)

## Goal

Insert Markdown fragments at the beginning or end of a named section using an AST, not regex.

## Deliverables

- [x] `MarkdownEditor` struct with Comrak implementation
- [x] Section navigation helpers (find headings, compute section bounds)
- [x] Golden tests and fixtures

## Implementation

### Module Structure

```
crates/core/src/markdown_ast/
├── mod.rs              # Public re-exports
├── types.rs            # Types and error enum
├── editor.rs           # High-level MarkdownEditor API
└── comrak.rs           # Comrak implementation
```

### Public API

```rust
pub struct MarkdownEditor;

impl MarkdownEditor {
    /// Insert a fragment into a named section
    pub fn insert_into_section(
        input: &str,
        section: &SectionMatch,
        fragment: &str,
        position: InsertPosition,
    ) -> Result<InsertResult, MarkdownAstError>;

    /// Find all headings in a document
    pub fn find_headings(input: &str) -> Vec<HeadingInfo>;

    /// Check if a section exists in the document
    pub fn section_exists(input: &str, section: &SectionMatch) -> bool;
}
```

### Types

```rust
pub enum InsertPosition {
    Begin,  // After section heading
    End,    // Before next heading or EOF
}

pub struct SectionMatch {
    pub title: String,
    pub case_sensitive: bool,  // default: false
}

pub struct HeadingInfo {
    pub title: String,
    pub level: u8,
}

pub struct InsertResult {
    pub content: String,
    pub matched_heading: HeadingInfo,
}

pub enum MarkdownAstError {
    SectionNotFound(String),
    EmptyDocument,
    RenderError(String),
}
```

## Design Decisions

### 1. Struct over Trait

Followed existing codebase patterns (`ConfigLoader`, `TemplateRepository`). No trait abstraction needed yet — can be extracted later if multiple backends are required.

### 2. Case-Insensitive by Default

Markdown heading text is display text; users expect "Inbox" to match "inbox". Optional `case_sensitive` flag available for edge cases.

### 3. Section Bounds Algorithm

A section starts at a heading and ends at:
1. Next heading of same or higher level (lower number)
2. End of document

This matches common Markdown semantics.

### 4. Fragment Parsed as Markdown

The fragment string is parsed into an AST and its nodes are spliced into position, ensuring proper formatting preservation.

## Edge Cases Handled

| Case | Behavior |
|------|----------|
| Section not found | `Err(SectionNotFound)` |
| Empty document | `Err(EmptyDocument)` |
| Code block with `#` | Ignored (Comrak treats as Code, not Heading) |
| Empty fragment | No-op, return Ok with unchanged content |
| Empty section | Insert after heading |
| Last section | Extends to EOF |
| Setext headings | Supported (both `===` and `---`) |
| Multiple same-name sections | Matches first occurrence |

## Dependencies Added

```toml
[dependencies]
comrak = "0.35"

[dev-dependencies]
insta = "1.43"
```

## Test Coverage

### Unit Tests (22 tests)
- Basic insertion (begin/end)
- Case sensitivity options
- Error conditions
- Code block handling
- Nested sections
- Setext headings
- Multiple same-name sections

### Golden Tests (5 tests)
- Changelog insertion scenarios
- Complex document formatting preservation
- Heading discovery

### Fixtures
- `fixtures/changelog_simple.md`
- `fixtures/changelog_complex.md`

## Integration Points

This module will be consumed by:
- **Phase 5**: `FilePlan` transforms for capture operations
- **Phase 6**: `Coordinator` for `capture` command execution

## Files Changed

### New Files
- `crates/core/src/markdown_ast/mod.rs`
- `crates/core/src/markdown_ast/types.rs`
- `crates/core/src/markdown_ast/editor.rs`
- `crates/core/src/markdown_ast/comrak.rs`
- `crates/core/tests/markdown_ast_insert.rs`
- `crates/core/tests/markdown_ast_golden.rs`
- `crates/core/tests/fixtures/changelog_simple.md`
- `crates/core/tests/fixtures/changelog_complex.md`
- `crates/core/tests/snapshots/*.snap` (5 files)

### Modified Files
- `crates/core/Cargo.toml` — Added comrak, insta
- `crates/core/src/lib.rs` — Added `pub mod markdown_ast;`
