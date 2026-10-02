# scout-cli

ScoutAPM command-line client. Query apps, endpoints, traces, metrics, errors,
usage, and billing. Binary name is `scout`.

The crates.io package is `scout-cli` because `scout` is already taken by an
unrelated fuzzy-finder crate.

## Install

```bash
cargo install scout-cli
# or from the repository path:
cargo install --path scout
```

Store the ScoutAPM API key in a secret backend (1Password, Bitwarden, or
KeePassXC). See the repository README for config layout.

## Library

Shared client code lives in [`scout_lib`](https://crates.io/crates/scout_lib).

## Links

- Repository: <https://github.com/amkisko/scout-cli.rs>
- Changelog: <https://github.com/amkisko/scout-cli.rs/blob/main/CHANGELOG.md>
