# mdvault

**Your Markdown Vault on the Command Line**

Version: 0.2.0 (pre-public launch)  
Status: Active development - scope evolution in progress  
Language: Rust  
Command: `mdv` (formerly `markadd`)

---

##  Important: Project Evolution in Progress

### Name Change: markadd → mdvault

**Timeline**: December 2024  
**Status**: Pre-public launch (limited early testers)  
**Impact**: Safe to rename - no public users yet

**Changes**:
- **Repository**: markadd → mdvault
- **Command**: `markadd` → `mdv`
- **Config directory**: `.markadd/` → `.mdvault/`
- **Package name**: markadd → mdvault

### Scope Evolution: QuickAdd → Complete Vault Manager

**Original Vision** (Inspired by Obsidian's QuickAdd):
-  Templates (note creation from templates)
-  Captures (quick content insertion)
-  Macros (workflow automation)
-  Multi-choice (organization)
- Focus: **Quick input and automation**

**Expanded Vision** (Complete Terminal Vault Manager):
- Templates, captures, macros (existing)
- Search (vault-wide text search) - **PRIORITY 1**
- Query (frontmatter-based filtering) - **PRIORITY 2**
- Links (backlinks, orphans, graph analysis) - **PRIORITY 3**
- List/Browse (enhanced navigation)
- Read/View (content access with options)
- Batch operations (bulk updates)
- Focus: **Complete vault management from terminal**

**Why the Evolution?**

The integration with markdown-vault-mcp revealed opportunities:
1. **Performance**: Search/query in Rust faster than Python
2. **Consistency**: Same logic for CLI and MCP integration
3. **Standalone value**: Terminal users benefit without MCP
4. **Clear positioning**: "Obsidian for the terminal" + automation
5. **Market opportunity**: Complete vault manager fills a gap

---

## Project Overview

mdvault is a complete terminal interface for markdown-based knowledge vaults. It combines the quick-input automation of Obsidian's QuickAdd with comprehensive vault management features.

**What mdvault does**:
- Create notes from templates with variables and date math
- Quick capture to daily notes and projects
- Multi-step workflow automation (macros)
- Full-text search across your vault (planned)
- Query notes by frontmatter metadata (planned)
- Analyse backlinks, orphans, and connections (planned)
- Browse and read vault contents (planned)

**Compatible with**:
- Obsidian, Logseq, Dendron, Foam
- Any markdown-based vault system
- Works standalone OR with MCP integration

---

## Architecture Context

### The Ecosystem

mdvault is part of a two-project ecosystem:

```
┌─────────────────────────────────────┐
│          mdvault (Rust)             │
│       command: mdv                  │
│                                     │
│  Your markdown vault on the         │
│  command line                       │
│                                     │
│  • Templates & Captures             │
│  • Search & Query (planned)         │
│  • Links & Graph (planned)          │
│  • Automation & Workflows           │
└──────────────┬──────────────────────┘
               │
               │ called by
               │
┌──────────────▼──────────────────────┐
│   markdown-vault-mcp (Python)       │
│                                     │
│  Bridge to AI assistants via MCP    │
│                                     │
│  • MCP protocol handling            │
│  • Tool delegation to mdvault       │
│  • Claude Desktop integration       │
└─────────────────────────────────────┘
```

### Integration Strategy

**Phase 1 (Current)**: mdvault provides templates/captures/macros
**Phase 2 (Next)**: mdvault adds search/query/links
**Phase 3 (Future)**: MCP server migrates to use mdvault for performance

This allows:
- mdvault to be valuable standalone
- MCP server to start quickly (Python)
- Gradual optimization (migrate to Rust)
- Both tools to benefit from same features

---

## Current Project Structure

```
mdvault/
├── Cargo.toml              # Rust package manifest
├── Cargo.lock
├── CLAUDE.md               # This file - AI assistant context
├── README.md               # User documentation
├── LICENSE                 # MIT
├── clippy.toml             # Linter config
├── rustfmt.toml            # Formatter config
├── rust-toolchain.toml     # Rust version
├── justfile                # Task runner
├── .github/
│   └── workflows/
│       └── ci.yml          # CI/CD pipeline
├── crates/
│   ├── cli/                # Main CLI binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs
│   ├── core/               # Core library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── config.rs
│   │       ├── template.rs
│   │       ├── capture.rs
│   │       └── macro.rs
│   └── ... (other crates)
├── docs/
│   ├── config.md           # Configuration reference
│   ├── templates.md        # Template authoring
│   ├── capture.md          # Captures reference
│   └── macros.md           # Macros reference
└── examples/               # Example templates/captures/macros
```

---

## Feature Roadmap

###  Implemented (v0.1.x)

**Templates**:
- Variable substitution (`{{variable}}`)
- Date math expressions (`{{today + 1d}}`, `{{next monday}}`)
- Frontmatter with variable metadata
- Custom output paths
- Interactive prompts (or batch mode)

**Captures**:
- Content insertion into existing notes
- Section targeting (find by heading)
- Position control (start/end/before/after)
- Variable substitution
- Target file specification

**Macros**:
- Multi-step workflows
- Template + capture combinations
- Variable passing between steps
- Shell command execution (with `--trust`)

**Core Features**:
- Profile management (multiple vaults)
- Batch mode (no prompts)
- Security model (shell commands require trust)
- TUI for browsing templates/captures/macros

### In Development (v0.2.0) - PRIORITY

**Search Command** 
```bash
mdv search "network optimization"
mdv search "TODO" --folder projects --context-lines 3
mdv search "query" --format json
mdv search "ml" --tag research --after 2024-01-01
```

**Why**:
- Most requested feature from MCP integration
- Rust performance critical for large vaults
- Foundation for advanced features
- High standalone value

**Implementation notes**:
- Use ripgrep crate or similar for performance
- Support regex and literal matching
- Cache search indices for repeated queries
- JSON output format for MCP integration
- Respect .gitignore and similar exclusion patterns

### Planned (v0.3.0)

**Query Command** 
```bash
mdv query --where "status=todo"
mdv query --where "due<2025-01-01" --where "priority=high"
mdv query --tag research --sort-by "created"
mdv query --has-field "due" --format json
```

**Why**:
- Critical for task management workflows
- Natural extension of frontmatter expertise
- Enables academic/research use cases
- High value for knowledge workers

**Implementation notes**:
- Parse frontmatter efficiently (serde_yaml)
- Build in-memory index of metadata
- Support date/number comparisons
- Handle missing fields gracefully
- Cache parsed frontmatter

### Planned (v0.4.0)

**Links Command** 
```bash
mdv links note.md --backlinks
mdv links note.md --outgoing
mdv links --orphans --folder research
mdv links --stats
```

**Why**:
- Zettelkasten workflow support
- Knowledge graph analysis
- Find disconnected notes
- Academic citation tracking

**Implementation notes**:
- Parse wikilinks: `[[Note]]`
- Parse markdown links: `[text](path.md)`
- Support Obsidian aliases: `[[Note|Alias]]`
- Build link graph cache
- Handle relative vs absolute paths

###  Future (v0.5.0+)

**List/Browse**:
- Enhanced listing with metadata filters
- Tree view of vault structure
- Recently modified/created

**Read/View**:
- Display note contents
- Expand template variables
- Format conversion options

**Batch Operations**:
- Bulk metadata updates
- Template application to multiple notes
- Automated cleanup/maintenance

---

## Command Migration Guide

### For Early Testers

If you've been using `markadd`, here's how to migrate:

**1. Update command**:
```bash
# Old
markadd new --template daily

# New
mdv new --template daily
```

**2. Update config location**:
```bash
# Old location
~/.config/markadd/config.toml
~/.markadd/templates/

# New location
~/.config/mdvault/config.toml
~/.mdvault/templates/
```

**3. Update config file**:
```toml
# Change template/capture/macro paths
[profiles.default]
vault_root = "~/vault"
templates_dir = "{{vault_root}}/.mdvault/templates"  # was .markadd
captures_dir  = "{{vault_root}}/.mdvault/captures"   # was .markadd
macros_dir    = "{{vault_root}}/.mdvault/macros"     # was .markadd
```

**4. Reinstall**:
```bash
# If installed from source
cargo install --path crates/cli

# Command is now 'mdv'
mdv --version
```

---

## Design Principles

### 1. Performance First
- Rust for speed on large vaults
- Lazy loading and caching
- Efficient file traversal
- Regex optimization

### 2. Terminal Native
- Fast, keyboard-driven workflows
- Minimal dependencies
- Works over SSH
- Scriptable and automatable

### 3. Vault Agnostic
- Works with any markdown system
- No proprietary formats
- Standard frontmatter (YAML)
- Standard link formats

### 4. Security Conscious
- Path validation (no directory traversal)
- Shell commands require explicit `--trust`
- Clear security model
- Safe defaults

### 5. MCP Integration Ready
- JSON output formats for tooling
- Batch mode (no interactive prompts)
- Structured error messages
- Comprehensive `--help` text

---

## Implementation Patterns

### Command Structure
```rust
// Each major feature gets a subcommand
mdv <subcommand> [OPTIONS]

// Subcommands:
new         - Create from template
capture     - Quick capture content
macro       - Run workflow automation
search      - Search vault (planned)
query       - Query by metadata (planned)
links       - Analyze connections (planned)
list        - List/browse (planned)
read        - View note (planned)
```

### Output Formats
```rust
// Support multiple formats for tooling
--format json        # Structured for MCP/scripting
--format markdown    # Human-readable
--format table       # Tabular data
```

### Error Handling
```rust
// Use anyhow for error context
use anyhow::{Context, Result};

fn process_template(path: &Path) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .context("Failed to read template file")?;
    Ok(content)
}

// User-facing errors should be clear
"Template not found: daily"
"Invalid date format. Use YYYY-MM-DD"
"Vault not configured. Run: mdv doctor"
```

### Configuration
```rust
// Use serde for TOML parsing
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct Config {
    version: u32,
    profile: String,
    profiles: HashMap<String, Profile>,
    security: SecurityConfig,
}
```

---

## Integration with markdown-vault-mcp

### Current Integration (v0.1.0)

MCP server calls mdvault for:
- `create_from_template()` → `mdv new --template X`
- `run_capture()` → `mdv capture X`
- `run_macro()` → `mdv macro X`

### Future Integration (v0.2.0+)

MCP server will also call:
- `search_notes()` → `mdv search "query" --format json`
- `query_notes()` → `mdv query --where "status=todo" --format json`
- `find_backlinks()` → `mdv links note.md --backlinks --format json`

### Integration Pattern
```python
# In markdown-vault-mcp (Python)
def search_notes(query: str) -> str:
    """Delegate to mdvault for performance"""
    result = subprocess.run(
        ["mdv", "search", query, "--format", "json"],
        capture_output=True,
        text=True
    )
    return parse_and_format(result.stdout)
```

### Benefits
- **Performance**: Rust speed for Python tools
- **Consistency**: Same logic CLI ↔ MCP
- **Maintenance**: One codebase for search/query/links
- **Testing**: Test in mdvault, use in MCP

---

## Development Workflow

### Setup
```bash
# Clone repository
git clone https://github.com/agustinvalencia/mdvault
cd mdvault

# Build
cargo build --release

# Install locally
cargo install --path crates/cli

# Verify
mdv --version
```

### Development
```bash
# Run without installing
cargo run -- new --template daily

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run -- search "query"

# Format code
cargo fmt

# Lint
cargo clippy
```

### Testing Strategy

**Unit Tests**:
- Template parsing and rendering
- Date math expressions
- Variable substitution
- Frontmatter parsing

**Integration Tests**:
- End-to-end command execution
- Config loading and validation
- File creation and modification
- Error handling

**MCP Integration Tests**:
- JSON output format validation
- Batch mode operation
- Error message format
- Performance benchmarks

---

## Feature Implementation Guide

### Adding Search (Priority 1)

**Steps**:
1. Add `search` subcommand to CLI
2. Implement vault traversal (respect .gitignore)
3. Add regex/literal matching
4. Implement context lines extraction
5. Add JSON output format
6. Add filtering (folder, tags, dates)
7. Write tests
8. Update documentation
9. Coordinate with MCP server team

**Key considerations**:
- Performance on large vaults (10k+ notes)
- Memory usage with large files
- Incremental/cached indexing
- Graceful handling of binary files

### Adding Query (Priority 2)

**Steps**:
1. Add `query` subcommand
2. Implement frontmatter parsing crate
3. Build in-memory metadata index
4. Implement condition parser (field=value, field<value)
5. Add tag filtering
6. Add sorting and limiting
7. JSON output format
8. Tests and docs

**Key considerations**:
- Efficient frontmatter parsing
- Date/number comparison logic
- Missing field handling
- Index caching strategy

### Adding Links (Priority 3)

**Steps**:
1. Add `links` subcommand
2. Implement link parser (wikilinks, markdown, aliases)
3. Build link graph data structure
4. Implement backlink/outgoing/orphan detection
5. Add graph statistics
6. JSON output format
7. Tests and docs

**Key considerations**:
- Multiple link format support
- Relative vs absolute path handling
- Circular reference detection
- Graph caching and invalidation

---

## Testing with MCP Server

### Setup Test Environment
```bash
# Terminal 1: Build mdvault
cd mdvault
cargo build --release
export PATH="$PWD/target/release:$PATH"

# Terminal 2: Run MCP server
cd markdown-vault-mcp
export MARKDOWN_VAULT_PATH="~/test-vault"
uv run python -m markdown_vault_mcp

# Terminal 3: Use Claude Desktop or claude-code
# Verify mdvault tools work through MCP
```

### Validation Checklist
- [ ] mdv command accessible from MCP server
- [ ] JSON output parseable by Python
- [ ] Error messages formatted correctly
- [ ] Batch mode works (no prompts)
- [ ] Security flags respected (--trust)
- [ ] Performance acceptable for typical vaults

---

## Performance Targets

### Search Performance
- **Small vault** (< 100 notes): < 50ms
- **Medium vault** (100-1000 notes): < 200ms
- **Large vault** (1000-10000 notes): < 1s
- **Huge vault** (10000+ notes): < 5s

### Query Performance
- **Metadata index build**: < 500ms for 1000 notes
- **Query execution**: < 100ms typical
- **Sorted results**: < 200ms

### Links Analysis
- **Link graph build**: < 1s for 1000 notes
- **Backlink lookup**: < 50ms
- **Orphan detection**: < 200ms

---

## Documentation Needs

### User Documentation
- [ ] Update README with new scope
- [ ] New tagline: "Your markdown vault on the command line"
- [ ] Feature overview with search/query/links
- [ ] Migration guide (markadd → mdvault)
- [ ] Comparison with alternatives

### Technical Documentation
- [ ] Architecture documentation
- [ ] API documentation (for MCP integration)
- [ ] JSON output format specifications
- [ ] Performance optimization guide
- [ ] Contributing guidelines

### Examples
- [ ] Search use cases and examples
- [ ] Query examples for common workflows
- [ ] Links analysis examples
- [ ] Integration examples with MCP

---

## Business Considerations

### Open Source Strategy
- **Core features**: Open source (MIT)
- **Community building**: Encourage contributions
- **Ecosystem value**: Integrations welcome

### Potential Premium Features
- Cloud sync integration
- Encrypted vaults
- Team collaboration features
- Advanced analytics
- Enterprise support

### Positioning
- **Standalone**: Complete terminal vault manager
- **Integration**: Foundation for MCP server
- **Market**: Terminal users + knowledge workers + researchers

---

## Quick Reference for AI Assistants

### When implementing search:
1. Use efficient file traversal (ignore patterns)
2. Support regex and literal matching
3. Extract context lines efficiently
4. Output JSON for MCP integration
5. Handle binary files gracefully

### When implementing query:
1. Parse frontmatter with serde_yaml
2. Build in-memory index
3. Support comparison operators
4. Handle missing fields
5. Return sorted, limited results

### When implementing links:
1. Parse multiple link formats
2. Build bidirectional link graph
3. Cache for performance
4. Handle relative paths
5. Detect orphans efficiently

### Key principles:
- Validate all paths (no directory traversal)
- Support `--format json` for tooling
- Provide `--batch` mode (no prompts)
- Clear error messages
- Performance matters (Rust advantage)

---

## Current Status Summary

**What works today**:
-  Templates with variables and date math
-  Captures to existing notes
-  Macros for workflows
-  Profile management
-  TUI interface
-  MCP integration

**What's next**:
-  Search command (implementing)
-  Query command (designing)
-  Links command (designing)
-  List/read commands (planned)
-  Batch operations (planned)

**Rename status**:
-  Pending: markadd → mdvault
-  Pending: `markadd` → `mdv` command
-  Pending: `.markadd/` → `.mdvault/`
-  Decision made, ready to execute

---

## Related Projects & Resources

- **markdown-vault-mcp**: Python MCP server (companion project)
- **Obsidian QuickAdd**: Original inspiration for templates/captures/macros
- **ripgrep**: Inspiration for search performance
- **fzf**: Inspiration for TUI selection
- **MCP Protocol**: https://modelcontextprotocol.io

---

*This file provides context for AI assistants working on mdvault. Keep it updated as features are implemented and scope evolves.*
