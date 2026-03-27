# TASK-10: Map External Capabilities To Current Families

## Problem

Operator-readable capability names are only useful if contributors can see how they map to today's actual internal capability families and trust boundaries.

## User/Persona

- Persona: Maintainers, reviewers, and advanced operators
- Journey: Translating the new public vocabulary into current Guild implementation truth
- Surface: capability docs and guardrail docs

## Current Friction

The current docs do not yet connect names like `k8s:restart` to the actual executable frontier rooted in `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`.

## Desired Behavior

The capability docs should show how the external names map to current families, constraints, and host-mediated boundaries without implying that the implementation has already been renamed.

## Concrete Command/Output Examples

```text
# desired
k8s:restart -> bounded host-mediated action expressed today through documented internal capability families and policy constraints
```

## Acceptance Criteria

- [ ] The mapping doc names the current internal families explicitly.
- [ ] The mapping shows where capabilities are docs-only targets versus live runtime surfaces.
- [ ] The task preserves the repo's fail-closed posture in its wording.

## Non-Goals

- Do not redesign the capability evaluator.
- Do not add new capability families solely to satisfy the docs.

## Repo-Grounded Surfaces

- `docs/strategy/guild-repositioning/03-capability-taxonomy-v1.md`
- `README.md`
- `SPECS.md`
- `ARCHITECTURE.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p guild-mcp --bin guild -- help grants
cargo run -q -p guild-mcp --bin guild -- grants template http-request
cargo run -q -p guild-mcp --bin guild -- grants template read-resource
cargo run -q -p guild-mcp --bin guild -- grants template invoke-skill
```

## Migration Notes

- Treat this mapping as explanatory, not normative.
- If a mapping cannot be explained honestly with current surfaces, mark it deferred instead of inventing precision.

## Risks / Fallback

- Risk: docs imply capabilities that are not truly available today.
- Fallback: mark unsupported or future-only areas as deferred and keep the mapping limited to the proven frontier.

## Suggested Owner

`runtime`

## Size

`M`

## Suggested Labels

- `enhancement`
- `docs`
- `runtime`

## Linked Epic

- [EPIC-03: Capability Model V1](../epics/EPIC-03-capability-model-v1.md)

## Dependency Links

- Blocked by: [TASK-09](./TASK-09-publish-capability-taxonomy-v1.md)
- Blocks: [TASK-11](./TASK-11-update-capability-examples-in-docs.md), [TASK-12](./TASK-12-prototype-human-readable-capability-rendering.md)
