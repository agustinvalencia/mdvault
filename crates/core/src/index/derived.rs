//! Derived index computation.
//!
//! This module builds secondary indices from the primary note and link data:
//! - `temporal_activity`: When notes are referenced in daily notes
//! - `activity_summary`: Aggregated activity metrics per note
//! - `note_cooccurrence`: Notes that appear together in daily notes

use chrono::{Duration, NaiveDate, Utc};
use thiserror::Error;

use super::IndexError;
use super::db::IndexDb;

/// Errors that can occur during derived index computation.
#[derive(Debug, Error)]
pub enum DerivedError {
    #[error("Index database error: {0}")]
    Index(#[from] IndexError),

    #[error("Failed to parse date: {0}")]
    DateParse(String),
}

/// Statistics from derived index computation.
#[derive(Debug, Clone, Default)]
pub struct DerivedStats {
    /// Number of daily notes processed.
    pub dailies_processed: usize,
    /// Number of temporal activity records created.
    pub activity_records: usize,
    /// Number of activity summaries computed.
    pub summaries_computed: usize,
    /// Number of cooccurrence pairs found.
    pub cooccurrence_pairs: usize,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// Builder for computing derived indices.
pub struct DerivedIndexBuilder<'a> {
    db: &'a IndexDb,
}

impl<'a> DerivedIndexBuilder<'a> {
    /// Create a new derived index builder.
    pub fn new(db: &'a IndexDb) -> Self {
        Self { db }
    }

    /// Compute all derived indices.
    ///
    /// This should be called after the primary index is built/updated.
    pub fn compute_all(&self) -> Result<DerivedStats, DerivedError> {
        let start = std::time::Instant::now();
        let mut stats = DerivedStats::default();

        // Clear existing derived data
        self.db.clear_derived_tables()?;

        // Step 1: Build temporal activity from daily notes
        stats.dailies_processed = self.build_temporal_activity()?;

        // Step 2: Count activity records
        stats.activity_records = self.db.count_temporal_activity()? as usize;

        // Step 3: Compute activity summaries
        stats.summaries_computed = self.compute_activity_summaries()?;

        // Step 4: Compute cooccurrence matrix
        stats.cooccurrence_pairs = self.compute_cooccurrence()?;

        stats.duration_ms = start.elapsed().as_millis() as u64;
        Ok(stats)
    }

    /// Build temporal activity records from daily notes.
    ///
    /// For each daily note, finds all outgoing links and creates
    /// temporal_activity records linking the referenced note to the daily.
    fn build_temporal_activity(&self) -> Result<usize, DerivedError> {
        // Get all daily notes
        let dailies = self.db.get_notes_by_type("daily")?;
        let mut count = 0;

        for daily in &dailies {
            let daily_id = match daily.id {
                Some(id) => id,
                None => continue,
            };

            // Extract date from the daily note
            // Daily notes typically have date in frontmatter or path
            let activity_date = self.extract_daily_date(daily)?;

            // Get all outgoing links from this daily
            let links = self.db.get_outgoing_links(daily_id)?;

            for link in &links {
                // Skip self-references and unresolved links
                if link.target_id.is_none() {
                    continue;
                }

                let target_id = link.target_id.unwrap();
                if target_id == daily_id {
                    continue; // Skip self-links
                }

                // Create temporal activity record
                self.db.insert_temporal_activity(
                    target_id,
                    daily_id,
                    &activity_date,
                    link.context.as_deref(),
                )?;
            }

            count += 1;
        }

        Ok(count)
    }

    /// Extract the date from a daily note.
    fn extract_daily_date(
        &self,
        daily: &super::types::IndexedNote,
    ) -> Result<String, DerivedError> {
        // Try to get date from frontmatter first
        if let Some(ref fm_json) = daily.frontmatter_json
            && let Ok(fm) = serde_json::from_str::<serde_json::Value>(fm_json)
            && let Some(date) = fm.get("date").and_then(|v| v.as_str())
        {
            return Ok(date.to_string());
        }

        // Fall back to extracting date from path (e.g., "daily/2025-01-15.md")
        let path_str = daily.path.to_string_lossy();
        if let Some(date_str) = extract_date_from_path(&path_str) {
            return Ok(date_str);
        }

        // Fall back to modified date
        Ok(daily.modified.format("%Y-%m-%d").to_string())
    }

