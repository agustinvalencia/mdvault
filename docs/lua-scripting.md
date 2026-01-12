# Lua Scripting in mdvault

mdvault includes a sandboxed Lua scripting layer that provides access to core functionality like date math and template rendering. This enables user-defined validation rules, custom type definitions, and automation scripts.

## Overview

The Lua scripting layer is designed with these principles:

- **Safe by default**: Dangerous operations (file I/O, shell execution, module loading) are disabled
- **Access to core engines**: Date math and template rendering are exposed via the `mdv` global table
- **User-configurable**: Define your own note types, validation rules, and workflows

## Quick Start

### Using Lua from Rust

```rust
use mdvault_core::scripting::LuaEngine;

// Create a sandboxed Lua engine
let engine = LuaEngine::sandboxed().unwrap();

// Evaluate date expressions
let date = engine.eval_string(r#"mdv.date("today + 7d")"#).unwrap();
println!("One week from now: {}", date);

// Render templates
let greeting = engine.eval_string(
    r#"mdv.render("Hello {{name}}!", { name = "World" })"#
).unwrap();
println!("{}", greeting);  // "Hello World!"
```

## The `mdv` Global Table

All mdvault functionality is exposed through the `mdv` global table.

### `mdv.date(expr, format?)`

Evaluate date math expressions with optional custom formatting.

```lua
-- Basic date expressions
mdv.date("today")           -- "2025-12-29"
mdv.date("now")             -- "2025-12-29T14:30:45"
mdv.date("time")            -- "14:30"
mdv.date("week")            -- "52" (ISO week number)
mdv.date("year")            -- "2025"

-- Date arithmetic
mdv.date("today + 1d")      -- Tomorrow
mdv.date("today - 7d")      -- One week ago
mdv.date("today + 2w")      -- Two weeks from now
mdv.date("today + 1M")      -- One month from now
mdv.date("now + 2h")        -- Two hours from now
mdv.date("now - 30m")       -- 30 minutes ago

-- Weekday expressions
mdv.date("today + monday")  -- Next Monday
mdv.date("today - friday")  -- Previous Friday

-- Week/year expressions
mdv.date("week/start")      -- Start of current week (Monday)
mdv.date("week/end")        -- End of current week (Sunday)
mdv.date("year/start")      -- January 1st of current year

-- Custom formatting (strftime)
mdv.date("today", "%A")           -- "Sunday" (weekday name)
mdv.date("today", "%B %d, %Y")    -- "December 29, 2025"
mdv.date("now", "%H:%M:%S")       -- "14:30:45"
mdv.date("week", "%Y-W%V")        -- "2025-W52"

-- Combined expressions
mdv.date("today + monday", "%Y-%m-%d")  -- Next Monday in ISO format
```

#### Supported Duration Units

| Unit | Syntax | Example |
|------|--------|---------|
| Minutes | `m` | `now + 30m` |
| Hours | `h` | `now + 2h` |
| Days | `d` | `today + 1d` |
| Weeks | `w` | `today + 2w` |
| Months | `M` | `today + 1M` |
| Years | `y` | `today + 1y` |

#### Supported Weekdays

Both full names and abbreviations work: `monday`/`mon`, `tuesday`/`tue`, `wednesday`/`wed`, `thursday`/`thu`, `friday`/`fri`, `saturday`/`sat`, `sunday`/`sun`.

### `mdv.render(template, context)`

Render templates with variable substitution.

```lua
-- Simple variable substitution
mdv.render("Hello {{name}}!", { name = "World" })
-- "Hello World!"

-- Multiple variables
mdv.render("{{greeting}}, {{name}}!", { greeting = "Hi", name = "Lua" })
-- "Hi, Lua!"

-- Numbers are converted to strings
mdv.render("Count: {{n}}", { n = 42 })
-- "Count: 42"

-- Booleans work too
mdv.render("Active: {{active}}", { active = true })
-- "Active: true"

-- Date expressions in templates are evaluated
mdv.render("Today is {{today}}", {})
-- "Today is 2025-12-29"

mdv.render("Created on {{today + 1d}}", {})
-- "Created on 2025-12-30"
```

#### Context Value Types

The context table can contain:

| Lua Type | Conversion |
|----------|------------|
| `string` | Used directly |
| `number` | Converted to string |
| `boolean` | `"true"` or `"false"` |
| `nil` | Empty string |
| `table`/`function` | Error |

#### Filters

Template expressions support filters using pipe syntax:

```lua
-- slugify: convert to URL-friendly slug
mdv.render("{{title | slugify}}", { title = "My New Task!" })
-- "my-new-task"

-- lowercase/lower: convert to lowercase
mdv.render("{{name | lowercase}}", { name = "HELLO" })
-- "hello"

-- uppercase/upper: convert to uppercase
mdv.render("{{name | upper}}", { name = "hello" })
-- "HELLO"

-- trim: remove leading/trailing whitespace
mdv.render("{{text | trim}}", { text = "  hello  " })
-- "hello"
```

