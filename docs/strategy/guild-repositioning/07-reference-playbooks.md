# Reference Playbooks

## Why These Matter

Guild needs concrete operational stories that make admission, isolation, capabilities, receipts, evidence, and replay feel useful to an operator.

The current repo already has incident-analysis examples. The next wave should add playbooks that show action plus trust, not just post-run explanation.

For the bounded playbook surface and current repo boundary, see
[`04-playbook-surface-v1.md`](04-playbook-surface-v1.md).
For a repo-grounded bridge from today's examples into that framing, see
[`08-manifest-to-playbook-translation-note.md`](08-manifest-to-playbook-translation-note.md).

## Approved Set

This is the approved reference playbook set for follow-on docs and example
work in this repo.

Use it to decide which operator workflows Guild should lead with, which example
surfaces should point at those workflows, and which future playbook-oriented
stories still need to stay docs-first.

This document is not a claim that Guild already ships a first-class playbook
runtime or that every capability below is runnable today. The current repo
still runs skills directly and is strongest today at read-only review,
explanation, receipts, and evidence over stored Guild refs.

## Coverage At A Glance

| Playbook | Primary operator outcome | Coverage themes | Current-reality status |
| --- | --- | --- | --- |
| Diagnose service -> restart pods -> notify on-call | Remediate a service incident with verification and communication | remediation, notification, evidence | docs-first target with a strong bridge from today's explain/report examples |
| Rollback deployment -> verify health -> annotate incident | Reverse a risky change and keep the incident trail legible | rollback, validation, evidence | docs-first target built on today's receipt and explanation strength |
| Cert renewal -> validate -> notify | Rotate expiring trust material and confirm recovery | validation, notification, evidence | docs-first target; no direct runtime support today |
| Noisy node remediation -> cordon / drain -> verify recovery | Stabilize cluster health through bounded infrastructure action | remediation, validation, evidence | docs-first target; better once playbook step semantics and action surfaces are broader |
| Cache purge with evidence trail | Perform one narrow operational action with obvious auditability | evidence-heavy workflow, remediation | docs-first target and the best near-term narrow trust demo |
| Secret rotation with policy gate | Show admission and approval pressure on sensitive change | notification, evidence, policy-heavy workflow | docs-first target after admission and approval story stays stable |

## 1. Diagnose Service -> Restart Pods -> Notify On-Call

- Operator problem:
  - A service is failing and the operator needs one trusted recovery path that
    covers diagnosis, one bounded action, and an explicit notification step.
- Required capabilities:
  - `metrics:query`
  - `logs:query`
  - `k8s:restart`
  - `chat:post`
- Strategic value:
  - This is the simplest credible trusted operational automation story in the
    set because it combines review, action, validation, and communication.
- Current repo anchor:
  - Builds naturally on the current explain/report examples, Guild Ops
    Starter, and the existing receipt and evidence model.
- Current-reality status:
  - Docs-first target. Today's repo can honestly anchor the diagnose and
    explain side of this flow, but not the restart and notify action surfaces.
- Suggested sequencing:
  - First hero playbook once one action-heavy example is ready to stay inside
    the current trust frontier.

## 2. Rollback Deployment -> Verify Health -> Annotate Incident

- Operator problem:
  - A deployment caused damage and the operator needs a reversible recovery
    flow with explicit verification and incident context.
- Required capabilities:
  - `deploy:rollback`
  - `metrics:query`
  - `incident:create`
- Strategic value:
  - Shows a high-stakes operational change where admission, post-change
    validation, and receipts matter more than generic automation polish.
- Current repo anchor:
  - Extends today's receipt, explanation, and comparison strength into an
    action-oriented workflow without changing the trust chain underneath.
- Current-reality status:
  - Docs-first target. The repo can already explain and compare runs honestly,
    but rollback and incident actions remain future action surfaces.
- Suggested sequencing:
  - Second, after the restart/notify story is stable enough to keep the
    action-heavy narrative honest.

