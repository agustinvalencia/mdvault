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

Type definitions allow you to define custom note types with schema validation and lifecycle hooks. Type definitions are Lua files stored in `~/.config/mdvault/types/`.

### Creating a Type Definition

Create a `.lua` file in `~/.config/mdvault/types/`. The filename becomes the type name:

```lua
-- ~/.config/mdvault/types/meeting.lua
return {
    name = "meeting",
    description = "Meeting notes with attendees and action items",

    -- Schema defines the expected frontmatter fields
    schema = {
        title = {
            type = "string",
            required = true
        },
        date = {
            type = "date",
            required = true
        },
        attendees = {
            type = "list",
            required = true,
            min_items = 1
        },
        status = {
            type = "string",
            enum = { "scheduled", "in-progress", "completed", "cancelled" },
            default = "scheduled"
        },
        duration_minutes = {
            type = "number",
            min = 1,
            max = 480
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
    default = "value",         -- Default value

    -- String constraints
    enum = { "a", "b", "c" },  -- Allowed values
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

### Validating Notes

Use the `mdv validate` command to validate notes:

```bash
# Validate all notes in the vault
mdv validate

# Validate only notes of a specific type
mdv validate --type task

# Show available type definitions
mdv validate --list-types

# JSON output for scripting
mdv validate --json
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

### Error Handling

All vault operations return two values for graceful error handling:

| Function | Success Return | Failure Return |
|----------|----------------|----------------|
| `mdv.template()` | `(content, nil)` | `(nil, error_message)` |
| `mdv.capture()` | `(true, nil)` | `(false, error_message)` |
| `mdv.macro()` | `(true, nil)` | `(false, error_message)` |

Hooks should check for errors but failures are non-fatal—the CLI logs a warning but the note creation still succeeds.

### Complete Hook Example

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
            text = string.format("- Created task: [[%s]] (%s)",
                note.path,
                note.frontmatter.title or "untitled")
        })

        if not ok then
            print("Warning: could not log to daily: " .. err)
        end

        -- Optionally run a macro for additional setup
        mdv.macro("setup-task-reminders", {
            task_path = note.path,
            due = note.frontmatter.due
        })

        return note
    end
}
```

## Future: Extended Vault Context

Additional bindings are planned for future releases:

```lua
-- Planned API
local note = mdv.current_note()
local backlinks = mdv.backlinks(note.path)
local tasks = mdv.query({ type = "task", status = "open" })
```

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
