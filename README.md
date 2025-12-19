# markadd

[![Build Status](https://github.com/agustinvalencia/markadd/actions/workflows/ci.yml/badge.svg)](https://github.com/agustinvalencia/markadd/actions)
[![codecov](https://codecov.io/gh/agustinvalencia/markadd/branch/main/graph/badge.svg)](https://codecov.io/gh/agustinvalencia/markadd)

`markadd` is a terminal-first Markdown automation tool inspired by Obsidian's QuickAdd plugin.

It allows you to:

- Generate Markdown files from templates with variable substitution
- Insert content into Markdown sections via captures
- Execute multi-step workflows with macros
- Use date math expressions like `{{today + 1d}}` or `{{today + monday}}`
- Interactive prompts for missing variables (or batch mode for scripts)
- Launch an interactive TUI for browsing and executing templates, captures, and macros

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
---
output: "daily/{{today}}.md"
vars:
  focus:
    prompt: "What's your focus today?"
    default: "General work"
---
# {{today}}

Focus: {{focus}}

## Tasks

- [ ]

## Notes

EOF
```

3. Generate a file from the template:

```bash
# Interactive mode - prompts for missing variables
markadd new --template daily

# Or provide variables directly
markadd new --template daily --var focus="Ship the feature"

# Or use batch mode (fails if required vars missing)
markadd new --template daily --batch
```

4. Launch the TUI:

```bash
markadd
```

## Commands

### TUI Mode

Run `markadd` without arguments to launch the interactive TUI:

```bash
markadd
```

The TUI displays templates, captures, and macros in a palette. Navigate with `j/k` or arrow keys, press `Enter` to execute, and `q` to quit.

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
markadd new --template <name> [--output <path>] [--var KEY=VALUE]...
```

Options:
- `--template` - Logical template name (e.g., `daily` or `blog/post`)
- `--output` - Output file path (optional if template defines output in frontmatter)
- `--var KEY=VALUE` - Pass variables (can be repeated)
- `--batch` - Fail on missing variables instead of prompting

### capture

Insert content into an existing Markdown file using a capture workflow.

```bash
markadd capture <capture-name> [--var KEY=VALUE]...
```

Options:
- `--list`, `-l` - List available captures
- `--var KEY=VALUE` - Pass variables to the capture (can be repeated)
- `--batch` - Fail on missing variables instead of prompting

### macro

Execute a multi-step macro workflow.

```bash
markadd macro <macro-name> [--var KEY=VALUE]...
```

Options:
- `--list`, `-l` - List available macros
- `--var KEY=VALUE` - Pass variables (can be repeated)
- `--batch` - Fail on missing variables instead of prompting
- `--trust` - Allow shell command execution (required for macros with shell steps)

Example:

```bash
# List available macros
markadd macro --list

# Run a macro
markadd macro weekly-review --var topic="Q1 Planning"

# Run a macro with shell commands
markadd macro deploy-notes --trust
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
| `{{today}}` | Alias for `{{date}}` |
| `{{time}}` | Current time (HH:MM) |
| `{{now}}` | Alias for `{{datetime}}` |
| `{{datetime}}` | ISO 8601 datetime |
| `{{vault_root}}` | Configured vault root path |
| `{{template_name}}` | Logical name of the template |
| `{{output_path}}` | Full output file path |
| `{{output_filename}}` | Output filename only |

### Date Math

Use date math expressions for dynamic dates:

```markdown
Tomorrow: {{today + 1d}}
Yesterday: {{today - 1d}}
Next week: {{today + 1w}}
Next month: {{today + 1M}}
Next year: {{today + 1y}}

# Weekday navigation
Next Monday: {{today + monday}}
Last Friday: {{today - friday}}

# Custom formatting
Day name: {{today | %A}}
Month name: {{today | %B}}
Full date: {{today | %Y-%m-%d}}
```

### Variable Metadata

Define variables with prompts, defaults, and descriptions in template frontmatter:

```yaml
---
output: "meetings/{{title}}.md"
vars:
  title:
    prompt: "Meeting title"
    required: true
  attendees:
    prompt: "Who's attending?"
    default: "TBD"
    description: "Comma-separated list of names"
---
```

Simple form (just the prompt):

```yaml
vars:
  title: "Meeting title"
```

### Example Template

```markdown
---
output: "meetings/{{date}}-{{title}}.md"
vars:
  title:
    prompt: "Meeting title"
  attendees:
    prompt: "Attendees"
    default: "Team"
---
# Meeting: {{title}}

**Date**: {{date}}
**Time**: {{time}}
**Attendees**: {{attendees}}

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

vars:
  item:
    prompt: "What to add?"
  priority:
    prompt: "Priority"
    default: "normal"

target:
  file: "{{vault_root}}/inbox.md"
  section: Inbox
  position: end

content: |
  - [{{priority}}] {{item}}
```

Run a capture:

```bash
markadd capture inbox-item --var item="Review PR #42"
```

For more on captures, see [`docs/capture.md`](./docs/capture.md).

## Macros

Macros are multi-step workflows that combine templates, captures, and shell commands. They're stored as YAML files in your `macros_dir`.

### Example Macro

```yaml
name: weekly-review
description: Set up weekly review documents

vars:
  week_topic:
    prompt: "What's the focus this week?"

steps:
  # Create weekly summary from template
  - template: weekly-summary
    with:
      topic: "{{week_topic}}"

  # Archive completed tasks
  - capture: archive-tasks

  # Optional: commit changes (requires --trust)
  # - shell: "git add . && git commit -m 'Weekly review'"
```

Run a macro:

```bash
markadd macro weekly-review
```

### Shell Commands

Macros can include shell commands, but these require the `--trust` flag for security:

```yaml
steps:
  - shell: "git add {{file}}"
    description: Stage file in git
```

```bash
markadd macro deploy --trust
```

For more on macros, see [`docs/macros.md`](./docs/macros.md).

## Documentation

- [Configuration Reference](./docs/config.md)
- [Template Authoring](./docs/templates.md)
- [Captures Reference](./docs/capture.md)
- [Macros Reference](./docs/macros.md)
- [Development Guide](./docs/development.md)

## License

See [LICENSE](./LICENSE).
