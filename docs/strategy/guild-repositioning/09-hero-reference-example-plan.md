# Hero Reference Example Plan

## Chosen Hero Example

The first hero reference example should be:

- diagnose service -> restart pods -> notify on-call

## Why This One

This is the strongest first example because it combines the most important
operator-facing promises in one believable workflow:

- review what a workflow is allowed to do before it runs
- run one bounded operational change
- verify outcome with receipts and evidence
- notify humans explicitly instead of hiding the final step

It is also the best bridge from current repo truth. Today's repo is already
strong at diagnosis, stored-run explanation, failure scanning, comparison, and
evidence inspection. That makes the diagnose and verify parts honest now,
while keeping the restart and notify steps clearly docs-first until the runtime
frontier expands.

## Why Not Start With Another Playbook

- `cache purge with evidence trail` remains the best narrow trust demo, but it
  proves a smaller operational story.
- `rollback deployment -> verify health -> annotate incident` is a strong
  second example, but it is harder to explain credibly before the restart and
  notify flow is anchored.
- cert, node-remediation, and secret-rotation stories are still more dependent
  on future action surfaces and should stay later in the sequence.

## Current Repo Boundary

This plan is intentionally bounded to today's repo truth:

- Guild still runs skills directly, not a first-class playbook runtime.
- The current runnable bridge is read-only operational review over stored
  receipts, bounded queries, and evidence.
- Restart and notification actions are not implemented examples today and must
  stay docs-first in this phase.

## Exact Repo Surfaces

Use these files as the implementation and storytelling surfaces for the first
hero example:

- `README.md`
- `examples/README.md`
- `examples/skills/guild-ops-starter/README.md`
- `docs/strategy/guild-repositioning/07-reference-playbooks.md`
- `docs/strategy/guild-repositioning/08-manifest-to-playbook-translation-note.md`

Their roles are:

- `README.md`: top-level pointer to the starter and surrounding example/docs entrypoints
- `examples/README.md`: example inventory and reference-playbook bridge
- `examples/skills/guild-ops-starter/README.md`: current runnable starter that explains the inspect/compare/evidence part of the hero story
- `07-reference-playbooks.md`: approved reference-playbook sequence
- `08-manifest-to-playbook-translation-note.md`: honest bridge from today's skill-driven repo into playbook framing

## Current Runnable Bridge

Today the hero example is only partially runnable, and that is acceptable as
long as the docs say so directly.

The current runnable bridge is:

- diagnose and explain one stored execution with `incident-brief`
- compare two runs with `run-diff`
- scan recent failures with `recent-failures`
- inspect one stored evidence record with `evidence-summary`

That bridge proves the trust/review half of the hero example today:

- diagnosis
- verification context
- evidence trail
- operator-readable reporting

It does not yet prove:

- `k8s:restart`
- `chat:post`

## Required Capability Set

The full hero example should continue to use the approved operator-facing
capability vocabulary from the reference playbook set:

- `metrics:query`
- `logs:query`
- `k8s:restart`
- `chat:post`

The current runnable bridge remains anchored to today's internal families such
as bounded `read-resource` and bounded `invoke-skill` where those example
skills are already real.

## Likely Proof Commands Today

The first hero example should stay grounded in proof commands the repo can run
today:

```bash
guild codex bootstrap --registry-root "$GUILD_REGISTRY_ROOT" --reset
guild codex scenario --registry-root "$GUILD_REGISTRY_ROOT" --scenario recent-failure-triage --json
guild why <subject_execution_uri>
guild run incident-brief@^0.1 --input-json '{"execution_uri":"<subject_execution_uri>"}' --grants-json '<bounded-grants>'
guild run run-diff@^0.1 --input-json '{"left_execution_uri":"<left_execution_uri>","right_execution_uri":"<right_execution_uri>"}' --grants-json '<bounded-grants>'
guild run recent-failures@^0.1 --input-json '{"query_uri":"<query_uri>"}' --grants-json '<bounded-grants>'
guild ls evidence --limit 5
guild run evidence-summary@^0.1 --input-json '{"evidence_uri":"<evidence_uri>"}' --grants-json '<bounded-grants>'
```

No restart or notification proof command should be promised in the first hero
example plan until those action surfaces actually exist.

## Rollout Plan

1. Keep the hero example explicit in the strategy docs and examples entrypoints.
2. Use Guild Ops Starter as the current runnable bridge into the hero story.
3. Treat restart and notification steps as docs-first until a later task adds
   honest action surfaces on top of the current trust chain.
4. Graduate the hero example from docs-first to runnable only when it can stay
   inside the current capability and evidence boundaries without inventing a
   new workflow engine.
