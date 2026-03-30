//! Centralised vault path resolution.
//!
//! All vault structure conventions live here. Behaviours, services, and CLI
//! commands use `PathResolver` instead of hardcoded `format!()` strings.

use std::path::{Path, PathBuf};

/// Resolves vault paths from a root directory.
///
/// Lightweight borrowed struct — construct locally, use, drop.
pub struct PathResolver<'a> {
    vault_root: &'a Path,
}

impl<'a> PathResolver<'a> {
    pub fn new(vault_root: &'a Path) -> Self {
        Self { vault_root }
    }

    // ── Note paths ───────────────────────────────────────────────────────

    /// `Inbox/{id}.md`
    pub fn inbox_task(&self, id: &str) -> PathBuf {
        self.vault_root.join(format!("Inbox/{id}.md"))
    }

    /// `Projects/{project}/Tasks/{id}.md`
    pub fn project_task(&self, project: &str, id: &str) -> PathBuf {
        self.vault_root.join(format!("Projects/{project}/Tasks/{id}.md"))
    }

    /// `Projects/{project}`
    pub fn project_dir(&self, project: &str) -> PathBuf {
        self.vault_root.join(format!("Projects/{project}"))
    }

    /// `Projects/{project}/{project}.md`
    pub fn project_note(&self, project: &str) -> PathBuf {
        self.vault_root.join(format!("Projects/{project}/{project}.md"))
    }

    /// `Projects/_archive/{project}/{project}.md`
    pub fn archive_project_note(&self, project: &str) -> PathBuf {
        self.vault_root.join(format!("Projects/_archive/{project}/{project}.md"))
    }

    /// `Journal/{year}/Daily/{date}.md` — `date` must be `YYYY-MM-DD`.
    pub fn daily_note(&self, date: &str) -> PathBuf {
        let year = &date[..4];
        self.vault_root.join(format!("Journal/{year}/Daily/{date}.md"))
    }

    /// `Journal/{year}/Weekly/{week}.md` — `week` must be `YYYY-Wxx`.
    pub fn weekly_note(&self, week: &str) -> PathBuf {
        let year = &week[..4];
        self.vault_root.join(format!("Journal/{year}/Weekly/{week}.md"))
    }

    /// `Meetings/{year}/{id}.md` — extracts year from `date` (`YYYY-MM-DD`).
    pub fn meeting_note(&self, date: &str, id: &str) -> PathBuf {
        let year = &date[..4];
        self.vault_root.join(format!("Meetings/{year}/{id}.md"))
    }

    /// `zettels/{slug}.md`
    pub fn zettel(&self, slug: &str) -> PathBuf {
        self.vault_root.join(format!("zettels/{slug}.md"))
    }

    /// `{type_name}s/{slug}.md` — fallback for custom types.
    pub fn custom_type(&self, type_name: &str, slug: &str) -> PathBuf {
        self.vault_root.join(format!("{type_name}s/{slug}.md"))
    }

    // ── Meetings directory ───────────────────────────────────────────────

    /// `Meetings/{year}` — for scanning existing meeting IDs.
    pub fn meetings_dir(&self, year: &str) -> PathBuf {
        self.vault_root.join(format!("Meetings/{year}"))
    }

    // ── System paths ─────────────────────────────────────────────────────

    /// `.mdvault/index.db`
    pub fn index_db(&self) -> PathBuf {
        self.vault_root.join(".mdvault/index.db")
    }

    /// `.mdvault/state`
    pub fn state_dir(&self) -> PathBuf {
        self.vault_root.join(".mdvault/state")
    }

    /// `.mdvault/state/context.toml`
    pub fn state_file(&self) -> PathBuf {
        self.vault_root.join(".mdvault/state/context.toml")
    }

    /// `.mdvault/activity.jsonl`
    pub fn activity_log(&self) -> PathBuf {
        self.vault_root.join(".mdvault/activity.jsonl")
    }

    /// `.mdvault/activity_archive`
    pub fn activity_archive_dir(&self) -> PathBuf {
        self.vault_root.join(".mdvault/activity_archive")
    }

    // ── Path predicates ──────────────────────────────────────────────────

