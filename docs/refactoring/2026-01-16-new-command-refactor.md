# Refactoring Analysis: `mdv new` and Domain Behaviors

**Date:** 2026-01-16
**Status:** Partial Integration
**Analyst:** Claude (verified)

## Overview

Analysis of `crates/cli/src/cmd/new.rs` reveals that while the new `domain` behaviors (`DomainNoteType`, `CreationContext`, `NoteCreator`) are imported and instantiated, the refactoring is incomplete. The CLI command currently maintains two parallel execution paths and retains significant logic that should be delegated to the domain layer.

## Architecture Summary

```
CLI new.rs
    |
    +-- run_template_mode() [LEGACY - bypasses domain entirely]
    |       Lines 77-377
    |
    +-- run_scaffolding_mode()
            |
            +-- [IF template exists] Manual orchestration (lines 544-762)
            |       - Calls behavior.before_create() manually
            |       - Resolves output path manually
            |       - Renders template manually
            |       - Enforces core metadata manually
            |       - Validates manually
            |       - Writes file manually
            |       - Calls behavior.after_create() manually
            |       - Runs hooks manually
            |       - Logs to daily manually
            |
            +-- [ELSE no template] Uses NoteCreator::create() (lines 763-853)
                    - Proper domain delegation
                    - BUT still manually handles: hooks, log_to_daily
```

## Findings

### 1. Parallel Execution Pipelines

There is a hard split between "Template Mode" and "Scaffolding Mode":

- **`run_template_mode`** (lines 77-377): Completely bypasses the new domain architecture. It manually handles variable collection, context building, rendering, and writing. Uses a local ad-hoc flow that doesn't benefit from behaviors.

- **`run_scaffolding_mode`** (lines 379-854): Uses the new `DomainNoteType` resolution but internally splits again:
  - **With Template** (lines 544-762): Manual orchestration that duplicates `NoteCreator::create()` logic
  - **Without Template** (lines 763-853): Uses `NoteCreator::create()` properly

### 2. Duplicate Struct Definitions

**`CoreMetadata`** is defined in TWO places with identical fields:

| CLI (`new.rs` lines 31-46) | Domain (`context.rs` lines 14-56) |
|----------------------------|-----------------------------------|
| `note_type: Option<String>` | `note_type: Option<String>` |
| `title: Option<String>` | `title: Option<String>` |
| `project_id: Option<String>` | `project_id: Option<String>` |
| `task_id: Option<String>` | `task_id: Option<String>` |
| `task_counter: Option<u32>` | `task_counter: Option<u32>` |
| `project: Option<String>` | `project: Option<String>` |
| - | `date: Option<String>` |
| - | `week: Option<String>` |

The CLI version is a subset. It's only used to bridge values from `ctx.core_metadata` (domain) to the local `ensure_core_metadata` function.

### 3. Duplicate Function Implementations

**`ensure_core_metadata`** exists in BOTH:

| CLI (`new.rs` lines 861-905) | Domain (`creator.rs` lines 124-171) |
|------------------------------|-------------------------------------|
| Takes `CoreMetadata` (CLI struct) | Takes `CreationContext` (domain) |
| Same logic: parse frontmatter, inject core fields, serialize | Same logic |

Both do exactly the same thing but operate on different types.

### 4. Incomplete Delegation in Behaviors' `after_create`

The behavior implementations have TODO placeholders for logic currently active in the CLI:

**`task.rs` lines 101-114:**
```rust
fn after_create(&self, ctx: &CreationContext, _content: &str) -> DomainResult<()> {
    // ...increment counter...
    // TODO: Log to daily note
    // TODO: Run Lua on_create hook if defined
    // TODO: Reindex vault
    Ok(())
}
```

**`project.rs` lines 88-94:**
```rust
fn after_create(&self, _ctx: &CreationContext, _content: &str) -> DomainResult<()> {
    // TODO: Log to daily note
    // TODO: Run Lua on_create hook if defined
    // TODO: Reindex vault
    Ok(())
}
```

### 5. Helper Function Locations

| Function | CLI Location | Domain Location | Status |
|----------|--------------|-----------------|--------|
| `generate_inbox_task_id` | - | `task.rs:148-170` | OK - domain only |
| `get_project_info` | - | `task.rs:173-201` | OK - domain only |
| `find_project_file` | - | `task.rs:204-220` | OK - domain only |
| `increment_project_counter` | - | `task.rs:223-255` | OK - domain only |
| `generate_project_id` | - | `project.rs:132-161` | OK - domain only |
| `prompt_project_selection` | `new.rs:1195-1247` | - | CLI-only (needs UI) |
| `render_output_path` | `new.rs:1415-1430` | `behaviors/mod.rs:31-73` | DUPLICATE |

