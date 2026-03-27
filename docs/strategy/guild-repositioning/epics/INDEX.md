# Epic Index

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
- Break the epic into the one-PR tasks listed in `../tasks.md`.
- Tag any item that changes current guardrails or CLI wording as needing test and migration review.
