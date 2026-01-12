# Towards Version 0.2.0

> **STATUS: COMPLETE**
>
> All phases of the v0.2.0 plan have been implemented. This document is retained for historical reference.
>
> This document supersedes all previous planning documents:
> - `PLAN.md` - Original development plan
> - `PLAN-lua-first.md` - Lua-first migration plan (all phases complete)
> - `ARCHITECTURE-domain-types.md` - Domain types rearchitecture proposal

## Executive Summary

Version 0.2.0 focused on **architectural cleanup** and **Lua-first completion**. All goals have been achieved:

1. **Test Coverage**: Added regression tests for critical code paths
2. **Domain Types Rearchitecture**: Replaced string-matching dispatch with trait-based polymorphism
3. **Lua-First Completion**: Cleaned up deprecated code, implemented Lua-based captures and macros

## Implementation Status

### What's Complete (v0.2.0)

| Component | Status |
|-----------|--------|
| Lua-First Phases 1-5 | **Complete** |
| Template-Lua linking (`lua:` field) | Complete |
| Schema-driven prompts | Complete |
| Validation integration | Complete |
| Hook execution (on_create) | Complete |
| `inherited` flag for deferred validation | Complete |
| Domain types rearchitecture | **Complete** |
| Trait-based polymorphic dispatch | **Complete** |
| `vars:` DSL removed | **Complete** |
| Lua-based captures | **Complete** |
| Lua-based macros | **Complete** |
| YAML deprecation warnings | **Complete** |

### Issues Resolved

| Problem | Resolution |
|---------|------------|
| Scattered `if type_name == "task"` checks | Replaced with `NoteType` enum and trait dispatch |
| Triple `ensure_core_metadata()` calls | Replaced with proper trait lifecycle |
| Template mode vs Scaffolding mode duplication | Unified code path via `NoteCreator` |
| Deprecated `vars:` DSL | Removed from `TemplateFrontmatter` |
| YAML-only captures/macros | Now support Lua format (YAML deprecated) |

---

## Phase 1: Test Coverage (Complete)

**Goal**: Add integration tests for all first-class type behaviors before moving code.

**Status**: Complete - Added comprehensive test coverage for the `new` command and type behaviors.

### 1.1 Test File Structure

Create `crates/cli/tests/new_builtin_types.rs` with the following test cases.

### 1.2 Task Creation Tests

```
Test: task_creation_with_project
─────────────────────────────────
Setup:
  - Create a project file at Projects/TST/TST.md with:
    ---
    type: project
    title: Test Project
    project-id: TST
    task_counter: 5
    ---
  - Index the vault

Action:
  mdv new task "My Task" --var project=TST --batch

Assertions:
  - Output file created at Projects/TST/Tasks/TST-006.md
  - Frontmatter contains:
    - type: task
    - title: My Task
    - task-id: TST-006
    - project: TST
  - Project file updated: task_counter: 6
  - Daily note contains link to [[TST-006]]
```

```
Test: task_creation_inbox
─────────────────────────
Setup:
  - Empty vault with typedefs

Action:
  mdv new task "Inbox Task" --batch

Assertions:
  - Output file created at Inbox/INB-XXX.md (where XXX is sequential)
  - Frontmatter contains:
    - type: task
    - task-id: INB-XXX
    - project: (absent or null)
  - Daily note contains link to task
```

```
Test: task_creation_preserves_core_metadata_after_hook
──────────────────────────────────────────────────────
Setup:
  - Create task.lua with on_create hook that tries to modify task-id:
    on_create = function(note)
        note.frontmatter["task-id"] = "HACKED"
        return note
    end

Action:
  mdv new task "Protected Task" --var project=TST --batch

Assertions:
  - task-id is TST-007, NOT "HACKED"
  - Hook's other modifications (if any) are preserved
```

### 1.3 Project Creation Tests

```
Test: project_creation_generates_id
───────────────────────────────────
Action:
  mdv new project "My Cool Project" --batch

Assertions:
  - Output file at Projects/MCP/MCP.md (3-letter ID from title)
  - Frontmatter contains:
    - type: project
    - title: My Cool Project
    - project-id: MCP
    - task_counter: 0
  - Daily note contains link to [[MCP]]
```

```
Test: project_creation_handles_collision
────────────────────────────────────────
Setup:
  - Create existing project with ID "MCP"

Action:
  mdv new project "More Cool Projects" --batch

Assertions:
  - Either fails with clear error, or generates alternative ID
  - Does not overwrite existing project
```

### 1.4 Daily/Weekly Creation Tests

