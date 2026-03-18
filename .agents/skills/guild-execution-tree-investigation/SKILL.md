---
name: guild-execution-tree-investigation
description: Prepare Guild's deterministic composite execution-tree scenario and use Guild MCP resources plus explain-execution-tree to find the first failing or denied node in lineage.
---

# Guild Execution Tree Investigation

Use this skill when you need to walk a stored parent/child execution lineage instead of focusing on one receipt in isolation.

## Prepare

Run the repo helper first:

```bash
bash .agents/skills/guild-execution-tree-investigation/scripts/prepare.sh
```

Read the returned JSON and keep:

- `registry_root`
- `subject_execution_uris[0]`
- `recommended_codex_ask`

## Workflow

- Read the root execution resource first so you know the stored receipt and immediate child links.
- Run `example/explain-execution-tree` against `subject_execution_uris[0]`.
- Use `read-resource` scope `guild://executions/` and `guild://objects/records/` so the tree skill can walk child receipts and evidence metadata descriptors.
- Summarize the first failing or denied node if one exists; otherwise summarize the successful lineage and note where evidence was emitted.

## Guardrails

- Prefer the stored tree over hand-following digests or source aliases unless the user explicitly asks for source-level follow-up.
- Keep traversal bounded to the prepared scenario unless the user asks to widen scope.
- Treat this as inspect-only analysis. Do not mutate the execution store.
