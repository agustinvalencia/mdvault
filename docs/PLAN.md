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

- [ ] SQLite database schema
  - [ ] `notes` table (id, path, type, created, modified, title, frontmatter JSON, content_hash)
  - [ ] `links` table (source_id, target_id, link_text, link_type, context)
  - [ ] `temporal_activity` table (note_id, daily_id, activity_date, context)
- [ ] Index builder
  - [ ] Walk vault and parse markdown files
  - [ ] Extract frontmatter metadata
  - [ ] Parse wikilinks and markdown links
  - [ ] Compute content hashes for change detection
- [ ] Incremental updates
  - [ ] Detect changed files via mtime/hash
  - [ ] Partial reindex on file changes
- [ ] Basic queries
  - [ ] Find notes by type
  - [ ] Find links to/from a note
  - [ ] List notes modified in date range

### Phase 2: Structure Enforcement

**Goal**: Implement note types and validation.

- [ ] Note type system
  - [ ] Parse `type:` from frontmatter
  - [ ] Define required fields per type
  - [ ] Validate enum values (task status, project status)
- [ ] Validation rules
  - [ ] Required field checking
  - [ ] Link integrity (target exists)
  - [ ] Type-specific constraints
- [ ] Linting system
  - [ ] `mdv lint` — report issues
  - [ ] `mdv lint --fix` — auto-fix safe issues
  - [ ] Severity levels (error, warning, info)
- [ ] Note scaffolding
  - [ ] Type-aware templates
  - [ ] Auto-populate required fields
  - [ ] Link to daily note on creation

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
