# Testing And Proof Flows

This document holds the verification commands and proof workflows that do not belong in the top-level README.

The README is intentionally focused on Guild itself and on a few usage examples. This page is the deeper operator and contributor reference for regression sweeps, example proofs, and Codex helper flows.

The source-of-truth declaration lives in `SPECS.md` section "Source Of Truth".
For the frozen runtime-contract surfaces in this milestone, use `SPECS.md` section "Contract Surface v1 (core)" rather than treating this testing guide as a parallel source.

## Full Verification

Run the repository-wide verification sweep with:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

`make verify` runs that same repo-wide check plus the Rust-native draft-v1 truth gate.

If the format check reports drift and you want to rewrite files locally instead of only checking them, run `cargo fmt --all`.

If you want the focused CLI and stdio regression suites first:

```bash
cargo test -p guild-mcp --test guild_cli --test codex_workflow --test mcp_server_stdio
```

These proof flows intentionally keep using explicit temp or `target/dev-local-registry/...` roots so they never depend on or mutate a developer's real `~/.guild` or `~/.codex`.

## Draft Schema Bundle Validation

The migrated draft-v1 truth path under `docs/schemas/draft-v1/` is now Rust-native and repo-native:

```bash
cargo run -q -p xtask -- draft-v1 truth check
```

That standard truth command now covers:

- schema validation for the checked contract, runtime, request, plan, proof, token, and witness examples in `docs/schemas/draft-v1/examples/`
- negative schema probes for omitted and invalid runtime guarantee fields
- exact `family_support_matrix.json` regeneration or drift detection
- exact `compatibility_matrix.md` regeneration or drift detection plus the fail-closed `wit_worlds` probes
- benchmark artifact schema and report validation plus live scenario alignment against the real Rust runner

The checked truth-output commands are:

```bash
cargo run -q -p xtask -- draft-v1 support-matrix check
cargo run -q -p xtask -- draft-v1 compatibility check
cargo run -q -p xtask -- draft-v1 benchmark check
```

The artifact-regeneration commands are:

```bash
cargo run -q -p xtask -- draft-v1 support-matrix write
cargo run -q -p xtask -- draft-v1 compatibility write
cargo run -q -p xtask -- draft-v1 benchmark write
```

No Python virtualenv is required for those migrated truth flows anymore. The remaining direct Python engines later in this document are legacy draft-harness utilities for manual inspection and development work, not the standard repo truth path.
The checked JSON and Markdown artifacts remain outputs of that Rust-native path; they do not become runtime-contract sources just because they are checked into the repo.

The Rust-native truth gate now checks the current repo-backed draft-v1 truth surface conservatively:

- bundled schemas and checked examples still validate structurally
- bundled runtimes and contracts still line up on the active inspect world `guild-skill-inspect-v1`
- `family_support_matrix.json` stays aligned with the canonical live-family vocabulary and the current bounded draft-v1 layer statuses
- `compatibility_matrix.md` stays aligned with the fail-closed hard-requirement precheck logic, including the omitted and unsupported `wit_worlds` probes
- `benchmark_matrix.json` and `m8-real-path-benchmark.md` stay aligned with the real Rust live-proof scenarios, including supported slices, unsupported fallback slices, and explicit fail-closed walls

The older draft-harness M5, M6, and M7 Python engines still exist for manual example work, but they are no longer the standard repo truth gate. Their shared-secret HMAC MAC model and bounded fixture behavior remain draft-local rather than runtime-general.

Current live-runtime claim status after M8c is explicit and per family:

- M5 now has bounded live proof for `read-resource` over immutable Guild execution/object-record roots only
- M5 now has real live family proof for `log-write` over the observed discrete level slice
- M5 now has bounded live proof for `http-request` only over six deterministic replay-fixtured slices over `http`: loopback IP `GET` and `HEAD`, each with an explicit-port form and an implicit-default-port form, plus explicit-port `localhost` `GET` and `HEAD` with deterministic loopback-only resolution bindings, all with no query and no redirects
- M5 now has one bounded live-proof-backed `invoke-skill` slice only for exactly one declared alias resolved through the installed dependency snapshot to one exact zero-authority child on `guild-skill-inspect-v1`, with deterministic child input, the child-aware normalized inspect comparator, and zero nested child executions
- M5 still remains `not_proven` for `localhost` default-port `GET`, `localhost` default-port `HEAD`, other hostname forms, query or fragment components, redirects, multiple exercised `http-request` calls, `https`, broader `invoke-skill` shapes, and all current `emit-evidence` flows. The runtime now binds a fixed local-object-store sink descriptor and uses a dedicated single-sink comparator profile, but the tested exact single-emission shrink still fails closed on replay, so the family stays fail-closed. The current canonical `emit-evidence` authority model also remains too coarse for proof-backed linkage.
- M6 now issues and verifies direct canonical scopes for `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`, but it remains a draft-local token layer
- M8c now proves honest live end-to-end chains for `read-resource`, for the narrow bounded `http-request` replay slices, and for the exact bounded single-child `invoke-skill` slice: plan -> proof -> token -> witness
- M7 witness linkage now stays proof-linked only where the supplied proof is a real live-runtime proof and otherwise remains explicitly unlinked
- per-family, per-layer machine-readable status now lives in `docs/schemas/draft-v1/family_support_matrix.json`

