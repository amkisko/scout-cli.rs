# scoutapm-cli

ScoutAPM command-line client. Query apps, endpoints, traces, metrics, errors,
usage, and billing. Binary name is `scout`.

The crates.io package is `scoutapm-cli` because `scout` and `scout-cli` are
already taken by unrelated projects.

## Install

```bash
cargo install scoutapm-cli
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
