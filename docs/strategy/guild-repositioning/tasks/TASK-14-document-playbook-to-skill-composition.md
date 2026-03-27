# TASK-14: Document Playbook-To-Skill Composition

## Problem

Guild already has skills, not a first-class playbook engine. The repositioning work needs one explicit task that explains how playbooks compose the existing skill model instead of hand-waving over it.

## User/Persona

- Persona: Maintainers and advanced operators
- Journey: Reconciling the new playbook story with the current execution model
- Surface: playbook docs and technical guidance

## Current Friction

Without an explicit composition explanation, readers may assume playbooks replace skills or that Guild already ships a broader orchestration runtime than it does.

## Desired Behavior

The docs should explain clearly that playbooks are the public automation surface and skills remain the reusable execution units underneath.

## Concrete Command/Output Examples

```text
# desired
playbook step -> existing skill invocation -> bounded capabilities -> receipts/evidence
```

## Acceptance Criteria

- [ ] The relationship between playbooks and portable skills is explicit.
- [ ] The doc points to current Guild execution boundaries rather than abstract future behavior.
- [ ] The change reduces the risk of workflow-engine overreach in later tasks.

## Non-Goals

- Do not add a new manifest format in this task.
- Do not change the live execution semantics.

## Repo-Grounded Surfaces

- `docs/strategy/guild-repositioning/04-playbook-surface-v1.md`
- `README.md`
- `SPECS.md`
- `ARCHITECTURE.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
```

## Migration Notes

- Keep the explanation concrete and tied to current Guild terms like skills, capabilities, receipts, and evidence.
- If the composition story needs an example, use one current example rather than a speculative one.

## Risks / Fallback

- Risk: the composition explanation becomes too architecture-heavy.
- Fallback: keep one short public explanation and move deeper mechanics into a follow-on note.

## Suggested Owner

`runtime`

## Size

`M`

## Suggested Labels

- `enhancement`
- `docs`
- `runtime`

## Linked Epic

- [EPIC-04: Playbook Surface V1](../epics/EPIC-04-playbook-surface-v1.md)

## Dependency Links

- Blocked by: [TASK-13](./TASK-13-publish-playbook-v1-surface.md)
- Blocks: [TASK-15](./TASK-15-add-playbook-concept-entrypoint.md), [TASK-16](./TASK-16-draft-manifest-to-playbook-translation-note.md)
