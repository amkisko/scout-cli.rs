# scout-cli

[![Test Status](https://github.com/amkisko/scout-cli.rs/actions/workflows/test.yml/badge.svg)](https://github.com/amkisko/scout-cli.rs/actions/workflows/test.yml)

ScoutAPM CLI — query apps, endpoints, traces, metrics, and errors from the terminal.

Sponsored by [Kisko Labs](https://www.kiskolabs.com).

## Requirements

- Rust 1.70+ (for building from source), or use a pre-built package below.

## Quick start

1. Create an API key in ScoutAPM: [Organization settings](https://scoutapm.com/settings).
2. Store the key in a **secret backend** (1Password, Bitwarden, or KeePassXC) and configure the CLI via the backend's env vars—see below.

### API key (secret backends only)

**Plain-text API keys are not supported.** The CLI does not accept `--api-key` or `API_KEY` / `SCOUT_APM_API_KEY` environment variables. You must use one of the supported secret backends so the key is never on the command line or in shell history.

Resolution order: **1Password** → **Bitwarden** → **KeePassXC**. Each backend is only tried when its environment variables are set.

| Backend     | Env vars | Notes |
|------------|----------|--------|
| **1Password** | `SCOUT_OP_ENTRY_PATH=op://Vault/Item` or `SCOUT_OP_VAULT` + `SCOUT_OP_ITEM` | Optional `SCOUT_OP_FIELD` (default `API_KEY`). Uses `op read`. |
| **Bitwarden** | `SCOUT_BW_ITEM_ID` (login item UUID) | Optional `SCOUT_BW_SESSION` (from `bw unlock --raw`). Uses `bw get password`. |
| **KeePassXC** | `SCOUT_KPXC_DB` (path to .kdbx), `SCOUT_KPXC_ENTRY` (entry title/path) | Optional `SCOUT_KPXC_ATTRIBUTE` (default `Password`). Uses `keepassxc-cli show`. |

Install the CLI for your chosen backend (`op`, `bw`, or `keepassxc-cli`) and ensure the vault is unlocked (e.g. `op signin`, `bw unlock`) when running `scout`.

### Install

**Cargo (from source)**

```bash
cargo install --path scout
# or from git
cargo install --git https://github.com/amkisko/scout-cli.rs --package scout
```

**Homebrew** (macOS/Linux)

```bash
brew tap amkisko/tap  # once
brew install scout-cli
```

Formula lives in [packaging/homebrew/scout-cli.rb](packaging/homebrew/scout-cli.rb). Update the formula's `url` and `sha256` for new releases.

**Nix**

```bash
nix build .#default
# or with flake-utils: nix run .#default
```

See [flake.nix](flake.nix) and [packaging/nix/](packaging/nix/).

**Arch Linux (AUR)**

```bash
yay -S scout-cli
```

PKGBUILD and instructions: [packaging/aur/](packaging/aur/).

**FreeBSD**

Port template in [packaging/freebsd/](packaging/freebsd/). Or install Rust (`pkg install rust`) and run `cargo install --path scout` from the repo.

**Gentoo**

Ebuild template in [packaging/gentoo/](packaging/gentoo/). For a full offline build run `cargo ebuild` from the repo and use the generated ebuild in a local overlay. Or `cargo install --path scout`.

**Flatpak**

See [packaging/flatpak/](packaging/flatpak/). Build may require a Rust-enabled SDK.

## Usage

All [OpenAPI v0.1](doc/openapi.yaml) endpoints are supported. For ScoutAPM API questions or additional endpoints, see [ScoutAPM documentation](https://scoutapm.com/docs).

**Output format:** use `-o` / `--output` to choose how results are printed:

- **plain** (default) — human-readable tables and key-value text
- **json** — JSON (pretty-printed) for scripting or piping

**Interactive TUI:** run `scout` with no arguments to start the interactive TUI and browse apps and endpoints (↑/↓ to select, Enter to load endpoints for the selected app, q or Esc to quit). Timestamps are shown in your local timezone by default; use `--utc` to show UTC only.

```bash
# Plain text (default)
scout apps
scout -o json apps    # JSON output

# Interactive TUI (no arguments)
scout

# Applications
scout apps
scout app 123

# Metrics
scout metrics 123
scout metric 123 response_time --range 7days
scout metric 123 errors --from 2025-01-01T00:00:00Z --to 2025-01-02T00:00:00Z

# Endpoints
scout endpoints 123 --range 1day
scout endpoint-metric 123 <endpoint_id> response_time --range 7days
scout endpoint-traces 123 <endpoint_id> --range 1day

# Traces
scout trace 123 456

# Errors
scout errors 123 [--from ...] [--to ...] [--endpoint <base64>]
scout error 123 789
scout error-group-errors 123 789

# Insights (current + history with pagination)
scout insights 123 [--limit 20]
scout insight 123 n_plus_one [--limit 20]
scout insights-history 123 [--from ...] [--to ...] [--limit 10] [--pagination-cursor ...] [--pagination-direction forward|backward] [--pagination-page 1]
scout insights-history-by-type 123 n_plus_one [same options]

# Utilities
scout parse-url "https://scoutapm.com/apps/123/endpoints/.../trace/456"
scout version
```

API key: configure one secret backend (see above). Plain-text keys are not supported.

## Development

- Format: `cargo fmt --all`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Tests: `cargo test --workspace`
- Release (from repo root): `cargo run -p release` — runs checks, then publish and GitHub release.

## Repository layout

- `scout_lib` — ScoutAPM API client library
- `scout` — CLI binary
- `usr/bin/release` — Rust release script (format, clippy, test, tag, publish)
- `packaging/` — Homebrew, Nix, Flatpak, AUR, FreeBSD (port), Gentoo (ebuild)
- `.github/workflows` — CI (test, format, clippy)

## Contributing

Bug reports and pull requests are welcome on GitHub at https://github.com/amkisko/scout-cli.rs

Contribution policy:
- New features are not necessarily added to the project
- Pull request should have test coverage for affected parts
- Pull request should have changelog entry

Review policy:
- It might take up to 2 calendar weeks to review and merge critical fixes
- It might take up to 6 calendar months to review and merge pull request
- It might take up to 1 calendar year to review an issue

For questions or coordination, see [CONTRIBUTING.md](CONTRIBUTING.md) or open a [GitHub Discussion](https://github.com/amkisko/scout-cli.rs/discussions).

## Security

If you discover a security vulnerability, please report it responsibly. **Do not** open a public issue. See [SECURITY.md](SECURITY.md) for how to report.

## License

MIT. See [LICENSE.md](LICENSE.md).
