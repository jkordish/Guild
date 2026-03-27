# Glossary And Banned Terms

## Canonical Terms

| Term | Use It For | Rationale | Preferred Usage |
| --- | --- | --- | --- |
| trusted operational automation | The product category | Says what Guild does for operators in plain language | "Guild is trusted operational automation for engineering teams." |
| ops playbook | The operator-facing workflow unit | Makes the application model concrete | "This playbook rolls back the deployment and verifies recovery." |
| portable skill | The reusable execution unit | Keeps the existing technical truth without leading with artifacts | "The playbook uses portable skills for diagnosis, rollback, and notification." |
| capability | Human-readable permission | Better operator term than grant algebra | "This playbook needs `chat:post` and `deploy:rollback`." |
| admission | Pre-execution approval and narrowing | Ties policy to an operator checkpoint | "Admission confirms the requested capabilities before execution." |
| isolation | Runtime boundary language | Easier to understand than runtime-surface jargon | "Execution stays isolated from ambient host authority." |
| receipt | Durable execution record | Shorter and more operator-readable than execution-record-first wording | "The run produced a receipt you can inspect later." |
| evidence | Durable proof material | Already strong and user-meaningful | "Evidence includes the health check output and notification record." |
| replay | Re-run or re-check from stored execution context | Useful operator concept and future CLI anchor | "Replay the rollback flow against the stored receipt." |
| inspectability | Ability to explain what happened after the run | Better user-value phrase than provenance-heavy wording | "Guild prioritizes inspectability over hidden automation." |

## Discouraged Terms

| Term | Status | Rationale | Use Instead |
| --- | --- | --- | --- |
| artifact | Discouraged as a lead term | Technically correct, but too packaging-heavy for the opening story | portable skill, receipt, evidence |
| substrate | Avoid | Internal-architecture word with no operator value | platform surface, runtime boundary, system |
| runtime | Avoid as a hero noun | Important technically, but not the user promise | isolation, execution boundary, host-mediated execution |
| typed grant algebra | Avoid publicly | Accurate internally, unreadable externally | capability model, capability policy |
| trust layer | Discouraged as lead wording | Abstract and infrastructure-shaped | admission, isolation, receipts, evidence |
| provenance-heavy phrasing without user value | Avoid | Explains mechanics before value | inspectability, replay, evidence trail |
| reference application | Discouraged as default example framing | Explains repo organization, not operator benefit | operator starter set, reference playbook |
| playbook engine | Avoid as a lead product category | Sounds generic and platform-heavy | trusted operational automation |

## Preferred Usage Examples

- Prefer:
  - "Guild admits and runs ops playbooks with explicit capabilities."
- Instead of:
  - "Guild executes portable artifacts through a trust layer."

- Prefer:
  - "This playbook needs `k8s:restart`, `metrics:query`, and `chat:post`."
- Instead of:
  - "This run requires multiple capability grants and transport-scoped constraints."

- Prefer:
  - "Inspect the receipt and evidence trail after the rollback."
- Instead of:
  - "Review the durable execution record and artifact provenance after the run."

## When Mechanism-Layer Terms Are Still Necessary

- `artifact_digest`, `ResolvedSkillRef`, and similar terms remain correct in specs, code, and migration notes.
- Internal family names such as `read-resource` and `invoke-skill` remain canonical for the current runtime and manifest contract until a later implementation phase changes that explicitly.
- `runtime/compatibility`, `trust/verification`, and other established failure labels should only change when the CLI contract changes with tests and migration notes.

## Editorial Rule

Lead with the operator-readable term first. Introduce the mechanism-layer term only when the repo needs exact technical precision.
