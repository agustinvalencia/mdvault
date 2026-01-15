# Lua-First Architecture Plan

> **COMPLETE**: All phases of the Lua-first migration are now complete. This document is retained for historical reference and detailed Lua integration documentation. See [PLAN-v0.2.0.md](./PLAN-v0.2.0.md) for the overall v0.2.0 release status.

## Overview

Consolidate the template/type system into a Lua-first architecture where:
- **Markdown templates** define content structure only
- **Lua scripts** define all behavior: schema, prompts, validation, hooks, output paths

## Current State (Problems)

```
templates/
  task.md          # Has frontmatter DSL: output, vars, extra fields
  project.md       # Duplicates schema info from Lua

types/
  task.lua         # Has schema (unused), validate (unused), hooks
  project.lua      # Disconnected from template

captures/
  inbox.yaml       # Yet another DSL

macros/
  weekly-review.lua
```

Issues:
- Two places define "what is a task" (template frontmatter + Lua schema)
- Lua schema not used for validation during creation
- Template `vars:` duplicates Lua schema fields
- No clear mapping between template and type definition

## Proposed Architecture

### 1. Template Format (Simplified)

Templates become pure content with a single Lua reference:

```markdown
---
lua: task.lua
---

# {{title}}

## Description

{{description}}

## Checklist

- [ ] {{checklist_item}}
```

The `lua:` field points to the script that defines all behavior for this template.

### 2. Lua Script Structure

```lua
-- task.lua
return {
    -- Identity
    name = "task",
    description = "Actionable task with project association",

    -- Schema defines ALL fields (core + custom)
    schema = {
        -- Core fields (managed by Rust, always present)
        ["type"] = { type = "string", core = true },
        ["title"] = { type = "string", core = true, required = true },
        ["task-id"] = { type = "string", core = true },
        ["project"] = { type = "string", core = true },

        -- User fields (prompted/defaulted)
        ["status"] = {
            type = "string",
            enum = { "todo", "in-progress", "blocked", "done" },
            default = "todo",
        },
        ["priority"] = {
            type = "string",
            enum = { "low", "medium", "high" },
            prompt = "Priority level?",  -- Will prompt user
        },
        ["due"] = {
            type = "date",
            prompt = "Due date (optional)?",
            required = false,
        },
        ["description"] = {
            type = "string",
            prompt = "Task description?",
            multiline = true,
        },
    },

    -- Output path (supports {{variables}})
    output = "Projects/{{project}}/Tasks/{{task-id}}.md",

    -- Fallback output for inbox tasks
    output_inbox = "Inbox/{{task-id}}.md",

    -- Validation (called before writing)
    validate = function(note)
        if note.frontmatter.status == "done" and not note.frontmatter.completed_at then
            return false, "Completed tasks must have completed_at"
        end
        return true
    end,

    -- Hooks
    on_create = function(note, ctx)
        -- Can modify note.frontmatter, note.content
        -- ctx provides vault access
    end,

    on_update = function(note, old_note, ctx)
        -- Called when note is modified
    end,
}
```

### 3. Rust Core Responsibilities

```
┌─────────────────────────────────────────────────────────────┐
│                      Rust Core                               │
├─────────────────────────────────────────────────────────────┤
│  1. Load template.md, extract `lua:` path                   │
│  2. Load and execute Lua script                             │
│  3. Generate core fields (project-id, task-id, etc.)        │
│  4. Prompt user for schema fields (via Lua schema)          │
│  5. Validate against schema                                  │
│  6. Render template with all variables                       │
│  7. Run on_create hook (Lua can modify)                     │
│  8. Ensure core metadata preserved                           │
│  9. Write file                                               │
│  10. Reindex                                                 │
└─────────────────────────────────────────────────────────────┘
```

### 4. Variable Resolution Order

When rendering `{{variable}}`:

1. **Core variables** (from Rust): `type`, `title`, `project-id`, `task-id`, `today`, `now`, etc.
2. **CLI variables** (--var key=value): Override anything
3. **Schema defaults** (from Lua): `status = "todo"`
4. **User prompts** (from Lua schema): Fields with `prompt` attribute
5. **Computed** (from Lua on_create): Can add/modify fields