```
Test: daily_creation_uses_date_path
───────────────────────────────────
Action:
  mdv new daily --batch

Assertions:
  - Output file at journal/daily/YYYY-MM-DD.md (today's date)
  - Frontmatter contains:
    - type: daily
    - date: YYYY-MM-DD
```

```
Test: weekly_creation_uses_week_path
────────────────────────────────────
Action:
  mdv new weekly --batch

Assertions:
  - Output file at journal/weekly/YYYY-WXX.md
  - Frontmatter contains:
    - type: weekly
    - week: YYYY-WXX
```

### 1.5 Core Metadata Preservation Tests

```
Test: core_metadata_survives_template_rendering
───────────────────────────────────────────────
Setup:
  - Template that omits type/title from frontmatter

Action:
  mdv new task "Test" --batch

Assertions:
  - Generated file still has type: task, title: Test
```

```
Test: core_metadata_survives_hook_modification
──────────────────────────────────────────────
Setup:
  - Lua hook that returns modified frontmatter without core fields

Action:
  mdv new task "Test" --batch

Assertions:
  - Core fields (type, title, task-id, project) preserved
```

### 1.6 Hook Execution Tests

```
Test: on_create_hook_can_add_fields
───────────────────────────────────
Setup:
  - Lua type with on_create that adds custom_field: "from_hook"

Action:
  mdv new custom_type "Test" --batch

Assertions:
  - custom_field: from_hook present in output
```

```
Test: on_create_hook_can_modify_content
───────────────────────────────────────
Setup:
  - Lua hook that appends "## Added by hook" to content

Action:
  mdv new custom_type "Test" --batch

Assertions:
  - Output contains "## Added by hook"
```

### 1.7 Template Mode vs Scaffolding Mode Tests

```
Test: template_mode_uses_lua_schema
───────────────────────────────────
Setup:
  - Template with lua: custom.lua
  - custom.lua defines schema with default values

Action:
  mdv new --template custom "Test" --batch

Assertions:
  - Schema defaults applied
  - Output path from Lua used (if defined)
```

```
Test: scaffolding_mode_uses_lua_schema
──────────────────────────────────────
Setup:
  - custom.lua in typedefs with schema

Action:
  mdv new custom "Test" --batch

Assertions:
  - Same schema defaults applied as template mode
```

### 1.8 Test Implementation Notes

**Test harness pattern** (based on existing tests):

```rust
use assert_cmd::prelude::*;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn write(path: &PathBuf, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn setup_vault() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let cfg_path = setup_config(&tmp, &vault);
    (tmp, vault, cfg_path)
}

fn setup_config(tmp: &tempfile::TempDir, vault: &PathBuf) -> PathBuf {
    let xdg = tmp.path().join("xdg");
    let cfg_dir = xdg.join("mdvault");
    let cfg_path = cfg_dir.join("config.toml");
    fs::create_dir_all(&cfg_dir).unwrap();

    // Create required directories
    fs::create_dir_all(vault.join(".mdvault/typedefs")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/templates")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/captures")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/macros")).unwrap();

    let toml = format!(r#"
version = 1
profile = "default"

[profiles.default]
vault_root = "{vault}"
typedefs_dir = "{vault}/.mdvault/typedefs"
templates_dir = "{vault}/.mdvault/templates"
captures_dir = "{vault}/.mdvault/captures"
macros_dir = "{vault}/.mdvault/macros"
"#, vault = vault.display());

    fs::write(&cfg_path, toml).unwrap();
    cfg_path
}

fn run_mdv(cfg_path: &PathBuf, args: &[&str]) -> std::process::Output {
    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("NO_COLOR", "1");
    cmd.args(["--config", cfg_path.to_str().unwrap()]);
    cmd.args(args);
    cmd.output().expect("Failed to run mdv")
}
```

**Run coverage after adding tests**:
```bash
just coverage
```

**Target**: Achieve 60%+ coverage on `new.rs` before proceeding to Phase 2.

---

## Phase 2: Domain Types Rearchitecture (Complete)

**Goal**: Replace string-matching with trait-based dispatch.

**Status**: Complete - Implemented `NoteType` enum with trait-based polymorphic dispatch. Created `domain` module with `NoteBehavior` traits.

### 2.1 Design Principle

> **Lua is for extending, not replacing.**

First-class types (task, project, daily, weekly) have behaviors owned by Rust. Lua provides the customization layer (schema, prompts, hooks).

### 2.2 Trait Definitions

Create `crates/core/src/domain/traits.rs`:

