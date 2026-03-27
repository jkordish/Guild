# EPIC-01: Narrative Reset

## Title
Reset Guild's public narrative around trusted operational automation.

## Problem
Guild already has real trust and execution boundaries, but the repo leads with platform and substrate language instead of operator outcomes. `README.md`, `docs/project-positioning.md`, and related docs currently emphasize portable skill artifacts, trust layers, and reference applications before they explain the operator problem Guild solves.

## Outcome
Guild's top-level story consistently leads with safe operational automation, playbooks, capabilities, admission, evidence, and replay. Platform and runtime details remain available, but they no longer carry the first paragraph.

## User Value
Ops, platform, SRE, and security engineers can understand what Guild is for in one pass and decide whether it fits their workflows without learning Guild's internal architecture first.

## In Scope
- Top-level narrative for `README.md`
- Supporting framing in `docs/project-positioning.md`
- Supporting framing in `docs/how-guild-works.md`
- Narrative alignment in `examples/README.md`
- Narrative alignment in roadmap and issue-planning surfaces

## Out of Scope
- Runtime or ABI changes
- Capability taxonomy implementation
- Playbook schema implementation
- External website work that is not stored in this repository

## Deliverables
- Approved north-star narrative for Guild
- Updated top-level narrative guidance for repo-facing docs
- Before-and-after messaging inventory
- Migration notes for old phrases that should be retired or demoted

## Acceptance Criteria
- `README.md` leads with trusted operational automation instead of portable artifacts.
- Top-level Guild positioning explains playbooks before mechanism-layer terms.
- Repo docs use the same product thesis, target audience, and promise set.
- Mechanism-layer terms remain only where technically necessary.
- No updated doc implies capabilities or runtime features the repo does not actually support today.

## Dependencies
- `00-north-star.md`
- `01-messaging-audit.md`
- `02-glossary-and-banned-terms.md`

## Risks
- Over-correcting into vague marketing copy
- Hiding real trust boundaries behind softer language
- Creating tension with existing project-positioning guardrails before they are explicitly updated

## Suggested Labels
- `epic`
- `docs`
- `positioning`
- `product`

## Priority
P0

## Sequencing Notes
This epic should land first. The glossary can start in parallel, but broad doc or site edits should not proceed until the narrative center is approved.

## Child Task Files

1. [TASK-01: README hero reset](../tasks/TASK-01-readme-hero-reset.md)
2. [TASK-02: README overview language reset](../tasks/TASK-02-readme-overview-language-reset.md)
3. [TASK-03: Project positioning decision and update](../tasks/TASK-03-project-positioning-decision-and-update.md)
4. [TASK-04: How Guild Works operator reframe](../tasks/TASK-04-how-guild-works-operator-reframe.md)
