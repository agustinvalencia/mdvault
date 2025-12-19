# Templates in markadd

This document describes how templates work in `markadd`:
how they are discovered, named, and rendered.



## Overview

A *template* in `markadd` is a Markdown file stored inside the profile's configured `templates_dir`.
Templates are discovered recursively and exposed through the CLI:

```bash
markadd list-templates
```

Each template has:

- a **physical path** (inside your vault)
- a **logical name** (used in CLI commands)
- optional **frontmatter** with metadata and output path
- **variables** that are substituted at render time



## Template Location

Templates live under the directory:

templates_dir

Defined per-profile in `config.toml`:

```
[profiles.default]
templates_dir = “{{vault_root}}/.markadd/templates”
```

You may structure templates however you wish.  
Folders become logical namespaces.

Example vault tree:

```
Notes/.markadd/templates/
daily.md
weekly.md
blog/
draft.md
```



## What Counts as a Template

A template is any file whose name ends **exactly** in:

```
.md
```

The following are **allowed** templates:

```
daily.md
blog/post.md
meeting/notes.md
```

The following are **excluded** on purpose:

```
ignore.tpl.md
document.tmpl.md
file.markdown
notes.mdx
template.txt
```

This rule keeps the early system simple while preparing for future template syntaxes.



## Logical Names

Physical paths are converted into **logical names**, which are used when invoking templates via CLI.

Rules:

1. Remove `templates_dir` prefix  
2. Strip the `.md` extension  
3. Use `/` as separator  

Example:

Vault file:

```
Notes/.markadd/templates/blog/post.md
```

Logical name:

```
blog/post
```

This name is what appears in:

```
markadd list-templates
markadd new –template blog/post …
```



## Listing Templates

List the templates available in the active profile:

```
markadd list-templates
```

Example output:

```
blog/post
daily
– 2 templates –
```

You may switch profiles:

```
markadd –profile work list-templates
```



## Template Frontmatter

Templates can include YAML frontmatter to define metadata and default output paths.

### Output Path in Frontmatter

The `output` field defines where the rendered file will be created:

```markdown
---
output: "daily/{{date}}.md"
---
# Daily Note: {{date}}

## Tasks

- [ ]

## Notes

```

When using a template with an `output` field, the `--output` flag becomes optional:

```bash
# Uses output path from frontmatter
markadd new --template daily

# CLI flag overrides frontmatter
markadd new --template daily --output ~/Notes/custom.md
```

The output path supports variable substitution (`{{date}}`, `{{vault_root}}`, etc.) and is resolved relative to `vault_root`.

### Extra Frontmatter Fields

Other frontmatter fields are passed through to the rendered output:

```markdown
---
output: "posts/{{date}}-draft.md"
tags: [draft]
author: "{{user}}"
---
# New Post
```

## Variable Substitution

Templates support `{{variable}}` placeholders that are replaced at render time.

### Built-In Variables

| Variable | Example | Description |
|----------|---------|-------------|
| `{{date}}` | `2024-01-15` | Current date (YYYY-MM-DD) |
| `{{today}}` | `2024-01-15` | Alias for `{{date}}` |
| `{{time}}` | `14:30` | Current time (HH:MM) |
| `{{now}}` | `2024-01-15T14:30:00+00:00` | Alias for `{{datetime}}` |
| `{{datetime}}` | `2024-01-15T14:30:00+00:00` | ISO 8601 datetime |
| `{{vault_root}}` | `/home/user/vault` | Vault root path |
| `{{template_name}}` | `daily` | Logical name of the template |
| `{{template_path}}` | `/vault/.markadd/templates/daily.md` | Full path to template |
| `{{output_path}}` | `/vault/daily/2024-01-15.md` | Full output path |
| `{{output_filename}}` | `2024-01-15.md` | Output filename only |
| `{{output_dir}}` | `/vault/daily` | Output directory |

### Date Math Expressions

Date math allows you to calculate relative dates:

```markdown
Tomorrow: {{today + 1d}}
Yesterday: {{today - 1d}}
Next week: {{today + 1w}}
Next month: {{today + 1M}}
Next year: {{today + 1y}}
In 2 hours: {{now + 2h}}
30 minutes ago: {{now - 30m}}
```

**Weekday navigation:**

