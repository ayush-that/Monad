//! # monad-extractor
//!
//! `YouTube` audio extraction for Monad using yt-dlp.
//!
//! Features:
//! - Disk caching for instant repeated plays
//! - Downloads audio directly to avoid session-bound URL issues
//! - Streaming extraction for playback before download completes

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tokio::io::AsyncReadExt;
use tokio::process::{Child as AsyncChild, Command as AsyncCommand};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use monad_core::{Error, Result};

// Re-export StreamChunk for convenience
pub use monad_core::StreamChunk;

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

/// Handle to a streaming extraction in progress.
pub struct StreamingExtraction {
    /// Receiver for streaming chunks.
    pub rx: mpsc::Receiver<StreamChunk>,
    /// Handle to the download task.
    task: JoinHandle<()>,
}

impl StreamingExtraction {
    /// Abort the streaming extraction.
    pub fn abort(&self) {
        self.task.abort();
    }
}

impl Drop for StreamingExtraction {
    fn drop(&mut self) {
        // Abort the task when dropped to prevent orphaned background tasks
        self.task.abort();
    }
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

    /// Check if audio is cached for a video ID.
    pub fn is_cached(&self, video_id: &str) -> bool {
        let path = self.cache_path(video_id);
        path.exists() && fs::metadata(&path).map_or(false, |m| m.len() > 0)
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

    /// Start streaming extraction for a video ID.
    /// Returns immediately with a receiver for streaming chunks.
    /// Audio data is sent as it's downloaded, enabling playback before download completes.
    pub fn extract_streaming(&self, video_id: &str) -> Result<StreamingExtraction> {
        // Check cache first - if cached, return complete data immediately
        if let Some(cached) = self.load_from_cache(video_id) {
            info!("Cache hit for {video_id} - returning immediately");
            let (tx, rx) = mpsc::channel(16);
            let data = cached.data;
            let task = tokio::spawn(async move {
                // Send cached data as a single chunk
                let _ = tx.send(StreamChunk::Data(data)).await;
                let _ = tx.send(StreamChunk::Complete).await;
            });
            return Ok(StreamingExtraction { rx, task });
        }

        info!("Cache miss - starting streaming extraction for {video_id}");
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

        let yt_dlp_path = self.yt_dlp_path.clone();
        let cache_path = self.cache_path(video_id);
        let video_id_owned = video_id.to_string();

        let (tx, rx) = mpsc::channel(64);

        let task = tokio::spawn(async move {
            debug!("Spawning yt-dlp for streaming extraction");

            // Use null for stderr to avoid blocking if yt-dlp writes too much
            let mut child: AsyncChild = match AsyncCommand::new(&yt_dlp_path)
                .args(&args)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                Ok(child) => child,
                Err(e) => {
                    if tx
                        .send(StreamChunk::Error(format!("Failed to spawn yt-dlp: {e}")))
                        .await
                        .is_err()
                    {
                        warn!("Failed to send error notification");
                    }
                    return;
                }
            };

            let mut stdout = match child.stdout.take() {
                Some(stdout) => stdout,
                None => {
                    if tx
                        .send(StreamChunk::Error(
                            "Failed to capture yt-dlp stdout".to_string(),
                        ))
                        .await
                        .is_err()
                    {
                        warn!("Failed to send error notification");
                    }
                    return;
                }
            };

            // Accumulate all data for caching
            let mut all_data = Vec::new();
            let mut buffer = vec![0u8; 65536]; // 64KB chunks
            let mut total_sent = 0usize;
            let mut last_logged = 0usize;

            loop {
                match stdout.read(&mut buffer).await {
                    Ok(0) => {
                        // EOF - download complete
                        debug!(
                            "Streaming extraction complete: {} bytes total",
                            all_data.len()
                        );
                        break;
                    }
                    Ok(n) => {
                        // Extend all_data directly from buffer to avoid double copy
                        all_data.extend_from_slice(&buffer[..n]);
                        let chunk = buffer[..n].to_vec();
                        total_sent += n;

                        if tx.send(StreamChunk::Data(chunk)).await.is_err() {
                            debug!("Receiver dropped, aborting streaming extraction");
                            if let Err(e) = child.kill().await {
                                warn!("Failed to kill yt-dlp: {e}");
                            }
                            // Reap the zombie process
                            let _ = child.wait().await;
                            return;
                        }

                        // Log progress every 256KB using threshold-based approach
                        if total_sent >= last_logged + 256 * 1024 {
                            last_logged = total_sent;
                            debug!("Streaming: {} KB sent so far", total_sent / 1024);
                        }
                    }
                    Err(e) => {
                        if tx
                            .send(StreamChunk::Error(format!("Read error: {e}")))
                            .await
                            .is_err()
                        {
                            warn!("Failed to send error notification");
                        }
                        return;
                    }
                }
            }

            // Check exit status
            match child.wait().await {
                Ok(status) if status.success() => {
                    if all_data.is_empty() {
                        if tx
                            .send(StreamChunk::Error("yt-dlp returned empty data".to_string()))
                            .await
                            .is_err()
                        {
                            warn!("Failed to send error notification");
                        }
                        return;
                    }

                    // Cache the complete download
                    if let Err(e) = fs::write(&cache_path, &all_data) {
                        warn!("Failed to cache audio: {e}");
                    } else {
                        debug!(
                            "Cached {} bytes for {} at {:?}",
                            all_data.len(),
                            video_id_owned,
                            cache_path
                        );
                    }

                    if tx.send(StreamChunk::Complete).await.is_err() {
                        warn!("Failed to send completion notification");
                    }
                }
                Ok(status) => {
                    let error_msg =
                        format!("yt-dlp exited with error (exit code: {:?})", status.code());
                    if tx.send(StreamChunk::Error(error_msg)).await.is_err() {
                        warn!("Failed to send error notification");
                    }
                }
                Err(e) => {
                    if tx
                        .send(StreamChunk::Error(format!(
                            "Failed to wait for yt-dlp: {e}"
                        )))
                        .await
                        .is_err()
                    {
                        warn!("Failed to send error notification");
                    }
                }
            }
        });

        Ok(StreamingExtraction { rx, task })
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
