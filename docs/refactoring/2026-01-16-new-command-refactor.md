# Refactoring Analysis: `mdv new` and Domain Behaviors

**Date:** 2026-01-16
**Status:** Partial Integration

## Overview
Analysis of `crates/cli/src/cmd/new.rs` reveals that while the new `domain` behaviors (`DomainNoteType`, `CreationContext`, `NoteCreator`) are imported and instantiated, the refactoring is incomplete. The CLI command currently maintains two parallel execution paths and retains significant logic that should be delegated to the domain layer.

## Findings

### 1. Parallel Execution Pipelines
There is a hard split between "Template Mode" and "Scaffolding Mode":
- **`run_template_mode`**: Completely bypasses the new domain architecture. It manually handles variable collection, context building, rendering, and writing.
- **`run_scaffolding_mode`**: Uses the new `DomainNoteType` resolution but internally splits again between "Template Scaffolding" (manual orchestration) and "Pure Scaffolding" (using `NoteCreator`).

### 2. Incomplete Delegation in `run_scaffolding_mode`
Even within the scaffolding path, the CLI retains logic that belongs in `NoteCreator` or specific `NoteBehavior` implementations:
- **Manual Orchestration:** The CLI manually calls `behavior.before_create()`, validates output paths, renders templates, and enforces `CoreMetadata`. This duplicates the responsibility of `NoteCreator::create`.
- **Logic Leaks:** `ensure_core_metadata` is implemented in the CLI to enforce core fields, whereas `NoteIdentity::core_fields()` exists in the domain but isn't fully utilized to enforce this integrity within the domain layer itself.

### 3. "TODO" implementations in Domain Layer
The behavior implementations (`Daily`, `Task`, `Project`, etc.) have placeholders for logic currently active in the CLI:
- **`after_create`**: Marked as `// TODO` in behaviors for:
    - Logging to daily note (currently `log_to_daily` in CLI).
    - Running Lua `on_create` hooks (currently `run_on_create_hook_if_exists` in CLI).
    - Vault reindexing (currently `reindex_vault` in CLI).

## Dead Code Candidates & Redundancy
Once the refactoring targets full delegation to `NoteCreator`, the following CLI components will become obsolete:

1.  **`run_template_mode` function**: Entire function is legacy.
2.  **`log_to_daily` function**: Should be moved to a shared service/trait in `domain` used by `TaskBehavior` and `ProjectBehavior`.
3.  **`reindex_vault` function**: Should be part of the `VaultContext` or `NoteCreator` finalization.
4.  **`run_on_create_hook_if_exists`**: Should be internal to `NoteCreator` or `CustomBehavior`.
5.  **`prompt_project_selection`**: Logic moved to `TaskBehavior::type_prompts`, but the standalone function remains in CLI.
6.  **`generate_inbox_task_id` / `get_project_info`**: Likely duplicated between CLI helpers and `TaskBehavior` internal helpers.

## Recommendations

1.  **Enhance `NoteCreator`**: Update `NoteCreator::create` to handle template rendering internally if a template is present in the context.
2.  **Migrate `after_create` Logic**: Move daily logging and hook execution from `new.rs` into the specific behavior's `after_create` implementation or a generic `NoteCreator` post-processing step.
3.  **Unified Entry Point**: Refactor `run` to immediately resolve a `DomainNoteType` (defaulting to a "Generic/Template" type if only `--template` is passed) and call `NoteCreator::create`.
4.  **Remove Legacy Code**: Delete `run_template_mode` and the manual orchestration block in `run_scaffolding_mode`.
