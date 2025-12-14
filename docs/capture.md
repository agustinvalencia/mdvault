# Captures

Captures allow you to quickly append content to specific sections of existing Markdown files. Think of them as "quick add" shortcuts for your notes.

## Quick Start

```bash
# List available captures
markadd capture --list

# Capture a task to your inbox
markadd capture inbox --var text="Review PR #42"
```

This inserts `- [ ] Review PR #42` into the "Inbox" section of your daily note.

## Capture Specification

Captures are defined as YAML files in your `captures_dir` (default: `~/.markadd/captures/` or `{{vault_root}}/.markadd/captures/`).

### Basic Structure

```yaml
name: inbox
description: Add a quick note to today's inbox

target:
  file: "daily/{{date}}.md"
  section: "Inbox"
  position: begin

content: "- [ ] {{text}}"
```

### Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Logical name for the capture |
| `description` | No | Human-readable description |
| `target.file` | Yes | Path to target file (relative to vault_root) |
| `target.section` | Yes | Heading text to find |
| `target.position` | No | `begin` (default) or `end` of section |
| `content` | Yes | Content template to insert |

## Variables

Both `target.file` and `content` support variable substitution using `{{variable}}` syntax.

### Built-in Variables

| Variable | Example | Description |
|----------|---------|-------------|
| `{{date}}` | `2024-01-15` | Current date (YYYY-MM-DD) |
| `{{time}}` | `14:30` | Current time (HH:MM) |
| `{{datetime}}` | `2024-01-15T14:30:00+00:00` | ISO 8601 datetime |
| `{{vault_root}}` | `/home/user/vault` | Vault root path |

### User Variables

Pass custom variables with `--var`:

```bash
markadd capture inbox --var text="My note" --var priority=high
```

In the capture spec:
```yaml
content: "- [ ] [{{priority}}] {{text}}"
```

## Section Matching

The `target.section` field matches heading text:

- **Case-insensitive** by default: `"Inbox"` matches `# inbox`, `## INBOX`, etc.
- **Trimmed**: Leading/trailing whitespace is ignored
- **First match wins**: If multiple sections have the same name, the first is used

### Section Bounds

Content is inserted within the section's bounds:
- **Start**: Immediately after the heading
- **End**: Before the next heading of same or higher level, or end of file

Example document:
```markdown
# Daily Note

## Inbox          <- Section starts here

- Existing item   <- Section content

## Done           <- Section ends before this
```

## Insert Positions

### `position: begin`

Inserts immediately after the section heading:

```markdown
## Inbox
- NEW ITEM        <- Inserted here
- Existing item
```

### `position: end`

Inserts at the end of the section, before the next heading:

```markdown
## Inbox
- Existing item
- NEW ITEM        <- Inserted here

## Done
```

## Examples

### Daily Inbox

```yaml
# captures/inbox.yaml
name: inbox
description: Quick capture to today's daily note

target:
  file: "daily/{{date}}.md"
  section: "Inbox"
  position: begin

content: "- [ ] {{text}}"
```

Usage:
```bash
markadd capture inbox --var text="Call dentist"
```

### Project TODO

```yaml
# captures/project-todo.yaml
name: project-todo
description: Add task to project TODO

target:
  file: "projects/current.md"
  section: "TODO"
  position: end

content: "- [ ] {{task}} (added {{date}})"
```

Usage:
```bash
markadd capture project-todo --var task="Implement feature X"
```

### Meeting Notes

```yaml
# captures/meeting.yaml
name: meeting
description: Log a quick meeting note

target:
  file: "meetings/{{date}}.md"
  section: "Notes"
  position: end

content: |
  ### {{time}} - {{title}}

  {{notes}}
```

Usage:
```bash
markadd capture meeting \
  --var title="Standup" \
  --var notes="Discussed sprint progress"
```

## Error Handling

### Section Not Found

If the target section doesn't exist, markadd shows available sections:

```
Section not found: 'Inbox'
Available sections in /vault/daily/2024-01-15.md:
  - Daily Note (level 1)
  - Tasks (level 2)
  - Done (level 2)
```

### File Not Found

The target file must exist before capturing:

```
Failed to read target file /vault/daily/2024-01-15.md
Hint: The target file must exist before capturing to it.
```

Use `markadd new` to create files from templates first.

### Capture Not Found

If the capture doesn't exist, markadd lists available captures:

```
Capture not found: unknown
Available captures:
  - inbox
  - todo
  - meeting
```

## Best Practices

1. **Create templates for target files**: Use `markadd new` to create daily notes with the expected sections.

2. **Use descriptive names**: Name captures after their purpose (`inbox`, `todo`, `meeting-note`).

3. **Keep content simple**: Captures are for quick additions. For complex content, use templates.

4. **Use `position: begin` for priority items**: New items appear at the top.

5. **Use `position: end` for chronological items**: New items appear at the bottom.

## Related

- [config.md](./config.md) - Configuration reference
- [templates.md](./templates.md) - Template authoring guide
