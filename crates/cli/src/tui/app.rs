//! Application state and update logic following The Elm Architecture.

use std::collections::HashMap;
use std::path::PathBuf;

use markadd_core::captures::CaptureInfo;
use markadd_core::config::types::ResolvedConfig;
use markadd_core::templates::discovery::TemplateInfo;
use markadd_core::templates::engine::{
    build_minimal_context, resolve_template_output_path,
};
use markadd_core::templates::repository::TemplateRepository;

/// Unified item that can be either a template or capture.
#[derive(Debug, Clone)]
pub enum PaletteItem {
    Template(TemplateInfo),
    Capture(CaptureInfo),
}

impl PaletteItem {
    pub fn name(&self) -> &str {
        match self {
            PaletteItem::Template(t) => &t.logical_name,
            PaletteItem::Capture(c) => &c.logical_name,
        }
    }
}

/// Current operating mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Browsing palette, selecting item.
    Browse,
    /// Entering output path for template.
    OutputPath,
    /// Entering variables for selected item.
    Input { var_index: usize },
    /// Showing result (success/error).
    Result,
}

/// Preview content for the selected item.
#[derive(Debug, Clone)]
pub enum Preview {
    None,
    Template { content: String },
    Capture { content: String },
    Error(String),
}

/// Feedback message to display in status bar.
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
}

/// Messages that drive state updates.
#[derive(Debug, Clone)]
pub enum Message {
    // Navigation
    SelectNext,
    SelectPrev,

    // Mode transitions
    Execute,
    Cancel,

    // Input handling
    InputChar(char),
    InputBackspace,
    InputSubmit,

    // System
    Quit,
    ClearStatus,
}

/// Main application state.
pub struct App {
    /// Operating mode.
    pub mode: Mode,

    /// Resolved configuration.
    pub config: ResolvedConfig,

    /// All palette items (templates + captures).
    pub items: Vec<PaletteItem>,

    /// Index where captures start in items list.
    pub captures_start_index: usize,

    /// Currently selected index in palette.
    pub selected: usize,

    /// Preview of currently selected item.
    pub preview: Preview,

    /// Variables required by current item.
    pub required_vars: Vec<String>,

    /// Variable values entered by user.
    pub var_values: HashMap<String, String>,

    /// Current input buffer (for variable/path entry).
    pub input_buffer: String,

    /// Status bar message.
    pub status: Option<StatusMessage>,

    /// Should quit.
    pub should_quit: bool,

    /// Resolved output path for template (from frontmatter or user input).
    pub resolved_output_path: Option<PathBuf>,
}

impl App {
    /// Create a new App with loaded config and discovered items.
    pub fn new(
        config: ResolvedConfig,
        templates: Vec<TemplateInfo>,
        captures: Vec<CaptureInfo>,
    ) -> Self {
        let captures_start_index = templates.len();

        let mut items: Vec<PaletteItem> =
            templates.into_iter().map(PaletteItem::Template).collect();
        items.extend(captures.into_iter().map(PaletteItem::Capture));

        let mut app = App {
            mode: Mode::Browse,
            config,
            items,
            captures_start_index,
            selected: 0,
            preview: Preview::None,
            required_vars: Vec::new(),
            var_values: HashMap::new(),
            input_buffer: String::new(),
            status: None,
            should_quit: false,
            resolved_output_path: None,
        };

        // Load preview for first item if any
        app.load_preview();
        app
    }

