# Context State Schema

> **For Python MCP Integration**: This document defines the schema for `.mdvault/state/context.toml` that both the Rust CLI and Python MCP should use.

## File Location

```
<vault_root>/.mdvault/state/context.toml
```

The context state is **per-vault**, stored inside the vault's `.mdvault/` directory.

## Schema

```toml
# Optional: Active focus context
[focus]
project = "MCP"                           # Required: Project ID (string)
started_at = "2026-01-18T10:30:00+01:00"  # Optional: ISO 8601 datetime
note = "Working on OAuth implementation"  # Optional: Description of current work
```

## Field Definitions

### `[focus]` (Optional Section)

When present, indicates the user has an active focus context.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `project` | string | Yes | Project identifier (e.g., "MCP", "VAULT") |
| `started_at` | ISO 8601 datetime | No | When focus was set |
| `note` | string | No | Description of current work |

### Empty State

When no focus is active, the file may be empty or contain only:

```toml
# No active focus
```

Or simply not exist at all.

## JSON Representation

The `mdv focus --json` command outputs the state in JSON format:

```json
{
  "focus": {
    "project": "MCP",
    "started_at": "2026-01-18T10:30:00+01:00",
    "note": "Working on OAuth implementation"
  }
}
```

When no focus is active:

```json
{
  "focus": null
}
```

## Usage in Python MCP

### Reading Context State

```python
import tomllib
from pathlib import Path

def get_active_context(vault_root: Path) -> dict | None:
    """Read the active focus context from the vault."""
    state_file = vault_root / ".mdvault" / "state" / "context.toml"

    if not state_file.exists():
        return None

    with open(state_file, "rb") as f:
        state = tomllib.load(f)

    return state.get("focus")
```

### Example MCP Tool Implementation

```python
@mcp.tool()
def get_active_context() -> str:
    """Get the current focus context for the vault.

    Returns the active project and any associated note.
    Use this to understand what the user is currently working on.
    """
    vault_root = Path(os.environ["MARKDOWN_VAULT_PATH"])
    context = get_active_context(vault_root)

    if context is None:
        return "No active focus set."

    result = f"Active project: {context['project']}"
    if note := context.get("note"):
        result += f"\nNote: {note}"
    if started := context.get("started_at"):
        result += f"\nFocused since: {started}"

    return result
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `mdv focus` | Show current focus |
| `mdv focus <PROJECT>` | Set focus to project |
| `mdv focus <PROJECT> --note "..."` | Set focus with note |
| `mdv focus --clear` | Clear focus |
| `mdv focus --json` | Output state as JSON |

## Integration Points

### Implicit Project Context

When focus is active, commands like `mdv new task` should use the active project as a default:

```bash
# With focus set to MCP:
mdv new task "Implement OAuth"
# Equivalent to:
mdv new task "Implement OAuth" --var project=MCP
```

Explicit `--project` or `--var project=X` flags should override the context.

### Search Scoping

Search commands can scope to the active project:

```bash
# With focus set to MCP:
mdv search "OAuth"
# Could default to searching within MCP project
```

### TUI Integration

The TUI can:
- Display active focus in the status bar
- Filter views to show only focused project content
- Provide quick-toggle for focus on/off

## Future Extensions

### Context Stack (Planned)

For push/pop context switching:

```toml
[focus]
project = "SIDE"
started_at = "2026-01-18T11:00:00+01:00"

[[stack]]
project = "MCP"
started_at = "2026-01-18T10:30:00+01:00"
note = "Paused for side quest"
```

### Session Metadata (Planned)

```toml
[session]
last_activity = "2026-01-18T11:30:00+01:00"
daily_note = "Journal/Daily/2026-01-18.md"
```
