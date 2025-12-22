[ Back to devlogs index](../devlogs/)

# Phase 01 — Development Log  
## Configuration System

Phase 01 focused on implementing a robust, flexible configuration system capable of supporting multiple workflows and vault structures.



## Goals

1. Load configuration from a TOML file.  
2. Support profiles for different vaults or environments.  
3. Provide XDG integration and CLI override support.  
4. Implement interpolation for directory values.  
5. Validate configuration via a CLI command.



## What Was Implemented

### 1. config.toml Loader
Features implemented:

- `version` and `profile` selection  
- Multiple profiles under `[profiles.<name>]`  
- Expansion of:
  - `~` home directory  
  - environment variables (`$VAR`)  
  - interpolation (`{{vault_root}}`)  
- Absolute normalization of all directories  

### 2. XDG + CLI Overrides

Configuration is searched in order:

1. `--config <path>`  
2. `$XDG_CONFIG_HOME/markadd/config.toml`  
3. `~/.config/markadd/config.toml`

### 3. Directory Structure
Each profile defines:

- `vault_root`  
- `templates_dir`  
- `captures_dir`  
- `macros_dir`  

### 4. Security Flags
Added (but not yet enforced):

- `allow_shell`  
- `allow_http`

### 5. `markadd doctor`
A lightweight CLI command that prints the active profile and resolved paths, and validates the configuration.



## Outcome

We now have a predictable, explicit configuration system with excellent test coverage.  
This forms the foundation for all vault-specific features.
