# EPIC-04: Playbook Surface V1

## Title
Define the first operator-facing playbook surface for Guild.

## Problem
Guild already has reusable skills and a trust-aware execution model, but the repo does not present playbooks as the primary user-facing automation unit. That leaves the operator story fragmented and makes Guild sound like a runtime toolkit instead of a product for safe operational automation.

## Outcome
Guild has a documented v1 playbook surface that explains how playbooks relate to skills, capabilities, admission, evidence, and replay.

## User Value
Users can picture how they would describe a real operational workflow in Guild without reading low-level manifest or runtime internals first.

## In Scope
- Definition of a playbook in Guild terms
- Relationship between playbooks and portable skills
- Minimum viable playbook schema shape
- Example YAML for at least one real ops workflow
- Execution and evidence model explanation
- Open questions and explicit non-decisions

## Out of Scope
- Shipping a production-ready playbook engine
- Designing a large workflow DSL
- Hiding or replacing the existing skill system
- Apply-mode expansion beyond current trusted boundaries

## Deliverables
- Playbook surface v1 doc
- At least one example YAML
- Explicit statement of what remains planning-only
- Open-questions section for follow-on implementation work

## Acceptance Criteria
- The repo clearly states that the playbook is the operator-facing application unit.
- The doc shows how a playbook composes existing skills and trust controls.
- The minimum schema shape is concrete enough to guide examples and CLI planning.
- The doc does not invent runtime behavior that Guild cannot currently guarantee.
- Open questions are captured explicitly instead of hidden in vague prose.

## Dependencies
- `00-north-star.md`
- `03-capability-taxonomy-v1.md`
- current execution and evidence model in `README.md`, `SPECS.md`, and `ARCHITECTURE.md`

## Risks
- Designing a playbook shape that ignores current Guild execution primitives
- Turning the doc into an implicit DSL proposal too early
- Blurring inspect, plan, and apply boundaries

## Suggested Labels
- `epic`
- `playbooks`
- `docs`
- `ux`

## Priority
P1

## Sequencing Notes
This epic should follow capability-model agreement. It provides the conceptual bridge between the new narrative and later example/CLI work.

## Starter Tasks
1. Define what a playbook is and is not in Guild.
2. Document how playbooks compose portable skills rather than replacing them.
3. Draft a minimum schema shape for playbook metadata, inputs, capabilities, steps, admission, and evidence.
4. Author one concrete YAML example for diagnose, restart, and notify.
5. Document how execution results, receipts, and evidence should be surfaced for a playbook run.
6. Capture unresolved questions around storage format, CLI invocation, and replay semantics.
