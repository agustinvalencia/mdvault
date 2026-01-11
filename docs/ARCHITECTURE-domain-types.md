# Domain Types Architecture: Rust Core + Lua Extensions

> **Note**: This document provides detailed design for Phase 2 of [PLAN-v0.2.0.md](./PLAN-v0.2.0.md). See that document for the overall roadmap.

## Overview

This document defines the architectural approach for **first-class note types** (task, project, daily, weekly) in mdvault. It clarifies the boundary between Rust core logic and Lua extensibility.

**Related**: [Lua-First Architecture Plan](./PLAN-lua-first.md)

## Design Principle

> **Lua is for extending, not replacing.**

First-class types (tasks, projects, journaling) are foundational to mdvault's value proposition. Future features like progress tracking, monthly reporting, and activity analytics depend on predictable structure and behavior. Moving this logic to user-editable Lua would make the system brittle.

### The Hybrid Approach

| Layer | Responsibility | Examples |
|-------|---------------|----------|
| **Rust Core** | Invariants, identity, lifecycle | Task ID generation, project counters, daily logging |
| **Lua Extensions** | Customization, schema, hooks | Extra prompts, field defaults, custom validation |

A `TaskBehavior` can delegate to the user's `task.lua` for schema and hooks, but ID generation and project counter logic stay in Rust.

## Current Problem: Entanglement

The `crates/cli/src/cmd/new.rs` file has:

1. **Scattered conditionals**: `if template_name == "task"` checks in 6+ locations
2. **Triple metadata preservation**: `ensure_core_metadata()` called 3 times
3. **Two parallel code paths**: Template mode and scaffolding mode with duplicated logic
4. **No abstraction**: Type-specific behavior determined by string matching

This makes the code hard to maintain and extend.

## Proposed Architecture

### 1. Trait Hierarchy

Split concerns into focused traits rather than one monolithic `NoteBehavior`:

```rust
// crates/core/src/domain/traits.rs

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

### 2. First-Class Type Implementations

Each core type gets its own implementation in Rust:

```rust
// crates/core/src/domain/task.rs

pub struct TaskBehavior {
    /// Optional Lua extension for user customization
    lua_extension: Option<TypeDefinition>,
}

impl NoteIdentity for TaskBehavior {
    fn generate_id(&self, ctx: &CreationContext) -> Option<String> {
        // Hardcoded: project-counter or inbox logic
        match &ctx.project {
            Some(project) => {
                let counter = get_and_increment_project_counter(project)?;
                Some(format!("{}-{:03}", project.id, counter))
            }
            None => Some(generate_inbox_task_id()),
        }
    }

    fn output_path(&self, ctx: &CreationContext) -> PathBuf {
        // Rust default, can be overridden by Lua extension
        if let Some(lua) = &self.lua_extension {
            if let Some(output) = &lua.output {
                return render_output_path(output, ctx);
            }
        }

        match &ctx.project {
            Some(p) => format!("Projects/{}/Tasks/{}.md", p.id, ctx.task_id),
            None => format!("Inbox/{}.md", ctx.task_id),
        }.into()
    }
}

impl NoteLifecycle for TaskBehavior {
    fn before_create(&self, ctx: &mut CreationContext) -> Result<()> {
        // Ensure task-id is set before hooks run
        if ctx.task_id.is_none() {
            ctx.task_id = self.generate_id(ctx);
        }
        Ok(())
    }

    fn after_create(&self, note: &Note, vault: &Vault) -> Result<()> {
        // Log to daily note
        log_to_daily(note, vault)?;

        // Trigger reindex
        vault.reindex_note(note.path())?;

        // Run Lua on_create hook if defined
        if let Some(lua) = &self.lua_extension {
            if lua.has_on_create_hook {
                run_lua_hook(lua, "on_create", note)?;
            }
        }

        Ok(())
    }
}

impl NotePrompts for TaskBehavior {
    fn prompts(&self, ctx: &PromptContext) -> Vec<FieldPrompt> {
        let mut prompts = vec![
            // Hardcoded: project selector is always first
            FieldPrompt::ProjectSelector,
        ];

        // Merge with Lua-defined prompts
        if let Some(lua) = &self.lua_extension {
            prompts.extend(lua.schema_prompts());
        }

        prompts
    }
}
```

### 3. Lua Custom Behavior

User-defined types delegate entirely to Lua:

```rust
// crates/core/src/domain/custom.rs

