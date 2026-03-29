# 02. Glossary and Banned Terms

**Status:** Proposed
**Owner:** Product + docs
**Last updated:** 2026-03-28

This is the canonical vocabulary for Guild. If a noun is not in the approved list, it should not be a top-level noun in site copy, README copy, or CLI help unless there is a strong reason.

## Canonical terms

| Term | Definition | Use when | Avoid when |
| --- | --- | --- | --- |
| **Capability** | A permissionable action alias such as `k8s:restart` or `metrics:query`. | Explaining what a playbook is allowed to do. | Referring to a whole workflow. |
| **Skill** | A reusable procedural unit that teaches the system how to perform a task. | Explaining the portable building block. | Referring to the end-user outcome. |
| **Pack** | A versioned bundle of skills, playbooks, metadata, and verification state. | Talking about installable distribution. | Referring to a single skill. |
| **Playbook** | A user-facing operational workflow assembled from skills and capabilities. | Talking about the thing a user runs. | Referring to a single tool call. |
| **Approval** | A policy or human gate that must pass before risky mutation. | Talking about control and governance. | Referring to generic auth or identity. |
| **Evidence** | The facts collected during a run: inputs, observations, checks, outputs. | Talking about what was gathered. | Referring to the whole run record. |
| **Receipt** | The structured record of intent, approvals, actions, evidence, and outcome for a run. | Talking about run history or auditability. | Talking about a single evidence item. |
| **Verify** | Evaluate whether a skill, pack, or playbook is valid, installable, compatible, and tested. | Talking about curation and trust. | Talking about runtime health checks only. |
| **Replay** | Re-run or reconstruct a prior execution path using a receipt. | Talking about post-run review and reproducibility. | Talking about simple retries. |
| **Curated** | Reviewed by the Guild team with basic trust checks. | Labeling first rung of trust. | Claiming deep compatibility or eval coverage. |
| **Verified** | Passed the verification matrix for supported targets. | Labeling strongest trust state. | Any asset lacking tests and receipts. |

## Discouraged or banned top-level terms

| Term | Why it hurts | Preferred replacement |
| --- | --- | --- |
| **Agent operating system** | Too broad, sounds infrastructure-heavy, invites the wrong comparison set. | Trusted playbook layer |
| **Workflow engine** | Sounds generic and loses the AI / packaging / trust angle. | Playbook runtime |
| **Orchestration layer** | Internal-mechanism framing. | Playbook layer or execution layer |
| **Artifact** (as a headline noun) | Too abstract. Users care about packs, skills, and receipts. | Pack, skill, receipt |
| **Autonomous remediation** | Over-claims and triggers immediate trust skepticism. | Policy-gated remediation |
| **Self-healing** | Marketing language unless narrowly bounded and proven. | Verified remediation playbook |
| **Marketplace** | Premature and implies breadth over trust. | Curated pack catalog |
| **Memory platform** | Competes in a much broader category and muddies the thesis. | Not a Guild headline concept |
| **Multi-agent system** | Tells the user about internals they did not ask for. | Playbooks, approvals, receipts |

## Naming rules

### Capability names

- Format: `domain:verb`
- Examples: `metrics:query`, `k8s:restart`, `deploy:rollback`, `chat:post`
- Keep the external name coarse and human-readable.
- Put tool-specific nuance in metadata or adapters.

### Skill names

- Use short, descriptive, action-oriented names.
- Example: `k8s-restart`, `logs-query`, `cert-validate`.
- Do not put environment or vendor details in the primary name unless they are essential.

### Pack names

- Name by operational outcome, not implementation detail.
- Good: `incident-triage`, `safe-change`, `k8s-remediation`
- Bad: `ops-skill-collection`, `multi-tool-runtime-pack`

### Playbook names

- Name by user intent and outcome.
- Good: `restart-service-with-evidence`, `rollback-and-verify`
- Bad: `run-pipeline-7`, `dynamic-procedure-02`

## Style rules

- Prefer concrete verbs over abstract claims.
- Name the target audience when possible.
- Describe the blast radius before celebrating automation.
- Explain trust with evidence, approval, and verification, not with adjectives.

## Copy examples

### Good

- Run a playbook.
- Inspect the receipt.
- Verify the pack before installing it.
- This playbook needs `k8s:restart` and `chat:post`.

### Bad

- Execute an intelligent operational artifact.
- Autonomous remediation with confidence.
- Universal workflow substrate.
