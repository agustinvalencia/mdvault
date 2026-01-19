//! Integration tests for the ContextManager.

use mdvault_core::context::{ContextManager, ContextState, FocusContext};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_context_manager_load_empty_vault() {
    let tmp = tempdir().unwrap();

    let manager = ContextManager::load(tmp.path()).unwrap();

    assert!(!manager.has_focus());
    assert!(manager.active_project().is_none());
    assert!(manager.focus().is_none());
}

#[test]
fn test_context_manager_set_focus() {
    let tmp = tempdir().unwrap();

    let mut manager = ContextManager::load(tmp.path()).unwrap();
    manager.set_focus("MDV").unwrap();

    assert!(manager.has_focus());
    assert_eq!(manager.active_project(), Some("MDV"));

    let focus = manager.focus().unwrap();
    assert_eq!(focus.project, "MDV");
    assert!(focus.started_at.is_some());
    assert!(focus.note.is_none());
}

#[test]
fn test_context_manager_set_focus_with_note() {
    let tmp = tempdir().unwrap();

    let mut manager = ContextManager::load(tmp.path()).unwrap();
    manager.set_focus_with_note("PROJ", "Working on feature X").unwrap();

    let focus = manager.focus().unwrap();
    assert_eq!(focus.project, "PROJ");
    assert_eq!(focus.note.as_deref(), Some("Working on feature X"));
}

#[test]
fn test_context_manager_clear_focus() {
    let tmp = tempdir().unwrap();

    let mut manager = ContextManager::load(tmp.path()).unwrap();
    manager.set_focus("MDV").unwrap();
    assert!(manager.has_focus());

    manager.clear_focus().unwrap();
    assert!(!manager.has_focus());
    assert!(manager.active_project().is_none());
}

#[test]
fn test_context_manager_persistence() {
    let tmp = tempdir().unwrap();

    // Set focus
    {
        let mut manager = ContextManager::load(tmp.path()).unwrap();
        manager.set_focus_with_note("TEST", "Persistence check").unwrap();
    }

    // Load again and verify
    {
        let manager = ContextManager::load(tmp.path()).unwrap();
        assert!(manager.has_focus());
        assert_eq!(manager.active_project(), Some("TEST"));

        let focus = manager.focus().unwrap();
        assert_eq!(focus.note.as_deref(), Some("Persistence check"));
    }
}

#[test]
fn test_context_manager_state_file_location() {
    let tmp = tempdir().unwrap();

    let mut manager = ContextManager::load(tmp.path()).unwrap();
    manager.set_focus("LOC").unwrap();

    let state_file = tmp.path().join(".mdvault/state/context.toml");
    assert!(state_file.exists());

    let content = fs::read_to_string(&state_file).unwrap();
    assert!(content.contains("project = \"LOC\""));
}

#[test]
fn test_context_manager_replace_focus() {
    let tmp = tempdir().unwrap();

    let mut manager = ContextManager::load(tmp.path()).unwrap();

    manager.set_focus("FIRST").unwrap();
    assert_eq!(manager.active_project(), Some("FIRST"));

    manager.set_focus("SECOND").unwrap();
    assert_eq!(manager.active_project(), Some("SECOND"));

    // Verify only one focus exists
    let state = manager.state();
    assert!(state.focus.is_some());
}

#[test]
fn test_context_state_json_serialization() {
    let state = ContextState { focus: Some(FocusContext::new("JSON")) };

    let json = serde_json::to_string(&state).unwrap();
    assert!(json.contains("\"project\":\"JSON\""));

    let parsed: ContextState = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.focus.unwrap().project, "JSON");
}

#[test]
fn test_context_state_toml_serialization() {
    let state =
        ContextState { focus: Some(FocusContext::with_note("TOML", "Test note")) };

    let toml = toml::to_string_pretty(&state).unwrap();
    assert!(toml.contains("project = \"TOML\""));
    assert!(toml.contains("note = \"Test note\""));

    let parsed: ContextState = toml::from_str(&toml).unwrap();
    let focus = parsed.focus.unwrap();
    assert_eq!(focus.project, "TOML");
    assert_eq!(focus.note.as_deref(), Some("Test note"));
}

#[test]
fn test_empty_state_serialization() {
    let state = ContextState::default();

    let toml = toml::to_string_pretty(&state).unwrap();
    let parsed: ContextState = toml::from_str(&toml).unwrap();

    assert!(parsed.focus.is_none());
}

#[test]
fn test_load_existing_state_file() {
    let tmp = tempdir().unwrap();

    // Create state directory and file manually
    let state_dir = tmp.path().join(".mdvault/state");
    fs::create_dir_all(&state_dir).unwrap();

    let state_content = r#"
[focus]
project = "EXISTING"
note = "Pre-existing state"
"#;
    fs::write(state_dir.join("context.toml"), state_content).unwrap();

    // Load and verify
    let manager = ContextManager::load(tmp.path()).unwrap();
    assert!(manager.has_focus());
    assert_eq!(manager.active_project(), Some("EXISTING"));

    let focus = manager.focus().unwrap();
    assert_eq!(focus.note.as_deref(), Some("Pre-existing state"));
}
