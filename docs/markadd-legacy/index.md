# mdvault Documentation

Welcome to the documentation for **mdvault** (formerly markadd), your Markdown vault on the command line.

mdvault is a complete terminal interface for markdown-based knowledge vaults. It combines the quick-input automation of Obsidian's QuickAdd with comprehensive vault management features.

## What mdvault does

- 📝 Create notes from templates with variables and date math
- 📥 Quick capture to daily notes and projects
- 🔁 Multi-step workflow automation (macros)
- 🔍 Full-text search across your vault (planned)
- 📊 Query notes by frontmatter metadata (planned)
- 🔗 Analyse backlinks, orphans, and connections (planned)
- 📚 Browse and read vault contents (planned)

## Getting Started

If you're new, begin with:

- [config.md](./config.md) — Configuration reference
- [templates.md](./templates.md) — Template authoring guide
- [capture.md](./capture.md) — Captures reference
- [macros.md](./macros.md) — Macros reference

For development progress, see:

`docs/devlogs/`

## Contents

### User Documentation
- [config.md](./config.md) — Configuration reference
- [templates.md](./templates.md) — Template authoring guide
- [capture.md](./capture.md) — Captures reference
- [macros.md](./macros.md) — Macros reference

### Developer Documentation
- [development.md](./development.md) — Repository structure, testing, contributing
- [01_development_plan.md](./01_development_plan.md) — Full phase plan with UML diagrams
- [03_focus_change.md](./03_focus_change.md) — Scope evolution and roadmap

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
crates/cli    – command-line interface and TUI
```

## Philosophy

`mdvault` aims to be:

- **Performance first** — Rust for speed on large vaults
- **Terminal native** — Fast, keyboard-driven workflows
- **Vault agnostic** — Works with Obsidian, Logseq, Dendron, or any markdown system
- **Extensible** — Through templates, captures, and macros
- **MCP integration ready** — JSON output formats for tooling

## Related Projects

mdvault is part of a two-project ecosystem:

- **mdvault** (this project) — Complete terminal vault manager
- **markdown-vault-mcp** — Python MCP server that delegates to mdvault for AI integration

Return to project root: [README.md](../README.md)
