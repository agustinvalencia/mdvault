# Daily/Weekly notes for future dates: title expression not fully evaluated

**GitHub Issue**: https://github.com/agustinvalencia/mdvault/issues/87

## Description

When creating daily or weekly notes using date math expressions (e.g., `today + 7d`, `today + 2w`), the date expression is correctly evaluated for:
- The frontmatter `date`/`week` field
- The frontmatter `title` field
- The output file path (when using built-in behavior without typedef)

However, the date expression is **NOT** evaluated for:
- The markdown heading (`# <title>`)
- The output file path when a Lua typedef uses `{{title}}` in its output template

## Repro steps

### Bug 1: Heading always shows raw expression

```shell
mdv new daily "today + 7d" --batch
```

**Expected output file** (`Journal/Daily/2026-01-28.md`):
```md
---
date: 2026-01-28
title: 2026-01-28
type: daily
---

# 2026-01-28
```

**Actual output file**:
```md
---
date: 2026-01-28
title: 2026-01-28
type: daily
---

# today + 7d
```

The frontmatter is correct, but the heading contains the raw expression.

### Bug 2: Output path uses raw expression when typedef has `{{title}}`

Given a `daily.lua` typedef:
```lua
return {
    output = "Journal/Daily/{{title}}.md",
    schema = {
        date = { type = "string", default_expr = "os.date('%Y-%m-%d')" }
    }
}
```

Running:
```shell
mdv new daily "today + 7d" --batch
```

**Expected**: Creates `Journal/Daily/2026-01-28.md`
**Actual**: Creates `Journal/Daily/today + 7d.md`

The file name is literally `today + 7d.md` instead of the evaluated date.

Note: Without the typedef, the built-in behavior correctly uses the evaluated date for the file path.

## Root cause analysis

### Bug 1 (Heading)

In `crates/core/src/types/scaffolding.rs:94`:
```rust
format!("---\n{}---\n\n# {}\n\n", yaml, title)
```

The `title` parameter is passed from `NoteCreator::generate_content()` as `ctx.title` (the raw input), not `ctx.core_metadata.title` (the evaluated date).

While `ensure_core_metadata()` later fixes the frontmatter title, the heading is never corrected.

### Bug 2 (Output path with typedef)

In `crates/core/src/domain/behaviors/mod.rs:48`:
```rust
render_ctx.insert("title".into(), ctx.title.clone());
```

This inserts the raw `ctx.title` into the render context, so `{{title}}` in the output template gets the unevaluated expression.

Note: The `date` variable IS correctly set from `ctx.core_metadata.date` at line 61, so using `{{date}}` in the typedef would work correctly.

## Same issue affects weekly notes

```shell
mdv new weekly "today + 2w" --batch
```

Produces:
- **File path**: `Journal/Weekly/2026-W06.md` (correct without typedef)
- **Frontmatter**: `week: 2026-W06`, `title: 2026-W06` (correct)
- **Heading**: `# today + 2w` (incorrect)

## Suggested fixes

### Fix for Bug 1 (Heading)

In `NoteCreator::generate_content()`, pass the evaluated title to `generate_scaffolding`:

```rust
// Use evaluated title from core_metadata if available
let title_for_scaffolding = ctx.core_metadata.title
    .as_ref()
    .unwrap_or(&ctx.title);

Ok(generate_scaffolding(
    &ctx.type_name,
    ctx.typedef.as_deref(),
    title_for_scaffolding,
    &ctx.vars,
))
```

### Fix for Bug 2 (Output path)

In `render_output_template()`, prefer `ctx.core_metadata.title` over `ctx.title`:

```rust
// Use evaluated title if available (e.g., for daily/weekly notes with date expressions)
if let Some(ref title) = ctx.core_metadata.title {
    render_ctx.insert("title".into(), title.clone());
} else {
    render_ctx.insert("title".into(), ctx.title.clone());
}
```

## Additional notes

- MCP tools (`add_to_daily_note`, `log_to_daily_note`) only work for today's date - there is no MCP tool for creating daily/weekly notes for arbitrary dates
- Direct ISO dates work correctly (e.g., `mdv new daily 2026-01-25 --batch`)
- The issue only manifests with date math expressions that need evaluation
