# Incident Casefile

`incident-casefile` turns one stored Guild execution, plus optional
comparison, query, and evidence refs, into one compact markdown casefile.

It answers:

- what happened in the subject execution
- what nearby comparison, query, and evidence refs add context
- which exact Guild refs the report used
- which `guild why`, `guild get`, and `guild show` follow-ups matter next

Canonical shape:

```bash
guild --registry-root target/dev-local-registry/ops-pack install examples/skills/incident-casefile
guild --registry-root target/dev-local-registry/ops-pack run skill://example/incident-casefile@^0.1 --input-json '{"subject_execution_uri":"guild://executions/<subject-execution-id>","comparison_execution_uri":"guild://executions/<comparison-execution-id>","query_uri":"guild://queries/executions/failures/recent/10","evidence_uri":"guild://objects/records/<evidence-record-id>"}' --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}},{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://queries/executions/"],"resource_kinds":["query"]}},{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://objects/records/"],"resource_kinds":["object"]}}]}'
```

Smallest valid shape:

```bash
guild --registry-root target/dev-local-registry/ops-pack run skill://example/incident-casefile@^0.1 --input-json '{"subject_execution_uri":"guild://executions/<subject-execution-id>"}' --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}}]}'
```

If you want a concrete starting point before composing that `--grants-json`
payload, begin with:

```bash
guild grants template read-resource
```

Then keep only the bounded `guild://executions/`, `guild://queries/executions/`,
and `guild://objects/records/` scopes you actually need for the report you are
building. Query and evidence scopes are optional and should only be granted
when the matching `query_uri` or `evidence_uri` is present in the input.

This skill does not widen execution semantics. It stays inside the current
inspect-only `read-resource` surface, assembles one report directly, and avoids
the formatter-child composition path entirely.
