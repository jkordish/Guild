# EPIC-06: Reference Playbooks

## Title
Publish concrete reference playbooks for real ops workflows.

## Problem
Guild has examples, but they read more like capability and runtime proofs than operator workflows. That makes the repo less credible to the target audience because it does not immediately show how Guild helps with common, auditable operational tasks.

## Outcome
Guild has a curated set of reference playbooks that anchor the new product story in real operational automation.

## User Value
Users can evaluate Guild against recognizable workflows such as service restart, rollback, cert renewal, node remediation, cache purge, and secret rotation.

## In Scope
- Reference playbook concepts and sequencing
- Required capability lists
- Suggested implementation shape for docs/examples
- Strategic rationale for each playbook

## Out of Scope
- Building every playbook end-to-end in one pass
- Deep provider-specific implementations for every environment
- Expanding runtime support solely to satisfy an example

## Deliverables
- Reference playbook strategy doc
- Prioritized playbook set
- Capability requirements per playbook
- Suggested rollout sequence for examples and docs

## Acceptance Criteria
- The repo has at least five strategically useful playbook concepts documented.
- Each playbook lists the operator problem, required capabilities, and why it matters.
- The playbook set covers remediation, rollback, validation, notification, and evidence-heavy workflows.
- The sequence reflects current Guild truth rather than fantasy integrations.

## Dependencies
- `03-capability-taxonomy-v1.md`
- `04-playbook-surface-v1.md`
- examples inventory under `examples/`

## Risks
- Picking examples that look impressive but do not map cleanly to current Guild constraints
- Overloading the first wave with too many integrations
- Producing playbooks that are too generic to feel operationally real

## Suggested Labels
- `epic`
- `examples`
- `playbooks`
- `docs`

## Priority
P1

## Sequencing Notes
This epic should start once capability names and playbook shape are stable enough to avoid churn in every example.

## Starter Tasks
1. Lock the initial reference playbook set and their order of delivery.
2. Define the minimal success criteria for each playbook example.
3. Map each playbook to capabilities, evidence expectations, and trust boundaries.
4. Decide which playbooks should be docs-only first versus executable examples.
5. Rewrite example intros so operator intent is visible before implementation detail.
6. Prepare one hero playbook for the README and launch narrative.
