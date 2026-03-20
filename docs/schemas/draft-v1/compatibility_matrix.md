# Compatibility Matrix

Deterministic admission check for the bundled examples.

This matrix enforces component-model compatibility, explicit WIT-world publication, and required effect-class support in addition to the runtime guarantee thresholds.

Negative fail-closed probes for omitted and unsupported `wit_worlds` declarations are asserted by `compatibility_check.py` but omitted from this table because they mutate the base runtime examples.

| Skill contract | Runtime | Result | Notes |
|---|---|---|---|
| `local-log-analyzer.contract.json` | `wasmtime-strict.runtime.json` | PASS | all required guarantees satisfied |
| `local-log-analyzer.contract.json` | `node-wasi-basic.runtime.json` | FAIL | execution isolation too weak; filesystem isolation too weak; required child-process mode unsupported; required token passthrough mode unsupported; audience binding unsupported; call-chain binding unsupported; anti-replay unsupported; max-hops enforcement unsupported; required witness level unsupported; acceptable signature mode unsupported; trusted time source unsupported; redacted I/O hashes unsupported; authority-plan digest unsupported |
| `zero-authority.contract.json` | `wasmtime-strict.runtime.json` | PASS | all required guarantees satisfied |
| `zero-authority.contract.json` | `node-wasi-basic.runtime.json` | FAIL | execution isolation too weak; filesystem isolation too weak; required child-process mode unsupported; required token passthrough mode unsupported |
| `fetch-transform.contract.json` | `wasmtime-strict.runtime.json` | PASS | all required guarantees satisfied |
| `fetch-transform.contract.json` | `node-wasi-basic.runtime.json` | FAIL | unsupported effect classes: secret.read; execution isolation too weak; filesystem isolation too weak; network policy granularity too weak; required child-process mode unsupported; required token passthrough mode unsupported; required revocation mode unsupported; audience binding unsupported; call-chain binding unsupported; anti-replay unsupported; max-hops enforcement unsupported; required witness level unsupported; acceptable signature mode unsupported; trusted time source unsupported; redacted I/O hashes unsupported; authority-plan digest unsupported |
| `cluster-rollout.contract.json` | `wasmtime-strict.runtime.json` | PASS | all required guarantees satisfied |
| `cluster-rollout.contract.json` | `node-wasi-basic.runtime.json` | FAIL | unsupported effect classes: capability.delegate, component.invoke, secret.read; execution isolation too weak; filesystem isolation too weak; network policy granularity too weak; required child-process mode unsupported; required token passthrough mode unsupported; required revocation mode unsupported; audience binding unsupported; call-chain binding unsupported; anti-replay unsupported; max-hops enforcement unsupported; required witness level unsupported; acceptable tamper-evidence mode unsupported; acceptable signature mode unsupported; trusted time source unsupported; redacted I/O hashes unsupported; authority-plan digest unsupported |
