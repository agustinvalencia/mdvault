//! Interactive dashboard TUI.
//!
//! Launched via `mdv report --dashboard --tui` or `mdv dashboard`.
//! Displays project progress, task breakdowns, and activity sparklines
//! using data from `DashboardReport`.

mod app;
mod event;
mod ui;

use std::io;
use std::path::Path;
use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::{
    event::{Event, poll, read},
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    },
};
use ratatui::prelude::*;

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::IndexDb;
use mdvault_core::paths::PathResolver;
use mdvault_core::report::{DashboardOptions, build_dashboard};

use app::DashboardApp;
use event::map_key_event;

/// Run the interactive dashboard TUI.
pub fn run(
    config_path: Option<&Path>,
    profile: Option<&str>,
    project: Option<&str>,
    activity_days: u32,
) -> Result<()> {
    let cfg = ConfigLoader::load(config_path, profile).map_err(|e| {
        color_eyre::eyre::eyre!("Configuration error: {e}\nRun 'mdv doctor' to diagnose.")
    })?;

    let index_path = PathResolver::new(&cfg.vault_root).index_db();
    let db = IndexDb::open(&index_path).map_err(|e| {
        color_eyre::eyre::eyre!("Failed to open index: {e}\nRun 'mdv reindex' first.")
    })?;

    let options = DashboardOptions {
        project: project.map(String::from),
        activity_days,
        ..Default::default()
    };

    let report = build_dashboard(&db, &options)
        .map_err(|e| color_eyre::eyre::eyre!("Failed to build dashboard: {e}"))?;

    let app = DashboardApp::new(
        report,
        cfg.vault_root.clone(),
        config_path.map(|p| p.to_path_buf()),
        profile.map(String::from),
    );

    let mut terminal = setup_terminal()?;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_app(&mut terminal, app)
    }));

    restore_terminal(&mut terminal)?;

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(color_eyre::eyre::eyre!("Dashboard panicked")),
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
    mut app: DashboardApp,
) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, &app))?;

        if poll(Duration::from_millis(100))?
            && let Event::Key(key) = read()?
            && let Some(msg) = map_key_event(&app, key)
        {
            app.update(msg);
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
