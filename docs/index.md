# markadd Documentation

Welcome to the documentation for **markadd**, a terminal-first Markdown automation tool inspired by Obsidian’s QuickAdd.

This documentation includes:

- configuration reference  
- architecture notes  
- development logs  
- roadmap overview  

If you're new, begin with:

`docs/config.md`

For development progress, see:

`docs/devlogs/`

## Contents

### User Documentation
- [config.md](./config.md) — Configuration reference
- [templates.md](./templates.md) — Template authoring guide
- [capture.md](./capture.md) — Captures reference

### Developer Documentation
- [development.md](./development.md) — Repository structure, testing, contributing
- [01_development_plan.md](./01_development_plan.md) — Full phase plan with UML diagrams

### Development Logs
- [devlogs/phase-00.md](./devlogs/phase-00.md)
- [devlogs/phase-01.md](./devlogs/phase-01.md)
- [devlogs/phase-02.md](./devlogs/phase-02.md)
- [devlogs/phase-03-architecture.md](./devlogs/phase-03-architecture.md)
- [devlogs/phase-04.md](./devlogs/phase-04.md)
- [devlogs/phase-04-mvp.md](./devlogs/phase-04-mvp.md)

### Proposals
- [devlogs/tui-integration-proposal.md](./devlogs/tui-integration-proposal.md) — Progressive TUI integration

### Source Layout

```
crates/core   – configuration, template discovery, template engine, markdown AST
crates/cli    – command-line interface
crates/tui    – terminal UI (in development)
```

## Philosophy

`markadd` aims to be:

- deterministic and testable
- terminal-native
- Markdown-first
- extensible through templates, captures, and macros

Return to project root: [README.md](../README.md)
