# mdvault

**CLI and MCP Server for Markdown Vault Management**

[![Build Status](https://github.com/agustinvalencia/mdvault/actions/workflows/ci.yml/badge.svg)](https://github.com/agustinvalencia/mdvault/actions)
[![codecov](https://codecov.io/gh/agustinvalencia/mdvault/branch/main/graph/badge.svg)](https://codecov.io/gh/agustinvalencia/mdvault)

mdvault is a Rust-based CLI tool and MCP (Model Context Protocol) server for managing markdown vaults. It's designed for knowledge workers who need structure without friction—particularly those who excel at capturing information but struggle with retrieval and maintenance.

## Design Philosophy

**Pull-Optimised, Not Push-Optimised**: Notes get created correctly during hyperfocus but go stale without maintenance. Retrieval is the primary failure mode, so mdvault focuses on finding information when you need it rather than optimising capture.

**Opinionated Structure**: Rather than maximising flexibility, mdvault enforces structure through required frontmatter fields, automatic scaffolding, validation workflows, and enforced linking patterns.

**ADHD-Friendly Principles**:
- Reduce cognitive load with smart defaults
- Progressive capture—quick entry, structured cleanup later
- Automated maintenance—proactive detection of stale/broken/orphaned content
- Passive surfacing—don't wait for searches, show relevant context

## Current Status

mdvault is undergoing a significant expansion. The core templating and capture system is functional, while the indexing, search, and MCP integration are in development.

### Working Now

- **Templates**: Create notes from templates with variable substitution
- **Captures**: Quick append to existing notes (daily logs, project notes)
- **Macros**: Multi-step workflow automation
- **Date Math**: Expressions like `{{today + 1d}}` or `{{today + monday}}`
- **TUI**: Interactive palette for templates, captures, and macros
- **MCP Server**: Basic vault browsing and note operations

### In Development

- SQLite-based indexing (metadata, links, temporal activity)
- Contextual search (graph neighbourhood + temporal signals)
- Structure validation and linting
- Type-specific workflows (task, project, zettel, daily, weekly)
- Proactive maintenance (stale detection, orphan finding)

## Installation

```bash
cargo install --path crates/cli
```

Or build from source:

```bash
cargo build --release
```

## Quick Start

1. Create a configuration file:

```bash
mkdir -p ~/.config/mdvault
cat > ~/.config/mdvault/config.toml << 'EOF'
version = 1
profile = "default"

[profiles.default]
vault_root = "~/Notes"
templates_dir = "{{vault_root}}/.mdvault/templates"
captures_dir  = "{{vault_root}}/.mdvault/captures"
macros_dir    = "{{vault_root}}/.mdvault/macros"

[security]
allow_shell = false
allow_http  = false
EOF
```

2. Verify your setup:

```bash
mdv doctor
```

3. Launch the TUI:

```bash
mdv
```

## Commands

| Command | Description |
|---------|-------------|
| `mdv` | Launch interactive TUI |
| `mdv doctor` | Validate configuration |
| `mdv new --template <name>` | Create note from template |
| `mdv capture <name>` | Run a capture workflow |
| `mdv macro <name>` | Execute a multi-step macro |
| `mdv list-templates` | List available templates |

See `mdv --help` for full options.

## Vault Structure (Planned)

mdvault will enforce note types via frontmatter:

| Type | Purpose | Required Fields |
|------|---------|-----------------|
| `daily` | Daily notes, temporal backbone | `date` |
| `weekly` | Weekly overviews | `week_start_date` |
| `task` | Individual tasks | `status`, `project` |
| `project` | Task collections | `status`, `created_date` |
| `zettel` | Knowledge notes | `tags` |
| `none` | Uncategorised (triage queue) | — |

## MCP Integration

mdvault includes an MCP server for AI-assisted vault interaction:

```bash
# Run as MCP server
mdv mcp
```

This enables Claude and other MCP clients to:
- Browse and search vault contents
- Create properly structured notes
- Surface relevant context automatically
- Prompt for maintenance tasks

## Documentation

- [Architecture and Design](./docs/architecture.md) — Full design philosophy and technical details
- [Development Plan](./docs/PLAN.md) — Implementation phases and roadmap

### Legacy Documentation

The original markadd documentation (templates, captures, macros, configuration) is preserved in [`docs/markadd-legacy/`](./docs/markadd-legacy/).

## Compatibility

Works with any markdown-based vault system:
- Obsidian
- Logseq
- Dendron
- Foam
- Plain markdown folders

## License

See [LICENSE](./LICENSE).
