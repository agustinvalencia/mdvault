# mdvault: Architecture and Design Philosophy

## Project Overview

**mdvault** is a Rust-based CLI tool and MCP (Model Context Protocol) server for managing markdown vaults, specifically designed to support knowledge work whilst accommodating ADHD-friendly workflows. It evolved from "markadd" with an expanded scope beyond simple note addition to comprehensive vault management, search, and AI-assisted interaction.

## Core Design Philosophy

### Pull-Optimised, Not Push-Optimised

The architecture assumes:
- Notes will be created correctly (user excels at capture and classification during hyperfocus)
- Notes will go stale without active maintenance
- **Retrieval is the primary failure mode** (finding information when needed)
- Passive maintenance > active maintenance (automate checks rather than require manual review)

### Opinionated Structure Enforcement

mdvault enforces structure rather than maximising flexibility:
- Required frontmatter fields by note type
- Automatic scaffolding when creating notes
- Validation and correction workflows
- Enforced linking patterns (e.g., tasks must link to projects)

This provides accountability and structure for users who struggle with consistency.

### ADHD-Friendly Principles

1. **Reduce cognitive load**: Smart defaults everywhere, minimal required input
2. **Progressive capture**: Quick capture with structured cleanup later
3. **Automated maintenance**: Proactive detection of stale/broken/orphaned content
4. **Forgiveness in parsing**: Accept flexible formats, fuzzy matching, auto-correction
5. **Passive surfacing**: Don't wait for searches—proactively show relevant context

## Vault Structure and Topology

### Note Types (Flat Hierarchy)

Notes use an explicit `type:` field in frontmatter:

- **daily**: Daily notes with implicit/explicit dates, link to both tasks/projects and zettels
- **weekly**: Broad overview notes, link to that week's dailies
- **task**: Individual tasks with required `status` and `project` fields
- **project**: Collections of related tasks with `status` field
- **zettel**: Knowledge notes (Zettelkasten-style) with required tags
- **none**: Uncategorised notes awaiting triage

### Graph Topology

The vault exhibits specific clustering:
- **Daily notes act as temporal backbone**: They link to both task/project clusters and zettelkasten clusters
- **Task/project clusters**: Projects contain tasks, both link to dailies
- **Zettelkasten clusters**: Knowledge notes link to each other and to dailies
- **Weeklies provide overview**: Link to dailies within their week, not to individual tasks/zettels

This topology informs search and retrieval strategies—dailies are integration points for traversing the graph.

## Search and Indexing Architecture

### Multi-Dimensional Indexing Strategy

**Three-Layer Index System** (SQLite-based):

#### 1. Node Layer (Metadata)
```sql
notes:
  - id (primary key)
  - path
  - type (task|project|zettel|daily|weekly|none)
  - created, modified
  - title
  - frontmatter (JSON blob)
  - content_hash (for change detection)
```

#### 2. Edge Layer (Relationships)
```sql
links:
  - source_id, target_id
  - link_text (content within [[brackets]])
  - link_type (wikilink, markdown, frontmatter)
  - context (surrounding text)
```

#### 3. Derived Layer (Computed Properties)
```sql
temporal_activity:
  - note_id, daily_id
  - activity_date (when note was referenced in a daily)
  - context (how it was referenced)

activity_summary:
  - note_id
  - last_seen, access_count_30d, access_count_90d
  - staleness_score

note_cooccurrence:
  - note_a_id, note_b_id
  - shared_daily_count (how often they appear together)
  - most_recent_cooccurrence

context_paths:
  - start_note_id, end_note_id
  - path_type (e.g., "task->project->daily->zettel")
  - hop_count, path_strength
```

### Search Strategy: Contextual Retrieval Over Keyword Search

**Core principle**: Users remember context, not keywords.

Search queries should return:
1. Direct matches (notes matching query)
2. Graph neighbourhood (linked notes within N hops)
3. Temporal context (recent dailies referencing matches)
4. Cooccurrence matches (notes that appeared together in dailies)

**Temporal signals as first-class citizens**:
- Weight recent activity heavily
- Track access patterns (frequency in recent dailies)
- Detect activity clusters (related work in same time period)
- Surface stale content (not mentioned in dailies for N days)

### Hybrid Index Maintenance

**Indexing approach**:
- **Always-on lightweight index**: Metadata, links, frontmatter (fast rebuild)
- **On-demand content search**: Full-text via direct scan or temporary index (adequate for typical vault sizes)
- **Optimistic consistency**: Assume vault stable during operations, reindex on staleness detection

