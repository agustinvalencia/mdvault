# markadd — Expanded Development Plan (with UML & Sequence Diagrams)

This document outlines the **phased roadmap** for developing **markadd**, including architectural diagrams
, class interactions, and sequence flows.  
Each phase builds incrementally on the previous ones, converging toward a safe, extensible Markdown
automation CLI/TUI inspired by Obsidian’s QuickAdd.

# markadd — Development Plan with Descriptions, Deliverables, Diagrams, and Folder Snapshots

This roadmap details each phase with: a short **Goal**, a **Description** of scope and constraints, explicit **Deliverables**, a **UML class diagram**, a **sequence diagram** of supported/tested flows, and a **filesystem snapshot** expected at the end of the phase.

Note: tree views are illustrative; some files (e.g., Cargo.lock) omitted for clarity.

## Phase 0 — Repo Bootstrap & CI

**Goal**  
Establish a clean multi-crate workspace, CI pipeline, and a compiling “doctor” stub to validate toolchain and wiring.

**Description**  
Create a Cargo workspace with `core`, `cli`, and a stub `tui`. Add GitHub Actions for fmt, clippy, and tests. Provide a minimal `doctor` command that prints build/version info. Define coding conventions and contribution guidelines.

**Deliverables**  
- Cargo workspace with crates: `core`, `cli`, `tui` (stub)  
- CI workflow (fmt, clippy, test)  
- `markadd doctor` stub  
- CONTRIBUTING and basic README

```mermaid
classDiagram
  direction LR
  class Workspace { +/crates/core +/crates/cli +/crates/tui +/docs +/examples }
  class CI { +fmt() +clippy() +test() +cacheDeps() }
  class DoctorStub { +run(): Output }
  Workspace --> CI
  Workspace --> DoctorStub
```

```mermaid
sequenceDiagram
  participant Dev
  participant CI
  participant CLI as cli::main
  participant Doctor as DoctorStub
  Dev->>CI: push repo
  CI->>CI: fmt + clippy + test
  Dev->>CLI: markadd doctor
  CLI->>Doctor: run()
  Doctor-->>CLI: version/build info
  CLI-->>Dev: prints diagnostics
```

Filesystem snapshot
```
markadd/
├─ Cargo.toml
├─ .github/workflows/ci.yml
├─ README.md
├─ CONTRIBUTING.md
├─ crates/
│  ├─ core/  
│  │  ├─ Cargo.toml  
│  │  └─ src/lib.rs
│  ├─ cli/   
│  │  ├─ Cargo.toml  
│  │  └─ src/main.rs
│  └─ tui/   
│     ├─ Cargo.toml  
│     └─ src/main.rs
├─ docs/DEVELOPMENT.md
└─ examples/.gitkeep
```

## Phase 1 — Config Loader (TOML) & Doctor

**Goal**  
Load and validate the ground-truth `~/.config/markadd/config.toml`, resolve the active profile, expand paths, and report via `doctor`.

**Description**  
Implement `ConfigLoader` with schema v1. Ensure deterministic precedence for `--config` and `--profile`. Validate directories and security flags. Extend `doctor` to show resolved state and actionable errors.

**Deliverables**  
- `ResolvedConfig` and `SecurityPolicy` types  
- Loader with `~` expansion and absolute path normalisation  
- Detailed `doctor` output and error taxonomy  
- Unit tests for config edge cases

```mermaid
classDiagram
  direction LR
  class ConfigLoader { +load(path, profile?): ResolvedConfig }
  class ResolvedConfig { +profile +vault_root +templates_dir +captures_dir +macros_dir +security }
  class SecurityPolicy { +allow_shell: bool +allow_http: bool }
  class DoctorCmd { +run(rc: ResolvedConfig): Report }
  ConfigLoader --> ResolvedConfig
  ResolvedConfig o--> SecurityPolicy
  DoctorCmd --> ResolvedConfig
```

