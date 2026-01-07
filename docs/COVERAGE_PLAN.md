# Code Coverage Improvement Plan

## Current Status Analysis

Based on a structural analysis of the codebase (proxy for full coverage report), we have identified significant gaps in unit testing within the `crates/core` library. While some modules like `vars/datemath` and `scripting/engine` are well-tested, several critical components lack direct unit tests.

### Coverage Heat Map (Proxy)
* **High Coverage:** `vars/datemath`, `scripting/engine`, `index/builder`
* **Low/No Coverage:** `captures/*`, `templates/discovery`, `templates/repository`, `markdown_ast/editor`, `config/loader`

## Strategic Plan

The goal is to increase coverage by targeting low-hanging fruit (modules with logic but no tests) and critical paths.

### Phase 1: Core Discovery & Repository Logic
These modules handle file I/O and structure discovery. They are critical for the tool to function correctly on different file systems.

1.  **`crates/core/src/captures/discovery.rs`**
    *   **Goal:** Verify `discover_captures` finds YAML files and ignores others.
    *   **Method:** Use `tempfile` to create a directory structure with mixed file types and assert the returned list.
    *   **Status:** Completed.

2.  **`crates/core/src/templates/discovery.rs`**
    *   **Goal:** Verify template discovery logic similar to captures.
    *   **Method:** Same as above.
    *   **Status:** Completed.

### Phase 2: AST Manipulation
The `markdown_ast` module is complex and prone to edge cases.

3.  **`crates/core/src/markdown_ast/editor.rs`**
    *   **Goal:** Test specific insertion logic (e.g., "insert after frontmatter", "insert at end").
    *   **Method:** Unit tests with string inputs/outputs (using `comrak` AST where needed, or checking string reconstruction).
    *   **Status:** Completed.

### Phase 3: Configuration Loading
4.  **`crates/core/src/config/loader.rs`**
    *   **Goal:** Test configuration merging and default fallback logic.
    *   **Status:** Completed.

## Immediate Next Steps
All initial phases are complete.
1.  Run the full coverage report (`just coverage`).
2.  Analyze the report to identify the next set of low-coverage areas.
3.  Update this plan with Phase 4.