**When to rebuild**:
- File timestamp changes detected
- Explicit user command (`mdvault reindex`)
- After bulk operations (rename, batch updates)
- On staleness threshold (configurable)

### Type-Specific Search Behaviours

**Tasks**:
- Parse TODO syntax (`- [ ]`, `- [x]`)
- Filter by status, project, date ranges
- Surface stale tasks (in-progress >14 days, not in recent dailies)

**Projects**:
- Aggregate task completion rates
- Build activity timeline from daily mentions
- Identify related zettels via shared daily references

**Zettels**:
- Content search prioritised
- Rank by "influence" (frequency of daily references)
- Cluster by cooccurrence patterns

**Dailies/Weeklies**:
- Parse dates from filenames/frontmatter
- Extract work summaries (outgoing links by type)
- Identify themes and patterns over time

## Validation and Structure Enforcement

### Required Fields by Type

**Tasks**:
- `status`: enum [open, in-progress, blocked, done, cancelled]
- `project`: reference to project note
- Must be linked from at least one daily (within 30 days)

**Projects**:
- `status`: enum [planning, active, paused, completed, archived]
- `created_date`: timestamp
- Should have at least one linked task

**Zettels**:
- `tags`: array, minimum one tag

**Dailies**:
- `date`: ISO format or inferred from filename

**Weeklies**:
- `week_start_date`: ISO format
- Links to that week's dailies

### Validation System

```rust
struct ValidationRule {
    note_type: NoteType,
    required_fields: Vec<String>,
    field_validators: HashMap<String, FieldValidator>,
    link_requirements: LinkRequirements,
}

enum LinkRequirements {
    MustLinkTo(Vec<NoteType>),
    MustBeLinkedFrom(Vec<NoteType>),
    Both,
}
```

**Linting capabilities**:
- Detect missing required fields
- Validate enum values
- Check link integrity
- Identify orphaned notes (no daily links)
- Find stale projects/tasks
- Suggest types for untyped notes

**Auto-fix options**:
```bash
mdvault lint                    # Report issues
mdvault lint --fix              # Auto-fix safe issues
mdvault lint --interactive      # Guided fixing
```

## CLI Design

### Opinionated Commands (Not Generic CRUD)

**Creation Commands** (with scaffolding):
```bash
mdvault task new "Implement feature X" --project RAN-optimization
  # Creates task with proper frontmatter
  # Links to project
  # Adds entry to today's daily
  # Prompts for status if missing

mdvault project new "Network Slicing Research" --status planning
  # Creates project note with scaffolding
  # Generates task list section
  # Links to today's daily

mdvault zettel new "KAN architecture notes" --tags ml,kan,research
  # Creates zettel with tags
  # Optionally links to today's daily

mdvault quick "Investigate Mamba for RAN"
  # Quick capture: creates note with type: none
  # Adds to today's daily
  # Can triage later
```

**Workflow Commands**:
```bash
mdvault workon RAN-optimization
  # Opens project
  # Shows open tasks
  # Creates daily entry "Started work on [project]"
  # Displays recent activity and relevant zettels

mdvault done "tasks/implement-search.md" "Implemented basic indexing"
  # Marks task complete
  # Adds completion note to today's daily
  # Updates project status if appropriate

mdvault today
  # Shows tasks due/overdue
  # Lists recent project activity
  # Surfaces stale items needing attention
  # Suggests next actions

mdvault review
  # Interactive triage for untyped notes
  # Fix validation issues
  # Classify pending items

mdvault weekly-planning
  # Creates/opens current week's weekly note
  # Shows open tasks across projects
  # Prompts for weekly goals
```

**Search and Discovery**:
```bash
mdvault find "KAN distillation" --context full
  # Shows matches + neighbourhood + temporal activity

mdvault timeline "RAN optimization" --since "2 weeks ago"
  # Chronological view of all activity

mdvault related-to "notes/mcp-architecture.md" --depth 2
  # Graph traversal: related notes within N hops

mdvault stale --threshold 30d
  # Projects/tasks not touched in 30 days

mdvault orphans
  # Notes never linked from dailies

mdvault stuck
  # Tasks "in-progress" for >14 days
```

### Configuration

