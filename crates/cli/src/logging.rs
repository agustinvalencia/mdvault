use mdvault_core::config::types::ResolvedConfig;
use std::fs::File;
use std::sync::Mutex;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

static LOG_GUARD: Mutex<Option<tracing_appender::non_blocking::WorkerGuard>> =
    Mutex::new(None);

pub fn init(cfg: &ResolvedConfig) {
    let stdout_level = parse_level(&cfg.logging.level).unwrap_or(LevelFilter::INFO);

    let stdout_filter =
        EnvFilter::builder().with_default_directive(stdout_level.into()).from_env_lossy();

    let stdout_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .with_target(false)
        .with_filter(stdout_filter);

    let registry = tracing_subscriber::registry().with(stdout_layer);

    if let Some(ref path) = cfg.logging.file {
        let file_level_str =
            cfg.logging.file_level.as_deref().unwrap_or(&cfg.logging.level);

        let file_level = parse_level(file_level_str).unwrap_or(LevelFilter::DEBUG);

        let file_filter = EnvFilter::builder()
            .with_default_directive(file_level.into())
            .from_env_lossy();

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

        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_file(true)
            .with_line_number(true)
            .with_filter(file_filter);

        registry.with(file_layer).init();
    } else {
        registry.init();
    }
}

fn parse_level(s: &str) -> Option<LevelFilter> {

    match s.to_lowercase().as_str() {

        "error" => Some(LevelFilter::ERROR),

        "warn" => Some(LevelFilter::WARN),

        "info" => Some(LevelFilter::INFO),

        "debug" => Some(LevelFilter::DEBUG),

        "trace" => Some(LevelFilter::TRACE),

        _ => None,

    }

}



#[cfg(test)]

mod tests {

    use super::*;

    use tracing_subscriber::filter::LevelFilter;



    #[test]

    fn test_parse_level() {

        assert_eq!(parse_level("error"), Some(LevelFilter::ERROR));

        assert_eq!(parse_level("WARN"), Some(LevelFilter::WARN));

        assert_eq!(parse_level("Info"), Some(LevelFilter::INFO));

        assert_eq!(parse_level("debug"), Some(LevelFilter::DEBUG));

        assert_eq!(parse_level("trace"), Some(LevelFilter::TRACE));

        assert_eq!(parse_level("invalid"), None);

        assert_eq!(parse_level(""), None);

    }

}
