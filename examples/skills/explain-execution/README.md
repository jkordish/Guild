# Explain Execution

`explain-execution` is an inspect-only example skill that reads a stored Guild execution resource through the host-mediated `read-resource` import and returns a structured explanation.

It proves:

- stored execution records are reusable inputs, not just return values
- evidence/object URIs can be consumed through the same local resource backend used by MCP
- `EvidenceRef` values identify per-emission evidence records, not just payload digests
- resource access stays capability-scoped and host-mediated
- failed and rejected execution records can be explained without requiring a skill-authored output

The skill expects an execution URI and can optionally read the first linked evidence object.

Canonical local proof flow:

```bash
cargo run -p guild-mcp --example explain_execution_local
cargo run -p guild-mcp --example explain_failure_local
cargo run -p guild-mcp --example codex_explain_execution_local
```

That command:

1. installs `hello-inspect`
2. runs `guild.inspect` to produce a stored execution URI
3. installs `explain-execution`
4. executes `explain-execution` against the stored URI through the same Wasmtime-backed path

`explain_failure_local` uses the same skill to explain a persisted rejected execution record returned through an MCP error receipt URI.

For real Codex dogfooding, first bootstrap a local Guild root with:

```bash
cargo run -p guild-mcp --bin guild-codex -- bootstrap --registry-root target/dev-local-registry/codex-local --reset
```

Then add Guild to Codex with the printed stdio config and ask Codex to run `hello-inspect` followed by `explain-execution` against the returned execution URI. `codex_explain_execution_local` is the deterministic MCP-path smoke version of that same flow.

Imported verified skills produce the same local execution resources. That means `explain-execution` can also be used against records created by the native signed-bundle, OCI image layout, and OCI registry portability proof flows after import.

The required `read-resource` capability is constrained to local Guild execution and object URIs:

- `guild://executions/`
- `guild://objects/records/`

The current typed grant shape is:

- `resource_kinds: ["execution", "object"]`
- `uri_prefixes: ["guild://executions/", "guild://objects/records/"]`

Those `uri_prefixes` are canonical Guild scope roots, not free-form string prefixes. When the explain skill reads an evidence-record URI, the host parses that Guild URI, authorizes it against the canonical scope set, then dereferences the record through the same local backend and returns the referenced payload. Authorization denials stay host-owned rejections rather than guest-domain failures.
