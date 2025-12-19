//! Application state and update logic following The Elm Architecture.

use std::collections::HashMap;
use std::path::PathBuf;

use markadd_core::captures::CaptureInfo;
use markadd_core::config::types::ResolvedConfig;
use markadd_core::macros::{requires_trust, MacroInfo};
use markadd_core::templates::discovery::TemplateInfo;
use markadd_core::templates::engine::build_minimal_context;
use markadd_core::templates::repository::TemplateRepository;
use markadd_core::vars::collect_all_variables;

/// Unified item that can be either a template, capture, or macro.
#[derive(Debug, Clone)]
pub enum PaletteItem {
    Template(TemplateInfo),
    Capture(CaptureInfo),
    Macro(MacroInfo),
}

impl PaletteItem {
    pub fn name(&self) -> &str {
        match self {
            PaletteItem::Template(t) => &t.logical_name,
            PaletteItem::Capture(c) => &c.logical_name,
            PaletteItem::Macro(m) => &m.logical_name,
        }
    }
}

/// Variable info with display metadata.
#[derive(Debug, Clone)]
pub struct VarInfo {
    /// Variable name.
    pub name: String,
    /// Prompt text to show user.
    pub prompt: Option<String>,
    /// Description of what this variable is for.
    pub description: Option<String>,
    /// Default value (pre-fills input).
    pub default: Option<String>,
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
    Macro { content: String, requires_trust: bool },
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

    /// All palette items (templates + captures + macros).
    pub items: Vec<PaletteItem>,

    /// Index where captures start in items list.
    pub captures_start_index: usize,

    /// Index where macros start in items list.
    pub macros_start_index: usize,

    /// Currently selected index in palette.
    pub selected: usize,

    /// Preview of currently selected item.
    pub preview: Preview,

