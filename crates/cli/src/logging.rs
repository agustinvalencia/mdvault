use mdvault_core::config::types::ResolvedConfig;
use std::fs::File;
use std::sync::Mutex;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

static LOG_GUARD: Mutex<Option<tracing_appender::non_blocking::WorkerGuard>> =
    Mutex::new(None);

pub fn init(cfg: &ResolvedConfig) {
    let level_filter = match cfg.logging.level.to_lowercase().as_str() {
        "error" => LevelFilter::ERROR,
        "warn" => LevelFilter::WARN,
        "info" => LevelFilter::INFO,
        "debug" => LevelFilter::DEBUG,
        "trace" => LevelFilter::TRACE,
        _ => LevelFilter::INFO,
    };

    let filter =
        EnvFilter::builder().with_default_directive(level_filter.into()).from_env_lossy(); // Allow RUST_LOG to override

    if let Some(ref path) = cfg.logging.file {
        // Log to file
        let file = File::create(path).unwrap_or_else(|e| {
            eprintln!("Failed to create log file {}: {}", path.display(), e);
            std::process::exit(1);
        });

        let (non_blocking, guard) = tracing_appender::non_blocking(file);

        // Store guard to keep file logger alive
        if let Ok(mut g) = LOG_GUARD.lock() {
            *g = Some(guard);
        }

        tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false) // Disable colors for files
                    .with_file(true)
                    .with_line_number(true),
            )
            .init();
    } else {
        // Log to stdout
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(std::io::stderr).with_ansi(true))
            .init();
    }
}
