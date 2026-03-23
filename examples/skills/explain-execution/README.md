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
guild --registry-root target/dev-local-registry/explain-execution install examples/skills/hello-inspect
guild --registry-root target/dev-local-registry/explain-execution run skill://example/hello-inspect@^0.1 --input-json '{"name":"Ada"}' --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}'
guild --registry-root target/dev-local-registry/explain-execution install examples/skills/explain-execution
guild --registry-root target/dev-local-registry/explain-execution run skill://example/explain-execution@^0.1 --input-json '{"execution_uri":"guild://executions/<execution-id>","include_first_evidence":true}' --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/","guild://objects/records/"],"resource_kinds":["execution","object"]}}]}'
guild --registry-root target/dev-local-registry/explain-execution get guild://executions/<execution-id>
```

Replace `<execution-id>` with the durable execution id returned by the `hello-inspect` run receipt.

Deterministic helper proofs:

```bash
guild codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution
cargo run -p guild-mcp --example explain_execution_local
cargo run -p guild-mcp --example explain_failure_local
cargo run -p guild-mcp --example codex_explain_execution_local
```

That flow:

1. installs `hello-inspect`
2. runs `guild run` to produce a stored `guild://executions/...` URI
3. installs `explain-execution`
4. executes `explain-execution` against the stored URI through the same Wasmtime-backed path

`explain_failure_local` uses the same skill to explain a persisted rejected execution record returned through an MCP error receipt URI.

For real Codex dogfooding, first bootstrap a local Guild root with:

```bash
guild codex bootstrap --registry-root target/dev-local-registry/codex-local --reset
```

Then add Guild to Codex with the printed stdio config and ask Codex to run `hello-inspect` followed by `explain-execution` against the returned execution URI. `codex_explain_execution_local` is the deterministic MCP-path smoke version of that same flow.

If you want the same deterministic flow without leaving the helper, run:

```bash
guild codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution
```

`codex_explain_execution_local` remains as the lower-level compatibility smoke command and now wraps that same shared helper path.

Imported verified skills produce the same local execution resources. That means `explain-execution` can also be used against records created by the native signed-bundle, OCI image layout, and OCI registry portability proof flows after import.

The required `read-resource` capability is constrained to local Guild execution and object URIs:

- `guild://executions/`
- `guild://objects/records/`

The current typed grant shape is:

- `resource_kinds: ["execution", "object"]`
- `uri_prefixes: ["guild://executions/", "guild://objects/records/"]`

Those `uri_prefixes` are canonical Guild scope roots, not free-form string prefixes. When the explain skill reads an evidence-record URI, the host parses that Guild URI, authorizes it against the canonical scope set, then dereferences the record through the same local backend and returns the referenced payload. The companion metadata URI for that same emission now lives at `guild://objects/records/{evidence_record_id}/metadata` under the same object-record scope. Authorization denials stay host-owned rejections rather than guest-domain failures.
