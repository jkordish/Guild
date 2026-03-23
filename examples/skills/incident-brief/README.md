# Incident Brief

`incident-brief` turns one stored Guild execution into a compact markdown brief.

It answers:

- what ran
- how it finished
- whether the current posture is `proof-backed`, `upper-bound`, or `refused`
- which reason to inspect first
- which nearby child or evidence refs matter next

Canonical shape:

```bash
guild --registry-root target/dev-local-registry/ops-pack install examples/skills/render-report
guild --registry-root target/dev-local-registry/ops-pack install examples/skills/incident-brief
guild --registry-root target/dev-local-registry/ops-pack run skill://example/incident-brief@^0.1 --input-json '{"execution_uri":"guild://executions/<execution-id>"}' --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}},{"id":"invoke-skill","access":"invoke","constraints":{"aliases":["renderer"]}}]}'
```

This skill does not broaden execution semantics. It reads one stored execution
record, derives a compact report, and uses the zero-authority `render-report`
child once for formatting only.