```mermaid
sequenceDiagram
  participant User
  participant CLI as cli::doctor
  participant CFG as core::ConfigLoader
  participant OS as FS/Env
  participant OUT as Report
  User->>CLI: markadd doctor [--config|--profile]
  CLI->>OS: resolve config path
  CLI->>CFG: load(path, profile)
  CFG-->>CLI: ResolvedConfig
  CLI->>OUT: build diagnostics
  OUT-->>User: profile, dirs, security
```

Filesystem snapshot
```
markadd/
├─ crates/core/src/config/
│  ├─ loader.rs
│  └─ types.rs
├─ crates/core/tests/config_tests.rs
└─ crates/cli/src/cmd/doctor.rs
```

## Phase 2 — Content Parsers (YAML/MD)

**Goal**  
Parse Template (MD+front-matter), Capture (YAML), and Macro (YAML) with strict validation and friendly errors. Provide `list`.

**Description**  
Implement a `ContentLoader` that reads templates with YAML front-matter (vars, target policy) and YAML files for captures/macros. Reject unknown keys. `list` enumerates valid items with names/paths.

**Deliverables**  
- `TemplateSpec`, `CaptureSpec`, `MacroSpec` types  
- Front-matter splitter for `.md` templates  
- Strict serde_yaml parsers and error messages with file/line  
- `markadd list` command and tests

```mermaid
classDiagram
  direction LR
  class ContentLoader {
    +load_template(dir, nameOrPath): TemplateSpec
    +load_capture(dir, nameOrPath): CaptureSpec
    +load_macro(dir, nameOrPath): MacroSpec
    +scan(dir, kind): Vec<ItemMeta>
  }
  class TemplateSpec { +name +vars +target +body }
  class CaptureSpec { +name +vars +target +content +dedupe? }
  class MacroSpec   { +name +vars +steps }
```

```mermaid
sequenceDiagram
  participant User
  participant CLI as cli::list
  participant CFG as ConfigLoader
  participant CTL as ContentLoader
  User->>CLI: markadd list templates|captures|macros
  CLI->>CFG: load
  CFG-->>CLI: ResolvedConfig
  CLI->>CTL: scan & parse items
  CTL-->>CLI: ItemMeta[]
  CLI-->>User: print names/descriptions
```

Filesystem snapshot
```
markadd/
├─ crates/core/src/content/
│  ├─ loader.rs
│  ├─ template.rs
│  ├─ capture.rs
│  ├─ macro.rs
│  └─ errors.rs
├─ crates/cli/src/cmd/list.rs
└─ examples/.markadd/{templates,captures,macros}/...
```

## Phase 3 — Variable Resolution & Tera Rendering

**Goal**  
Deterministically resolve variables from providers/defaults/CLI and render both output paths and Markdown bodies. Provide `preview`.

**Description**  
Add `Resolver` with precedence: providers → YAML defaults → `with:` → CLI `--var` → prompt (UI). Integrate Tera and custom filters. Implement `preview` to render without writing.

**Deliverables**  
- `Resolver`, `Provider` trait, and core providers (time, uuid, git, env)  
- Tera engine with helpers (date, slugify, sha1)  
- `markadd preview` command  
- Tests for validation and rendering

```mermaid
classDiagram
  direction LR
  class Resolver { +resolve(vars, inputs, providers): Context }
  class Provider <<interface>> { +enrich(ctx) }
  class TimeProvider
  class UuidProvider
  class GitProvider
  class EnvProvider
  class TemplateEngine { +render_str(tpl, ctx): String }
  Provider <|.. TimeProvider
  Provider <|.. UuidProvider
  Provider <|.. GitProvider
  Provider <|.. EnvProvider
  Resolver --> TemplateEngine
```

```mermaid
sequenceDiagram
  participant User
  participant CLI as cli::preview
  participant CFG as Config
  participant CTL as Content
  participant RES as Resolver
  participant TPL as Tera
  User->>CLI: markadd preview template meeting-note --var title=Sync
  CLI->>CFG: load
  CLI->>CTL: load_template
  CLI->>RES: resolve context
  RES-->>CLI: Context
  CLI->>TPL: render path/body
  TPL-->>CLI: strings
  CLI-->>User: rendered preview
```

