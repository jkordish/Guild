# EPIC-02: Glossary And Language Simplification

## Title
Adopt a canonical operator-facing glossary and retire abstract lead terms.

## Problem
Guild's current language is accurate but too abstract for the intended audience. Terms like "artifact," "substrate," "reference application," and provenance-heavy trust phrasing create comprehension drag and make simple operator actions sound more academic than they need to be.

## Outcome
Guild uses a stable, operator-readable vocabulary across docs, examples, help text, roadmap material, and issue templates.

## User Value
Users can quickly map Guild concepts to familiar operational concerns such as capabilities, admission, isolation, evidence, and replay without translating internal product language.

## In Scope
- Canonical glossary
- Deprecated or discouraged term list
- Usage guidance with examples
- Language updates in docs and issue templates
- CLI/help text terminology review

## Out of Scope
- Renaming internal Rust types only for wording reasons
- Runtime-surface changes
- External capability implementation

## Deliverables
- Canonical glossary doc
- Discouraged term inventory with rationale
- Usage examples for preferred phrasing
- Terminology update checklist for docs and CLI surfaces

## Acceptance Criteria
- The repo has one canonical glossary for Guild-facing terms.
- Docs and planning artifacts use the preferred terms consistently.
- Discouraged lead terms are either removed or explicitly demoted.
- CLI help text and examples avoid prototype-ish or substrate-heavy wording where no technical precision is lost.
- The glossary distinguishes operator-facing language from mechanism-layer language rather than pretending they are identical.

## Dependencies
- `00-north-star.md`
- `01-messaging-audit.md`

## Risks
- Replacing precise technical words with softer but less accurate language
- Inconsistent adoption across docs and CLI help
- Regressing to new jargon that is just different, not clearer

## Suggested Labels
- `epic`
- `docs`
- `ux-copy`
- `cli`

## Priority
P0

## Sequencing Notes
Start immediately after the north-star is approved. This epic should complete before broad doc rewrites and before finalizing CLI simplification copy.

## Child Task Files

1. [TASK-05: Publish glossary entrypoint](../tasks/TASK-05-publish-glossary-entrypoint.md)
2. [TASK-06: Top-level discouraged-terms sweep](../tasks/TASK-06-top-level-discouraged-terms-sweep.md)
3. [TASK-07: Issue-template language update](../tasks/TASK-07-issue-template-language-update.md)
4. [TASK-08: CLI help terminology review](../tasks/TASK-08-cli-help-terminology-review.md)
