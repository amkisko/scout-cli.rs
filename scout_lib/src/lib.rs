//! ScoutAPM API client library.
//!
//! Provides a typed client for the ScoutAPM REST API: apps, metrics, endpoints,
//! traces, errors, and insights.

pub mod client;
pub mod error;
pub mod helpers;
pub mod secret;

pub use client::Client;
pub use error::{ApiError, AuthError, Error};
pub use helpers::{format_timestamp_display, get_api_key, parse_scout_url, ApiKeySource};
pub use secret::{bitwarden, keepassxc, one_password};

/// Library version for User-Agent and diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
