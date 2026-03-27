# EPIC-03: Capability Model V1

## Title
Define a human-readable external capability model for operators.

## Problem
Guild's current capability model is real and typed, but it is surfaced mainly through low-level family names and grant templates such as `http-request`, `read-resource`, and `invoke-skill`. That is internally coherent, but it is not the right mental model for an operator approving a restart, rollback, incident annotation, or cache purge.

## Outcome
Guild has a v1 external capability taxonomy that reads like operator intent, while the internal implementation remains rigorous and stable.

## User Value
Users can understand what a playbook is allowed to do without reading runtime internals or grant-template details.

## In Scope
- External capability naming rules
- Capability families and examples
- Scoping guidance
- Mapping guidance to current internal families
- Migration notes for docs, examples, and future CLI output

## Out of Scope
- Replacing the internal capability evaluator
- Renaming internal family identifiers in Rust or WIT during the first wave
- Expanding runtime support to capability families the repo does not yet implement

## Deliverables
- Capability taxonomy v1 doc
- Mapping guidance from external names to current internal capability families
- Constraints guidance for naming, scoping, and policy presentation
- Migration recommendations for docs and future CLI output

## Acceptance Criteria
- The repo has a documented external capability taxonomy using operator-readable names like `k8s:restart` and `logs:query`.
- The taxonomy explicitly states that v1 is a UX layer, not a runtime-contract rename.
- Capability names describe intent, not transport or implementation detail.
- The doc distinguishes name scope from parameter scope.
- Examples do not imply support for runtime families that do not exist today.

## Dependencies
- `00-north-star.md`
- `02-glossary-and-banned-terms.md`
- current capability evidence from CLI help and code/docs

## Risks
- Designing a taxonomy disconnected from current Guild primitives
- Overpromising future capability coverage
- Creating naming collisions or overly broad verbs

## Suggested Labels
- `epic`
- `capabilities`
- `docs`
- `runtime`

## Priority
P1

## Sequencing Notes
This should follow narrative and glossary approval, then feed playbook and CLI work. Implementation should start with docs/examples and only later consider runtime output changes.

## Starter Tasks
1. Define the external naming rule as `domain:action` with operator-readable verbs.
2. Draft capability families for Kubernetes, deploy, logs, metrics, chat, incident, secrets, and cache actions.
3. Document scoping guidance so names stay simple and parameters carry resource detail.
4. Map the external names to today's internal capability families and host-mediated boundaries.
5. Review example playbooks against the taxonomy and tighten names that are too broad.
6. Identify which future CLI surfaces should show external names first and internal details second.
