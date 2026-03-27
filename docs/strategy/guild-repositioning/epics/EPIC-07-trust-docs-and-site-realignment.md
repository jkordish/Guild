# EPIC-07: Trust Docs And Site Realignment

## Title
Reframe Guild's trust model as operator-facing product value across docs and launch surfaces.

## Problem
Guild already has the substance for trust, but the repo often explains it in platform terms first. Admission, isolation, receipts, evidence, and replay are part of the real product, yet they are not consistently narrated as the reason an operator should trust Guild with operational automation.

## Outcome
Guild's trust chain is explained as a user-facing product promise across docs, examples, and launch surfaces, with clear boundaries around what is implemented today versus later.

## User Value
Operators can understand why Guild is safer than ad hoc automation and what proof they will get after a run, without reading architecture docs first.

## In Scope
- Trust-language realignment in repo docs
- Admission, isolation, receipts, evidence, and replay framing
- Launch-surface planning for in-repo marketing copy and issue templates
- Explicit note when external website work sits outside this repository

## Out of Scope
- New cryptographic or runtime features
- Replay engine implementation
- External site implementation if the source is not in this repo

## Deliverables
- Trust-focused narrative guidance
- Cross-reference plan for README, architecture, specs, and examples
- In-repo launch-surface alignment plan
- Explicit list of trust claims that must stay bounded to current reality

## Acceptance Criteria
- Trust concepts are described in user-value terms before mechanism-layer detail.
- Docs clearly explain how admission, isolation, receipts, evidence, and replay fit together.
- The repo does not imply stronger guarantees than Guild currently provides.
- External-site work is separated cleanly from in-repo deliverables when no site source exists here.

## Dependencies
- `00-north-star.md`
- `01-messaging-audit.md`
- `06-roadmap.md`
- `ARCHITECTURE.md`
- `SPECS.md`

## Risks
- Trust claims drifting beyond the current implementation
- Replay being described as shipped when it is still a target surface
- Confusing evidence records with broader compliance guarantees

## Suggested Labels
- `epic`
- `docs`
- `trust`
- `website`

## Priority
P2

## Sequencing Notes
This epic should start after the narrative, glossary, capability, and playbook direction are set. It is the bridge into relaunch work and external marketing copy.

## Child Task Files

1. [TASK-25: Add trust-chain explainer](../tasks/TASK-25-add-trust-chain-explainer.md)
2. [TASK-26: Trust-heavy docs realignment](../tasks/TASK-26-trust-heavy-docs-realignment.md)
3. [TASK-27: Repo-local launch copy pack](../tasks/TASK-27-repo-local-launch-copy-pack.md)
4. [TASK-28: Trust-proof walkthrough](../tasks/TASK-28-trust-proof-walkthrough.md)
