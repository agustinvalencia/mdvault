# markadd

`markadd` is a terminal-first Markdown automation tool inspired by Obsidian’s QuickAdd plugin.  
It allows you to:

- generate Markdown files from templates  
- list templates available in your vault  
- validate configuration profiles  
- (soon) insert content into Markdown sections  
- (later) run capture workflows and programmable macros  

The project is written in Rust with emphasis on speed, determinism, and testability.

For extended documentation and development logs:

`docs/index.md`  
`docs/devlogs/`


## Status

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
- Excludes `*.tpl.md`, `*.tmpl.md`, `.markdown`, `.mdx`  
- Logical names from relative paths  
- Command: `markadd list-templates`


## Repository Structure

```text
markadd/
├─ crates/
│  ├─ core/        # config loader, template discovery, future engines
│  ├─ cli/         # command-line interface (doctor, list-templates)
│  └─ tui/         # placeholder for future TUI
├─ docs/
│  ├─ CONFIG.md
│  └─ devlogs/
├─ .clippy.toml
└─ Cargo.toml
```


## Configuration

`markadd` loads configuration from one of:

- `$XDG_CONFIG_HOME/markadd/config.toml`
- `~/.config/markadd/config.toml`
- `--config <path>`

Example:

```toml
version = 1
profile = "default"

[profiles.default]
vault_root = "~/Notes"
templates_dir = "{{vault_root}}/.markadd/templates"
captures_dir  = "{{vault_root}}/.markadd/captures"
macros_dir    = "{{vault_root}}/.markadd/macros"

[security]
allow_shell = false
allow_http  = false
```

## CLI

Validate and inspect configuration.
```bash
markadd doctor
```

Recursively list Markdown templates found in the active profile.
```bash
markadd list-templates
```


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


## Roadmap

*Phase 3* — Template Engine MVP (next)
Minimal variable substitution and file generation.

*Phase 4* — Markdown Section Insertion
Use *comrak` to insert content into specific headers.

*Phase 5* — Capture Definitions (YAML)

*Phase 6* — Macro System

*Phase 7* — TUI

## Vision

`markadd` aims to be a focused, scriptable, deterministic Markdown automation toolkit for people who prefer working in terminals and integrating with existing vaults or note-management systems.