Filters are commonly used in Lua type definitions for output paths:

```lua
-- In types/task.lua
return {
    output = "tasks/{{title | slugify}}.md",
    schema = {
        title = {
            type = "string",
            required = true,
            prompt = "Task title"
        }
    }
}
```

This creates files like `tasks/my-new-task.md` from titles like "My New Task!".

### `mdv.is_date_expr(str)`

Check if a string is a valid date expression.

```lua
mdv.is_date_expr("today")         -- true
mdv.is_date_expr("today + 1d")    -- true
mdv.is_date_expr("week/start")    -- true
mdv.is_date_expr("now - 2h")      -- true
mdv.is_date_expr("hello")         -- false
mdv.is_date_expr("random text")   -- false
```

## Sandbox Security

The Lua environment is sandboxed to prevent dangerous operations:

### Removed Globals

| Global | Reason |
|--------|--------|
| `io` | File system access |
| `os` | Shell execution, environment access |
| `require` | Loading external modules |
| `package` | Package system access |
| `load` | Loading arbitrary bytecode |
| `loadfile` | Loading code from files |
| `dofile` | Executing files |
| `debug` | Debugging/introspection (can bypass sandbox) |
| `collectgarbage` | Resource exhaustion attacks |

### Available Standard Libraries

| Library | Purpose |
|---------|---------|
| `base` | Core functions: `print`, `type`, `tostring`, `pairs`, `ipairs`, etc. |
| `table` | Table manipulation: `table.insert`, `table.sort`, etc. |
| `string` | String operations: `string.upper`, `string.find`, etc. |
| `utf8` | Unicode support: `utf8.len`, `utf8.codes`, etc. |
| `math` | Math functions: `math.floor`, `math.random`, etc. |

### Memory Limits

The default sandbox configuration limits memory usage to 10 MB to prevent resource exhaustion.

## Configuration

### SandboxConfig

```rust
use mdvault_core::scripting::{LuaEngine, SandboxConfig};

// Default restrictive sandbox
let engine = LuaEngine::sandboxed().unwrap();

// Custom configuration
let config = SandboxConfig {
    memory_limit: 20 * 1024 * 1024,  // 20 MB
    instruction_limit: 200_000,
    allow_require: false,
};
let engine = LuaEngine::new(config).unwrap();

// Unrestricted (use with caution!)
let config = SandboxConfig::unrestricted();
```

## Error Handling

Lua errors are converted to Rust errors:

```rust
use mdvault_core::scripting::{LuaEngine, ScriptingError};

let engine = LuaEngine::sandboxed().unwrap();

// Invalid date expression
match engine.eval_string(r#"mdv.date("invalid_expr")"#) {
    Ok(result) => println!("Result: {}", result),
    Err(ScriptingError::Lua(e)) => println!("Lua error: {}", e),
    Err(e) => println!("Other error: {}", e),
}
```

## Type Definitions

Type definitions allow you to define custom note types with schema validation, interactive prompts, and lifecycle hooks. Type definitions are Lua files stored in `~/.config/mdvault/types/`.

### Lua-Template Integration

Templates can link to Lua type definitions using the `lua:` frontmatter field:

```markdown
---
lua: meeting.lua
---

# {{title}}

**Attendees**: {{attendees}}
**Priority**: {{priority}}
```

The `lua:` path is resolved relative to your `types_dir` (e.g., `~/.config/mdvault/types/`). This enables:

1. **Schema-driven prompts**: Fields with `prompt` attribute are asked interactively
2. **Default values**: Fields with `default` use that value when not provided
3. **Output paths**: Lua's `output` field is used when template doesn't specify one
4. **Validation**: Schema and `validate()` function are applied before writing

### Creating a Type Definition

Create a `.lua` file in `~/.config/mdvault/types/`. The filename becomes the type name:

```lua
-- ~/.config/mdvault/types/meeting.lua
return {
    description = "Meeting notes with attendees and action items",

    -- Output path template (used when template doesn't specify one)
    -- Supports filters like {{title | slugify}}
    output = "Meetings/{{title | slugify}}.md",

    -- Template variables (ephemeral inputs for the template body)
    -- These are NOT saved to frontmatter unless you explicitly add them
    variables = {
        context = {
            prompt = "Meeting context?",
            default = "General"
        }
    },

    -- Schema defines the expected frontmatter fields
    schema = {
        title = {
            type = "string",
            required = true,
            core = true  -- Managed by Rust, passed from CLI
        },
        date = {
            type = "date",
            default = "today"  -- Uses date math expressions
        },
        attendees = {
            type = "string",
            required = true,
            prompt = "Who's attending?"  -- Prompts user interactively
        },
        status = {
            type = "string",
            enum = { "scheduled", "in-progress", "completed", "cancelled" },
            default = "scheduled",
            prompt = "Meeting status?"  -- Shows as selector
        },
        duration_minutes = {
            type = "number",
            min = 1,
            max = 480,
            prompt = "Duration in minutes?"
        }
    },

    -- Custom validation function (optional)
    validate = function(note)
        if note.frontmatter.status == "completed" and not note.frontmatter.summary then
            return false, "Completed meetings must have a summary"
        end
        return true
    end,

    -- Lifecycle hooks (optional)
    on_create = function(note)
        note.frontmatter.created_at = mdv.date("now", "%Y-%m-%dT%H:%M:%S")
        return note
    end,

    on_update = function(note, previous)
        note.frontmatter.updated_at = mdv.date("now", "%Y-%m-%dT%H:%M:%S")
        return note
    end
}
```

