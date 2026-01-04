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
name: inbox
description: Quick capture to inbox

target:
  file: "inbox.md"
  section: "Inbox"
  position: end

content: "- [ ] {{text}} ({{time}})"

vars:
  text:
    required: true
    prompt: "What to capture?"
```

Use `create_if_missing: true` in the target to auto-create the file if it doesn't exist (useful for daily notes).

## Common Workflows

### Creating Notes

```bash
# Create a task (interactive - prompts to select project)
mdv new task "Implement search feature"

# Create a task with explicit project
mdv new task "Implement search feature" --var project=myproject

# Create from a template
mdv new --template daily

# Create a project
mdv new project "New Project" --var status=active
```

### Task and Project Management

```bash
# List all tasks grouped by project
mdv task list

# Filter tasks by project
mdv task list --project myproject

# Filter tasks by status
mdv task list --status todo
mdv task list --status in-progress

# Mark a task as done
mdv task done Projects/myproject/Tasks/my-task.md

# Mark done with a summary (logged to the task)
mdv task done Projects/myproject/Tasks/my-task.md --summary "Completed implementation"

# List all projects with task counts
mdv project list

# Filter projects by status
mdv project list --status active

# Show tasks for a project in kanban-style view
mdv project myproject tasks
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

### Renaming Notes

The `rename` command safely renames a note and updates all references to it across your vault:

```bash
# Rename a note and update all references
mdv rename old-note.md new-note.md

# Preview changes without modifying files
mdv rename old-note.md new-note.md --dry-run

# Skip confirmation prompt
mdv rename old-note.md new-note.md --yes
```

Reference types updated automatically:
- Wikilinks: `[[old-note]]`, `[[old-note|alias]]`, `[[old-note#section]]`
- Markdown links: `[text](old-note.md)`, `[text](../path/old-note.md)`
- Frontmatter references: `project: old-note`, `related: [old-note, other]`

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

# ISO date literals (absolute dates)
mdv list --modified-after "2025-01-15"
mdv list --modified-before "2025-01-15 + 7d"

# In templates
{{today}}              # 2025-12-31
{{today + 7d}}         # One week from now
{{monday}}             # This week's Monday
{{2025-01-15}}         # Specific date
{{2025-01-15 + 7d}}    # Specific date + offset
{{2025-01-15 | %A}}    # Day name for specific date
```

### Supported Bases

| Base | Description | Example |
|------|-------------|---------|
| `today` | Current date | `today + 1d` |
| `now` | Current datetime | `now + 2h` |
| `time` | Current time | `time - 30m` |
| `week` | Current ISO week number | `week + 1w` |
| `year` | Current year | `year - 1y` |
| `week_start` | Monday of current week | `week_start + 1w` |
| `week_end` | Sunday of current week | `week_end` |
| `YYYY-MM-DD` | ISO date literal | `2025-01-15 + 7d` |
| `YYYY-Www` | ISO week (Monday) | `2025-W03 + 6d` |

### Supported Offsets

| Unit | Description | Example |
|------|-------------|---------|
| `d` | Days | `today + 7d` |
| `w` | Weeks | `today - 2w` |
| `M` | Months | `today + 1M` |
| `y` | Years | `today - 1y` |
| `h` | Hours | `now + 2h` |
| `m` | Minutes | `now - 30m` |
| weekday | Relative weekday | `today + friday` |

### Weekly Note Example

Generate links to all dailies in a weekly template:

```markdown
---
type: weekly
week: {{week | %Y-W%V}}
---

# Week {{week}}

## Daily Notes
- [[{{week_start}}]] Monday
- [[{{week_start + 1d}}]] Tuesday
- [[{{week_start + 2d}}]] Wednesday
- [[{{week_start + 3d}}]] Thursday
- [[{{week_start + 4d}}]] Friday
- [[{{week_start + 5d}}]] Saturday
- [[{{week_start + 6d}}]] Sunday
```

Or for a specific week using ISO notation:

```markdown
## Week 3, 2025
- [[{{2025-W03}}]] Monday
- [[{{2025-W03 + 1d}}]] Tuesday
...
- [[{{2025-W03 + 6d}}]] Sunday
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