```rust
/// How does this note type get identified?
pub trait NoteIdentity {
    /// Generate a unique ID for this note (e.g., task-id, project-id)
    fn generate_id(&self, ctx: &CreationContext) -> Option<String>;

    /// Determine the output path for this note
    fn output_path(&self, ctx: &CreationContext) -> PathBuf;
}

/// What happens during the note lifecycle?
pub trait NoteLifecycle {
    /// Called before the note is written (can modify context)
    fn before_create(&self, ctx: &mut CreationContext) -> Result<()>;

    /// Called after the note is successfully written
    fn after_create(&self, note: &Note, vault: &Vault) -> Result<()>;
}

/// What interactive prompts does this type need?
pub trait NotePrompts {
    /// Return prompts specific to this type (e.g., project selector for tasks)
    fn prompts(&self, ctx: &PromptContext) -> Vec<FieldPrompt>;
}
```

### 2.3 Type Implementations

Create implementations for each first-class type:

| File | Type | Key Behaviors |
|------|------|---------------|
| `domain/task.rs` | `TaskBehavior` | ID from project+counter or inbox, project selector prompt, log to daily |
| `domain/project.rs` | `ProjectBehavior` | 3-letter ID from title, counter=0, log to daily |
| `domain/daily.rs` | `DailyBehavior` | Date-based path, date field |
| `domain/weekly.rs` | `WeeklyBehavior` | Week-based path, week field |
| `domain/zettel.rs` | `ZettelBehavior` | Minimal, mostly Lua-driven |
| `domain/custom.rs` | `LuaCustomBehavior` | Delegates everything to Lua |

### 2.4 NoteType Enum

```rust
pub enum NoteType {
    Task(TaskBehavior),
    Project(ProjectBehavior),
    Daily(DailyBehavior),
    Weekly(WeeklyBehavior),
    Zettel(ZettelBehavior),
    Custom(LuaCustomBehavior),
}

impl NoteType {
    pub fn from_name(name: &str, registry: &TypeRegistry) -> Result<Self> {
        match name {
            "task" => Ok(NoteType::Task(TaskBehavior::new(registry.builtin_override("task")))),
            "project" => Ok(NoteType::Project(ProjectBehavior::new(...))),
            "daily" => Ok(NoteType::Daily(DailyBehavior::new(...))),
            "weekly" => Ok(NoteType::Weekly(WeeklyBehavior::new(...))),
            "zettel" => Ok(NoteType::Zettel(ZettelBehavior::new(...))),
            _ => {
                let def = registry.get(name)?;
                Ok(NoteType::Custom(LuaCustomBehavior::new(def)))
            }
        }
    }
}
```

### 2.5 File Structure

```
crates/core/src/
├── domain/                    # NEW
│   ├── mod.rs                 # NoteType enum, from_name()
│   ├── traits.rs              # NoteIdentity, NoteLifecycle, NotePrompts
│   ├── context.rs             # CreationContext, PromptContext
│   ├── task.rs                # TaskBehavior
│   ├── project.rs             # ProjectBehavior
│   ├── daily.rs               # DailyBehavior
│   ├── weekly.rs              # WeeklyBehavior
│   ├── zettel.rs              # ZettelBehavior
│   └── custom.rs              # LuaCustomBehavior
├── types/                     # Existing (unchanged)
└── scripting/                 # Existing (unchanged)
```

### 2.6 Refactored `new.rs`

The command becomes simple orchestration:

```rust
fn run_scaffolding_mode(cfg: &ResolvedConfig, type_name: &str, args: &NewArgs) -> Result<()> {
    let registry = load_type_registry(cfg)?;
    let note_type = NoteType::from_name(type_name, &registry)?;

    let mut ctx = CreationContext::new(args.title.clone(), cfg);

    // Collect prompts (polymorphic)
    let prompts = note_type.prompts(&ctx.prompt_context());
    let answers = run_prompts(prompts)?;
    ctx.apply_answers(answers);

    // Before hook (polymorphic)
    note_type.before_create(&mut ctx)?;

    // Generate identity (polymorphic)
    let path = note_type.output_path(&ctx);

    // Common: scaffold, validate, write
    let content = scaffold_note(&ctx, &note_type)?;
    validate_content(&content, &note_type)?;
    let note = write_note(&path, &content)?;

    // After hook (polymorphic)
    note_type.after_create(&note, &vault)?;

    Ok(())
}
```

### 2.7 Migration Steps

1. **Create `domain/` module** with traits and empty implementations
2. **Implement `TaskBehavior`** by extracting code from `new.rs`
3. **Implement `ProjectBehavior`** similarly
4. **Implement remaining types**
5. **Add `NoteType::from_name()`**
6. **Refactor `run_scaffolding_mode`** to use dispatch
7. **Unify template mode** to use same dispatch
8. **Remove dead code** (ensure_core_metadata, scattered if/else)
9. **Run tests** - all Phase 1 tests must pass