### Supported Field Types

| Type | Description | Constraints |
|------|-------------|-------------|
| `string` | Text value | `enum`, `pattern`, `min_length`, `max_length` |
| `number` | Numeric value | `min`, `max`, `integer` |
| `boolean` | True/false | - |
| `date` | Date (YYYY-MM-DD) | `min`, `max` |
| `datetime` | ISO 8601 datetime | `min`, `max` |
| `list` | Array of values | `items`, `min_items`, `max_items` |
| `reference` | Link to another note | `note_type` |

### Field Schema Properties

```lua
field_name = {
    type = "string",           -- Field type (required)
    required = true,           -- Is the field mandatory?
    description = "...",       -- Human-readable description
    default = "value",         -- Default value when not provided

    -- Interactive prompting (for Lua-template integration)
    prompt = "Enter value?",   -- If set, prompts user interactively
    multiline = false,         -- Allow multiline input (for strings)
    core = false,              -- If true, managed by Rust (not user-editable)
    inherited = false,         -- If true, value will be set by on_create hook

    -- String constraints
    enum = { "a", "b", "c" },  -- Allowed values (shown as selector)
    pattern = "^[A-Z]+$",      -- Regex pattern
    min_length = 1,            -- Minimum length
    max_length = 100,          -- Maximum length

    -- Number constraints
    min = 0,                   -- Minimum value
    max = 100,                 -- Maximum value
    integer = true,            -- Must be whole number

    -- List constraints
    min_items = 1,             -- Minimum items
    max_items = 10,            -- Maximum items
    items = { type = "string" }, -- Schema for list items

    -- Reference constraints
    note_type = "project"      -- Restrict to specific type
}
```

### Inherited Fields

Fields marked with `inherited = true` indicate that their value will be set by the `on_create` hook rather than being prompted or required upfront. This is useful for fields that should be derived from other data (like a parent note):

```lua
schema = {
    context = {
        type = "string",
        required = true,
        enum = { "work", "personal", "uni" },
        inherited = true  -- Value will be set by on_create hook
    }
}
```

When a field has `inherited = true`:
1. **No prompting**: The user is not prompted for this field during note creation
2. **Validation skipped**: Required-field validation is skipped before hooks run
3. **Hook responsibility**: The `on_create` hook must set the value

Example: A task inherits its `context` from its parent project:

```lua
on_create = function(note)
    local project_id = note.variables["project-id"]
    if project_id then
        local project = mdv.find_project(project_id)
        if project and project.frontmatter then
            if not note.frontmatter.context and project.frontmatter.context then
                note.frontmatter.context = project.frontmatter.context
            end
        end
    end
    return note
end
```

> **Important**: Use `note.variables` (not `note.frontmatter`) to access template variables like `project-id` in hooks.

### Template Variables

The `variables` block defines ephemeral inputs that are used for template rendering but are **not** automatically added to the note's frontmatter (unlike `schema` fields). This is useful for helper variables or context that you don't want to persist.

```lua
variables = {
    context = {
        prompt = "What is the context?",  -- Prompts user interactively
        default = "General",              -- Default value
        required = true,                  -- Must provide value
        description = "Context for the note"
    },
    -- Simple form (string is treated as prompt if it ends with ?, or default otherwise)
    mood = "How are you feeling?",
    tags = "default,tags"
}
```

Values collected from `variables` are available in the template as `{{context}}`, `{{mood}}`, etc., and are passed to lifecycle hooks in `note.variables`.

#### Interactive Prompt Behavior

When a template with `lua:` is used:

| Field Config | Interactive Mode | Batch Mode (`--batch`) |
|-------------|------------------|------------------------|
| `prompt` set, no `--var` | Prompts user | Uses `default` or fails if `required` |
| `prompt` set, `--var` provided | Uses `--var` value | Uses `--var` value |
| No `prompt`, has `default` | Uses default silently | Uses default |
| No `prompt`, no `default`, `required` | Error | Error |

### Custom Validation Function

The `validate` function receives a note table and returns validation status:

```lua
validate = function(note)
    -- note.type - the note type string
    -- note.path - path to the note file
    -- note.content - note content (body text)
    -- note.frontmatter - table with frontmatter fields

    -- Check custom business rules
    if note.frontmatter.priority > 5 and not note.frontmatter.assignee then
        return false, "High priority tasks must have an assignee"
    end

    -- Use mdv functions
    if mdv.is_date_expr(note.frontmatter.due) then
        local due = mdv.date(note.frontmatter.due)
        -- Additional date-based validation...
    end

    return true  -- Validation passed
end
```

### Lifecycle Hooks

Lifecycle hooks are called during note operations. The `on_create` hook is executed after a note is created via `mdv new`.

```lua
-- Called when creating a new note of this type
on_create = function(note)
    -- Add automatic timestamps
    note.frontmatter.created_at = mdv.date("now", "%Y-%m-%dT%H:%M:%S")

    -- Log to daily note using a capture
    local ok, err = mdv.capture("log-to-daily", {
        text = "Created: [[" .. note.path .. "]]"
    })
    if not ok then
        print("Warning: " .. err)
    end

    return note
end

-- Called when updating an existing note
on_update = function(note, previous)
    -- Track modification time
    note.frontmatter.updated_at = mdv.date("now", "%Y-%m-%dT%H:%M:%S")

    -- Preserve creation date from previous version
    note.frontmatter.created_at = previous.frontmatter.created_at

    return note
end
```

> **Note**: The `on_update` hook is defined but not yet called automatically. The `on_create` hook is fully integrated with `mdv new`.

### Built-in Types

mdvault has five built-in types: `daily`, `weekly`, `task`, `project`, and `zettel`. You can create Lua files with these names to add validation and hooks to them:

```lua
-- ~/.config/mdvault/types/task.lua
-- This overrides/extends the built-in task type
return {
    schema = {
        status = {
            type = "string",
            required = true,
            enum = { "open", "in-progress", "blocked", "done", "cancelled" }
        },
        project = {
            type = "reference",
            required = true
        },
        due = {
            type = "date"
        }
    },

    validate = function(note)
        if note.frontmatter.status == "done" and not note.frontmatter.completed_date then
            return false, "Done tasks require a completed_date field"
        end
        return true
    end
}
```

### Creating Notes with Type Scaffolding

Use `mdv new` with a type name to create notes with auto-generated frontmatter:

```bash
# Create a task with frontmatter from schema
mdv new task "Implement feature X"
# Creates: tasks/implement-feature-x.md

# Provide field values via --var
mdv new task "Fix bug" --var project=mdvault --var priority=high

# Create a project note
mdv new project "Network Slicing Research" --var status=planning

# Specify custom output path
mdv new task "Custom location" -o notes/custom.md

# Batch mode (no prompts, fail on missing required fields)
mdv new task "Batch task" --batch --var project=myproject
```

The generated note includes:
- `type` field set to the type name
- `title` field from the positional argument
- Fields from schema with defaults or provided values
- `created` field with current date
- A heading matching the title

Example output:
```yaml
---
type: task
title: Implement feature X
status: open        # from schema default
priority: medium    # from schema default
created: 2025-12-30
---

# Implement feature X

```

If a template exists with the same name as the type (e.g., `task.md`), it will be used instead of scaffolding.

### Validating Notes

Use `mdv validate` (or `mdv lint`) to validate notes against their type schemas:

```bash
# Validate all notes in the vault
mdv validate

# Validate a specific file
mdv validate path/to/note.md

# Validate only notes of a specific type
mdv validate --type task

# Auto-fix safe issues (missing defaults, enum case)
mdv validate --fix

# Show available type definitions
mdv validate --list-types

# JSON output for scripting
mdv validate --json

# Quiet mode (paths only)
mdv validate -q
```

#### Auto-fix Capabilities

The `--fix` flag automatically corrects:
- **Missing required fields**: Adds fields that have default values defined in the schema
- **Enum case normalization**: Fixes "OPEN" to "open" if the schema expects lowercase

Example:
```bash
$ mdv validate tasks/my-task.md
Validation Results: 0 valid, 1 with errors (of 1 total)

tasks/my-task.md  [type: task]
  - missing required field: status

$ mdv validate tasks/my-task.md --fix
Validation Results: 0 valid, 1 fixed, 0 with errors (of 1 total)

tasks/my-task.md  [type: task]
  + Added missing field 'status' with default 'open'
```

## Capture Definitions

Captures are quick append workflows that add content to a target file/section. Captures can be defined in Lua (preferred) or YAML (deprecated).

### Lua Capture Format

Create a `.lua` file in your `captures_dir` (default: `~/.config/mdvault/captures/`):

