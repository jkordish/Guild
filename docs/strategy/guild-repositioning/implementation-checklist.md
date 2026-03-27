# Guild Repositioning Implementation Checklist

This checklist turns the strategy pack into an execution sequence. It is planning-only and should stay aligned with the current runtime and trust frontier.
Use [`02-glossary-and-banned-terms.md`](./02-glossary-and-banned-terms.md)
as the canonical operator-facing vocabulary and user-facing language source for
wording decisions across this checklist.

## Preflight Decisions

- [ ] Approve the north-star narrative in [00-north-star.md](./00-north-star.md).
- [ ] Approve the glossary defaults in [02-glossary-and-banned-terms.md](./02-glossary-and-banned-terms.md).
- [ ] Confirm aliases-first CLI migration from [05-cli-simplification.md](./05-cli-simplification.md).
- [ ] Confirm that external capability names are a UX layer first, not an internal type rename.
- [ ] Confirm whether `examples/skills/guild-ops-starter/README.md` becomes a playbook-oriented starter.
- [ ] Confirm whether any external website work is in scope outside this repo.

## Phase 1: Narrative, Docs, And Glossary

- [ ] [TASK-01](./tasks/TASK-01-readme-hero-reset.md) Rewrite the `README.md` hero and opening story.
- [ ] [TASK-02](./tasks/TASK-02-readme-overview-language-reset.md) Reset artifact-first framing in the `README.md` overview.
- [ ] [TASK-03](./tasks/TASK-03-project-positioning-decision-and-update.md) Resolve `docs/project-positioning.md` and the guardrail dependency.
- [ ] [TASK-04](./tasks/TASK-04-how-guild-works-operator-reframe.md) Reframe `docs/how-guild-works.md` around operator value.
- [ ] [TASK-05](./tasks/TASK-05-publish-glossary-entrypoint.md) Publish the glossary entrypoint and cross-links.
- [ ] [TASK-06](./tasks/TASK-06-top-level-discouraged-terms-sweep.md) Sweep discouraged lead terms from top-level docs.
- [ ] [TASK-07](./tasks/TASK-07-issue-template-language-update.md) Align GitHub issue templates with the new vocabulary.
- [ ] [TASK-08](./tasks/TASK-08-cli-help-terminology-review.md) Review CLI help wording against the glossary.

Phase 1 exit criteria:

- [ ] `README.md`, `docs/how-guild-works.md`, and `docs/project-positioning.md` no longer fight each other.
- [ ] `cargo run -q -p xtask -- project-positioning check` passes with the intended wording.
- [ ] Discouraged terms are demoted or removed from the touched entrypoint docs.

## Phase 2: Capability, Playbook, And CLI UX

- [ ] [TASK-09](./tasks/TASK-09-publish-capability-taxonomy-v1.md) Publish the external capability taxonomy.
- [ ] [TASK-10](./tasks/TASK-10-map-external-capabilities-to-current-families.md) Document mapping to current capability families.
- [ ] [TASK-11](./tasks/TASK-11-update-capability-examples-in-docs.md) Update capability examples in docs and examples.
- [ ] [TASK-12](./tasks/TASK-12-prototype-human-readable-capability-rendering.md) Prototype one human-readable capability rendering path.
- [ ] [TASK-13](./tasks/TASK-13-publish-playbook-v1-surface.md) Publish the playbook v1 surface doc entrypoint.
- [ ] [TASK-14](./tasks/TASK-14-document-playbook-to-skill-composition.md) Document playbook-to-skill composition.
- [ ] [TASK-15](./tasks/TASK-15-add-playbook-concept-entrypoint.md) Add a playbook concept entrypoint to the docs tree.
- [ ] [TASK-16](./tasks/TASK-16-draft-manifest-to-playbook-translation-note.md) Draft a manifest-to-playbook translation note.
- [ ] [TASK-17](./tasks/TASK-17-command-language-target-flow-update.md) Update `docs/command-language.md` with the target operator flow.
- [ ] [TASK-18](./tasks/TASK-18-command-mapping-table.md) Add the command mapping table.
- [ ] [TASK-19](./tasks/TASK-19-inspect-first-cli-alias-preview.md) Stage an inspect-first alias or help-preview path.
- [ ] [TASK-20](./tasks/TASK-20-cli-example-journey-rewrite.md) Rewrite CLI examples around admit/exec/inspect/replay intent.

Phase 2 exit criteria:

- [ ] Capability examples use operator-readable language without renaming internal contracts.
- [ ] Playbook docs stay explicit that Guild is not yet shipping a generic workflow engine.
- [ ] CLI docs and help previews do not claim commands the binary does not honestly support.

## Phase 3: Examples, Trust Proof, And Relaunch Surfaces

- [x] [TASK-21](./tasks/TASK-21-publish-reference-playbook-set.md) Publish the reference playbook set and sequence.
- [x] [TASK-22](./tasks/TASK-22-examples-index-operator-reframe.md) Reframe the examples index around operator workflows.
- [ ] [TASK-23](./tasks/TASK-23-guild-ops-starter-playbook-reframe.md) Reframe Guild Ops Starter as a playbook-oriented starter.
- [ ] [TASK-24](./tasks/TASK-24-build-one-hero-reference-example-plan.md) Define the hero reference example plan on current surfaces.
- [ ] [TASK-25](./tasks/TASK-25-add-trust-chain-explainer.md) Add the trust-chain explainer.
- [ ] [TASK-26](./tasks/TASK-26-trust-heavy-docs-realignment.md) Realign trust-heavy docs around operator value.
- [ ] [TASK-27](./tasks/TASK-27-repo-local-launch-copy-pack.md) Prepare a repo-local launch copy pack.
- [ ] [TASK-28](./tasks/TASK-28-trust-proof-walkthrough.md) Add a trust-proof walkthrough over current receipts/evidence.

Phase 3 exit criteria:

- [ ] The example and trust surfaces reinforce the same operator story as the top-level docs.
- [ ] No example or launch copy widens runtime or support claims by wording alone.
- [ ] Any external website follow-on is explicitly tracked outside this repo.

## Final Readiness Check

- [ ] `git diff --check` passes.
- [ ] `cargo run -q -p xtask -- project-positioning check` passes.
- [ ] `cargo test -p guild-mcp --test guild_cli` passes for any CLI/help-text change.
- [ ] Every completed task file records its validation commands and migration notes.
