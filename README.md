# mdvault

**Your Markdown Vault on the Command Line**

[![Build Status](https://github.com/agustinvalencia/markadd/actions/workflows/ci.yml/badge.svg)](https://github.com/agustinvalencia/markadd/actions)
[![codecov](https://codecov.io/gh/agustinvalencia/markadd/branch/main/graph/badge.svg)](https://codecov.io/gh/agustinvalencia/markadd)

> **Note**: This project is being renamed from `markadd` to `mdvault`. The command will change from `markadd` to `mdv`. See [Migration Guide](#migration-from-markadd) below.

mdvault is a complete terminal interface for markdown-based knowledge vaults. It combines the quick-input automation of Obsidian's QuickAdd with comprehensive vault management features.

## What mdvault does

**Available now:**
- Create notes from templates with variable substitution
- Quick capture to daily notes and projects
- Multi-step workflow automation (macros)
- Date math expressions like `{{today + 1d}}` or `{{today + monday}}`
- Interactive prompts for missing variables (or batch mode for scripts)
- TUI for browsing and executing templates, captures, and macros

**Coming soon:**
- Full-text search across your vault
- Query notes by frontmatter metadata
- Backlinks, orphans, and graph analysis
- Browse and read vault contents

**Compatible with:**
- Obsidian, Logseq, Dendron, Foam
- Any markdown-based vault system
- Works standalone OR with MCP integration

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
mkdir -p ~/.config/mdvault
cat > ~/.config/mdvault/config.toml << 'EOF'
version = 1
profile = "default"

[profiles.default]
vault_root = "~/Notes"
templates_dir = "{{vault_root}}/.mdvault/templates"
captures_dir  = "{{vault_root}}/.mdvault/captures"
macros_dir    = "{{vault_root}}/.mdvault/macros"

[security]
allow_shell = false
allow_http  = false
EOF
```

2. Create a template:

```bash
mkdir -p ~/Notes/.mdvault/templates
cat > ~/Notes/.mdvault/templates/daily.md << 'EOF'
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
mdv new --template daily

# Or provide variables directly
mdv new --template daily --var focus="Ship the feature"

# Or use batch mode (fails if required vars missing)
mdv new --template daily --batch
```

4. Launch the TUI:

```bash
mdv
```

## Commands

### TUI Mode

Run `mdv` without arguments to launch the interactive TUI:

```bash
mdv
```

The TUI displays templates, captures, and macros in a palette. Navigate with `j/k` or arrow keys, press `Enter` to execute, and `q` to quit.

### doctor

Validate configuration and print resolved paths.

```bash
mdv doctor
```

### list-templates

List available templates in the active profile.

```bash
mdv list-templates
```

### new

Generate a new file from a template.

```bash
mdv new --template <name> [--output <path>] [--var KEY=VALUE]...
```

Options:
- `--template` - Logical template name (e.g., `daily` or `blog/post`)
- `--output` - Output file path (optional if template defines output in frontmatter)
- `--var KEY=VALUE` - Pass variables (can be repeated)
- `--batch` - Fail on missing variables instead of prompting

### capture

Insert content into an existing Markdown file using a capture workflow.

```bash
mdv capture <capture-name> [--var KEY=VALUE]...
```

Options:
- `--list`, `-l` - List available captures
- `--var KEY=VALUE` - Pass variables to the capture (can be repeated)
- `--batch` - Fail on missing variables instead of prompting

### macro

Execute a multi-step macro workflow.

```bash
mdv macro <macro-name> [--var KEY=VALUE]...
```

Options:
- `--list`, `-l` - List available macros
- `--var KEY=VALUE` - Pass variables (can be repeated)
- `--batch` - Fail on missing variables instead of prompting
- `--trust` - Allow shell command execution (required for macros with shell steps)

Example:

```bash
# List available macros
mdv macro --list

# Run a macro
mdv macro weekly-review --var topic="Q1 Planning"

# Run a macro with shell commands
mdv macro deploy-notes --trust
```

### Planned Commands

```bash
# Search vault
mdv search "network optimization"
mdv search "TODO" --folder projects --context-lines 3

# Query by metadata
mdv query --where "status=todo"
mdv query --where "due<2025-01-01" --where "priority=high"

# Analyze links
mdv links note.md --backlinks
mdv links --orphans --folder research
```

## Configuration

`mdv` loads configuration from:

1. `--config <path>` (if provided)
2. `$XDG_CONFIG_HOME/mdvault/config.toml`
3. `~/.config/mdvault/config.toml`

### Example Configuration

```toml
version = 1
profile = "default"

[profiles.default]
vault_root = "~/Notes"
templates_dir = "{{vault_root}}/.mdvault/templates"
captures_dir  = "{{vault_root}}/.mdvault/captures"
macros_dir    = "{{vault_root}}/.mdvault/macros"

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
mdv --profile work list-templates
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
mdv capture inbox-item --var item="Review PR #42"
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
mdv macro weekly-review
```

### Shell Commands

Macros can include shell commands, but these require the `--trust` flag for security:

```yaml
steps:
  - shell: "git add {{file}}"
    description: Stage file in git
```

```bash
mdv macro deploy --trust
```

For more on macros, see [`docs/macros.md`](./docs/macros.md).

## Migration from markadd

If you've been using `markadd`, here's how to migrate:

### 1. Update command

```bash
# Old
markadd new --template daily

# New
mdv new --template daily
```

### 2. Update config location

```bash
# Old location
~/.config/markadd/config.toml
~/.markadd/templates/

# New location
~/.config/mdvault/config.toml
~/.mdvault/templates/
```

### 3. Update config file paths

```toml
[profiles.default]
vault_root = "~/vault"
templates_dir = "{{vault_root}}/.mdvault/templates"  # was .markadd
captures_dir  = "{{vault_root}}/.mdvault/captures"   # was .markadd
macros_dir    = "{{vault_root}}/.mdvault/macros"     # was .markadd
```

### 4. Reinstall

```bash
cargo install --path crates/cli

# Command is now 'mdv'
mdv --version
```

## Documentation

- [Configuration Reference](./docs/config.md)
- [Template Authoring](./docs/templates.md)
- [Captures Reference](./docs/capture.md)
- [Macros Reference](./docs/macros.md)
- [Development Guide](./docs/development.md)
- [Scope Evolution](./docs/03_focus_change.md)

## License

See [LICENSE](./LICENSE).