```lua
-- captures/inbox.lua
return {
    name = "inbox",
    description = "Add a quick note to today's inbox",

    -- Variables with prompts
    vars = {
        text = "What to capture?",  -- Simple form: string is the prompt
        -- OR full form:
        priority = {
            prompt = "Priority level?",
            default = "medium",
            required = false,
        },
    },

    -- Target file and section
    target = {
        file = "daily/{{date}}.md",
        section = "Inbox",
        position = "begin",  -- "begin" or "end"
        create_if_missing = true,  -- Create file if it doesn't exist
    },

    -- Content template (supports {{variable}} placeholders)
    content = "- [ ] {{text}} ({{priority}})",

    -- Frontmatter operations (optional)
    frontmatter = {
        -- Simple form: direct key-value sets
        last_updated = "{{date}}",

        -- OR explicit operations form:
        -- { field = "count", op = "increment" },
        -- { field = "active", op = "toggle" },
        -- { field = "tags", op = "append", value = "inbox" },
    },
}
```

### Capture Variables

Variables support two formats:

```lua
vars = {
    -- Simple: string is used as prompt
    text = "What to capture?",

    -- Full: object with metadata
    priority = {
        prompt = "Priority level?",  -- Interactive prompt
        default = "medium",          -- Default value
        required = true,             -- Must provide value
        description = "Help text",   -- Shown during prompting
    },
}
```

### Target Configuration

| Field | Type | Description |
|-------|------|-------------|
| `file` | string | Target file path (supports `{{var}}` placeholders) |
| `section` | string | Section heading to insert into (optional for frontmatter-only) |
| `position` | `"begin"` or `"end"` | Where in section to insert (default: `"begin"`) |
| `create_if_missing` | boolean | Create file if missing (default: `false`) |

### Frontmatter Operations

Captures can modify frontmatter in the target file:

```lua
-- Simple form: direct key-value sets
frontmatter = {
    status = "active",
    updated = "{{date}}",
}

-- Operations form: explicit operations
frontmatter = {
    { field = "count", op = "increment" },       -- Add 1 to numeric field
    { field = "active", op = "toggle" },         -- Flip boolean
    { field = "tags", op = "append", value = "new-tag" },  -- Add to list
    { field = "status", op = "set", value = "updated" },   -- Set value
}

-- Mixed form: both simple sets and explicit operations
frontmatter = {
    status = "active",  -- Simple set
    { field = "count", op = "increment" },  -- Explicit operation
}
```

| Operation | Description |
|-----------|-------------|
| `set` | Set field to value (default if `op` not specified) |
| `toggle` | Flip boolean field (false→true, true→false) |
| `increment` | Add 1 to numeric field (creates as 0 if missing) |
| `append` | Add value to list field (creates list if missing) |

### Using Captures

```bash
# Run a capture interactively
mdv capture inbox

# Provide variables via --var
mdv capture inbox --var text="Buy groceries" --var priority=high

# List available captures
mdv capture --list
```

### Migration from YAML

YAML captures are deprecated and will show a warning. To migrate:

**Before (YAML)**:
```yaml
name: inbox
description: Add to inbox
target:
  file: "daily/{{date}}.md"
  section: "Inbox"
  position: begin
content: "- [ ] {{text}}"
```

**After (Lua)**:
```lua
return {
    name = "inbox",
    description = "Add to inbox",
    vars = {
        text = "What to capture?",
    },
    target = {
        file = "daily/{{date}}.md",
        section = "Inbox",
        position = "begin",
    },
    content = "- [ ] {{text}}",
}
```

### Capture Examples

**Quick todo to daily note**:
```lua
-- captures/todo.lua
return {
    name = "todo",
    description = "Quick task to daily note",
    vars = {
        task = "What needs to be done?",
    },
    target = {
        file = "daily/{{date}}.md",
        section = "Tasks",
        position = "end",
        create_if_missing = true,
    },
    content = "- [ ] {{task}}",
}
```

**Meeting note with frontmatter**:
```lua
-- captures/meeting-note.lua
return {
    name = "meeting-note",
    description = "Add meeting note and update project",
    vars = {
        note = {
            prompt = "Meeting note?",
            multiline = true,
        },
    },
    target = {
        file = "projects/{{project}}.md",
        section = "Meeting Notes",
        position = "begin",
    },
    content = "### {{date}}\n\n{{note}}",
    frontmatter = {
        { field = "meeting_count", op = "increment" },
        last_meeting = "{{date}}",
    },
}
```

**Log entry with timestamp**:
```lua
-- captures/log.lua
return {
    name = "log",
    description = "Timestamped log entry",
    vars = {
        entry = "Log entry?",
    },
    target = {
        file = "logs/{{date}}.md",
        section = "Log",
        position = "end",
        create_if_missing = true,
    },
    content = "- **{{time}}** {{entry}}",
}
```

## Vault Operations

When running inside lifecycle hooks, the `mdv` table provides access to vault operations. These functions allow hooks to render templates, execute captures, and run macros.

