# Macros Reference

Macros are multi-step workflows that combine templates, captures, and shell commands. They enable complex automation sequences to be executed with a single command.

## Overview

Macros are defined as YAML files in your `macros_dir` (typically `~/.markadd/macros/` or `{{vault_root}}/.markadd/macros/`).

## Macro Structure

```yaml
name: macro-name
description: Human-readable description

# Optional: Variable definitions with prompts and defaults
vars:
  variable_name:
    prompt: "Prompt shown to user"
    default: "default value"
    description: "Help text"

# Steps to execute in order
steps:
  - template: template-name
  - capture: capture-name
  - shell: "command"

# Optional: Error handling policy
on_error: abort  # or "continue"
```

## Step Types

### Template Step

Execute a template to create a new file:

```yaml
steps:
  - template: meeting-note
    output: "meetings/{{date}}.md"  # Optional: override output path
    with:                            # Optional: variable overrides
      title: "Weekly Sync"
```

If `output` is not specified, the template's frontmatter `output` is used.

### Capture Step

Execute a capture to insert content into an existing file:

```yaml
steps:
  - capture: inbox-add
    with:
      text: "{{item}}"
      priority: "high"
```

### Shell Step

Execute a shell command (requires `--trust` flag):

```yaml
steps:
  - shell: "git add {{file}}"
    description: Stage file in git
```

Shell steps support variable substitution. The description is shown to users when listing macros or when trust is required.

## Variables

### Defining Variables

Variables can be defined in two forms:

**Simple form** (just the prompt):

```yaml
vars:
  title: "Meeting title"
```

**Full form** (with metadata):

```yaml
vars:
  title:
    prompt: "Meeting title"
    required: true
  status:
    prompt: "Initial status"
    default: "planning"
    description: "One of: planning, active, completed"
```

### Variable Precedence

1. Command-line `--var` arguments (highest priority)
2. Step-level `with` overrides
3. Macro-level `vars` defaults
4. Built-in variables (`date`, `today`, etc.)

### Date Math in Variables

Defaults can use date math expressions:

```yaml
vars:
  due_date:
    prompt: "Due date"
    default: "{{today + 7d}}"
```

## Error Handling

The `on_error` field controls behavior when a step fails:

- `abort` (default): Stop execution immediately
- `continue`: Execute remaining steps

```yaml
on_error: continue

steps:
  - template: optional-note    # If this fails...
  - capture: always-log        # ...this still runs
```

## Trust and Security

Macros with shell steps require the `--trust` flag:

```bash
markadd macro deploy --trust
```

This is a security measure to prevent accidental execution of shell commands. When listing macros, those requiring trust are marked:

```
$ markadd macro --list
safe-macro    (2 steps)
deploy-macro  (3 steps) [requires --trust]
```

## Examples

### Weekly Review Macro

```yaml
name: weekly-review
description: Set up weekly review documents

vars:
  focus:
    prompt: "What's your focus this week?"
  week_of:
    prompt: "Week date"
    default: "{{today + monday}}"

steps:
  # Create weekly summary
  - template: weekly-summary
    with:
      title: "Week of {{week_of}}"
      focus: "{{focus}}"

  # Archive last week's tasks
  - capture: archive-completed-tasks

  # Create fresh task list
  - template: weekly-tasks
    output: "tasks/{{week_of}}.md"
```

### Meeting Setup Macro

```yaml
name: new-meeting
description: Create meeting note and log it

vars:
  title:
    prompt: "Meeting title"
  attendees:
    prompt: "Attendees"
    default: "Team"

steps:
  - template: meeting-note
    with:
      title: "{{title}}"
      attendees: "{{attendees}}"

  - capture: meeting-log
    with:
      meeting: "{{title}}"
```

### Git Commit Macro (with shell)

```yaml
name: commit-notes
description: Commit all note changes

vars:
  message:
    prompt: "Commit message"
    default: "Update notes"

steps:
  - shell: "git add ."
    description: Stage all changes

  - shell: "git commit -m '{{message}}'"
    description: Create commit
```

Run with:

```bash
markadd macro commit-notes --trust --var message="Daily update"
```

### Daily Standup Macro

```yaml
name: standup
description: Daily standup template and task review

vars:
  yesterday:
    prompt: "What did you do yesterday?"
  today:
    prompt: "What will you do today?"
  blockers:
    prompt: "Any blockers?"
    default: "None"

steps:
  - template: standup-note
    output: "standups/{{date}}.md"
    with:
      yesterday: "{{yesterday}}"
      today: "{{today}}"
      blockers: "{{blockers}}"

  - capture: standup-log
    with:
      date: "{{date}}"
```

## Running Macros

### Interactive Mode

```bash
markadd macro weekly-review
```

Prompts for any missing variables.

### With Variables

```bash
markadd macro weekly-review --var focus="Ship v2.0"
```

### Batch Mode

```bash
markadd macro weekly-review --var focus="Ship v2.0" --batch
```

Fails if any required variables are missing.

### With Trust

```bash
markadd macro deploy --trust
```

Required for macros with shell steps.

## TUI Support

Macros appear in the TUI palette under the "MACROS" section (yellow header). Macros with shell steps show a warning and redirect to CLI usage since shell execution requires explicit trust.

## Best Practices

1. **Use descriptions**: Always include a `description` for macros and shell steps
2. **Provide defaults**: Use sensible defaults to reduce prompting
3. **Minimize shell usage**: Only use shell steps when necessary
4. **Document variables**: Use the `description` field for complex variables
5. **Test incrementally**: Build macros step by step, testing each addition
