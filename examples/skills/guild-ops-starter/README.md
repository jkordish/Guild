# Guild Ops Starter Pack

This is a starter pack of ordinary example skills. There is no new pack type here.

The point of the pack is narrow: show useful local operational analysis over real persisted Guild artifacts without widening runtime, proof, token, or witness semantics.

If you want one current user-facing Guild workflow, start here.

## Skills

| Skill | Question it answers | Input | Required capabilities |
| --- | --- | --- | --- |
| `incident-brief` | What happened in this one stored execution? | one `guild://executions/<id>` ref | `read-resource` on `guild://executions/`, `invoke-skill` for alias `renderer` |
| `run-diff` | What changed between these two stored executions? | two `guild://executions/<id>` refs | `read-resource` on `guild://executions/`, `invoke-skill` for alias `renderer` |
| `recent-failures` | What do the latest failed or refused executions look like inside this bounded query? | one `guild://queries/executions/...` ref | `read-resource` on `guild://queries/executions/` and `guild://executions/` |
| `evidence-summary` | What is this stored evidence record and why does it exist? | one `guild://objects/records/<id>` ref | `read-resource` on `guild://objects/records/` |
| `render-report` | Format a normalized report as compact markdown | normalized JSON report input | none |

`render-report` is the only composition demo in the pack. It is used as an exact single zero-authority child by `incident-brief` and `run-diff`. There is no fan-out, no recursion, and no hidden orchestration.

## Install

```bash
export GUILD_REGISTRY_ROOT=target/dev-local-registry/ops-pack

guild install examples/skills/render-report
guild install examples/skills/incident-brief
guild install examples/skills/run-diff
guild install examples/skills/recent-failures
guild install examples/skills/evidence-summary

guild verify skill://example/incident-brief@^0.1
```

## First Five Minutes

Use the existing deterministic repo-local scenario prep to get real execution and query refs:

```bash
guild codex bootstrap --registry-root "$GUILD_REGISTRY_ROOT" --reset
guild codex scenario --registry-root "$GUILD_REGISTRY_ROOT" --scenario recent-failure-triage --json
```

That JSON includes:

- `subject_execution_uris`
- `comparison_execution_uris`
- `query_uris`

Use those real refs directly with the pack skills.

Core workflow:

1. Run `incident-brief` on one stored execution ref:

```bash
guild run \
  incident-brief@^0.1 \
  --input-json '{"execution_uri":"<paste one subject_execution_uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}},{"id":"invoke-skill","access":"invoke","constraints":{"aliases":["renderer"]}}]}'
```

2. Run `run-diff` on two stored execution refs:

```bash
guild run \
  run-diff@^0.1 \
  --input-json '{"left_execution_uri":"<paste first execution_uri>","right_execution_uri":"<paste second execution_uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}},{"id":"invoke-skill","access":"invoke","constraints":{"aliases":["renderer"]}}]}'
```

3. Optionally run `recent-failures` on the bounded query ref:

```bash
guild run \
  recent-failures@^0.1 \
  --input-json '{"query_uri":"<paste one query_uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://queries/executions/"],"resource_kinds":["query"]}},{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}}]}'
```

4. Inspect the resulting refs with the normal CLI:

```bash
guild ls runs --limit 5
guild why exec:<execution-id-prefix>
```

## Evidence Add-On

`evidence-summary` needs a real stored evidence ref. One easy way to get one is the existing `hello-inspect` example:

```bash
guild install examples/skills/hello-inspect

guild run \
  hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}' \
  --json
```

Take one emitted evidence URI from that JSON and run:

```bash
guild run \
  evidence-summary@^0.1 \
  --input-json '{"evidence_uri":"<paste one emitted evidence uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://objects/records/"],"resource_kinds":["object"]}}]}'
```

## What This Pack Does Not Show

- It does not broaden `invoke-skill`. Composition stays inside the current exact single-child zero-authority slice.
- It does not use broad `http-request` as the hero path.
- It does not use `emit-evidence` as a proof claim. `emit-evidence` remains explicitly `not_proven`.
- It does not add a workflow engine, a pack manifest, or a new installer abstraction.
- It does not dump raw JSON by default. The report skills return compact markdown strings so `guild run` prints useful terminal output directly.
