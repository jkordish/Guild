# Guild Ops Starter Quickstart

This is the shortest honest starter workflow in the repo today.

It uses the deterministic repo-local dogfood path to hand you real stored
Guild refs, then it keeps the actual review flow on the normal installed CLI:
`guild why`, `guild why --lineage`, and one `incident-casefile` run.

## Quickstart

```bash
export GUILD_REGISTRY_ROOT=target/dev-local-registry/ops-pack

guild codex bootstrap --registry-root "$GUILD_REGISTRY_ROOT" --reset
guild codex scenario --registry-root "$GUILD_REGISTRY_ROOT" --scenario recent-failure-triage --json
```

`guild codex bootstrap` installs the starter skills used by the recommended
scenario and smoke flows under that explicit repo-local root.

Copy one `subject_execution_uri`, one `comparison_execution_uri`, and one
`query_uri` from that scenario JSON.

Review the subject execution in the native CLI first:

```bash
guild why <paste one subject_execution_uri>
guild why --lineage <paste one subject_execution_uri>
```

Then build one compact casefile over those same stored refs:

```bash
guild run \
  incident-casefile@^0.1 \
  --input-json '{"subject_execution_uri":"<paste one subject_execution_uri>","comparison_execution_uri":"<paste one comparison_execution_uri>","query_uri":"<paste one query_uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}},{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://queries/executions/"],"resource_kinds":["query"]}}]}'
```

If you already have one concrete evidence record URI, extend the input and the
bounded read grants:

```bash
guild run \
  incident-casefile@^0.1 \
  --input-json '{"subject_execution_uri":"<paste one subject_execution_uri>","comparison_execution_uri":"<paste one comparison_execution_uri>","query_uri":"<paste one query_uri>","evidence_uri":"<paste one evidence_uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}},{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://queries/executions/"],"resource_kinds":["query"]}},{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://objects/records/"],"resource_kinds":["object"]}}]}'
```

## What This Proves

- one deterministic setup path to real stored refs
- native CLI first for execution explanation
- one cohesive starter artifact for compact incident review
- bounded `read-resource` only; no mutation hidden inside the starter flow

## What This Does Not Claim

- It does not imply that restart, notification, or incident-write actions already ship as runnable starter steps.
- It does not add a first-class rerun surface.
- It does not add a workflow engine, pack manifest, or new MCP surface.
- It does not widen the trust model beyond current execution, query, and object-record reads.