## 3. Cert Renewal -> Validate -> Notify

- Operator problem:
  - A certificate is expiring and the operator needs a bounded renewal flow
    with visible validation and downstream communication.
- Required capabilities:
  - `secrets:rotate`
  - `metrics:query`
  - `chat:post`
- Strategic value:
  - Shows safe credential hygiene plus post-action validation without drifting
    into a generic security automation story.
- Current repo anchor:
  - Reinforces explicit capability review, evidence collection, and the trust
    narrative already present in the repo.
- Current-reality status:
  - Docs-first target. The repo has the trust-language and evidence spine, but
    not direct renewal or notification runtime support.
- Suggested sequencing:
  - Fourth, after one or two stronger operational recovery stories are fixed.

## 4. Noisy Node Remediation -> Cordon / Drain -> Verify Recovery

- Operator problem:
  - One unhealthy node is harming a cluster and the operator needs a bounded
    remediation path with obvious validation checkpoints.
- Required capabilities:
  - `k8s:cordon`
  - `k8s:drain`
  - `metrics:query`
  - `logs:query`
- Strategic value:
  - Strong SRE story with high operator legibility and clear separation
    between review, action, and post-action verification.
- Current repo anchor:
  - A good fit once playbook step semantics and action-oriented example
    surfaces are clearer.
- Current-reality status:
  - Docs-first target. The current repo can support the review and evidence
    posture around this flow, but not the infrastructure actions themselves.
- Suggested sequencing:
  - Fifth, after the smaller operational recovery and trust-demo stories are in
    place.

## 5. Cache Purge With Evidence Trail

- Operator problem:
  - A stale cache is causing customer-visible issues and the operator needs one
    narrow corrective action with a clean evidence trail afterward.
- Required capabilities:
  - `cache:purge`
  - `metrics:query`
- Strategic value:
  - Shows a smaller-scope, audit-friendly action whose value comes from
    explicit receipts and evidence rather than broad workflow complexity.
- Current repo anchor:
  - The best near-term trust-demo fit because the repo already explains stored
    executions and evidence well, even if the action layer is still future work.
- Current-reality status:
  - Docs-first target with the strongest narrow bridge from current repo truth.
- Suggested sequencing:
  - Third, ahead of broader infrastructure or secret-management stories if one
    lower-complexity trust demo is needed first.

## 6. Secret Rotation With Policy Gate

- Operator problem:
  - A sensitive secret needs rotation and the operator needs a path that makes
    capability review, policy gatekeeping, and follow-up communication explicit.
- Required capabilities:
  - `secrets:rotate`
  - `incident:create`
  - `chat:post`
- Strategic value:
  - Strong security-engineering story centered on admission, approval posture,
    and evidence-backed operational review.
- Current repo anchor:
  - Pairs naturally with the trust and evidence narrative already present in
    the repo.
- Current-reality status:
  - Docs-first target. Keep it bounded until the admission and approval story
    is stable enough not to overclaim support.
- Suggested sequencing:
  - Sixth, after the capability taxonomy and playbook-oriented admission story
    are stable.

## Suggested Sequence

1. diagnose service -> restart pods -> notify on-call
2. rollback deployment -> verify health -> annotate incident
3. cache purge with evidence trail
4. cert renewal -> validate -> notify
5. noisy node remediation -> cordon / drain -> verify recovery
6. secret rotation with policy gate

## Recommendation

Start with one operational recovery playbook and one narrowly scoped audit playbook. That gives Guild one action-heavy story and one trust-heavy story without waiting for a full playbook portfolio.

For the current repo, that means:

- keep the reference set explicit and operator-facing in docs
- use today's read-only starter examples as the honest bridge into that story
- pick one future hero example only when it fits the current trust and
  capability frontier without widening runtime claims

The approved first hero example is
`diagnose service -> restart pods -> notify on-call`; use
[`09-hero-reference-example-plan.md`](09-hero-reference-example-plan.md) for
the current bounded rollout plan and proof commands.
