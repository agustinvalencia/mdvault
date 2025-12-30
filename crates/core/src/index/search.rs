//! Contextual search beyond keyword matching.
//!
//! This module provides multi-modal search capabilities:
//! - Direct match: Notes matching a query string
//! - Graph neighbourhood: Linked notes within N hops
//! - Temporal context: Recent dailies referencing matches
//! - Cooccurrence: Notes that appeared together in dailies

use std::collections::{HashMap, HashSet};

use super::IndexError;
use super::db::IndexDb;
use super::types::{IndexedNote, NoteType};

/// Search mode determining how results are expanded.
#[derive(Debug, Clone, Copy, Default)]
pub enum SearchMode {
    /// Only return notes directly matching the query.
    #[default]
    Direct,
    /// Include linked notes within N hops.
    Neighbourhood { hops: u32 },
    /// Include recent dailies referencing matching notes.
    Temporal { days: u32 },
    /// Include notes that cooccur with matches in dailies.
    Cooccurrence { min_shared: u32 },
    /// Combined: neighbourhood + temporal + cooccurrence.
    Full,
}

/// Search query parameters.
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    /// Text to search for (in title, path, or content).
    pub text: Option<String>,
    /// Filter by note type.
    pub note_type: Option<NoteType>,
    /// Path prefix filter.
    pub path_prefix: Option<String>,
    /// Search mode for result expansion.
    pub mode: SearchMode,
    /// Maximum results to return.
    pub limit: Option<u32>,
    /// Favour recently active notes.
    pub temporal_boost: bool,
}

/// A search result with relevance information.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The matching note.
    pub note: IndexedNote,
    /// Relevance score (higher = more relevant).
    pub score: f64,
    /// How this result was found.
    pub match_source: MatchSource,
    /// Staleness score if available (lower = more active).
    pub staleness: Option<f64>,
}

/// How a search result was matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchSource {
    /// Direct text match.
    Direct,
    /// Linked from a direct match.
    Linked { hops: u32 },
    /// Referenced in a daily with a direct match.
    Temporal { daily_path: String },
    /// Cooccurs with a direct match.
    Cooccurrence { shared_dailies: u32 },
}

/// Search engine using the vault index.
pub struct SearchEngine<'a> {
    db: &'a IndexDb,
}

impl<'a> SearchEngine<'a> {
    /// Create a new search engine.
    pub fn new(db: &'a IndexDb) -> Self {
        Self { db }
    }

    /// Execute a search query.
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, IndexError> {
        // Step 1: Find direct matches
        let direct_matches = self.find_direct_matches(query)?;
        let direct_ids: HashSet<i64> =
            direct_matches.iter().filter_map(|n| n.id).collect();

        let mut results: Vec<SearchResult> = direct_matches
            .into_iter()
            .map(|note| SearchResult {
                staleness: self.get_staleness(note.id),
                note,
                score: 1.0,
                match_source: MatchSource::Direct,
            })
            .collect();

        // Step 2: Expand based on mode
        match query.mode {
            SearchMode::Direct => {}
            SearchMode::Neighbourhood { hops } => {
                let expanded = self.expand_neighbourhood(&direct_ids, hops)?;
                results.extend(expanded);
            }
            SearchMode::Temporal { days } => {
                let expanded = self.expand_temporal(&direct_ids, days)?;
                results.extend(expanded);
            }
            SearchMode::Cooccurrence { min_shared } => {
                let expanded = self.expand_cooccurrence(&direct_ids, min_shared)?;
                results.extend(expanded);
            }
            SearchMode::Full => {
                // Combine all expansion modes
                let neighbourhood = self.expand_neighbourhood(&direct_ids, 2)?;
                let temporal = self.expand_temporal(&direct_ids, 30)?;
                let cooccurrence = self.expand_cooccurrence(&direct_ids, 2)?;
                results.extend(neighbourhood);
                results.extend(temporal);
                results.extend(cooccurrence);
            }
        }

        // Step 3: Apply temporal boost if requested
        if query.temporal_boost {
            for result in &mut results {
                if let Some(staleness) = result.staleness {
                    // Boost score based on freshness (1 - staleness)
                    result.score *= 1.0 + (1.0 - staleness) * 0.5;
                }
            }
        }

        // Step 4: Deduplicate and sort by score
        results = self.deduplicate_results(results);
        results.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Step 5: Apply limit
        if let Some(limit) = query.limit {
            results.truncate(limit as usize);
        }

        Ok(results)
    }

