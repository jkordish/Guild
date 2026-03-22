# M9 Invention Brief

This packet is grounded in the checked repo artifacts [`benchmark_matrix.json`](../schemas/draft-v1/benchmark_matrix.json), [`m8-real-path-benchmark.md`](../benchmarking/m8-real-path-benchmark.md), [`family_support_matrix.json`](../schemas/draft-v1/family_support_matrix.json), and [`SPECS.md`](../../SPECS.md). It is technical source material for counsel, not a legal opinion.

## Measured Invention Statement

Guild's measured technical delta is a fail-closed method for handling a portable component invocation in four explicit stages:

1. compute a host-owned upper-bound admission plan for the requested invocation
2. attempt bounded live proof only for family-specific slices with an honest deterministic replay or comparator basis
3. carry that proof into downstream token and witness linkage only where the proof exists
4. refuse or fall back explicitly to upper-bound-only and unlinked outputs outside the proven envelope

The center of gravity is not generic capability gating by itself. The measured delta is the combination of upper-bound admission, bounded counterfactual live proof, proof-backed downstream linkage when available, and explicit fail-closed behavior when it is not.

## 30-Second Novelty Line

Guild does not claim runtime-general authority minimization. The measured novelty is a fail-closed chain from upper-bound admission to bounded live proof to proof-backed downstream control-plane linkage, with explicit refusal or upper-bound fallback when the repo cannot honestly prove the narrower result.

## Claim Pillars

1. Fail-closed upper-bound admission is treated as a distinct technical stage, not smuggled in as already-minimized authority.
2. Bounded live proof is family-specific and comparator-specific; only measured replay-backed slices are carried forward as proof-backed.
3. Downstream token issuance and witness linkage preserve whether the basis was proof-backed, proof-only, upper-bound fallback, or unlinked.
4. Unsupported or not-proven shapes are first-class outputs with reason codes, refusal paths, and fail-closed walls rather than hidden noise.
5. The measured support frontier is machine-readable, slice-aware, and tied to checked tests, scenarios, and benchmark artifacts.

## Measured Frontier Now

| Family or slice | Measured now | Explicitly not claimed |
| --- | --- | --- |
| `read-resource` | One bounded immutable-root slice with checked plan -> proof -> token -> witness linkage. | Query-resource shrink or broader resource shapes. |
| `http-request` | Six bounded deterministic replay-backed `http` slices: loopback IP `GET` and `HEAD` with explicit and default ports, plus explicit-port `localhost` `GET` and `HEAD` with deterministic loopback-only resolution binding. | Redirects, `https`, other hostnames, query or fragment components, multiple requests, and `localhost` default-port forms. |
| `invoke-skill` | One bounded exact single-child zero-authority inspect slice with checked plan -> proof -> token -> witness linkage. | Multi-child fan-out, recursion, child authority use, broader resolution, and non-inspect child targets. |
| `log-write` | One exact observed `info`-level proof-only slice on the real path. | A checked real-path M6 or M7 linkage claim for `log-write`. |
| `emit-evidence` | Canonical family vocabulary exists in admission, tokens, and witnesses. | Any live proof-backed `emit-evidence` linkage. The measured repo still marks this family `not_proven` for live proof. |

## Non-Goals And Non-Claims

- No runtime-general proof claim across all families.
- No broad `emit-evidence` live-proof claim.
- No broad `invoke-skill` call-graph proof claim.
- No broad outbound HTTP proof claim beyond the six measured slices.
- No runtime-general delegated-token enforcement claim.
- No general positive factual witness claim system.
- No attempt to turn the packet into a filing form, patentability conclusion, or pitch deck.

## Companion Packet Documents

- [Measured technical delta memo](./m9-technical-delta.md)
- [Technical prior-art and delta matrix](./m9-prior-art-kill-matrix.md)
- [Technical claim ladder](./m9-claim-ladder.md)
- [Claim-to-evidence map](./m9-evidence-map.md)
- [Figure source set](./m9-figures.md)
- [Non-claims memo](./m9-non-claims.md)
- [Packet manifest](./m9-packet-manifest.json)
- [M10 next-step memo](./m9-m10-next.md)