Filesystem snapshot
```
markadd/
├─ crates/core/src/vars/
│  ├─ resolver.rs
│  ├─ provider.rs
│  ├─ providers/{time.rs,uuid.rs,git.rs,env.rs}
│  └─ types.rs
├─ crates/core/src/template/tera_engine.rs
└─ crates/cli/src/cmd/preview.rs
```

## Phase 4 — Markdown AST Insertions (Comrak)

**Goal**  
Insert Markdown fragments at the beginning or end of a named section using an AST, not regex.

**Description**  
Wrap Comrak to parse, find headings, compute section bounds, splice fragment, and render back. Golden tests cover tricky documents (code fences, tables, last section).

**Deliverables**  
- `MarkdownEdit` trait with Comrak implementation  
- Section navigation helpers  
- Golden tests and fixtures

```mermaid
classDiagram
  direction LR
  class MarkdownEdit { +insert_into_section(input, section, frag, pos): String }
  class ComrakAdapter {
    +parse(md): Ast
    +render(ast): String
    +find_heading(ast, title): Node
    +section_tail(node, level): Node
    +splice_after(anchor, fragmentAst)
  }
  MarkdownEdit --> ComrakAdapter
```

```mermaid
sequenceDiagram
  participant Test
  participant Edit as MarkdownEdit
  participant Comrak as Adapter
  Test->>Edit: insert_into_section(md,"Inbox",frag,Begin)
  Edit->>Comrak: parse
  Comrak-->>Edit: AST
  Edit->>Comrak: find_heading
  Edit->>Comrak: parse(fragment)
  Edit->>Comrak: splice_after
  Edit->>Comrak: render
  Comrak-->>Edit: newMd
  Edit-->>Test: assert golden
```

Filesystem snapshot
```
markadd/
├─ crates/core/src/markdown_ast/
│  ├─ mod.rs
│  ├─ comrak.rs
│  └─ tests/{insert_tests.rs,golden_*}
└─ docs/CAPTURE.md
```

## Phase 5 — File Planner, Atomic Writes, Undo Log

**Goal**  
Guarantee safe writes using temp+rename+fsync and record a JSONL operation log enabling undo.

**Description**  
Define `FilePlan` for Create/Edit with pure transforms. Implement atomic executor with per-op logging and basic undo that restores pre-change content where possible.

**Deliverables**  
- `FilePlan`, `FileOp`, `Transform`, executor  
- JSONL op log and `undo` scaffolding  
- Crash-safety tests

```mermaid
classDiagram
  direction LR
  class FileOp { <<enum>> Create | Edit }
  class Transform { +apply(input): String }
  class FilePlan { +ops: FileOp[] }
  class Executor { +execute(plan): ExecReport }
  class OpLog { +append(entry) +read(id): Entry }
  class ExecReport { +ops +bytes +duration }
  FilePlan o--> FileOp
  FileOp o--> Transform
  Executor --> OpLog
```

```mermaid
sequenceDiagram
  participant Coord as Coordinator
  participant Plan as FilePlan
  participant Exec as Executor
  participant FS as Filesystem
  participant Log as OpLog
  Coord->>Exec: execute(plan)
  Exec->>FS: write temp
  Exec->>FS: fsync(temp)
  Exec->>FS: rename(temp->final)
  Exec->>FS: fsync(parent)
  Exec->>Log: append(entry)
  Exec-->>Coord: report
```

Filesystem snapshot
```
markadd/
├─ crates/core/src/planner/
│  ├─ plan.rs
│  ├─ exec.rs
│  ├─ oplog.rs
│  └─ tests/atomic_tests.rs
└─ docs/WRITES.md
```

## Phase 6 — Minimal CLI Wiring

**Goal**  
Expose working commands: `template`, `capture`, `macro`, `list`, `preview`, `doctor`, `undo` with human/JSON output.

**Description**  
Introduce a `Coordinator` facade in the CLI that wires config, content, vars, template engine, AST, and planner. Keep CLI thin; errors are categorised and surfaced cleanly.

