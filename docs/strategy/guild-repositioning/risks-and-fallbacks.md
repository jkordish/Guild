# Risks And Fallbacks

Use this file to keep the execution sequence honest when the preferred path is blocked.

## Risk 1: Guardrail Rejects The New Narrative

Why it matters:

- `cargo run -q -p xtask -- project-positioning check` currently enforces the old public framing.

Fallback:

- Land a guardrail-alignment task before broad narrative rewrites.
- If needed, split `docs/project-positioning.md` handling into a dedicated task so the thesis and guardrail move together.

## Risk 2: Docs Outrun Implementation Truth

Why it matters:

- Playbooks, replay, and operator-readable capability names are central to the repositioning story, but they are not all first-class implementation surfaces yet.

Fallback:

- Phrase changes as target operator model or preview surface until the code catches up.
- Keep every task explicit about what is current behavior versus target behavior.

## Risk 3: Capability Taxonomy Drifts Away From Current Runtime Families

Why it matters:

- Operator-readable capabilities are useful only if they still map to real authority and host mediation.

Fallback:

- Require mapping guidance in every capability-facing task.
- Avoid any task that renames internal capability families as part of the docs wave.

## Risk 4: CLI Language Becomes Aspirational

Why it matters:

- The repo already warns against aspirational command names that the CLI does not support honestly.

Fallback:

- Treat `admit` and `replay` as target-state verbs unless implemented.
- Prefer alias previews, command-mapping tables, and doc guidance over binary churn in the first wave.

## Risk 5: Guild Ops Starter Gets Reframed Too Broadly

Why it matters:

- The existing repo has already pushed back on calling Guild a generic playbook engine.

Fallback:

- Reframe Guild Ops Starter as a bounded ops playbook starter, not as a universal automation framework.
- If the example begins to imply broader product scope, keep the change docs-only and narrow the example thesis.

## Risk 6: External Website Work Has No Owner In This Repo

Why it matters:

- "Site realignment" can stall or create ambiguous work if there is no site source tree here.

Fallback:

- Keep all relaunch work repo-local until an external owner and source repo are named.
- Track external site work as follow-on rather than blocking in-repo doc alignment.

## Risk 7: Task Set Becomes Too Large To Execute Coherently

Why it matters:

- Twenty-eight PR-sized tasks are tractable, but only if the dependency order stays visible.

Fallback:

- Use the implementation checklist and `tasks/INDEX.md` as the source of truth for order.
- Merge only one narrative/guardrail-sensitive task at a time until the glossary and top-level thesis settle.
