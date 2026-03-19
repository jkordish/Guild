---
name: guild-incident-triage
description: Prepare Guild's deterministic recent-failure scenario and use Guild MCP resources plus the existing summary and explain skills to triage recent failed or rejected executions.
---

# Guild Incident Triage

Use this skill when you need a repeatable local incident-style workflow over Guild's stored execution failures.

## Prepare

Run the repo helper first:

```bash
bash .agents/skills/guild-incident-triage/scripts/prepare.sh
```

That command returns JSON from `guild codex scenario --json`. Read:

- `registry_root`
- `query_uris[0]`
- `subject_execution_uris`
- `recommended_codex_ask`

If the user already has a prepared Guild root, you may pass it as the first script argument and reuse it.

## Workflow

- Prefer Guild MCP `resources/read` for the raw query payload at `query_uris[0]`.
- Then run `example/summarize-execution-query` through `guild.inspect` with a `read-resource` grant scoped to `guild://queries/executions/`.
- If one execution needs a deeper explanation, run `example/explain-execution` against one `subject_execution_uris` entry with `read-resource` scoped to `guild://executions/` and `guild://objects/records/`.
- Keep the summary grounded in stored receipts, policy decisions, termination details, and any evidence descriptors already present in Guild resources.

## Guardrails

- Do not reconstruct failures from source code when the stored execution or query resources already answer the question.
- Prefer the prepared scenario's `recommended_codex_ask` as the starting prompt, then tighten it to the user's question.
- Treat this as inspect-only work. Do not mutate registry state beyond preparing the deterministic scenario root.
