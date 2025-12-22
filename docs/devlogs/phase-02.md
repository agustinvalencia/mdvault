[ Back to devlogs index](../devlogs/)
# Phase 02 — Development Log  
## Template Discovery

Phase 02 implemented recursive template discovery under the configured `templates_dir`.



## Goals

1. Discover `.md` templates inside the vault.  
2. Exclude template-flavoured double extensions (`*.tpl.md`, `*.tmpl.md`).  
3. Produce logical names based on relative paths.  
4. Provide CLI access via `list-templates`.  
5. Ensure full unit + integration test coverage.



## What Was Implemented

### 1. Template Definition (Phase 02)
A template is any file:

- ending exactly with `.md`  
- not ending with `.tpl.md` or `.tmpl.md`  
- located recursively under `templates_dir`  

### 2. Template Discovery Engine
Added to `core/templates/discovery.rs`:

- canonical directory walking  
- filtering based on extension rules  
- converting paths to logical names  
- sorted results for deterministic output  

Example:

```
templates/blog/post.md → blog/post
templates/daily.md     → daily
```

### 3. CLI Command: `markadd list-templates`

The CLI now uses Clap and exposes:

```bash
markadd list-templates
```

Produces: 

```
blog/post
daily
– 2 templates –
```

### 4. Testing

**Core tests:**  
- inclusion/exclusion rules  
- path normalization  
- logical name computation  

**CLI integration tests:**  
- temporary vault  
- XDG overrides  
- stable output (NO_COLOR=1)  



## Outcome

Phase 02 delivered robust template discovery, completing the foundation required for Phase 03 (Template Engine MVP).
