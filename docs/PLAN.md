# mdvault Development Plan

This document tracks the implementation phases for evolving mdvault from a templating tool into a comprehensive vault management system.

## Current State (v0.1.0)

**Core Features:**
- Template rendering with variable substitution and filters (`slugify`, `upper`, `lower`, `trim`)
- Type-aware note scaffolding (`mdv new task "Title"`)
- Capture workflows for appending to notes
- Macro system for multi-step automation
- Date math expressions (`today + 7d`, `monday`, `2025-01-15 + 7d`, `2025-W03`, `week_start`)
- TUI for interactive vault browsing

**Index & Search:**
- SQLite-based index with notes, links, and derived tables
- Incremental reindexing with content hash change detection
- Contextual search with graph neighbourhood, temporal context, and cooccurrence
- Staleness detection for finding neglected notes
- Backlinks, outlinks, and orphan note discovery

**Validation & Types:**
- Lua-based type definition system
- Schema validation (required fields, enums, constraints)
- Custom validation functions in Lua
- Lifecycle hooks (on_create, on_update)
- Auto-fix for safe issues (missing defaults, enum case normalization)
- Link integrity checking

**MCP Integration:**
- Basic MCP server with note browsing (Phase 5 pending full implementation)

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
- [x] Vault context bindings
  - [x] Expose `mdv.current_note()`
  - [x] Expose `mdv.backlinks()`, `mdv.outlinks()`
  - [x] Expose `mdv.query()` for index queries

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
  - [x] Link integrity (target exists) - via `--check-links` flag
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
  - [x] Call on_update() during capture operations

### Phase 3: Search and Retrieval

**Goal**: Implement contextual search beyond keyword matching.

- [x] Derived indices
  - [x] `activity_summary` (last_seen, access counts, staleness score)
  - [x] `note_cooccurrence` (notes appearing together in dailies)
  - [ ] `context_paths` (traversal paths between notes)
- [x] Search modes
  - [x] Direct match (notes matching query)
  - [x] Graph neighbourhood (linked notes within N hops)
  - [x] Temporal context (recent dailies referencing matches)
  - [x] Cooccurrence (notes that appeared together)
- [x] Temporal weighting
  - [x] Favour recently active notes (`--boost` flag)
  - [x] Track access patterns (activity_summary table)
  - [ ] Detect activity clusters
- [ ] Type-specific behaviours
  - [ ] Tasks: parse TODO syntax, filter by status/project
  - [ ] Projects: aggregate task completion
  - [ ] Zettels: content search priority
  - [ ] Dailies: extract work summaries

### Phase 4: Rename and Reference Management

**Goal**: Safe note renaming with reference updates.

- [x] Reference detection
  - [x] Wikilinks: `[[note]]`, `[[path/note]]`, `[[note|alias]]`, `[[note#section]]`
  - [x] Markdown links: `[text](path/note.md)`, `[text](../relative/note.md)`
  - [x] Frontmatter references: `project: note-name`, `related: [note1, note2]`
- [x] Format-preserving updates
  - [x] Maintain original link style
  - [x] Preserve aliases and section anchors
  - [x] Handle relative paths correctly
- [x] Rename workflow
  - [x] Preview all changes (`--dry-run`)
  - [x] Atomic file + index updates
  - [x] Confirmation prompt (skip with `--yes`)
- [x] Edge cases
  - [x] Case-insensitive matching
  - [x] Ambiguous reference warnings

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

## CLI Commands

### Currently Implemented

#### Note Creation
```bash
mdv new task "Implement feature X" --var project=myproject
mdv new project "My Project" --var status=active
mdv new zettel "Research notes" --var tags=ml,research
mdv new --template daily                    # Use template instead of type
mdv new task "Title" -o custom/path.md      # Custom output path
```

#### Capture & Macros
```bash
mdv capture --list                          # List available captures
mdv capture inbox --var text="Quick note"   # Append to inbox
mdv macro --list                            # List available macros
mdv macro weekly-review                     # Run a macro workflow
```

#### Index & Query
```bash
mdv reindex                                 # Incremental reindex
mdv reindex --force                         # Full rebuild
mdv list                                    # List all notes
mdv list --type task                        # Filter by type
mdv list --modified-after "today - 7d"      # Date filtering
mdv list --json                             # JSON output
mdv links notes/my-note.md                  # Show all links
mdv links notes/my-note.md --backlinks      # Only backlinks
mdv orphans                                 # Find orphan notes
```

#### Search
```bash
mdv search "query"                          # Direct text search
mdv search "query" --mode full              # Full contextual search
mdv search "query" --mode neighbourhood     # Include linked notes
mdv search "query" --mode temporal          # Include referencing dailies
mdv search "query" --type task --boost      # Type filter + temporal boost
```

#### Staleness Detection
```bash
mdv stale                                   # Notes with staleness > 0.5
mdv stale --threshold 0.7                   # Higher threshold
mdv stale --days 90                         # Not seen in 90 days
mdv stale --type task                       # Only stale tasks
```

#### Validation
```bash
mdv validate                                # Validate all notes
mdv validate path/to/note.md                # Validate specific file
mdv validate --type task                    # Validate only tasks
mdv validate --fix                          # Auto-fix safe issues
mdv validate --check-links                  # Include link integrity
mdv validate --list-types                   # Show type definitions
```

#### Rename & Reference Management
```bash
mdv rename old.md new.md              # Rename note and update all references
mdv rename old.md new.md --dry-run    # Preview changes without modifying files
mdv rename old.md new.md --yes        # Skip confirmation prompt
```

#### Utility
```bash
mdv doctor                                  # Check configuration
mdv list-templates                          # Show available templates
mdv                                         # Launch TUI (no subcommand)
```

### Planned (Not Yet Implemented)

#### Workflow Commands
```bash
mdv workon MyProject            # Open project, show tasks, log session
mdv done tasks/impl.md "Done"   # Complete task, update daily
mdv today                       # Show due tasks, recent activity
mdv review                      # Interactive triage
mdv weekly-planning             # Create/open weekly note
```

#### Advanced Search
```bash
mdv timeline "Project" --since "2 weeks ago"
mdv related-to notes/mcp.md --depth 2
mdv stuck                       # Tasks in-progress >14 days
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
