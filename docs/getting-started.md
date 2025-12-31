# Getting Started with mdvault

mdvault is a CLI tool for managing markdown vaults with structured note types, validation, and intelligent search.

## Installation

```bash
# Clone and build
git clone https://github.com/agustinvalencia/mdvault.git
cd mdvault
cargo build --release

# Add to PATH
cp target/release/mdv ~/.local/bin/
```

## Quick Start

### 1. Create a Configuration File

Create `~/.config/mdvault/config.toml`:

```toml
vault_root = "~/Documents/vault"  # Your vault location

[paths]
templates_dir = "~/.config/mdvault/templates"
captures_dir = "~/.config/mdvault/captures"
macros_dir = "~/.config/mdvault/macros"
types_dir = "~/.config/mdvault/types"
```

### 2. Verify Setup

```bash
mdv doctor
```

This shows your configuration and validates paths.

### 3. Build the Index

```bash
mdv reindex
```

This scans your vault and builds the SQLite index for fast queries.

## Core Concepts

### Note Types

mdvault understands different note types based on frontmatter:

```yaml
---
type: task
title: My Task
status: open
project: my-project
---
```

Built-in types: `daily`, `weekly`, `task`, `project`, `zettel`, `none`

### Templates

Templates live in `~/.config/mdvault/templates/`. Example `task.md`:

```markdown
---
type: task
title: {{title}}
status: open
project: {{project}}
created: {{today}}
---

# {{title}}

```

### Captures

Captures append content to existing files. Example `~/.config/mdvault/captures/inbox.yaml`:

```yaml
target: inbox.md
section: "## Inbox"
template: "- [ ] {{text}} ({{now}})"
variables:
  text:
    required: true
    prompt: "What to capture?"
```

## Common Workflows

### Creating Notes

```bash
# Create a task with type-based scaffolding
mdv new task "Implement search feature" --var project=myproject

# Create from a template
mdv new --template daily

# Create a project
mdv new project "New Project" --var status=active
```

### Querying Notes

```bash
# List all notes
mdv list

# List only tasks
mdv list --type task

# Recent notes (last 7 days)
mdv list --modified-after "today - 7d"

# JSON output for scripting
mdv list --type task --json
```

### Finding Links

```bash
# Show all links for a note
mdv links notes/my-note.md

# Only backlinks (notes linking TO this note)
mdv links notes/my-note.md --backlinks

# Only outlinks (notes this note links TO)
mdv links notes/my-note.md --outlinks

# Find orphan notes (no incoming links)
mdv orphans
```

### Searching

```bash
# Direct text search
mdv search "machine learning"

# Full contextual search (includes linked notes, temporal context)
mdv search "machine learning" --mode full

# Search with temporal boost (favour recent notes)
mdv search "project" --boost

# Search only in tasks
mdv search "bug" --type task
```

### Finding Stale Notes

```bash
# Notes with staleness score > 0.5
mdv stale

# More stale notes (higher threshold)
mdv stale --threshold 0.7

# Notes not referenced in last 90 days
mdv stale --days 90

# Only stale tasks
mdv stale --type task
```

### Validation

```bash
# Validate all notes against type definitions
mdv validate

# Validate only tasks
mdv validate --type task

# Auto-fix safe issues (missing defaults, enum case)
mdv validate --fix

# Check link integrity too
mdv validate --check-links

# Show available type definitions
mdv validate --list-types
```

### Capturing Content

```bash
# List available captures
mdv capture --list

# Quick capture to inbox
mdv capture inbox --var text="Buy groceries"

# Non-interactive mode (fails if variables missing)
mdv capture inbox --var text="Note" --batch
```

### Running Macros

```bash
# List available macros
mdv macro --list

# Run a macro
mdv macro weekly-review
```

## Custom Type Definitions

Create custom types in `~/.config/mdvault/types/`. Example `meeting.lua`:

```lua
return {
    name = "meeting",
    description = "Meeting notes",

    schema = {
        attendees = { type = "list", required = true },
        date = { type = "date", required = true },
        status = {
            type = "string",
            enum = { "scheduled", "completed", "cancelled" },
            default = "scheduled"
        },
    },

    validate = function(note)
        if note.frontmatter.status == "completed" then
            if not note.frontmatter.summary then
                return false, "Completed meetings must have a summary"
            end
        end
        return true
    end,

    on_create = function(note, ctx)
        -- Called when a meeting note is created
        print("Created meeting: " .. note.frontmatter.title)
    end,
}
```

## Date Math Expressions

mdvault supports date math in commands and templates:

```bash
# Relative dates
mdv list --modified-after "today - 7d"
mdv list --modified-before "yesterday"

# Named days
mdv list --modified-after "monday"
mdv list --modified-after "last friday"

# In templates
{{today}}           # 2025-12-31
{{today + 7d}}      # One week from now
{{monday}}          # This week's Monday
{{next week}}       # Next week's Monday
```

## Output Formats

Most query commands support multiple output formats:

```bash
# Table format (default)
mdv list

# JSON output
mdv list --json

# Quiet mode (paths only, good for scripting)
mdv list --quiet

# Explicit format
mdv list --output json
```

## Tips

### Incremental vs Full Reindex

```bash
# Incremental (only changed files) - default
mdv reindex

# Full rebuild (rebuilds everything)
mdv reindex --force
```

### Verbose Mode

```bash
# See each file as it's indexed
mdv reindex --verbose
```

### Configuration Profiles

```bash
# Use a different config file
mdv --config ~/other-vault/config.toml list

# Use a profile (if defined in config)
mdv --profile work list
```

### TUI Mode

Running `mdv` without any subcommand launches the terminal UI:

```bash
mdv
```

## Next Steps

- Read the [Architecture Guide](architecture.md) for design details
- Check [PLAN.md](PLAN.md) for the development roadmap
- Explore [Lua Scripting](lua-scripting.md) for advanced customization
