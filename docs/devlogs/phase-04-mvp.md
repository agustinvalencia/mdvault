# Phase 4 MVP — Captures Feature Addition

**Status**: Complete
**Branch**: `phase-03` (continuation)

## Plan Modification

The original development plan had captures implemented in Phase 6 (CLI Wiring), after Phase 5 (File Planner, Atomic Writes, Undo Log). However, for MVP purposes, we prioritized getting a working `capture` command earlier.

### Original Plan
```
Phase 4: Markdown AST (Comrak) ✓
Phase 5: File Planner, Atomic Writes, Undo Log
Phase 6: CLI Wiring (template, capture, macro commands)
```

### Modified Plan (MVP-focused)
```
Phase 4: Markdown AST (Comrak) ✓
Phase 4-MVP: Captures Command ✓  <-- Added
Phase 5: File Planner, Atomic Writes, Undo Log (deferred)
Phase 6: CLI Wiring (remaining: macro command)
```

### Rationale

For a useful MVP, you need both core features:
1. **Templates** (`markadd new`) - Create new files ✓
2. **Captures** (`markadd capture`) - Append to existing files ✓

The safety features (atomic writes, undo log) are valuable but not essential for initial usability. They can be added as enhancements later.

## Implementation Summary

### Core Module: `captures/`

```
crates/core/src/captures/
├── mod.rs              # Public re-exports
├── types.rs            # CaptureSpec, CaptureTarget, CapturePosition, errors
├── discovery.rs        # Find .yaml/.yml capture files
└── repository.rs       # CaptureRepository for loading captures
```

### CLI Command: `capture`

```bash
markadd capture <name> [--var key=value]...
```

**Flow:**
1. Load config and capture repository
2. Find capture spec by logical name
3. Build context (date, time, config paths + user vars)
4. Render target file path and content
5. Read existing target file
6. Use `MarkdownEditor::insert_into_section` to insert content
7. Write file back

### Capture Spec Format (YAML)

```yaml
name: inbox
description: Add a quick note to today's inbox

target:
  file: "daily/{{date}}.md"      # Supports {{var}} placeholders
  section: "Inbox"               # Section heading to find
  position: begin                # begin or end

content: "- [ ] {{text}}"        # Content to insert
```

### Dependencies Added

```toml
# crates/core/Cargo.toml
serde_yaml = "0.9"

# crates/cli/Cargo.toml
chrono = "0.4.42"
regex = "1.12.2"
```

## Test Coverage

### Integration Tests (5 tests)
- `capture_inserts_at_section_begin` - Insert at start of section
- `capture_inserts_at_section_end` - Insert at end of section
- `capture_fails_on_missing_section` - Error with available sections listed
- `capture_fails_on_missing_file` - Error when target doesn't exist
- `capture_not_found_shows_available` - Error lists available captures

## Usage Example

```bash
# Setup: Create a daily note with an Inbox section
echo "# Daily Note

## Inbox

## Done
" > ~/vault/daily/2024-01-15.md

# Create capture spec
cat > ~/.markadd/captures/inbox.yaml << 'EOF'
name: inbox
target:
  file: "daily/{{date}}.md"
  section: "Inbox"
  position: begin
content: "- [ ] {{text}}"
EOF

# Capture a quick note
markadd capture inbox --var text="Review PR #42"

# Result: The note is inserted at the beginning of the Inbox section
```

## Files Changed

### New Files
- `crates/core/src/captures/mod.rs`
- `crates/core/src/captures/types.rs`
- `crates/core/src/captures/discovery.rs`
- `crates/core/src/captures/repository.rs`
- `crates/cli/src/cmd/capture.rs`
- `crates/cli/tests/capture_simple.rs`
- `examples/.markadd/captures/inbox.yaml`
- `examples/.markadd/captures/todo.yaml`

### Modified Files
- `crates/core/Cargo.toml` - Added serde_yaml
- `crates/core/src/lib.rs` - Added `pub mod captures;`
- `crates/cli/Cargo.toml` - Added chrono, regex
- `crates/cli/src/cmd/mod.rs` - Added capture module
- `crates/cli/src/main.rs` - Added Capture command and args

## MVP Status

With this implementation, markadd has both core features needed for an MVP:

| Feature | Command | Status |
|---------|---------|--------|
| Templates | `markadd new --template <name> --output <path>` | ✓ Complete |
| Captures | `markadd capture <name> --var key=value` | ✓ Complete |

## Future Enhancements (deferred from Phase 5)

- Atomic writes (temp file + rename)
- Undo log (JSONL operation history)
- `markadd undo` command