### 5. Prompt Flow

```
$ mdv new task "Fix the bug"

[Rust reads task.lua schema, finds fields needing prompts]

? Select project for this task:
  > Inbox (no project)
    MCP - My Cool Project
    HAU - Home Automation

? Priority level? [low/medium/high] (medium): high

? Due date (optional)?: 2025-01-20

? Task description?:
  > Need to fix the null pointer in auth module
  > (empty line to finish)

Creating task...
OK   mdv new
type:   task
id:     MCP-042
output: Projects/MCP/Tasks/MCP-042.md
```

### 6. Schema Field Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `type` | string | `string`, `number`, `boolean`, `date`, `list` |
| `required` | bool | Must have value (default: false) |
| `default` | any | Default value if not provided |
| `enum` | list | Allowed values (shown as selector) |
| `prompt` | string | Prompt text (if set, will ask user) |
| `multiline` | bool | For strings, allow multiline input |
| `core` | bool | Managed by Rust, cannot be overridden by user |
| `description` | string | Help text shown during prompts |

### 7. Migration Path

#### Phase 1: Lua-template linking (Completed)
- [x] Support `lua:` frontmatter field in templates
- [x] Load Lua script when processing template
- [x] Use Lua schema for prompting (fields with `prompt` attribute)
- [x] Use Lua script's `output` path when template doesn't specify one
- [x] Support filters in output paths (e.g., `{{title | slugify}}`)
- [x] Remove deprecated `vars:` DSL (breaking change for v0.2.0)
- [x] Fix title handling in template mode (first positional arg)
- [x] Fix Lua Nil → None conversion for required field validation

#### Phase 2: Enhanced prompts (Completed)
- [x] Add enum selectors (Select widget for enum fields)
- [x] Support multiline input (Editor widget for multiline fields)
- [x] Make project-id promptable with computed default
- [ ] Add date picker UI (deferred - text input with validation works for now)

#### Phase 3: Validation integration (Completed)
- [x] Validate frontmatter against Lua schema before writing
- [x] Run Lua `validate()` function during creation
- [x] Show clear error messages for validation failures

#### Phase 4: Built-in templates (Completed)
- [x] Migrate built-in task/project templates to Lua-first
- [x] Add example templates in documentation (lua-scripting.md)

#### Phase 5: Captures and Macros (Completed)
- [x] Lua-based captures implemented (`crates/core/src/captures/lua_loader.rs`)
- [x] Lua-based macros implemented (`crates/core/src/macros/lua_loader.rs`)
- [x] YAML captures/macros deprecated with warning
- [x] Documentation updated (`docs/lua-scripting.md`)
- [x] Examples migrated to Lua

## File Structure (After Migration)

```
.mdvault/
├── types/
│   ├── task.lua        # Schema, validation, hooks
│   ├── project.lua
│   ├── daily.lua
│   └── zettel.lua
├── templates/
│   ├── task.md         # Just: lua: task.lua + content
│   ├── project.md
│   ├── daily.md
│   └── zettel.md
├── captures/           # TBD: Maybe convert to Lua
│   └── inbox.yaml
└── macros/
    └── weekly-review.lua
```

## Example: Complete Task Creation Flow

```
1. User runs: mdv new task "Fix auth bug"

2. Rust loads template:
   ---
   lua: task.lua
   ---
   # {{title}}
   ...

3. Rust loads task.lua, gets schema

4. Rust generates core fields:
   - type = "task"
   - title = "Fix auth bug"
   - task-id = (needs project first)

5. Rust prompts for schema fields:
   - project: [selector from index]
   - priority: [enum selector]
   - due: [optional date]
   - description: [multiline]

6. Rust generates task-id: MCP-043

7. Rust validates against schema

8. Rust renders template with all vars

9. Rust runs on_create hook (Lua can modify)

10. Rust ensures core metadata

11. Rust writes file

12. Rust logs to daily, reindexes
```

## Benefits

