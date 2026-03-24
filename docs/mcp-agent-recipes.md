# MCP Agent Recipes

This page is the task-shaped starting point for agent users and MCP integrators.

It is not the normative contract source. Use `SPECS.md` for the exact platform
contract and `ARCHITECTURE.md` for the fuller implementation view.

Guild keeps the MCP surface intentionally small:

- `resources/list` for the first useful durable entry points under the selected root
- `resources/templates/list` for the parameterized Guild URI families
- `resources/read` for durable reads
- `guild.inspect` for inspect-mode execution through the local runtime

Use the workflows below in that order unless you already know the exact URI you
need.

## Recipe 1: Inspect A Skill

Use this when you want to run one installed skill through the real Guild runtime.

1. Call `guild.inspect` with:
   - a requested skill ref such as `skill://example/hello-inspect@^0.1`
   - the input payload for that skill
   - the caller-requested grants the skill should receive for that run
2. Read the returned execution record in `structured_content`.
3. Follow the returned execution `ResourceLink` when you want the durable stored record.
4. Follow any returned evidence metadata links before reading evidence payload bytes.

What to remember:

- `guild.inspect` is not read-only; it persists a new execution record.
- It may also persist evidence records.
- The durable execution record is the first place to look when you want to explain what happened after the run completes.

## Recipe 2: Find An Execution

Use this when you need to discover stored work before you already know an exact
execution URI.

Start with `resources/list`. The first entries are the canonical bounded query
resources:

- `guild://queries/executions/recent/10`
- `guild://queries/executions/failures/recent/10`

Then:

1. Read one of those query URIs through `resources/read`.
2. Inspect the JSON results to find execution receipt URIs.
3. Read a specific `guild://executions/<id>` URI through `resources/read` when you want the full stored record.

Use `resources/templates/list` when you need another query shape such as:

- executions by status
- executions by resolved skill
- a different bounded limit

## Recipe 3: Fetch Evidence Safely

Use this when you already have an evidence record URI or when an execution/query
result points you at stored evidence.

Start with the metadata resource, not the payload:

1. Read `guild://objects/records/<evidence-record-id>/metadata` through `resources/read`.
2. Inspect the JSON metadata to confirm:
   - MIME type
   - size
   - audience
   - redaction class
   - producing execution, when present
3. Read `guild://objects/records/<evidence-record-id>` only when you actually need the payload bytes.

Use `guild://objects/sha256/<digest>` only when you explicitly need the raw
content-addressed blob URI.

## Recipe 4: Explain A Failure

Use this when a run failed or was rejected and you want the durable record
first, not a rerun.

The fastest MCP path is:

1. Read `guild://queries/executions/failures/recent/10` through `resources/read`.
2. Pick the failed or rejected execution URI you care about.
3. Read that `guild://executions/<id>` URI through `resources/read`.
4. Inspect the stored status, policy decision, termination detail, nearby child executions, and evidence references.

When your Guild root also has one of the explainer skills installed, you can
use `guild.inspect` with that skill on the stored execution URI for a richer
report. The important boundary is the same either way:

- `resources/read` explains stored state
- `guild.inspect` executes another skill over that stored state

## Keep Going

- Read [`README.md`](../README.md) for the compact product overview.
- Read [`docs/command-language.md`](command-language.md) for the full public CLI and MCP wording.
- Read [`docs/how-guild-works.md`](how-guild-works.md) for the short operator mental model.
- Read [`docs/testing.md`](testing.md) when you need proof commands and repo-local smoke coverage.
