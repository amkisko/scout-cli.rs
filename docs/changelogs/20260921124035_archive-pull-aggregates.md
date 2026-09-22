# Archive pull aggregates, windows, and metric tuples

## Participants

Andrei Makarov

## Decisions

Ship archive pull so a full cache is aggregates plus insights, not every trace. Accept live tuple metric series. Clamp each resource to Scout lookback with 1 hour slack. Continue after a refused window. Checkpoint the manifest after each resource and reindex disk files. `--range max` is 30 days before clamps. `--trace-endpoint-limit 0` means all.

## Effects

`archive pull --resource metrics` writes daily buckets from `[timestamp, value]` points. Default resources include insights, endpoint_metrics, and job_metrics and omit traces. Help and `--dry-run` document caps and the default set. Tests cover tuple merge, 10000-point merge ceiling, skip reindex, orphan traces, zero scan limit, and help text.

## Next

Bench default series pull at 50 endpoints over a 30-day window before calling that path cheap. Operators with leftover `traces/by_id` files can delete that tree if they want the disk back.

## Source

Issue `usr/docs/issues/20260921124035_archive-pull-windows-and-metrics.md`. Unreleased CHANGELOG bullets in this change set.
