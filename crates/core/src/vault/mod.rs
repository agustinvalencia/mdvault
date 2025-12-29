//! Vault file discovery and content extraction.
//!
//! This module provides utilities for walking vault directories,
//! extracting metadata from markdown files, and computing content hashes.

pub mod extractor;
pub mod hasher;
pub mod walker;

pub use extractor::{ExtractedLink, ExtractedNote, extract_note};
pub use hasher::{content_hash, content_hash_str};
pub use walker::{VaultWalker, VaultWalkerError, WalkedFile};