**Deliverables**  
- CLI subcommands with shared options (`--config`, `--profile`, `--var`, `--dry-run`, `--json`, `--trust`)  
- Integration tests for template/capture end-to-end  
- Stable JSON report structs

```mermaid
classDiagram
  direction LR
  class Coordinator {
    +run_template()
    +run_capture()
    +run_macro()
    +undo(id)
  }
  class CLI { +main() -parseArgs() -print() }
  CLI --> Coordinator
  Coordinator --> ConfigLoader
  Coordinator --> ContentLoader
  Coordinator --> Resolver
  Coordinator --> TemplateEngine
  Coordinator --> MarkdownEdit
  Coordinator --> Executor
```

```mermaid
sequenceDiagram
  participant User
  participant CLI
  participant Coord
  participant CFG
  participant CTL
  participant RES
  participant TPL
  participant AST
  participant EXE
  User->>CLI: markadd capture inbox --var text="Review PR #42"
  CLI->>Coord: run_capture
  Coord->>CFG: load
  Coord->>CTL: load_capture
  Coord->>RES: resolve
  RES-->>Coord: Context
  Coord->>TPL: render strings
  Coord->>AST: insert
  Coord->>EXE: execute(plan)
  EXE-->>Coord: report
  Coord-->>CLI: result
  CLI-->>User: output
```

Filesystem snapshot
```
markadd/
├─ crates/cli/src/
│  ├─ main.rs
│  └─ cmd/
│     ├─ doctor.rs
│     ├─ list.rs
│     ├─ preview.rs
│     ├─ template.rs
│     ├─ capture.rs
│     ├─ macro.rs
│     └─ undo.rs
└─ docs/CLI.md
```

## Phase 7 — Macro Runner & Security Gates

**Goal**  
Support multi-step workflows with shared context and enforce trust for shell (and, later, HTTP).

**Description**  
Implement `MacroRunner` executing steps sequentially, merging `with:` into the shared context. Gate shell actions via `SecurityGate` requiring config permission and `--trust`. Provide clear per-step logs and error policies.

**Deliverables**  
- Macro runner with `abort`/`continue` error handling  
- Security gate and safe shell execution wrapper  
- Integration tests covering trust and failure modes

```mermaid
classDiagram
  direction LR
  class MacroRunner { +run(spec, ctx): RunReport }
  class SecurityGate { +allow_shell(trustFlag) }
  class ShellExec { +run(cmd, args): ShellResult }
  MacroRunner --> SecurityGate
  SecurityGate --> ShellExec
```

```mermaid
sequenceDiagram
  participant User
  participant CLI
  participant Macro as MacroRunner
  participant Gate as Security
  participant Shell
  User->>CLI: markadd macro weekly-review --trust
  CLI->>Macro: run(spec, ctx)
  loop steps
    alt shell step
      Macro->>Gate: allow_shell(trust)
      Gate-->>Macro: ok
      Macro->>Shell: run(cmd)
    end
  end
  Macro-->>CLI: run report
  CLI-->>User: summary
```

Filesystem snapshot
```
markadd/
├─ crates/core/src/macro/
│  ├─ runner.rs
│  └─ types.rs
├─ crates/core/src/security/
│  ├─ gate.rs
│  └─ shell.rs
└─ tests/integration_macro.rs
```

## Phase 8 — Lua Hooks (Optional)

**Goal**  
Offer a sandboxed scripting escape hatch for programmable captures/macros, without compromising safety or determinism.

**Description**  
Embed Lua via `mlua` in safe mode. Expose a tiny API to call template/capture actions and pure helpers. Disallow OS/IO by default; shell and network remain gated and require `--trust`. Provide an evaluator for CI/dry runs.

**Deliverables**  
- `LuaEngine` with `api` bindings and sandbox  
- `markadd eval-lua` command  
- Tests for sandbox limits and trust gates

