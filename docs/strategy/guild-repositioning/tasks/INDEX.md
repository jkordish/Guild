# Repositioning Task Index

This directory contains the issue-ready, PR-sized execution tasks for the Guild repositioning work.

## Phase 1: Narrative, Docs, And Glossary

| ID | Task | Owner | Size | Depends On |
| --- | --- | --- | --- | --- |
| 01 | [README hero reset](./TASK-01-readme-hero-reset.md) | `docs` | `M` | north-star approval |
| 02 | [README overview language reset](./TASK-02-readme-overview-language-reset.md) | `docs` | `M` | TASK-01 |
| 03 | [Project positioning decision and update](./TASK-03-project-positioning-decision-and-update.md) | `docs` | `M` | north-star approval |
| 04 | [How Guild Works operator reframe](./TASK-04-how-guild-works-operator-reframe.md) | `docs` | `M` | TASK-01, TASK-03 |
| 05 | [Publish glossary entrypoint](./TASK-05-publish-glossary-entrypoint.md) | `docs` | `S` | north-star approval |
| 06 | [Top-level discouraged-terms sweep](./TASK-06-top-level-discouraged-terms-sweep.md) | `docs` | `M` | TASK-05 |
| 07 | [Issue-template language update](./TASK-07-issue-template-language-update.md) | `docs` | `S` | TASK-05 |
| 08 | [CLI help terminology review](./TASK-08-cli-help-terminology-review.md) | `CLI` | `M` | TASK-05 |

## Phase 2: Capability, Playbook, And CLI UX

| ID | Task | Owner | Size | Depends On |
| --- | --- | --- | --- | --- |
| 09 | [Publish capability taxonomy v1](./TASK-09-publish-capability-taxonomy-v1.md) | `docs` | `S` | TASK-05 |
| 10 | [Map external capabilities to current families](./TASK-10-map-external-capabilities-to-current-families.md) | `runtime` | `M` | TASK-09 |
| 11 | [Update capability examples in docs](./TASK-11-update-capability-examples-in-docs.md) | `docs` | `M` | TASK-09, TASK-10 |
| 12 | [Prototype human-readable capability rendering](./TASK-12-prototype-human-readable-capability-rendering.md) | `CLI` | `M` | TASK-10 |
| 13 | [Publish playbook v1 surface](./TASK-13-publish-playbook-v1-surface.md) | `docs` | `S` | TASK-09 |
| 14 | [Document playbook-to-skill composition](./TASK-14-document-playbook-to-skill-composition.md) | `runtime` | `M` | TASK-13 |
| 15 | [Add playbook concept entrypoint](./TASK-15-add-playbook-concept-entrypoint.md) | `docs` | `M` | TASK-13, TASK-14 |
| 16 | [Draft manifest-to-playbook translation note](./TASK-16-draft-manifest-to-playbook-translation-note.md) | `docs` | `M` | TASK-15 |
| 17 | [Command-language target-flow update](./TASK-17-command-language-target-flow-update.md) | `docs` | `M` | TASK-05, TASK-15 |
| 18 | [Command mapping table](./TASK-18-command-mapping-table.md) | `docs` | `S` | TASK-17 |
| 19 | [Inspect-first CLI alias preview](./TASK-19-inspect-first-cli-alias-preview.md) | `CLI` | `L` | TASK-08, TASK-17 |
| 20 | [CLI example journey rewrite](./TASK-20-cli-example-journey-rewrite.md) | `CLI` | `M` | TASK-18, TASK-19 |

## Phase 3: Examples, Trust Proof, And Relaunch Surfaces

| ID | Task | Owner | Size | Depends On |
| --- | --- | --- | --- | --- |
| 21 | [Publish reference playbook set](./TASK-21-publish-reference-playbook-set.md) | `docs` | `S` | TASK-15 |
| 22 | [Examples index operator reframe](./TASK-22-examples-index-operator-reframe.md) | `docs` | `M` | TASK-21 |
| 23 | [Guild Ops Starter playbook reframe](./TASK-23-guild-ops-starter-playbook-reframe.md) | `docs` | `M` | TASK-21, TASK-22 |
| 24 | [Build one hero reference example plan](./TASK-24-build-one-hero-reference-example-plan.md) | `runtime` | `L` | TASK-21, TASK-23 |
| 25 | [Add trust-chain explainer](./TASK-25-add-trust-chain-explainer.md) | `docs` | `M` | TASK-01, TASK-05 |
| 26 | [Trust-heavy docs realignment](./TASK-26-trust-heavy-docs-realignment.md) | `docs` | `L` | TASK-25, TASK-03 |
| 27 | [Repo-local launch copy pack](./TASK-27-repo-local-launch-copy-pack.md) | `website` | `M` | TASK-22, TASK-25 |
| 28 | [Trust-proof walkthrough](./TASK-28-trust-proof-walkthrough.md) | `docs` | `M` | TASK-24, TASK-26 |