pub struct LuaCustomBehavior {
    definition: TypeDefinition,
}

impl NoteIdentity for LuaCustomBehavior {
    fn generate_id(&self, ctx: &CreationContext) -> Option<String> {
        // Delegate to Lua if defined
        call_lua_fn(&self.definition, "generate_id", ctx).ok()
    }

    fn output_path(&self, ctx: &CreationContext) -> PathBuf {
        self.definition.output
            .as_ref()
            .map(|o| render_output_path(o, ctx))
            .unwrap_or_else(|| default_output_path(ctx))
    }
}

impl NoteLifecycle for LuaCustomBehavior {
    fn before_create(&self, ctx: &mut CreationContext) -> Result<()> {
        // User types have no hardcoded behavior
        Ok(())
    }

    fn after_create(&self, note: &Note, vault: &Vault) -> Result<()> {
        if self.definition.has_on_create_hook {
            run_lua_hook(&self.definition, "on_create", note)?;
        }
        Ok(())
    }
}
```

### 4. Type Dispatch via Enum

Replace string matching with a proper enum:

```rust
// crates/core/src/domain/mod.rs

pub enum NoteType {
    Task(TaskBehavior),
    Project(ProjectBehavior),
    Daily(DailyBehavior),
    Weekly(WeeklyBehavior),
    Zettel(ZettelBehavior),
    Custom(LuaCustomBehavior),
}

impl NoteType {
    /// Construct the appropriate behavior from a type name
    pub fn from_name(name: &str, registry: &TypeRegistry) -> Result<Self> {
        match name {
            "task" => Ok(NoteType::Task(
                TaskBehavior::new(registry.builtin_override("task"))
            )),
            "project" => Ok(NoteType::Project(
                ProjectBehavior::new(registry.builtin_override("project"))
            )),
            "daily" => Ok(NoteType::Daily(
                DailyBehavior::new(registry.builtin_override("daily"))
            )),
            "weekly" => Ok(NoteType::Weekly(
                WeeklyBehavior::new(registry.builtin_override("weekly"))
            )),
            "zettel" => Ok(NoteType::Zettel(
                ZettelBehavior::new(registry.builtin_override("zettel"))
            )),
            _ => {
                let def = registry.get(name)
                    .ok_or_else(|| anyhow!("Unknown type: {}", name))?;
                Ok(NoteType::Custom(LuaCustomBehavior::new(def)))
            }
        }
    }
}

// Delegate trait methods to inner type
impl NoteIdentity for NoteType {
    fn generate_id(&self, ctx: &CreationContext) -> Option<String> {
        match self {
            NoteType::Task(b) => b.generate_id(ctx),
            NoteType::Project(b) => b.generate_id(ctx),
            NoteType::Daily(b) => b.generate_id(ctx),
            NoteType::Weekly(b) => b.generate_id(ctx),
            NoteType::Zettel(b) => b.generate_id(ctx),
            NoteType::Custom(b) => b.generate_id(ctx),
        }
    }

    fn output_path(&self, ctx: &CreationContext) -> PathBuf {
        match self {
            NoteType::Task(b) => b.output_path(ctx),
            NoteType::Project(b) => b.output_path(ctx),
            NoteType::Daily(b) => b.output_path(ctx),
            NoteType::Weekly(b) => b.output_path(ctx),
            NoteType::Zettel(b) => b.output_path(ctx),
            NoteType::Custom(b) => b.output_path(ctx),
        }
    }
}
```

### 5. Clean `new.rs` - Single Dispatch Point

The command becomes simple orchestration:

```rust
// crates/cli/src/cmd/new.rs

