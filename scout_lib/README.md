# scout_lib

ScoutAPM API client library for Rust. Typed access to apps, metrics, endpoints,
jobs, traces, errors, insights, anomalies, org usage, and local archive helpers.

The `scout` CLI in this repository depends on this crate. Install the CLI from
GitHub, Homebrew (`scout-cli`), or crates.io package `scoutapm-cli` (binary name
`scout`). The crates.io names `scout` and `scout-cli` belong to unrelated projects.

## Install

```toml
[dependencies]
scout_lib = "0.6"
```

Optional parquet archive export:

```toml
scout_lib = { version = "0.6", features = ["export-parquet"] }
```

## Quick start

```rust
use scout_lib::Client;

let client = Client::new(api_key);
// async: client.list_apps(None).await?
```

Pass the API key from your own secret store. This crate can also resolve keys
from configured 1Password, Bitwarden, or KeePassXC backends used by the CLI.

## Docs

- API docs: <https://docs.rs/scout_lib>
- Repository: <https://github.com/amkisko/scout-cli.rs>
- OpenAPI: `docs/openapi.yaml` in the repository
