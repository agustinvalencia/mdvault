# markadd

[![Build Status](https://github.com/agustinvalencia/markadd/actions/workflows/ci.yml/badge.svg)](https://github.com/agustinvalencia/markadd/actions)
[![codecov](https://codecov.io/gh/agustinvalencia/markadd/branch/main/graph/badge.svg)](https://codecov.io/gh/agustinvalencia/markadd)

`markadd` is a terminal-first Markdown automation tool inspired by Obsidian's QuickAdd plugin.

It allows you to:

- generate Markdown files from templates
- insert content into Markdown sections via captures
- list templates and captures available in your vault
- validate configuration profiles
- (coming soon) programmable macros

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

### capture

Insert content into an existing Markdown file using a capture workflow.

```bash
markadd capture --name <capture-name>
```

Options:
- `--name` — Name of the capture to run
- `--dry-run` — Preview changes without writing to file

### list-captures

List available captures in the active profile.

```bash
markadd list-captures
```

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

For full configuration reference, see [`docs/config.md`](./docs/config.md).

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

## Captures

Captures are YAML files that define workflows for inserting content into existing Markdown files. They're stored in your `captures_dir`.

### Example Capture

```yaml
name: inbox-item
description: Add an item to inbox

target:
  file: "{{vault_root}}/inbox.md"
  section: Inbox
  position: end

content: |
  - [ ] {{item}}
```

Run a capture:

```bash
markadd capture --name inbox-item
```

This will prompt for the `{{item}}` variable and insert the content at the end of the "Inbox" section.

For more on captures, see [`docs/capture.md`](./docs/capture.md).

## Documentation

- [Configuration Reference](./docs/config.md)
- [Template Authoring](./docs/templates.md)
- [Captures Reference](./docs/capture.md)
- [Development Guide](./docs/development.md)

## License

See [LICENSE](./LICENSE).
