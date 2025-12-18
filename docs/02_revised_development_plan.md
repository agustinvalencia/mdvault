# markadd — Revised Development Plan v3

> **Status**: Active
> **Created**: 2024-12-18
> **Supersedes**: `01_development_plan.md` (Phases 5+)

This document outlines the revised roadmap for markadd development, incorporating lessons learned from the MVP implementation and prioritizing **usability**, **security**, and **extensibility**.

## Current State (Phases 0-4 Complete)

The MVP is functional with:
- Config loading with profiles and XDG discovery
- Template/capture discovery and `{{var}}` substitution
- Markdown AST insertions via Comrak
- Frontmatter parsing and modification operations
- TUI with Elm architecture (browse, preview, execute)
- CLI commands: `doctor`, `list-templates`, `new`, `capture`

**Key architectural decisions made:**
- TUI integrated into `cli` crate (not separate)
- Simple `{{var}}` substitution (not Tera)
- Direct module calls (no Coordinator facade yet)

## Design Principles

### Usability
- **Interactive by default**: Prompt for missing variables instead of failing
- **Progressive disclosure**: Simple commands for simple tasks, advanced options available
- **Helpful errors**: Every error message suggests a fix
- **Keyboard-first**: Efficient for terminal power users

### Security
- **Least privilege**: Nothing executes unless explicitly trusted
- **Defense in depth**: Config flags + CLI flags + TUI confirmation
- **Audit trail**: All risky operations logged
- **Fail-safe defaults**: No destructive action without explicit confirmation

### Extensibility
- **Lua for power users**: Full programmability without complex provider architecture
- **Declarative workflows**: YAML macros handle most automation
- **Simple core**: Date math built-in, everything else via Lua

---

## Phase 5: Macros + Interactive Variable Prompts

**Goal**: Real automation with smart variable handling.

This phase has two parallel tracks that reinforce each other.

### Track A: Interactive Variables

**Problem today**: Running `markadd new --template meeting` without `--var title=X` fails. This is poor UX—the tool should prompt for missing values.

#### Deliverables

1. **CLI interactive prompts**:
   ```bash
   $ markadd new --template meeting
   ? title: Weekly sync
   ? attendees: Alice, Bob
   ✓ Created notes/2024-01-15-weekly-sync.md
   ```

2. **Variable metadata in templates/captures** (enhanced frontmatter):
   ```yaml
   ---
   output: "meetings/{{date}}-{{title | slugify}}.md"
   vars:
     title:
       prompt: "Meeting title"
       required: true
     attendees:
       prompt: "Who's attending?"
       default: ""
     date:
       prompt: "Meeting date"
       default: "{{today}}"
   ---
   # Meeting: {{title}}
   ```

3. **Smart defaults**:
   - `default: "{{today}}"` — computed defaults with date math
   - `default: ""` — optional, empty if skipped
   - No default = required, must prompt

4. **Non-interactive mode**: `--no-interactive` or `--batch` flag for CI/scripting (fails fast on missing vars)

5. **TUI enhancements**:
   - Show variable prompts and descriptions
   - Tab to skip optional vars with defaults
   - Live preview updates as user types

#### CLI Behavior

```
$ markadd new --template meeting --var title="Sync"

1. Load template, extract required vars: [title, attendees, date]
2. Check provided via --var: title ✓
3. Check defaults: date has default "{{today}}" ✓
4. Missing without default: attendees
5. If interactive:
   ? attendees: _
6. If --no-interactive:
   ✗ Error: missing required variable 'attendees'
     Hint: use --var attendees="..." or remove --no-interactive
```

#### TUI Variable Input Screen

```
┌─────────────────────────────────────────────────┐
│ New: meeting                                    │
├─────────────────────────────────────────────────┤
│                                                 │
│  title: Weekly sync█                            │
│  ─────────────────────────────────────────────  │
│  Meeting title (required)                       │
│                                                 │
│  attendees: [Tab to skip]                       │
│  ─────────────────────────────────────────────  │
│  Who's attending?                               │
│                                                 │
│  date: 2024-01-15                               │
│  ─────────────────────────────────────────────  │
│  Default: today                                 │
│                                                 │
├─────────────────────────────────────────────────┤
│ [Enter] Next  [Tab] Skip optional  [Esc] Cancel │
└─────────────────────────────────────────────────┘
```

### Track B: Macro Foundation

#### Deliverables

