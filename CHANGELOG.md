# Changelog

All notable changes to mdvault will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.5] - 2026-02-20

### Added

- **Project archiving**: `mdv project archive <project>` moves completed projects to `Projects/_archive/`
  - Only projects with `status: done` can be archived
  - Remaining open tasks are automatically cancelled
  - Project frontmatter updated with `status: archived` and `archived_at` timestamp
  - Focus is cleared if the archived project was focused
  - All wikilinks and index entries updated to new paths
  - Logged to both daily and project notes
  - `--yes` flag to skip confirmation prompt

- **Archive-aware path matching**: All task/project lookups now search both `Projects/` and `Projects/_archive/`
  - `find_project_file`, `extract_project_from_path`, task listing, and context queries all handle archived paths

- **Task creation guard**: Cannot create tasks in archived projects — returns a clear error message

- **Year-based journal paths**: Daily notes now stored in `Journal/{year}/Daily/`, meetings in `Meetings/{year}/`

### Changed

- Task completion and cancellation now automatically log to the daily note

## [0.3.0] - 2026-02-03

This release completes the Lua-first architecture with a breaking change: YAML capture/macro files are no longer supported. All automation must now use Lua. In return, you get powerful lifecycle hooks, a new built-in meeting type, and comprehensive activity tracking.

### Highlights

- **Lua-only captures and macros**: Clean, consistent scripting
- **Built-in meeting type**: First-class meeting note management
- **Capture hooks**: Transform content before/after insertion
- **Focus mode**: Set active project for frictionless task creation
- **Activity tracking**: Daily dashboard, progress reports, context queries

### Added

- **New built-in "meeting" note type**: First-class support for meeting notes
  - Auto-generated IDs: `MTG-2025-01-15-001` (date-based with counter)
  - Prompts for date (defaults to today) and attendees
  - Output path: `Meetings/{meeting-id}.md`
  - Logged to daily note on creation
  - Usage: `mdv new meeting "Team Sync" --var attendees="Alice, Bob"`

- **Capture lifecycle hooks**: Lua hooks for captures
  - `before_insert(content, vars, target)`: Modify content before insertion
  - `after_insert(content, vars, target, result)`: Run side effects after insertion
  - Hooks are optional and defined in capture Lua files

- **Progress tracking**: `mdv project progress [PROJECT]` shows completion percentage, task breakdown by status, and velocity metrics

- **Activity reporting**: `mdv report --month YYYY-MM` or `--week YYYY-Wxx` generates activity summaries with heatmaps

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
  - `mdv context note PATH` - Full context for a note
  - `mdv context focus` - Current focus project with task counts

- **Activity logging infrastructure**: Automatic logging of task/project creation to daily notes

- **Interactive Lua selectors**: Schema fields with `enum` show interactive selector menus

- **Excluded folders**: `excluded_folders` config option to filter vault operations

- **Future journal notes**: Create daily/weekly notes for any date
  - `mdv new daily "today + 1d"` creates tomorrow's daily
  - `mdv new weekly "today + 1w"` creates next week's weekly

- **Lua-based captures**: Captures defined in `.lua` files with full Lua power

- **Lua-based macros**: Macros defined in `.lua` files with simplified syntax

### Removed

- **BREAKING**: Removed deprecated YAML support for captures and macros
  - YAML capture (`.yaml`) files are no longer recognized
  - YAML macro (`.yaml`) files are no longer recognized
  - All captures and macros must now be defined in Lua (`.lua`) format
  - Migration: Convert YAML files to Lua format (see `docs/lua-scripting.md`)

### Changed

- **Architecture**: Replaced scattered type checks with trait-based polymorphic dispatch
  - New `domain` module with `NoteType` enum and `NoteBehavior` traits
  - Behaviors for Task, Project, Daily, Weekly, Meeting, Zettel, and Custom types
  - `NoteCreator` provides unified creation flow with lifecycle hooks

- Documentation fully updated for Lua-only captures and macros

### Fixed

- Template variables in frontmatter now properly substitute
- Boolean values no longer incorrectly quoted as strings in YAML frontmatter
- Context variables take precedence over date expressions in templates
- Date expressions in note titles correctly evaluate for headings and output paths
- Schema fields with `prompt` attribute properly prompt in template mode
- Template variables from Lua typedef `variables` section collected correctly
- Schema defaults used for title instead of prompting when available
- Lua output templates and schema prompts respected in all creation flows

### Migration Guide

**Converting YAML captures to Lua:**

Before (`inbox.yaml`):
```yaml
name: inbox
target:
  file: "inbox.md"
  section: "Inbox"
content: "- [ ] {{text}}"
vars:
  text:
    required: true
    prompt: "What to capture?"
```

After (`inbox.lua`):
```lua
return {
    name = "inbox",
    target = {
        file = "inbox.md",
        section = "Inbox",
    },
    content = "- [ ] {{text}}",
    vars = {
        text = {
            required = true,
            prompt = "What to capture?",
        },
    },
}
```

**Converting YAML macros to Lua:**

Before (`weekly-review.yaml`):
```yaml
name: weekly-review
steps:
  - type: template
    template: weekly
```

After (`weekly-review.lua`):
```lua
return {
    name = "weekly-review",
    steps = {
        { template = "weekly" },
    },
}
```

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