```toml
# ~/.config/mdvault/config.toml

[validation]
strict_mode = true
auto_link_to_daily = true
default_task_status = "open"
allowed_task_statuses = ["open", "in-progress", "blocked", "done", "cancelled"]
allowed_project_statuses = ["planning", "active", "paused", "completed", "archived"]

[workflows]
task_completion_requires_summary = true
stale_project_threshold_days = 14
auto_create_weekly = true
archive_completed_tasks_after_days = 30

[paths]
daily_notes = "journal/daily"
weekly_notes = "journal/weekly"
projects = "projects"
tasks = "tasks"
zettels = "knowledge"
quick_capture = "inbox"

[search]
default_context_depth = 2
temporal_weight = 0.7  # Favour recent notes
max_search_results = 20
enable_full_text_index = false  # Use on-demand search
```

## MCP (Model Context Protocol) Integration

### Design Goals

1. **Claude as proactive assistant**: Don't wait for explicit requests, surface context automatically
2. **Reduce user cognitive load**: Claude handles context assembly and maintenance prompts
3. **Enforce structure through interaction**: Make it easy to create properly structured notes via Claude
4. **Enable conversational vault interaction**: Natural language queries instead of CLI memorisation

### Core MCP Tools

**Task Management**:
```python
search_tasks(
    project: Optional[str],
    status: Optional[str],
    modified_after: Optional[str]
) -> List[Task]

create_task(
    title: str,
    project: str,  # Required (not optional)
    details: str = "",
    status: str = "open",
    add_to_daily: bool = True
) -> Task
  # Enforces: project must exist, status must be valid
  # Auto-generates proper frontmatter
  # Links to daily automatically

update_task_status(
    task_path: str,
    status: str,
    completion_summary: Optional[str] = None
) -> Task
  # If marking done, adds to today's daily

complete_task(
    task_path: str,
    summary: str
) -> Task
  # Convenience wrapper: marks done + logs to daily
```

**Project Management**:
```python
create_project(
    name: str,
    status: str = "planning",
    description: str = ""
) -> Project

get_project_context(
    project_path: str,
    include_tasks: bool = True,
    include_zettels: bool = True,
    days_back: int = 30
) -> ProjectContext
  # Returns: project info, tasks, recent activity, related knowledge

list_projects(
    status: Optional[str] = None,
    active_within_days: Optional[int] = None
) -> List[Project]
```

**Knowledge Retrieval**:
```python
find_zettels(
    query: str,
    tags: Optional[List[str]] = None,
    related_to_project: Optional[str] = None,
    referenced_after: Optional[str] = None
) -> List[Zettel]

get_working_context(
    query: str,
    auto_expand: bool = True,      # Include related notes
    temporal_window: str = "30d",   # Recent activity
    max_notes: int = 15
) -> WorkingContext
  # Returns structured context:
  #   - Directly matching notes
  #   - Related notes (via links + cooccurrence)
  #   - Temporal context (when active)
  #   - Open tasks in related projects
  #   - Suggested actions
```

**Daily Integration**:
```python
get_daily_summary(date: str) -> DailySummary
  # What was worked on (grouped by project/theme)
  # Tasks completed/created
  # Zettels referenced

add_to_daily(
    date: str,
    content: str,
    link_to_notes: List[str] = []
) -> Daily

get_work_timeline(
    start_date: str,
    end_date: str,
    group_by: str = "project"
) -> Timeline
  # Chronological summary of activity
```

**Maintenance and Health**:
```python
detect_stalled_work() -> List[Alert]
  # Projects not touched recently
  # Stuck tasks (in-progress >14 days)
  # Orphaned notes

suggest_connections(note_path: str) -> List[Suggestion]
  # "This zettel might relate to project X"
  # Based on content similarity + temporal overlap

find_dropped_threads() -> List[Thread]
  # Things in weekly planning not executed

get_vault_health() -> HealthReport
  # Summary of issues: orphans, broken links, stale items
```

### Proactive Behaviour Patterns

**When user mentions a project**:
```python
User: "I need to work on the RAN optimisation"

# Claude automatically:
1. Calls get_project_context("RAN-optimization")
2. Surfaces: open tasks, recent activity, related zettels
3. Asks: "I see you have 3 open tasks. Want to start with [task]?"
4. Calls add_to_daily() to log session start
```

**When user shares troubleshooting/discussion**:
```python
User: [Long explanation of debugging MCP server issues]

# Claude:
1. Calls find_zettels("MCP server debugging")
2. References existing knowledge: "You documented similar issues before"
3. Offers: "Should I create a zettel summarising this discussion?"
4. On confirmation: creates zettel, links to project, adds to daily
```

**Periodic maintenance prompts**:
```python
# Claude can proactively call detect_stalled_work()
Claude: "I notice the 'Network Slicing' project hasn't been touched 
         in 45 days. Should we archive it or resume work?"

# Or suggest connections:
Claude: "This zettel about KAN architectures relates to your 
         RAN-optimisation project. Should I link them?"
```