```markdown
Next Monday: {{today + monday}}
Last Friday: {{today - friday}}
```

**Custom formatting:**

```markdown
Day name: {{today | %A}}
Month name: {{today | %B}}
Full date: {{today | %Y-%m-%d}}
ISO week: {{today | %G-W%V}}
```

Supported units:
- `d` - days
- `w` - weeks
- `M` - months
- `y` - years
- `h` - hours
- `m` - minutes

Supported weekdays: `monday`, `tuesday`, `wednesday`, `thursday`, `friday`, `saturday`, `sunday`

### Example Template

```markdown
---
output: "meetings/{{date}}.md"
---
# Meeting Notes: {{date}}

**Time**: {{time}}
**File**: {{output_filename}}

## Attendees

-

## Agenda

1.

## Notes

## Action Items

- [ ]
```

## Rendering via CLI

Generate a new file from a template:

```bash
markadd new --template <name> --output <path>
```

Examples:

```bash
# Specify output path
markadd new --template daily --output ~/Notes/2025-01-15.md

# Use output path from template frontmatter
markadd new --template daily

# Use nested template
markadd new --template blog/post --output ~/Notes/posts/my-post.md
```

## Variable Metadata

Templates can define variables with prompts, defaults, and descriptions in the frontmatter.

### Simple Form

Just specify the prompt text:

```yaml
---
output: "notes/{{title}}.md"
vars:
  title: "Note title"
---
```

### Full Form

Specify complete metadata:

```yaml
---
output: "meetings/{{title}}.md"
vars:
  title:
    prompt: "Meeting title"
    required: true
  attendees:
    prompt: "Who's attending?"
    default: "TBD"
    description: "Comma-separated list of names"
  due:
    prompt: "Due date"
    default: "{{today + 7d}}"
---
```

### Metadata Fields

| Field | Description |
|-------|-------------|
| `prompt` | Text shown when prompting user for input |
| `default` | Default value if not provided (supports date math expressions) |
| `required` | If `true`, template fails in batch mode without this variable |
| `description` | Help text shown in interactive mode |

### Interactive vs Batch Mode

By default, markadd prompts for missing variables:

```bash
# Interactive - prompts for title if not provided
markadd new --template meeting
```

Use `--batch` to fail on missing required variables:

```bash
# Batch mode - uses defaults, fails if required vars missing
markadd new --template meeting --batch
```

## Future: Scripting Hooks

If security flags allow (`allow_shell = true`), templates may embed:

- pre-render hooks
- post-render hooks
- transformations

These features belong to later phases.



## Recommended Template Layouts

### Flat Layout

```
templates/
daily.md
weekly.md
notes.md
```

### Hierarchical Layout (namespaced)

```
templates/
daily/
work.md
personal.md
blog/
post.md
```

Logical names become:

```
daily/work
daily/personal
blog/post
```

### Mix and Match

```
templates/
daily.md
templates/journal/morning.md
templates/journal/evening.md
```



## Editing Templates

Because templates are plain Markdown:

- your editor already supports them  
- Obsidian, Neovim, VS Code, Emacs all work out of the box  
- version control is straightforward  
- they integrate naturally with vault-based workflows  



## Interaction with Other markadd Features

### Captures

Templates and captures are complementary:

- **templates** generate *new files*
- **captures** insert content into *existing files*
- later, **macros** will combine both

A common workflow:
1. Use `markadd new` to create a daily note from a template
2. Use `markadd capture` throughout the day to add content

### Macros (Future)

Macros will orchestrate:

1. running a template
2. inserting structured text
3. executing optional hooks

Templates become building blocks for higher-level automation.



## Summary

- Templates are Markdown files inside `templates_dir`
- Only `.md` files are treated as templates
- Nested folders produce namespaced logical names
- Templates support `{{variable}}` substitution with built-in and custom variables
- Date math expressions like `{{today + 1d}}` and `{{today | %A}}` are supported
- Optional frontmatter with `output` field defines default output path
- Variable metadata (prompts, defaults, descriptions) can be defined in frontmatter
- Interactive mode prompts for missing variables; `--batch` mode fails on missing required variables
- `markadd list-templates` shows what's available
- `markadd new --template <name>` renders a template to a file
- Future phases will add scripting hooks