1. **MacroSpec YAML format**:
   ```yaml
   name: weekly-review
   description: "Set up weekly review documents"
   vars:
     week_topic:
       prompt: "What's the focus this week?"
     start_date:
       prompt: "Week starting"
       default: "{{today - monday}}"
   steps:
     - template: weekly-summary
       with: { topic: "{{week_topic}}" }
     - capture: archive-tasks
       with: { archive_date: "{{start_date}}" }
   ```

2. **Variable flow in macros**:
   ```
   CLI --var flags
        ↓
   Macro-level vars (prompt if missing)
        ↓
   Step-level `with:` overrides
        ↓
   Template/capture vars (prompt if still missing)
   ```

3. **Macro CLI command**:
   ```bash
   $ markadd macro weekly-review
   ? week_topic: Q1 Planning
   ? start_date: [2024-01-15]
   ✓ Step 1/2: Created weekly-summary.md
   ✓ Step 2/2: Updated tasks.md
   ```

4. **Macro discovery**: `.yaml` files in `macros_dir` (from config)

5. **`markadd macro --list`**: List available macros with descriptions

6. **TUI macro support**:
   - Macros appear in palette alongside templates/captures
   - Step-by-step variable collection
   - Progress indicator during execution

### Track C: Date Math Expressions

Date math fits naturally with interactive defaults.

#### Syntax

| Expression | Result |
|------------|--------|
| `{{today}}` | Current date (YYYY-MM-DD) |
| `{{now}}` | Current datetime (ISO 8601) |
| `{{time}}` | Current time (HH:MM) |
| `{{today + 1d}}` | Tomorrow |
| `{{today - 1w}}` | One week ago |
| `{{today + 2M}}` | Two months from now |
| `{{now + 3h}}` | Three hours from now |
| `{{today - monday}}` | Previous Monday |
| `{{today + friday}}` | Next Friday |

#### Format Specifier

| Expression | Result |
|------------|--------|
| `{{today \| %Y-%m-%d}}` | 2024-01-15 |
| `{{today \| %A}}` | Monday |
| `{{today \| %B %d, %Y}}` | January 15, 2024 |
| `{{now \| %H:%M}}` | 14:30 |

#### Implementation Notes

- Use `chrono` for date/time handling
- Parse expressions in template engine before variable substitution
- Support in: template bodies, frontmatter values, capture content, macro `with:` blocks

### Variable Metadata Schema

Full schema for template/capture frontmatter:

```yaml
---
# Output path (templates only)
output: "path/to/{{var}}.md"

# Variable definitions
vars:
  var_name:
    prompt: "Human-readable prompt"     # Shown when prompting user
    description: "Longer explanation"   # Shown in TUI/help
    required: true                      # Default: true if no default
    default: "value"                    # Static or computed ({{today}})
    # Future extensions:
    # options: [a, b, c]                # Selection prompt
    # validate: "regex pattern"         # Input validation
    # type: string | date | number      # Type hints
---
```

### File Changes

```
crates/core/src/
├── macros/                    # NEW
│   ├── mod.rs
│   ├── spec.rs                # MacroSpec, MacroStep types
│   ├── runner.rs              # Sequential step execution
│   └── discovery.rs           # Find macros in macros_dir
├── templates/
│   ├── engine.rs →            # Add date math parsing
│   └── vars.rs                # NEW: VarSpec, VarMetadata types
├── captures/
│   └── spec.rs →              # Add vars metadata support
└── lib.rs →                   # Export macros module

crates/cli/src/
├── cmd/
│   ├── new.rs →               # Add interactive prompts
│   ├── capture.rs →           # Add interactive prompts
│   └── macro_cmd.rs           # NEW: macro command
├── tui/
│   ├── app.rs →               # Add Macro variant to PaletteItem
│   ├── ui/
│   │   └── input.rs →         # Enhanced variable input UI
│   └── actions.rs →           # Add macro execution
└── prompt.rs                  # NEW: CLI interactive prompts
```

---

## Phase 6: Atomic Writes & Dry-Run

**Goal**: Safe operations with recovery options.

### Deliverables

1. **Atomic file writes**:
   - Write to temporary file in same directory
   - `fsync` the temp file
   - Rename temp → target (atomic on POSIX)
   - `fsync` parent directory
   - No partial writes on crash

2. **`--dry-run` flag**:
   - Shows all prompts (interactive mode)
   - Renders full output
   - Prints what would be written
   - Returns success/failure without writing
   - Works for templates, captures, and macros

