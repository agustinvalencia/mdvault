//! Content hashing for change detection.

use std::fs::File;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{BufRead, BufReader, Result};
use std::path::Path;

/// Compute a hash of file content for change detection.
/// Uses DefaultHasher for speed (non-cryptographic, fast).
/// Returns hex-encoded hash string.
pub fn content_hash(path: &Path) -> Result<String> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut hasher = DefaultHasher::new();

    for line in reader.lines() {
        let line = line?;
        line.hash(&mut hasher);
    }

    Ok(format!("{:016x}", hasher.finish()))
}

/// Compute hash from content string (for testing).
pub fn content_hash_str(content: &str) -> String {
    let mut hasher = DefaultHasher::new();

    for line in content.lines() {
        line.hash(&mut hasher);
    }

    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_content_hash_str_consistent() {
        let content = "# Hello\n\nThis is a test.";
        let hash1 = content_hash_str(content);
        let hash2 = content_hash_str(content);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_content_hash_str_different_content() {
        let hash1 = content_hash_str("# Hello");
        let hash2 = content_hash_str("# World");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_content_hash_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "# Test\n\nContent here.").unwrap();

        let hash = content_hash(&path).unwrap();
        assert_eq!(hash.len(), 16); // 64-bit hash as 16 hex chars
    }

    #[test]
    fn test_content_hash_file_matches_str() {
        let content = "# Test\n\nContent here.";
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, content).unwrap();

        let file_hash = content_hash(&path).unwrap();
        let str_hash = content_hash_str(content);
        assert_eq!(file_hash, str_hash);
    }
}
