# Evidence Summary

`evidence-summary` explains one stored evidence record without pretending
`emit-evidence` is proof-backed today.

It answers:

- what the evidence record is
- which execution produced it
- which sink and blob linkage it uses
- which normalized details are safely readable now
- which refs to inspect next

Canonical shape:

```bash
cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/ops-pack install examples/skills/evidence-summary
cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/ops-pack run skill://example/evidence-summary@^0.1 --input-json '{"evidence_uri":"guild://objects/records/<evidence-record-id>"}' --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://objects/records/"],"resource_kinds":["object"]}}]}'
```

The report reads metadata first and only normalizes payload details when the
stored payload is already small and readable.
