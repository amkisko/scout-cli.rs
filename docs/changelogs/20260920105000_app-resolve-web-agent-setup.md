# App resolve, web open, and agent setup

## Participants

Andrei Makarov

## Decisions

Ship RFC 0006 as Proposed with implementation: APP accepts id or name, `--web` opens or prints UI URLs, `scout setup` prefers pray and Prayfile for the scout-cli agent skill.

## Effects

Prayfile provisions `scout/scout-cli` from `prayers/scout-cli` under `.agents/skills/scout-cli` and publishes the catalog to `prayers/v1/`. Config keys `app.id` and `app.name` are writable targeting defaults.

## Next

Discuss RFC 0006 until 2026-10-04. Decide whether setup should write a Prayfile stanza automatically.

## Source

RFC 0006. Implementation in this change set.
