//! # monad-core
//!
//! Core types, traits, and error handling for the Monad `YouTube` Music client.

pub mod error;
pub mod types;

pub use error::{Error, HttpError, Result};
pub use types::*;
