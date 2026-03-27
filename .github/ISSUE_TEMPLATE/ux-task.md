---
name: UX Task
about: Implement one concrete operator-facing docs or CLI task for Guild
title: "[Task] "
labels: enhancement
---

## Problem

State the specific user-facing problem in one or two sentences.

## User/Persona

- Persona:
- Journey:
- Surface:

## Current Friction

Describe what the user sees today and why it is confusing or costly.

## Desired Behavior

Describe the intended result in concrete user-visible terms.

## Concrete Command/Output Examples

```text
# current
guild ...

# desired
guild ...
```

## Acceptance Criteria

- [ ] The change is observable from the intended user surface
- [ ] Help, docs, or examples are updated if the user-visible contract changes
- [ ] Core regression coverage or proof commands are identified
- [ ] The wording uses the approved Guild glossary where it fits

## Non-Goals

- Do not widen runtime authority or contract surface unless explicitly required
- Do not introduce aspirational command names or playbook/replay claims that the CLI does not support honestly

## Files/Specs Likely Touched

- `README.md`
- `docs/command-language.md`
- `crates/guild-mcp/src/cli.rs`

## Linked Epic

- Epic:

## Dependency Links

- Blocked by:
- Blocks:
