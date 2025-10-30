# markadd — Development Plan (phased, practical, future-proof)

## ## Phase 0 — Repo bootstrap (1–2 days)

Goals: compile, run a no-op CLI, lock interfaces.
	- 	Repo layout

```
markadd/
  Cargo.toml
  crates/
    core/              # engine (pure logic; no TTY I/O)
    cli/               # thin adapter over core
    tui/               # (stub for now)
  docs/
  examples/
  .github/workflows/ci.yml
```



- 	Decide crate deps (add but keep unused features off):
- 	core: comrak, tera, serde, serde_yaml, toml, thiserror/anyhow, tempfile
- 	cli: clap (later), color-eyre (dev)
- 	CI: Rust stable + clippy + fmt + test; cache deps.
- 	“Doctor” stub command prints version/build info.

Deliverables: compiling workspace; CONTRIBUTING.md; RFC-000 “Vision & Interfaces”.



## Phase 1 — Ground-truth config (TOML) + loaders (2–4 days)

Goals: deterministic configuration via one ~/.config/markadd/config.toml.
- 	Schema v1 (frozen):
- 	profile, profiles. {vault_root, templates_dir, captures_dir, macros_dir}
- 	security {allow_shell, allow_http}
- 	Loader:
- 	Resolve config path (env/flag → default path).
- 	Parse + validate + normalise paths (expand ~, absolutise).
- 	Output ResolvedConfig (immutable).
- 	markadd doctor:
- 	Prints active profile, resolved dirs, security flags.
- 	Validates dirs exist; helpful errors.

Deliverables: core::config module + tests; doctor command.

Exit criteria: misconfigured paths produce actionable errors.



## Phase 2 — Content specs (YAML/MD) + parsers (3–5 days)

Goals: strict, friendly parsing for templates/captures/macros.
- 	TemplateSpec: front-matter (vars, target path policy) + body (Markdown).
- 	CaptureSpec: {target.path, section, position, content, vars, dedupe?}.
- 	MacroSpec: steps {template|capture|shell}, with shared vars.
- 	Parsers:
- 	Templates: read Markdown; split YAML front-matter; validate.
- 	Captures/Macros: serde_yaml with strict schemas (deny unknown fields).
- 	Pretty error messages (point to file/line/field).

Deliverables: core::content module + golden tests in examples/.

Exit criteria: markadd list enumerates available items from dirs.



## Phase 3 — Variables & Providers + Template engine (3–5 days)

Goals: deterministic context resolution and rendering.
- 	Providers (opt-in internally, enabled by default):
- 	now (UTC/local), uuid, cwd, env (read-only), git branch (best effort), clipboard (off by default).
- 	VarSpec types: string, integer, bool, enum{…}, date; constraints (regex, min/max).
- 	Resolution order: providers → YAML defaults → with: → CLI --var → interactive (deferred to UI; core exposes “unresolved”).
- 	Template engine: Tera with helpers (date, slugify, sha1, path_sanitise).
- 	markadd preview:
- 	For template: renders path + body to stdout (no writes).
- 	For capture: renders target path + content (no file edit).

Deliverables: core::vars + core::template; preview command.

Exit criteria: rendering is pure, testable; unresolved vars are reported.



## Phase 4 — Markdown AST edits (Comrak) (3–4 days)

Goals: reliable section insertion.
- 	Implement insert_into_section(input, section, frag_md, Position) -> String.
- 	Section = nodes between heading and next heading of same/higher level.
- 	Preserve spacing; block-level only; safe round-trip.
- 	Edge cases: empty section, last section, code fences/tables adjacent.
- 	Golden tests for begin/end on real-ish fixtures.

Deliverables: core::markdown_ast + tests.

Exit criteria: captures pass golden tests; diff is minimal and stable.



## Phase 5 — File Planner & Atomic writes (2–3 days)

Goals: never corrupt notes; enable undo.
- 	Plan types: Create{path, bytes, if_exists} | Edit{path, transform}.
- 	Atomic write: temp in same dir → write → fsync(temp) → rename → fsync(parent).
- 	Dedupe markers (optional): hash content to skip re-insert.
- 	Ops log: ~/.config/markadd/.ops.jsonl (timestamp, profile, cmd, paths, hashes).
- 	markadd undo <id> (basic: restore from backup temp kept in .markadd/undo/).

