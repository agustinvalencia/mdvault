# markadd Configuration Guide

This document describes how `markadd` loads, interprets, and validates its configuration.  
Configuration is stored in a `config.toml` file and supports multiple profiles, directory interpolation, XDG integration, and environment expansion.

The configuration system is implemented in the `markadd-core` crate and is used by all CLI commands.



## Configuration File Locations

`markadd` resolves configuration from the following locations, in order:

1. `--config <path>` (explicit override)
2. `$XDG_CONFIG_HOME/markadd/config.toml`
3. `~/.config/markadd/config.toml`

The first valid configuration file found is used.

You may use this structure to manage multiple independent vaults or environments.



## Example config.toml

```toml
version = 1
profile = “default”

[profiles.default]
vault_root   = “~/Notes”
templates_dir = “{{vault_root}}/.markadd/templates”
captures_dir  = “{{vault_root}}/.markadd/captures”
macros_dir    = “{{vault_root}}/.markadd/macros”

[security]
allow_shell = false
allow_http  = false
```



## Fields

### version
The schema version for the configuration file. Currently must be:

```toml
version = 1
```

### profile
The name of the active profile.  
A profile defines a set of directories for a specific environment or vault.

Example:

```toml
profile = “work”
```

You may override the active profile via:

```bash
markadd –profile work …
```



## Profiles

All user-defined profiles live inside a `[profiles.<name>]` table.

Example:

```toml
[profiles.default]
vault_root = “~/Notes”
templates_dir = “{{vault_root}}/.markadd/templates”
captures_dir  = “{{vault_root}}/.markadd/captures”
macros_dir    = “{{vault_root}}/.markadd/macros”

[profiles.work]
vault_root = “~/WorkNotes”
templates_dir = “{{vault_root}}/.markadd/templates”
captures_dir  = “{{vault_root}}/.markadd/captures”
macros_dir    = “{{vault_root}}/.markadd/macros”
```

Every profile must define:

```
vault_root
templates_dir
captures_dir
macros_dir
```

All four are expanded and resolved to absolute paths.



## Directory Expansion Rules

Directory fields may contain:

### 1. `~` home expansion

```
vault_root = “~/Notes”
```

expands to your home directory.

### 2. Environment variables

```
vault_root = “$HOME/Notes”
```

### 3. Interpolation of other fields
The syntax:

```
{{field_name}}
```

can reference any directory field in the same profile.

For example:

```
captures_dir = “{{vault_root}}/.markadd/captures”
```

Interpolation happens after environment and home expansion.

### 4. Absolute Normalization
All expanded paths are:

- canonicalized where possible
- cleaned of redundant components (`./`, `../`)
- resolved relative to the profile’s root

This ensures consistent comparison and matching.



## Security Options

Security settings apply globally (not per profile):

```toml
[security]
allow_shell = false
allow_http  = false
```

These do not affect early phases but will be used in:

- capture definitions (Phase 5)
- macro system (Phase 6)

If set to `false`, future features involving shell or HTTP calls will be disabled.



## CLI Interaction

### Overriding the profile

```bash
markadd –profile work doctor
```

### Overriding the config path

```bash
markadd –config ~/custom/config.toml list-templates
```

### Inspecting configuration

```bash
markadd doctor
```

This prints the resolved configuration including absolute directories and active profile.

Example output:

```text
OK   markadd doctor
path: ~/.config/markadd/config.toml
profile: default
vault_root: /home/user/Notes
templates_dir: /home/user/Notes/.markadd/templates
captures_dir: /home/user/Notes/.markadd/captures
macros_dir: /home/user/Notes/.markadd/macros
security.allow_shell: false
security.allow_http:  false
```



## Validation Rules

On load, `markadd` validates that:

- the file exists and parses as TOML  
- `version = 1`  
- the selected profile exists  
- all required fields are present  
- all directory paths expand to valid UTF-8 strings  
- `vault_root` exists (warning for non-existent directories is planned)

Any failure produces a user-friendly error and exits non-zero.



## Tips

### Switching between personal and work vaults
Create two profiles:

```toml
profile = “personal”

[profiles.personal]
vault_root = “~/Notes”
…

[profiles.work]
vault_root = “~/WorkNotes”
…
```

Then switch with:

```bash
markadd –profile work list-templates
```

### Custom location for config
Point markadd to a project-local config:

```bash
markadd –config ./config/markadd.toml doctor
```

### Using environment variables

```toml
vault_root = “$NOTES_HOME”
```



## Future Extensions

Upcoming phases will extend configuration to support:

- user-defined template variables  
- capture definitions stored under `.markadd/captures/*.yaml`  
- macro pipelines  
- shell or HTTP actions depending on security flags  
- shared variables across templates and captures  

These will not break the current configuration schema.



## Conclusion

The configuration system is the backbone of `markadd`.  
It provides deterministic directory resolution, flexible per-profile behaviour, and prepares the ground for templating, captures, and macro scripting.