## Note Renaming and Reference Updates

### The Challenge

When renaming a note, must update:
1. Wikilinks: `[[old-name]]`, `[[path/to/old-name]]`
2. Markdown links: `[text](path/to/old-name.md)`
3. Frontmatter references: `project: old-name`
4. Index records (edges, temporal activity, cached paths)
5. Handle edge cases: aliases, broken links, ambiguous matches

### Architecture: Hybrid Scan Strategy

**Approach**: Index-guided with verification
1. Query index for known links (fast path)
2. Parse files index says contain references (verify)
3. Optionally perform full vault scan (validation)
4. Report discrepancies between index and reality

**Not purely index-based** (might miss updates if index stale)
**Not purely scan-based** (too slow for large vaults)

### Reference Types

**Explicit (definitive)**:
- Wikilinks: `[[note]]`, `[[path/note]]`, `[[note|alias]]`
- Markdown links: `[text](path/note.md)`, relative or absolute
- Frontmatter: `field: note-name`

**Implicit (context-dependent)**:
- Partial title matches
- Text resembling links
- Inferred references (requires semantic understanding—defer)

### Implementation Design

```rust
pub struct RenameOperation {
    old_path: PathBuf,
    new_path: PathBuf,
    update_mode: UpdateMode,  // IndexOnly, FullScan, Hybrid
    dry_run: bool,
}

pub struct RenameResult {
    updated_files: Vec<PathBuf>,
    updated_references: usize,
    broken_links_created: Vec<BrokenLink>,
    warnings: Vec<String>,
}
```

**Process**:
1. **Find references**: Query index + parse candidate files
2. **Parse references**: Extract all link types with spans
3. **Rewrite references**: Preserve original format (wikilink style, relative paths, etc.)
4. **Update files**: Atomic write of changed files
5. **Update index**: Transaction covering all index changes
6. **Reindex updated files**: Ensure index consistency

### Link Format Preservation

Critical: maintain the **same format** originally used:

```rust
// If original was [[note]] -> [[new-note]]
// If original was [[path/note]] -> [[path/new-note]]
// If original was [text](../note.md) -> [text](../new-note.md)
// If original had alias [[note|My Task]] -> [[new-note|My Task]]
```

### CLI Interface

```bash
# Basic rename with preview
mdvault rename "tasks/old-name.md" "tasks/new-name.md"
  # Shows preview of all changes
  # Asks for confirmation
  # Updates files + index atomically

# Dry run (preview only)
mdvault rename "old.md" "new.md" --dry-run

# Force without confirmation
mdvault rename "old.md" "new.md" --yes

# Strategy selection
mdvault rename "old.md" "new.md" --mode full-scan
mdvault rename "old.md" "new.md" --mode index-only
```

**Interactive preview**:
```
Found 8 references across 5 files:

journal/daily/2024-12-20.md:
  Line 15: - [ ] Continue work on [[old-name]]
           -> - [ ] Continue work on [[new-name]]

projects/project-x.md:
  Frontmatter:
    related_tasks: [old-name, other-task]
    -> related_tasks: [new-name, other-task]

Warnings:
  - Creating new path: tasks/new-name.md
  - 1 broken link will remain: [[missing-note]]

Proceed? [y/N]
```

### Index Update (Atomic)

```rust
fn update_index_after_rename(...) -> Result<()> {
    let tx = index.begin_transaction()?;
    
    // Update node record
    tx.execute("UPDATE notes SET path = ?1 WHERE path = ?2", ...)?;
    
    // Update edge records (source and target)
    tx.execute("UPDATE links SET source_path = ?1 WHERE source_path = ?2", ...)?;
    tx.execute("UPDATE links SET target_path = ?1 WHERE target_path = ?2", ...)?;
    
    // Update temporal activity
    tx.execute("UPDATE temporal_activity SET note_path = ?1 WHERE note_path = ?2", ...)?;
    
    // Invalidate cached context paths
    tx.execute("DELETE FROM context_paths WHERE ... = ?1", ...)?;
    
    tx.commit()?;
    
    // Reindex affected files
    for updated_file in updated_files {
        index.reindex_note(updated_file)?;
    }
    
    Ok(())
}
```

### Edge Cases

**1. Ambiguous references**: Multiple notes match `[[ambiguous]]`
- Disambiguate by: same folder, recent activity, co-occurrence
- Prompt user if still ambiguous