fn run_scaffolding_mode(cfg: &Config, type_name: &str, title: &str) -> Result<()> {
    let registry = load_type_registry(cfg)?;
    let note_type = NoteType::from_name(type_name, &registry)?;

    // Build creation context
    let mut ctx = CreationContext::new(title, cfg);

    // Collect prompts (polymorphic - Task adds project selector)
    let prompts = note_type.prompts(&ctx.prompt_context());
    let answers = run_prompts(prompts)?;
    ctx.apply_answers(answers);

    // Before hook (polymorphic - sets up IDs, metadata)
    note_type.before_create(&mut ctx)?;

    // Generate identity (polymorphic - Task uses counter, Project uses title)
    let id = note_type.generate_id(&ctx);
    let path = note_type.output_path(&ctx);

    // Common: scaffold, validate, write
    let content = scaffold_note(&ctx, &note_type)?;
    validate_content(&content, &note_type)?;
    let note = write_note(&path, &content)?;

    // After hook (polymorphic - Task logs to daily, triggers reindex)
    note_type.after_create(&note, &vault)?;

    Ok(())
}
```

## File Structure

```
crates/core/src/
├── domain/                    # NEW: First-class type behaviors
│   ├── mod.rs                 # NoteType enum, dispatch
│   ├── traits.rs              # NoteIdentity, NoteLifecycle, NotePrompts
│   ├── context.rs             # CreationContext, PromptContext
│   ├── task.rs                # TaskBehavior
│   ├── project.rs             # ProjectBehavior
│   ├── daily.rs               # DailyBehavior
│   ├── weekly.rs              # WeeklyBehavior
│   ├── zettel.rs              # ZettelBehavior
│   └── custom.rs              # LuaCustomBehavior (delegates to Lua)
├── types/                     # Existing: Schema, validation, registry
│   ├── definition.rs          # TypeDefinition (Lua-loaded)
│   ├── schema.rs              # FieldSchema
│   ├── registry.rs            # TypeRegistry
│   └── validation.rs          # Schema validation
└── scripting/                 # Existing: Lua engine, hooks
    ├── engine.rs              # LuaEngine
    ├── hook_runner.rs         # run_on_create_hook
    └── bindings.rs            # mdv.* functions
```

## Benefits

| Current Problem | Solution |
|-----------------|----------|
| `if template_name == "task"` scattered in 6+ places | Single dispatch via `NoteType` enum |
| Core metadata preserved 3 times | `before_create` sets metadata once, correctly |
| Template mode vs Scaffolding mode duplication | Both call the same trait methods |
| Hard to add "Meeting" as first-class later | Just add `MeetingBehavior` struct |
| Lua can corrupt task IDs | Rust owns `generate_id`, Lua can only extend schema |

## What Stays in Lua

Lua remains the customization layer for:

| Feature | Mechanism |
|---------|-----------|
| Field schema definition | `schema` table in Lua |
| Field validation | `validate()` function |
| Prompt text/behavior | `prompt` in schema fields |
| Default values | `default` in schema fields |
| Output path templates | `output` string (can be overridden) |
| Post-creation hooks | `on_create()` for user-defined logic |
| Variable definitions | `variables` block |

## What Stays in Rust

Critical behaviors that must be predictable:

| Behavior | Reason |
|----------|--------|
| Task ID generation | Requires atomic counter increment |
| Project ID generation | Consistent format for references |
| Project counter management | Must be atomic across concurrent creates |
| Project selection prompt | Requires index database access |
| Log to daily note | Core workflow integration |
| Vault reindexing | Data integrity |
| Core metadata preservation | Prevents corruption by hooks |

## Migration Path

### Phase 1: Extract Domain Module
- Create `crates/core/src/domain/` with trait definitions
- Implement `TaskBehavior`, `ProjectBehavior` etc.
- Keep existing `new.rs` working (no behavior changes)

### Phase 2: Refactor `new.rs`
- Replace string matching with `NoteType::from_name()`
- Route through trait methods
- Remove duplicated code paths
- Remove triple `ensure_core_metadata()` calls

### Phase 3: Consolidate Template/Scaffolding Modes
- Unify the two creation paths
- Both use the same trait dispatch

### Phase 4: Test & Validate
- Ensure all existing tests pass
- Add trait-specific unit tests
- Verify Lua extensions still work

## Open Questions

1. **Zettel behavior**: Should zettel have any hardcoded behavior, or is it purely Lua-defined?
2. **Builtin overrides**: When a user provides `task.lua`, how much can it override?
3. **New first-class types**: What's the process to promote a custom type to first-class?
4. **Hook ordering**: Should Lua `on_create` run before or after `after_create` Rust logic?

## Conclusion

This architecture provides:

1. **Stability**: Core behaviors are predictable and testable
2. **Extensibility**: New first-class types are easy to add
3. **Customizability**: Users can extend (not replace) via Lua
4. **Maintainability**: Clean separation of concerns, no scattered conditionals

The key insight: **Rust owns the invariants, Lua owns the workflow customization.**
