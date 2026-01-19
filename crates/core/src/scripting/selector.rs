//! Interactive selector types for Lua scripting.
//!
//! This module provides types for interactive selection prompts
//! that can be triggered from Lua scripts via `mdv.selector()`.

use std::sync::Arc;

/// An item that can be displayed in a selector.
#[derive(Debug, Clone)]
pub struct SelectorItem {
    /// Display label shown to the user.
    pub label: String,
    /// Value returned when this item is selected.
    pub value: String,
    /// Optional description shown below the label.
    pub description: Option<String>,
}

impl SelectorItem {
    /// Create a new selector item.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self { label: label.into(), value: value.into(), description: None }
    }

    /// Create a selector item with description.
    pub fn with_description(
        label: impl Into<String>,
        value: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            description: Some(description.into()),
        }
    }
}

/// Options for the selector prompt.
#[derive(Debug, Clone, Default)]
pub struct SelectorOptions {
    /// Prompt text shown to the user.
    pub prompt: String,
    /// Whether fuzzy search is enabled.
    pub fuzzy: bool,
    /// Default selection index.
    pub default: Option<usize>,
    /// Whether to allow cancellation (returns None).
    pub allow_cancel: bool,
}

impl SelectorOptions {
    /// Create new selector options with a prompt.
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            fuzzy: true, // Enable fuzzy search by default
            default: Some(0),
            allow_cancel: true,
        }
    }

    /// Set whether fuzzy search is enabled.
    pub fn with_fuzzy(mut self, fuzzy: bool) -> Self {
        self.fuzzy = fuzzy;
        self
    }

    /// Set the default selection index.
    pub fn with_default(mut self, index: usize) -> Self {
        self.default = Some(index);
        self
    }
}

/// Type alias for the selector callback function.
///
/// The callback receives a list of items and options, and returns
/// the selected item's value (or None if cancelled).
pub type SelectorCallback =
    Arc<dyn Fn(&[SelectorItem], &SelectorOptions) -> Option<String> + Send + Sync>;

/// A no-op selector that always returns None.
/// Used when no interactive selector is available.
pub fn noop_selector(
    _items: &[SelectorItem],
    _options: &SelectorOptions,
) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selector_item_new() {
        let item = SelectorItem::new("Label", "value");
        assert_eq!(item.label, "Label");
        assert_eq!(item.value, "value");
        assert!(item.description.is_none());
    }

    #[test]
    fn test_selector_item_with_description() {
        let item = SelectorItem::with_description("Label", "value", "A description");
        assert_eq!(item.label, "Label");
        assert_eq!(item.value, "value");
        assert_eq!(item.description, Some("A description".to_string()));
    }

    #[test]
    fn test_selector_options_defaults() {
        let opts = SelectorOptions::new("Select an item");
        assert_eq!(opts.prompt, "Select an item");
        assert!(opts.fuzzy);
        assert_eq!(opts.default, Some(0));
        assert!(opts.allow_cancel);
    }

    #[test]
    fn test_selector_options_with_fuzzy() {
        let opts = SelectorOptions::new("Select").with_fuzzy(false);
        assert!(!opts.fuzzy);
    }

    #[test]
    fn test_selector_options_with_default() {
        let opts = SelectorOptions::new("Select").with_default(5);
        assert_eq!(opts.default, Some(5));
    }

    #[test]
    fn test_noop_selector() {
        let items = vec![
            SelectorItem::new("Item 1", "val1"),
            SelectorItem::new("Item 2", "val2"),
        ];
        let opts = SelectorOptions::new("Select");
        assert_eq!(noop_selector(&items, &opts), None);
    }

    #[test]
    fn test_selector_callback_type() {
        // Verify the callback type can be constructed
        let callback: SelectorCallback = Arc::new(|items, _opts| {
            // Return first item's value if available
            items.first().map(|i| i.value.clone())
        });

        let items = vec![SelectorItem::new("First", "first_value")];
        let opts = SelectorOptions::new("Test");
        let result = callback(&items, &opts);
        assert_eq!(result, Some("first_value".to_string()));
    }
}