1. **Single source of truth**: Lua script defines everything about a type
2. **Powerful validation**: Schema + custom validate() function
3. **Flexible prompts**: Schema-driven, with enum selectors, multiline, etc.
4. **Clear separation**: Markdown = content, Lua = behavior
5. **Extensible**: Users can add custom types easily
6. **Testable**: Lua scripts can be tested independently

## Architectural Boundary: Rust Core vs Lua Extensions

**See**: [Domain Types Architecture](./ARCHITECTURE-domain-types.md)

The Lua-first migration does **not** mean moving all logic to Lua. First-class types (task, project, daily, weekly) remain Rust-owned for:

- **Stability**: Future features (progress tracking, reporting) need predictable structure
- **Atomicity**: ID generation, counter management require Rust guarantees
- **Performance**: Critical paths stay in compiled code

Lua provides the **extension layer**:
- Schema definitions and prompts
- Validation hooks
- Post-creation customization
- User-defined types

The refactoring of `crates/cli/src/cmd/new.rs` will use trait-based dispatch (`NoteIdentity`, `NoteLifecycle`, `NotePrompts`) rather than moving logic to Lua scripts.

## Open Questions

1. Should captures become Lua-based too? (Probably yes for consistency)
2. How to handle templates without Lua? (Fallback to minimal scaffolding?)
3. Should we support inline Lua in templates? (Probably no - keep it simple)
4. How to version/migrate existing user configs?

## Quick Start: Using Lua-Template Integration

Phase 1 is now implemented! Here's how to use it:

### 1. Create a Lua type definition

**`~/.config/mdvault/types/meeting.lua`**:
```lua
return {
    description = "Meeting notes template",

    schema = {
        -- Fields with 'prompt' will be asked interactively
        attendees = {
            type = "string",
            prompt = "Who's attending?",
            required = true,
        },
        agenda = {
            type = "string",
            prompt = "Meeting agenda?",
            multiline = true,
        },
        priority = {
            type = "string",
            enum = { "low", "normal", "high" },
            default = "normal",
            prompt = "Priority level?",
        },
    },

    -- Output path (template can override this)
    output = "Meetings/{{title | slugify}}.md",
}
```

### 2. Create a template that references it

**`~/.config/mdvault/templates/meeting.md`**:
```markdown
---
lua: meeting.lua
---

# {{title}}

**Date**: {{today}}
**Attendees**: {{attendees}}
**Priority**: {{priority}}

## Agenda

{{agenda}}

## Notes

(Add meeting notes here)

## Action Items

- [ ]
```

### 3. Use it

```bash
# Interactive mode - prompts for schema fields with 'prompt' attribute
mdv new --template meeting "Weekly Standup"

# With vars - skip prompts by providing values
mdv new --template meeting "Design Review" \
    --var attendees="Alice, Bob, Carol" \
    --var agenda="Review Q2 roadmap" \
    --var priority=high

# Batch mode - uses defaults, fails if required fields missing
mdv new --template meeting "Quick Sync" --batch \
    --var attendees="Team"
```

### How it works

1. Template's `lua:` field points to the Lua script
2. Rust loads the Lua script and extracts the schema
3. For each schema field with `prompt` set:
   - Skip if already provided via `--var`
   - In batch mode: use default or fail if required
   - In interactive mode: prompt the user
4. Variables are passed to template rendering
5. Output path comes from: CLI `--output` > template frontmatter > Lua `output`

### Breaking Change (v0.2.0)

The deprecated `vars:` DSL in template frontmatter is no longer supported.
Templates must now use `lua:` to reference a Lua script for prompts and schema.

Templates without `lua:` will only have access to CLI-provided variables (`--var`).

## Completion Summary

All phases of the Lua-first architecture migration are complete:

1. ~~Phase 1: Lua-template linking~~ Complete (v0.2.0)
2. ~~Phase 2: Enhanced prompts~~ Complete (enum selectors, multiline, project-id prompt)
3. ~~Phase 3: Validation integration~~ Complete (schema + Lua validate() on create)
4. ~~Phase 4: Built-in templates~~ Complete (task, project, daily examples in docs)
5. ~~Phase 5: Captures and Macros~~ Complete (Lua-based captures/macros with YAML deprecation)

Future work will be tracked in separate planning documents.
