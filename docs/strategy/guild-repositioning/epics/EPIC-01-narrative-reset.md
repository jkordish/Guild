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

## Starter Tasks
1. Rewrite the `README.md` hero and opening sections around trusted operational automation.
2. Replace "reference application" lead-ins with "ops playbook" framing where the repo already means operator workflow.
3. Update `docs/project-positioning.md` to align with the new north-star or explicitly mark it as historical framing.
4. Tighten `docs/how-guild-works.md` so trust-chain concepts are expressed as operator value before architecture detail.
5. Update `examples/README.md` intros so examples read like operator workflows, not substrate demos.
6. Add one short positioning checklist for future doc authors so the language does not drift back.