### 2.8 What Gets Deleted

| Code | Reason |
|------|--------|
| `ensure_core_metadata()` | Replaced by proper trait lifecycle |
| `if template_name == "task"` blocks | Replaced by `TaskBehavior` |
| `if template_name == "project"` blocks | Replaced by `ProjectBehavior` |
| Duplicate template/scaffolding logic | Unified code path |

---

## Phase 3: Cleanup and Polish (Complete)

**Status**: Complete - Removed deprecated `vars:` DSL and updated documentation.

### 3.1 Remove Deprecated `vars:` DSL

In `crates/core/src/templates/types.rs`:

```rust
// BEFORE
pub struct TemplateFrontmatter {
    pub lua: Option<String>,
    #[deprecated]
    pub vars: Option<VarsMap>,  // Remove this
    // ...
}

// AFTER
pub struct TemplateFrontmatter {
    pub lua: Option<String>,
    // vars field removed
    // ...
}
```

Update any code that references `frontmatter.vars` to use Lua schema instead.

### 3.2 Documentation Updates

- Update `docs/lua-scripting.md` with trait-based architecture
- Update `docs/getting-started.md` if CLI behavior changed
- Archive old plan documents or mark as superseded

### 3.3 Breaking Changes for v0.2.0

| Change | Migration Path |
|--------|----------------|
| `vars:` DSL removed | Use `lua:` field pointing to type definition |
| Internal architecture | No user-facing changes |

---

## Phase 4: Lua-Based Captures and Macros (Complete)

**Status**: Complete - Implemented Lua-based captures and macros. YAML format is deprecated.

### 4.1 Lua-First Phase 5: Captures and Macros

- [x] Captures support Lua format (`crates/core/src/captures/lua_loader.rs`)
- [x] Macros support Lua format (`crates/core/src/macros/lua_loader.rs`)
- [x] YAML format deprecated with warning messages
- [x] Examples migrated to Lua (`examples/.markadd/captures/*.lua`, `examples/.markadd/macros/*.lua`)
- [x] Documentation updated (`docs/lua-scripting.md`)

See [PLAN-captures-lua.md](./PLAN-captures-lua.md) and [PLAN-macros-lua.md](./PLAN-macros-lua.md) for implementation details.

---

## Future Work (Post v0.2.0)

These are deferred to future versions:

### New First-Class Types

With the trait architecture, adding new types is straightforward:

1. Create `domain/meeting.rs` implementing traits
2. Add `NoteType::Meeting` variant
3. Update `from_name()` match

### Advanced Features

- Progress tracking for tasks/projects
- Monthly reporting
- Activity analytics
- Lifecycle hooks for captures (`before_insert`, `after_insert`)
- Remove YAML support for captures/macros (v0.3.0)

---

## Success Criteria for v0.2.0 (All Met)

| Criterion | Status |
|-----------|--------|
| Test coverage on `new.rs` | **Met** |
| No scattered type checks | **Met** - Zero `if type_name == "task"` in new.rs |
| Single code path | **Met** - Template and scaffolding modes unified |
| All existing tests pass | **Met** - `just ci` green |
| Breaking changes documented | **Met** - CHANGELOG.md updated |
| Lua-based captures | **Met** |
| Lua-based macros | **Met** |

---

## Quick Reference: Commands

```bash
# Run all tests
just test

# Run coverage analysis
just coverage

# Run specific test file
cargo test -p markadd --test new_builtin_types

# Check for regressions
just ci
```

---

## Appendix: Existing Test Coverage

### Files with Good Coverage (>50%)

These are safe to refactor:

- `types/registry.rs` - 96.6%
- `types/discovery.rs` - 83.6%
- `scripting/engine.rs` - 83.3%
- `types/scaffolding.rs` - 72.6%
- `types/schema.rs` - 76.7%
- `types/validation.rs` - 50.4%

### Files Needing Tests Before Refactoring

- `cmd/new.rs` - 31.3% (HIGH PRIORITY)
- `scripting/hook_runner.rs` - 41.1%

### Existing Test Files

| File | What it tests |
|------|---------------|
| `tests/new_simple.rs` | Basic template rendering |
| `tests/new_autofix.rs` | Auto-fix for missing defaults |
| `tests/lua_variables.rs` | Hook modifying variables |
| `tests/template_frontmatter.rs` | Template frontmatter parsing |
| `tests/variable_metadata.rs` | Variable metadata handling |

---

## Document History

| Date | Change |
|------|--------|
| 2025-01-11 | Initial version, consolidating all plans |
| 2025-01-12 | Updated to reflect completion of all phases including Lua captures/macros |
