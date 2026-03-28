# Guild Repositioning Implementation Checklist

This checklist turns the strategy pack into an execution sequence. It is planning-only and should stay aligned with the current runtime and trust frontier.
Use [`02-glossary-and-banned-terms.md`](./02-glossary-and-banned-terms.md)
as the canonical operator-facing vocabulary and user-facing language source for
wording decisions across this checklist.

Historical status note as of `2026-03-28`: the repo-local repositioning pass
tracked here is complete. Use this file as an audit trail for the completed
docs reframing work, not as the next implementation backlog. Current follow-on
work now lives in the portable-skill starter slice and the roadmap epic at
[`../../roadmap/epics/portable-skill-receipts-and-reference-apps.md`](../../roadmap/epics/portable-skill-receipts-and-reference-apps.md).

## Preflight Decisions

- [x] Approve the north-star narrative in [00-north-star.md](./00-north-star.md).
- [x] Approve the glossary defaults in [02-glossary-and-banned-terms.md](./02-glossary-and-banned-terms.md).
- [x] Confirm aliases-first CLI migration from [05-cli-simplification.md](./05-cli-simplification.md).
- [x] Confirm that external capability names are a UX layer first, not an internal type rename.
- [x] Confirm whether `examples/skills/guild-ops-starter/README.md` becomes a playbook-oriented starter.
- [x] Confirm whether any external website work is in scope outside this repo.

## Phase 1: Narrative, Docs, And Glossary

- [x] [TASK-01](./tasks/TASK-01-readme-hero-reset.md) Rewrite the `README.md` hero and opening story.
- [x] [TASK-02](./tasks/TASK-02-readme-overview-language-reset.md) Reset artifact-first framing in the `README.md` overview.
- [x] [TASK-03](./tasks/TASK-03-project-positioning-decision-and-update.md) Resolve `docs/project-positioning.md` and the guardrail dependency.
- [x] [TASK-04](./tasks/TASK-04-how-guild-works-operator-reframe.md) Reframe `docs/how-guild-works.md` around operator value.
- [x] [TASK-05](./tasks/TASK-05-publish-glossary-entrypoint.md) Publish the glossary entrypoint and cross-links.
- [x] [TASK-06](./tasks/TASK-06-top-level-discouraged-terms-sweep.md) Sweep discouraged lead terms from top-level docs.
- [x] [TASK-07](./tasks/TASK-07-issue-template-language-update.md) Align GitHub issue templates with the new vocabulary.
- [x] [TASK-08](./tasks/TASK-08-cli-help-terminology-review.md) Review CLI help wording against the glossary.

Phase 1 exit criteria:

- [x] `README.md`, `docs/how-guild-works.md`, and `docs/project-positioning.md` no longer fight each other.
- [x] `cargo run -q -p xtask -- project-positioning check` passes with the intended wording.
- [x] Discouraged terms are demoted or removed from the touched entrypoint docs.

## Phase 2: Capability, Playbook, And CLI UX

- [x] [TASK-09](./tasks/TASK-09-publish-capability-taxonomy-v1.md) Publish the external capability taxonomy.
- [x] [TASK-10](./tasks/TASK-10-map-external-capabilities-to-current-families.md) Document mapping to current capability families.
- [x] [TASK-11](./tasks/TASK-11-update-capability-examples-in-docs.md) Update capability examples in docs and examples.
- [x] [TASK-12](./tasks/TASK-12-prototype-human-readable-capability-rendering.md) Prototype one human-readable capability rendering path.
- [x] [TASK-13](./tasks/TASK-13-publish-playbook-v1-surface.md) Publish the playbook v1 surface doc entrypoint.
- [x] [TASK-14](./tasks/TASK-14-document-playbook-to-skill-composition.md) Document playbook-to-skill composition.
- [x] [TASK-15](./tasks/TASK-15-add-playbook-concept-entrypoint.md) Add a playbook concept entrypoint to the docs tree.
- [x] [TASK-16](./tasks/TASK-16-draft-manifest-to-playbook-translation-note.md) Draft a manifest-to-playbook translation note.
- [x] [TASK-17](./tasks/TASK-17-command-language-target-flow-update.md) Update `docs/command-language.md` with the target operator flow.
- [x] [TASK-18](./tasks/TASK-18-command-mapping-table.md) Add the command mapping table.
- [x] [TASK-19](./tasks/TASK-19-inspect-first-cli-alias-preview.md) Stage an inspect-first alias or help-preview path.
- [x] [TASK-20](./tasks/TASK-20-cli-example-journey-rewrite.md) Rewrite CLI examples around admit/exec/inspect/replay intent.

Phase 2 exit criteria:

- [x] Capability examples use operator-readable language without renaming internal contracts.
- [x] Playbook docs stay explicit that Guild is not yet shipping a generic workflow engine.
- [x] CLI docs and help previews do not claim commands the binary does not honestly support.

## Phase 3: Examples, Trust Proof, And Relaunch Surfaces

- [x] [TASK-21](./tasks/TASK-21-publish-reference-playbook-set.md) Publish the reference playbook set and sequence.
- [x] [TASK-22](./tasks/TASK-22-examples-index-operator-reframe.md) Reframe the examples index around operator workflows.
- [x] [TASK-23](./tasks/TASK-23-guild-ops-starter-playbook-reframe.md) Reframe Guild Ops Starter as a playbook-oriented starter.
- [x] [TASK-24](./tasks/TASK-24-build-one-hero-reference-example-plan.md) Define the hero reference example plan on current surfaces.
- [x] [TASK-25](./tasks/TASK-25-add-trust-chain-explainer.md) Add the trust-chain explainer.
- [x] [TASK-26](./tasks/TASK-26-trust-heavy-docs-realignment.md) Realign trust-heavy docs around operator value.
- [x] [TASK-27](./tasks/TASK-27-repo-local-launch-copy-pack.md) Prepare a repo-local launch copy pack.
- [x] [TASK-28](./tasks/TASK-28-trust-proof-walkthrough.md) Add a trust-proof walkthrough over current receipts/evidence.

Phase 3 exit criteria:

- [x] The example and trust surfaces reinforce the same operator story as the top-level docs.
- [x] No example or launch copy widens runtime or support claims by wording alone.
- [x] Any external website follow-on is explicitly tracked outside this repo.

## Final Readiness Check

- [ ] `git diff --check` passes.
- [ ] `cargo run -q -p xtask -- project-positioning check` passes.
- [ ] `cargo test -p guild-mcp --test guild_cli` passes for any CLI/help-text change.
- [ ] Every completed task file records its validation commands and migration notes.
