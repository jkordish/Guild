# Run Diff

`run-diff` compares two stored Guild executions and reports the bounded changes
that actually show up in the persisted records.

It answers:

- did status change
- did the primary reason change
- did child execution linkage change
- did evidence linkage change
- which refs are worth opening next

Canonical shape:

```bash
guild --registry-root target/dev-local-registry/ops-pack install examples/skills/render-report
guild --registry-root target/dev-local-registry/ops-pack install examples/skills/run-diff
guild --registry-root target/dev-local-registry/ops-pack run skill://example/run-diff@^0.1 --input-json '{"left_execution_uri":"guild://executions/left","right_execution_uri":"guild://executions/right"}' --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}},{"id":"invoke-skill","access":"invoke","constraints":{"aliases":["renderer"]}}]}'
```

This is a conservative record diff, not semantic replay or root-cause inference.
