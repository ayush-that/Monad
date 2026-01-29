# Agent Guidelines for Monad

## Build Commands

```bash
cargo build                          # Build all workspace crates
cargo build --release               # Release build
cargo build -p monad-app            # Build specific crate
cargo build -p monad                # Build binary only
```

## Test Commands

```bash
cargo test                           # Run all tests
cargo test -p monad-core            # Tests for specific crate
cargo test test_error_retryable     # Run single test by name
cargo test --doc                    # Run doc tests
cargo test -- --nocapture           # With output
```

## Linting

```bash
cargo clippy                         # Run clippy
cargo clippy --fix                   # Auto-fix suggestions
cargo clippy -- --deny warnings     # Strict check
cargo fmt                            # Format code
cargo fmt --check                    # Check formatting
```

## Project Structure

- **monad-core**: Core types, traits, and error handling
- **monad-innertube**: YouTube Music API client
- **monad-audio**: Audio playback engine (symphonia, cpal)
- **monad-extractor**: Media extraction
- **monad-cache**: SQLite caching layer
- **monad-app**: Dioxus desktop GUI application

## Code Style

### Imports

```rust
use std::sync::Arc;

use dashmap::DashMap;
use tracing::{debug, warn};

use monad_core::{Error, Result};
use crate::context::ClientContext;
```

### Naming

- **Types**: PascalCase (`Track`, `PlaybackState`)
- **Functions/Variables**: snake_case
- **Constants**: SCREAMING_SNAKE_CASE
- **Modules**: snake_case singular

### Error Handling

```rust
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] HttpError),
}
```

### Structs

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artists: Vec<TrackArtist>,
    pub duration: Duration,
}
```

### Enums

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}
```

### Concurrency

Use `parking_lot` (not std sync):

```rust
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

state: Arc<RwLock<PlaybackState>>,
volume: Arc<Mutex<f32>>,
```

### Logging

```rust
use tracing::{debug, info, warn, trace};
info!("Audio output: {} Hz", sample_rate);
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_retryable() {
        assert!(Error::Network("test".into()).is_retryable());
    }
}
```

### Clippy Allow

```rust
#[allow(clippy::unwrap_used)] // Header values are ASCII-safe
```

## Config

- Edition: 2021, Rust min: 1.80
- Max line width: 100, Tab spaces: 4
- Lints: pedantic, nursery enabled
- Use `cargo fmt` before committing
