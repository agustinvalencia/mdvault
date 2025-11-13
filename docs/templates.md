# Templates in markadd

This document describes how templates work in `markadd`:  
how they are discovered, how they are named, and how they will be rendered once the Template Engine MVP (Phase 03) arrives.

Currently (after Phase 02), templates are **discovered only**, not yet rendered.  
This guide covers both current behaviour and upcoming features.



## Overview

A *template* in `markadd` is a Markdown file stored inside the profile’s configured `templates_dir`.  
Templates are discovered recursively and exposed through the CLI:

markadd list-templates

Each template has:

- a **physical path** (inside your vault)  
- a **logical name** (used in CLI commands)  

In later phases, templates will also support **variables**, **built-in functions**, and **output rendering**.



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



## What Counts as a Template (Phase 02)

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



## Template Engine (Phase 03 Preview)

Although template *rendering* is not implemented yet, here is what Phase 03 will introduce.

### 1. Variable Substitution

Templates will eventually support variable placeholders:

```
Title: {{title}}
Date: {{date}}
```

These will be replaced at render-time using:

- built-in variables (date, time, vault paths)  
- file-based variables (output filename, directory)  
- user-supplied variables via CLI prompts  

### 2. Built-In Variables (planned)

Examples (names tentative):

```
{{date}}
{{time}}
{{datetime}}
{{vault_root}}
{{template_name}}
{{output_path}}
{{output_filename}}
```

### 3. Rendering via CLI

The first CLI command of the Template Engine MVP:

```
markadd new –template  –output 
```

Example:

```
markadd new –template daily –output ~/Notes/2025-06-10.md
```

### 4. Future: User Prompts

Later, templates may declare variables that require user input, e.g.:

```
{{prompt:meeting_subject}}
```

Running a template containing prompts will ask:

```
Enter meeting_subject:

```
### 5. Future: Scripting Hooks

If security flags allow it:

```
[security]
allow_shell = true
```

templates may eventually embed:

- pre-render hooks  
- post-render hooks  
- transformations  

These features belong to later phases (captures and macros).



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

### Captures (Phase 5)

Templates and captures are separate:

- **templates** generate *new files*  
- **captures** insert content into *existing files*  
- later, **macros** will combine both  

### Macros (Phase 6)

Macros will orchestrate:

1. running a template  
2. inserting structured text  
3. executing optional hooks  

Templates become building blocks for higher-level automation.



## Summary

- Templates are Markdown files inside `templates_dir`.  
- Only `.md` files are treated as templates.  
- Nested folders produce namespaced logical names.  
- `markadd list-templates` shows what’s available.  
- Template rendering begins in Phase 03.  
- Future phases extend templates with variables, prompts, and scripting hooks.  

This system is intentionally simple now to support predictable growth later.

