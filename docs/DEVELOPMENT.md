# Development Guide

This document covers the technical structure of `markadd`, development phases, and contribution guidelines.

For usage documentation, see the main [README](../README.md).

## Repository Structure

```text
markadd/
├─ crates/
│  ├─ core/        # config loader, template discovery, template engine
│  ├─ cli/         # command-line interface
│  └─ tui/         # terminal UI (in development)
├─ docs/
│  ├─ CONFIG.md          # configuration reference
│  ├─ templates.md       # template authoring guide
│  ├─ DEVELOPMENT.md     # this file
│  ├─ 00_Conceptualisation.md
│  ├─ 01_development_plan.md
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
- **Error handling**: thiserror
- **Template engine**: regex-based (MVP), with path to Tera
- **Testing**: cargo test, insta (snapshots), assert_cmd (CLI integration)
- **CI**: GitHub Actions (fmt, clippy, test, coverage)

## Development Status

### Completed

**Phase 0 — Workspace, Tooling, CI**
- Workspace crates: `core`, `cli`, `tui`
- Rust 2024 edition
- Strict linting via `.clippy.toml`
- GitHub Actions: fmt, clippy, unit/integration/snapshot tests, coverage
- Deterministic snapshot rules in CI

**Phase 1 — Configuration System**
- `config.toml` with profile support
- XDG discovery, `~` expansion, environment variable expansion
- Interpolation (`{{vault_root}}`, etc.)
- Directory structure for templates / captures / macros
- Command: `markadd doctor`

**Phase 2 — Template Discovery**
- Recursive search in `templates_dir`
- Only `.md` files treated as templates
- Logical names from relative paths
- Command: `markadd list-templates`

**Phase 3 — Template Engine MVP**
- Template repository with logical name lookup
- Simple `{{var}}` substitution
- Built-in context variables (date, time, vault_root, etc.)
- Command: `markadd new --template <name> --output <path>`

### Roadmap

**Phase 4** — Markdown Section Insertion
Use Comrak to insert content into specific headers.

**Phase 5** — File Planner, Atomic Writes, Undo Log

**Phase 6** — CLI Wiring & Coordinator Facade

**Phase 7** — Macro Runner & Security Gates

**Phase 8** — Lua Hooks (optional)

**Phase 9** — TUI Polish

**Phase 10** — Documentation & Release

For detailed phase descriptions with UML diagrams, see [`01_development_plan.md`](./01_development_plan.md).

For the progressive TUI integration proposal, see [`devlogs/tui-integration-proposal.md`](./devlogs/tui-integration-proposal.md).

## Testing

Run full test suite:

```bash
cargo test --all
```

Update snapshots locally:

```bash
INSTA_UPDATE=auto cargo test -p markadd
```

Snapshots are immutable in CI.

### Test Categories

- **Unit tests**: Core logic (config parsing, template rendering)
- **Integration tests**: CLI commands with real filesystems (tempdir)
- **Snapshot tests**: Stable output format verification

## Code Style

- Follow `rustfmt` defaults
- Clippy with strict lints (see `.clippy.toml`)
- Prefer explicit error types over `anyhow` in library code
- Keep CLI layer thin; business logic lives in `core`

## Architecture Principles

1. **Core is reusable**: `markadd-core` should work for CLI, TUI, and future integrations
2. **Determinism**: Same inputs produce same outputs; avoid hidden state
3. **Fail fast**: Validate early, provide actionable error messages
4. **Testability**: Prefer pure functions; inject dependencies where needed

## Contributing

1. Check existing issues or open a new one to discuss
2. Fork and create a feature branch
3. Write tests for new functionality
4. Ensure `cargo fmt` and `cargo clippy` pass
5. Submit a PR with a clear description

## License

See [LICENSE](../LICENSE) in the repository root.
