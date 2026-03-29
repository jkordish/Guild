# 05. CLI Simplification

**Status:** Proposed
**Owner:** CLI / DX
**Last updated:** 2026-03-28

## Goal

Give Guild a command surface that matches the product thesis and does not leak internal implementation nouns into the operator experience.

## Strong position

**The top-level CLI should optimize for three roles only:**

- author
- operator
- reviewer

If a command does not clearly serve one of those roles, it probably belongs behind a subcommand or should not exist.

## Target command surface

| Command | Role | Purpose |
| --- | --- | --- |
| `guild pack init` | author | scaffold a new pack |
| `guild pack build` | author | compile authoring files into distributable assets |
| `guild pack export` | author | export a pack to target formats / locations |
| `guild verify` | author / reviewer | validate and score a skill, pack, or playbook |
| `guild eval` | author / reviewer | run eval fixtures and smoke scenarios |
| `guild admit` | operator | perform preflight policy and approval checks |
| `guild exec` | operator | run a playbook |
| `guild inspect` | reviewer | inspect the receipt, evidence, and decisions from a run |
| `guild replay` | reviewer / operator | replay or reconstruct a prior execution |

## Commands that should stay out of the top level

Do not make these top-level concepts unless they prove essential:

- adapters
- graphs
- loaders
- manifests
- registries
- resolver internals

These can exist internally or under expert subcommands. They do not belong in the primary user story.

## Recommended operator flow

```bash
guild admit playbooks/restart-service-with-evidence --env prod --input service=payments-api
guild exec playbooks/restart-service-with-evidence --env prod --input service=payments-api
guild inspect runs/run-2026-03-28-001
guild replay runs/run-2026-03-28-001
```

## Recommended author flow

```bash
guild pack init incident-triage
guild pack build packs/incident-triage
guild verify packs/incident-triage
guild pack export packs/incident-triage --target openai
```

## Recommended reviewer flow

```bash
guild verify packs/incident-triage --report markdown
guild inspect runs/run-2026-03-28-001 --format summary
```

## Help text style

Help text should sound like this:

> Build, verify, run, inspect, and replay trusted playbooks.

It should not sound like this:

> Manage artifacts, registries, graphs, and execution adapters.

## Output expectations

### `guild admit`

Show:

- declared capabilities
- target environment
- policy result
- approvals required
- reasons for deny / require-approval outcomes

### `guild exec`

Show:

- playbook name
- environment
- major steps
- approval pauses
- receipt id on completion

### `guild inspect`

Show:

- run summary
- approvals
- capabilities used
- evidence summary
- mutation summary
- final outcome

### `guild replay`

Show:

- source receipt id
- replay mode
- environment / target overrides
- differences from original run

## Deprecation strategy

- Add aliases from current commands to the new surface.
- Update docs first.
- Print deprecation notices for one minor release.
- Remove old surface only after examples and quickstarts are migrated.

## Decision

**Use the CLI to reinforce the narrative.** The command surface should make Guild feel like a playbook tool with trust semantics, not a bag of engine internals.