### `mdv.template(name, vars?)`

Render a template by name and return its content.

```lua
-- Returns: (content, nil) on success, (nil, error) on failure
local content, err = mdv.template("meeting-summary", {
    title = note.frontmatter.title,
    date = mdv.date("today")
})

if err then
    print("Template error: " .. err)
else
    print(content)
end
```

### `mdv.capture(name, vars?)`

Execute a capture workflow (append content to a target file).

```lua
-- Returns: (true, nil) on success, (false, error) on failure
local ok, err = mdv.capture("log-to-daily", {
    text = "Created task: [[" .. note.path .. "]]"
})

if not ok then
    print("Capture error: " .. err)
end
```

### `mdv.macro(name, vars?)`

Execute a macro workflow (multi-step operations).

```lua
-- Returns: (true, nil) on success, (false, error) on failure
local ok, err = mdv.macro("on-task-created", {
    task_path = note.path,
    project = note.frontmatter.project
})

if not ok then
    print("Macro error: " .. err)
end
```

> **Note**: Shell steps in macros are NOT executed from hooks (no `--trust` context). Only template and capture steps will run.

### `mdv.read_note(path)`

Read a note's content and frontmatter by path.

```lua
-- Returns: (note_table, nil) on success, (nil, error) on failure
local note, err = mdv.read_note("projects/my-project.md")
if err then
    print("Error: " .. err)
else
    print("Title: " .. (note.title or "untitled"))
    print("Body length: " .. #note.body)
    if note.frontmatter then
        print("Status: " .. (note.frontmatter.status or "unknown"))
    end
end
```

The returned note table contains:

| Field | Type | Description |
|-------|------|-------------|
| `path` | string | The resolved path to the note |
| `content` | string | Full file content including frontmatter |
| `body` | string | Note body without frontmatter |
| `frontmatter` | table or nil | Frontmatter fields as a Lua table |
| `title` | string or nil | Title from frontmatter (convenience field) |
| `type` | string or nil | Note type from frontmatter (convenience field) |

Path resolution:
- Relative paths are resolved from the vault root
- The `.md` extension is optional (automatically appended if missing)

### Error Handling

All vault operations return two values for graceful error handling:

| Function | Success Return | Failure Return |
|----------|----------------|----------------|
| `mdv.template()` | `(content, nil)` | `(nil, error_message)` |
| `mdv.capture()` | `(true, nil)` | `(false, error_message)` |
| `mdv.macro()` | `(true, nil)` | `(false, error_message)` |
| `mdv.read_note()` | `(note_table, nil)` | `(nil, error_message)` |

Hooks should check for errors but failures are non-fatal—the CLI logs a warning but the note creation still succeeds.

### Complete Hook Example: Linking to Daily Note

This example shows how to automatically log task creation to the daily note. First, create a capture that targets the daily note:

```yaml
# ~/.config/mdvault/captures/log-to-daily.yaml
name: log-to-daily
description: Log a note link to today's daily note

target:
  file: "daily/{{date}}.md"
  section: "Created"
  position: end
  create_if_missing: true  # Creates daily note if it doesn't exist

content: "- [[{{note_path}}]] {{note_title}}"
```

Then create a type definition with an `on_create` hook:

```lua
-- ~/.config/mdvault/types/task.lua
return {
    name = "task",
    schema = {
        title = { type = "string", required = true },
        status = { type = "string", enum = { "open", "in-progress", "done" } },
        project = { type = "reference" }
    },

    on_create = function(note)
        -- Log task creation to daily note
        local ok, err = mdv.capture("log-to-daily", {
            note_path = note.path,
            note_title = note.frontmatter.title or "untitled"
        })

        if not ok then
            print("Warning: could not log to daily: " .. err)
        end

        return note
    end
}
```

Now when you create a task:

```bash
mdv new task "Implement search feature" --var project=myproject
```

The task is created and automatically linked in today's daily note under the "Created" section. If the daily note doesn't exist, it's created automatically with the target section.

### Inheriting Fields from Parent Notes

Use `mdv.read_note()` to inherit fields from a parent note. This is useful when child notes should share properties with their parent:

```lua
-- ~/.config/mdvault/types/task.lua
return {
    name = "task",
    schema = {
        title = { type = "string", required = true },
        status = { type = "string", enum = { "open", "in-progress", "done" } },
        project = { type = "reference" },
        context = { type = "string" }  -- inherited from project
    },

    on_create = function(note)
        -- If task has a project reference, inherit context from it
        if note.frontmatter.project then
            local project, err = mdv.read_note(note.frontmatter.project)
            if project and project.frontmatter then
                -- Inherit context if not already set
                if not note.frontmatter.context and project.frontmatter.context then
                    note.frontmatter.context = project.frontmatter.context
                end
            end
        end

        return note
    end
}
```

With this configuration, when you create a task:

```bash
mdv new task "Implement feature" --var project=projects/api-redesign
```

