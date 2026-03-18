---
name: guild-bundle-verification-check
description: Reuse Guild's imported-bundle policy-denial scenario to inspect execution receipts for digest provenance, verification state, trust tier, and whether imported HTTP authority should have been granted.
---

# Guild Bundle Verification Check

Use this skill when the question is really about imported bundle provenance and trust, not just a generic execution failure.

## Prepare

Run the repo helper first:

```bash
bash .agents/skills/guild-bundle-verification-check/scripts/prepare.sh
```

The script reuses the shared `policy-denial-debug` scenario. Read:

- `subject_execution_uris[0]`
- `comparison_execution_uris`
- `candidate_urls`

## Workflow

- Read the denied and comparison execution resources through Guild MCP `resources/read`.
- Compare `resolved_skill.digest`, `policy_decision.trust_tier`, and `policy_decision.verification_state` across the imported executions.
- Use `example/diff-execution-authority` when you need a structured grant comparison between the trusted and restricted imported receipts.
- Use `example/explain-capability-denial` to confirm which requested capability was denied for the stored execution.
- Use `example/explain-http-authority` with one prepared candidate URL to answer whether the imported execution should have been allowed HTTP.

## Guardrails

- Keep the analysis rooted in host-owned execution records. This skill is about what Guild recorded after import and policy evaluation, not about reconstructing bundle contents manually.
- Call out when the digest stays constant but trust tier or verification state changes the outcome.
- Do not add a new Guild tool for bundle inspection in this workflow; reuse the existing execution resources and explain skills.
