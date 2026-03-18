# ADR 0014: Invoke-skill policy family

Status: Accepted  
Date: 2026-03-17

## Context

Guild now has real composite execution, real child lineage, and real nested
grant reduction.

Without an ADR, `invoke-skill` can easily get misremembered as "run some other
skill," which would blur the boundary between declared composition and arbitrary
execution.

## Decision

The `invoke-skill` family authorizes invocation of declared dependency aliases
only.

In the current repository:

- the manifest-declared family name is `invoke-skill`
- the guest ABI host import is named `invoke-dependency`
- installed manifests pin each dependency alias to a digest-pinned
  `ResolvedSkillRef`

This family is not arbitrary ref execution.

The invocation path is:

1. the parent guest asks to invoke a dependency alias
2. the host checks that the alias is declared in the installed manifest
3. the host checks that at least one `invoke-skill` grant covers that alias
4. the host checks child execution budget
5. the host loads the exact installed dependency pinned in the manifest
6. the host derives child requested capabilities by reducing parent grants
   against the child manifest requirements
7. the child request re-enters the same host authorization path with
   `parent_execution_id` set

The current typed policy dimension is:

- `aliases`

Projection into `guild-skill-inspect-v1` is full for the current
`invoke-skill` grant shape. Installed dependency pinning, child grant
reduction, child policy decisions, and durable child execution records remain
host-owned state outside the guest ABI.

Alias semantics are bounded:

- if `aliases` is omitted on a grant, the grant is unbounded only across the
  current manifest's declared aliases
- undeclared aliases still fail with `dependency-not-declared`
- runtime invocation checks the concrete alias against declared aliases and the
  granted alias set at call time
- nested grant reduction must still resolve back to declared aliases and is not
  a general selector language

The parent-child authority boundary is explicit:

- the parent does not hand the child ambient authority
- child capability input is derived from the parent grant set plus the child
  manifest requirements
- if a child requirement cannot be reduced from the parent grant, invocation is
  denied with `child-capability-mismatch`
- the child is then re-authorized by host policy, so policy may narrow again
  but never widen authority

The current repository also applies budget pressure to composite execution:

- `Budget.max_child_executions` is decremented on each child call
- zero remaining child budget fails closed with `child-budget-exhausted`
- child requests inherit the parent trace ID and mode, use the same tenant ID,
  and carry a host-derived request ID

Failure behavior remains host-owned and durable:

- alias authorization denials are host-owned
- child capability reduction failures are host-owned
- child policy rejections produce a real child `ExecutionRecord`
- parent failure to complete a child call surfaces as `child-invocation-failed`
  while preserving child lineage and any persisted child record

## Consequences

Positive:

- composite skills stay within declared, installed, digest-pinned closure
- least-authority composition is enforced instead of implied
- child execution remains auditable as a first-class execution record

Costs and limits:

- composite skills cannot dynamically name arbitrary requested refs
- recursion depth is intentionally bounded by child execution budget
- policy evaluation may reject child execution even after parent reduction

## Explicit invariants

- `invoke-skill` is not "run anything"
- invocation is alias-scoped and manifest-declared
- dependencies resolve from installed digest-pinned state
- child grants are reduced from the parent grant set
- nested authority never widens
- child executions create durable lineage rather than disappearing into parent
  logs or output

## Explicit non-goals / deferred work

- arbitrary runtime invocation by requested ref
- remote dependency discovery
- automatic authority inheritance beyond typed reduction
- broader recursion orchestration semantics than the current budget model
- apply-mode workflow design

## Cross-references

- `SPECS.md`
- `ARCHITECTURE.md`
- `docs/adr/0012-capability-policy-layering-model.md`
- `docs/adr/0006-execution-record-schema.md`
- `crates/guild-manifest/src/lib.rs`
- `crates/guild-runner/src/lib.rs`
- `crates/guild-runner/tests/composition.rs`
- `crates/guild-runner/tests/http_requests.rs`
- `examples/skills/hello-composite/README.md`
