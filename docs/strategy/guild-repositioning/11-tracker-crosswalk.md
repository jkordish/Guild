# 11. Tracker Crosswalk

**Status:** Active interpretation layer
**Last updated:** 2026-03-29

This doc processes the imported repositioning stack against the current Guild
repository and GitHub tracker.

Use it to answer four questions:

1. Which imported milestones are already done versus still open?
2. Which imported epics map cleanly to existing GitHub issues?
3. Which imported tasks are already complete, absorbed into an open issue, or
   intentionally deferred?
4. Which imported assumptions are accepted as strategy input versus rejected as
   current repo truth?

This file is the bridge between the imported strategy stack and the live
contracts-first repository. It is not runtime truth. Runtime truth still lives
in `README.md`, `SPECS.md`, `ARCHITECTURE.md`, `docs/command-language.md`, and
`docs/testing.md`.

## Adoption Rules

- Treat the imported stack as planning input, not automatic product truth.
- Keep already-completed M1 repositioning work closed unless the repo needs new
  code or docs.
- Absorb overlapping M2-M4 work into the active follow-on issue set instead of
  creating a second parallel roadmap.
- Reject imported assumptions that overstate the current support frontier.

## Imported Assumption Review

| Imported assumption | Tracker disposition | Notes |
| --- | --- | --- |
| Keep `SKILL.md` canonical | **Bounded / not adopted literally** | Guild can target skill-compatible outputs, but runtime and contract truth remain Rust, manifests, WIT, and host-owned receipts. This is tracked under `#131`. |
| Add a friendlier Guild authoring layer | **Accepted with guardrails** | The idea is in scope, but only as an evaluated authoring layer that must not fork the contract surface. Tracked under `#131`. |
| Package curated starter packs now | **Accepted in a narrower form** | Current focus is one honest incident-casefile-first starter path, then bounded starter-pack progression. Tracked under `#130`, `#136`, and `#137`. |
| Differentiate on trust primitives | **Accepted** | This is already the repo direction and is tracked under `#132`, `#133`, `#138`, and `#134`. |
| Aim at ops / platform / security teams first | **Accepted** | This aligns with the current repo positioning and does not require a new issue by itself. |

## Milestone Crosswalk

| Imported milestone | Current tracker status | GitHub issues | Notes |
| --- | --- | --- | --- |
| **M1. Make Guild Legible** | **Largely completed** | Closed `#86`, `#87`, `#88`, `#89`, `#90`, `#91`, `#92`, plus closed task issues `#93` through `#120` | Imported M1 mostly overlaps work already landed in the repo and should stay closed unless a concrete repo gap reappears. |
| **M2. Make Guild Installable and Useful** | **Active** | Open `#130`, `#131`, `#136`, `#137`, `#139` | This is the current live phase, but the imported stack is narrowed to current proven install/export/read-only starter surfaces. |
| **M3. Make Guild Trustworthy and Differentiated** | **Planned after M2 gate** | Open `#132`, `#133`, `#138`, parts of `#134` | Imported trust work is accepted, but only after the M2 install/package/story path stays honest. |
| **M4. Make Guild Adoptable by Teams** | **Later-phase planning** | Open `#134` | Private distribution, governance, signing, audit, and adoption guidance stay later-phase. |

## Epic Crosswalk

| Imported epic | Tracker disposition | GitHub issues | Notes |
| --- | --- | --- | --- |
| **EPIC-01. Thesis and Narrative Freeze** | Completed overlap | Closed `#86`, with task follow-ons in closed `#93` to `#99` | Do not reopen unless new repo entrypoints drift. |
| **EPIC-02. Glossary and Capability Model v1** | Completed overlap | Closed `#87`, `#88`, with task follow-ons in closed `#97` to `#104` | Current repo already carries this legibility layer. |
| **EPIC-03. Friendly Authoring Schema and Validator** | Expanded overlap | Open `#131` | Imported ambition is accepted only as guarded evaluation; no second contract surface, no literal `guild/v1alpha1` rollout by default. |
| **EPIC-04. Packaging and Install Surface** | Direct overlap | Open `#136`, supported by `#130` | Imported packaging ideas are kept honest against current shipped bundle/export/import/trust flows. |
| **EPIC-05. Starter Packs and Reference Playbooks** | Split overlap | Open `#130`, `#137`, `#132` | Current runtime only supports a narrower incident-casefile-first starter plus docs-first/deferred reference playbook progression. |
| **EPIC-06. Receipt Chain, Replay, and Policy** | Split overlap | Open `#138`, `#132`, `#134` | Parts of the imported epic are already shipped today; the remaining replay/policy/redaction/governance work is split across these issues. |
| **EPIC-07. Verification Matrix and Verified Catalog** | Direct overlap | Open `#133` | Imported verification guidance maps cleanly to the current verification-matrix follow-on issue. |
| **EPIC-08. Private Registry and Governance** | Direct overlap | Open `#134` | Imported adoption/governance work stays later-phase and policy-heavy. |

