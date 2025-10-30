# markadd — Development Plan (with UML & Sequence Diagrams)

This document details the phased development plan for **markadd**, including per-phase architecture snapshots:
1) a **UML/class diagram** of key types/modules and their relationships, and  
2) a **sequence diagram** for the flows implemented and tested in that phase.

> Legend: these diagrams are intentionally minimal to reflect the scope of each phase. Names stabilise as we progress.


---

## Phase 0 — Repo bootstrap

**Goals:** Workspace scaffolding, CI, versioning, and an initial `doctor` stub.

```mermaid
classDiagram
  direction LR
  class Workspace {
    +/crates/core
    +/crates/cli
    +/crates/tui (stub)
    +/docs
    +/examples
  }

  class CI {
    +fmt()
    +clippy()
    +test()
    +cacheDeps()
  }

  class DoctorStub {
    +run(): Output
  }

  Workspace --> CI : GitHub Actions
  Workspace --> DoctorStub : crate(cli)
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
  CLI-->>Dev: prints stub diagnostics
```

**Deliverables:** Workspace compiles; CI green; `markadd doctor` prints version/build info.


---

## Phase 1 — Config loader (TOML) + doctor

**Goals:** Deterministic config via `~/.config/markadd/config.toml` and clear diagnostics.

```mermaid
classDiagram
  direction LR
  class ConfigLoader {
    +load(path, profile?): ResolvedConfig
  }
  class ResolvedConfig {
    +profile: String
    +vault_root: Path
    +templates_dir: Path
    +captures_dir: Path
    +macros_dir: Path
    +security: SecurityPolicy
  }
  class SecurityPolicy {
    +allow_shell: bool
    +allow_http: bool
  }
  class DoctorCmd {
    +run(rc: ResolvedConfig): Report
  }

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
  participant OUT as Doctor Report

  User->>CLI: markadd doctor [--config|--profile]
  CLI->>OS: resolve path/env
  CLI->>CFG: load(config_path, profile)
  CFG-->>CLI: ResolvedConfig or Error
  CLI->>OUT: build diagnostics
  OUT-->>User: show active profile, dirs, security flags
```

**Tests:** Missing/invalid path; profile not found; path expansion; XDG vs explicit path.  


---

## Phase 2 — Content specs (YAML/MD) + parsers

**Goals:** Parse template (MD+front-matter), capture (YAML), macro (YAML) with strict validation.

```mermaid
classDiagram
  direction LR
  class ContentLoader {
    +load_template(dir, nameOrPath): TemplateSpec
    +load_capture(dir, nameOrPath): CaptureSpec
    +load_macro(dir, nameOrPath): MacroSpec
  }
  class TemplateSpec {
    +name: String
    +vars: VarSpec[]
    +target: TargetPolicy
    +body: String
  }
  class CaptureSpec {
    +name: String
    +vars: VarSpec[]
    +target: CaptureTarget
    +content: String
    +dedupe: DedupeSpec?
  }
  class MacroSpec {
    +name: String
    +vars: VarSpec[]
    +steps: Step[]
  }

  class VarSpec { +id: String +type: VarType +prompt?: String +default?: String }
  class TargetPolicy { +path: String +if_exists: OnExists }
  class CaptureTarget { +path: String +section: String +position: Position }
  class DedupeSpec { +marker: String +scope: String }

  ContentLoader --> TemplateSpec
  ContentLoader --> CaptureSpec
  ContentLoader --> MacroSpec
  TemplateSpec o--> VarSpec
  TemplateSpec o--> TargetPolicy
  CaptureSpec o--> VarSpec
  CaptureSpec o--> CaptureTarget
  CaptureSpec o--> DedupeSpec
  MacroSpec o--> VarSpec
```

```mermaid
sequenceDiagram
  participant User
  participant CLI as cli::list
  participant CFG as ConfigLoader
  participant CTL as ContentLoader
  User->>CLI: markadd list templates|captures|macros
  CLI->>CFG: load(...)
  CFG-->>CLI: ResolvedConfig
  CLI->>CTL: scan & parse items in dirs
  CTL-->>CLI: Parsed specs (names, descriptions)
  CLI-->>User: print enumerated items
```

**Tests:** Unknown keys rejected; required fields enforced; helpful error spans.  


---

## Phase 3 — Variables & Providers + Template engine (Tera) + preview

**Goals:** Deterministic context resolution and rendering to strings.

```mermaid
classDiagram
  direction LR
  class Resolver {
    +resolve(specVars, defaults, withVars, cliVars, providers): Context
  }
  class Provider { <<interface>> +enrich(ctx): void }
  class TimeProvider
  class UuidProvider
  class GitProvider
  class EnvProvider
  class TemplateEngine {
    +render_str(tpl, ctx): String
  }

  Resolver --> Provider
  Provider <|.. TimeProvider
  Provider <|.. UuidProvider
  Provider <|.. GitProvider
  Provider <|.. EnvProvider
  Resolver --> Context
  TemplateEngine --> Context
```

```mermaid
sequenceDiagram
  participant User
  participant CLI as cli::preview
  participant CFG as ConfigLoader
  participant CTL as ContentLoader
  participant RES as Resolver
  participant TPL as TemplateEngine

  User->>CLI: markadd preview template <name> [--var k=v]
  CLI->>CFG: load config
  CFG-->>CLI: ResolvedConfig
  CLI->>CTL: load_template(dir, name)
  CTL-->>CLI: TemplateSpec
  CLI->>RES: resolve(vars, defaults, with, cliVars, providers)
  RES-->>CLI: Context
  CLI->>TPL: render_str(pathTpl, ctx) & render_str(bodyTpl, ctx)
  TPL-->>CLI: path, body
  CLI-->>User: print rendered preview
```

**Tests:** Enum/regex validation; date formatting; slugify; missing var error vs prompt (deferred to UI).  


---

## Phase 4 — Markdown AST edits (Comrak)

**Goals:** Reliable section insertion (begin/end) using an AST.

```mermaid
classDiagram
  direction LR
  class MarkdownEdit {
    +insert_into_section(input, section, frag_md, pos): String
  }
  class ComrakAdapter {
    +parse(md): Ast
    +render(ast): String
    +find_heading(ast, title): Node
    +section_tail(node, level): Node
    +splice_after(anchor, fragmentAst): void
  }

  MarkdownEdit --> ComrakAdapter
```

```mermaid
sequenceDiagram
  participant Core as MarkdownEdit
  participant Comrak as ComrakAdapter
  participant Test as GoldenTest

  Test->>Core: insert_into_section(md, "Inbox", frag, Begin)
  Core->>Comrak: parse(md)
  Comrak-->>Core: Ast
  Core->>Comrak: find_heading(Ast,"Inbox")
  Comrak-->>Core: headingNode,level
  Core->>Comrak: parse(frag)
  Core->>Comrak: splice_after(headingNode, fragAst)
  Core->>Comrak: render(Ast)
  Comrak-->>Core: newMd
  Core-->>Test: newMd (assert golden)
```

**Tests:** Empty section; last section; adjacency to code blocks/tables; Unicode headings.  


---

## Phase 5 — File planner & atomic writes (+ undo log)

**Goals:** Never corrupt notes; log changes for undo.

```mermaid
classDiagram
  direction LR
  class FileOp {
    <<enum>>
    Create(path, bytes, if_exists)
    Edit(path, transform)
  }
  class Transform { +apply(input: String): String }
  class FilePlan { +ops: FileOp[] }
  class Executor {
    +execute(plan): ExecReport
    -writeTemp()
    -fsync()
    -renameAtomic()
  }
  class OpLog {
    +append(entry)
    +entries(): Iterator
  }
  class ExecReport { +ops: int +bytes: int +duration: Duration }

  Executor --> FilePlan
  FilePlan o--> FileOp
  FileOp o--> Transform
  Executor --> OpLog
  Executor --> ExecReport
```

```mermaid
sequenceDiagram
  participant Core as Planner
  participant Exec as Executor
  participant FS as Filesystem
  participant Log as OpLog
  participant Test

  Test->>Core: plan Create/Edit ops
  Core-->>Exec: FilePlan
  Exec->>FS: write temp
  Exec->>FS: fsync(temp)
  Exec->>FS: rename(temp->final)
  Exec->>FS: fsync(parent)
  Exec->>Log: append(entry)
  Exec-->>Test: ExecReport
```

**Tests:** Crash-safety (simulated); if_exists policies; dedupe skip.  


---

## Phase 6 — Minimal CLI wiring

**Goals:** Usable commands: `template`, `capture`, `macro`, `list`, `preview`, `doctor`, `undo`.

```mermaid
classDiagram
  direction LR
  class Coordinator {
    +run_template(nameOrPath, args): Report
    +run_capture(nameOrPath, args): Report
    +run_macro(nameOrPath, args): Report
  }
  class CLI {
    +main()
    -parseArgs()
    -printHumanOrJson()
  }
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
  participant Coord as Coordinator
  participant CFG as ConfigLoader
  participant CTL as ContentLoader
  participant RES as Resolver
  participant TPL as TemplateEngine
  participant AST as MarkdownEdit
  participant EXE as Executor

  User->>CLI: markadd capture inbox --var text="Review PR #42"
  CLI->>Coord: run_capture("inbox", vars)
  Coord->>CFG: load config
  Coord->>CTL: load_capture(...)
  Coord->>RES: resolve(...)
  RES-->>Coord: Context
  Coord->>TPL: render_str(path/section/content)
  TPL-->>Coord: values
  Coord->>AST: edit(targetMd, section, fragment, pos)
  AST-->>Coord: newMd
  Coord->>EXE: execute(FilePlan(Edit))
  EXE-->>Coord: report
  Coord-->>CLI: Report
  CLI-->>User: Human/JSON output
```

**Tests:** End-to-end happy paths; JSON output shape stability.  


---

## Phase 7 — Macro runner + Security gates

**Goals:** Compose steps with shared context; gate dangerous ops (`shell`, later HTTP).

