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

For M8c, that validation path now also checks the live-runtime alignment layer explicitly:

- bundled contracts and runtimes now align on the real inspect world `guild-skill-inspect-v1`
- the draft runtime examples publish both `supported_effect_classes` for draft compatibility and `supported_canonical_families` for live-runtime truth
- live Rust `authority_observations` fixtures are normalized into draft-v1 witnesses without silently widening vocabulary
- direct canonical `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write` observations now stay direct through plan, token, and witness handling
- a real bounded live `read-resource` proof is generated through the Rust runtime and then consumed by the draft token and witness layers
- a real bounded `http-request` live proof is generated for six deterministic replay-fixtured slices over `http`: loopback IP `GET` and `HEAD`, each with an explicit-port form and an implicit-default-port form, plus explicit-port `localhost` `GET` and `HEAD` with deterministic loopback-only resolution bindings, and unsupported redirect or no-replay cases stay fail-closed
- a real live `log-write` family proof is checked over the observed discrete log-level slice without pretending that `emit-evidence` became proven at the same time
- legacy `net.connect` and `component.invoke` compatibility paths are still checked explicitly as deprecated narrowing-only aliases rather than being treated as canonical support

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

It now also covers the draft-bundle M7 witness examples:

- proof-backed within-envelope witnessing over actual exercised authority
- explicit out-of-envelope witnessing when observed authority escapes the admissible or tokenized envelope
- coverage-limited witnessing where negative claims fail closed
- redacted witness claim success and redaction-blocked claim failure
- blocked-attempt tracking distinct from exercised authority
- delegation-chain witnessing over checked child-token linkage
- zero-authority witnessing with explicit negative-claim semantics
- runtime-binding mismatch verification failure
- vocabulary-mapping limitation handling that stays coverage-limited rather than pretending the unmapped effect did not happen
- live-runtime alignment cases for bounded live `read-resource` proof-backed linkage, bounded live `http-request` proof-backed linkage over the replay-fixtured loopback IP `GET` and `HEAD` slices with either an explicit port or the implicit default HTTP port plus the explicit-port `localhost` `GET` and `HEAD` slices with deterministic loopback-only resolution bindings, bounded live `invoke-skill` proof-backed linkage for the exact single-child zero-authority inspect slice, unsupported multi-child `invoke-skill` fail-closed behavior, unsupported redirect and no-replay `http-request` fail-closed behavior, and exact live `log-write` family proof support
- explicit alias deprecation or rejection checks for `net.connect`, `component.invoke`, and `net.resolve`
- deterministic normalization of identical runtime-native inputs
- fail-closed live proof prerequisite behavior

The current M7 protection mechanism in this draft harness is also a shared-secret HMAC MAC over canonical JSON claims. It is not public-key attestation, and the bounded harness and fixture paths do not imply runtime-general witness completeness.

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

If you touched the live Rust proof path, run the focused integration suite explicitly:

```bash
cargo test -p guild-runner --test live_proofs -- --nocapture
```

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

`docs/schemas/draft-v1/validate_examples.py` is now the end-to-end proof path for the checked M7 witness bundle. It regenerates the stored witness examples, re-verifies them, re-checks fixed claims, and confirms that negative claims fail closed under partial coverage or redaction.

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
