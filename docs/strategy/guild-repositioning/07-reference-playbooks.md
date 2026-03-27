# Reference Playbooks

## Why These Matter

Guild needs concrete operational stories that make admission, isolation, capabilities, receipts, evidence, and replay feel useful to an operator.

The current repo already has incident-analysis examples. The next wave should add playbooks that show action plus trust, not just post-run explanation.

## 1. Diagnose Service -> Restart Pods -> Notify On-Call

- Required capabilities:
  - `metrics:query`
  - `logs:query`
  - `k8s:restart`
  - `chat:post`
- Strategic value:
  - This is the simplest credible "safe operational automation" story.
- Current repo anchor:
  - Builds naturally on current explain/report examples and the existing evidence model.
- Suggested sequencing:
  - First playbook example.

## 2. Rollback Deployment -> Verify Health -> Annotate Incident

- Required capabilities:
  - `deploy:rollback`
  - `metrics:query`
  - `incident:create`
- Strategic value:
  - Shows a high-stakes change with admission, verification, and evidence trail.
- Current repo anchor:
  - Extends current receipt/explanation strength into an action-oriented workflow.
- Suggested sequencing:
  - Second, after restart/notify.

## 3. Cert Renewal -> Validate -> Notify

- Required capabilities:
  - `secrets:rotate`
  - `metrics:query`
  - `chat:post`
- Strategic value:
  - Shows safe credential hygiene plus post-action validation.
- Current repo anchor:
  - Reinforces explicit capability review and evidence collection.
- Suggested sequencing:
  - Third, after rollback.

## 4. Noisy Node Remediation -> Cordon / Drain -> Verify Recovery

- Required capabilities:
  - `k8s:cordon`
  - `k8s:drain`
  - `metrics:query`
  - `logs:query`
- Strategic value:
  - Strong SRE story with high operator legibility.
- Current repo anchor:
  - Good fit once playbook step semantics are clearer.
- Suggested sequencing:
  - Fourth.

## 5. Cache Purge With Evidence Trail

- Required capabilities:
  - `cache:purge`
  - `metrics:query`
- Strategic value:
  - Shows a narrow, audit-friendly action with obvious receipts and replay value.
- Current repo anchor:
  - Smaller-scope example that can land earlier if runtime support is still thin elsewhere.
- Suggested sequencing:
  - Can be advanced earlier as a lower-complexity trust demo.

## 6. Secret Rotation With Policy Gate

- Required capabilities:
  - `secrets:rotate`
  - `incident:create`
  - `chat:post`
- Strategic value:
  - Strong security-engineering story centered on admission and approval.
- Current repo anchor:
  - Pairs well with the trust and evidence narrative.
- Suggested sequencing:
  - After the capability taxonomy and admission story are stable.

## Suggested Sequence

1. diagnose service -> restart pods -> notify on-call
2. rollback deployment -> verify health -> annotate incident
3. cache purge with evidence trail
4. cert renewal -> validate -> notify
5. noisy node remediation -> cordon/drain -> verify recovery
6. secret rotation with policy gate

## Recommendation

Start with one operational recovery playbook and one narrowly scoped audit playbook. That gives Guild one action-heavy story and one trust-heavy story without waiting for a full playbook portfolio.
