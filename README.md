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
- **Type-Aware Scaffolding**: `mdv new task "My Task"` creates notes with schema-based frontmatter
- **Interactive Task Creation**: `mdv new task` prompts to select a project from existing ones
- **Task Management**: `mdv task list` shows tasks by project, `mdv task done` marks complete
- **Project Overview**: `mdv project list` shows projects with open/done task counts
- **Captures**: Quick append to existing notes (daily logs, project notes)
- **Macros**: Multi-step workflow automation
- **Date Math**: Expressions like `{{today + 1d}}` or `{{today + monday}}`
- **Template Filters**: `{{title | slugify}}`, `{{name | lowercase}}`, etc.
- **TUI**: Interactive palette for templates, captures, and macros
- **Vault Indexing**: SQLite-based index with note metadata, link graph, and incremental updates
- **Index Queries**: List notes, find backlinks/outlinks, detect orphans via CLI
- **Lua Scripting**: Sandboxed Lua runtime with access to date math and template engines
- **Type System**: Lua-based type definitions with schemas, validation, and lifecycle hooks
- **Validation**: `mdv validate` checks notes against type schemas with auto-fix support
- **MCP Server**: Basic vault browsing and note operations

### In Development
- Contextual search (graph neighbourhood + temporal signals)
- Daily logging integration for task completion

## Installation

### Pre-built Binaries (Recommended)

Download the latest release for your platform from the [Releases page](https://github.com/agustinvalencia/mdvault/releases).

```bash
# macOS/Linux: Extract and move to PATH
tar xzf mdv-*.tar.gz
sudo mv mdv /usr/local/bin/

# Verify installation
mdv --version
```

### Homebrew (macOS/Linux)

```bash
brew install agustinvalencia/tap/mdvault
```

### Cargo (Rust)

```bash
cargo install mdvault
```

### Build from Source

```bash
git clone https://github.com/agustinvalencia/mdvault.git
cd mdvault
cargo build --release
# Binary is at target/release/mdv
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
| `mdv new <type> "Title"` | Create note with type-based scaffolding |
| `mdv new --template <name>` | Create note from template |
| `mdv capture <name>` | Run a capture workflow |
| `mdv macro <name>` | Execute a multi-step macro |
| `mdv list-templates` | List available templates |
| `mdv reindex` | Build or rebuild the vault index |
| `mdv list` | List notes with filters (type, date, limit) |
| `mdv links <note>` | Show backlinks and outgoing links |
| `mdv orphans` | Find notes with no incoming links |
| `mdv validate` | Validate notes against type schemas |
| `mdv validate --fix` | Auto-fix safe validation issues |
| `mdv rename <old> <new>` | Rename note and update all references |
| `mdv search <query>` | Search notes with contextual matching |
| `mdv stale` | Find neglected notes |
| `mdv task list` | List tasks grouped by project with status |
| `mdv task done <task>` | Mark a task as done |
| `mdv project list` | List projects with task counts |
| `mdv project <name> tasks` | Show project tasks in kanban view |

See `mdv --help` for full options.

## Note Types

mdvault enforces note types via frontmatter. Types can be customized with Lua definitions in `~/.config/mdvault/types/`:

| Type | Purpose | Required Fields |
|------|---------|-----------------|
| `daily` | Daily notes, temporal backbone | `date` |
| `weekly` | Weekly overviews | `week_start_date` |
| `task` | Individual tasks | `status`, `project` |
| `project` | Task collections | `status`, `created_date` |
| `zettel` | Knowledge notes | `tags` |
| `none` | Uncategorised (triage queue) | — |

## MCP Integration

mdvault has a [sister project](https://github.com/agustinvalencia/markdown-vault-mcp) being developed for an MCP server for AI-assisted vault interaction

This enables Claude and other MCP clients to:
- Browse and search vault contents
- Create properly structured notes
- Surface relevant context automatically
- Prompt for maintenance tasks

## Documentation

- [Architecture and Design](./docs/architecture.md) — Full design philosophy and technical details
- [Development Plan](./docs/PLAN.md) — Implementation phases and roadmap
- [Lua Scripting](./docs/lua-scripting.md) — Using the Lua scripting layer

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