3. **Operation log**:
   - JSONL format in `~/.local/share/markadd/oplog.jsonl`
   - Records: timestamp, operation type, file path, before hash, after hash
   - Configurable retention (default: 100 operations)

4. **Basic undo**:
   ```bash
   $ markadd undo           # Undo last operation
   $ markadd undo --last 3  # Undo last 3 operations
   $ markadd undo --list    # Show recent operations
   ```
   - Restores file content from before operation
   - Only works for operations in oplog
   - Warns if file was modified since operation

### File Changes

```
crates/core/src/
├── planner/                   # NEW
│   ├── mod.rs
│   ├── file_op.rs             # FileOp enum (Create, Edit)
│   ├── executor.rs            # Atomic write implementation
│   └── oplog.rs               # Operation logging, undo support
└── lib.rs →                   # Export planner module

crates/cli/src/
├── cmd/
│   ├── new.rs →               # Use atomic executor
│   ├── capture.rs →           # Use atomic executor
│   ├── macro_cmd.rs →         # Use atomic executor
│   └── undo.rs                # NEW: undo command
└── main.rs →                  # Add --dry-run global flag
```

---

## Phase 7: Security Gates

**Goal**: Safe execution of shell steps in macros.

### Deliverables

1. **`--trust` CLI flag**:
   ```bash
   $ markadd macro deploy-notes --trust
   ```
   Required for any operation that executes shell commands.

2. **Config-level security policy**:
   ```toml
   [security]
   allow_shell = false    # Even with --trust, deny shell
   allow_http = false     # Future: HTTP requests in Lua
   audit_log = true       # Log all trusted operations
   ```

3. **Shell steps in macros**:
   ```yaml
   steps:
     - shell: "git add {{file}}"
       description: "Stage file in git"
       # trust_required is implicit for shell steps
     - shell: "git commit -m 'Add {{title}}'"
   ```

4. **Security gate behavior**:
   - No `--trust` flag → shell steps skipped with warning
   - `--trust` flag + `allow_shell = false` → error
   - `--trust` flag + `allow_shell = true` → execute

5. **TUI trust confirmation**:
   ```
   ┌─────────────────────────────────────────────────┐
   │ ⚠ Trust Required                               │
   ├─────────────────────────────────────────────────┤
   │ This macro wants to run shell commands:        │
   │                                                 │
   │   $ git add notes/meeting.md                   │
   │   $ git commit -m 'Add meeting notes'          │
   │                                                 │
   │ Allow execution?                               │
   ├─────────────────────────────────────────────────┤
   │     [Y] Yes, trust    [N] No, skip shells      │
   └─────────────────────────────────────────────────┘
   ```

6. **Audit log**:
   - Separate from oplog: `~/.local/share/markadd/audit.jsonl`
   - Records: timestamp, user, command, trust granted, shell commands run
   - Not rotated (permanent record)

### File Changes

```
crates/core/src/
├── security/                  # NEW
│   ├── mod.rs
│   ├── gate.rs                # SecurityGate, trust checking
│   ├── policy.rs              # SecurityPolicy from config
│   └── shell.rs               # Safe shell execution wrapper
├── macros/
│   └── runner.rs →            # Integrate security gate
└── config/
    └── types.rs →             # Add SecurityPolicy struct

crates/cli/src/
├── cmd/
│   └── macro_cmd.rs →         # Add --trust flag
├── tui/
│   └── ui/
│       └── trust_dialog.rs    # NEW: Trust confirmation UI
└── main.rs →                  # Add --trust global flag
```

---

## Phase 8: Lua Hooks

**Goal**: Programmable extensibility without complex provider architecture.

### Deliverables

1. **Sandboxed Lua engine**:
   - Use `mlua` crate in safe mode
   - No `os`, `io`, `debug` libraries
   - CPU/memory limits via mlua hooks

2. **Lua steps in macros**:
   ```yaml
   name: smart-note
   vars:
     topic:
       prompt: "What topic?"
   steps:
     - lua: |
         local topic = markadd.var("topic")
         local slug = topic:lower():gsub(" ", "-")
         markadd.set("slug", slug)
         markadd.set("filename", slug .. ".md")
     - template: topic-note
       with: { slug: "{{slug}}" }
       output: "topics/{{filename}}"
   ```

