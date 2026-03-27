# EPIC-05: CLI Tightening

## Title
Stage the Guild CLI toward `admit`, `exec`, `inspect`, and `replay`.

## Problem
Guild's current CLI is usable, but its verbs reflect the current substrate more than the desired operator journey. `run`, `show`, `get`, `why`, and `verify` are understandable in isolation, yet they do not present Guild as a system centered on admission, trusted execution, inspectability, and replay.

## Outcome
Guild has a documented, compatibility-preserving path toward a tighter operator command set.

## User Value
Operators can discover the product through verbs that match their workflow instead of reverse-engineering which low-level command maps to which intent.

## In Scope
- Target command-set definition
- Mapping from current verbs to target verbs
- Alias and deprecation strategy
- Example command lines for the operator journey
- Migration notes for docs and help text

## Out of Scope
- Breaking CLI changes in the first wave
- Replacing trust-specific subcommands without a migration path
- Implementing replay semantics that do not yet exist

## Deliverables
- CLI simplification doc
- Command mapping table
- Alias/deprecation plan
- Example flows for admit, exec, inspect, and replay

## Acceptance Criteria
- The target CLI set is documented with concrete user-facing examples.
- The plan preserves current command compatibility in the first wave.
- The docs make clear which verbs are target-state versus implemented today.
- Help-text follow-on work has clear migration rules.
- The plan does not imply replay or admission functionality that is not yet implemented.

## Dependencies
- `00-north-star.md`
- `02-glossary-and-banned-terms.md`
- current CLI help and `docs/command-language.md`

## Risks
- Confusing aspirational command names with current shipping commands
- Breaking existing users too early
- Accumulating aliases without a clean long-term simplification path

## Suggested Labels
- `epic`
- `cli`
- `docs`
- `ux`

## Priority
P1

## Sequencing Notes
CLI wording should follow the glossary and narrative reset. Any code changes should be staged behind aliases and doc updates before hard deprecations are considered.

## Starter Tasks
1. Publish the target command set and the compatibility-first migration plan.
2. Map current commands to target verbs in `docs/command-language.md`.
3. Identify help text that sounds prototype-ish or substrate-heavy.
4. Draft target examples for `guild admit`, `guild exec`, `guild inspect`, and `guild replay`.
5. Document which current commands remain canonical during the first migration phase.
6. Define the cutoff criteria for when aliases can become primary verbs.
