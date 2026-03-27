# Guild Ops Starter

Guild Ops Starter is the first operator starter set in the repo and the first reference application built on that trust chain. It is not the whole product story.

This is a small set of ordinary example skills. There is no new pack type here.

The point of this example set is narrow: show useful local operational analysis over real persisted Guild execution receipts, bounded query resources, and evidence artifacts without widening runtime, proof, token, or witness semantics.

If you want one current user-facing Guild workflow, start here.

## Journey Map

This example set is organized around four practical questions:

- explain one stored execution
- compare two stored executions
- scan recent failures inside one bounded query
- inspect one stored evidence record

Keep using the normal CLI around those richer summaries:

- `guild ls` to see recent local activity
- `guild why` to explain one stored execution
- `guild get` to read the raw stored resource
- `guild show` to summarize one stored evidence ref

## Skills

| Skill | Question it answers | Input | Required capabilities |
| --- | --- | --- | --- |
| `incident-brief` | What happened in this one stored execution? | one `guild://executions/<id>` ref | `read-resource` on `guild://executions/`, `invoke-skill` for alias `renderer` |
| `run-diff` | What changed between these two stored executions? | two `guild://executions/<id>` refs | `read-resource` on `guild://executions/`, `invoke-skill` for alias `renderer` |
| `recent-failures` | What do the latest failed or refused executions look like inside this bounded query? | one `guild://queries/executions/...` ref | `read-resource` on `guild://queries/executions/` and `guild://executions/` |
| `evidence-summary` | What is this stored evidence record and why does it exist? | one `guild://objects/records/<id>` ref | `read-resource` on `guild://objects/records/` |
| `render-report` | Format a normalized report as compact markdown | normalized JSON report input | none |

`render-report` is the only composition demo in this example set. It is used as an exact single zero-authority child by `incident-brief` and `run-diff`. There is no fan-out, no recursion, and no hidden orchestration.

## Install The Example Set

```bash
export GUILD_REGISTRY_ROOT=target/dev-local-registry/ops-pack

guild install examples/skills/render-report
guild install examples/skills/incident-brief
guild install examples/skills/run-diff
guild install examples/skills/recent-failures
guild install examples/skills/evidence-summary

guild verify skill://example/incident-brief@^0.1
```

## Prepare Real Refs

Use the existing deterministic repo-local scenario prep to get real execution and query refs:

```bash
guild codex bootstrap --registry-root "$GUILD_REGISTRY_ROOT" --reset
guild codex scenario --registry-root "$GUILD_REGISTRY_ROOT" --scenario recent-failure-triage --json
```

That JSON includes:

- `subject_execution_uris`
- `comparison_execution_uris`
- `query_uris`

Use those real refs directly with these skills.

The repo-local scenario prep is only there to hand you real stored refs quickly.
The workflows below still use the normal installed `guild` CLI and the same
durable Guild resources you would read in day-to-day use.

## Journey 1: Explain One Stored Execution

Start with the primary CLI explanation path:

```bash
guild why <paste one subject_execution_uri>
guild why --lineage <paste one subject_execution_uri>
```

Then run `incident-brief` when you want a compact markdown report over that
same stored execution:

```bash
guild run \
  incident-brief@^0.1 \
  --input-json '{"execution_uri":"<paste one subject_execution_uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}},{"id":"invoke-skill","access":"invoke","constraints":{"aliases":["renderer"]}}]}'
```

## Journey 2: Compare Two Stored Executions

Use `run-diff` when you want one compact report for two stored executions:

```bash
guild run \
  run-diff@^0.1 \
  --input-json '{"left_execution_uri":"<paste first execution_uri>","right_execution_uri":"<paste second execution_uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}},{"id":"invoke-skill","access":"invoke","constraints":{"aliases":["renderer"]}}]}'
```

## Journey 3: Scan Recent Failures

Start with the raw bounded query if you want to see the stored resource:

```bash
guild get <paste one query_uri>
```

Then run `recent-failures` when you want the compact grouped summary:

```bash
guild run \
  recent-failures@^0.1 \
  --input-json '{"query_uri":"<paste one query_uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://queries/executions/"],"resource_kinds":["query"]}},{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}}]}'
```

## Keep Going With The Normal CLI

After any of those runs, keep using the daily CLI to inspect the same refs:

```bash
guild ls runs --limit 5
guild why exec:<execution-id-prefix>
guild get guild://executions/<execution-id>
```

## Journey 4: Discover And Inspect One Stored Evidence Record

If your selected Guild root already has stored evidence, start discovery with:

```bash
guild ls evidence --limit 5
```

`evidence-summary` still needs one real stored evidence ref. If your root is empty, one easy way to create one is the existing `hello-inspect` example:

```bash
guild install examples/skills/hello-inspect

guild run \
  hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}' \
  --json
```

Start with the normal CLI summary for that evidence ref:

```bash
guild show <paste one emitted evidence uri>
```

Then run `evidence-summary` for the richer markdown report:

```bash
guild run \
  evidence-summary@^0.1 \
  --input-json '{"evidence_uri":"<paste one emitted evidence uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://objects/records/"],"resource_kinds":["object"]}}]}'
```

## What This Example Set Does Not Show

- It does not broaden `invoke-skill`. Composition stays inside the current bounded zero-authority inspect slices, and this pack itself still uses the single-child formatter path.
- It does not use broad `http-request` as the hero path.
- It does not use `emit-evidence` as a proof claim. `emit-evidence` remains explicitly `not_proven`.
- It does not add a workflow engine, a pack manifest, or a new installer abstraction.
- It does not dump raw JSON by default. The report skills return compact markdown strings so `guild run` prints useful terminal output directly.
