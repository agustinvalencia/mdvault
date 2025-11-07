# `markadd`

`markadd` is a Rust-powered Markdown automation tool inspired by Obsidian’s QuickAdd plugin, designed for terminal-first workflows.  
It allows you to create files from templates, insert content into existing Markdown sections, and eventually define higher-level macros that combine templating and capture logic.

The project is in active development.  
This README documents the current implemented capabilities and the roadmap.

## Status

### Completed so far (Phases 0–1)
- Workspace structure with `core`, `cli`, and `tui` crates.
- Rust 2024 edition across the workspace.
- Robust configuration system using TOML with support for:
  - multiple profiles
  - directory interpolation
  - XDG directory support
  - vault/templating/capture/macro directories
  - security flags (not yet enforced)
- `markadd doctor` command for inspecting and validating configuration.
- Comprehensive test suite:
  - core unit tests
  - CLI integration tests
  - snapshot tests using `insta`
- Deterministic snapshot behaviour:
  - local: snapshots can be updated manually
  - CI: snapshot updates are forbidden (`INSTA_UPDATE=no`)
- GitHub Actions CI pipeline:
  - fmt, clippy, tests
  - coverage using `cargo tarpaulin`

This foundation sets the stage for templating, capture, section insertion, macro scripting, and eventually a TUI.

## Repository Structure

```
markadd/
├─ .clippy.toml
├─ .github/workflows/ci.yml
├─ crates/
│  ├─ core/         # configuration loader, error types, upcoming template/capture engines
│  ├─ cli/          # markadd binary, doctor command, upcoming commands
│  └─ tui/          # future interactive interface
├─ docs/
│  └─ CONFIG.md
└─ Cargo.toml       # workspace root
```

# Configuration

`markadd` loads configuration from:
 - `$XDG_CONFIG_HOME/markadd/config.toml`
 - `~/.config/markadd/config.toml`
 - or via `--config <path>` in the CLI

A complete example:

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

Path interpolation rules:
 - `~` is expanded to the user’s home
 - environment variables are expanded
 - `{{vault_root}}` is expanded inside directory paths

# CLI

Currently implemented:

## markadd doctor

Validates the configuration and prints the resolved profile, paths, and security flags.

Usage:

```shell
markadd doctor
markadd doctor --config path/to/config.toml
markadd doctor --profile work
```


Example output:

```shell
OK   markadd doctor
path: ~/.config/markadd/config.toml
profile: default
vault_root: /home/user/Notes
templates_dir: /home/user/Notes/.markadd/templates
captures_dir: /home/user/Notes/.markadd/captures
macros_dir: /home/user/Notes/.markadd/macros
security.allow_shell: false
security.allow_http:  false
```


# Tests

Run all tests:

```shell
cargo test --all
```

Snapshot tests (locally, allow updates):

```shell
INSTA_UPDATE=auto cargo test -p markadd
```


Snapshot behaviour is deterministic in CI (`INSTA_UPDATE=no`).

# Continuous Integration

The CI pipeline performs:
- `rustfmt check`
- `clippy with -D warnings`
- unit, integration, and snapshot tests
- `tarpaulin` coverage


# Development Roadmap

The following phases describe where markadd is heading.

## Phase 2 — Template Discovery
 - Read templates from configured directory.
 - CLI: `markadd list-templates`.

## Phase 3 — Template Engine MVP
 - Basic variable substitution: date, vault paths, file metadata.
 - CLI: `markadd new --template <name> --output <path>`.

## Phase 4 — Markdown Section Insertion
 - Markdown parsing using comrak.
 - Insert text at section boundaries.
 - CLI: `markadd insert --section "<header>" --position start|end --text "<content>"`.

## Phase 5 — Capture Definitions
 - YAML-based capture definitions in the vault.
 - Filling in fields from user input or variables.
 - Optional scripting hooks (Lua or Rust plugins).

## Phase 6 — Macro System
 - Combine templates, captures, scripts into high-level actions.

## Phase 7 — TUI
 - Browse templates
 - Quick selection and filling
 - Preview before writing

# Goals

`markadd` aims to be:
 - a terminal-native QuickAdd alternative
 - fully scriptable, predictable, and versioned
 - designed around reproducible workflows and automation
 - tightly testable and CI-friendly
 - extensible (TUI, scripting, macros, plugins)