    /// Find notes directly matching the query.
    fn find_direct_matches(
        &self,
        query: &SearchQuery,
    ) -> Result<Vec<IndexedNote>, IndexError> {
        // Build a NoteQuery from SearchQuery
        let note_query = super::types::NoteQuery {
            note_type: query.note_type,
            path_prefix: query.path_prefix.as_ref().map(Into::into),
            limit: query.limit,
            ..Default::default()
        };

        let notes = self.db.query_notes(&note_query)?;

        // Filter by text if provided
        if let Some(text) = &query.text {
            let text_lower = text.to_lowercase();
            Ok(notes
                .into_iter()
                .filter(|n| {
                    n.title.to_lowercase().contains(&text_lower)
                        || n.path.to_string_lossy().to_lowercase().contains(&text_lower)
                })
                .collect())
        } else {
            Ok(notes)
        }
    }

    /// Expand results by following links up to N hops.
    fn expand_neighbourhood(
        &self,
        seed_ids: &HashSet<i64>,
        max_hops: u32,
    ) -> Result<Vec<SearchResult>, IndexError> {
        let mut results = Vec::new();
        let mut visited: HashSet<i64> = seed_ids.clone();
        let mut frontier: HashSet<i64> = seed_ids.clone();

        for hop in 1..=max_hops {
            let mut next_frontier = HashSet::new();

            for &note_id in &frontier {
                // Get outgoing links
                let outlinks = self.db.get_outgoing_links(note_id)?;
                for link in outlinks {
                    if let Some(target_id) = link.target_id
                        && !visited.contains(&target_id)
                    {
                        visited.insert(target_id);
                        next_frontier.insert(target_id);

                        if let Some(note) = self.db.get_note_by_id(target_id)? {
                            results.push(SearchResult {
                                staleness: self.get_staleness(note.id),
                                note,
                                score: 0.5 / (hop as f64), // Decay by distance
                                match_source: MatchSource::Linked { hops: hop },
                            });
                        }
                    }
                }

                // Get backlinks
                let backlinks = self.db.get_backlinks(note_id)?;
                for link in backlinks {
                    if !visited.contains(&link.source_id) {
                        visited.insert(link.source_id);
                        next_frontier.insert(link.source_id);

                        if let Some(note) = self.db.get_note_by_id(link.source_id)? {
                            results.push(SearchResult {
                                staleness: self.get_staleness(note.id),
                                note,
                                score: 0.5 / (hop as f64),
                                match_source: MatchSource::Linked { hops: hop },
                            });
                        }
                    }
                }
            }

            frontier = next_frontier;
            if frontier.is_empty() {
                break;
            }
        }

        Ok(results)
    }

    /// Expand results by finding recent dailies referencing matches.
    fn expand_temporal(
        &self,
        seed_ids: &HashSet<i64>,
        _days: u32,
    ) -> Result<Vec<SearchResult>, IndexError> {
        let mut results = Vec::new();
        let mut seen_dailies: HashSet<i64> = HashSet::new();

        for &note_id in seed_ids {
            // Get backlinks to find dailies referencing this note
            let backlinks = self.db.get_backlinks(note_id)?;
            for link in backlinks {
                if let Some(source_note) = self.db.get_note_by_id(link.source_id)?
                    && source_note.note_type == NoteType::Daily
                    && !seen_dailies.contains(&link.source_id)
                    && !seed_ids.contains(&link.source_id)
                {
                    seen_dailies.insert(link.source_id);
                    let path = source_note.path.to_string_lossy().to_string();
                    results.push(SearchResult {
                        staleness: self.get_staleness(source_note.id),
                        note: source_note,
                        score: 0.4,
                        match_source: MatchSource::Temporal { daily_path: path },
                    });
                }
            }
        }

        Ok(results)
    }