```mermaid
classDiagram
  direction LR
  class MacroRunner {
    +run(spec: MacroSpec, ctx): RunReport
  }
  class SecurityGate {
    +allow_shell(trustFlag): void
    +allow_http(trustFlag): void
  }
  class ShellExec {
    +run(cmd, args): ShellResult
  }

  MacroRunner --> SecurityGate
  MacroRunner --> Coordinator : invokes sub-actions
  SecurityGate --> ShellExec
```

```mermaid
sequenceDiagram
  participant User
  participant CLI
  participant Macro as MacroRunner
  participant Gate as SecurityGate
  participant Coord as Coordinator
  participant Shell as ShellExec

  User->>CLI: markadd macro weekly-review --trust
  CLI->>Macro: run(spec, ctx)
  loop steps
    Macro->>Coord: template/capture step
    alt shell step
      Macro->>Gate: allow_shell(trust)
      Gate-->>Macro: ok
      Macro->>Shell: run(cmd)
      Shell-->>Macro: result
    end
  end
  Macro-->>CLI: RunReport
  CLI-->>User: summary + logs
```

**Tests:** Abort vs continue; trust flag enforced; shell quoting.  


---

## Phase 8 — Lua hooks (optional)

**Goals:** Programmable captures/macros via sandboxed Lua API (escape hatch).

```mermaid
classDiagram
  direction LR
  class LuaEngine {
    +run_capture(luaFile, ctx): CapturePlan
    +run_macro(luaFile, ctx): FilePlan
  }
  class LuaApi {
    +render_string()
    +template()
    +capture()
    +now()/uuid()/slugify()/sha1()
    +sh()~gated
  }
  class Sandbox {
    +limits(cpu, mem, steps)
    -no_os_io_debug
  }

  LuaEngine --> LuaApi
  LuaEngine --> Sandbox
  LuaApi --> Coordinator : delegates actions
  Sandbox ..> SecurityGate
```

```mermaid
sequenceDiagram
  participant User
  participant CLI
  participant Lua as LuaEngine
  participant API as LuaApi
  participant Gate as Security
  participant Coord as Coordinator

  User->>CLI: markadd macro lua:macros/plan.lua --trust
  CLI->>Lua: run_macro(file, ctx)
  Lua->>API: template()/capture() calls
  API->>Coord: run_template/capture(...)
  alt sh() called
    Lua->>Gate: allow_shell(trust)
    Gate-->>Lua: ok
    API->>Coord: shell step execution
  end
  Lua-->>CLI: FilePlan/Report
  CLI-->>User: results
```

**Tests:** Sandbox disallows IO/OS; instruction/time limits; gated ops require trust.  


---

## Phase 9 — TUI (optional MVP)

**Goals:** Palette (fzf-like), previews, prompts; non-blocking engine.

```mermaid
classDiagram
  direction LR
  class TuiApp {
    +run()
    -palette
    -preview
    -prompts
  }
  class EngineFacade {
    +preview()
    +execute()
  }

  TuiApp --> EngineFacade
  EngineFacade --> Coordinator
```

```mermaid
sequenceDiagram
  participant User
  participant TUI as TuiApp
  participant Eng as EngineFacade
  participant Coord as Coordinator

  User->>TUI: open palette, select "capture: inbox"
  TUI->>Eng: preview(capture, vars)
  Eng->>Coord: preview flow (no writes)
  Coord-->>Eng: rendered content/diff
  Eng-->>TUI: show preview
  User->>TUI: confirm
  TUI->>Eng: execute(capture, vars)
  Eng->>Coord: run_capture(...)
  Coord-->>Eng: report
  Eng-->>TUI: status/log
```

**Tests:** Headless snapshot tests; prompt validation; cancel flows.  


---

## Phase 10 — Docs, polish, release

**Goals:** User docs, binaries, packaging.

```mermaid
classDiagram
  direction LR
  class Docs {
    +UserGuide
    +AuthoringTemplates
    +SecurityModel
    +CLIReference
  }
  class Release {
    +binaries(macOS/Linux)
    +homebrew_tap
    +cargo_install
  }
  Docs ..> CLI
  Docs ..> Core
  Release ..> CI
```

```mermaid
sequenceDiagram
  participant Maint
  participant CI
  participant Release
  participant Users

  Maint->>CI: tag v0.1.0
  CI->>Release: build artifacts
  Release-->>Users: brew/cargo install paths
  Maint-->>Users: docs site updated
```

---

## Cross-cutting quality gates

- **Error taxonomy:** config/content/vars/template/markdown/io/security with context-rich messages.
- **Atomic writes:** temp→fsync→rename→fsync(parent).
- **Audit/undo:** JSONL operation log.
- **Benchmarks:** large-file capture stays sub-50ms typical.
- **Fuzzing:** AST inserter (edge Markdown).

---

## Immediate next Epics

1) **Config + Doctor + List** (Phases 1–2)  
2) **Variables + AST + Planner** (Phases 3–5) → yields working `template` and `capture`.



