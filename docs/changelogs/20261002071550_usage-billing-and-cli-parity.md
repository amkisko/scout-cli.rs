# Usage, billing, and CLI parity

## Decisions

Ship RFC 0007 surfaces: usage, billing, compact relative times, job full-name encode, pipe auto-JSON, scout docs, chart honesty, and related query polish.

## Effects

Library helpers for resource ids, time parse, usage math, and org `/usage`. CLI commands usage, billing, docs. TUI charts downsample by peak magnitude and drop partial buckets. OpenAPI moved to `docs/openapi.yaml`. Usage fan-out capped by `--concurrency` (default 4). Tests cover helpers, chart stats, concurrency, docs, and help text.

## Next

Live API smoke for usage/billing against a real org key. Shipped in 0.6.0; see docs/changelogs/20261002075804_scout-cli-0.6.0.md.

## Source

- rfcs/0007-usage-billing-and-cli-parity.md
- docs/issues/20261002070606_usage-billing-and-cli-parity.md
