# First Honest Mutation Demo

This note chooses the first honest post-starter mutation target for Guild.
It is a docs-first planning artifact, not a runtime-contract source.

Normative runtime truth still lives in `SPECS.md`, `ARCHITECTURE.md`, WIT, and
the Rust runtime/types. The current runner still rejects `apply` mode with
`apply mode remains globally gated until audit and approval paths exist`, so
this document defines the narrow target and readiness bar without pretending
that a runnable apply path already ships.

## Current Boundary

Use these constraints as the starting point for any mutation-demo planning:

- manifests can already express `apply_requires_approval` and
  `apply_requires_idempotency_key`, but that is planning truth, not runnable
  operator surface yet
- durable execution receipts and evidence records exist today for inspect-mode
  and rejected executions, so the mutation target should reuse that host-owned
  receipt/evidence model rather than invent a second audit path
- the first mutation slice should prove why approvals, idempotency, retry
  discipline, and evidence matter before Guild broadens into k8s, chat, or
  secret-heavy action stories

## Candidate Comparison

| Candidate | Current fit | Why it is believable or blocked |
| --- | --- | --- |
| `cache purge with evidence trail` | preferred target | smallest single-surface mutation in the current planning set; operator-facing capability name is legible (`cache:purge`); before/after evidence can stay focused on purge intent, provider acknowledgement, and one verification check without pulling in broader workflow coordination |
| `rollback-and-annotate` | fallback | the review half already maps to today's explain/compare/evidence surfaces, but the action half still couples one risky rollback with a second write path for incident annotation and therefore needs broader coordination semantics than cache purge |
| `restart-and-notify` | deferred broader candidate | remains the long-term hero story, but it combines infrastructure mutation with a separate notification side effect, making partial-failure handling and audit scope materially broader than the first mutation slice should carry |

## Chosen Direction

Preferred target:

- `cache purge with evidence trail`

Fallback if the cache-specific path stalls:

- `rollback-and-annotate`

Why `cache purge with evidence trail` wins first:

- it narrows the mutable surface to one explicit operator intent instead of
  coupling infrastructure action and follow-up coordination
- it keeps blast radius review legible: scope, target objects, and verification
  checks are easier to name upfront than restart or rollback semantics across a
  wider service boundary
- it can prove the value of approval, idempotency, receipt, and evidence
  requirements without claiming broad k8s, deploy, chat, or secret support

Why `rollback-and-annotate` stays the fallback instead of the lead:

- it is still believable later because the review-side bridge already exists in
  the docs-first `rollback verification pack`
- it asks Guild to support both deployment mutation and incident annotation in
  the same action story, which is a larger coordination step than the first
  mutation slice should carry

## Minimum Readiness Bar Before Implementation

The first runnable mutation demo should not exist until all of the following
are true.

### Approval

- every purge request must carry an explicit approval or policy decision that
  names the target environment, cache scope, and operator reason
- the approval reference must become part of the durable receipt before any
  mutation attempt starts
- docs-only metadata or playbook prose must never imply an approval happened
  automatically

### Idempotency

- the request must require a caller-supplied idempotency key tied to the purge
  intent
- the same idempotency key plus the same target scope must converge on one
  durable outcome instead of emitting duplicate purge attempts
- retries must reuse the same idempotency key rather than minting a new action
  identity after an ambiguous failure

### Evidence

- pre-mutation evidence must capture the requested purge scope, the operator
  reason, and the verification check the run expects to perform afterward
- post-mutation evidence must capture the provider acknowledgement or refusal,
  any returned request identifier, and one verification result that shows the
  cache state actually changed or remained unchanged
- emitted evidence must stay host-owned and durable so later explanation can
  trace the action without trusting guest-only narration

### Retry Discipline

- only ambiguous transport or provider failures may be retried automatically,
  and only with the same idempotency key
- confirmed success, confirmed refusal, or policy denial must not auto-retry
- partial outcomes that leave cache state uncertain must require operator
  review before another mutation attempt is admitted

### Receipt And Audit

- the durable receipt must record the requested mutation intent, granted
  capability slice, approval reference, idempotency key, emitted evidence
  references, and final outcome
- rejected attempts must remain durable exactly like other Guild rejections so
  the audit chain does not disappear when policy says no
- no side channel should be needed to answer who approved the action, what was
  attempted, what evidence was emitted, and whether the provider response was
  definitive or ambiguous

## Why Broader Candidates Stay Deferred

Keep these boundaries explicit even after choosing the first target:

- `restart-and-notify` stays deferred because it needs both infrastructure
  mutation and follow-up notification semantics, which broadens partial-failure
  handling and evidence scope too early
- certificate renewal, node remediation, and secret rotation remain later-phase
  because they introduce larger blast radius, identity or secret material, or
  multi-system propagation checks that the first mutation slice should avoid
- future replay execution claims must remain subordinate to the same approval,
  idempotency, audit, and evidence bar rather than being used to shortcut it

## Done-When Restatement

Issue `#132` is done when the repo has one believable first mutation target,
one clear fallback, and one explicit readiness bar that keeps apply-mode work
honest until the host-owned approval and audit path is real.
