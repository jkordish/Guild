# Testing And Proof Flows

This document holds the verification commands and proof workflows that do not belong in the top-level README.

The README is intentionally focused on Guild itself and on a few usage examples. This page is the deeper operator and contributor reference for regression sweeps, example proofs, and Codex helper flows.

## Full Verification

Run the repository-wide verification sweep with:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

If you want the focused CLI and stdio regression suites first:

```bash
cargo test -p guild-mcp --test guild_cli --test codex_workflow --test mcp_server_stdio
```

These proof flows intentionally keep using explicit temp or `target/dev-local-registry/...` roots so they never depend on or mutate a developer's real `~/.guild` or `~/.codex`.

## CLI Smoke Flows

Minimal local CLI smoke:

```bash
export GUILD_REGISTRY_ROOT=target/dev-local-registry/cli-local

cargo run -q -p guild-mcp --bin guild -- install examples/skills/hello-inspect

cargo run -q -p guild-mcp --bin guild -- list

cargo run -q -p guild-mcp --bin guild -- inspect \
  skill://example/hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}'

cargo run -q -p guild-mcp --bin guild -- list executions --limit 5

cargo run -q -p guild-mcp --bin guild -- read guild://executions/<execution-id>

cargo run -q -p guild-mcp --bin guild -- mcp serve --stdio
```

Trust and signed-bundle smoke:

```bash
cargo run -q -p guild-mcp --bin guild -- trust generate \
  --publisher-id local.example \
  --display-name "Local Example" \
  --output target/dev-local-registry/local.example.json

cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/a export bundle \
  skill://example/hello-inspect@^0.1 \
  --signer target/dev-local-registry/local.example.json \
  --output target/dev-local-registry/bundle

cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/b trust add \
  --identity-file target/dev-local-registry/local.example.json

cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/b import bundle \
  target/dev-local-registry/bundle
```

OCI registry smoke:

```bash
cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/a push \
  skill://example/hello-inspect@^0.1 \
  --reference 127.0.0.1:5000/guild-example-hello-inspect:0.1.0 \
  --signer target/dev-local-registry/local.example.json \
  --allow-http

cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/b pull \
  127.0.0.1:5000/guild-example-hello-inspect:0.1.0 \
  --allow-http
```

## Codex Workflow

Persistent operator setup:

```bash
guild init
guild init --global
guild init --project
```

`guild init` is the only persistent local setup workflow. It creates the selected Guild root, always prints the current stdio Codex wiring for the running `guild` binary, and `--global` / `--project` make the config writes explicit.

Deterministic repo-local bootstrap and Cargo-based config helper:

```bash
cargo run -p guild-mcp --bin guild -- codex bootstrap --registry-root target/dev-local-registry/codex-local --reset
cargo run -p guild-mcp --bin guild -- codex print-config --registry-root target/dev-local-registry/codex-local
```

Deterministic scenario prep:

```bash
cargo run -p guild-mcp --bin guild -- codex scenario --registry-root target/dev-local-registry/codex-local --scenario recent-failure-triage --json
cargo run -p guild-mcp --bin guild -- codex scenario --registry-root target/dev-local-registry/codex-local --scenario policy-denial-debug --json
cargo run -p guild-mcp --bin guild -- codex scenario --registry-root target/dev-local-registry/codex-local --scenario execution-tree --json
```

Deterministic smoke flows:

```bash
cargo run -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution
cargo run -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution-tree
cargo run -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow recent-failure-triage
cargo run -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow policy-denial-debug
```

## Example Proof Commands

Inspect and runtime path:

```bash
cargo run -p guild-mcp --example inspect_local
cargo run -p guild-mcp --example inspect_http_json_local
cargo run -p guild-mcp --example inspect_policy_local
cargo run -p guild-mcp --example filesystem_rejection_local
cargo run -p guild-mcp --example inspect_composite_local
```

Explain and query path:

```bash
cargo run -p guild-mcp --example explain_execution_local
cargo run -p guild-mcp --example explain_execution_tree_local
cargo run -p guild-mcp --example explain_failure_local
cargo run -p guild-mcp --example explain_recent_failures_local
cargo run -p guild-mcp --example codex_explain_execution_local
cargo run -p guild-mcp --example codex_explain_execution_tree_local
```

Installed portability and trust path:

```bash
cargo run -p guild-mcp --example export_import_local
cargo run -p guild-mcp --example export_import_oci_local
cargo run -p guild-mcp --example export_import_composite_local
cargo run -p guild-mcp --example export_import_composite_oci_local
cargo run -p guild-mcp --example signed_import_failures_local
cargo run -p guild-mcp --example signed_import_oci_failures_local
cargo run -p guild-mcp --example push_pull_oci_registry_local
cargo run -p guild-mcp --example push_pull_composite_oci_registry_local
cargo run -p guild-mcp --example signed_pull_oci_registry_failures_local
```

MCP stdio proof:

```bash
cargo run -p guild-mcp --example mcp_stdio_local
```

That example now proves one real paginated MCP interaction by walking `resources/templates/list` across two cursor-linked pages before exercising `guild.inspect` and `resources/read`.

## Where To Go Next

- For the public command and URI grammar, see [`command-language.md`](command-language.md).
- For example-specific behavior and expected output, see the README in each example skill directory under [`examples/skills/`](../examples/skills/).
- For the stable contract and architecture, see [`../SPECS.md`](../SPECS.md) and [`../ARCHITECTURE.md`](../ARCHITECTURE.md).
