# Guild Repositioning Backlog

This backlog is ordered by dependency and grouped by epic. Each task is sized to fit in a single PR.

## EPIC-01: Narrative Reset

1. Rewrite the `README.md` hero, subhead, and opening bullets around trusted operational automation, playbooks, and trust chain value. Owner: `docs`. Size: `M`.
2. Replace artifact-first framing in `README.md` overview sections with operator-facing outcome language while preserving real trust boundaries. Owner: `docs`. Size: `M`.
3. Update `docs/project-positioning.md` to either adopt the new north-star or clearly mark itself as superseded historical framing. Owner: `docs`. Size: `M`.
4. Tighten `docs/how-guild-works.md` so it explains playbooks and trusted operations before mechanism-layer architecture. Owner: `docs`. Size: `M`.

## EPIC-02: Glossary And Language Simplification

5. Publish the canonical glossary in contributor-facing docs and link it from the main docs index. Owner: `docs`. Size: `S`.
6. Sweep top-level docs for discouraged lead terms such as "artifact," "substrate," and "reference application," then replace or demote them. Owner: `docs`. Size: `M`.
7. Update `.github/ISSUE_TEMPLATE/ux-epic.md` and `.github/ISSUE_TEMPLATE/ux-task.md` to use operator-facing terminology. Owner: `docs`. Size: `S`.
8. Review `guild --help` and the most-used subcommand help text for wording that should switch to the approved glossary. Owner: `CLI`. Size: `M`.

## EPIC-03: Capability Model V1

9. Publish the external capability taxonomy in docs with naming rules, examples, and scoping guidance. Owner: `docs`. Size: `S`.
10. Add a mapping table from external operator-readable capabilities to current internal capability families and constraints. Owner: `runtime`. Size: `M`.
11. Update capability examples in CLI docs and examples to show external names first and internal detail second where appropriate. Owner: `docs`. Size: `M`.
12. Prototype one human-readable capability rendering path for CLI or docs output without changing the internal contract model. Owner: `CLI`. Size: `M`.

## EPIC-04: Playbook Surface V1

13. Publish the playbook v1 surface doc with minimum schema shape and one example YAML. Owner: `docs`. Size: `S`.
14. Define how an operator-facing playbook references existing portable skills without inventing a new execution engine. Owner: `runtime`. Size: `M`.
15. Add a playbook-oriented concept page or section to the main docs tree so playbooks become a first-class public concept. Owner: `docs`. Size: `M`.
16. Draft an example manifest-to-playbook translation note for one current example so contributors can see the migration path. Owner: `docs`. Size: `M`.

## EPIC-05: CLI Tightening

17. Update `docs/command-language.md` with the target `admit / exec / inspect / replay` command story and compatibility notes. Owner: `docs`. Size: `M`.
18. Add doc-level command mapping tables from `run`, `show`, `get`, `why`, and `verify` to target verbs. Owner: `docs`. Size: `S`.
19. Introduce non-breaking CLI aliases or help-text previews for one target command family, starting with `inspect` if feasible. Owner: `CLI`. Size: `L`.
20. Rewrite CLI examples so the operator journey reads as admission, execution, inspection, and replay rather than raw substrate navigation. Owner: `CLI`. Size: `M`.

## EPIC-06: Reference Playbooks

21. Publish the prioritized reference playbook set with required capabilities and sequencing guidance. Owner: `docs`. Size: `S`.
22. Reframe `examples/README.md` around operator workflows and identify which examples map to future playbooks. Owner: `docs`. Size: `M`.
23. Replace or reframe `examples/skills/guild-ops-starter/README.md` so it reads as an ops playbook starter instead of a generic reference application. Owner: `docs`. Size: `M`.
24. Build one executable hero example from the reference set using only currently supported Guild trust and capability surfaces. Owner: `runtime`. Size: `L`.

## EPIC-07: Trust Docs And Site Realignment

25. Add a short trust-chain explainer to the top-level docs path that connects admission, isolation, receipts, evidence, and replay. Owner: `docs`. Size: `M`.
26. Update trust-heavy sections of `README.md`, `SPECS.md`, and `ARCHITECTURE.md` so they lead with operator value and keep guarantees honest. Owner: `docs`. Size: `L`.
27. Prepare a repo-local launch copy pack for README, release notes, and issue templates; note any external website work as a follow-up outside this repo. Owner: `website`. Size: `M`.
28. Add one trust-proof walkthrough that shows what an operator sees before, during, and after a playbook run, using current receipts and evidence surfaces only. Owner: `docs`. Size: `M`.
