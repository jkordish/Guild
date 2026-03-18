---
name: guild-policy-denial-debug
description: Prepare Guild's deterministic policy-denial scenario and use Guild MCP resources plus the authority-debug skills to explain denied execution receipts and trusted-vs-restricted grant differences.
---

# Guild Policy Denial Debug

Use this skill when you need to explain why a stored Guild execution was denied and how trust tier or local policy changed the effective authority.

## Prepare

Run the repo helper first:

```bash
bash .agents/skills/guild-policy-denial-debug/scripts/prepare.sh
```

Read the returned JSON and keep these fields handy:

- `registry_root`
- `subject_execution_uris[0]`
- `comparison_execution_uris`
- `candidate_urls`
- `recommended_codex_ask`

## Workflow

- Read the stored denied execution with Guild MCP `resources/read`.
- Run `example/explain-capability-denial` against `subject_execution_uris[0]`.
- Run `example/diff-execution-authority` against the trusted and restricted comparison execution URIs.
- If HTTP behavior matters, run `example/explain-http-authority` against the denied execution using the prepared `candidate_urls`.
- Use `read-resource` scope `guild://executions/` for the denial, diff, and HTTP explain flows.
- When you need the fuller inspect narrative for the denied execution, follow up with `example/explain-execution` using `guild://executions/` and `guild://objects/records/`.

## Guardrails

- Prefer stored execution records and policy decisions over re-deriving policy outcomes from manifests or source directories.
- Keep the explanation focused on requested vs granted authority, trust tier, verification state, and concrete denial reasons.
- Treat the imported bundle story as host-owned provenance; do not add new tools or ad hoc shell analysis when the current Guild resources already contain the answer.