    /// Check whether a task path belongs to a given project folder.
    ///
    /// Matches both active (`Projects/{folder}/`) and archived
    /// (`Projects/_archive/{folder}/`) paths.
    pub fn is_project_task(task_path: &str, project_folder: &str) -> bool {
        task_path.contains(&format!("Projects/{project_folder}/"))
            || task_path.contains(&format!("Projects/_archive/{project_folder}/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn resolver() -> PathResolver<'static> {
        PathResolver::new(Path::new("/vault"))
    }

    #[test]
    fn inbox_task_path() {
        assert_eq!(
            resolver().inbox_task("INB-001"),
            Path::new("/vault/Inbox/INB-001.md")
        );
    }

    #[test]
    fn project_task_path() {
        assert_eq!(
            resolver().project_task("my-proj", "MP-001"),
            Path::new("/vault/Projects/my-proj/Tasks/MP-001.md")
        );
    }

    #[test]
    fn project_dir_path() {
        assert_eq!(
            resolver().project_dir("my-proj"),
            Path::new("/vault/Projects/my-proj")
        );
    }

    #[test]
    fn project_note_path() {
        assert_eq!(
            resolver().project_note("my-proj"),
            Path::new("/vault/Projects/my-proj/my-proj.md")
        );
    }

    #[test]
    fn archive_project_note_path() {
        assert_eq!(
            resolver().archive_project_note("old-proj"),
            Path::new("/vault/Projects/_archive/old-proj/old-proj.md")
        );
    }

    #[test]
    fn daily_note_path() {
        assert_eq!(
            resolver().daily_note("2026-03-15"),
            Path::new("/vault/Journal/2026/Daily/2026-03-15.md")
        );
    }

    #[test]
    fn weekly_note_path() {
        assert_eq!(
            resolver().weekly_note("2026-W13"),
            Path::new("/vault/Journal/2026/Weekly/2026-W13.md")
        );
    }

    #[test]
    fn meeting_note_path() {
        assert_eq!(
            resolver().meeting_note("2026-01-15", "MTG-2026-01-15-001"),
            Path::new("/vault/Meetings/2026/MTG-2026-01-15-001.md")
        );
    }

    #[test]
    fn zettel_path() {
        assert_eq!(
            resolver().zettel("my-knowledge-note"),
            Path::new("/vault/zettels/my-knowledge-note.md")
        );
    }

    #[test]
    fn custom_type_path() {
        assert_eq!(
            resolver().custom_type("contact", "john-doe"),
            Path::new("/vault/contacts/john-doe.md")
        );
    }

    #[test]
    fn index_db_path() {
        assert_eq!(resolver().index_db(), Path::new("/vault/.mdvault/index.db"));
    }

    #[test]
    fn state_paths() {
        assert_eq!(resolver().state_dir(), Path::new("/vault/.mdvault/state"));
        assert_eq!(
            resolver().state_file(),
            Path::new("/vault/.mdvault/state/context.toml")
        );
    }

    #[test]
    fn activity_paths() {
        assert_eq!(
            resolver().activity_log(),
            Path::new("/vault/.mdvault/activity.jsonl")
        );
        assert_eq!(
            resolver().activity_archive_dir(),
            Path::new("/vault/.mdvault/activity_archive")
        );
    }

    #[test]
    fn is_project_task_active() {
        assert!(PathResolver::is_project_task(
            "Projects/my-proj/Tasks/MP-001.md",
            "my-proj"
        ));
    }

    #[test]
    fn is_project_task_archived() {
        assert!(PathResolver::is_project_task(
            "Projects/_archive/my-proj/Tasks/MP-001.md",
            "my-proj"
        ));
    }

    #[test]
    fn is_project_task_wrong_project() {
        assert!(!PathResolver::is_project_task(
            "Projects/other/Tasks/MP-001.md",
            "my-proj"
        ));
    }

    #[test]
    fn is_project_task_inbox() {
        assert!(!PathResolver::is_project_task("Inbox/INB-001.md", "my-proj"));
    }

    #[test]
    fn meetings_dir_path() {
        assert_eq!(resolver().meetings_dir("2026"), Path::new("/vault/Meetings/2026"));
    }

    #[test]
    fn is_project_task_not_confused_by_substring() {
        // "proj" should not match "my-proj"
        assert!(!PathResolver::is_project_task(
            "Projects/my-proj/Tasks/MP-001.md",
            "proj"
        ));
    }
}