    /// Variables required by current item (with metadata for prompts).
    pub required_var_infos: Vec<VarInfo>,

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
        macros: Vec<MacroInfo>,
    ) -> Self {
        let captures_start_index = templates.len();
        let macros_start_index = templates.len() + captures.len();

        let mut items: Vec<PaletteItem> =
            templates.into_iter().map(PaletteItem::Template).collect();
        items.extend(captures.into_iter().map(PaletteItem::Capture));
        items.extend(macros.into_iter().map(PaletteItem::Macro));

        let mut app = App {
            mode: Mode::Browse,
            config,
            items,
            captures_start_index,
            macros_start_index,
            selected: 0,
            preview: Preview::None,
            required_var_infos: Vec::new(),
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
                self.required_var_infos.clear();
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
                self.required_var_infos.clear();
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
        use markadd_core::macros::MacroRepository;

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
            PaletteItem::Macro(info) => {
                // Load macro to check if it requires trust
                let needs_trust = match MacroRepository::new(&self.config.macros_dir) {
                    Ok(repo) => match repo.get_by_name(&info.logical_name) {
                        Ok(loaded) => requires_trust(&loaded.spec),
                        Err(_) => false,
                    },
                    Err(_) => false,
                };
                match std::fs::read_to_string(&info.path) {
                    Ok(content) => {
                        self.preview =
                            Preview::Macro { content, requires_trust: needs_trust }
                    }
                    Err(e) => {
                        self.preview = Preview::Error(format!("Failed to read: {e}"))
                    }
                }
            }
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
                // Load template variables first
                match self.load_template_var_infos(&info.logical_name) {
                    Ok(var_infos) => {
                        self.required_var_infos = var_infos;
                        self.var_values.clear();
                        if self.required_var_infos.is_empty() {
                            // No vars needed, proceed to output path resolution
                            self.proceed_to_template_output();
                        } else {
                            // Pre-fill with default if available
                            if let Some(default) = &self.required_var_infos[0].default {
                                self.input_buffer = default.clone();
                            } else {
                                self.input_buffer.clear();
                            }
                            self.mode = Mode::Input { var_index: 0 };
                        }
                    }
                    Err(e) => {
                        self.status = Some(StatusMessage { text: e, is_error: true });
                        self.mode = Mode::Result;
                    }
                }
            }
            PaletteItem::Capture(info) => {
                // Load capture to extract required variables with metadata
                match self.load_capture_var_infos(&info.logical_name) {
                    Ok(var_infos) => {
                        self.required_var_infos = var_infos;
                        self.var_values.clear();
                        if self.required_var_infos.is_empty() {
                            // No vars needed, execute immediately
                            self.execute_capture();
                        } else {
                            // Pre-fill with default if available
                            if let Some(default) = &self.required_var_infos[0].default {
                                self.input_buffer = default.clone();
                            } else {
                                self.input_buffer.clear();
                            }
                            self.mode = Mode::Input { var_index: 0 };
                        }
                    }
                    Err(e) => {
                        self.status = Some(StatusMessage { text: e, is_error: true });
                        self.mode = Mode::Result;
                    }
                }
            }
            PaletteItem::Macro(info) => {
                // Load macro to extract required variables with metadata
                match self.load_macro_var_infos(&info.logical_name) {
                    Ok((var_infos, needs_trust)) => {
                        if needs_trust {
                            // Macros with shell commands aren't supported in TUI yet
                            self.status = Some(StatusMessage {
                                text: "Macro requires --trust flag. Use CLI: markadd macro --trust".to_string(),
                                is_error: true,
                            });
                            self.mode = Mode::Result;
                            return;
                        }
                        self.required_var_infos = var_infos;
                        self.var_values.clear();
                        if self.required_var_infos.is_empty() {
                            // No vars needed, execute immediately
                            self.execute_macro();
                        } else {
                            // Pre-fill with default if available
                            if let Some(default) = &self.required_var_infos[0].default {
                                self.input_buffer = default.clone();
                            } else {
                                self.input_buffer.clear();
                            }
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
        use markadd_core::templates::engine::render_string;

        let repo = TemplateRepository::new(&self.config.templates_dir)
            .map_err(|e| format!("Failed to load templates: {e}"))?;

        let loaded = repo
            .get_by_name(name)
            .map_err(|e| format!("Failed to load template: {e}"))?;

        let info = TemplateInfo {
            logical_name: loaded.logical_name.clone(),
            path: loaded.path.clone(),
        };

        // Build context with user variables for output path resolution
        let mut ctx = build_minimal_context(&self.config, &info);
        for (k, v) in &self.var_values {
            ctx.insert(k.clone(), v.clone());
        }

        // Check if template has output path in frontmatter
        if let Some(ref fm) = loaded.frontmatter {
            if let Some(ref output) = fm.output {
                let rendered = render_string(output, &ctx)
                    .map_err(|e| format!("Failed to render output path: {e}"))?;
                let path = self.config.vault_root.join(&rendered);
                return Ok(Some(path));
            }
        }

        Ok(None)
    }

    /// Proceed to output path resolution after collecting variables.
    fn proceed_to_template_output(&mut self) {
        let Some(PaletteItem::Template(info)) = self.items.get(self.selected) else {
            return;
        };

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
                if var_index < self.required_var_infos.len() {
                    let var_name = self.required_var_infos[var_index].name.clone();
                    self.var_values.insert(var_name, self.input_buffer.clone());

                    if var_index + 1 < self.required_var_infos.len() {
                        // Pre-fill next input with default if available
                        if let Some(default) =
                            &self.required_var_infos[var_index + 1].default
                        {
                            self.input_buffer = default.clone();
                        } else {
                            self.input_buffer.clear();
                        }
                        // More vars to collect
                        self.mode = Mode::Input { var_index: var_index + 1 };
                    } else {
                        // All vars collected, execute based on item type
                        self.input_buffer.clear();
                        match &self.items[self.selected] {
                            // Templates need output path resolution after vars
                            PaletteItem::Template(_) => self.proceed_to_template_output(),
                            PaletteItem::Capture(_) => self.execute_capture(),
                            PaletteItem::Macro(_) => self.execute_macro(),
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Load capture and extract user-defined variables with metadata.
    fn load_capture_var_infos(&self, name: &str) -> Result<Vec<VarInfo>, String> {
        use markadd_core::captures::CaptureRepository;

        let repo = CaptureRepository::new(&self.config.captures_dir)
            .map_err(|e| format!("Failed to load captures: {e}"))?;

        let loaded =
            repo.get_by_name(name).map_err(|e| format!("Failed to load capture: {e}"))?;

        // Get variable names from content/target/section
        let var_names = super::actions::extract_user_variables(&loaded.spec);

        // Enrich with metadata from vars spec
        let var_infos: Vec<VarInfo> = var_names
            .into_iter()
            .map(|name| {
                let (prompt, description, default) =
                    if let Some(vars_map) = &loaded.spec.vars {
                        if let Some(var_spec) = vars_map.get(&name) {
                            let prompt_text = var_spec.prompt();
                            // Only use prompt if non-empty (simple form has prompt, full form might have empty)
                            let prompt_opt = if prompt_text.is_empty() {
                                None
                            } else {
                                Some(prompt_text.to_string())
                            };
                            (
                                prompt_opt,
                                var_spec.description().map(|s| s.to_string()),
                                var_spec.default().map(|s| s.to_string()),
                            )
                        } else {
                            (None, None, None)
                        }
                    } else {
                        (None, None, None)
                    };
                VarInfo { name, prompt, description, default }
            })
            .collect();

        Ok(var_infos)
    }

    /// Load macro and extract user-defined variables with metadata.
    /// Returns (var_infos, needs_trust).
    fn load_macro_var_infos(&self, name: &str) -> Result<(Vec<VarInfo>, bool), String> {
        use markadd_core::macros::MacroRepository;

        let repo = MacroRepository::new(&self.config.macros_dir)
            .map_err(|e| format!("Failed to load macros: {e}"))?;

        let loaded =
            repo.get_by_name(name).map_err(|e| format!("Failed to load macro: {e}"))?;

        let needs_trust = requires_trust(&loaded.spec);

        // Get variables from macro spec
        let var_infos: Vec<VarInfo> = if let Some(vars_map) = &loaded.spec.vars {
            vars_map
                .iter()
                .map(|(name, spec)| {
                    let prompt_text = spec.prompt();
                    let prompt_opt = if prompt_text.is_empty() {
                        None
                    } else {
                        Some(prompt_text.to_string())
                    };
                    VarInfo {
                        name: name.clone(),
                        prompt: prompt_opt,
                        description: spec.description().map(|s| s.to_string()),
                        default: spec.default().map(|s| s.to_string()),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok((var_infos, needs_trust))
    }

    /// Load template and extract user-defined variables with metadata.
    fn load_template_var_infos(&self, name: &str) -> Result<Vec<VarInfo>, String> {
        let repo = TemplateRepository::new(&self.config.templates_dir)
            .map_err(|e| format!("Failed to load templates: {e}"))?;

        let loaded = repo
            .get_by_name(name)
            .map_err(|e| format!("Failed to load template: {e}"))?;

        // Collect variables from frontmatter vars and body content
        let all_vars = collect_all_variables(
            loaded.frontmatter.as_ref().and_then(|fm| fm.vars.as_ref()),
            &loaded.body,
        );

        // Convert to VarInfo with metadata
        let var_infos: Vec<VarInfo> = all_vars
            .into_iter()
            .map(|(name, spec_opt)| {
                if let Some(spec) = spec_opt {
                    let prompt_text = spec.prompt();
                    let prompt_opt = if prompt_text.is_empty() {
                        None
                    } else {
                        Some(prompt_text.to_string())
                    };
                    VarInfo {
                        name,
                        prompt: prompt_opt,
                        description: spec.description().map(|s| s.to_string()),
                        default: spec.default().map(|s| s.to_string()),
                    }
                } else {
                    // Variable found in content but not declared in frontmatter
                    VarInfo { name, prompt: None, description: None, default: None }
                }
            })
            .collect();

        Ok(var_infos)
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

    /// Execute macro workflow.
    fn execute_macro(&mut self) {
        let Some(PaletteItem::Macro(info)) = self.items.get(self.selected) else {
            return;
        };

        match super::actions::execute_macro(
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
    pub fn current_input_label(&self) -> Option<String> {
        match &self.mode {
            Mode::OutputPath => Some("Output path".to_string()),
            Mode::Input { var_index } => {
                self.required_var_infos.get(*var_index).map(|info| {
                    // Use prompt if available, otherwise variable name
                    info.prompt.clone().unwrap_or_else(|| info.name.clone())
                })
            }
            _ => None,
        }
    }

    /// Get current input description (if available).
    pub fn current_input_description(&self) -> Option<&str> {
        match &self.mode {
            Mode::Input { var_index } => self
                .required_var_infos
                .get(*var_index)
                .and_then(|info| info.description.as_deref()),
            _ => None,
        }
    }
}
