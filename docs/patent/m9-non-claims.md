# M9 What We Are Not Claiming

This memo exists to prevent overclaiming. It is intentionally blunt.

## Not Claimed

- Related claim ids: `c6-explicit-fail-closed-unsupported-walls-and-reason-codes`, `c9-real-path-log-write-downstream-linkage`, `c10-not-claimable-yet-surfaces`
- No runtime-general proof claim across all families.
- No broad `emit-evidence` live-proof claim.
- No broad `invoke-skill` call-graph proof claim.
- No broad outbound HTTP proof claim beyond the six measured replay-backed slices.
- No runtime-general delegated-token enforcement claim.
- No general positive factual witness claim system.
- No runtime-general absence proof from the current witness coverage.
- No claim that admission alone proves the minimal authority set.
- No claim that proof-backed linkage exists for `log-write` on the checked real path today.
- No claim beyond the measured bounded slices and measured fail-closed walls in the checked artifacts.

## Specifically Excluded Family Surfaces

- `emit-evidence`
  The repo still marks live proof, plan -> proof -> token linkage, and proof -> witness linkage as `not_proven`.
- `invoke-skill`
  Multi-child fan-out, recursion, child authority use, broader resolution, and non-inspect child targets remain outside the measured envelope.
- `http-request`
  Redirects, `https`, query and fragment components, multiple requests, other hostname forms, and `localhost` default-port forms remain outside the measured envelope.
- `read-resource`
  Query-resource shrink remains outside the measured bounded proof slice.
- `log-write`
  The checked real path measures proof only. It does not currently claim proof-backed downstream token or witness linkage.

## Control-Plane Exclusions

- The draft-v1 token layer is not being claimed as runtime-general enforcement.
- The witness layer is not being claimed as a general factual attestation layer.
- The packet does not treat the draft control-plane harness as the primary runtime-contract source of truth.
- The packet does not turn machine-readable support or benchmark artifacts into broader guarantees by wording alone.

## Drafting Guardrail

If a sentence needs words like "all families," "runtime-general enforcement," or "already proven everywhere," it should not be in the M9 packet.
