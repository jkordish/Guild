# hello-inspect

This example is intentionally small, but it now really runs through Guild's local inspect-only slice with a Wasmtime-backed Wasm component.

It is the primitive skill used both on its own and as the child dependency for `hello-composite`.
It is not the M8c proof-backed `invoke-skill` child fixture; that narrower slice
uses `invoke-child-zero` because `hello-inspect` exercises `emit-evidence`.

It proves:

- requested refs resolve before execution
- source manifests are installed into digest-pinned executable records
- source installs stage and validate before an atomic move into installed state
- installed manifests and staged artifact digests are validated locally
- explicit grants flow into `ExecutionContext`
- durable execution IDs are host-minted; caller request IDs are correlation-only
- typed `emit-evidence` and `log-write` capability constraints are enforced by the host
- the Wasm guest emits evidence through the host boundary
- identical payload blobs may dedupe by digest while each evidence emission still gets its own host-issued `EvidenceRef`
- a skill returns `SkillOutput`
- the runner wraps that into `ExecutionRecord` and persists it locally
- `guild.inspect` uses the same path

Implementation notes:

- source runtime kind: `wasm-component`
- mode support: `inspect` only
- source manifest: `manifest.json`
- guest source: `skill-rust/`

This directory is the source fixture for the example skill:

- `manifest.json` is the human-authored source manifest
- `input.schema.json` and `output.schema.json` pin the structured I/O shape
- `tests/` contains the inspect fixture pair used by repo tests

User journey: install and run a skill locally.

Start with the primary daily CLI path from the repository root:

```bash
guild --registry-root target/dev-local-registry/hello-inspect install examples/skills/hello-inspect
guild --registry-root target/dev-local-registry/hello-inspect show skill://example/hello-inspect@^0.1
guild --registry-root target/dev-local-registry/hello-inspect run skill://example/hello-inspect@^0.1 --input-json '{"name":"Ada"}' --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}' --json
guild --registry-root target/dev-local-registry/hello-inspect why exec:<execution-id-prefix>
guild --registry-root target/dev-local-registry/hello-inspect verify skill://example/hello-inspect@^0.1
```

Replace `<execution-id-prefix>` with the short execution id prefix from the run receipt.

Deep developer proof helpers:

```bash
cargo run -p guild-mcp --example inspect_local
```

Run the composite example that depends on it:

```bash
cargo run -p guild-mcp --example inspect_composite_local
```

Run the signed portable bundle proof for the primitive skill:

```bash
cargo run -p guild-mcp --example export_import_local
cargo run -p guild-mcp --example export_import_oci_local
cargo run -p guild-mcp --example signed_import_failures_local
cargo run -p guild-mcp --example signed_import_oci_failures_local
cargo run -p guild-mcp --example push_pull_oci_registry_local
cargo run -p guild-mcp --example signed_pull_oci_registry_failures_local
```

That command:

1. builds the guest from `skill-rust/`
2. installs it into its own cleaned subdirectory under `target/dev-local-registry/`
3. resolves the public `skill://example/hello-inspect@^0.1` ref to installed executable state
4. executes it through `guild run`
5. reads back the stored execution and evidence resources

The stored execution URI is host-issued. Any caller-supplied request ID is preserved only as correlation metadata inside the durable record.

`export_import_local` uses the same installed artifact as the transport unit instead of the source tree:

1. installs `hello-inspect` into registry A
2. generates a local publisher identity
3. exports a signed portable bundle from the installed record
4. trusts that publisher in fresh registry B
5. imports the verified bundle into registry B
6. resolves `skill://example/hello-inspect@^0.1` in registry B
7. executes it through the normal Wasmtime-backed path without rebuilding

`export_import_oci_local` proves the same portability contract through a local OCI image layout instead of the native bundle directory:

1. installs `hello-inspect` into registry A
2. generates a local publisher identity
3. exports the installed signed bundle payload as an OCI image layout
4. trusts that publisher in fresh registry B
5. imports the verified OCI layout into registry B
6. resolves `skill://example/hello-inspect@^0.1` in registry B
7. executes it through the normal Wasmtime-backed path without rebuilding

`signed_import_failures_local` proves the negative path:

1. an untrusted signed bundle is rejected before installation
2. a tampered signed bundle is rejected even after the publisher is trusted

`signed_import_oci_failures_local` proves the same local fail-closed behavior for OCI image layout import:

1. an untrusted OCI-carried signed bundle is rejected before installation
2. a tampered OCI blob is rejected even after the publisher is trusted

`push_pull_oci_registry_local` proves the same installed-state portability contract through a real local OCI registry:

1. installs `hello-inspect` into registry A
2. generates a local publisher identity
3. publishes the OCI-mapped signed installed bundle to a local OCI registry
4. trusts that publisher in fresh registry B
5. pulls the artifact from the registry and re-runs the same local trust/signature verification before installation
6. resolves `skill://example/hello-inspect@^0.1` in registry B
7. executes it through the normal Wasmtime-backed path without rebuilding

`signed_pull_oci_registry_failures_local` proves the same fail-closed behavior for OCI registry transport:

1. an untrusted pulled signed bundle is rejected before installation
2. a tampered pulled OCI blob is rejected even after the publisher is trusted

The normal happy path grants:

- `emit-evidence` with a bounded byte limit plus allowed audience/redaction sets
- optional `log-write` with `info` level when the caller wants the guest to emit a log line

The active inspect runtime slice is intentionally limited to `emit-evidence`, `log-write`, `read-resource`, and `invoke-skill`. Other typed capability families may exist elsewhere in the contracts, but this slice rejects them before execution.

Manual rebuilds are no longer the normal workflow. If you want to inspect the raw guest build directly, you can still run:

```bash
rustup target add wasm32-wasip2
cargo build --manifest-path examples/skills/hello-inspect/skill-rust/Cargo.toml --target wasm32-wasip2 --release
```