    /// Process a message and update state.
    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::SelectNext => {
                if self.selected < self.items.len().saturating_sub(1) {
                    self.selected += 1;
                    self.load_preview();
                }
            }
            Message::SelectPrev => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.load_preview();
                }
            }
            Message::Execute => {
                self.start_execution();
            }
            Message::Cancel => {
                self.mode = Mode::Browse;
                self.input_buffer.clear();
                self.required_vars.clear();
                self.var_values.clear();
                self.resolved_output_path = None;
            }
            Message::InputChar(c) => {
                self.input_buffer.push(c);
            }
            Message::InputBackspace => {
                self.input_buffer.pop();
            }
            Message::InputSubmit => {
                self.submit_input();
            }
            Message::ClearStatus => {
                self.status = None;
                self.mode = Mode::Browse;
                self.input_buffer.clear();
                self.required_vars.clear();
                self.var_values.clear();
                self.resolved_output_path = None;
            }
            Message::Quit => {
                self.should_quit = true;
            }
        }
    }

    /// Load preview for currently selected item.
    pub fn load_preview(&mut self) {
        if self.items.is_empty() {
            self.preview = Preview::None;
            return;
        }

        let item = &self.items[self.selected];
        match item {
            PaletteItem::Template(info) => match std::fs::read_to_string(&info.path) {
                Ok(content) => self.preview = Preview::Template { content },
                Err(e) => self.preview = Preview::Error(format!("Failed to read: {e}")),
            },
            PaletteItem::Capture(info) => match std::fs::read_to_string(&info.path) {
                Ok(content) => self.preview = Preview::Capture { content },
                Err(e) => self.preview = Preview::Error(format!("Failed to read: {e}")),
            },
        }
    }

    /// Start execution workflow for selected item.
    fn start_execution(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let item = &self.items[self.selected];
        match item {
            PaletteItem::Template(info) => {
                // Try to resolve output path from frontmatter
                match self.resolve_template_output(&info.logical_name) {
                    Ok(Some(path)) => {
                        // Template has frontmatter output, execute directly
                        self.resolved_output_path = Some(path);
                        self.execute_template();
                    }
                    Ok(None) => {
                        // No frontmatter output, prompt user
                        self.resolved_output_path = None;
                        self.input_buffer.clear();
                        self.mode = Mode::OutputPath;
                    }
                    Err(e) => {
                        self.status = Some(StatusMessage { text: e, is_error: true });
                        self.mode = Mode::Result;
                    }
                }
            }
            PaletteItem::Capture(info) => {
                // Load capture to extract required variables
                match self.load_capture_vars(&info.logical_name) {
                    Ok(vars) => {
                        self.required_vars = vars;
                        self.var_values.clear();
                        if self.required_vars.is_empty() {
                            // No vars needed, execute immediately
                            self.execute_capture();
                        } else {
                            self.input_buffer.clear();
                            self.mode = Mode::Input { var_index: 0 };
                        }
                    }
                    Err(e) => {
                        self.status = Some(StatusMessage { text: e, is_error: true });
                        self.mode = Mode::Result;
                    }
                }
            }
        }
    }

    /// Try to resolve template output path from frontmatter.
    fn resolve_template_output(&self, name: &str) -> Result<Option<PathBuf>, String> {
        let repo = TemplateRepository::new(&self.config.templates_dir)
            .map_err(|e| format!("Failed to load templates: {e}"))?;

        let loaded = repo
            .get_by_name(name)
            .map_err(|e| format!("Failed to load template: {e}"))?;

        let info = TemplateInfo {
            logical_name: loaded.logical_name.clone(),
            path: loaded.path.clone(),
        };

        let ctx = build_minimal_context(&self.config, &info);

        resolve_template_output_path(&loaded, &self.config, &ctx)
            .map_err(|e| format!("Failed to resolve output path: {e}"))
    }

    /// Submit current input and advance to next step.
    fn submit_input(&mut self) {
        match &self.mode {
            Mode::OutputPath => {
                if self.input_buffer.is_empty() {
                    return;
                }
                // Convert input to absolute path and execute
                let output_path = PathBuf::from(&self.input_buffer);
                let output_path = if output_path.is_absolute() {
                    output_path
                } else {
                    self.config.vault_root.join(&output_path)
                };
                self.resolved_output_path = Some(output_path);
                self.execute_template();
            }
            Mode::Input { var_index } => {
                let var_index = *var_index;
                if var_index < self.required_vars.len() {
                    let var_name = self.required_vars[var_index].clone();
                    self.var_values.insert(var_name, self.input_buffer.clone());
                    self.input_buffer.clear();

                    if var_index + 1 < self.required_vars.len() {
                        // More vars to collect
                        self.mode = Mode::Input { var_index: var_index + 1 };
                    } else {
                        // All vars collected, execute
                        self.execute_capture();
                    }
                }
            }
            _ => {}
        }
    }

    /// Load capture and extract user-defined variables.
    fn load_capture_vars(&self, name: &str) -> Result<Vec<String>, String> {
        use markadd_core::captures::CaptureRepository;

        let repo = CaptureRepository::new(&self.config.captures_dir)
            .map_err(|e| format!("Failed to load captures: {e}"))?;

        let loaded =
            repo.get_by_name(name).map_err(|e| format!("Failed to load capture: {e}"))?;

        Ok(super::actions::extract_user_variables(&loaded.spec))
    }

    /// Execute template creation.
    fn execute_template(&mut self) {
        let Some(PaletteItem::Template(info)) = self.items.get(self.selected) else {
            return;
        };

        let Some(output_path) = self.resolved_output_path.take() else {
            self.status = Some(StatusMessage {
                text: "No output path resolved".to_string(),
                is_error: true,
            });
            self.mode = Mode::Result;
            return;
        };

        match super::actions::execute_template(
            &self.config,
            &info.logical_name,
            &output_path,
        ) {
            Ok(msg) => {
                self.status = Some(StatusMessage { text: msg, is_error: false });
            }
            Err(msg) => {
                self.status = Some(StatusMessage { text: msg, is_error: true });
            }
        }
        self.mode = Mode::Result;
        self.input_buffer.clear();
    }

    /// Execute capture insertion.
    fn execute_capture(&mut self) {
        let Some(PaletteItem::Capture(info)) = self.items.get(self.selected) else {
            return;
        };

        match super::actions::execute_capture(
            &self.config,
            &info.logical_name,
            &self.var_values,
        ) {
            Ok(msg) => {
                self.status = Some(StatusMessage { text: msg, is_error: false });
            }
            Err(msg) => {
                self.status = Some(StatusMessage { text: msg, is_error: true });
            }
        }
        self.mode = Mode::Result;
    }

    /// Get current input prompt label.
    pub fn current_input_label(&self) -> Option<&str> {
        match &self.mode {
            Mode::OutputPath => Some("Output path"),
            Mode::Input { var_index } => {
                self.required_vars.get(*var_index).map(|s| s.as_str())
            }
            _ => None,
        }
    }
}
