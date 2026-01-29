//! Error types for Monad.

use thiserror::Error;

/// Result type alias using Monad's Error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for Monad.
#[derive(Error, Debug)]
pub enum Error {
    // Network errors
    #[error("HTTP request failed: {0}")]
    Http(#[from] HttpError),

    #[error("Network error: {0}")]
    Network(String),

    // InnerTube API errors
    #[error("InnerTube API error: {0}")]
    InnerTube(String),

    #[error("Failed to parse API response: {0}")]
    ParseError(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("Content not available: {0}")]
    ContentNotAvailable(String),

    #[error("Rate limited, retry after {retry_after_secs:?} seconds")]
    RateLimited { retry_after_secs: Option<u64> },

    // Audio errors
    #[error("Audio decode error: {0}")]
    AudioDecode(String),

    #[error("Audio output error: {0}")]
    AudioOutput(String),

    #[error("Unsupported audio format: {0}")]
    UnsupportedFormat(String),

    #[error("Stream extraction failed: {0}")]
    StreamExtraction(String),

    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),

    // Cache errors
    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Database error: {0}")]
    Database(String),

    // IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // Serialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // Generic errors
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// HTTP-specific errors.
#[derive(Error, Debug)]
pub enum HttpError {
    #[error("Request failed with status {status}: {message}")]
    StatusError { status: u16, message: String },

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Request timeout")]
    Timeout,

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
}

impl Error {
    /// Returns true if this error is retryable.
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Network(_)
                | Self::RateLimited { .. }
                | Self::Http(HttpError::ConnectionFailed(_) | HttpError::Timeout)
        )
    }

    /// Returns true if this is a rate limit error.
    pub const fn is_rate_limited(&self) -> bool {
        matches!(self, Self::RateLimited { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_retryable() {
        assert!(Error::Network("test".into()).is_retryable());
        assert!(Error::RateLimited {
            retry_after_secs: Some(60)
        }
        .is_retryable());
        assert!(!Error::InvalidArgument("test".into()).is_retryable());
    }

    #[test]
    fn test_error_display() {
        let err = Error::InnerTube("test error".into());
        assert_eq!(err.to_string(), "InnerTube API error: test error");
    }
}