The M8-proper real-path benchmark is still separate from the schema examples and can be rerun locally with:

```bash
cargo run -q -p xtask -- draft-v1 benchmark write
cargo run -q -p xtask -- draft-v1 benchmark check
```

That benchmark writes:

- `docs/schemas/draft-v1/benchmark_matrix.json`
- `docs/benchmarking/m8-real-path-benchmark.md`

Current measured benchmark truth on the checked path is:

- supported proof-linked slices: `read-resource` immutable roots, six bounded `http-request` replay slices, and one bounded `invoke-skill` single-child zero-authority slice
- supported proof-only slice: `log-write` observed `info` level through M4 plus M5 only
- benchmarked unsupported slices: redirect `http-request`, multi-child `invoke-skill`, and replay-unavailable `emit-evidence`, each with `10/10` default refusal, `10/10` explicit upper-bound fallback issuance, and `10/10` unlinked witness generation
- benchmarked fail-closed walls: `http-request` no-replay, `read-resource` execution-query shrink, and `invoke-skill` child-authority use, each triggered `10/10` in the checked scenarios
- measured overhead distributions now live in the checked `benchmark_matrix.json` and paired Markdown report; the Rust-native benchmark generator owns those values directly
- negative-claim checks remain coverage-limited in the checked scenarios: every measured non-`log-write` slice recorded `0` success, `0` fail, and `3` coverage-limited outcomes

If you touched the live Rust proof path, run the focused integration suite explicitly:

```bash
cargo test -p guild-runner --test live_proofs -- --nocapture
```

If you want to use the remaining legacy draft-harness engines directly, make sure their local Python dependencies are installed first. They are no longer part of the standard repo truth path.

If you want one direct M4 admission run through the remaining draft-harness engine:

```bash
python3 docs/schemas/draft-v1/admission_engine.py \
  --contract docs/schemas/draft-v1/examples/zero-authority.contract.json \
  --request docs/schemas/draft-v1/examples/zero-authority.migrate.request.json \
  --runtime docs/schemas/draft-v1/examples/node-wasi-basic.runtime.json \
  --runtime docs/schemas/draft-v1/examples/wasmtime-strict.runtime.json
```

If you want one direct M5 proof run over an already-admissible plan through the remaining draft-harness engine:

```bash
python3 docs/schemas/draft-v1/minimization_engine.py \
  --plan docs/schemas/draft-v1/examples/local-log-analyzer.admit.plan.json \
  --contract docs/schemas/draft-v1/examples/local-log-analyzer.contract.json \
  --request docs/schemas/draft-v1/examples/local-log-analyzer.admit.request.json \
  --runtime docs/schemas/draft-v1/examples/wasmtime-strict.runtime.json \
  --invocation-input docs/schemas/draft-v1/examples/local-log-analyzer.invocation.json \
  --comparator-profile docs/schemas/draft-v1/examples/local-log-analyzer.canonical-json.comparator.json \
  --created-at 2026-03-20T12:10:00Z \
  --cache-dir /tmp/guild-m5-cache
```

If you want one direct M6 root-issuance run through the remaining draft-harness engine:

```bash
python3 docs/schemas/draft-v1/token_engine.py issue-root \
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

If you want one direct M6 verification run over the checked delegated-child example through the remaining draft-harness engine:

```bash
python3 docs/schemas/draft-v1/token_engine.py verify \
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

The standard repo truth path is now `cargo run -q -p xtask -- draft-v1 truth check`. The remaining direct Python engines below are legacy draft-harness tools and are no longer required for support-matrix, compatibility, or benchmark validation.

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

Version note: the current workspace Cargo packages are `0.1.1`, but the checked-in example Guild skill manifests and OCI transport examples intentionally still use manifest version `0.1.0` / `@^0.1`. Those smoke commands follow Guild manifest identity, not Cargo package version.

Minimal local CLI smoke:

```bash
export GUILD_REGISTRY_ROOT=target/dev-local-registry/cli-local

cargo run -q -p guild-mcp --bin guild -- install examples/skills/hello-inspect

cargo run -q -p guild-mcp --bin guild -- show skill://example/hello-inspect@^0.1

cargo run -q -p guild-mcp --bin guild -- run \
  skill://example/hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}'

cargo run -q -p guild-mcp --bin guild -- ls runs --limit 5

cargo run -q -p guild-mcp --bin guild -- why exec:<execution-id-prefix>

cargo run -q -p guild-mcp --bin guild -- get guild://executions/<execution-id>

cargo run -q -p guild-mcp --bin guild -- verify skill://example/hello-inspect@^0.1

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