3. **Standalone Lua macros**:
   - `.lua` files in `macros_dir`
   - Full programmatic control
   ```lua
   -- macros/weekly-setup.lua
   local week = markadd.prompt("week_num", "Week number")

   markadd.template("weekly-summary", {
     week = week,
     start = markadd.today() - "monday"
   })

   for i = 1, 5 do
     local day = markadd.today() - "monday" + (i - 1) .. "d"
     markadd.template("daily-note", { date = day })
   end
   ```

4. **Lua API**:

   | Function | Description |
   |----------|-------------|
   | `markadd.var(name)` | Get current variable value |
   | `markadd.set(name, value)` | Set variable for subsequent steps |
   | `markadd.prompt(name, message, default?)` | Prompt user mid-macro |
   | `markadd.today()` | Current date (supports date math) |
   | `markadd.now()` | Current datetime |
   | `markadd.env(name)` | Get environment variable |
   | `markadd.template(name, vars)` | Execute template |
   | `markadd.capture(name, vars)` | Execute capture |
   | `markadd.sh(cmd)` | Shell execution (requires trust) |
   | `markadd.log(msg)` | Log message to output |

5. **TUI Lua preview**:
   - Show Lua source before execution
   - Indicate sandbox status
   - List shell commands that will require trust

### File Changes

```
crates/core/src/
├── lua/                       # NEW
│   ├── mod.rs
│   ├── engine.rs              # Lua VM setup, sandbox config
│   ├── api.rs                 # markadd.* function bindings
│   └── sandbox.rs             # Resource limits, blocked modules
├── macros/
│   ├── runner.rs →            # Add Lua step handling
│   └── discovery.rs →         # Discover .lua macros
└── lib.rs →                   # Export lua module

crates/cli/src/
├── cmd/
│   └── eval_lua.rs            # NEW: markadd eval-lua <script>
└── tui/
    └── ui/
        └── preview.rs →       # Lua source preview
```

### Cargo Dependencies

```toml
[dependencies]
mlua = { version = "0.9", features = ["lua54", "vendored", "send"] }
```

---

## Phase 9: Obsidian Integration (Nice-to-Have)

**Goal**: Bridge markadd and Obsidian for seamless workflow.

### Deliverables

1. **Obsidian URI steps in macros**:
   ```yaml
   steps:
     - template: meeting-note
     - obsidian: open
       file: "{{output_path}}"
   ```

2. **Supported URI actions**:
   | Action | URI Generated |
   |--------|---------------|
   | `open` | `obsidian://open?vault=X&file=Y` |
   | `new` | `obsidian://new?vault=X&file=Y&content=Z` |
   | `search` | `obsidian://search?vault=X&query=Q` |

3. **CLI command**:
   ```bash
   $ markadd open notes/meeting.md  # Opens in Obsidian
   ```

4. **Config**:
   ```toml
   [obsidian]
   vault_name = "MyVault"    # Default vault for URI generation
   ```

5. **Platform support**:
   - macOS: `open "obsidian://..."`
   - Linux: `xdg-open "obsidian://..."`
   - Windows: `start obsidian://...`

### File Changes

```
crates/core/src/
├── obsidian/                  # NEW
│   ├── mod.rs
│   └── uri.rs                 # URI builder, opener
├── macros/
│   └── runner.rs →            # Add obsidian step handling
└── config/
    └── types.rs →             # Add ObsidianConfig

crates/cli/src/
└── cmd/
    └── open.rs                # NEW: markadd open command
```

---

## Phase 10: Polish & Documentation

**Goal**: Production-ready release.

### Deliverables

1. **TUI polish**:
   - Fuzzy search in palette (using `fuzzy-matcher` or `nucleo`)
   - Help overlay (`?` key)
   - Configurable keybindings
   - Theme support (optional)

2. **Error message improvements**:
   - Every error includes actionable fix suggestion
   - Context-aware hints (e.g., "Did you mean 'meeting-note'?")
   - Colored output for clarity

3. **Documentation**:
   - `docs/user-guide.md` — Getting started, common workflows
   - `docs/templates.md` — Template authoring (update existing)
   - `docs/captures.md` — Capture authoring (update existing)
   - `docs/macros.md` — Macro authoring (new)
   - `docs/lua.md` — Lua API reference (new)
   - `docs/security.md` — Trust model, audit, sandboxing (new)
   - `docs/cli-reference.md` — All commands and flags

