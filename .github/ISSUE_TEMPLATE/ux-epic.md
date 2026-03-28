---
name: UX Epic
about: Track one trusted operational automation docs or CLI epic in the Guild follow-on program
title: "[Epic] "
labels: enhancement
---

## Summary

Describe the operator-facing trust, docs, or CLI problem this epic solves in
one short paragraph.

## Persona And Journey

- Persona:
- Journey:
- Surface:

## Current Friction

Describe what is confusing, slow, risky, or hard to discover today.

## Desired Behavior

Describe the user-visible outcome we want when this epic is complete.

## Success Criteria

- [ ] A user can complete the target journey without spec archaeology
- [ ] The command/doc/resource language matches the real Guild behavior
- [ ] Failure paths are actionable instead of merely correct
- [ ] The wording uses the approved Guild glossary where it fits

## Acceptance Criteria

- [ ] Acceptance criteria are explicit and testable
- [ ] Out-of-scope behavior is called out
- [ ] Any contract-sensitive docs or examples that must move are identified
- [ ] Repo-grounded surfaces and validation commands are named when they matter

## Non-Goals

- No runtime-contract widening unless a separate contract issue says so
- No aspirational command names that the CLI does not already support honestly
- No repo-local planning file that duplicates the active GitHub issue tree
- No playbook, replay, or capability claims that overstate the current support frontier

## Likely Files Or Surfaces

- `README.md`
- `docs/command-language.md`
- `docs/testing.md`
- `crates/guild-mcp/src/cli.rs`

## Child Tasks

- [ ] Add child task issues here after triage

## Dependency Links

- Blocked by:
- Blocks:
