# markadd

[![Build Status](https://github.com/agustinvalencia/markadd/actions/workflows/ci.yml/badge.svg)](https://github.com/agustinvalencia/markadd/actions)

**markadd** is a terminal-first Markdown automation tool inspired by Obsidian’s QuickAdd plugin.  
It provides a composable, scriptable way to create, capture, and organise Markdown notes directly from the command line.

`markadd` lets you render templates, insert captures into existing notes, and compose multi-step macros for automated workflows.  
It is designed to be deterministic, safe by default, and easily extensible through YAML and Lua hooks.

## Key Features

- **Template generation**: create Markdown files from reusable templates with variable prompts  
- **Capture actions**: insert text or rendered fragments into existing sections  
- **Macro workflows**: combine multiple actions and optional shell commands in declarative YAML  
- **Atomic file operations**: always write safely, with undo support  
- **Trust model**: shell and network access disabled by default, gated by explicit `--trust` flag  
- **Optional Lua scripting**: define programmable captures or macros with a sandboxed API  
- **TUI mode**: (planned) an interactive palette for selecting templates and filling variables  

## Getting Started

### Installation

For now, clone and build locally (Cargo):

```shell
git clone https://github.com/<yourname>/markadd.git
cd markadd
cargo install --path crates/cli
```

Later releases will be available via Homebrew and `cargo install markadd`.

### First Run

```shell
markadd doctor
```

This command checks for your configuration file and prints the active profile, directories, and security settings.

### Configuration

Create the main config file at `~/.config/markadd/config.toml`:

```toml
version = 1
profile = "default"

[profiles.default]
vault_root = "~/Documents/Obsidian/Vault"
templates_dir = "{{vault_root}}/.markadd/templates"
captures_dir  = "{{vault_root}}/.markadd/captures"
macros_dir    = "{{vault_root}}/.markadd/macros"

[security]
allow_shell = false
allow_http  = false
```

This file defines where `markadd` looks for templates, captures, and macros.  
Security settings control which features require explicit trust.

### Directory Layout

```
.markadd/
  templates/   → Markdown templates with YAML front-matter
  captures/    → YAML capture recipes
  macros/      → YAML macro definitions
```

### Example Template

```markdown
---
name: meeting-note
description: Meeting notes template
vars:
  - id: title
    prompt: "Title"
    type: string
  - id: date
    prompt: "Date"
    type: date
    default: "{{ now | date('%Y-%m-%d') }}"
target:
  path: "notes/{{ date }}/{{ title | slugify }}.md"
  if_exists: append
---

# {{ title }}

**Date:** {{ date }}

## Agenda

- 

## Notes

- 
```

### Example Capture

```yaml
name: inbox
target:
  path: "Daily/{{ now | date('%Y-%m-%d') }}.md"
  section: "Inbox"
  position: begin
content: "- [ ] {{ text }}"
vars:
  - id: text
    prompt: "What to capture?"
    type: string
```

### Example Macro

```yaml
name: weekly-review
steps:
  - template:
      use: "weekly-note.md"
      with:
        date: "{{ now | date('%Y-%m-%d') }}"
  - capture:
      use: "inbox"
      with:
        text: "Plan next week"
  - shell:
      run: "git add . && git commit -m 'notes: weekly review'"
      on_error: continue
```

## Command Overview

| Command | Description |
|----------|--------------|
| `markadd template <name>` | Create a new file from a template |
| `markadd capture <name>` | Insert content into an existing file |
| `markadd macro <name>` | Run a multi-step workflow |
| `markadd list` | List available templates, captures, or macros |
| `markadd preview` | Render a template or capture without writing |
| `markadd doctor` | Validate configuration and environment |
| `markadd undo <id>` | Revert a recent file operation |
| `markadd eval-lua` | (optional) Evaluate Lua script for debugging |

All commands accept global flags:
- `--config` to specify a custom config file  
- `--profile` to override the active profile  
- `--var key=value` to provide variable values  
- `--dry-run` to preview actions without writing  
- `--trust` to allow shell/network operations

## Variable Resolution

Variables are resolved in a fixed, deterministic order:

1. Automatic providers (now, uuid, cwd, git, env)  
2. Defaults defined in the template or capture YAML  
3. Values passed through `with:` in a macro step  
4. CLI arguments (`--var key=value`)  
5. Interactive prompts (CLI or TUI mode)

## Security Model

`markadd` is secure by default.  
Shell and HTTP features are disabled unless explicitly allowed in your `config.toml`.  
Even then, they require the `--trust` flag at runtime.

All operations are logged to:

```
~/.config/markadd/.ops.jsonl
```

This log allows inspection and undoing of past file edits.

## Development Roadmap

`markadd` is built in clearly defined phases:

1. **Config Loader** – deterministic config via TOML  
2. **Content Parsers** – strict YAML/MD parsing  
3. **Variable Resolution & Rendering** – using Tera  
4. **Markdown AST Insertions** – via Comrak  
5. **File Planner** – atomic writes + undo  
6. **CLI Commands** – template, capture, macro, list  
7. **Macro Runner & Security** – gated shell steps  
8. **Lua Hooks** – optional scripting extension  
9. **TUI Interface** – fuzzy palette, preview, prompts  
10. **Documentation & Packaging** – user guides, binaries  

## Contributing

1. Fork the repository  
2. Create a feature branch  
3. Run `cargo fmt && cargo clippy` before committing  
4. Add tests for new logic  
5. Submit a pull request

Use `cargo test --all` to run the full suite.  
Each phase must pass CI and maintain stable APIs.

## License

MIT License. See the `LICENSE` file for details.

## Acknowledgements

Inspired by Obsidian’s QuickAdd plugin and the Unix philosophy of composable tools.  
Built with Rust, Tera, Comrak, and an unreasonable fondness for deterministic workflows.