The task will automatically inherit the `context` field from `projects/api-redesign.md` if that project has one defined.

### Dynamic Variables in Hooks

The `on_create` hook can access and modify template variables via `note.variables`. This allows you to compute variables dynamically in Lua and have them injected into the template rendering context.

**Template (`templates/report.md`):**
```markdown
---
type: report
lua: report.lua
---
# Weekly Report

**Week**: {{week_number}}
**Generated**: {{generated_at}}

{{content}}
```

**Type Definition (`types/report.lua`):**
```lua
return {
    on_create = function(note)
        note.variables = note.variables or {}
        
        -- Compute dynamic variables
        note.variables.week_number = mdv.date("week", "%Y-W%V")
        note.variables.generated_at = mdv.date("now", "%Y-%m-%d %H:%M")
        
        return note
    end
}
```

When you create a note with `mdv new --template report`, the hook calculates `week_number` and `generated_at`, and the template is rendered with these values.

## Index Query Functions

These functions require the vault index (run `mdv reindex` first):

```lua
-- Get current note being processed
local note = mdv.current_note()

-- Get notes linking to a path
local backlinks = mdv.backlinks(note.path)

-- Get notes a path links to
local outlinks = mdv.outlinks(note.path)

-- Query the vault index
local tasks = mdv.query({ type = "task", limit = 10 })

-- Find a project by its project-id
local project = mdv.find_project("MCP")
```

> **Note**: These functions require running `mdv reindex` first to build the vault index.

## Examples

### Date Formatting for Notes

```lua
-- Create a formatted date for a daily note title
local title = mdv.date("today", "%A, %B %d, %Y")
-- "Sunday, December 29, 2025"

-- Create week identifier
local week_id = mdv.date("today", "%Y-W%V")
-- "2025-W52"

-- Format for frontmatter
local iso_date = mdv.date("today", "%Y-%m-%d")
-- "2025-12-29"
```

### Dynamic Template Variables

```lua
-- Build context with computed values
local context = {
  title = "Weekly Review",
  week = mdv.date("week"),
  week_start = mdv.date("week/start"),
  week_end = mdv.date("week/end"),
  created = mdv.date("now")
}

local content = mdv.render([[
# {{title}} - Week {{week}}

**Period**: {{week_start}} to {{week_end}}
**Created**: {{created}}
]], context)
```

### Conditional Logic

```lua
-- Check if an expression is a date before processing
local expr = "today + 1d"
if mdv.is_date_expr(expr) then
  local result = mdv.date(expr)
  print("Evaluated: " .. result)
else
  print("Not a date expression: " .. expr)
end
```

## API Reference

### LuaEngine

```rust
impl LuaEngine {
    /// Create with custom sandbox config
    pub fn new(config: SandboxConfig) -> Result<Self, ScriptingError>;

    /// Create with default restrictive sandbox
    pub fn sandboxed() -> Result<Self, ScriptingError>;

    /// Evaluate script, returns None for nil
    pub fn eval(&self, script: &str) -> Result<Option<String>, ScriptingError>;

    /// Evaluate script, error if nil
    pub fn eval_string(&self, script: &str) -> Result<String, ScriptingError>;

    /// Evaluate script expecting boolean
    pub fn eval_bool(&self, script: &str) -> Result<bool, ScriptingError>;

    /// Access underlying Lua state
    pub fn lua(&self) -> &Lua;
}
```

### ScriptingError

```rust
pub enum ScriptingError {
    Lua(mlua::Error),          // Lua runtime error
    DateMath(String),          // Date expression error
    TemplateRender(String),    // Template rendering error
    SandboxViolation(String),  // Security violation
}
```

### SandboxConfig

```rust
pub struct SandboxConfig {
    pub memory_limit: usize,      // Max memory in bytes (0 = unlimited)
    pub instruction_limit: u32,   // Max instructions (0 = unlimited)
    pub allow_require: bool,      // Allow module loading
}

impl SandboxConfig {
    pub fn restricted() -> Self;   // Safe defaults
    pub fn unrestricted() -> Self; // No limits (dangerous!)
}
```

## Built-in Type Examples

These examples show how to create Lua-first type definitions for the built-in types (task, project). Place these files in your `types_dir` (default: `~/.config/mdvault/types/`).

### Task Type Definition

