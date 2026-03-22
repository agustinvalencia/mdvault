//! Dashboard application state and update logic (Elm Architecture).

use mdvault_core::report::{DashboardReport, ProjectReport};

/// Which panel has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Projects,
    Detail,
}

/// Current operating mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Normal browsing.
    Browse,
    /// Showing a status message (press any key to dismiss).
    Status,
}

/// Messages that drive state updates.
#[derive(Debug, Clone)]
pub enum Message {
    // Navigation
    SelectNext,
    SelectPrev,
    SwitchPanel,

    // Actions
    GeneratePng,
    ExportToNote,
    Refresh,

    // System
    Quit,
    DismissStatus,
}

/// Feedback message.
pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
}

/// Main dashboard application state.
pub struct DashboardApp {
    pub report: DashboardReport,
    pub panel: Panel,
    pub project_index: usize,
    pub detail_scroll: usize,
    pub mode: Mode,
    pub status: Option<StatusMessage>,
    pub should_quit: bool,

    // Config for actions
    pub vault_root: std::path::PathBuf,
    pub config_path: Option<std::path::PathBuf>,
    pub profile: Option<String>,
}

impl DashboardApp {
    pub fn new(
        report: DashboardReport,
        vault_root: std::path::PathBuf,
        config_path: Option<std::path::PathBuf>,
        profile: Option<String>,
    ) -> Self {
        Self {
            report,
            panel: Panel::Projects,
            project_index: 0,
            detail_scroll: 0,
            mode: Mode::Browse,
            status: None,
            should_quit: false,
            vault_root,
            config_path,
            profile,
        }
    }

    /// Currently selected project (if any).
    pub fn selected_project(&self) -> Option<&ProjectReport> {
        self.report.projects.get(self.project_index)
    }

    /// Count of alerts for a given project ID.
    pub fn project_alert_count(&self, proj_id: &str) -> usize {
        self.report.overdue.iter().filter(|t| t.project == proj_id).count()
            + self
                .report
                .upcoming_deadlines
                .iter()
                .filter(|t| t.project == proj_id)
                .count()
            + self.report.high_priority.iter().filter(|t| t.project == proj_id).count()
            + self.report.zombie.iter().filter(|t| t.project == proj_id).count()
    }

    /// Process a message and update state.
    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::SelectNext => match self.panel {
                Panel::Projects => {
                    if self.project_index < self.report.projects.len().saturating_sub(1) {
                        self.project_index += 1;
                        self.detail_scroll = 0;
                    }
                }
                Panel::Detail => {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                }
            },
            Message::SelectPrev => match self.panel {
                Panel::Projects => {
                    self.project_index = self.project_index.saturating_sub(1);
                    self.detail_scroll = 0;
                }
                Panel::Detail => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                }
            },
            Message::SwitchPanel => {
                self.panel = match self.panel {
                    Panel::Projects => Panel::Detail,
                    Panel::Detail => Panel::Projects,
                };
            }
            Message::GeneratePng => {
                self.generate_png();
            }
            Message::ExportToNote => {
                self.export_to_note();
            }
            Message::Refresh => {
                self.refresh_report();
            }
            Message::Quit => {
                self.should_quit = true;
            }
            Message::DismissStatus => {
                self.status = None;
                self.mode = Mode::Browse;
            }
        }
    }

    fn generate_png(&mut self) {
        use crate::cmd::charts;

        let filename = match &self.report.scope {
            mdvault_core::report::ReportScope::Project { id, .. } => {
                format!("dashboard-{}.png", id.to_lowercase())
            }
            mdvault_core::report::ReportScope::Vault => "dashboard-vault.png".to_string(),
        };
        let png_path = self.vault_root.join("assets").join("dashboards").join(&filename);

        let is_project = matches!(
            self.report.scope,
            mdvault_core::report::ReportScope::Project { .. }
        );

        let result = if is_project {
            charts::generate_project_dashboard_png(&self.report, &png_path)
        } else {
            charts::generate_dashboard_png(&self.report, &png_path)
        };

        match result {
            Ok(()) => {
                let rel = png_path.strip_prefix(&self.vault_root).unwrap_or(&png_path);
                self.status = Some(StatusMessage {
                    text: format!("PNG saved: {}", rel.display()),
                    is_error: false,
                });
                self.mode = Mode::Status;
            }
            Err(e) => {
                self.status = Some(StatusMessage {
                    text: format!("PNG failed: {e}"),
                    is_error: true,
                });
                self.mode = Mode::Status;
            }
        }
    }

    fn export_to_note(&mut self) {
        let json = match serde_json::to_string_pretty(&self.report) {
            Ok(j) => j,
            Err(e) => {
                self.status = Some(StatusMessage {
                    text: format!("JSON serialisation failed: {e}"),
                    is_error: true,
                });
                self.mode = Mode::Status;
                return;
            }
        };

        let filename = match &self.report.scope {
            mdvault_core::report::ReportScope::Project { id, .. } => {
                format!("dashboard-{}.json", id.to_lowercase())
            }
            mdvault_core::report::ReportScope::Vault => {
                "dashboard-vault.json".to_string()
            }
        };
        let out_path = self.vault_root.join("assets").join("dashboards").join(&filename);

        if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match std::fs::write(&out_path, &json) {
            Ok(()) => {
                let rel = out_path.strip_prefix(&self.vault_root).unwrap_or(&out_path);
                self.status = Some(StatusMessage {
                    text: format!("Exported: {}", rel.display()),
                    is_error: false,
                });
                self.mode = Mode::Status;
            }
            Err(e) => {
                self.status = Some(StatusMessage {
                    text: format!("Export failed: {e}"),
                    is_error: true,
                });
                self.mode = Mode::Status;
            }
        }
    }

    fn refresh_report(&mut self) {
        use mdvault_core::config::loader::ConfigLoader;
        use mdvault_core::index::IndexDb;

        let cfg = match ConfigLoader::load(
            self.config_path.as_deref(),
            self.profile.as_deref(),
        ) {
            Ok(c) => c,
            Err(e) => {
                self.status = Some(StatusMessage {
                    text: format!("Config error: {e}"),
                    is_error: true,
                });
                self.mode = Mode::Status;
                return;
            }
        };

        let index_path = cfg.vault_root.join(".mdvault/index.db");
        let db = match IndexDb::open(&index_path) {
            Ok(db) => db,
            Err(e) => {
                self.status = Some(StatusMessage {
                    text: format!("Index error: {e}"),
                    is_error: true,
                });
                self.mode = Mode::Status;
                return;
            }
        };

        let project = match &self.report.scope {
            mdvault_core::report::ReportScope::Project { id, .. } => Some(id.clone()),
            mdvault_core::report::ReportScope::Vault => None,
        };

        let options = mdvault_core::report::DashboardOptions {
            project,
            activity_days: self.report.activity.period_days,
            ..Default::default()
        };

        match mdvault_core::report::build_dashboard(&db, &options) {
            Ok(r) => {
                self.report = r;
                self.status = Some(StatusMessage {
                    text: "Refreshed".to_string(),
                    is_error: false,
                });
                self.mode = Mode::Status;
            }
            Err(e) => {
                self.status = Some(StatusMessage {
                    text: format!("Refresh failed: {e}"),
                    is_error: true,
                });
                self.mode = Mode::Status;
            }
        }
    }
}
