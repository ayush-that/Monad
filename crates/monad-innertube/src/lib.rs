//! # monad-innertube
//!
//! `YouTube` Music `InnerTube` API client for Monad.
//!
//! This crate provides a Rust implementation of the `InnerTube` protocol
//! used by `YouTube` Music for searching, browsing, and retrieving stream URLs.

pub mod client;
pub mod context;
pub mod endpoints;
pub mod parser;
pub mod types;

pub use client::InnerTubeClient;
pub use context::ClientContext;
pub use types::{SearchFilter, SearchResults};
