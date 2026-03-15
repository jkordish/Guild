# hello-inspect

This example is intentionally small, but it now really runs through Guild's local inspect-only slice with a Wasmtime-backed Wasm component.

It is the primitive skill used both on its own and as the child dependency for `hello-composite`.

It proves:

- requested refs resolve before execution
- source manifests are installed into digest-pinned executable records
- installed manifests and staged artifact digests are validated locally
- explicit grants flow into `ExecutionContext`
- typed `emit-evidence` and `log-write` capability constraints are enforced by the host
- the Wasm guest emits evidence through the host boundary
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

Run it locally from the repository root:

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
cargo run -p guild-mcp --example signed_import_failures_local
```

That command:

1. builds the guest from `skill-rust/`
2. installs it into its own cleaned subdirectory under `target/dev-local-registry/`
3. resolves it by `RequestedSkillRef`
4. executes it through `guild.inspect`
5. reads back the stored execution and evidence resources

`export_import_local` uses the same installed artifact as the transport unit instead of the source tree:

1. installs `hello-inspect` into registry A
2. generates a local publisher identity
3. exports a signed portable bundle from the installed record
4. trusts that publisher in fresh registry B
5. imports the verified bundle into registry B
6. resolves `hello-inspect` by `RequestedSkillRef` in registry B
7. executes it through the normal Wasmtime-backed path without rebuilding

`signed_import_failures_local` proves the negative path:

1. an untrusted signed bundle is rejected before installation
2. a tampered signed bundle is rejected even after the publisher is trusted

The normal happy path grants:

- `emit-evidence` with a bounded byte limit plus allowed audience/redaction sets
- optional `log-write` with `info` level when the caller wants the guest to emit a log line

Manual rebuilds are no longer the normal workflow. If you want to inspect the raw guest build directly, you can still run:

```bash
rustup target add wasm32-wasip2
cargo build --manifest-path examples/skills/hello-inspect/skill-rust/Cargo.toml --target wasm32-wasip2 --release
```
