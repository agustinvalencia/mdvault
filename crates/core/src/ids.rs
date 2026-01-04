//! ID generation utilities for projects and tasks.
//!
//! Projects get a 3-letter ID derived from their title.
//! Tasks get the project ID + a 3-digit counter (e.g., "MCP-001").

/// Generate a project ID from a title.
///
/// Takes up to 3 meaningful letters from the title:
/// - If multiple words, takes first letter of each word (up to 3)
/// - If single word, takes first 3 letters
/// - Converts to uppercase
///
/// # Examples
/// ```
/// use mdvault_core::ids::generate_project_id;
///
/// assert_eq!(generate_project_id("My Cool Project"), "MCP");
/// assert_eq!(generate_project_id("Inventory"), "INV");
/// assert_eq!(generate_project_id("AI"), "AI");
/// assert_eq!(generate_project_id("a b c d e"), "ABC");
/// ```
pub fn generate_project_id(title: &str) -> String {
    let words: Vec<&str> = title.split_whitespace().filter(|w| !w.is_empty()).collect();

    let id = if words.len() >= 3 {
        // Take first letter of first 3 words
        words.iter().take(3).filter_map(|w| w.chars().next()).collect::<String>()
    } else if words.len() == 2 {
        // Two words: first letter of each, plus second letter of longer word
        let mut chars: Vec<char> =
            words.iter().filter_map(|w| w.chars().next()).collect();
        // Add one more char from the longer word
        let longer = if words[0].len() >= words[1].len() { words[0] } else { words[1] };
        if let Some(c) = longer.chars().nth(1) {
            chars.push(c);
        }
        chars.into_iter().collect()
    } else if words.len() == 1 {
        // Single word: take first 3 letters
        words[0].chars().take(3).collect()
    } else {
        // Empty title, generate placeholder
        "XXX".to_string()
    };

    id.to_uppercase()
}

/// Generate a task ID from a project ID and counter.
///
/// Format: `{project_id}-{counter:03}`
///
/// # Examples
/// ```
/// use mdvault_core::ids::generate_task_id;
///
/// assert_eq!(generate_task_id("MCP", 1), "MCP-001");
/// assert_eq!(generate_task_id("INV", 42), "INV-042");
/// assert_eq!(generate_task_id("AI", 999), "AI-999");
/// ```
pub fn generate_task_id(project_id: &str, counter: u32) -> String {
    format!("{}-{:03}", project_id, counter)
}

/// Parse a task ID to extract project ID and counter.
///
/// Returns None if the format is invalid.
///
/// # Examples
/// ```
/// use mdvault_core::ids::parse_task_id;
///
/// assert_eq!(parse_task_id("MCP-001"), Some(("MCP".to_string(), 1)));
/// assert_eq!(parse_task_id("INV-042"), Some(("INV".to_string(), 42)));
/// assert_eq!(parse_task_id("invalid"), None);
/// ```
pub fn parse_task_id(task_id: &str) -> Option<(String, u32)> {
    let parts: Vec<&str> = task_id.splitn(2, '-').collect();
    if parts.len() != 2 {
        return None;
    }

    let project_id = parts[0].to_string();
    let counter = parts[1].parse::<u32>().ok()?;

    Some((project_id, counter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_id_multiple_words() {
        assert_eq!(generate_project_id("My Cool Project"), "MCP");
        assert_eq!(generate_project_id("Knowledge Base System"), "KBS");
    }

    #[test]
    fn test_project_id_two_words() {
        // Two words: first letter of each + second letter of longer word
        assert_eq!(generate_project_id("Home Automation"), "HAU");
        assert_eq!(generate_project_id("AI Research"), "ARE");
    }

    #[test]
    fn test_project_id_single_word() {
        assert_eq!(generate_project_id("Inventory"), "INV");
        assert_eq!(generate_project_id("AI"), "AI");
        assert_eq!(generate_project_id("X"), "X");
    }

    #[test]
    fn test_project_id_empty() {
        assert_eq!(generate_project_id(""), "XXX");
        assert_eq!(generate_project_id("   "), "XXX");
    }

    #[test]
    fn test_project_id_lowercase() {
        assert_eq!(generate_project_id("my cool project"), "MCP");
    }

    #[test]
    fn test_task_id_generation() {
        assert_eq!(generate_task_id("MCP", 1), "MCP-001");
        assert_eq!(generate_task_id("MCP", 42), "MCP-042");
        assert_eq!(generate_task_id("MCP", 999), "MCP-999");
    }

    #[test]
    fn test_parse_task_id() {
        assert_eq!(parse_task_id("MCP-001"), Some(("MCP".to_string(), 1)));
        assert_eq!(parse_task_id("INV-042"), Some(("INV".to_string(), 42)));
        assert_eq!(parse_task_id("invalid"), None);
        assert_eq!(parse_task_id("MCP-abc"), None);
    }
}
