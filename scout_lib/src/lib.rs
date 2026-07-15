//! ScoutAPM API client library.
//!
//! Provides a typed client for the ScoutAPM REST API: apps, metrics, endpoints,
//! jobs, traces, errors, insights, and anomaly events.

pub mod client;
pub mod config;
pub mod error;
pub mod helpers;
pub mod secret;

pub use client::Client;
pub use config::{
    config_file_path, get_config_entry, list_config_entries, load_home_config, scout_home,
    set_config_entry, unset_config_entry, ConfigEntry, ConfigSource,
};
pub use error::{ApiError, AuthError, Error};
pub use helpers::{format_timestamp_display, get_api_key, parse_scout_url, ApiKeySource};
pub use secret::{bitwarden, keepassxc, one_password};

/// Library version for User-Agent and diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
