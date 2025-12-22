# Development Guide

This document covers the technical structure of `mdvault` (formerly `markadd`), development phases, and contribution guidelines.

For usage documentation, see the main [README](../README.md).

## Repository Structure

```text
mdvault/
├─ crates/
│  ├─ core/        # config loader, templates, captures, markdown AST, frontmatter
│  └─ cli/         # command-line interface and TUI
├─ docs/
│  ├─ config.md          # configuration reference
│  ├─ templates.md       # template authoring guide
│  ├─ capture.md         # captures reference
│  ├─ macros.md          # macros reference
│  ├─ development.md     # this file
│  ├─ 00_Conceptualisation.md
│  ├─ 01_development_plan.md
│  ├─ 02_revised_development_plan.md
│  ├─ 03_focus_change.md # scope evolution
│  └─ devlogs/           # per-phase development logs
├─ .clippy.toml
├─ .github/
│  └─ workflows/
│     └─ ci.yml
└─ Cargo.toml
```

## Tech Stack

- **Language**: Rust (2024 edition)
- **CLI parsing**: clap
- **Error handling**: thiserror, color-eyre (TUI)
- **Template engine**: regex-based (MVP), with path to Tera
- **Markdown AST**: comrak (CommonMark parser)
- **TUI**: ratatui, crossterm
- **Testing**: cargo test, insta (snapshots), assert_cmd (CLI integration)
- **CI**: GitHub Actions (fmt, clippy, test, coverage)

## Development Status

### Completed (v0.1.x)

**Phase 0 — Workspace, Tooling, CI**
- Workspace crates: `core`, `cli`
- Rust 2024 edition
- Strict linting via `.clippy.toml`
- GitHub Actions: fmt, clippy, unit/integration/snapshot tests, coverage
- Deterministic snapshot rules in CI

**Phase 1 — Configuration System**
- `config.toml` with profile support
- XDG discovery, `~` expansion, environment variable expansion
- Interpolation (`{{vault_root}}`, etc.)
- Directory structure for templates / captures / macros
- Command: `mdv doctor`

**Phase 2 — Template Discovery**
- Recursive search in `templates_dir`
- Only `.md` files treated as templates
- Logical names from relative paths
- Command: `mdv list-templates`

**Phase 3 — Template Engine MVP**
- Template repository with logical name lookup
- Simple `{{var}}` substitution
- Built-in context variables (date, time, vault_root, etc.)
- Command: `mdv new --template <name> --output <path>`

**Phase 4 — Markdown AST Insertions**
- `MarkdownEditor` API for section-based insertions
- Comrak-based AST parsing (not regex)
- Insert at beginning or end of named sections
- Case-insensitive section matching (configurable)
- Support for ATX and Setext headings
- Golden/snapshot tests for complex documents

**Phase 4 MVP — Captures**
- Capture specs (YAML) with target file, section, position
- Variable substitution in file paths and content
- Command: `mdv capture <name> --var key=value`
- Integration tests for capture workflows

**Frontmatter System**
- YAML frontmatter parsing and serialization
- Frontmatter modification operations (set, toggle, increment, append)
- Template frontmatter with `output` field for default output paths
- Capture frontmatter operations for modifying target file frontmatter
- Frontmatter-only captures (no content insertion required)

**TUI Integration**
- TUI integrated into CLI crate (ratatui + crossterm)
- Launch TUI by running `mdv` without subcommand
- Template and capture browsing
- Elm-inspired architecture (App state, Message, update loop)

**Macros**
- Multi-step workflow execution
- Template + capture step combinations
- Variable passing between steps
- Shell command execution (with `--trust`)

### Roadmap (v0.2.0+)

The project scope has evolved from QuickAdd-style automation to a complete terminal vault manager. See [03_focus_change.md](./03_focus_change.md) for the full vision.

**Priority 1 — Search Command (v0.2.0)**
```bash
mdv search "network optimization"
mdv search "TODO" --folder projects --context-lines 3
mdv search "query" --format json
```

**Priority 2 — Query Command (v0.3.0)**
```bash
mdv query --where "status=todo"
mdv query --where "due<2025-01-01" --where "priority=high"
mdv query --tag research --sort-by "created"
```

**Priority 3 — Links Command (v0.4.0)**
```bash
mdv links note.md --backlinks
mdv links note.md --outgoing
mdv links --orphans --folder research
```

**Future (v0.5.0+)**
- List/Browse enhancements
- Read/View commands
- Batch operations

For detailed feature specifications, see [03_focus_change.md](./03_focus_change.md).

## Testing

Run full test suite:

```bash
cargo test --all
```

Update snapshots locally:

```bash
INSTA_UPDATE=auto cargo test -p mdvault
```

Snapshots are immutable in CI.

### Test Categories

- **Unit tests**: Core logic (config parsing, template rendering, markdown AST)
- **Integration tests**: CLI commands with real filesystems (tempdir)
- **Snapshot tests**: Stable output format verification (CLI output, markdown transformations)

## Code Style

- Follow `rustfmt` defaults
- Clippy with strict lints (see `.clippy.toml`)
- Prefer explicit error types over `anyhow` in library code
- Keep CLI layer thin; business logic lives in `core`

## Architecture Principles

1. **Core is reusable**: `mdvault-core` should work for CLI, TUI, and future integrations
2. **Determinism**: Same inputs produce same outputs; avoid hidden state
3. **Fail fast**: Validate early, provide actionable error messages
4. **Testability**: Prefer pure functions; inject dependencies where needed
5. **MCP integration ready**: JSON output formats for tooling

## Contributing

1. Check existing issues or open a new one to discuss
2. Fork and create a feature branch
3. Write tests for new functionality
4. Ensure `cargo fmt` and `cargo clippy` pass
5. Submit a PR with a clear description

## License

See [LICENSE](../LICENSE) in the repository root.
