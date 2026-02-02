# Changelog

All notable changes to mdvault will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - v0.3.0

### Removed

- **BREAKING**: Removed deprecated YAML support for captures and macros
  - YAML capture (`.yaml`) files are no longer recognized
  - YAML macro (`.yaml`) files are no longer recognized
  - All captures and macros must now be defined in Lua (`.lua`) format
  - Migration: Convert YAML files to Lua format (see examples in `docs/lua-scripting.md`)

### Changed

- Documentation updated to remove YAML migration sections (YAML is no longer supported)

## [0.2.5] - 2026-02-01

### Added

- **Progress tracking**: `mdv project progress [PROJECT]` shows completion percentage, task breakdown by status, and velocity metrics
  - Progress bars and percentages for all projects
  - Rolling 4-week velocity calculation

- **Activity reporting**: `mdv report --month YYYY-MM` or `--week YYYY-Wxx` generates activity summaries
  - Tasks completed/created counts
  - Activity heatmap visualization
  - Markdown output option (`--output file.md`)

- **Daily planning dashboard**: `mdv today` shows daily context at a glance
  - Open tasks by status (in-progress, due today, blocked)
  - Yesterday's completions
  - Suggested focus based on priorities

- **Focus mode**: `mdv focus PROJECT` sets active project context
  - New tasks automatically use focused project (no prompt)
  - Override with `--var project=OTHER`
  - Persistent across sessions (stored in `.mdvault/state/context.toml`)
  - `mdv focus --clear` to remove focus
  - `mdv focus --json` for integrations

- **Context commands**: Rich context queries for AI/MCP integration
  - `mdv context day [DATE]` - Activity for a specific day
  - `mdv context week [WEEK]` - Activity for a specific week
  - `mdv context note PATH` - Full context for a note (metadata, sections, activity)
  - `mdv context focus` - Current focus project with task counts

- **Activity logging infrastructure**: Automatic logging of task/project creation and completion to daily notes

- **Interactive Lua selectors**: Schema fields with `enum` show interactive selector menus

- **Excluded folders**: `excluded_folders` config option to filter vault operations

- **Future journal notes**: Create daily/weekly notes for any date using date expressions
  - `mdv new daily "today + 1d"` creates tomorrow's daily
  - `mdv new weekly "today + 1w"` creates next week's weekly

### Fixed

- Template variables in frontmatter now properly substitute (e.g., `{{title}}` in output paths)
- Boolean values (`true`/`false`) no longer incorrectly quoted as strings in YAML frontmatter
- Context variables now take precedence over date expression keywords in templates
- Date expressions in note titles now correctly evaluate for headings and output paths
- Schema fields with `prompt` attribute now properly prompt in template mode
- Template variables from Lua typedef `variables` section now collected correctly
- Schema defaults now used for title instead of prompting when available
- Lua output templates and schema prompts now respected in all creation flows

## [0.2.1] - 2025-01-15

### Added

- **Lua-based captures**: Captures can now be defined in Lua (`.lua` files)
  - Lua captures use the same format as type definitions with `vars`, `target`, `content`, and `frontmatter`
  - See `docs/lua-scripting.md` for the capture format

- **Lua-based macros**: Macros can now be defined in Lua (`.lua` files)
  - Supports all step types: template, capture, shell
  - Simplified syntax allows omitting `type` field for common cases
  - Supports `on_error` policy (abort/continue)
  - See `docs/lua-scripting.md` for the macro format

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
