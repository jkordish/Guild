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

## Draft Schema Bundle Validation

The draft admission bundle under `docs/schemas/draft-v1/` now has its own focused validation path:

```bash
python3 -m venv /tmp/guild-schema-venv
/tmp/guild-schema-venv/bin/pip install -r docs/schemas/draft-v1/requirements.txt
/tmp/guild-schema-venv/bin/python docs/schemas/draft-v1/validate_examples.py
/tmp/guild-schema-venv/bin/python docs/schemas/draft-v1/compatibility_check.py
```

`validate_examples.py` covers schema validation plus the checked-in `admit`, `downgrade`, `migrate`, and `refuse` execution-plan examples. `compatibility_check.py` remains the narrower hard-requirement precheck and regenerates `docs/schemas/draft-v1/compatibility_matrix.md`.

It now also covers the draft-bundle M5 examples:

- exact reduction
- cache-hit reuse
- comparator-unavailable fail-closed behavior
- exact no-reduction
- bounded-minimal scope shrinking
- zero-authority exact minimality
- strict cache bypass when runtime, comparator, or plan identity changes

It now also covers the draft-bundle M6 examples:

- proof-backed root issuance from an acceptable M5 proof
- explicit upper-bound issuance only when policy allows it
- explicit refusal by default when proof-backed issuance is unavailable and upper-bound issuance is not enabled
- one-hop delegated child issuance with narrower scope, audience, and TTL
- explicit empty-capability token issuance for zero-authority invocations
- fail-closed verification for replay, audience mismatch, holder mismatch, passthrough attempts, parent-child broadening, runtime mismatch, call-chain mismatch, and expiry

The current M6 protection mechanism in this draft harness is a shared-secret HMAC MAC over canonical JSON claims. It is not a public-key signature flow, and the replay/revocation checks are local verifier-state mechanisms only.

If you want one direct M4 admission run:

```bash
/tmp/guild-schema-venv/bin/python docs/schemas/draft-v1/admission_engine.py \
  --contract docs/schemas/draft-v1/examples/zero-authority.contract.json \
  --request docs/schemas/draft-v1/examples/zero-authority.migrate.request.json \
  --runtime docs/schemas/draft-v1/examples/node-wasi-basic.runtime.json \
  --runtime docs/schemas/draft-v1/examples/wasmtime-strict.runtime.json
```

If you want one direct M5 proof run over an already-admissible plan:

```bash
/tmp/guild-schema-venv/bin/python docs/schemas/draft-v1/minimization_engine.py \
  --plan docs/schemas/draft-v1/examples/local-log-analyzer.admit.plan.json \
  --contract docs/schemas/draft-v1/examples/local-log-analyzer.contract.json \
  --request docs/schemas/draft-v1/examples/local-log-analyzer.admit.request.json \
  --runtime docs/schemas/draft-v1/examples/wasmtime-strict.runtime.json \
  --invocation-input docs/schemas/draft-v1/examples/local-log-analyzer.invocation.json \
  --comparator-profile docs/schemas/draft-v1/examples/local-log-analyzer.canonical-json.comparator.json \
  --created-at 2026-03-20T12:10:00Z \
  --cache-dir /tmp/guild-m5-cache
```

If you want one direct M6 root-issuance run:

```bash
/tmp/guild-schema-venv/bin/python docs/schemas/draft-v1/token_engine.py issue-root \
  --plan docs/schemas/draft-v1/examples/local-log-analyzer.admit.plan.json \
  --contract docs/schemas/draft-v1/examples/local-log-analyzer.contract.json \
  --proof docs/schemas/draft-v1/examples/local-log-analyzer.proof.json \
  --holder-id urn:guild:service:local-log-analyzer \
  --issuer-id urn:guild:issuer:draft-control-plane:v1 \
  --key-id draft-hmac-2026-03 \
  --shared-secret guild-draft-shared-secret-2026-03 \
  --issuer-epoch 3 \
  --issued-at 2026-03-20T13:00:00Z \
  --token-id urn:guild:token:local-log-analyzer:root:v1
```

If you want one direct M6 verification run over the checked delegated-child example:

```bash
/tmp/guild-schema-venv/bin/python docs/schemas/draft-v1/token_engine.py verify \
  --token docs/schemas/draft-v1/examples/cluster-rollout.child-token.json \
  --issuer-id urn:guild:issuer:draft-control-plane:v1 \
  --key-id draft-hmac-2026-03 \
  --shared-secret guild-draft-shared-secret-2026-03 \
  --verification-time 2026-03-20T13:05:20Z \
  --holder-id urn:guild:service:kube-api-client \
  --runtime-guarantee-id urn:guild:runtime:wasmtime-strict:v1 \
  --plan docs/schemas/draft-v1/examples/cluster-rollout.admit.plan.json \
  --contract docs/schemas/draft-v1/examples/cluster-rollout.contract.json \
  --parent-token docs/schemas/draft-v1/examples/cluster-rollout.root-token.json \
  --audience cluster-prod \
  --resource-binding-json '{"effect_class":"net.connect","audience":"cluster-prod","resource":"https://kube-api.prod.example.internal/apis/apps/"}' \
  --chain-link urn:guild:actor:ops-user \
  --chain-link urn:guild:workflow:cluster-rollout \
  --chain-link urn:guild:token:cluster-rollout:root:v1 \
  --chain-link urn:guild:service:kube-api-client \
  --replay-state-dir /tmp/guild-m6-replay
```

If you want one explicit sign-and-verify pass for a generated M4 plan:

```bash
cargo run -q -p guild-mcp --bin guild -- trust generate \
  --publisher-id local.example \
  --display-name "Local Example" \
  --output /tmp/guild-plan-signer.json

cargo run -q -p guild-mcp --bin guild -- \
  --registry-root /tmp/guild-plan-registry trust add \
  --identity-file /tmp/guild-plan-signer.json

cargo run -q -p guild-mcp --bin guild -- trust sign-plan \
  --plan docs/schemas/draft-v1/examples/zero-authority.admit.plan.json \
  --identity-file /tmp/guild-plan-signer.json \
  --output /tmp/zero-authority.admit.signed.plan.json

cargo run -q -p guild-mcp --bin guild -- \
  --registry-root /tmp/guild-plan-registry trust verify-plan \
  --plan /tmp/zero-authority.admit.signed.plan.json
```

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
