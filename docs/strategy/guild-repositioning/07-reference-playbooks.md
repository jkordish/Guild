# 07. Reference Playbooks

**Status:** Proposed
**Owner:** Product + platform
**Last updated:** 2026-03-28

These are the first playbooks Guild should ship because they prove the thesis with real operational work, bounded blast radius, and clear evidence requirements.

## Selection criteria

A first-party reference playbook should be:

- common in real operations work
- understandable by non-authors
- bounded enough to demo safely
- dependent on clear approvals and evidence
- reusable across teams and stacks

## Playbook 1: Diagnose service, restart workload, notify on-call

**Outcome:** restore a degraded service after collecting basic evidence.

- **Pack:** `incident-triage`
- **Capabilities:** `metrics:query`, `logs:query`, `k8s:read`, `k8s:restart`, `chat:post`
- **Approval:** required in production before restart
- **Evidence required:** service id, health signals, approval decision, restart action, post-restart verification, final notification
- **Why this matters:** this is the most legible end-to-end example of useful but bounded mutation

## Playbook 2: Roll back deployment, verify health, annotate incident

**Outcome:** reverse a bad release with a visible trust chain.

- **Pack:** `safe-change`
- **Capabilities:** `deploy:rollback`, `metrics:query`, `logs:query`, `incident:annotate`
- **Approval:** required in production
- **Evidence required:** target deployment, rollback reason, rollback action, health verification, incident annotation
- **Why this matters:** demonstrates change safety and evidence after mutation

## Playbook 3: Certificate renewal, endpoint validation, notify

**Outcome:** rotate expiring cert material and verify endpoint health.

- **Pack:** `secrets-and-edge`
- **Capabilities:** `secrets:rotate`, `dns:read`, `chat:post`
- **Approval:** required when touching production certs or routing
- **Evidence required:** cert target, rotation action, endpoint validation, notification
- **Why this matters:** shows high-trust work beyond pure Kubernetes workflows

## Playbook 4: Node remediation, cordon, drain, verify recovery

**Outcome:** isolate a bad node and verify workload recovery.

- **Pack:** `k8s-remediation`
- **Capabilities:** `k8s:read`, `k8s:cordon`, `k8s:drain`, `metrics:query`, `chat:post`
- **Approval:** required in production
- **Evidence required:** node identity, reason for remediation, cordon/drain actions, replacement scheduling evidence, final health check
- **Why this matters:** proves Guild can handle higher-blast-radius operational procedures safely

## Playbook 5: Cache purge with evidence trail

**Outcome:** invalidate stale edge content and prove what changed.

- **Pack:** `secrets-and-edge`
- **Capabilities:** `cache:purge`, `chat:post`
- **Approval:** policy dependent, usually required for production-wide purge
- **Evidence required:** purge target, scope, action issued, post-purge validation, notification
- **Why this matters:** simple mutation, easy to understand, good demo candidate

## Playbook 6: Secret rotation with approval gate and receipts

**Outcome:** rotate a secret, verify propagation, and preserve an audit trail.

- **Pack:** `secrets-and-edge`
- **Capabilities:** `secrets:rotate`, `k8s:read`, `chat:post`
- **Approval:** required in production
- **Evidence required:** secret target, approval decision, rotation action, propagation check, final status message
- **Why this matters:** directly ties Guild to security-sensitive operations

## What each reference playbook must ship with

Every first-party playbook should include:

- a human-readable summary
- declared capabilities
- explicit approval rules
- evidence contract
- at least one happy-path fixture
- at least one denial / unsafe-path fixture
- a sample receipt
- a short walkthrough for docs / demos

## Demo rule

Demo the playbook, not the plumbing.

A good demo shows:

1. the declared capabilities
2. the approval gate
3. the mutation step
4. the receipt and evidence summary

If a demo only shows the compiler or manifest format, the thesis is not being proven.