4. **Release pipeline**:
   - GitHub Actions release workflow
   - Build for: macOS (arm64, x86_64), Linux (x86_64, arm64)
   - GitHub Releases with changelogs
   - Homebrew formula
   - `cargo install markadd` (crates.io)

5. **Testing**:
   - Integration tests for all CLI commands
   - TUI snapshot tests
   - Macro execution tests
   - Security gate tests

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                       CLI / TUI                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Commands  │  │  Prompts    │  │   TUI App           │  │
│  │  (clap)     │  │  (dialoguer)│  │   (ratatui)         │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         └────────────────┼────────────────────┘             │
│                          │                                  │
├──────────────────────────┼──────────────────────────────────┤
│                    Macro Runner                             │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────────────┐ │
│  │Template │  │ Capture │  │  Shell  │  │      Lua        │ │
│  │  Step   │  │  Step   │  │  Step   │  │      Step       │ │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────────┬────────┘ │
│       └───────────┬┴───────────┬┘                │          │
│                   │            │                 │          │
│  ┌────────────────▼────────────▼─────────────────▼────────┐ │
│  │                  Security Gate                         │ │
│  │         (--trust required for shell/Lua sh())          │ │
│  └────────────────────────────┬───────────────────────────┘ │
│                               │                             │
│  ┌────────────────────────────▼───────────────────────────┐ │
│  │               Atomic File Executor                     │ │
│  │          (temp → fsync → rename → log)                 │ │
│  └────────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                      markadd-core                           │
│  ┌──────────┐ ┌───────────┐ ┌────────────┐ ┌─────────────┐  │
│  │ Template │ │  Capture  │ │ Markdown   │ │    Date     │  │
│  │  Engine  │ │  Engine   │ │    AST     │ │    Math     │  │
│  └──────────┘ └───────────┘ └────────────┘ └─────────────┘  │
│  ┌──────────┐ ┌───────────┐ ┌────────────┐                  │
│  │  Config  │ │Frontmatter│ │    Lua     │                  │
│  │  Loader  │ │   Ops     │ │   Engine   │                  │
│  └──────────┘ └───────────┘ └────────────┘                  │
└─────────────────────────────────────────────────────────────┘
```

---

## Priority Summary

| Priority | Feature | Phase | Effort |
|----------|---------|-------|--------|
| **Critical** | Interactive variable prompts | 5 | Medium |
| **Critical** | Macros (multi-step workflows) | 5 | High |
| **Critical** | Date math expressions | 5 | Medium |
| **High** | Atomic writes | 6 | Medium |
| **High** | Dry-run mode | 6 | Low |
| **High** | Security gates for shell | 7 | Medium |
| **Medium** | Undo support | 6 | Medium |
| **Medium** | Lua hooks | 8 | High |
| **Nice-to-have** | Obsidian integration | 9 | Low |
| **Nice-to-have** | Fuzzy search in TUI | 10 | Low |

---

## Migration Notes

### Breaking Changes (None Expected)

The revised plan builds on the existing MVP without breaking changes:
- Existing templates continue to work (vars metadata is optional)
- Existing captures continue to work
- CLI commands remain compatible

### New Dependencies

| Phase | Crate | Purpose |
|-------|-------|---------|
| 5 | `dialoguer` | CLI interactive prompts |
| 8 | `mlua` | Lua scripting engine |
| 10 | `nucleo` or `fuzzy-matcher` | Fuzzy search |

---

## Open Questions

1. **Variable validation**: Should we support regex validation in var metadata now, or defer?
2. **Macro error handling**: Default to `abort` on step failure, or make configurable per-macro?
3. **Lua sandboxing**: What resource limits are appropriate (memory, CPU cycles)?
4. **Obsidian detection**: Auto-detect vault path from Obsidian config, or require explicit config?

---

## Appendix: Full Variable Metadata Schema

```yaml
vars:
  variable_name:
    # Display
    prompt: "Short prompt shown to user"
    description: "Longer description for help text"

    # Requirements
    required: true | false      # Default: true if no default provided
    default: "value"            # Static string or "{{expression}}"

    # Future extensions (not in Phase 5)
    type: string | date | number | boolean
    options: [opt1, opt2, opt3] # Selection/dropdown prompt
    validate: "^[a-z]+$"        # Regex validation
    min: 0                      # For numbers
    max: 100                    # For numbers
    multiline: false            # For string input
```

---

*This plan will be updated as development progresses. See `devlogs/` for per-phase implementation notes.*