## Dead Code Candidates & Redundancy

Once the refactoring achieves full delegation to `NoteCreator`, the following CLI components become obsolete:

### Definitely Dead After Full Integration

1. **`run_template_mode` function** (lines 77-377)
   - Entire function is legacy
   - Should be unified with scaffolding mode using NoteCreator

2. **CLI `CoreMetadata` struct** (lines 31-46)
   - Redundant with `domain::CoreMetadata`
   - Only exists to bridge to local `ensure_core_metadata`

3. **CLI `ensure_core_metadata` function** (lines 861-905)
   - Redundant with `creator::ensure_core_metadata`
   - Should use domain version directly

4. **Manual orchestration block in `run_scaffolding_mode`** (lines 544-762)
   - Duplicates `NoteCreator::create()` flow
   - Should extend NoteCreator to handle templates

5. **`render_output_path` function** (lines 1415-1430)
   - Redundant with `behaviors::render_output_template`

### Should Move to Domain Layer

6. **`log_to_daily` function** (lines 1114-1191)
   - Should be a shared service/trait in `domain`
   - Used by `TaskBehavior::after_create` and `ProjectBehavior::after_create`

7. **`reindex_vault` function** (lines 907-928)
   - Should be part of `NoteCreator` finalization or a vault service

8. **`run_on_create_hook_if_exists` function** (lines 941-1056)
   - Should be internal to `NoteCreator` or `CustomBehavior`
   - Currently called manually in 3 places

9. **`apply_hook_modifications` function** (lines 1059-1110)
   - Related to hooks, should move with hook execution

### CLI-Specific (Keep in CLI)

10. **`prompt_project_selection` function** (lines 1195-1247)
    - CLI-specific (needs index access, dialoguer)
    - TaskBehavior returns `PromptType::ProjectSelector` to request this
    - Correct separation: behavior declares need, CLI implements UI

11. **`prompt_for_schema_field` function** (lines 1346-1400)
    - CLI-specific prompt implementation

12. **`collect_schema_variables` function** (lines 1254-1338)
    - CLI-specific variable collection

## Recommendations

### Phase 1: Eliminate Struct/Function Duplication

1. **Remove CLI `CoreMetadata`**: Use `domain::CoreMetadata` directly
2. **Remove CLI `ensure_core_metadata`**: Use domain version via NoteCreator
3. **Remove CLI `render_output_path`**: Use `behaviors::render_output_template`

### Phase 2: Enhance NoteCreator

1. **Add template rendering capability**: `NoteCreator::create()` should accept an optional template and render it if present, otherwise use scaffolding
2. **Integrate hook execution**: Move `run_on_create_hook_if_exists` and `apply_hook_modifications` into the creation flow
3. **Add post-creation callbacks**: Allow behaviors to register post-creation actions

### Phase 3: Create Domain Services

1. **`DailyLogService`**: Handle logging to daily notes
2. **`VaultIndexService`**: Handle reindexing after mutations
3. These can be injected into `NoteCreator` or called from behaviors

### Phase 4: Unify Entry Points

1. **Merge `run_template_mode` into unified flow**: Template mode should resolve a `DomainNoteType` and use `NoteCreator`
2. **Remove manual orchestration block**: All creation goes through `NoteCreator::create()`

## Impact Assessment

| Component | Lines of Code | Removal Impact |
|-----------|---------------|----------------|
| `run_template_mode` | ~300 | High - major simplification |
| CLI `CoreMetadata` + `ensure_core_metadata` | ~60 | Low - direct replacement |
| Manual orchestration block | ~220 | High - requires NoteCreator enhancement |
| `log_to_daily` | ~80 | Medium - needs domain service |
| Hook functions | ~170 | Medium - requires NoteCreator integration |
| `render_output_path` | ~15 | Low - direct replacement |

**Total potential reduction: ~845 lines** (out of ~1575 in new.rs = ~54% reduction)

## Next Steps

1. [ ] Create domain services for daily logging and reindexing
2. [ ] Extend NoteCreator with template rendering support
3. [ ] Migrate hook execution into NoteCreator
4. [ ] Remove CLI `CoreMetadata` and `ensure_core_metadata`
5. [ ] Unify template mode and scaffolding mode entry points
6. [ ] Remove dead code incrementally with test verification
