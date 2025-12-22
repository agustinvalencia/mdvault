//! TUI module for interactive mode.

mod actions;
mod app;
mod event;
mod ui;

use std::io;
use std::path::Path;
use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::{
    event::{poll, read, Event},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::prelude::*;

use mdvault_core::captures::CaptureRepository;
use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::macros::MacroRepository;
use mdvault_core::templates::repository::TemplateRepository;

use app::App;
use event::map_key_event;

/// Run the TUI application.
pub fn run(config_path: Option<&Path>, profile: Option<&str>) -> Result<()> {
    // Load config (fail fast if config broken)
    let config = ConfigLoader::load(config_path, profile).map_err(|e| {
        color_eyre::eyre::eyre!(
            "Configuration error: {e}\nRun 'mdv doctor' to diagnose."
        )
    })?;

    // Discover templates
    let templates = match TemplateRepository::new(&config.templates_dir) {
        Ok(repo) => repo.list_all().to_vec(),
        Err(e) => {
            eprintln!("Warning: Failed to load templates: {e}");
            Vec::new()
        }
    };

    // Discover captures
    let captures = match CaptureRepository::new(&config.captures_dir) {
        Ok(repo) => repo.list_all().to_vec(),
        Err(e) => {
            eprintln!("Warning: Failed to load captures: {e}");
            Vec::new()
        }
    };

    // Discover macros
    let macros = match MacroRepository::new(&config.macros_dir) {
        Ok(repo) => repo.list_all().to_vec(),
        Err(e) => {
            eprintln!("Warning: Failed to load macros: {e}");
            Vec::new()
        }
    };

    // Initialize app
    let app = App::new(config, templates, captures, macros);

    // Setup terminal
    let mut terminal = setup_terminal()?;

    // Run with cleanup on panic
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_app(&mut terminal, app)
    }));

    // Always restore terminal
    restore_terminal(&mut terminal)?;

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(color_eyre::eyre::eyre!("Application panicked")),
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
) -> Result<()> {
    loop {
        // 1. Render current state
        terminal.draw(|frame| ui::draw(frame, &app))?;

        // 2. Poll for events (with timeout for responsiveness)
        if poll(Duration::from_millis(100))? {
            if let Event::Key(key) = read()? {
                // 3. Map key event to message
                if let Some(msg) = map_key_event(&app, key) {
                    // 4. Process message
                    app.update(msg);
                }
            }
        }

        // 5. Check quit condition
        if app.should_quit {
            return Ok(());
        }
    }
}
