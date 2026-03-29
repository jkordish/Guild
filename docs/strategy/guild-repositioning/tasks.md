# Guild Repositioning Backlog

**Status:** Proposed
**Last updated:** 2026-03-28

This is a PR-sized backlog ordered roughly by dependency. Tasks are intentionally shaped so they can become GitHub issues with minimal rewriting.

## Legend

- **Size S** - small PR, usually one focused change
- **Size M** - moderate PR, still bounded
- **Size L** - larger but still should be broken over a short branch window

---

## EPIC-01 · Thesis and Narrative Freeze

### GR-001 Rewrite the README opener around trusted playbooks
- **Milestone:** M1
- **Size:** S
- **Labels:** `docs`, `positioning`
- **Touchpoints:** `README.md`
- **Dependencies:** none
- **Acceptance criteria:**
  - README first screen uses the approved product definition.
  - Includes one concrete operational example.
  - Uses canonical nouns and removes discouraged terms from the opening section.

### GR-002 Rewrite the site hero, subhead, and top-level value props
- **Milestone:** M1
- **Size:** M
- **Labels:** `site`, `positioning`
- **Touchpoints:** landing page content, homepage components, site metadata
- **Dependencies:** GR-001
- **Acceptance criteria:**
  - Hero leads with trusted operational playbooks.
  - Trust claims appear above the fold.
  - CTA path points to a reference playbook or receipt, not generic docs.

### GR-003 Add a canonical narrative hierarchy doc and link it from contributor docs
- **Milestone:** M1
- **Size:** S
- **Labels:** `docs`, `contributors`
- **Touchpoints:** docs index, contributor docs
- **Dependencies:** GR-001
- **Acceptance criteria:**
  - Narrative hierarchy is documented in one place.
  - Contributor docs link to it.
  - New authors can tell which nouns are headline nouns versus supporting nouns.

### GR-004 Add a copy-review checklist to the PR template or docs contribution guide
- **Milestone:** M1
- **Size:** S
- **Labels:** `process`, `docs`
- **Touchpoints:** PR template, contribution guide
- **Dependencies:** GR-003
- **Acceptance criteria:**
  - New docs/site PRs have a narrative checklist.
  - Checklist includes audience, outcome, trust, and example requirements.
  - Checklist references the glossary.

---

## EPIC-02 · Glossary and Capability Model v1

### GR-005 Publish the glossary and banned-terms page
- **Milestone:** M1
- **Size:** S
- **Labels:** `docs`, `terminology`
- **Touchpoints:** docs navigation, glossary page
- **Dependencies:** GR-001
- **Acceptance criteria:**
  - Canonical nouns are documented.
  - Discouraged terms include preferred replacements.
  - Page is linked from contributor-facing docs.

### GR-006 Define capability naming rules and approved verb set in code or docs constants
- **Milestone:** M1
- **Size:** S
- **Labels:** `schema`, `policy`
- **Touchpoints:** docs, constants, parser stubs if present
- **Dependencies:** GR-005
- **Acceptance criteria:**
  - Capability grammar is explicit.
  - Approved verbs are enumerated.
  - Examples show domain + verb naming clearly.

### GR-007 Add alias mapping guidance for first-party adapters
- **Milestone:** M1
- **Size:** M
- **Labels:** `docs`, `adapters`, `policy`
- **Touchpoints:** adapter docs, capability docs
- **Dependencies:** GR-006
- **Acceptance criteria:**
  - At least one adapter-to-capability mapping table exists.
  - User-facing capabilities remain tool-agnostic.
  - Guidance explains where tool-specific detail belongs.

### GR-008 Add a terminology lint/check step for docs and marketing copy
- **Milestone:** M1
- **Size:** M
- **Labels:** `ci`, `docs`, `quality`
- **Touchpoints:** CI, scripts, docs checks
- **Dependencies:** GR-005
- **Acceptance criteria:**
  - PRs can fail on banned headline terms or mismatched canonical nouns.
  - The rules are documented and override-friendly.
  - The initial ruleset covers at least the top discouraged terms.

---

## EPIC-03 · Friendly Authoring Schema and Validator

### GR-009 Draft the `guild/v1alpha1` spec for Skill, Playbook, and Pack
- **Milestone:** M2
- **Size:** M
- **Labels:** `schema`, `design`
- **Touchpoints:** schema docs, examples
- **Dependencies:** GR-006
- **Acceptance criteria:**
  - Spec defines required and optional fields for all three kinds.
  - Capabilities, approvals, evidence, and evals are first-class fields.
  - Versioning and migration posture are documented.

