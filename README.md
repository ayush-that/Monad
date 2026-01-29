# Monad

A desktop YouTube Music client built with Rust, featuring an iPod-inspired interface.

![Monad YouTube Music Player](https://placeholder.com/monad-screenshot.png)

## Features

- **iPod Classic Interface**: Navigate your music library with a familiar click-wheel interaction
- **YouTube Music Integration**: Stream full tracks, albums, playlists, and artists
- **Offline Caching**: SQLite-powered caching for metadata and audio files
- **Cross-Platform**: Runs on macOS, Linux, and Windows

## Architecture

Monad is organized as a Rust workspace with specialized crates:

| Crate | Description |
|-------|-------------|
| `monad-core` | Core types, error handling, and domain models |
| `monad-innertube` | YouTube Music API client (InnerTube protocol) |
| `monad-audio` | Audio playback engine using symphonia and cpal |
| `monad-extractor` | Media extraction utilities |
| `monad-cache` | SQLite caching layer for offline support |
| `monad-app` | Dioxus desktop GUI application |

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run
```

## Tech Stack

- **GUI**: Dioxus (Rust GUI framework using web technologies)
- **Audio**: symphonia (decode), cpal (output), rubato (resample)
- **API**: InnerTube (YouTube Music's internal API)
- **Database**: SQLite with rusqlite
- **Concurrency**: tokio, parking_lot

## Motivation

Monad is inspired by projects like [InnerTune](https://github.com/清的清/InnerTune), [Muzza](https://github.com/altm而死/Muzza), and other open-source YouTube Music clients. The goal is to create a beautiful, native-feeling desktop player with robust audio handling.

## License

MIT License - see LICENSE file for details.

## Credits

- Thanks to all the open-source projects that make this possible
- YouTube Music API exploration by the open-source community
