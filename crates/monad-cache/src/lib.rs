//! # monad-cache
//!
//! Offline caching (`SQLite` + filesystem) for Monad.
//!
//! This crate provides persistent caching for:
//! - Audio files for offline playback
//! - Metadata (tracks, albums, playlists)
//! - Thumbnails and artwork

use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use lru::LruCache;
use monad_core::{Error, Result};
use parking_lot::Mutex;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use tracing::info;

/// Cache manager for Monad.
pub struct CacheManager {
    /// `SQLite` database connection.
    db: Arc<Mutex<Connection>>,
    /// Cache directory path.
    cache_dir: PathBuf,
    /// In-memory LRU cache for hot data.
    memory_cache: Arc<Mutex<LruCache<String, Bytes>>>,
}

impl CacheManager {
    /// Create a new cache manager with default paths.
    pub fn new() -> Result<Self> {
        let project_dirs = ProjectDirs::from("com", "monad", "Monad")
            .ok_or_else(|| Error::Cache("Failed to determine cache directory".to_string()))?;

        let cache_dir = project_dirs.cache_dir().to_path_buf();
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| Error::Cache(format!("Failed to create cache directory: {e}")))?;

        Self::with_path(cache_dir)
    }

    /// Create a new cache manager with a custom path.
    pub fn with_path(cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| Error::Cache(format!("Failed to create cache directory: {e}")))?;

        let db_path = cache_dir.join("cache.db");
        let db = Connection::open(&db_path)
            .map_err(|e| Error::Cache(format!("Failed to open database: {e}")))?;

        // Initialize database schema
        db.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS audio_cache (
                id TEXT PRIMARY KEY,
                video_id TEXT NOT NULL,
                format TEXT NOT NULL,
                quality TEXT NOT NULL,
                file_path TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                duration_secs REAL,
                cached_at TEXT NOT NULL,
                last_accessed TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS metadata_cache (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                cached_at TEXT NOT NULL,
                expires_at TEXT
            );

            CREATE TABLE IF NOT EXISTS thumbnail_cache (
                url_hash TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                file_path TEXT NOT NULL,
                cached_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_audio_video_id ON audio_cache(video_id);
            CREATE INDEX IF NOT EXISTS idx_metadata_expires ON metadata_cache(expires_at);
            ",
        )
        .map_err(|e| Error::Cache(format!("Failed to initialize database: {e}")))?;

        info!("Cache initialized at {}", cache_dir.display());

        // SAFETY: 100 is a non-zero constant
        #[allow(clippy::expect_used)]
        let cache_size = std::num::NonZeroUsize::new(100).expect("100 is non-zero");

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            cache_dir,
            memory_cache: Arc::new(Mutex::new(LruCache::new(cache_size))),
        })
    }

    /// Get the cache directory path.
    pub const fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Check if audio is cached for a video.
    pub fn has_audio(&self, video_id: &str) -> bool {
        let db = self.db.lock();
        db.query_row(
            "SELECT 1 FROM audio_cache WHERE video_id = ? LIMIT 1",
            [video_id],
            |_| Ok(()),
        )
        .is_ok()
    }

    /// Get the file path for cached audio.
    pub fn get_audio_path(&self, video_id: &str) -> Option<PathBuf> {
        let db = self.db.lock();
        db.query_row(
            "SELECT file_path FROM audio_cache WHERE video_id = ? ORDER BY quality DESC LIMIT 1",
            [video_id],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(PathBuf::from)
    }

    /// Store metadata in the cache.
    pub fn set_metadata(&self, key: &str, value: &str, ttl_secs: Option<i64>) -> Result<()> {
        let now = Utc::now();
        let expires_at = ttl_secs.map(|secs| now + chrono::Duration::seconds(secs));

        let db = self.db.lock();
        db.execute(
            "INSERT OR REPLACE INTO metadata_cache (key, value, cached_at, expires_at) VALUES (?, ?, ?, ?)",
            rusqlite::params![
                key,
                value,
                now.to_rfc3339(),
                expires_at.map(|e| e.to_rfc3339())
            ],
        )
        .map_err(|e| Error::Cache(format!("Failed to store metadata: {e}")))?;

        Ok(())
    }

    /// Get metadata from the cache.
    pub fn get_metadata(&self, key: &str) -> Option<String> {
        let db = self.db.lock();
        let result: rusqlite::Result<(String, Option<String>)> = db.query_row(
            "SELECT value, expires_at FROM metadata_cache WHERE key = ?",
            [key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );

        match result {
            Ok((value, expires_at)) => {
                // Check expiration
                if let Some(expires_str) = expires_at {
                    if let Ok(expires) = DateTime::parse_from_rfc3339(&expires_str) {
                        if expires < Utc::now() {
                            return None;
                        }
                    }
                }
                Some(value)
            }
            Err(_) => None,
        }
    }

    /// Generate a hash for a URL.
    #[allow(dead_code)] // Will be used by thumbnail caching
    fn hash_url(url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let db = self.db.lock();

        let audio_count: i64 = db
            .query_row("SELECT COUNT(*) FROM audio_cache", [], |row| row.get(0))
            .unwrap_or(0);

        let audio_size: i64 = db
            .query_row(
                "SELECT COALESCE(SUM(size_bytes), 0) FROM audio_cache",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let metadata_count: i64 = db
            .query_row("SELECT COUNT(*) FROM metadata_cache", [], |row| row.get(0))
            .unwrap_or(0);

        let thumbnail_count: i64 = db
            .query_row("SELECT COUNT(*) FROM thumbnail_cache", [], |row| row.get(0))
            .unwrap_or(0);

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        CacheStats {
            audio_count: audio_count as usize,
            audio_size_bytes: audio_size as u64,
            metadata_count: metadata_count as usize,
            thumbnail_count: thumbnail_count as usize,
        }
    }

    /// Clear all cached data.
    pub fn clear(&self) -> Result<()> {
        let db = self.db.lock();
        db.execute_batch(
            "
            DELETE FROM audio_cache;
            DELETE FROM metadata_cache;
            DELETE FROM thumbnail_cache;
            ",
        )
        .map_err(|e| Error::Cache(format!("Failed to clear cache: {e}")))?;

        // Clear memory cache
        self.memory_cache.lock().clear();

        info!("Cache cleared");
        Ok(())
    }
}

impl Default for CacheManager {
    /// # Panics
    /// Panics if the cache directory cannot be created or database cannot be initialized.
    #[allow(clippy::expect_used)]
    fn default() -> Self {
        Self::new().expect("Failed to create default cache manager")
    }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of cached audio files.
    pub audio_count: usize,
    /// Total size of cached audio in bytes.
    pub audio_size_bytes: u64,
    /// Number of cached metadata entries.
    pub metadata_count: usize,
    /// Number of cached thumbnails.
    pub thumbnail_count: usize,
}

impl CacheStats {
    /// Get the total audio size in megabytes.
    #[allow(clippy::cast_precision_loss)]
    pub fn audio_size_mb(&self) -> f64 {
        self.audio_size_bytes as f64 / (1024.0 * 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_hash() {
        let hash1 = CacheManager::hash_url("https://example.com/image1.jpg");
        let hash2 = CacheManager::hash_url("https://example.com/image2.jpg");
        assert_ne!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA256 hex
    }
}
