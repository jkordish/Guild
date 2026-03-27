# Epic Index

Use [`../02-glossary-and-banned-terms.md`](../02-glossary-and-banned-terms.md)
as the canonical operator-facing vocabulary and user-facing language source for
epic titles, descriptions, and follow-on task wording.

## Sequence

1. `EPIC-01-narrative-reset.md`
   - Reset the public story around trusted operational automation.
2. `EPIC-02-glossary-and-language-simplification.md`
   - Make the new vocabulary concrete and reusable.
3. `EPIC-03-capability-model-v1.md`
   - Define operator-readable capabilities and mapping guidance.
4. `EPIC-04-playbook-surface-v1.md`
   - Define the playbook as the operator-facing automation surface.
5. `EPIC-05-cli-tightening.md`
   - Stage the CLI path toward admit / exec / inspect / replay.
6. `EPIC-06-reference-playbooks.md`
   - Build the concrete operator workflow anchors.
7. `EPIC-07-trust-docs-and-site-realignment.md`
   - Align trust, evidence, replay, and launch surfaces with the new story.

## Dependency Notes

- Epic 01 unlocks the public-facing rewrite.
- Epic 02 stabilizes the terms that Epics 03 through 07 depend on.
- Epic 03 feeds capability wording into playbooks, CLI, and examples.
- Epic 04 depends on the glossary and capability model.
- Epic 05 depends on the glossary and playbook UX.
- Epic 06 depends on the capability and playbook definitions.
- Epic 07 depends on the narrative reset and trust-language decisions, and partially on the example direction.

## Filing Checklist

- Convert each epic file into a tracked issue before broad rewrites begin.
- Break the epic into the one-PR task files listed in `../tasks/INDEX.md`.
- Tag any item that changes current guardrails or CLI wording as needing test and migration review.

## Task Links

- [EPIC-01 task files](../tasks/TASK-01-readme-hero-reset.md), [TASK-02](../tasks/TASK-02-readme-overview-language-reset.md), [TASK-03](../tasks/TASK-03-project-positioning-decision-and-update.md), [TASK-04](../tasks/TASK-04-how-guild-works-operator-reframe.md)
- [EPIC-02 task files](../tasks/TASK-05-publish-glossary-entrypoint.md), [TASK-06](../tasks/TASK-06-top-level-discouraged-terms-sweep.md), [TASK-07](../tasks/TASK-07-issue-template-language-update.md), [TASK-08](../tasks/TASK-08-cli-help-terminology-review.md)
- [EPIC-03 task files](../tasks/TASK-09-publish-capability-taxonomy-v1.md), [TASK-10](../tasks/TASK-10-map-external-capabilities-to-current-families.md), [TASK-11](../tasks/TASK-11-update-capability-examples-in-docs.md), [TASK-12](../tasks/TASK-12-prototype-human-readable-capability-rendering.md)
- [EPIC-04 task files](../tasks/TASK-13-publish-playbook-v1-surface.md), [TASK-14](../tasks/TASK-14-document-playbook-to-skill-composition.md), [TASK-15](../tasks/TASK-15-add-playbook-concept-entrypoint.md), [TASK-16](../tasks/TASK-16-draft-manifest-to-playbook-translation-note.md)
- [EPIC-05 task files](../tasks/TASK-17-command-language-target-flow-update.md), [TASK-18](../tasks/TASK-18-command-mapping-table.md), [TASK-19](../tasks/TASK-19-inspect-first-cli-alias-preview.md), [TASK-20](../tasks/TASK-20-cli-example-journey-rewrite.md)
- [EPIC-06 task files](../tasks/TASK-21-publish-reference-playbook-set.md), [TASK-22](../tasks/TASK-22-examples-index-operator-reframe.md), [TASK-23](../tasks/TASK-23-guild-ops-starter-playbook-reframe.md), [TASK-24](../tasks/TASK-24-build-one-hero-reference-example-plan.md)
- [EPIC-07 task files](../tasks/TASK-25-add-trust-chain-explainer.md), [TASK-26](../tasks/TASK-26-trust-heavy-docs-realignment.md), [TASK-27](../tasks/TASK-27-repo-local-launch-copy-pack.md), [TASK-28](../tasks/TASK-28-trust-proof-walkthrough.md)
