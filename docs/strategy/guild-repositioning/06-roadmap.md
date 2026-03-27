# Repositioning Roadmap

## Phase 0: Decisions

Goal: lock the operator story before rewriting public surfaces.

- Approve the north-star narrative and glossary defaults.
- Confirm aliases-first CLI migration.
- Confirm that external capabilities are a UX layer first.
- Confirm whether Guild Ops Starter evolves into a playbook-oriented starter set.
- Confirm scope for site work outside the repo.

## Phase 1: Narrative, Docs, And Glossary

Goal: make the repo read like trusted operational automation.

- Rewrite the top-level story in `README.md`, `docs/how-guild-works.md`, and `docs/command-language.md`.
- Replace lead substrate language with operator-facing terms.
- Update examples index and GitHub planning templates to use the new vocabulary.
- Migrate the existing `project-positioning` guardrail so it enforces the new narrative instead of the old one.

## Phase 2: Capability And Playbook UX

Goal: define how operators review automation.

- Publish the operator-readable capability taxonomy.
- Define playbook v1 shape and example YAML.
- Introduce admission, execution, inspection, and replay as the target operator flow.
- Add mapping guidance from new operator terms to current implementation surfaces.

## Phase 3: Examples, Trust Proof, And Relaunch

Goal: show concrete operator value with believable examples.

- Build reference playbook examples for restart, rollback, cert, node remediation, cache purge, and secret rotation.
- Reframe trust docs around admission, isolation, receipts, evidence, and replay.
- Align in-repo docs and launch-ready copy around the new story.
- Decide whether an external website repo needs a parallel rollout.

## Dependencies

- Phase 1 depends on human approval of the north-star, glossary, and CLI posture.
- Phase 2 depends on the glossary so capability and playbook terms are stable.
- Phase 3 depends on capability and playbook UX so examples do not drift into one-off language.
- Guardrail updates depend on agreement that `docs/project-positioning.md` will no longer be the old portable-artifact-first framing source.

## Risks

- The current `project-positioning check` will resist parts of the new story until it is updated intentionally.
- The repo may overpromise playbook support if docs move faster than examples or CLI affordances.
- A future capability rename could become too invasive if UX-layer and runtime-contract work are conflated.
- The lack of an in-repo website means "site relignment" could stall unless ownership is clarified.
- Examples could drift into generic workflow-engine territory if capability and evidence boundaries are not kept explicit.