## Task Crosswalk

Status legend used below:

- **Done**: already represented by closed GitHub issues or already shipped repo truth.
- **Absorbed**: covered by an open issue without needing a new standalone issue.
- **Split**: one imported task maps to more than one open issue.
- **Deferred**: intentionally not active until a later milestone gate.

| Task | Imported title | Status | GitHub tracker mapping | Notes |
| --- | --- | --- | --- | --- |
| `GR-001` | Rewrite the README opener around trusted playbooks | Done | Closed `#93` | Landed as part of M1 docs reset. |
| `GR-002` | Rewrite the site hero, subhead, and top-level value props | Done | Closed `#92` | Repo-side trust/site realignment work is already closed; reopen only if site code drifts again. |
| `GR-003` | Add a canonical narrative hierarchy doc and link it from contributor docs | Done | Closed `#95`, `#99` | Narrative hierarchy and contributor-facing guardrails were absorbed during M1. |
| `GR-004` | Add a copy-review checklist to the PR template or docs contribution guide | Done | Closed `#99`, `#65` | Repo templates now carry follow-on guardrails. |
| `GR-005` | Publish the glossary and banned-terms page | Done | Closed `#97` | Already landed. |
| `GR-006` | Define capability naming rules and approved verb set | Done | Closed `#88`, closed `#101` | Already landed in docs and capability vocabulary work. |
| `GR-007` | Add alias mapping guidance for first-party adapters | Done | Closed `#102` | Already landed. |
| `GR-008` | Add a terminology lint/check step for docs and marketing copy | Done | Closed `#98` plus repo doc regressions | Current repo uses focused docs regressions rather than a broad standalone linter. |
| `GR-009` | Draft the `guild/v1alpha1` spec for Skill, Playbook, and Pack | Absorbed | Open `#131` | Evaluate only after source-of-truth boundaries are explicit; do not assume adoption. |
| `GR-010` | Implement parsing for `guild.skill.yaml`, `guild.playbook.yaml`, and `guild-pack.yaml` | Deferred | Open `#131` | Parsing is downstream of the design decision in `#131`, not a default next step. |
| `GR-011` | Implement compiler output to standard `SKILL.md` bundles | Deferred | Open `#131` | Imported proposal conflicts with current contracts-first defaults unless deliberately approved later. |
| `GR-012` | Add a scaffolding command or templates for new skills, playbooks, and packs | Deferred | Open `#131` | Consider only if an authoring layer is accepted first. |
| `GR-013` | Add validator, golden tests, and migration docs from raw `SKILL.md` | Deferred | Open `#131` | Same dependency chain as the rest of EPIC-03. |
| `GR-014` | Define the pack bundle layout, versioning rules, and manifest semantics | Absorbed | Open `#136` | Must be reconciled against already-shipped bundle/export/import truth. |
| `GR-015` | Implement `guild pack build` | Deferred | Open `#136` | Naming and scope must be justified against current CLI and transport behavior first. |
| `GR-016` | Implement `guild pack export --target openai|copilot|local` | Deferred | Open `#136` | Export-target guidance is useful, but the CLI surface stays honest-first. |
| `GR-017` | Add install/import flow and pack lockfile | Absorbed | Open `#136` | Current install/import truth exists; this task becomes “clarify and extend current flows,” not start from zero. |
| `GR-018` | Publish a 5-minute first useful run guide | Split | Open `#130`, `#136` | The current honest version is the incident-casefile-first quickstart. |
| `GR-019` | Create the `incident-triage` starter pack | Split | Open `#130`, `#137` | Current bounded analogue is the Guild Ops Starter plus incident-casefile path. |
| `GR-020` | Create the `k8s-remediation` starter pack | Deferred | Open `#137` | Docs-first candidate only until apply-mode/runtime support grows. |
| `GR-021` | Create the `safe-change` starter pack | Deferred | Open `#137` | Same as above. |
| `GR-022` | Create the `secrets-and-edge` starter pack | Deferred | Open `#137` | Same as above. |
| `GR-023` | Author three reference playbooks for service restart, rollback, and cert validation | Deferred | Open `#137` | Useful conceptually, but not honest as runnable current product work. |
| `GR-024` | Author three reference playbooks for node remediation, cache purge, and secret rotation | Split | Open `#137`, `#132` | `cache purge with evidence trail` is the leading current mutation-demo candidate; the rest remain deferred. |
| `GR-025` | Add demo fixtures and example environments for all reference playbooks | Deferred | Open `#137` | Only after specific reference playbooks are classified as real versus docs-first. |
| `GR-026` | Publish walkthrough docs and terminal transcripts for first-party packs | Split | Open `#130`, `#137` | Current walkthrough work belongs to the starter path first. |
| `GR-027` | Define the receipt schema and run object model | Done / Absorbed | Open `#138` | Receipt and execution truth already exists in the repo; `#138` handles the remaining explanation/replay follow-on. |
| `GR-028` | Emit receipts during `guild admit` and `guild exec` | Done / Absorbed | Open `#138` | Durable execution/evidence records already exist; future admission/apply evolution stays later. |
| `GR-029` | Implement `guild inspect` | Done | Current shipped CLI | Already shipped in the current CLI surface. |
| `GR-030` | Implement `guild replay` | Deferred | Open `#138` | Replay execution is not honest current scope. |
| `GR-031` | Add approval policy model and production mutation gate | Split | Open `#132`, `#134` | Mutation gating belongs to the first honest mutation demo and later team/governance boundaries. |
| `GR-032` | Add evidence redaction and retention hooks for sensitive fields | Absorbed | Open `#134` | Governance/retention/redaction are tracked there. |
| `GR-033` | Define the verification matrix and compatibility report format | Absorbed | Open `#133` | Direct overlap. |
| `GR-034` | Implement `guild verify` with spec, install, dry-run, and eval checks | Absorbed | Open `#133` | Keep scope honest against current proof and trust signals. |
| `GR-035` | Implement eval runner and smoke scenarios for first-party packs | Absorbed | Open `#133` | Current repo already has smoke-oriented proof patterns; issue `#133` determines how they become matrix inputs. |
| `GR-036` | Generate curated / verified badges and publish verification reports | Absorbed | Open `#133` | Direct overlap. |
| `GR-037` | Implement private pack source configuration | Absorbed | Open `#134` | Later-phase team adoption surface. |
| `GR-038` | Add bundle signing and signature verification | Done / Absorbed | Open `#134` | Signed bundle verification largely exists today; later-phase governance and private-distribution implications live in `#134`. |
| `GR-039` | Add run history export and audit summary output | Absorbed | Open `#134` | Later-phase audit/governance follow-on. |
| `GR-040` | Add a governance guide and pilot onboarding checklist | Absorbed | Open `#134` | Direct overlap. |

