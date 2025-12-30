# mdvault Development Plan

This document tracks the implementation phases for evolving mdvault from a templating tool into a comprehensive vault management system.

## Current State

The existing codebase provides:
- Template rendering with variable substitution
- Capture workflows for appending to notes
- Macro system for multi-step automation
- Date math expressions
- TUI for interactive use
- Basic MCP server with note browsing

## Implementation Phases

### Phase 1: Core Infrastructure

**Goal**: Build the foundation for indexing and querying.

- [x] SQLite database schema
  - [x] `notes` table (id, path, type, created, modified, title, frontmatter JSON, content_hash)
  - [x] `links` table (source_id, target_id, link_text, link_type, context)
  - [x] `temporal_activity` table (note_id, daily_id, activity_date, context)
  - [x] `activity_summary` and `note_cooccurrence` tables (for Phase 3)
  - [x] Schema versioning and migration support
- [x] Index builder
  - [x] Walk vault and parse markdown files
  - [x] Extract frontmatter metadata
  - [x] Parse wikilinks and markdown links
  - [x] Compute content hashes for change detection
- [x] Incremental updates
  - [x] Detect changed files via content hash
  - [x] Partial reindex on file changes (--force for full rebuild)
- [x] Basic queries (database layer)
  - [x] Find notes by type
  - [x] Find links to/from a note (backlinks, outlinks)
  - [x] List notes modified in date range
  - [x] Find orphan notes
- [x] Basic queries (CLI commands)
  - [x] `mdv list` command with filters (--type, --modified-after, --modified-before, --limit)
  - [x] `mdv links` command for backlinks/outlinks (--backlinks, --outlinks)
  - [x] `mdv orphans` command
  - [x] Output formats: --json, --quiet, --output table|json|quiet

### Phase 1.5: Lua Scripting Layer

**Goal**: Enable user-configurable type system and validation via Lua.

- [x] Core Lua runtime
  - [x] Add mlua dependency (Lua 5.4, vendored, sandboxed)
  - [x] Create `scripting` module with LuaEngine
  - [x] Expose `mdv.date(expr, format?)` for date math
  - [x] Expose `mdv.render(template, context)` for template rendering
  - [x] Expose `mdv.is_date_expr(str)` for type checking
  - [x] Sandbox: remove io, os, require, load, debug
- [x] Type definitions in Lua
  - [x] Load type definitions from `~/.config/mdvault/types/*.lua`
  - [x] Parse type schema (required fields, types, enums, constraints)
  - [x] Custom validate() function hooks
  - [x] Lifecycle hooks (on_create, on_update) - stored, ready for integration
  - [x] TypeRegistry for built-in + custom types
  - [x] `mdv validate` CLI command
- [ ] Vault context bindings
  - [ ] Expose `mdv.current_note()`
  - [ ] Expose `mdv.backlinks()`, `mdv.outlinks()`
  - [ ] Expose `mdv.query()` for index queries

### Phase 2: Structure Enforcement

**Goal**: Implement note types and validation.

- [x] Note type system (Lua-driven)
  - [x] Parse `type:` from frontmatter
  - [x] Define required fields per type (via Lua schema)
  - [x] Validate enum values via schema constraints
  - [x] Custom types extend built-in types (daily, weekly, task, project, zettel)
- [x] Validation rules
  - [x] Required field checking
  - [x] Type constraints (string, number, boolean, date, datetime, list, reference)
  - [x] Enum constraints
  - [x] Number range (min/max)
  - [x] String length and pattern (regex)
  - [x] List item count (min/max)
  - [x] Custom validate() hooks in Lua
  - [ ] Link integrity (target exists)
- [x] Linting system
  - [x] `mdv lint` — alias for validate
  - [x] `mdv validate --fix` — auto-fix safe issues (missing defaults, enum case)
  - [x] Per-file validation (`mdv validate path/to/note.md`)
  - [x] Severity levels (errors vs warnings in output)
- [x] Note scaffolding
  - [x] Type-aware creation (`mdv new task "Title"`)
  - [x] Auto-populate required fields from schema defaults
  - [x] Auto-generate output paths (`tasks/my-title.md`)
  - [x] Template filters (`{{title | slugify}}`)
  - [ ] Link to daily note on creation (via on_create hook)
- [x] Hook integration
  - [x] Call on_create() during `mdv new`
  - [x] Vault operations in hooks (`mdv.template()`, `mdv.capture()`, `mdv.macro()`)
  - [ ] Call on_update() during capture operations

### Phase 3: Search and Retrieval

**Goal**: Implement contextual search beyond keyword matching.

- [ ] Derived indices
  - [ ] `activity_summary` (last_seen, access counts, staleness score)
  - [ ] `note_cooccurrence` (notes appearing together in dailies)
  - [ ] `context_paths` (traversal paths between notes)
