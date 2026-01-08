# Work Log - 2026-01-07

## Feature: Configurable Verbosity Logging

Implemented a configurable logging system for the CLI.

### Changes
- **Core**: Added `logging` section to `config.toml` via `LoggingConfig` struct.
  - `level`: Log level (error, warn, info, debug, trace). Default: "info".
  - `file`: Optional path to a log file. If omitted, logs to stderr.
- **CLI**:
  - Integrated `tracing` ecosystem (`tracing`, `tracing-subscriber`, `tracing-appender`).
  - Added `logging` module to initialize the subscriber early in `main`.
  - Configured formatting:
    - Files: Non-blocking, no ANSI colors, includes file/line number.
    - Stdout: ANSI colors enabled.
- **Testing**: Added `crates/cli/tests/logging_config.rs` to verify configuration parsing and file creation.

### Configuration Example
```toml
[logging]
level = "debug"
file = "/path/to/mdvault.log"
```