## New Tracker Work Added During Import

The imported stack surfaced one new repo-level issue that did not fit cleanly in
the prior follow-on set:

- Open `#139`: reconcile the duplicate repositioning strategy docs into one
  canonical stack.

That issue exists because the imported stack added useful structure, but it now
coexists with earlier repositioning docs and duplicated epic files.

## Current Canonical Issue Set

Use this issue set for active follow-on planning:

- `#129` umbrella follow-on program and milestone sequencing
- `#130` current honest starter path
- `#131` bounded authoring-layer evaluation and guardrails
- `#132` first honest mutation demo
- `#133` verification matrix and labels
- `#134` private-pack, governance, retention, and team-adoption boundaries
- `#136` packaging and install-surface follow-on
- `#137` starter-pack and reference-playbook progression
- `#138` receipt-chain and replay-oriented explanation follow-on
- `#139` reconcile duplicate repositioning docs into one canonical stack

## What This Crosswalk Decides

- The imported strategy stack is now processed and mapped, not left as a second
  independent roadmap.
- M1 remains closed unless the repo drifts.
- M2 stays the active phase.
- M3 and M4 remain sequenced behind M2 rather than pulled forward by optimism.
- Imported ideas that conflict with Guild's contracts-first boundary are treated
  as bounded proposals, not automatic implementation commitments.