### GR-010 Implement parsing for `guild.skill.yaml`, `guild.playbook.yaml`, and `guild-pack.yaml`
- **Milestone:** M2
- **Size:** M
- **Labels:** `schema`, `implementation`
- **Touchpoints:** parser package / module, fixtures
- **Dependencies:** GR-009
- **Acceptance criteria:**
  - Parser can load all three document kinds.
  - Validation errors point to actionable fields.
  - Example fixtures cover happy path and invalid path.

### GR-011 Implement compiler output to standard `SKILL.md` bundles
- **Milestone:** M2
- **Size:** L
- **Labels:** `compiler`, `skills`
- **Touchpoints:** compiler module, dist output, fixtures
- **Dependencies:** GR-010
- **Acceptance criteria:**
  - Authoring files compile to deterministic `SKILL.md` outputs.
  - Generated bundles include referenced resources.
  - Generated outputs do not require hand-editing to be usable.

### GR-012 Add a scaffolding command or templates for new skills, playbooks, and packs
- **Milestone:** M2
- **Size:** M
- **Labels:** `dx`, `cli`, `templates`
- **Touchpoints:** CLI, template files
- **Dependencies:** GR-010
- **Acceptance criteria:**
  - Users can scaffold a new pack and at least one skill/playbook pair.
  - Templates include capabilities, evidence, and eval placeholders.
  - Generated files follow the approved naming conventions.

### GR-013 Add validator, golden tests, and migration docs from raw `SKILL.md`
- **Milestone:** M2
- **Size:** M
- **Labels:** `validation`, `tests`, `docs`
- **Touchpoints:** validator, tests, migration guide
- **Dependencies:** GR-011
- **Acceptance criteria:**
  - Validator catches missing required fields and invalid capability names.
  - Golden tests protect compiler output.
  - Docs explain how existing raw skills can coexist or migrate.

---

## EPIC-04 · Packaging and Install Surface

### GR-014 Define the pack bundle layout, versioning rules, and manifest semantics
- **Milestone:** M2
- **Size:** M
- **Labels:** `packaging`, `design`
- **Touchpoints:** packaging docs, manifest spec
- **Dependencies:** GR-009
- **Acceptance criteria:**
  - Bundle layout is documented.
  - Versioning rules are explicit.
  - Manifest includes compatibility and verification metadata.

### GR-015 Implement `guild pack build`
- **Milestone:** M2
- **Size:** M
- **Labels:** `cli`, `packaging`
- **Touchpoints:** CLI, pack build pipeline
- **Dependencies:** GR-011, GR-014
- **Acceptance criteria:**
  - Command builds a pack from authoring files.
  - Output is deterministic and directory-structured.
  - Errors clearly identify the failing source file.

### GR-016 Implement `guild pack export --target openai|copilot|local`
- **Milestone:** M2
- **Size:** M
- **Labels:** `cli`, `export`, `compatibility`
- **Touchpoints:** export pipeline, target adapters
- **Dependencies:** GR-015
- **Acceptance criteria:**
  - At least three export targets are supported.
  - Export output matches each target’s expected layout.
  - Export reports what was emitted and where.

### GR-017 Add install/import flow and pack lockfile
- **Milestone:** M2
- **Size:** M
- **Labels:** `packaging`, `install`
- **Touchpoints:** install/import command, lockfile format
- **Dependencies:** GR-016
- **Acceptance criteria:**
  - Packs can be imported or installed through one clear path.
  - Lockfile captures source, version, and checksums.
  - Repeated install is stable and idempotent.

### GR-018 Publish a 5-minute first useful run guide
- **Milestone:** M2
- **Size:** S
- **Labels:** `docs`, `quickstart`
- **Touchpoints:** quickstart docs, README links
- **Dependencies:** GR-017
- **Acceptance criteria:**
  - Guide starts from a clean environment.
  - Guide ends with a real reference playbook run.
  - Guide includes expected output and common failure notes.

---

## EPIC-05 · Starter Packs and Reference Playbooks

### GR-019 Create the `incident-triage` starter pack
- **Milestone:** M2
- **Size:** M
- **Labels:** `pack`, `playbooks`, `ops`
- **Touchpoints:** pack manifest, skills, playbook docs
- **Dependencies:** GR-015
- **Acceptance criteria:**
  - Pack builds and exports cleanly.
  - Pack includes observation plus bounded mutation.
  - Pack docs show at least one real incident workflow.

### GR-020 Create the `k8s-remediation` starter pack
- **Milestone:** M2
- **Size:** M
- **Labels:** `pack`, `kubernetes`, `ops`
- **Touchpoints:** pack manifest, k8s skills, fixtures
- **Dependencies:** GR-015
- **Acceptance criteria:**
  - Pack includes read plus remediation capabilities.
  - Production mutation paths declare approvals.
  - Fixtures cover at least one node or workload remediation flow.

