# Explain Execution Tree

`explain-execution-tree` is an inspect-only example skill that walks stored Guild execution lineage through the host-mediated `read-resource` import and returns a deterministic tree report.

It proves:

- persisted execution records are rich enough to explain parent and child lineage without adding a search subsystem
- the same Guild execution and evidence resources exposed through MCP can be consumed by a Wasm guest through capability-scoped reads
- execution tree explanation stays local, inspect-only, and bounded rather than turning into a workflow engine
- failed and rejected records remain explainable because execution and evidence identity are host-owned durable artifacts

The skill expects a root execution URI and supports bounded traversal:

- `max_depth` defaults to `4` and is hard-capped at `8`
- `max_nodes` defaults to `32` and is hard-capped at `128`
- traversal is depth-first and follows stored child order
- revisits, missing descendants, and optional evidence-read gaps are surfaced as structured warnings

Evidence handling stays intentionally small:

- counts and summary buckets are derived from stored `ExecutionRecord.emitted_evidence`
- the report includes a bounded list of notable evidence URIs
- optional evidence resource reads return only lightweight descriptors, not raw payloads
- if object-read scope is not granted, those optional reads are skipped and reported honestly

Canonical local proof flow:

```bash
guild --registry-root target/dev-local-registry/explain-execution-tree install examples/skills/hello-inspect
guild --registry-root target/dev-local-registry/explain-execution-tree install examples/skills/hello-composite
guild --registry-root target/dev-local-registry/explain-execution-tree run skill://example/hello-composite@^0.1 --input-json '{"name":"Ada"}' --grants-json '{"grants":[{"id":"invoke-skill","access":"write","constraints":{"aliases":["hello"]}},{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}'
guild --registry-root target/dev-local-registry/explain-execution-tree install examples/skills/explain-execution-tree
guild --registry-root target/dev-local-registry/explain-execution-tree run skill://example/explain-execution-tree@^0.1 --input-json '{"execution_uri":"guild://executions/<root-execution-id>","max_depth":4,"max_nodes":32,"include_evidence_resources":true}' --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/","guild://objects/records/"],"resource_kinds":["execution","object"]}}]}'
```

Replace `<root-execution-id>` with the durable execution id returned by the `hello-composite` run receipt.

Deterministic helper proofs:

```bash
guild codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution-tree
cargo run -p guild-mcp --example explain_execution_tree_local
cargo run -p guild-mcp --example codex_explain_execution_tree_local
```

That flow:

1. installs `hello-inspect`
2. installs `hello-composite`
3. runs `guild run` to produce a stored parent and child execution tree under durable `guild://executions/...` URIs
4. installs `explain-execution-tree`
5. executes `explain-execution-tree` against the stored root execution URI through the same Wasmtime-backed path

For real Codex dogfooding, first bootstrap a local Guild root with:

```bash
guild codex bootstrap --registry-root target/dev-local-registry/codex-local --reset
```

Then add Guild to Codex with the printed stdio config and ask Codex to run `hello-composite` followed by `explain-execution-tree` against the returned root execution URI. `codex_explain_execution_tree_local` is the deterministic MCP-path smoke version of that same flow.

If you want the same deterministic flow without leaving the helper, run:

```bash
guild codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution-tree
```

`codex_explain_execution_tree_local` remains as the lower-level compatibility smoke command and now wraps that same shared helper path.

The required `read-resource` capability stays tightly scoped to local Guild execution URIs:

- `guild://executions/`

Optional evidence descriptor reads use the existing local object-record scope:

- `guild://objects/records/`

When enabled, those optional reads now consume `guild://objects/records/{evidence_record_id}/metadata` resources through the same backend, so the tree summary can report host-owned evidence metadata such as `blob_uri` and `produced_by_execution` without reading payload bytes.

No new host imports, policy engine, indexing layer, or MCP tool surface are added by this skill. It is a focused inspect/debug consumer of the current local Guild substrate.
