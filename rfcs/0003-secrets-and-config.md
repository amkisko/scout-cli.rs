# RFC 0003: Secrets and config

- Feature Name: secrets-and-config
- Type: Standards Track
- Status: Stable
- Created: 2026-08-18
- Author: Andrei Makarov
- Relates: RFC 0002

## Summary

API keys live in a secret backend. The CLI rejects `--api-key` and `SCOUT_APM_API_KEY`. Resolution order is 1Password, then Bitwarden, then KeePassXC. Config precedence is flags, process environment, `config.local.env`, `config.env`, then `.scout.env` or `.env`.

## Motivation

Keys on the command line or in env files land in shell history and process lists. Changing backend order or precedence without a numbered RFC breaks existing operator setups.

## Guide-level explanation

Store the key in 1Password (`op read`), Bitwarden (`bw get password`), or KeePassXC (`keepassxc-cli show`). Point `SCOUT_HOME` or the XDG path `~/.config/scout/` (legacy `~/.scout/`) at that setup.

`scout config path|list|get|set|unset` inspects and writes `config.env` only. Plain-text API keys are rejected. `--output json` works on `list` and `get`.

Friendly keys map to env vars such as `op.entry_path` → `SCOUT_OP_ENTRY_PATH`.

## Reference-level explanation

Each backend is tried only when its settings are present. 1Password uses `SCOUT_OP_ENTRY_PATH` or vault plus item; default field `API_KEY`. Bitwarden uses `SCOUT_BW_ITEM_ID` and optional `SCOUT_BW_SESSION`. KeePassXC uses `SCOUT_KPXC_DB`, `SCOUT_KPXC_ENTRY`, optional `SCOUT_KPXC_ATTRIBUTE` (default `Password`). `scout config set` MUST NOT write a plain-text API key.

## Security considerations

The key is never accepted as a CLI flag or plain env file value.

## Registrar

Env prefixes: `SCOUT_OP_*`, `SCOUT_BW_*`, `SCOUT_KPXC_*`, `SCOUT_HOME`.

## Drawbacks

Operators need a secret manager. Backend order is fixed. Legacy `~/.scout/` must keep resolving.

## Rationale and alternatives

Reading `SCOUT_APM_API_KEY` from the environment would be shorter to document and would put the secret in process listings. A single vendor backend would shrink the matrix and would lock operators in. Doing nothing leaves keys in shell history.

## Prior art

1Password, Bitwarden, KeePassXC CLI. Twelve-factor env files as the anti-pattern for this key. RFC 0002 requires a secret backend.

## Unresolved questions

Whether secret-backend order and config precedence should freeze as a dedicated registrar of env keys.
