# Project and Task Management

mdvault provides a structured system for managing projects and tasks as markdown notes with automatic ID generation, status tracking, and daily note integration.

## Core Concepts

### ID System

Every project and task gets a unique identifier embedded in its frontmatter:

- **Project IDs**: 3-letter codes derived from the project title
  - "My Cool Project" → `MCP`
  - "Home Automation" → `HAU` (first letters + second letter of longer word)
  - "Inventory" → `INV` (first 3 letters of single word)

- **Task IDs**: Project ID + 3-digit counter
  - First task in MCP → `MCP-001`
  - Second task → `MCP-002`
  - Inbox tasks (no project) → `INB-001`, `INB-002`, etc.

IDs are the **source of truth** stored in note frontmatter. The index mirrors them for fast queries.

### File Structure

```
vault/
├── Projects/
│   ├── MCP/
│   │   ├── MCP.md              # Project note
│   │   └── Tasks/
│   │       ├── MCP-001.md      # Task notes
│   │       ├── MCP-002.md
│   │       └── MCP-003.md
│   ├── HAU/
│   │   ├── HAU.md
│   │   └── Tasks/
│   │       └── HAU-001.md
│   └── _archive/               # Archived projects
│       └── OLD/
│           ├── OLD.md
│           └── Tasks/
│               └── OLD-001.md
├── Inbox/
│   ├── INB-001.md              # Tasks without a project
│   └── INB-002.md
└── Journal/
    └── 2025/
        └── Daily/
            └── 2025-01-15.md   # Daily notes with activity log
```

## Creating Projects

```bash
# Interactive mode (prompts for missing fields)
mdv new project "My Cool Project"

# With explicit status
mdv new project "My Cool Project" --var status=active

# Batch mode (no prompts, fails if required fields missing)
mdv new project "My Cool Project" --batch
```

### Project Frontmatter

```yaml
---
type: project
title: My Cool Project
project-id: MCP
task_counter: 3
status: active
created: 2025-01-15
---
```

| Field | Description |
|-------|-------------|
| `type` | Always `project` |
| `title` | Project name |
| `project-id` | Auto-generated 3-letter ID |
| `task_counter` | Current task count (auto-incremented) |
| `status` | `open`, `in-progress`, `done`, `archived` |
| `archived_at` | Timestamp set when project is archived |
| `created` | Creation date |

## Creating Tasks

```bash
# Interactive mode (shows project picker)
mdv new task "Implement search feature"

# With explicit project
mdv new task "Implement search feature" --var project=MCP

# To inbox (no project)
mdv new task "Quick todo item" --var project=inbox

# Batch mode
mdv new task "Fix bug" --var project=MCP --batch
```

### Task Frontmatter

```yaml
---
type: task
title: Implement search feature
task-id: MCP-001
project: MCP
status: todo
created: 2025-01-15
---
```

| Field | Description |
|-------|-------------|
| `type` | Always `task` |
| `title` | Task description |
| `task-id` | Auto-generated ID (project-id + counter) |
| `project` | Parent project ID or `inbox` |
| `status` | `todo`, `doing`, `blocked`, `done`, `cancelled` |
| `created` | Creation date |
| `completed_at` | Completion timestamp (set by `task done`) |
| `cancelled_at` | Cancellation timestamp (set by `task cancel` or project archive) |

## Listing Projects

```bash
# List all projects with task counts
mdv project list

# Filter by status
mdv project list --status active
```

Output:
```
╭─────┬───────────────────┬────────┬──────┬──────┬───────╮
│ ID  │ Title             │ Status │ Open │ Done │ Total │
├─────┼───────────────────┼────────┼──────┼──────┼───────┤
│ MCP │ My Cool Project   │ active │ 2    │ 1    │ 3     │
│ HAU │ Home Automation   │ active │ 5    │ 0    │ 5     │
╰─────┴───────────────────┴────────┴──────┴──────┴───────╯

Total: 2 projects
```

## Project Status

View detailed project status with task breakdown:

```bash
mdv project status MCP
```

Output:
```
Project: My Cool Project [MCP]
Status:  active

Task Summary:
  TODO:        1
  In Progress: 1
  Blocked:     0
  Done:        1
  Total:       3

TODO:
╭─────────┬─────────────────────────┬────────╮
│ ID      │ Title                   │ Status │
├─────────┼─────────────────────────┼────────┤
│ MCP-003 │ Write documentation     │ todo   │
╰─────────┴─────────────────────────┴────────╯

IN PROGRESS:
╭─────────┬─────────────────────────┬─────────────╮
│ ID      │ Title                   │ Status      │
├─────────┼─────────────────────────┼─────────────┤
│ MCP-002 │ Add unit tests          │ in-progress │
╰─────────┴─────────────────────────┴─────────────╯

DONE:
╭─────────┬─────────────────────────┬────────╮
│ ID      │ Title                   │ Status │
├─────────┼─────────────────────────┼────────┤
│ MCP-001 │ Implement search        │ done   │
╰─────────┴─────────────────────────┴────────╯
```

## Listing Tasks

```bash
# List all tasks
mdv task list

# Filter by project
mdv task list --project MCP

# Filter by status
mdv task list --status todo
mdv task list --status in-progress
```

Output:
```
╭─────────┬─────────────────────────┬─────────────┬─────────╮
│ ID      │ Title                   │ Status      │ Project │
├─────────┼─────────────────────────┼─────────────┼─────────┤
│ MCP-001 │ Implement search        │ done        │ MCP     │
│ MCP-002 │ Add unit tests          │ in-progress │ MCP     │
│ MCP-003 │ Write documentation     │ todo        │ MCP     │
│ INB-001 │ Quick todo item         │ todo        │ inbox   │
╰─────────┴─────────────────────────┴─────────────┴─────────╯

Total: 4 tasks
```