Deliverables: core::planner + executor; log module; smoke tests.

Exit criteria: power loss simulated tests still leave valid files.



## Phase 6 — Wire minimal CLI (3–4 days)

Goals: usable tool without TUI.
- 	Commands:
- 	template <name|path> [--var k=v]* [--dry-run]
- 	capture <name|path> [--var k=v]* [--dry-run]
- 	macro <name|path> [--var k=v]* [--trust?] [--dry-run]
- 	list (templates|captures|macros)
- 	preview (template|capture) <name|path>
- 	doctor
- 	Output: human by default; --json emits structured reports.
- 	Error taxonomy (config/content/vars/template/markdown/io/security) with clean messages.

Deliverables: cli crate; integration tests end-to-end.

Exit criteria: common flows work on sample vault.



## Phase 7 — Macros runner + Security gates (3–5 days)

Goals: compose steps; guard dangerous ops.
- 	MacroRunner: shared context; per-step with: merges; error policy (abort/continue).
- 	SecurityGate:
- 	Enforce config.security.* and --trust for shell.
- 	Shell execution with proper quoting; no interpolation foot-guns.
- 	Logging per step.

Deliverables: core::macro, core::security; macro tests with fake shell.

Exit criteria: macro with template→capture→shell runs with/without trust as expected.



## Phase 8 — Lua hooks (escape hatch) (optional; 5–7 days)

Goals: programmable captures/macros with a safe API.
- 	Embed mlua in safe mode; expose tiny API:
- 	Pure helpers: now, uuid, slugify, render_string, resolve_path, sha1, exists, read_text.
- 	Actions: template(use, with), capture(use/raw, with).
- 	Gated: sh(), http() (http maybe later).
- 	Sandbox: no os/io/debug; instruction limit + timeout; memory cap.
- 	CLI: --trust still required for gated ops.
- 	markadd eval-lua --file macros/foo.lua --json for CI.

Deliverables: core::lua module; examples; docs; tests with sandbox.

Exit criteria: Lua macro can loop/branch; safety gates respected.



## Phase 9 — TUI (ratatui/iocraft) MVP (optional; 1–2 weeks)

Goals: fast palette + preview.
- 	Fuzzy list of templates/captures/macros; search by name/description.
- 	Right pane: live preview (rendered doc / capture diff).
- 	Prompts: type-aware inputs (enum, date).
- 	History of last inputs; copy-to-clipboard; dry-run toggle.

Deliverables: tui crate; snapshot tests (insta) for screens.

Exit criteria: frictionless QuickAdd-like UX from terminal.



## Phase 10 — Docs, polish, release (3–4 days)
- 	User docs: install, config, authoring templates/captures/macros, examples, security model.
- 	“Doctoring” section: common misconfigurations.
- 	Changelog, versioning policy (semver).
- 	Release binaries for macOS (aarch64/x86_64), Linux (glibc/musl).
- 	Homebrew tap and/or cargo-install guidance.



Quality bar (always-on)
- 	Clippy: pedantic; deny warnings in CI.
- 	Unit + golden tests for each crate.
- 	Fuzz (optional) on AST inserter.
- 	Benchmark harness: capture on large Markdown files (200–500 KB) stays sub-50ms.



Risks & mitigations
- 	Markdown edge cases: use golden corpus; prefer AST round-trip over regex.
- 	Template complexity: limit Tera features; document best practices; precompile cache.
- 	Security: default deny for shell/http; explicit --trust; clear audit logs.
- 	Scope creep: keep Lua optional; YAML covers 80%.



Milestone acceptance (per phase)
- 	Passing CI + documented demos in examples/.
- 	doctor, list, preview always work.
- 	Reproducible runs with --json artifacts stored in CI.



Immediate next two epics
1.	Config + Doctor + List (## Phases 1–2 + part of 6)
2.	Variables/Rendering + AST Insert + Planner (## Phases 3–5)

