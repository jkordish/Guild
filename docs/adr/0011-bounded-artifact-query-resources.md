# ADR 0011: Bounded Artifact Query Resources

- Status: accepted
- Date: 2026-03-17

## Context

Guild already persists durable host-owned execution and evidence artifacts and already exposes direct artifact reads through Guild resources, guest `read-resource`, and MCP `resources/read`.

That direct-read model is useful once a caller already has an exact URI such as `guild://executions/{execution_id}` or `guild://objects/records/{evidence_record_id}`.

What was missing was a bounded, resource-shaped way to discover relevant persisted artifacts without widening the public MCP tool surface or introducing a general search engine.

The next step needed to preserve Guild's current shape was:

- local-first only
- inspect-only only
- deterministic and bounded
- host-mediated and fail-closed
- exposed as Guild resources and templates rather than a new family of MCP tools

## Decision

Guild adds a bounded local execution-query layer over the canonical persisted execution store.

The current repository exposes that layer as canonical Guild query resource URIs under:

- `guild://queries/executions/recent/{limit}`
- `guild://queries/executions/failures/recent/{limit}`
- `guild://queries/executions/by-status/{status}/{limit}`
- `guild://queries/executions/by-skill/{namespace}/{name}/{limit}`

The current repository also adds:

- `ResourceKind::Query`
- `GuildResourceScope::ExecutionQuery`
- bounded query result types built from canonical execution records
- query resource templates on the MCP resource surface

The current repository does not add:

- a new public MCP tool
- subscriptions or list-changed notifications
- full-text search
- arbitrary boolean query DSLs
- remote or distributed indexing
- broader evidence-specific query resources in this milestone

## Why This Shape

This decision preserves Guild's existing boundaries:

- persisted execution records remain the canonical truth
- query results are derived host-owned summaries, not guest-authored state
- guest and MCP resource reads still use the same backend path
- authorization remains resource-scoped and fail-closed
- `guild.inspect` remains the one stable public MCP tool

This also keeps the model honest:

- query resources are not direct artifact resources
- query URIs are canonical host-defined contracts, not arbitrary search strings
- result limits and ordering are explicit and deterministic

## Current Repository Rules

The current repository implements the following rules:

1. Query resources are execution-query resources only.
2. Query limits are bounded to `1..=50`.
3. Query results are sorted deterministically by:
   - `finished_at_utc` descending
   - `started_at_utc` descending
   - `execution_id` descending
4. Query results are structured summaries containing canonical execution and evidence URIs rather than raw search-engine documents.
5. Query authorization requires the explicit `guild://queries/executions/` scope root plus `resource_kind = query`.
6. Exact execution or object grants do not implicitly authorize query resources.
7. Guest `read-resource`, host resource reads, and MCP `resources/read` all use the same query backend.

## Consequences

Positive consequences:

- persisted failures and rejections become much more reusable
- explain/debug style skills can start from bounded discovery resources rather than exact URIs only
- the public MCP surface stays small
- the implementation stays local, deterministic, and auditable

Trade-offs:

- the current repository scans persisted execution records rather than maintaining a separate secondary index
- evidence-specific query surfaces are deferred
- broader analytics, subscriptions, and full-text search remain out of scope

## Deferred Work

This ADR intentionally does not solve:

- subscriptions or change notifications
- evidence-specific query resources
- full-text search
- remote or distributed search/index infrastructure
- retention and garbage-collection policy
- richer analytics or dashboard layers
