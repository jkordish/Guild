# Recent Failures

`recent-failures` summarizes a bounded execution-query resource and follows the
returned execution refs to produce a compact operator report.

It answers:

- which stored executions are failing or being refused now
- which reason codes dominate the bounded result set
- how many results are `upper-bound` vs `refused`
- which executions are the best next refs to inspect

Canonical shape:

```bash
cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/ops-pack install examples/skills/recent-failures
cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/ops-pack run skill://example/recent-failures@^0.1 --input-json '{"query_uri":"guild://queries/executions/failures/recent/10"}' --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://queries/executions/"],"resource_kinds":["query"]}},{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}}]}'
```

This stays inside the current bounded query surface. It does not perform any
extra search outside the returned execution refs.