## Task Status

View detailed information about a specific task:

```bash
mdv task status MCP-001
```

Output:
```
Task: Implement search [MCP-001]

  Status:       done
  Project:      MCP
  Created:      2025-01-10
  Completed:    2025-01-15T14:32:00
  Path:         Projects/MCP/Tasks/MCP-001.md
```

## Completing Tasks

Mark a task as done:

```bash
# Basic completion
mdv task done Projects/MCP/Tasks/MCP-001.md

# With summary (logged to the task body)
mdv task done Projects/MCP/Tasks/MCP-001.md --summary "Implemented full-text search with FTS5"
```

This updates the task frontmatter:
```yaml
status: done
completed_at: 2025-01-15T14:32:00
```

And optionally appends to the task body:
```markdown
- **[[2025-01-15]] 14:32** : Completed - Implemented full-text search with FTS5
```

## Daily Note Integration

When you create a project or task, an entry is automatically logged to today's daily note:

```markdown
---
type: daily
date: 2025-01-15
---

# 2025-01-15

## Log
- **09:15** Created project [MCP]: [[MCP|My Cool Project]]
- **09:20** Created task [MCP-001]: [[MCP-001|Implement search feature]]
- **14:30** Created task [MCP-002]: [[MCP-002|Add unit tests]]
```

The daily note is created automatically if it doesn't exist.

## Workflows

### Starting a New Project

```bash
# 1. Create the project
mdv new project "Website Redesign"

# 2. Add initial tasks
mdv new task "Create wireframes" --var project=WER
mdv new task "Design color palette" --var project=WER
mdv new task "Build homepage prototype" --var project=WER

# 3. Check project status
mdv project status WER
```

### Daily Task Review

```bash
# See all open tasks
mdv task list --status todo

# See what's in progress
mdv task list --status in-progress

# Check a specific project
mdv project status MCP
```

### Completing Work

```bash
# Mark task done with context
mdv task done Projects/MCP/Tasks/MCP-001.md --summary "Shipped v1.0"

# Reindex to update counts
mdv reindex

# Verify project progress
mdv project status MCP
```

### Finding Stale Tasks

```bash
# Tasks not referenced recently
mdv stale --type task

# Tasks in a specific project
mdv stale --type task --days 30
```

## Index and Queries

The index stores frontmatter as JSON for fast queries. After creating or modifying notes outside of mdv commands, rebuild the index:

```bash
# Incremental update (only changed files)
mdv reindex

# Full rebuild
mdv reindex --force
```

### JSON Output for Scripting

```bash
# Export task list as JSON
mdv list --type task --json

# Query specific project tasks
mdv list --type task --json | jq '.[] | select(.path | contains("MCP"))'
```

## Type Definitions

You can customize project and task schemas with Lua type definitions:

**`~/.config/mdvault/types/task.lua`**:
```lua
return {
    name = "task",
    description = "Actionable task",

    schema = {
        status = {
            type = "string",
            enum = { "todo", "in-progress", "blocked", "done" },
            default = "todo",
            required = true,
        },
        priority = {
            type = "string",
            enum = { "low", "medium", "high" },
            default = "medium",
        },
        due = {
            type = "date",
        },
    },

    validate = function(note)
        if note.frontmatter.status == "done" and not note.frontmatter.completed_at then
            return false, "Completed tasks must have completed_at"
        end
        return true
    end,
}
```

## Archiving Projects

When a project is complete, archive it to move files out of the active `Projects/` folder:

```bash
# Archive a completed project
mdv project archive my-cool-project

# Skip confirmation prompt
mdv project archive my-cool-project --yes
```

### What Archiving Does

1. **Validates** the project has `status: done` — rejects otherwise
2. **Cancels** any remaining open tasks (todo, doing, blocked)
3. **Updates** project frontmatter: `status: archived`, `archived_at: <timestamp>`
4. **Clears focus** if the archived project was the active focus
5. **Moves** all files from `Projects/{slug}/` to `Projects/_archive/{slug}/`
6. **Updates** all wikilinks and index entries to the new paths
7. **Logs** the archival to both the daily note and the project note

### Archived Project Behaviour

- Archived projects are excluded from `mdv project list` by default
- Task and project lookups still find archived items (for backlink resolution)
- **Cannot create new tasks** in an archived project — returns an error
- Archived files are safe in `_archive/` and can be browsed or searched

### Workflow

```bash
# 1. Mark project as done
mdv task done Projects/MCP/Tasks/MCP-003.md
# ... complete remaining tasks ...

# 2. Verify project is ready
mdv project status MCP

# 3. Archive
mdv project archive MCP --yes
```

## Tips

### Quick Inbox Capture

For quick captures without choosing a project:

```bash
mdv new task "Remember to do this" --var project=inbox --batch
```

### Bulk Status Check

```bash
# All active projects with open tasks
mdv project list --status active

# All blocked tasks across projects
mdv task list --status blocked
```

### Task IDs in Commit Messages

Reference task IDs in git commits for traceability:

```bash
git commit -m "feat: add search [MCP-001]"
```

### Linking Tasks in Notes

Reference tasks using wikilinks:

```markdown
Working on [[MCP-001|search feature]] today.
See also: [[MCP-002]] for test coverage.
```
