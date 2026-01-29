//! `InnerTube` API client implementation.

use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use monad_core::{Error, Result};
use parking_lot::RwLock;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, USER_AGENT};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

use crate::context::ClientContext;

const BASE_URL: &str = "https://music.youtube.com/youtubei/v1";
const ORIGIN: &str = "https://music.youtube.com";
const REFERER: &str = "https://music.youtube.com/";

/// Default timeout for requests.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum number of retries for failed requests.
const MAX_RETRIES: u32 = 3;

/// Base delay for exponential backoff (milliseconds).
const BASE_RETRY_DELAY_MS: u64 = 500;

/// Cache entry with expiration.
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    expires_at: std::time::Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        Self {
            value,
            expires_at: std::time::Instant::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        std::time::Instant::now() >= self.expires_at
    }
}

/// `YouTube` Music `InnerTube` API client.
#[derive(Clone)]
pub struct InnerTubeClient {
    /// HTTP client for making requests.
    http: reqwest::Client,
    /// Client context for requests.
    pub(crate) context: ClientContext,
    /// In-memory cache for responses.
    cache: Arc<DashMap<String, CacheEntry<Vec<u8>>>>,
    /// Cache TTL for API responses.
    cache_ttl: Duration,
    /// Rate limiter state.
    rate_limit_state: Arc<RwLock<RateLimitState>>,
}

#[derive(Debug, Default)]
struct RateLimitState {
    /// Time when we can make requests again (if rate limited).
    blocked_until: Option<std::time::Instant>,
    /// Number of requests made recently.
    request_count: u32,
    /// Window start for request counting.
    window_start: Option<std::time::Instant>,
}

impl RateLimitState {
    fn is_blocked(&self) -> bool {
        self.blocked_until
            .is_some_and(|until| std::time::Instant::now() < until)
    }

    fn block_for(&mut self, duration: Duration) {
        self.blocked_until = Some(std::time::Instant::now() + duration);
    }

    fn check_and_increment(&mut self) -> bool {
        let now = std::time::Instant::now();

        // Reset window if expired (1 minute window)
        let window_duration = Duration::from_secs(60);
        if let Some(start) = self.window_start {
            if now.duration_since(start) > window_duration {
                self.window_start = Some(now);
                self.request_count = 0;
            }
        } else {
            self.window_start = Some(now);
            self.request_count = 0;
        }

        // Check if we're over the limit (100 requests per minute)
        if self.request_count >= 100 {
            return false;
        }

        self.request_count += 1;
        true
    }
}

impl InnerTubeClient {
    /// Create a new `InnerTube` client with default settings.
    pub fn new() -> Result<Self> {
        Self::with_context(ClientContext::music_web())
    }

