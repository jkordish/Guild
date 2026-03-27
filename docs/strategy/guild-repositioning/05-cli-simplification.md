# CLI Simplification

## Goal

Tighten the Guild CLI story around the operator flow:

1. admit
2. exec
3. inspect
4. replay

This document defines the target UX. It does not require a hard CLI rename in the first wave.

## Current State

Current first-class commands are:

- `show`
- `grants`
- `run`
- `ls`
- `get`
- `why`
- `verify`
- install and transport commands

This is cleaner than earlier substrate-heavy wording, but it still exposes the mechanics of the current implementation rather than the operator journey.

## Target Command Set

| Target Command | Description | Current Closest Surface |
| --- | --- | --- |
| `guild admit` | Preview capability use, policy narrowing, and execution readiness before a run | no first-class equivalent today; partially described by docs and future `doctor/preview` direction |
| `guild exec` | Execute a playbook or skill | `guild run` |
| `guild inspect` | Inspect receipts, evidence, and execution history | `guild show`, `guild why`, `guild get`, `guild ls` |
| `guild replay` | Re-run or re-check from a stored receipt context | no first-class equivalent today |

## Recommended Migration Posture

- Keep current commands stable in the first implementation wave.
- Add aliases and doc mappings before any hard rename.
- Use the target verbs in strategy docs and roadmap language now.
- Avoid claiming that `admit` or `replay` already exist as shipped commands.

## Command Descriptions

### `guild admit`

- Review requested capabilities in operator language.
- Show policy narrowing, approval requirements, and isolation posture.
- Fail before execution when the requested playbook cannot be honestly admitted.

### `guild exec`

- Run the admitted skill or playbook.
- Preserve current receipt and evidence model.
- Keep execution output concise and operator-readable.

### `guild inspect`

- Unify the current show / why / get mental model under one inspectable surface.
- Keep receipt, evidence, and lineage navigation explicit.

### `guild replay`

- Start from a stored receipt.
- Re-run or re-check the automation in a bounded way.
- Make replay expectations explicit instead of implied.

## Examples

```bash
# target operator flow
guild admit playbooks/rollback-service.yaml --input-file rollback.json
guild exec playbooks/rollback-service.yaml --input-file rollback.json
guild inspect exec:abc123
guild replay exec:abc123
```

```bash
# current equivalent surfaces
guild grants template read-resource
guild run skill://example/incident-brief@^0.1 --input-json '{"execution_uri":"guild://executions/abc123"}'
guild why exec:abc123
guild get guild://executions/abc123
```

## Planned Deprecations And Aliases

| Current | Future Role |
| --- | --- |
| `run` | stable alias toward `exec` |
| `show` | inspect sub-surface or compatibility alias |
| `why` | inspect explanation sub-surface or compatibility alias |
| `get` | inspect raw-read sub-surface or compatibility alias |
| `grants template` | capability authoring helper, likely folded into `admit` and playbook tooling later |

## Migration Notes

- `verify` should remain a trust-specific command rather than being absorbed into `inspect`.
- `install`, `export`, `import`, `push`, `pull`, and `trust` remain important, but they should no longer dominate the top-level product story.
- The first CLI wave should be documentation and alias planning, not a disruptive command break.
- Any eventual rename must ship with tests, help updates, migration notes, and compatibility aliases.
