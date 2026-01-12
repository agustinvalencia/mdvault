# Captures → Lua Migration Plan

> **Status**: Complete
> **Parent**: [PLAN-v0.2.0.md](./PLAN-v0.2.0.md) Phase 4.1

## Overview

Migrate captures from YAML DSL to Lua-based definitions, aligning with the Lua-first architecture completed for templates in v0.2.0.

**Implementation complete** - Lua captures are fully supported. YAML captures are deprecated and show a warning when loaded.

## Current State

### YAML DSL Structure

```yaml
name: inbox
description: Add to today's inbox

vars:
  text: "What to capture?"
  # OR full form:
  text:
    prompt: "What to capture?"
    default: ""
    required: true

target:
  file: "daily/{{date}}.md"
  section: "Inbox"
  position: begin  # or "end"
  create_if_missing: false

content: "- [ ] {{text}}"

frontmatter:
  # Simple form (implicit set):
  status: pending
  # OR explicit operations:
  - field: counter
    op: increment
```

### Key Components

| File | Purpose |
|------|---------|
| `crates/core/src/captures/types.rs` | CaptureSpec, CaptureTarget, etc. |
| `crates/core/src/captures/repository.rs` | Discovery and loading |
| `crates/cli/src/cmd/capture.rs` | Command execution |
| `crates/core/src/vars/mod.rs` | VarsMap, VarSpec (shared with templates) |

## Proposed Lua Format

```lua
-- captures/inbox.lua
return {
    name = "inbox",
    description = "Add to today's inbox",

    -- Variables with prompts (same format as type schemas)
    vars = {
        text = {
            type = "string",
            prompt = "What to capture?",
            required = true,
        },
        priority = {
            type = "string",
            enum = { "low", "medium", "high" },
            default = "medium",
        },
    },

    -- Target specification
    target = {
        file = "daily/{{date}}.md",
        section = "Inbox",
        position = "begin",  -- "begin" | "end"
        create_if_missing = false,
    },

    -- Content template (supports {{variable}} placeholders)
    content = "- [ ] {{text}} ({{priority}})",

    -- Frontmatter operations (optional)
    frontmatter = {
        -- Simple set operations
        status = "pending",
        -- Explicit operations
        { field = "count", op = "increment" },
        { field = "tags", op = "append", value = "{{priority}}" },
    },

    -- Lifecycle hooks (optional, new capability)
    before_insert = function(ctx)
        -- Modify ctx.content, ctx.frontmatter before insertion
        return ctx
    end,

    after_insert = function(note, ctx)
        -- Post-processing after content inserted
    end,
}
```

## Benefits of Lua Migration

1. **Consistency**: Same format as type definitions
2. **Power**: Lifecycle hooks for custom logic
3. **Computed defaults**: Use Lua functions for dynamic defaults
4. **Validation**: Leverage schema validation from type system
5. **Reuse**: Reference existing type schemas via `lua_type`

## Implementation Phases

### Phase 1: Core Infrastructure

**Goal**: Enable Lua capture loading alongside YAML (backward compatible)

1. Add `CaptureSpec::from_lua()` method using existing Lua engine
2. Update `CaptureRepository` to discover both `.yaml` and `.lua` files
3. Prefer `.lua` over `.yaml` if both exist (same name)
4. Add `CaptureSpec` validation for Lua-loaded specs

**Files to modify**:
- `crates/core/src/captures/types.rs` - Add from_lua constructor
- `crates/core/src/captures/repository.rs` - Update discovery
- `crates/core/src/captures/mod.rs` - Re-export Lua loading

### Phase 2: Lua Loading Implementation

**Goal**: Full Lua support with schema-based vars

1. Create `captures/lua_loader.rs` module
2. Parse Lua table into CaptureSpec fields
3. Support both simple vars (`text = "prompt"`) and full form
4. Map Lua `vars` to existing VarsMap/VarSpec types

**Type mapping**:
```
Lua                     → Rust
---------------------------------------------
vars.text = "prompt"    → VarSpec::Simple(String)
vars.text = { ... }     → VarSpec::Full(VarMetadata)
target.position = "end" → CapturePosition::End
frontmatter = { ... }   → FrontmatterOps
```

### Phase 3: Lifecycle Hooks

**Goal**: Add optional before/after hooks

1. Add optional `before_insert` and `after_insert` to CaptureSpec
2. Execute hooks via existing HookRunner infrastructure
3. Pass context (variables, target, content) to hooks
4. Allow hooks to modify content/frontmatter

**Hook context**:
```rust
pub struct CaptureHookContext {
    pub variables: HashMap<String, String>,
    pub content: String,
    pub frontmatter: Option<FrontmatterOps>,
    pub target_path: PathBuf,
}
```

### Phase 4: Migration & Deprecation

**Goal**: Migrate examples, deprecate YAML

1. Convert all example captures to Lua
2. Add deprecation warning for YAML captures
3. Update documentation
4. Plan YAML removal for v0.3.0

## File Structure After Migration

```
.mdvault/
├── captures/
│   ├── inbox.lua       # Lua-based capture
│   ├── todo.lua
│   └── quick-note.lua
├── types/
│   └── ...             # Existing type definitions
└── templates/
    └── ...             # Existing templates
```

## Test Plan

### Unit Tests

| Test | Description |
|------|-------------|
| `lua_capture_basic` | Load simple Lua capture, verify fields |
| `lua_capture_vars` | Both simple and full var forms |
| `lua_capture_frontmatter` | All operation types |
| `lua_capture_hooks` | before/after hook execution |
| `lua_capture_invalid` | Error handling for bad Lua |

### Integration Tests

| Test | Description |
|------|-------------|
| `capture_lua_insert` | Full capture flow with Lua spec |
| `capture_lua_create_file` | With create_if_missing |
| `capture_lua_hooks` | Hooks modify content |
| `capture_yaml_still_works` | Backward compatibility |

## Breaking Changes

**v0.3.0** (future):
- YAML captures removed
- Migration: Convert `.yaml` to `.lua` format

**v0.2.x** (this release):
- No breaking changes (YAML still supported)
- Deprecation warning when loading YAML captures

## Open Questions

1. Should captures support referencing type schemas for vars?
   - `lua_type = "task"` to inherit task schema for vars
   - Decision: **Yes** - enables reuse

2. Should we support inline Lua in content templates?
   - `content = "- [ ] " .. ctx.text:upper()`
   - Decision: **No** - keep content as mustache templates

3. Hook return values:
   - Should `before_insert` be able to cancel insertion?
   - Decision: **Yes** - return `nil` or `false` to abort

## Implementation Order

1. ~~Create plan document~~ (this file)
2. ~~Add Lua loading to CaptureSpec~~ (`crates/core/src/captures/lua_loader.rs`)
3. ~~Update CaptureRepository discovery~~ (`crates/core/src/captures/discovery.rs`)
4. ~~Add lifecycle hook support~~ (deferred to future iteration)
5. ~~Migrate examples~~ (`examples/.markadd/captures/*.lua`)
6. ~~Update documentation~~ (`docs/lua-scripting.md`)
7. ~~Add deprecation warning for YAML~~

## Success Criteria

- [x] All existing capture tests pass
- [x] New Lua capture tests pass (`crates/cli/tests/capture_lua.rs`)
- [x] Example captures work in both formats
- [x] Documentation updated (`docs/lua-scripting.md`)
- [x] Deprecation warning shows for YAML captures