    /// Create a new `InnerTube` client with a specific context.
    #[allow(clippy::unwrap_used)] // Header values are ASCII-safe
    pub fn with_context(context: ClientContext) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "X-Goog-Api-Key",
            HeaderValue::from_static(context.client.api_key()),
        );
        headers.insert(
            "X-YouTube-Client-Name",
            HeaderValue::from_str(&context.client.client_id().to_string()).unwrap(),
        );
        headers.insert(
            "X-YouTube-Client-Version",
            HeaderValue::from_str(&context.client.client_version).unwrap(),
        );
        headers.insert("Origin", HeaderValue::from_static(ORIGIN));
        headers.insert("Referer", HeaderValue::from_static(REFERER));

        if let Some(ua) = &context.client.user_agent {
            headers.insert(USER_AGENT, HeaderValue::from_str(ua).unwrap());
        }

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(DEFAULT_TIMEOUT)
            .pool_max_idle_per_host(10)
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Network(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            http,
            context,
            cache: Arc::new(DashMap::new()),
            cache_ttl: Duration::from_secs(300), // 5 minutes default
            rate_limit_state: Arc::new(RwLock::new(RateLimitState::default())),
        })
    }

    /// Set the cache TTL for API responses.
    pub const fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Make a POST request to an `InnerTube` endpoint.
    pub(crate) async fn post<T, R>(&self, endpoint: &str, body: &T) -> Result<R>
    where
        T: Serialize,
        R: DeserializeOwned,
    {
        let url = format!("{BASE_URL}/{endpoint}");
        let body_bytes = serde_json::to_vec(body)?;

        // Generate cache key
        let cache_key = self.cache_key(endpoint, &body_bytes);

        // Check cache first
        if let Some(cached) = self.get_cached(&cache_key) {
            debug!("Cache hit for {endpoint}");
            return serde_json::from_slice(&cached).map_err(|e| Error::ParseError(e.to_string()));
        }

        // Check rate limit
        {
            let state = self.rate_limit_state.read();
            if state.is_blocked() {
                return Err(Error::RateLimited {
                    retry_after_secs: state
                        .blocked_until
                        .map(|until| until.duration_since(std::time::Instant::now()).as_secs()),
                });
            }
        }

        // Update rate limit counter
        {
            let mut state = self.rate_limit_state.write();
            if !state.check_and_increment() {
                state.block_for(Duration::from_secs(60));
                return Err(Error::RateLimited {
                    retry_after_secs: Some(60),
                });
            }
        }

        // Make request with retries
        let mut last_error = None;
        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                let delay = Duration::from_millis(BASE_RETRY_DELAY_MS * 2u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
                debug!("Retry attempt {attempt} for {endpoint} after {delay:?}");
            }

            match self.do_request(&url, &body_bytes).await {
                Ok(response_bytes) => {
                    // Cache the response
                    self.set_cached(cache_key, response_bytes.clone());

                    return serde_json::from_slice(&response_bytes)
                        .map_err(|e| Error::ParseError(format!("Failed to parse response: {e}")));
                }
                Err(e) => {
                    warn!("Request to {endpoint} failed (attempt {attempt}): {e}");

                    // Handle rate limiting
                    if e.is_rate_limited() {
                        let mut state = self.rate_limit_state.write();
                        state.block_for(Duration::from_secs(60));
                    }

                    // Don't retry non-retryable errors
                    if !e.is_retryable() {
                        return Err(e);
                    }

                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| Error::Network("Request failed".to_string())))
    }

    async fn do_request(&self, url: &str, body: &[u8]) -> Result<Vec<u8>> {
        let response = self
            .http
            .post(url)
            .body(body.to_vec())
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    Error::Http(monad_core::HttpError::Timeout)
                } else if e.is_connect() {
                    Error::Http(monad_core::HttpError::ConnectionFailed(e.to_string()))
                } else {
                    Error::Network(e.to_string())
                }
            })?;

        let status = response.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok());

            return Err(Error::RateLimited {
                retry_after_secs: retry_after,
            });
        }

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(Error::Http(monad_core::HttpError::StatusError {
                status: status.as_u16(),
                message,
            }));
        }

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| Error::Network(format!("Failed to read response body: {e}")))
    }

    fn cache_key(&self, endpoint: &str, body: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(endpoint.as_bytes());
        hasher.update(body);
        hex::encode(hasher.finalize())
    }

    fn get_cached(&self, key: &str) -> Option<Vec<u8>> {
        let entry = self.cache.get(key)?;
        if entry.is_expired() {
            drop(entry);
            self.cache.remove(key);
            return None;
        }
        Some(entry.value.clone())
    }

    fn set_cached(&self, key: String, value: Vec<u8>) {
        let entry = CacheEntry::new(value, self.cache_ttl);
        self.cache.insert(key, entry);

        // Cleanup expired entries occasionally
        if self.cache.len() > 100 {
            self.cleanup_cache();
        }
    }

    fn cleanup_cache(&self) {
        self.cache.retain(|_, entry| !entry.is_expired());
    }

    /// Clear the cache.
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Get the number of cached entries.
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }
}

impl Default for InnerTubeClient {
    /// # Panics
    /// Panics if the HTTP client cannot be created.
    #[allow(clippy::expect_used)]
    fn default() -> Self {
        Self::new().expect("Failed to create default InnerTube client")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = InnerTubeClient::new().unwrap();
        assert_eq!(client.cache_size(), 0);
    }

    #[test]
    fn test_cache_key_generation() {
        let client = InnerTubeClient::new().unwrap();
        let key1 = client.cache_key("search", b"query1");
        let key2 = client.cache_key("search", b"query2");
        let key3 = client.cache_key("search", b"query1");

        assert_ne!(key1, key2);
        assert_eq!(key1, key3);
    }

    #[test]
    fn test_rate_limit_state() {
        let mut state = RateLimitState::default();

        // Should allow requests initially
        assert!(!state.is_blocked());
        assert!(state.check_and_increment());

        // Block and check
        state.block_for(Duration::from_secs(1));
        assert!(state.is_blocked());
    }
}
