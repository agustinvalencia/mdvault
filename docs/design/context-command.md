# Design: `mdv context` Command

**Tracking Issue**: [#94](https://github.com/agustinvalencia/mdvault/issues/94)

## Overview

The `mdv context` command provides contextual information about notes, days, or weeks to support planning and reporting workflows. It is designed primarily for MCP (Model Context Protocol) consumption by AI assistants, with human-readable output as a secondary use case.

### Design Principles

1. **Progressive Discovery** - Return summaries and counts first; let consumers drill down for details
2. **Dual Source Aggregation** - Combine operation logs (structured) with note analysis (comprehensive)
3. **Flexible Time Resolution** - Support day, week, and arbitrary date ranges
4. **Note-Type Awareness** - Tailor context based on note type (project, task, daily, generic)

## Command Structure

```
mdv context <SUBCOMMAND> [OPTIONS]

SUBCOMMANDS:
    day     Context for a specific day
    week    Context for a specific week
    note    Context for a specific note
    focus   Context for the currently focused project (if set)

OPTIONS:
    --format <FORMAT>    Output format: md (default), json, summary
    --depth <DEPTH>      Detail level: minimal, normal (default), full
```

### Subcommand Details

#### `mdv context day [DATE]`

```
mdv context day                     # today
mdv context day yesterday           # yesterday
mdv context day 2026-01-20          # specific date
mdv context day "today - 3d"        # date expression
mdv context day --lookback          # today, or last day with activity
```

#### `mdv context week [WEEK]`

```
mdv context week                    # current week
mdv context week last               # last week
mdv context week 2026-W04           # specific ISO week
mdv context week "today - 2w"       # week containing date
```

#### `mdv context note <PATH>`

```
mdv context note Projects/mdvault/mdvault.md
mdv context note tasks/TST-044.md
mdv context note docs/design/architecture.md
```

#### `mdv context focus`

```
mdv context focus                   # context for active focus project
mdv context focus --with-tasks      # include all task details
```

## Activity Logging

### Log Format

Activity logs are stored in `.mdvault/activity.jsonl` as newline-delimited JSON:

```jsonl
{"ts":"2026-01-23T09:15:32Z","op":"new","type":"task","id":"TST-044","path":"tasks/TST-044.md","meta":{"project":"mdvault"}}
{"ts":"2026-01-23T09:20:15Z","op":"complete","type":"task","id":"TST-042","path":"tasks/TST-042.md"}
{"ts":"2026-01-23T10:05:00Z","op":"capture","type":"daily","path":"Journal/Daily/2026-01-23.md","meta":{"section":"Logs"}}
{"ts":"2026-01-23T10:30:00Z","op":"new","type":"daily","path":"Journal/Daily/2026-01-25.md"}
```

### Log Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `ts` | ISO 8601 datetime | Yes | Timestamp of operation |
| `op` | string | Yes | Operation type (see below) |
| `type` | string | Yes | Note type (task, project, daily, weekly, zettel, custom) |
| `id` | string | No | Note ID if applicable (task-id, project-id) |
| `path` | string | Yes | Path to note relative to vault root |
| `meta` | object | No | Additional context (project, section, etc.) |

### Operation Types

| Operation | Description |
|-----------|-------------|
| `new` | Note created |
| `update` | Note metadata updated |
| `complete` | Task/project marked complete |
| `reopen` | Task/project reopened |
| `capture` | Content captured/appended |
| `rename` | Note renamed/moved |
| `delete` | Note deleted |
| `focus` | Focus context changed |

### Log Rotation

- Logs older than 90 days are archived to `.mdvault/activity-archive/YYYY-MM.jsonl`
- Configurable via `activity_retention_days` in config

## Context Aggregation

### Data Sources

```
┌─────────────────────────────────────────────────────────────┐
│                      Context Engine                          │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Activity Log │  │ File System  │  │  Git (opt)   │      │
│  │  (primary)   │  │  (fallback)  │  │  (enhanced)  │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                 │                 │               │
│         ▼                 ▼                 ▼               │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                  Aggregator                          │   │
│  │  • Merge by timestamp                               │   │
│  │  • Deduplicate (prefer logged over detected)        │   │
│  │  • Enrich with note metadata                        │   │
│  │  • Parse notes for manual changes                   │   │
│  └─────────────────────────────────────────────────────┘   │
│                           │                                 │
│                           ▼                                 │
│                    Context Output                           │
└─────────────────────────────────────────────────────────────┘
```

### Aggregation Rules

1. **Logged operations** take precedence (higher confidence)
2. **Detected changes** fill gaps (manual edits in editors)
3. **Deduplication**: If a file was both logged and detected as modified, use logged entry
4. **Note parsing**: Extract structured data (tasks, logs) from note content

### Detecting Manual Changes

For unlogged changes:

1. **File mtime**: Compare against last logged operation for that file
2. **Git diff** (if available): Precise change detection with diff content
3. **Content parsing**: Detect new log entries, task status changes, etc.

## Output Formats

### Day Context - Markdown

```markdown
# Context: 2026-01-23 (Thursday)

## Summary
- 3 tasks completed
- 2 tasks created
- 5 notes modified
- Focus: mdvault

## Daily Note
- Path: Journal/Daily/2026-01-23.md
- Sections: Planning, Logs (5 entries), Notes
- [Read full note →]

## Task Activity

### Completed (3)
| Task | Title | Project |
|------|-------|---------|
| TST-042 | Fix date expression bug | mdvault |
| TST-043 | Add regression tests | mdvault |
| TST-041 | Update documentation | mdvault |

### Created (2)
| Task | Title | Project |
|------|-------|---------|
| TST-044 | Review MCP tools | mdvault |
| TST-045 | Design context command | mdvault |

### In Progress (2)
| Task | Title | Project |
|------|-------|---------|
| TST-044 | Review MCP tools | mdvault |
| TST-045 | Design context command | mdvault |

## Modified Notes (5)
| Note | Type | Change Source |
|------|------|---------------|
| docs/bugs/002-*.md | zettel | logged (new) |
| Projects/mdvault/mdvault.md | project | detected (+2 logs) |
| crates/core/src/domain/creator.rs | - | detected |
| ... | | |

## Projects with Activity
| Project | Tasks Done | Tasks Active | Logs Added |
|---------|------------|--------------|------------|
| mdvault | 3 | 2 | 5 |
```

### Day Context - JSON

```json
{
  "date": "2026-01-23",
  "day_of_week": "Thursday",
  "summary": {
    "tasks_completed": 3,
    "tasks_created": 2,
    "notes_modified": 5,
    "focus": "mdvault"
  },
  "daily_note": {
    "path": "Journal/Daily/2026-01-23.md",
    "exists": true,
    "sections": ["Planning", "Logs", "Notes"],
    "log_count": 5
  },
  "tasks": {
    "completed": [
      {"id": "TST-042", "title": "Fix date expression bug", "project": "mdvault"}
    ],
    "created": [
      {"id": "TST-044", "title": "Review MCP tools", "project": "mdvault"}
    ],
    "in_progress": [
      {"id": "TST-044", "title": "Review MCP tools", "project": "mdvault"}
    ]
  },
  "activity": [
    {"ts": "2026-01-23T09:15:32Z", "source": "logged", "op": "new", "type": "task", "id": "TST-044"},
    {"ts": "2026-01-23T10:30:00Z", "source": "detected", "summary": "Projects/mdvault.md +2 logs"}
  ],
  "modified_notes": [
    {"path": "docs/bugs/002-*.md", "type": "zettel", "source": "logged"},
    {"path": "Projects/mdvault/mdvault.md", "type": "project", "source": "detected"}
  ],
  "projects": [
    {"name": "mdvault", "tasks_done": 3, "tasks_active": 2, "logs_added": 5}
  ]
}
```

### Note Context - Project (JSON)

```json
{
  "type": "project",
  "path": "Projects/mdvault/mdvault.md",
  "metadata": {
    "project-id": "mdvault",
    "status": "active",
    "created": "2025-12-01",
    "title": "mdvault"
  },
  "sections": ["Overview", "Goals", "Roadmap", "Logs"],
  "tasks": {
    "summary": {"todo": 5, "doing": 2, "done": 48},
    "recent": {
      "completed": ["TST-042", "TST-043"],
      "active": ["TST-044", "TST-045"]
    }
  },
  "activity": {
    "period": "7d",
    "entries": [
      {"date": "2026-01-23", "source": "logged", "summary": "Completed TST-042, TST-044"},
      {"date": "2026-01-22", "source": "detected", "summary": "+3 log entries"}
    ]
  },
  "references": {
    "backlinks": ["Journal/Daily/2026-01-23.md", "docs/roadmap.md"],
    "backlink_count": 4,
    "outgoing": ["docs/design/architecture.md", "tasks/TST-044.md"],
    "outgoing_count": 8
  }
}
```

## MCP Integration

### New MCP Tools

| Tool | Description |
|------|-------------|
| `get_context_day` | Get context for a specific day |
| `get_context_week` | Get context for a specific week |
| `get_context_note` | Get context for a specific note |
| `get_context_focus` | Get context for current focus |

### Tool Definitions

```python
# get_context_day
{
  "name": "get_context_day",
  "description": "Get activity context for a specific day including tasks, notes modified, and logs",
  "parameters": {
    "date": {
      "type": "string",
      "description": "Date in YYYY-MM-DD format, or 'today', 'yesterday', or date expression",
      "default": "today"
    },
    "depth": {
      "type": "string",
      "enum": ["minimal", "normal", "full"],
      "default": "normal"
    }
  }
}

# get_context_note
{
  "name": "get_context_note",
  "description": "Get context for a specific note including metadata, sections, activity, and references",
  "parameters": {
    "note_path": {
      "type": "string",
      "description": "Path to the note relative to vault root",
      "required": true
    },
    "activity_days": {
      "type": "integer",
      "description": "Number of days of activity history to include",
      "default": 7
    }
  }
}
```

### Recommended MCP Flows

#### Flow 1: Daily Planning

```
Agent Goal: Help user plan their day

1. get_context_day(date="yesterday", depth="normal")
   → Understand what was done yesterday

2. get_context_day(date="today", depth="minimal")
   → See what's already planned/started today

3. get_context_focus()
   → Check if there's an active focus project

4. list_tasks(status="todo", project=<focus>)
   → Get available tasks for focused project

5. Synthesize: Suggest priorities based on:
   - Incomplete tasks from yesterday
   - Due dates
   - Project focus
```

#### Flow 2: Status Report Generation

```
Agent Goal: Generate weekly status report

1. get_context_week(week="last")
   → Get overview of last week's activity

2. For each project with activity:
   get_context_note(note_path=<project_path>)
   → Get project-specific details

3. For notable completed tasks:
   get_task_details(task_id=<id>)
   → Get task details for highlights

4. Synthesize: Generate report with:
   - Accomplishments (completed tasks)
   - Progress (task counts, project health)
   - Blockers (stuck tasks)
   - Next week focus
```

#### Flow 3: Project Deep Dive

```
Agent Goal: Understand current state of a project

1. get_context_note(note_path="Projects/X/X.md", activity_days=14)
   → Get project overview and recent activity

2. Based on task summary, if doing > 0:
   For each active task:
     get_task_details(task_id=<id>)
     → Understand what's in progress

3. If backlinks > 0:
   find_backlinks(note_path="Projects/X/X.md")
   → See what references this project

4. read_note_excerpt(note_path="Projects/X/X.md", section="Logs")
   → Read recent log entries for context

5. Synthesize: Provide project status with:
   - Current state
   - Active work
   - Recent progress
   - Connections to other notes
```

#### Flow 4: Resume Context

```
Agent Goal: Help user resume work after time away

1. get_context_day(date="today", depth="minimal")
   → Check today's state

2. If minimal activity today:
   get_context_day(date="--lookback")
   → Find last day with activity

3. get_context_focus()
   → Check if focus was set

4. If focus exists:
   get_context_note(note_path=<focus_project>)
   → Get project state

5. list_tasks(status="doing")
   → Find in-progress tasks

6. Synthesize: Provide resume summary:
   - Last activity: X days ago
   - Was working on: <project/task>
   - Left off at: <state>
   - Suggested next action
```

### MCP Best Practices

1. **Start with context, not content**
   - Use `get_context_*` before `read_note`
   - Context tells you what to read; avoid reading everything

2. **Use appropriate depth**
   - `minimal`: Quick checks, existence tests
   - `normal`: Planning, reporting (default)
   - `full`: Deep analysis, debugging

3. **Respect progressive discovery**
   - Get summaries first
   - Drill down only when needed
   - Avoid reading full notes unless necessary

4. **Trust logged over detected**
   - `source: "logged"` = high confidence
   - `source: "detected"` = inferred, may be incomplete

5. **Handle missing data gracefully**
   - Daily notes may not exist for all days
   - Activity logs may be empty for historical dates
   - Fall back to file analysis when logs unavailable

## Configuration

```toml
# In .mdvault/config.toml

[activity]
# Enable activity logging (default: true)
enabled = true

# Log retention in days (default: 90)
retention_days = 90

# Operations to log (default: all)
# Options: new, update, complete, reopen, capture, rename, delete, focus
log_operations = ["new", "update", "complete", "capture", "rename"]

[context]
# Default activity lookback for note context (days)
default_activity_days = 7

# Include git diff info if vault is a git repo
use_git = true

# Skip weekends in "lookback" mode
skip_weekends = true
```

## Implementation Plan

### Phase 1: Activity Logging ([#89](https://github.com/agustinvalencia/mdvault/issues/89))
- [ ] Add activity log writer to CLI operations
- [ ] Define log schema and storage location
- [ ] Add log rotation/archival

### Phase 2: Context Command (Day/Week) ([#90](https://github.com/agustinvalencia/mdvault/issues/90))
- [ ] Implement `mdv context day` with log aggregation
- [ ] Implement `mdv context week` with rollup
- [ ] Add file-based change detection fallback
- [ ] Add git integration (optional)

### Phase 3: Context Command (Note) ([#91](https://github.com/agustinvalencia/mdvault/issues/91))
- [ ] Implement `mdv context note` for projects
- [ ] Implement `mdv context note` for tasks
- [ ] Implement `mdv context note` for generic notes
- [ ] Implement `mdv context focus`

### Phase 4: MCP Integration ([#92](https://github.com/agustinvalencia/mdvault/issues/92))
- [ ] Add `get_context_day` tool
- [ ] Add `get_context_week` tool
- [ ] Add `get_context_note` tool
- [ ] Add `get_context_focus` tool
- [ ] Document recommended flows

### Phase 5: Polish ([#93](https://github.com/agustinvalencia/mdvault/issues/93))
- [ ] Add `--depth` levels
- [ ] Add `--lookback` smart detection
- [ ] Performance optimization for large vaults
- [ ] Add summary output format

## Open Questions

1. **Should context include content snippets?**
   - Pro: More useful without follow-up calls
   - Con: Violates progressive discovery, larger payloads

2. **How to handle vaults without activity logs?**
   - Option A: Full file-based analysis (slower, less precise)
   - Option B: Require logging enabled for context features

3. **Should we track "reading" operations?**
   - Knowing what was viewed could inform "resume context"
   - Privacy/noise concerns

4. **Integration with external tools?**
   - Could detect Obsidian edits via .obsidian metadata?
   - Git commit messages as activity source?
