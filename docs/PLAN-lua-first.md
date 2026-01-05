# Lua-First Architecture Plan

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

#### Phase 1: Add Lua-template linking
- [ ] Support `lua:` frontmatter field in templates
- [ ] Load Lua script when processing template
- [ ] Use Lua schema for prompting (alongside existing vars)
- [ ] Keep backward compatibility with old templates

#### Phase 2: Schema-driven prompts
- [ ] Replace template `vars:` with Lua schema prompts
- [ ] Add prompt UI based on schema field types
- [ ] Support enum selectors, multiline input, date pickers

#### Phase 3: Validation integration
- [ ] Validate frontmatter against Lua schema before writing
- [ ] Run Lua `validate()` function
- [ ] Show clear error messages for validation failures

#### Phase 4: Deprecate old DSL
- [ ] Warn when using template frontmatter DSL (output, vars)
- [ ] Migrate built-in templates to Lua-first
- [ ] Update documentation

#### Phase 5: Captures and Macros
- [ ] Consider if captures should also be Lua-based
- [ ] Evaluate macro system overlap

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

## Open Questions

1. Should captures become Lua-based too? (Probably yes for consistency)
2. How to handle templates without Lua? (Fallback to minimal scaffolding?)
3. Should we support inline Lua in templates? (Probably no - keep it simple)
4. How to version/migrate existing user configs?

## Next Steps

1. Review and approve this plan
2. Start with Phase 1: Lua-template linking
3. Iterate based on usage feedback
