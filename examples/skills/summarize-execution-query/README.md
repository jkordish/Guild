# Summarize Execution Query

`summarize-execution-query` is an inspect-only example skill that reads one bounded Guild execution query resource through the host-mediated `read-resource` import and returns a deterministic structured report.

It proves:

- stored execution artifacts are discoverable through bounded query resources, not only exact execution URIs
- guest-side `read-resource` and MCP resource reads can consume the same query result payloads
- query access stays capability-scoped and host-mediated
- failed and rejected executions can be summarized honestly without turning Guild into a general search engine

Canonical local proof flow:

```bash
cargo run -p guild-mcp --example explain_recent_failures_local
```

That command:

1. installs `inspect-http-json`
2. produces one succeeded, one failed, and one rejected persisted execution
3. reads `guild://queries/executions/failures/recent/10` directly through the Guild resource backend
4. installs `summarize-execution-query`
5. executes the summary skill against that query URI through the same Wasmtime-backed path

The required `read-resource` capability is constrained to the canonical Guild execution-query scope:

- `guild://queries/executions/`

The current typed grant shape is:

- `resource_kinds: ["query"]`
- `uri_prefixes: ["guild://queries/executions/"]`

This is intentionally a bounded local artifact-reuse layer, not full-text search, subscriptions, or a general analytics surface.