- [ ] Search modes
  - [ ] Direct match (notes matching query)
  - [ ] Graph neighbourhood (linked notes within N hops)
  - [ ] Temporal context (recent dailies referencing matches)
  - [ ] Cooccurrence (notes that appeared together)
- [ ] Temporal weighting
  - [ ] Favour recently active notes
  - [ ] Track access patterns
  - [ ] Detect activity clusters
- [ ] Type-specific behaviours
  - [ ] Tasks: parse TODO syntax, filter by status/project
  - [ ] Projects: aggregate task completion
  - [ ] Zettels: content search priority
  - [ ] Dailies: extract work summaries

### Phase 4: Rename and Reference Management

**Goal**: Safe note renaming with reference updates.

- [ ] Reference detection
  - [ ] Wikilinks: `[[note]]`, `[[path/note]]`, `[[note|alias]]`
  - [ ] Markdown links: `[text](path/note.md)`
  - [ ] Frontmatter references: `project: note-name`
- [ ] Format-preserving updates
  - [ ] Maintain original link style
  - [ ] Preserve aliases
  - [ ] Handle relative paths correctly
- [ ] Rename workflow
  - [ ] Preview all changes
  - [ ] Atomic file + index updates
  - [ ] Dry-run mode
- [ ] Edge cases
  - [ ] Ambiguous references
  - [ ] Case-sensitivity
  - [ ] Broken links reporting

### Phase 5: MCP Integration

**Goal**: Enable AI-assisted vault interaction.

- [ ] Task management tools
  - [ ] `search_tasks` (by project, status, date)
  - [ ] `create_task` (with scaffolding, daily linking)
  - [ ] `update_task_status`
  - [ ] `complete_task` (mark done + log to daily)
- [ ] Project management tools
  - [ ] `create_project`
  - [ ] `get_project_context` (tasks, activity, related zettels)
  - [ ] `list_projects`
- [ ] Knowledge retrieval tools
  - [ ] `find_zettels` (query, tags, related project)
  - [ ] `get_working_context` (auto-expanding context)
- [ ] Daily integration tools
  - [ ] `get_daily_summary`
  - [ ] `add_to_daily`
  - [ ] `get_work_timeline`
- [ ] Maintenance tools
  - [ ] `detect_stalled_work`
  - [ ] `suggest_connections`
  - [ ] `find_dropped_threads`
  - [ ] `get_vault_health`

### Phase 6: Advanced Features (Future)

- [ ] Semantic search via embeddings
- [ ] Trend detection over time
- [ ] Knowledge graph visualisation
- [ ] Multi-vault support
- [ ] Collaborative features

## CLI Commands (Target State)

### Creation Commands

```bash
mdv task new "Implement feature X" --project MyProject
mdv project new "OtherProject" --status planning
mdv zettel new "Quantum AI notes" --tags ai,research
mdv quick "Investigate Mamba for RAN"
```

### Workflow Commands

```bash
mdv workon MyProject            # Open project, show tasks, log session
mdv done tasks/impl.md "Done"   # Complete task, update daily
mdv today                       # Show due tasks, recent activity
mdv review                      # Interactive triage
mdv weekly-planning             # Create/open weekly note
mdv rename notte.md note.md     # Rename and reindex notes
```

### Search Commands

```bash
mdv find "an interesting topic" --context full
mdv timeline "MyProject" --since "2 weeks ago"
mdv related-to notes/mcp.md --depth 2
mdv stale --threshold 30d
mdv orphans
mdv stuck
```

### Maintenance Commands

```bash
mdv validate                    # Validate notes against type definitions
mdv validate --type task        # Validate only task notes
mdv validate --list-types       # Show available type definitions
mdv lint
mdv lint --fix
mdv reindex
mdv health
```

## Success Criteria

**CLI Performance**:
- Common commands complete in <100ms
- Intuitive commands requiring minimal memorisation
- Helpful error messages with suggestions
- Non-destructive by default (confirmations, dry-runs)

**MCP Effectiveness**:
- AI can create properly structured notes without explicit instructions
- Context retrieval feels automatic and relevant
- Maintenance prompts are actionable and non-intrusive

**User Experience**:
- Vault remains organised without constant manual effort
- Finding information is quick and reliable
- Structure enforcement helps rather than hinders

## Open Questions

1. **Semantic search priority**: How important is semantic similarity vs graph/temporal signals?
2. **Real-time sync**: Should mdvault watch the filesystem or assume a static vault?
3. **Archive strategy**: How to handle completed/old notes?
4. **Multi-vault**: Single tool for multiple vaults or one instance per vault?