**2. Case-sensitivity**: `Task.md` -> `task.md` on case-insensitive filesystems
- Detect and use temporary intermediate name

**3. Broken links**: Some links will remain broken after rename
- Report these in preview
- Option: create redirector note (`old.md` containing `-> [[new]]`)

**4. Link aliases**: `[[old|Custom Text]]`
- Preserve alias (it's custom text, not auto-generated)
- Option: offer to update if alias matched old filename

**5. Frontmatter formats**: Various YAML structures
```yaml
project: old              # Simple
related: [old, other]     # List
meta: {parent: old}       # Nested
```
- Robust YAML parsing preserving structure

### MCP Integration

```python
rename_note(
    old_path: str,
    new_path: str,
    update_references: bool = True,
    dry_run: bool = False
) -> RenameResult

suggest_note_rename(
    note_path: str,
    reason: str
) -> str
  # "Task filename doesn't match title"
  # Returns suggested new path

normalize_note_names(
    pattern: str = "tasks/*.md",
    strategy: str = "match-title"
) -> List[RenameProposal]
  # Batch suggestions for consistency
```

## Implementation Phases

### Phase 1: Core Infrastructure
- SQLite index (nodes, edges, temporal activity)
- Frontmatter parsing and validation
- Basic search (structural + content)
- Link graph construction

### Phase 2: Structure Enforcement
- Type-specific validation rules
- Required field checking
- Linting system with auto-fix
- Note creation with scaffolding

### Phase 3: Advanced Search & Retrieval
- Contextual search (graph + temporal)
- Cooccurrence tracking
- Anomaly detection (stale, orphaned, stuck)
- Proactive surfacing (`mdvault today`, `mdvault health`)

### Phase 4: Rename & Reference Management
- Reference detection (wikilinks, markdown, frontmatter)
- Format-preserving updates
- Atomic rename operations
- Index consistency maintenance

### Phase 5: MCP Integration
- Basic tools (create, search, update)
- Workflow tools (workon, done, review)
- Context-aware retrieval
- Proactive maintenance prompts

### Phase 6: Advanced Features (Future)
- Semantic search (embeddings)
- Trend detection (topic analysis over time)
- Knowledge graph visualisation
- Collaborative features

## Key Architectural Decisions

### Why SQLite?
- Embedded (no external dependencies)
- Robust transaction support (ACID guarantees)
- Expressive queries (no need for complex graph algorithms)
- Efficient for typical vault sizes (thousands of notes)
- Portable (single file database)

### Why Rust?
- Performance for large vaults
- Strong type system (fewer runtime errors)
- Excellent CLI tooling ecosystem
- Safe concurrency (for parallel operations)
- Compiles to single binary (easy distribution)

### Why Not Full-Text Index by Default?
- Most vaults are small enough for on-demand scanning
- Avoids index maintenance complexity
- Reduces disk usage
- Optional upgrade path for large vaults (Tantivy integration)

### Why Hybrid Rename Strategy?
- Index-only: Fast but risky if stale
- Full-scan: Reliable but slow
- Hybrid: Best of both—index guides, verification ensures correctness

### Why Type-Specific Behaviour?
- Enables intelligent defaults per note type
- Supports validation and linting
- Allows optimised queries (e.g., only FTS on zettels)
- Makes MCP tools more powerful (context-aware actions)

## Success Criteria

**For CLI**:
- Fast operations (<100ms for common commands)
- Intuitive commands (minimal memorisation)
- Helpful error messages with suggestions
- Non-destructive by default (confirmations, dry-runs)

**For MCP**:
- Claude can create properly structured notes without explicit instructions
- Context retrieval feels automatic and relevant
- Maintenance prompts are actionable and non-intrusive
- Reduces time spent searching/organising

**For User Experience**:
- Vault remains organised without constant manual effort
- Finding relevant information is quick and reliable
- Structure enforcement helps rather than hinders
- System adapts to user's workflow patterns

## Open Questions for Refinement

1. **Semantic search priority**: How important is semantic similarity vs graph/temporal signals?
2. **Batch operations**: How often are bulk renames/updates needed?
3. **Real-time sync**: Should mdvault watch filesystem or assume static vault?
4. **Template system**: Should there be customisable templates for note types?
5. **Archive strategy**: How to handle completed/old notes (separate folder, frontmatter flag, deletion)?
6. **Multi-vault support**: Single tool managing multiple vaults or one instance per vault?
7. **Export/backup**: Built-in backup system or rely on external tools (git, rsync)?
