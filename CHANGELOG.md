# Changelog

All notable changes to mdvault will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Lua-based captures**: Captures can now be defined in Lua (`.lua` files) in addition to YAML
  - Lua captures use the same format as type definitions with `vars`, `target`, `content`, and `frontmatter`
  - Lua files take precedence over YAML files with the same name
  - YAML captures are now deprecated and show a warning when loaded
  - See `docs/lua-scripting.md` for the new capture format

### Deprecated

- **YAML captures**: YAML capture definitions are deprecated in favor of Lua
  - Migration guide available in `docs/lua-scripting.md`
  - YAML support will be removed in a future version

## [0.2.0] - 2025-01-12

### Changed

- **Architecture**: Replaced scattered type checks with trait-based polymorphic dispatch
  - New `domain` module in `mdvault-core` with `NoteType` enum and `NoteBehavior` traits
  - Behaviors implemented for Task, Project, Daily, Weekly, Zettel, and Custom types
  - `NoteCreator` provides unified creation flow with lifecycle hooks
  - Reduced `new.rs` by ~700 lines through better abstraction

### Removed

- **BREAKING**: Removed `vars:` DSL from template frontmatter
  - Templates should now use `lua:` field to reference Lua type definitions for variable metadata (prompts, defaults)
  - Variable substitution (`{{var}}` placeholders) still works via `--var` flags
  - Captures and macros still support `vars:` (unchanged)

### Migration Guide

**Before (v0.1.x):**
```yaml
---
output: "tasks/{{title | slugify}}.md"
vars:
  title:
    prompt: "Task title"
    required: true
---
# {{title}}
```

**After (v0.2.0):**

1. Create a Lua type definition in `~/.config/mdvault/types/`:
```lua
-- types/mytask.lua
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

2. Reference it from your template:
```yaml
---
lua: mytask.lua
---
# {{title}}
```

## [0.1.2] - 2025-01-11

### Added

- `inherited` flag for schema fields set by `on_create` hooks
- Pre-filled editable prompts for default values

### Fixed

- Prevent ephemeral variables from polluting frontmatter in hooks

## [0.1.1] - 2025-01-10

### Added

- Lua-first architecture phases 1-4 complete
- Template-Lua linking via `lua:` frontmatter field
- Schema-driven prompts from Lua type definitions
- Validation integration with `mdv validate`
- Hook execution (`on_create`) integrated with `mdv new`

## [0.1.0] - 2025-01-01

### Added

- Initial release
- Template rendering with variable substitution and filters
- Type-aware note scaffolding (`mdv new task "Title"`)
- Capture workflows for appending to notes
- Macro system for multi-step automation
- Date math expressions (`today + 7d`, `monday`, etc.)
- TUI for interactive vault browsing
- SQLite-based index with incremental reindexing
- Contextual search with graph neighbourhood