    /// Compute activity summaries for all notes.
    fn compute_activity_summaries(&self) -> Result<usize, DerivedError> {
        let today = Utc::now().date_naive();
        let thirty_days_ago = today - Duration::days(30);
        let ninety_days_ago = today - Duration::days(90);

        // Get aggregated activity data
        let summaries = self.db.aggregate_activity(
            &thirty_days_ago.to_string(),
            &ninety_days_ago.to_string(),
        )?;

        let mut count = 0;
        for summary in summaries {
            // Compute staleness score
            let staleness = self.compute_staleness_score(
                summary.last_seen.as_deref(),
                summary.access_count_30d,
                summary.access_count_90d,
            );

            self.db.upsert_activity_summary(
                summary.note_id,
                summary.last_seen.as_deref(),
                summary.access_count_30d,
                summary.access_count_90d,
                staleness,
            )?;
            count += 1;
        }

        Ok(count)
    }

    /// Compute staleness score based on activity patterns.
    ///
    /// Score ranges from 0.0 (very active) to 1.0 (very stale).
    fn compute_staleness_score(
        &self,
        last_seen: Option<&str>,
        count_30d: i32,
        count_90d: i32,
    ) -> f64 {
        let today = Utc::now().date_naive();

        // Days since last seen (default to 365 if never seen)
        let days_since = last_seen
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .map(|d| (today - d).num_days() as f64)
            .unwrap_or(365.0);

        // Base staleness from recency (0.0 = today, 1.0 = 90+ days)
        let recency_score = (days_since / 90.0).min(1.0);

        // Activity factor (more activity = less stale)
        let activity_factor = if count_30d > 0 {
            0.0 // Active in last 30 days - not stale
        } else if count_90d > 0 {
            0.3 // Active in last 90 days - slightly stale
        } else {
            0.6 // No recent activity - more stale
        };

        // Combined score
        (recency_score * 0.6 + activity_factor * 0.4).min(1.0)
    }

    /// Compute note cooccurrence matrix.
    ///
    /// Finds pairs of notes that are referenced together in daily notes.
    fn compute_cooccurrence(&self) -> Result<usize, DerivedError> {
        // Get cooccurrence data from temporal activity
        let pairs = self.db.compute_cooccurrence_pairs()?;
        let mut count = 0;

        for pair in pairs {
            self.db.upsert_cooccurrence(
                pair.note_a_id,
                pair.note_b_id,
                pair.shared_count,
                pair.most_recent.as_deref(),
            )?;
            count += 1;
        }

        Ok(count)
    }
}

/// Extract a date string (YYYY-MM-DD) from a file path.
fn extract_date_from_path(path: &str) -> Option<String> {
    // Look for date patterns in the path
    let re = regex::Regex::new(r"(\d{4}-\d{2}-\d{2})").ok()?;
    re.captures(path).map(|c| c[1].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_date_from_path() {
        assert_eq!(
            extract_date_from_path("daily/2025-01-15.md"),
            Some("2025-01-15".to_string())
        );
        assert_eq!(
            extract_date_from_path("2025-01-15-meeting.md"),
            Some("2025-01-15".to_string())
        );
        assert_eq!(extract_date_from_path("notes/random.md"), None);
    }

    #[test]
    fn test_staleness_score() {
        let builder = DerivedIndexBuilder { db: &IndexDb::open_in_memory().unwrap() };

        // Very active (accessed today, high count)
        let score = builder.compute_staleness_score(
            Some(&Utc::now().format("%Y-%m-%d").to_string()),
            5,
            10,
        );
        assert!(score < 0.1, "Active notes should have low staleness");

        // Never seen: days_since=365, recency_score=1.0, activity_factor=0.6
        // Combined: 1.0*0.6 + 0.6*0.4 = 0.84
        let score = builder.compute_staleness_score(None, 0, 0);
        assert!(score > 0.8, "Never-seen notes should be stale (score: {})", score);
    }
}
