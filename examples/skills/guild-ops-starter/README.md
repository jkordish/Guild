# Guild Ops Starter Pack

This is a starter pack of ordinary example skills. There is no new pack type here.

The point of the pack is narrow: show useful local operational analysis over real persisted Guild artifacts without widening runtime, proof, token, or witness semantics.

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

cargo run -q -p guild-mcp --bin guild -- install examples/skills/render-report
cargo run -q -p guild-mcp --bin guild -- install examples/skills/incident-brief
cargo run -q -p guild-mcp --bin guild -- install examples/skills/run-diff
cargo run -q -p guild-mcp --bin guild -- install examples/skills/recent-failures
cargo run -q -p guild-mcp --bin guild -- install examples/skills/evidence-summary

cargo run -q -p guild-mcp --bin guild -- verify skill://example/incident-brief@^0.1
```

## Try It

Use the existing deterministic repo-local scenario prep to get real execution and query refs:

```bash
cargo run -p guild-mcp --bin guild -- codex bootstrap --registry-root "$GUILD_REGISTRY_ROOT" --reset
cargo run -p guild-mcp --bin guild -- codex scenario --registry-root "$GUILD_REGISTRY_ROOT" --scenario recent-failure-triage --json
```

That JSON includes:

- `subject_execution_uris`
- `comparison_execution_uris`
- `query_uris`

Use those real refs directly with the pack skills.

Run `incident-brief` on one stored execution ref:

```bash
cargo run -q -p guild-mcp --bin guild -- --registry-root "$GUILD_REGISTRY_ROOT" run \
  incident-brief@^0.1 \
  --input-json '{"execution_uri":"<paste one subject_execution_uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}},{"id":"invoke-skill","access":"invoke","constraints":{"aliases":["renderer"]}}]}'
```

Run `run-diff` on two stored execution refs:

```bash
cargo run -q -p guild-mcp --bin guild -- --registry-root "$GUILD_REGISTRY_ROOT" run \
  run-diff@^0.1 \
  --input-json '{"left_execution_uri":"<paste first execution_uri>","right_execution_uri":"<paste second execution_uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}},{"id":"invoke-skill","access":"invoke","constraints":{"aliases":["renderer"]}}]}'
```

Run `recent-failures` on the bounded query ref:

```bash
cargo run -q -p guild-mcp --bin guild -- --registry-root "$GUILD_REGISTRY_ROOT" run \
  recent-failures@^0.1 \
  --input-json '{"query_uri":"<paste one query_uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://queries/executions/"],"resource_kinds":["query"]}},{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}}]}'
```

Optionally inspect the last report run with the normal CLI:

```bash
guild ls runs --limit 5
guild why exec:<execution-id-prefix>
```

## Evidence Add-On

`evidence-summary` needs a real stored evidence ref. One easy way to get one is the existing `hello-inspect` example:

```bash
cargo run -q -p guild-mcp --bin guild -- install examples/skills/hello-inspect

cargo run -q -p guild-mcp --bin guild -- --registry-root "$GUILD_REGISTRY_ROOT" run \
  hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}' \
  --json
```

Take one emitted evidence URI from that JSON and run:

```bash
cargo run -q -p guild-mcp --bin guild -- --registry-root "$GUILD_REGISTRY_ROOT" run \
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