### GR-021 Create the `safe-change` starter pack
- **Milestone:** M2
- **Size:** M
- **Labels:** `pack`, `deploy`, `change-management`
- **Touchpoints:** pack manifest, rollback / change skills
- **Dependencies:** GR-015
- **Acceptance criteria:**
  - Pack focuses on bounded change execution.
  - Rollback and verification are both represented.
  - Incident or change annotation is part of the workflow.

### GR-022 Create the `secrets-and-edge` starter pack
- **Milestone:** M2
- **Size:** M
- **Labels:** `pack`, `security`, `edge`
- **Touchpoints:** pack manifest, secret / cache / dns skills
- **Dependencies:** GR-015
- **Acceptance criteria:**
  - Pack covers at least one secret rotation and one edge action.
  - High-risk operations declare stricter approvals.
  - Evidence expectations are explicit.

### GR-023 Author three reference playbooks for service restart, rollback, and cert validation
- **Milestone:** M2
- **Size:** L
- **Labels:** `playbooks`, `docs`
- **Touchpoints:** playbook specs, fixtures, docs
- **Dependencies:** GR-019, GR-021, GR-022
- **Acceptance criteria:**
  - All three playbooks declare capabilities and evidence contracts.
  - All three have at least one happy-path fixture.
  - All three build and export via the standard pack flow.

### GR-024 Author three reference playbooks for node remediation, cache purge, and secret rotation
- **Milestone:** M2
- **Size:** L
- **Labels:** `playbooks`, `security`, `kubernetes`
- **Touchpoints:** playbook specs, fixtures, docs
- **Dependencies:** GR-020, GR-022
- **Acceptance criteria:**
  - All three playbooks declare approvals and evidence contracts.
  - Unsafe or denied paths are documented.
  - All three run under the same packaging model as the others.

### GR-025 Add demo fixtures and example environments for all reference playbooks
- **Milestone:** M2
- **Size:** M
- **Labels:** `demo`, `fixtures`, `tests`
- **Touchpoints:** fixture directories, demo docs
- **Dependencies:** GR-023, GR-024
- **Acceptance criteria:**
  - Every reference playbook has fixtures.
  - Fixtures are reproducible and versioned.
  - Demo setup does not require undocumented manual steps.

### GR-026 Publish walkthrough docs and terminal transcripts for first-party packs
- **Milestone:** M2
- **Size:** M
- **Labels:** `docs`, `examples`
- **Touchpoints:** docs pages, screenshots or transcripts
- **Dependencies:** GR-025
- **Acceptance criteria:**
  - Each pack has a short walkthrough.
  - Walkthroughs show capabilities, approval, mutation, and receipt summary.
  - Docs link back to quickstart and verification concepts.

---

## EPIC-06 · Receipt Chain, Replay, and Policy

### GR-027 Define the receipt schema and run object model
- **Milestone:** M3
- **Size:** M
- **Labels:** `receipts`, `design`
- **Touchpoints:** schema docs, runtime model
- **Dependencies:** GR-023, GR-024
- **Acceptance criteria:**
  - Receipt schema includes intent, capabilities, approvals, evidence, mutations, and outcome.
  - Redaction fields are accounted for.
  - Schema is documented with examples.

### GR-028 Emit receipts during `guild admit` and `guild exec`
- **Milestone:** M3
- **Size:** L
- **Labels:** `runtime`, `receipts`
- **Touchpoints:** runtime, storage, CLI output
- **Dependencies:** GR-027
- **Acceptance criteria:**
  - Preflight and execution produce structured records.
  - Receipt ids are surfaced clearly to users.
  - Missing or partial evidence is represented explicitly, not silently dropped.

### GR-029 Implement `guild inspect`
- **Milestone:** M3
- **Size:** M
- **Labels:** `cli`, `receipts`
- **Touchpoints:** CLI, formatting, receipt reader
- **Dependencies:** GR-028
- **Acceptance criteria:**
  - Command can summarize a receipt clearly.
  - Output includes approvals, capabilities, evidence, and mutations.
  - Output supports a compact and a detailed view.

### GR-030 Implement `guild replay`
- **Milestone:** M3
- **Size:** L
- **Labels:** `cli`, `runtime`, `replay`
- **Touchpoints:** CLI, runtime, receipt reader
- **Dependencies:** GR-028
- **Acceptance criteria:**
  - Replay can re-run or reconstruct a reference playbook from a receipt.
  - Replay output surfaces differences from the original run.
  - Replay behavior is documented for dry-run versus live modes.

