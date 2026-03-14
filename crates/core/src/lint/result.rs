//! Lint result types for vault structural checking.

use serde::Serialize;

/// A single issue found by a lint check.
#[derive(Debug, Clone, Serialize)]
pub struct LintIssue {
    /// Relative path to the note.
    pub path: String,
    /// Line number where the issue was found (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Human-readable description of the issue.
    pub message: String,
    /// Suggested fix (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// Whether this issue can be auto-fixed.
    pub fixable: bool,
}

/// Results for a single check category.
#[derive(Debug, Clone, Serialize)]
pub struct CategoryReport {
    /// Machine-readable category name (e.g. "broken_references").
    pub name: String,
    /// Human-readable label (e.g. "Broken References").
    pub label: String,
    /// Errors found (structural problems).
    pub errors: Vec<LintIssue>,
    /// Warnings found (non-critical issues).
    pub warnings: Vec<LintIssue>,
}

impl CategoryReport {
    /// Create a new empty category report.
    pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Whether this category has any issues.
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty() && self.warnings.is_empty()
    }

    /// Total issue count.
    pub fn issue_count(&self) -> usize {
        self.errors.len() + self.warnings.len()
    }
}

/// Summary statistics for a lint run.
#[derive(Debug, Clone, Serialize)]
pub struct LintSummary {
    /// Total notes in the vault.
    pub total_notes: usize,
    /// Total errors across all categories.
    pub total_errors: usize,
    /// Total warnings across all categories.
    pub total_warnings: usize,
    /// Health score (0.0–1.0): notes without issues / total notes.
    pub health_score: f64,
    /// Whether a reindex was performed as part of the check.
    pub reindex_performed: bool,
}

/// Complete lint report aggregating all category results.
#[derive(Debug, Clone, Serialize)]
pub struct LintReport {
    /// Results per category.
    pub categories: Vec<CategoryReport>,
    /// Overall summary.
    pub summary: LintSummary,
}

impl LintReport {
    /// Whether the vault is completely clean (no errors or warnings).
    pub fn is_clean(&self) -> bool {
        self.summary.total_errors == 0 && self.summary.total_warnings == 0
    }

    /// Whether there are any errors (not just warnings).
    pub fn has_errors(&self) -> bool {
        self.summary.total_errors > 0
    }
}
