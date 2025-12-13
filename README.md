# markadd

[![Build Status](https://github.com/agustinvalencia/markadd/actions/workflows/ci.yml/badge.svg)](https://github.com/agustinvalencia/markadd/actions)
[![codecov](https://codecov.io/gh/agustinvalencia/markadd/branch/main/graph/badge.svg)](https://codecov.io/gh/agustinvalencia/markadd)

`markadd` is a terminal-first Markdown automation tool inspired by Obsidian's QuickAdd plugin.

It allows you to:

- generate Markdown files from templates
- list templates available in your vault
- validate configuration profiles
- (soon) insert content into Markdown sections
- (later) run capture workflows and programmable macros

## Installation

```bash
cargo install --path crates/cli
```

Or build from source:

```bash
cargo build --release
```

## Quick Start

1. Create a configuration file:

```bash
mkdir -p ~/.config/markadd
cat > ~/.config/markadd/config.toml << 'EOF'
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
EOF
```

2. Create a template:

```bash
mkdir -p ~/Notes/.markadd/templates
cat > ~/Notes/.markadd/templates/daily.md << 'EOF'
# {{date}}

## Tasks

- [ ]

## Notes

EOF
```

3. Generate a file from the template:

```bash
markadd new --template daily --output ~/Notes/2025-01-15.md
```

## Commands

### doctor

Validate configuration and print resolved paths.

```bash
markadd doctor
```

### list-templates

List available templates in the active profile.

```bash
markadd list-templates
```

### new

Generate a new file from a template.

```bash
markadd new --template <name> --output <path>
```

Options:
- `--template` — Logical template name (e.g., `daily` or `blog/post`)
- `--output` — Output file path to create

## Configuration

`markadd` loads configuration from:

1. `--config <path>` (if provided)
2. `$XDG_CONFIG_HOME/markadd/config.toml`
3. `~/.config/markadd/config.toml`

### Example Configuration

```toml
version = 1
profile = "default"

[profiles.default]
vault_root = "~/Notes"
templates_dir = "{{vault_root}}/.markadd/templates"
captures_dir  = "{{vault_root}}/.markadd/captures"
macros_dir    = "{{vault_root}}/.markadd/macros"

[profiles.work]
vault_root = "~/Work/notes"
templates_dir = "{{vault_root}}/templates"
captures_dir  = "{{vault_root}}/captures"
macros_dir    = "{{vault_root}}/macros"

[security]
allow_shell = false
allow_http  = false
```

Use `--profile` to switch profiles:

```bash
markadd --profile work list-templates
```

For full configuration reference, see [`docs/CONFIG.md`](./docs/CONFIG.md).

## Templates

Templates are Markdown files stored in your `templates_dir`. They support variable substitution using `{{variable}}` syntax.

### Built-in Variables

| Variable | Description |
|----------|-------------|
| `{{date}}` | Current date (YYYY-MM-DD) |
| `{{time}}` | Current time (HH:MM) |
| `{{datetime}}` | ISO 8601 datetime |
| `{{vault_root}}` | Configured vault root path |
| `{{template_name}}` | Logical name of the template |
| `{{output_path}}` | Full output file path |
| `{{output_filename}}` | Output filename only |

### Example Template

```markdown
# Meeting: {{date}}

**Time**: {{time}}
**File**: {{output_filename}}

## Attendees

-

## Agenda

1.

## Notes

## Action Items

- [ ]
```

For more on templates, see [`docs/templates.md`](./docs/templates.md).

## Documentation

- [Configuration Reference](./docs/CONFIG.md)
- [Template Authoring](./docs/templates.md)
- [Development Guide](./docs/DEVELOPMENT.md)

## License

See [LICENSE](./LICENSE).