### GR-031 Add approval policy model and production mutation gate
- **Milestone:** M3
- **Size:** M
- **Labels:** `policy`, `runtime`, `security`
- **Touchpoints:** policy config, runtime gate, docs
- **Dependencies:** GR-006, GR-027
- **Acceptance criteria:**
  - Mutating production actions can require approval by policy.
  - Denied actions explain why they were denied.
  - Policy examples use external capability names.

### GR-032 Add evidence redaction and retention hooks for sensitive fields
- **Milestone:** M3
- **Size:** M
- **Labels:** `security`, `governance`, `receipts`
- **Touchpoints:** receipt writer, config, docs
- **Dependencies:** GR-027
- **Acceptance criteria:**
  - Sensitive fields can be redacted in receipts.
  - Retention class or policy can be declared.
  - Docs explain the default behavior and limits.

---

## EPIC-07 · Verification Matrix and Verified Catalog

### GR-033 Define the verification matrix and compatibility report format
- **Milestone:** M3
- **Size:** M
- **Labels:** `verification`, `design`
- **Touchpoints:** docs, report schema
- **Dependencies:** GR-014, GR-027
- **Acceptance criteria:**
  - Report format includes spec validity, install validity, compatibility, dry-run, and eval coverage.
  - Trust labels are defined clearly.
  - First-party packs can be scored with the format.

### GR-034 Implement `guild verify` with spec, install, dry-run, and eval checks
- **Milestone:** M3
- **Size:** L
- **Labels:** `cli`, `verification`
- **Touchpoints:** CLI, verification runner
- **Dependencies:** GR-013, GR-017, GR-033
- **Acceptance criteria:**
  - Command runs more than syntax checks.
  - Report identifies failed dimensions clearly.
  - Verification output can be emitted as markdown and machine-readable data.

### GR-035 Implement eval runner and smoke scenarios for first-party packs
- **Milestone:** M3
- **Size:** M
- **Labels:** `evals`, `tests`, `verification`
- **Touchpoints:** eval fixtures, runner, CI
- **Dependencies:** GR-023, GR-024, GR-034
- **Acceptance criteria:**
  - Each first-party pack has at least one happy-path and one fail-safe eval.
  - Evals run in CI or a documented local flow.
  - Verification reports include eval results.

### GR-036 Generate curated / verified badges and publish verification reports for first-party packs
- **Milestone:** M3
- **Size:** M
- **Labels:** `verification`, `docs`, `catalog`
- **Touchpoints:** docs site, generated reports, pack metadata
- **Dependencies:** GR-034, GR-035
- **Acceptance criteria:**
  - Badge meaning is documented.
  - First-party packs publish a report alongside the badge.
  - Packs cannot claim `verified` without passing the required checks.

---

## EPIC-08 · Private Registry and Governance

### GR-037 Implement private pack source configuration
- **Milestone:** M4
- **Size:** M
- **Labels:** `distribution`, `private-registry`
- **Touchpoints:** config, CLI, docs
- **Dependencies:** GR-017, GR-036
- **Acceptance criteria:**
  - Guild can read packs from a private source.
  - Source configuration is documented.
  - Verification semantics are preserved for private packs.

### GR-038 Add bundle signing and signature verification
- **Milestone:** M4
- **Size:** M
- **Labels:** `security`, `packaging`, `provenance`
- **Touchpoints:** build/export pipeline, verify flow, docs
- **Dependencies:** GR-015, GR-034
- **Acceptance criteria:**
  - Packs can be signed at build/export time.
  - Signature verification is part of install / verify behavior.
  - Docs explain what signing proves and what it does not prove.

### GR-039 Add run history export and audit summary output
- **Milestone:** M4
- **Size:** M
- **Labels:** `audit`, `governance`, `receipts`
- **Touchpoints:** CLI, export format, docs
- **Dependencies:** GR-028, GR-029
- **Acceptance criteria:**
  - Run history can be exported in a documented format.
  - Audit summary is readable by non-authors.
  - Sensitive fields honor redaction policy.

### GR-040 Add a governance guide and pilot onboarding checklist
- **Milestone:** M4
- **Size:** S
- **Labels:** `docs`, `governance`, `adoption`
- **Touchpoints:** docs, pilot guide
- **Dependencies:** GR-037, GR-038, GR-039
- **Acceptance criteria:**
  - Guide explains approvals, trust labels, signing, and retention.
  - Pilot checklist tells a team what to review before adoption.
  - Guide links to pack verification and receipt inspection docs.
