//! # monad-extractor
//!
//! `YouTube` audio extraction for Monad using yt-dlp.
//!
//! Features:
//! - Disk caching for instant repeated plays
//! - Downloads audio directly to avoid session-bound URL issues

use monad_core::{Error, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, info, warn};

/// Authentication method for yt-dlp.
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// Use cookies from a browser (recommended for YouTube Premium).
    BrowserCookies(String),
    /// No authentication.
    None,
}

impl Default for AuthMethod {
    fn default() -> Self {
        Self::BrowserCookies("chrome".to_string())
    }
}

impl AuthMethod {
    fn to_args(&self) -> Vec<String> {
        match self {
            Self::BrowserCookies(browser) => {
                vec!["--cookies-from-browser".to_string(), browser.clone()]
            }
            Self::None => vec![],
        }
    }
}

/// Extracted audio data from YouTube.
#[derive(Debug, Clone)]
pub struct ExtractedAudio {
    /// Raw audio data (opus, m4a, etc.)
    pub data: Vec<u8>,
    /// MIME type of the audio
    pub mime_type: String,
    /// Title of the track (if available)
    pub title: Option<String>,
}

/// `YouTube` audio extractor using yt-dlp with disk caching.
#[allow(clippy::module_name_repetitions)]
pub struct Extractor {
    yt_dlp_path: PathBuf,
    cache_dir: PathBuf,
    auth_method: AuthMethod,
}

impl Extractor {
    /// Create a new extractor.
    pub fn new() -> Self {
        let dirs = directories::ProjectDirs::from("", "", "monad");

        let yt_dlp_path = dirs
            .as_ref()
            .map(|d| d.cache_dir().join("yt-dlp"))
            .unwrap_or_else(|| PathBuf::from("yt-dlp"));

        let cache_dir = dirs
            .as_ref()
            .map(|d| d.cache_dir().join("audio"))
            .unwrap_or_else(|| PathBuf::from(".cache/audio"));

        // Ensure cache directory exists
        let _ = fs::create_dir_all(&cache_dir);

        Self {
            yt_dlp_path,
            cache_dir,
            auth_method: AuthMethod::default(),
        }
    }

    /// Set the authentication method.
    #[must_use]
    pub fn with_auth_method(mut self, auth_method: AuthMethod) -> Self {
        self.auth_method = auth_method;
        self
    }

    /// Set browser cookies as the authentication method.
    #[must_use]
    pub fn with_browser_cookies(mut self, browser: impl Into<String>) -> Self {
        self.auth_method = AuthMethod::BrowserCookies(browser.into());
        self
    }

    /// Get the current authentication method.
    pub fn auth_method(&self) -> &AuthMethod {
        &self.auth_method
    }

    /// Clear the disk cache.
    pub fn clear_cache(&self) {
        if let Err(e) = fs::remove_dir_all(&self.cache_dir) {
            warn!("Failed to clear cache: {e}");
        }
        let _ = fs::create_dir_all(&self.cache_dir);
        info!("Audio cache cleared");
    }

    /// Get cache file path for a video ID.
    fn cache_path(&self, video_id: &str) -> PathBuf {
        self.cache_dir.join(format!("{video_id}.audio"))
    }

    /// Check if audio is cached and load it.
    fn load_from_cache(&self, video_id: &str) -> Option<ExtractedAudio> {
        let path = self.cache_path(video_id);
        if !path.exists() {
            return None;
        }

        match fs::read(&path) {
            Ok(data) => {
                if data.is_empty() {
                    let _ = fs::remove_file(&path);
                    return None;
                }
                let mime_type = detect_audio_mime(&data);
                info!("Loaded {} bytes from cache ({})", data.len(), mime_type);
                Some(ExtractedAudio {
                    data,
                    mime_type,
                    title: None,
                })
            }
            Err(e) => {
                warn!("Failed to read cache: {e}");
                None
            }
        }
    }

    /// Save audio to cache.
    fn save_to_cache(&self, video_id: &str, data: &[u8]) {
        let path = self.cache_path(video_id);
        if let Err(e) = fs::write(&path, data) {
            warn!("Failed to write cache: {e}");
        } else {
            debug!("Cached {} bytes to {:?}", data.len(), path);
        }
    }

    /// Download audio for a video ID.
    /// Returns cached audio instantly if available.
    pub async fn extract(&self, video_id: &str) -> Result<ExtractedAudio> {
        // Check cache first - instant return if cached
        if let Some(cached) = self.load_from_cache(video_id) {
            info!("Cache hit for {video_id}");
            return Ok(cached);
        }

        info!("Cache miss - downloading {video_id}");
        let url = format!("https://www.youtube.com/watch?v={video_id}");

        if !self.yt_dlp_path.exists() {
            return Err(Error::ExtractionFailed(format!(
                "yt-dlp not found at {:?}",
                self.yt_dlp_path
            )));
        }

        // Build yt-dlp args
        let mut args = self.auth_method.to_args();
        args.extend([
            "--no-warnings".to_string(),
            "--no-progress".to_string(),
            "--js-runtimes".to_string(),
            "node".to_string(),
            "--remote-components".to_string(),
            "ejs:github".to_string(),
            "-f".to_string(),
            "141/140/bestaudio[ext=m4a]/bestaudio[ext=webm]/bestaudio".to_string(),
            "-o".to_string(),
            "-".to_string(),
            url,
        ]);

        debug!("Running yt-dlp");

        let output = Command::new(&self.yt_dlp_path)
            .args(&args)
            .output()
            .map_err(|e| Error::ExtractionFailed(format!("Failed to run yt-dlp: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("yt-dlp stderr: {}", stderr);
            return Err(Error::ExtractionFailed(format!(
                "yt-dlp failed: {}",
                stderr.lines().next().unwrap_or("Unknown error")
            )));
        }

        let data = output.stdout;
        if data.is_empty() {
            return Err(Error::ExtractionFailed(
                "yt-dlp returned empty data".to_string(),
            ));
        }

        // Save to cache for next time
        self.save_to_cache(video_id, &data);

        let mime_type = detect_audio_mime(&data);
        info!("Downloaded {} bytes ({})", data.len(), mime_type);

        Ok(ExtractedAudio {
            data,
            mime_type,
            title: None,
        })
    }
}

impl Default for Extractor {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect audio MIME type from magic bytes.
fn detect_audio_mime(data: &[u8]) -> String {
    if data.len() < 12 {
        return "audio/unknown".to_string();
    }

    if data.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return "audio/webm".to_string();
    }

    if data.len() >= 8 && &data[4..8] == b"ftyp" {
        return "audio/mp4".to_string();
    }

    if data.starts_with(b"ID3") || (data[0] == 0xFF && (data[1] & 0xE0) == 0xE0) {
        return "audio/mpeg".to_string();
    }

    if data.starts_with(b"OggS") {
        return "audio/ogg".to_string();
    }

    if data.starts_with(b"fLaC") {
        return "audio/flac".to_string();
    }

    "audio/unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extractor_creation() {
        let extractor = Extractor::new();
        assert!(matches!(
            extractor.auth_method(),
            AuthMethod::BrowserCookies(_)
        ));
    }

    #[test]
    fn test_mime_detection() {
        assert_eq!(
            detect_audio_mime(&[0x1A, 0x45, 0xDF, 0xA3, 0, 0, 0, 0, 0, 0, 0, 0]),
            "audio/webm"
        );
        assert_eq!(
            detect_audio_mime(&[0, 0, 0, 0x20, b'f', b't', b'y', b'p', b'M', b'4', b'A', b' ']),
            "audio/mp4"
        );
    }
}
