# Logging in mdvault

mdvault uses the `tracing` ecosystem for structured logging. This allows for flexible configuration of log levels and output destinations (sinks).

## Configuration

Logging is configured in your `mdvault.toml` configuration file (typically located at `~/.config/mdvault/mdvault.toml` or `$XDG_CONFIG_HOME/mdvault/mdvault.toml`).

The `[logging]` section controls the behavior:

```toml
[logging]
# Global log level for console output.
# Options: "error", "warn", "info", "debug", "trace"
# Default: "info"
level = "info"

# Path to the log file. If omitted, file logging is disabled.
file = "/path/to/mdvault.log"
# OR use a relative path (relative to where you run the command)
# file = "mdvault.log"

# Log level specifically for the file output.
# If omitted, defaults to the global 'level'.
# This allows you to have quiet console output but verbose file logs.
file_level = "debug"
```

## Sinks

The application supports two primary sinks:

1.  **Console (Stderr)**:
    -   Active by default.
    -   Uses ANSI colors.
    -   Writes to standard error (`stderr`) to avoid interfering with command output (which often goes to `stdout`, especially for commands like `mdv template` or `mdv export`).
    -   Controlled by `logging.level`.

2.  **File**:
    -   Active only if `logging.file` is set.
    -   Writes structured, non-colored logs in **append mode** (preserving historical data).
    -   Includes file paths and line numbers for debugging.
    -   Controlled by `logging.file_level` (falls back to `logging.level` if unset).

## Adding Logs (Rust)

To add logs in the Rust codebase, use the `tracing` macros.

1.  **Import logging macros**:
    ```rust
    use tracing::{error, warn, info, debug, trace, instrument};
    ```

2.  **Emit logs**:
    ```rust
    // info is for general operational events
    info!("Starting reindex operation");

    // debug is for detailed information useful for debugging
    debug!("Processing file: {}", path.display());

    // warn is for non-fatal issues
    warn!("Could not parse frontmatter in {}, skipping", path.display());

    // error is for fatal or serious errors
    error!("Database corruption detected: {}", e);
    ```

3.  **Instrumentation (Optional)**:
    Use `#[instrument]` to automatically log function entry/exit and arguments.
    ```rust
    #[instrument(skip(self))]
    pub fn process(&self, data: &str) {
        // ...
    }
    ```

## Logging from Lua

Currently, **Lua scripts do not have direct access to the application's structured logging system**. You cannot write directly to the log file defined in `mdvault.toml` from a Lua script.

However, you have two alternatives:

### 1. Console Output (`print`)
You can use the standard Lua `print` function. This writes directly to **standard output (`stdout`)**.

```lua
print("Debug: variable x = " .. x)
```

> **Note**: This bypasses the `tracing` system. It will appear in your terminal but will **not** appear in the configured log file.

### 2. Logging to Notes (`mdv.capture`)
For persistent logs within your vault (e.g., an "Activity Log" or "Daily Note"), use `mdv.capture`. This is the idiomatic way to log "user-facing" events.

```lua
-- Log an event to today's daily note
local ok, err = mdv.capture("log-to-daily", {
    text = "Ran custom automation script at " .. mdv.date("now")
})
```
