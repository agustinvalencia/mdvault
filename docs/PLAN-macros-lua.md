# Macros → Lua Migration Plan

> **Parent**: [PLAN-v0.2.0.md](./PLAN-v0.2.0.md) Phase 4.2

## Overview

Migrate macros from YAML DSL to Lua-based definitions, following the same pattern established for captures in Phase 4.1.

## Current State

### YAML DSL Structure

```yaml
name: weekly-review
description: Set up weekly review documents

vars:
  focus:
    prompt: "What's your focus this week?"
    default: ""
  week_of:
    prompt: "Week date"
    default: "{{today}}"

steps:
  - template: weekly-summary
    output: "summaries/{{week_of}}.md"
    with:
      title: "Week of {{week_of}}"
  - capture: archive-tasks
  - shell: "git add ."
    description: "Stage changes"

on_error: abort  # or "continue"
```

### Key Components

| File | Purpose |
|------|---------|
| `crates/core/src/macros/types.rs` | MacroSpec, MacroStep, StepResult, etc. |
| `crates/core/src/macros/discovery.rs` | Discovery and loading |
| `crates/core/src/macros/runner.rs` | Execution logic, StepExecutor trait |
| `crates/cli/src/cmd/macro_cmd.rs` | CLI command |

## Proposed Lua Format

```lua
-- macros/weekly-review.lua
return {
    name = "weekly-review",
    description = "Set up weekly review documents",

    vars = {
        focus = {
            prompt = "What's your focus this week?",
            default = "",
        },
        week_of = {
            prompt = "Week date",
            default = "{{today}}",
        },
    },

    steps = {
        {
            type = "template",
            template = "weekly-summary",
            output = "summaries/{{week_of}}.md",
            with = {
                title = "Week of {{week_of}}",
            },
        },
        {
            type = "capture",
            capture = "archive-tasks",
        },
        {
            type = "shell",
            shell = "git add .",
            description = "Stage changes",
        },
    },

    on_error = "abort",  -- "abort" (default) or "continue"
}
```

## Key Differences from YAML

1. **Explicit step type**: Each step has a `type` field instead of relying on untagged enum detection
2. **Consistent structure**: Follows the same pattern as captures and type definitions
3. **Future extensibility**: Can add lifecycle hooks (`on_start`, `on_error`, `on_complete`)

## Implementation Phases

### Phase 1: Core Infrastructure

**Goal**: Enable Lua macro loading alongside YAML (backward compatible)

1. Add `MacroFormat` enum to track source format
2. Update `MacroInfo` with format field
3. Create `lua_loader.rs` module
4. Update `MacroRepository` for dual format discovery

**Files to modify**:
- `crates/core/src/macros/types.rs` - Add MacroFormat, update MacroInfo
- `crates/core/src/macros/discovery.rs` - Discover both .lua and .yaml
- `crates/core/src/macros/mod.rs` - Re-export lua_loader

**New files**:
- `crates/core/src/macros/lua_loader.rs` - Lua loading implementation

### Phase 2: Lua Loading Implementation

**Goal**: Full Lua support with step parsing

1. Parse Lua table into MacroSpec
2. Convert step tables to MacroStep variants
3. Support both simple and full vars format
4. Handle on_error policy

**Step type mapping**:
```
Lua step.type    → Rust MacroStep
--------------------------------------
"template"       → MacroStep::Template(TemplateStep)
"capture"        → MacroStep::Capture(CaptureStep)
"shell"          → MacroStep::Shell(ShellStep)
```

### Phase 3: Migration & Deprecation

**Goal**: Migrate examples, deprecate YAML

1. Convert example macros to Lua
2. Add deprecation warning for YAML macros
3. Update documentation
4. Plan YAML removal for v0.3.0

## File Structure After Migration

```
.mdvault/
├── macros/
│   ├── weekly-review.lua   # Lua-based macro
│   ├── daily-setup.lua
│   └── deploy.lua
├── captures/
│   └── ...
├── types/
│   └── ...
└── templates/
    └── ...
```

## Test Plan

### Unit Tests

| Test | Description |
|------|-------------|
| `lua_macro_basic` | Load simple Lua macro, verify fields |
| `lua_macro_vars` | Both simple and full var forms |
| `lua_macro_template_step` | Template step with output and with |
| `lua_macro_capture_step` | Capture step with vars |
| `lua_macro_shell_step` | Shell step with description |
| `lua_macro_multiple_steps` | Multiple steps in sequence |
| `lua_macro_on_error` | Error policy parsing |
| `lua_macro_invalid` | Error handling for bad Lua |

### Integration Tests

| Test | Description |
|------|-------------|
| `macro_lua_template_step` | Execute Lua macro with template |
| `macro_lua_capture_step` | Execute Lua macro with capture |
| `macro_lua_shell_step` | Execute Lua macro with shell (trusted) |
| `macro_lua_multiple_steps` | Full multi-step macro execution |
| `macro_yaml_still_works` | Backward compatibility |

## Breaking Changes

**v0.3.0** (future):
- YAML macros removed
- Migration: Convert `.yaml` to `.lua` format

**v0.2.x** (this release):
- No breaking changes (YAML still supported)
- Deprecation warning when loading YAML macros

## Implementation Order

1. ~~Create plan document~~ (this file)
2. Add MacroFormat enum and update types
3. Create lua_loader module
4. Update discovery for both formats
5. Update repository loading
6. Add unit tests
7. Add integration tests
8. Migrate examples
9. Update documentation
10. Add deprecation warning for YAML

## Success Criteria

- [ ] All existing macro tests pass
- [ ] New Lua macro tests pass
- [ ] Example macros work in both formats
- [ ] Documentation updated
- [ ] Deprecation warning shows for YAML macros
