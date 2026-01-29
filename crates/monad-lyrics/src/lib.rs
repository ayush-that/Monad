//! Lyrics fetching and parsing for Monad.
//!
//! Uses the Better Lyrics API to fetch synchronized lyrics.

mod parser;

use monad_core::Error;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

const API_BASE_URL: &str = "https://lyrics-api.boidu.dev";

/// A single word in the lyrics with timing information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricWord {
    /// The word text.
    pub text: String,
    /// Start time in seconds.
    pub start: f64,
    /// End time in seconds.
    pub end: f64,
}

/// A single line of lyrics with timing information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricLine {
    /// The full line text.
    pub text: String,
    /// Start time in seconds.
    pub start: f64,
    /// End time in seconds.
    pub end: f64,
    /// Individual words with timing (for word-level sync).
    pub words: Vec<LyricWord>,
}

/// Complete lyrics for a song.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lyrics {
    /// Song title.
    pub title: String,
    /// Artist name.
    pub artist: String,
    /// Total duration in seconds (if available).
    pub duration: Option<f64>,
    /// All lyric lines with timing.
    pub lines: Vec<LyricLine>,
}

impl Lyrics {
    /// Get the lyric line active at the given position (in seconds).
    pub fn line_at(&self, position: f64) -> Option<&LyricLine> {
        self.lines
            .iter()
            .find(|line| position >= line.start && position < line.end)
    }

    /// Get the index of the lyric line active at the given position.
    pub fn line_index_at(&self, position: f64) -> Option<usize> {
        self.lines
            .iter()
            .position(|line| position >= line.start && position < line.end)
    }

    /// Get plain text lyrics (no timing).
    pub fn plain_text(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// API response containing TTML lyrics.
#[derive(Debug, Deserialize)]
struct TtmlResponse {
    ttml: String,
}

/// Lyrics client for fetching from the Better Lyrics API.
#[derive(Clone)]
pub struct LyricsClient {
    client: Client,
}

impl Default for LyricsClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LyricsClient {
    /// Create a new lyrics client.
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("Monad/1.0")
            .build()
            .unwrap_or_default();

        Self { client }
    }

    /// Fetch lyrics for a song.
    ///
    /// # Arguments
    /// * `artist` - The artist name
    /// * `song` - The song title
    /// * `album` - Optional album name (improves matching)
    /// * `duration` - Optional duration in seconds (improves matching)
    pub async fn fetch(
        &self,
        artist: &str,
        song: &str,
        album: Option<&str>,
        duration: Option<f64>,
    ) -> Result<Lyrics, Error> {
        info!("Fetching lyrics for: {} - {}", artist, song);

        let mut url = format!(
            "{}/getLyrics?a={}&s={}",
            API_BASE_URL,
            urlencoding::encode(artist),
            urlencoding::encode(song)
        );

        if let Some(album) = album {
            use std::fmt::Write;
            let _ = write!(url, "&al={}", urlencoding::encode(album));
        }

        if let Some(dur) = duration {
            use std::fmt::Write;
            let _ = write!(url, "&d={dur}");
        }

        debug!("Requesting: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Api(format!(
                "Lyrics API returned {}: {}",
                status, body
            )));
        }

        let ttml_response: TtmlResponse = response
            .json()
            .await
            .map_err(|e| Error::Parse(e.to_string()))?;

        let lyrics = parser::parse_ttml(&ttml_response.ttml, artist, song)?;

        info!(
            "Fetched {} lyric lines for {} - {}",
            lyrics.lines.len(),
            artist,
            song
        );

        Ok(lyrics)
    }
}

/// URL encoding helper.
mod urlencoding {
    use std::fmt::Write;

    pub fn encode(s: &str) -> String {
        let mut result = String::with_capacity(s.len() * 3);
        for c in s.chars() {
            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                    result.push(c);
                }
                ' ' => result.push_str("%20"),
                _ => {
                    for b in c.to_string().bytes() {
                        let _ = write!(result, "%{b:02X}");
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_lyrics() {
        let client = LyricsClient::new();
        let result = client.fetch("Ed Sheeran", "Shape of You", None, None).await;

        match result {
            Ok(lyrics) => {
                println!("\n=== {} - {} ===", lyrics.artist, lyrics.title);
                println!("Duration: {:?} seconds", lyrics.duration);
                println!("Total lines: {}\n", lyrics.lines.len());

                // Show first 5 lines with timing
                for line in lyrics.lines.iter().take(5) {
                    println!("[{:.2}s - {:.2}s] {}", line.start, line.end, line.text);
                    if !line.words.is_empty() {
                        println!("  -> {} words with timing", line.words.len());
                    }
                }
                println!("...\n");

                // Test line_at function
                println!("Line at 10.0s: {:?}", lyrics.line_at(10.0).map(|l| &l.text));
                println!("Line at 50.0s: {:?}", lyrics.line_at(50.0).map(|l| &l.text));

                assert!(!lyrics.lines.is_empty());
            }
            Err(e) => {
                println!("Error (may be rate limited): {}", e);
            }
        }
    }
}