**`types/task.lua`**:
```lua
return {
    name = "task",
    description = "Actionable task with project association",

    -- Output path template
    output = "Projects/{{project}}/Tasks/{{title | slugify}}.md",

    -- Schema defines all fields
    schema = {
        -- Core fields (managed by Rust)
        ["type"] = { type = "string", core = true },
        ["title"] = { type = "string", core = true, required = true },
        ["task-id"] = { type = "string", core = true },
        ["project"] = { type = "string", core = true },

        -- User fields
        ["status"] = {
            type = "string",
            enum = { "todo", "in-progress", "blocked", "done" },
            default = "todo",
            prompt = "Status?",
        },
        ["priority"] = {
            type = "string",
            enum = { "low", "medium", "high" },
            default = "medium",
            prompt = "Priority?",
        },
        ["due"] = {
            type = "date",
            required = false,
            prompt = "Due date (optional)?",
        },
        ["tags"] = {
            type = "list",
            required = false,
        },
    },

    -- Custom validation
    validate = function(note)
        local fm = note.frontmatter

        -- Completed tasks should have completed_at
        if fm.status == "done" and not fm.completed_at then
            -- This is a warning, not a hard failure
            -- The on_create hook will set completed_at
        end

        return true
    end,

    -- Lifecycle hook: called when task is created
    on_create = function(note)
        local fm = note.frontmatter

        -- Auto-set completed_at when status is done
        if fm.status == "done" and not fm.completed_at then
            fm.completed_at = mdv.date("now", "%Y-%m-%dT%H:%M:%S")
        end

        -- Ensure created timestamp exists
        if not fm.created then
            fm.created = mdv.date("today")
        end

        note.frontmatter = fm
        return note
    end,
}
```

**`templates/task.md`**:
```markdown
---
lua: task.lua
---

# {{title}}

**Status**: {{status}}
**Priority**: {{priority}}
{{#if due}}**Due**: {{due}}{{/if}}

## Description

{{description}}

## Checklist

- [ ]

## Notes

```

### Project Type Definition

**`types/project.lua`**:
```lua
return {
    name = "project",
    description = "Project with associated tasks",

    -- Output path template
    output = "Projects/{{project-id}}/{{project-id}}.md",

    schema = {
        -- Core fields
        ["type"] = { type = "string", core = true },
        ["title"] = { type = "string", core = true, required = true },
        ["project-id"] = {
            type = "string",
            core = true,
            prompt = "Project ID (3-letter code)?",
        },
        ["task_counter"] = { type = "number", core = true, default = 0 },

        -- User fields
        ["status"] = {
            type = "string",
            enum = { "planning", "active", "on-hold", "completed", "archived" },
            default = "active",
            prompt = "Project status?",
        },
        ["description"] = {
            type = "string",
            prompt = "Project description?",
            multiline = true,
        },
        ["start_date"] = {
            type = "date",
            default = "today",
        },
        ["target_date"] = {
            type = "date",
            required = false,
            prompt = "Target completion date (optional)?",
        },
        ["tags"] = {
            type = "list",
            required = false,
        },
    },

    validate = function(note)
        local fm = note.frontmatter

        -- Project ID should be uppercase letters
        if fm["project-id"] then
            local id = fm["project-id"]
            if not id:match("^%u+$") then
                return false, "project-id must be uppercase letters only"
            end
        end

        return true
    end,

    on_create = function(note)
        local fm = note.frontmatter

        -- Ensure project-id is uppercase
        if fm["project-id"] then
            fm["project-id"] = string.upper(fm["project-id"])
        end

        -- Set created timestamp
        if not fm.created then
            fm.created = mdv.date("today")
        end

        note.frontmatter = fm
        return note
    end,
}
```

**`templates/project.md`**:
```markdown
---
lua: project.lua
---

# {{title}}

**ID**: {{project-id}}
**Status**: {{status}}
**Started**: {{start_date}}
{{#if target_date}}**Target**: {{target_date}}{{/if}}

## Overview

{{description}}

## Tasks

<!-- Tasks will be linked here -->

## Notes

```

### Daily Note Type Definition

**`types/daily.lua`**:
```lua
return {
    name = "daily",
    description = "Daily journal entry",

    output = "Journal/Daily/{{today}}.md",

    schema = {
        ["type"] = { type = "string", core = true },
        ["date"] = { type = "date", core = true },
        ["mood"] = {
            type = "string",
            enum = { "great", "good", "okay", "rough" },
            required = false,
            prompt = "How are you feeling?",
        },
    },

    on_create = function(note)
        -- Set date to today
        note.frontmatter.date = mdv.date("today")

        -- Add dynamic variables for template
        note.variables = note.variables or {}
        note.variables.day_name = mdv.date("today", "%A")
        note.variables.week_number = mdv.date("week", "%V")

        return note
    end,
}
```

**`templates/daily.md`**:
```markdown
---
lua: daily.lua
---

# {{day_name}}, {{today}}

Week {{week_number}}

## Morning

- [ ]

## Tasks

## Notes

## Evening Reflection

```

### Using the Built-in Types

```bash
# Create a new project (prompts for ID and status)
mdv new project "My New Project"

# Create a task in a project
mdv new task "Implement feature X" --var project=MNP

# Create today's daily note
mdv new --template daily

# Batch mode (uses defaults, no prompts)
mdv new project "Quick Project" --batch --var project-id=QKP
```