    /// Expand results by finding notes that cooccur with matches.
    fn expand_cooccurrence(
        &self,
        seed_ids: &HashSet<i64>,
        min_shared: u32,
    ) -> Result<Vec<SearchResult>, IndexError> {
        let mut results = Vec::new();
        let mut seen: HashSet<i64> = seed_ids.clone();

        for &note_id in seed_ids {
            let cooccurrent = self.db.get_cooccurrent_notes(note_id, 10)?;
            for (note, shared_count) in cooccurrent {
                if let Some(id) = note.id
                    && shared_count >= min_shared as i32
                    && !seen.contains(&id)
                {
                    seen.insert(id);
                    results.push(SearchResult {
                        staleness: self.get_staleness(note.id),
                        note,
                        score: 0.3 * (shared_count as f64 / 10.0).min(1.0),
                        match_source: MatchSource::Cooccurrence {
                            shared_dailies: shared_count as u32,
                        },
                    });
                }
            }
        }

        Ok(results)
    }

    /// Get staleness score for a note.
    fn get_staleness(&self, note_id: Option<i64>) -> Option<f64> {
        note_id.and_then(|id| {
            self.db
                .get_activity_summary(id)
                .ok()
                .flatten()
                .map(|s| s.staleness_score as f64)
        })
    }

    /// Deduplicate results, keeping highest score for each note.
    fn deduplicate_results(&self, results: Vec<SearchResult>) -> Vec<SearchResult> {
        let mut best: HashMap<i64, SearchResult> = HashMap::new();

        for result in results {
            if let Some(id) = result.note.id {
                best.entry(id)
                    .and_modify(|existing| {
                        if result.score > existing.score {
                            *existing = result.clone();
                        }
                    })
                    .or_insert(result);
            }
        }

        best.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::path::PathBuf;

    fn sample_note(path: &str, title: &str, note_type: NoteType) -> IndexedNote {
        IndexedNote {
            id: None,
            path: PathBuf::from(path),
            note_type,
            title: title.to_string(),
            created: Some(Utc::now()),
            modified: Utc::now(),
            frontmatter_json: None,
            content_hash: format!("hash-{}", path),
        }
    }

    #[test]
    fn test_direct_search() {
        let db = IndexDb::open_in_memory().unwrap();

        // Insert test notes
        db.insert_note(&sample_note(
            "tasks/task1.md",
            "Fix bug in parser",
            NoteType::Task,
        ))
        .unwrap();
        db.insert_note(&sample_note(
            "tasks/task2.md",
            "Write documentation",
            NoteType::Task,
        ))
        .unwrap();
        db.insert_note(&sample_note(
            "zettel/note1.md",
            "Parser internals",
            NoteType::Zettel,
        ))
        .unwrap();

        let engine = SearchEngine::new(&db);

        // Search for "parser"
        let query = SearchQuery {
            text: Some("parser".to_string()),
            mode: SearchMode::Direct,
            ..Default::default()
        };

        let results = engine.search(&query).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.match_source == MatchSource::Direct));
    }

    #[test]
    fn test_type_filter() {
        let db = IndexDb::open_in_memory().unwrap();

        db.insert_note(&sample_note("tasks/task1.md", "Task note", NoteType::Task))
            .unwrap();
        db.insert_note(&sample_note("zettel/note1.md", "Zettel note", NoteType::Zettel))
            .unwrap();

        let engine = SearchEngine::new(&db);

        let query = SearchQuery {
            note_type: Some(NoteType::Task),
            mode: SearchMode::Direct,
            ..Default::default()
        };

        let results = engine.search(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note.note_type, NoteType::Task);
    }
}