```mermaid
classDiagram
  direction LR
  class LuaEngine { +run_capture(file, ctx) +run_macro(file, ctx) }
  class LuaApi { +template() +capture() +render_string() +now() +uuid() +sh()~gated }
  class Sandbox { +limits(cpu,mem,steps) -no_os_io_debug }
  LuaEngine --> LuaApi
  LuaEngine --> Sandbox
  LuaApi --> Coordinator
```

```mermaid
sequenceDiagram
  participant User
  participant CLI
  participant Lua
  participant API
  participant Coord
  participant Gate
  User->>CLI: markadd macro lua:macros/plan.lua --trust
  CLI->>Lua: run_macro(file, ctx)
  Lua->>API: template()/capture()
  API->>Coord: run_template/capture
  API->>Gate: allow_shell(trust) when sh()
  Lua-->>CLI: report
  CLI-->>User: results
```

Filesystem snapshot
```
markadd/
├─ crates/core/src/lua/
│  ├─ engine.rs
│  ├─ api.rs
│  └─ sandbox.rs
└─ examples/.markadd/macros/plan.lua
```

## Phase 9 — TUI Palette (Optional)

**Goal**  
Provide an interactive palette with fuzzy search, live previews, typed prompts, and one-keystroke execution.

**Description**  
Build a Ratatui/Iocraft TUI that lists templates/captures/macros, previews the rendered output or diff, and collects variables interactively. The TUI delegates all work to the same core coordinator.

**Deliverables**  
- TUI app with palette, preview, prompts  
- Non-blocking engine calls; cancellable prompts  
- Snapshot tests for screens

```mermaid
classDiagram
  direction LR
  class TuiApp { +run() -palette -preview -prompts }
  class EngineFacade { +preview() +execute() }
  TuiApp --> EngineFacade
  EngineFacade --> Coordinator
```

```mermaid
sequenceDiagram
  participant User
  participant TUI
  participant Eng
  participant Coord
  User->>TUI: open palette
  TUI->>Eng: preview(template/capture/macro)
  Eng->>Coord: dry-run
  Coord-->>Eng: rendered content/diff
  Eng-->>TUI: show preview
  User->>TUI: confirm
  TUI->>Eng: execute
  Eng->>Coord: run
  Coord-->>Eng: report
  Eng-->>User: status
```

Filesystem snapshot
```
markadd/
├─ crates/tui/src/
│  ├─ main.rs
│  ├─ app.rs
│  ├─ palette.rs
│  ├─ preview.rs
│  └─ prompts.rs
└─ assets/theme.toml
```

## Phase 10 — Documentation, Packaging, Release

**Goal**  
Publish binaries and comprehensive docs; ensure reproducible builds and a friendly onboarding experience.

**Description**  
Write user and authoring guides, security and CLI references. Automate release builds for macOS/Linux. Provide Homebrew tap and `cargo install` paths. Keep `doctor` guidance up to date for self-service troubleshooting.

**Deliverables**  
- Docs: CONFIG, TEMPLATES, CAPTURE, MACROS, SECURITY, CLI  
- Release CI with signed artifacts  
- Homebrew formula and crate publication  
- Changelog and versioning policy

```mermaid
classDiagram
  direction LR
  class Docs { +UserGuide +Authoring +Security +CLIRef }
  class Release { +build() +sign() +publish() }
  Docs ..> CLI
  Release ..> CI
```

```mermaid
sequenceDiagram
  participant Maintainer
  participant CI
  participant Release
  participant Users
  Maintainer->>CI: tag v0.1.0
  CI->>Release: build artifacts
  Release-->>Users: brew/cargo availability
  Maintainer-->>Users: docs site update
```

Filesystem snapshot
```
markadd/
├─ .github/workflows/release.yml
├─ docs/
│  ├─ README.md
│  ├─ CONFIG.md
│  ├─ TEMPLATES.md
│  ├─ CAPTURE.md
│  ├─ MACROS.md
│  ├─ SECURITY.md
│  └─ CLI.md
├─ dist/               # CI artifacts
└─ Formula/markadd.rb  # Homebrew tap (optional)
```



