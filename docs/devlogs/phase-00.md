[ Back to devlogs index](../devlogs/)

# Phase 00 — Development Log  
## Workspace, Tooling, and CI

Phase 00 established the foundation of the markadd project.  
The goal was to create a clean workspace, adopt modern Rust standards, set up linting, and ensure a deterministic CI pipeline.



## Goals

1. Create the Rust workspace and crate structure.  
2. Adopt Rust 2024 edition.  
3. Enforce strict linting rules.  
4. Set up GitHub Actions CI for formatting, linting, testing, and coverage.  
5. Ensure deterministic snapshot behaviour.



## What Was Implemented

### 1. Workspace Structure
The project was split into three crates:

- `core` — configuration systems, template discovery, and future engines  
- `cli` — command-line interface, subcommands  
- `tui` — reserved for future interactive UI  

This separation ensures clean boundaries and scalable architecture.

### 2. Tooling
- Rust 2024 edition applied across workspace  
- `.clippy.toml` with strict, pedantic lints  
- `rustfmt` enforced in CI  
- Snapshot tests configured with insta

### 3. Continuous Integration
GitHub Actions pipeline executes:

- rustfmt check  
- clippy with `-D warnings`  
- all unit + integration tests  
- snapshot tests  
- tarpaulin coverage  

Snapshots are locked in CI via:

```
INSTA_UPDATE=no
```


### 4. Deterministic Testing
Explicit environment variables guarantee stable test results.  
Color output is disabled automatically in tests (`NO_COLOR=1`).


## Outcome

Phase 00 delivered a clean, well-structured repository with reliable tooling and CI.  
This became the basis for all future phases.
