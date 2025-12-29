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

## Future: Type Definitions

The Lua layer will be extended to support user-defined note types:

```lua
-- types/task.lua (planned)
return {
  name = "task",

  required_fields = { "status", "project" },

  fields = {
    status = {
      type = "enum",
      values = { "open", "in-progress", "blocked", "done" },
      default = "open"
    },
    project = {
      type = "wikilink",
      required = true
    }
  },

  validate = function(note)
    if note.frontmatter.status == "done" and not note.frontmatter.completed_date then
      return false, "Done tasks require completed_date"
    end
    return true
  end
}
```

## Future: Vault Context

Additional bindings will expose vault context:

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
