# Migration Notes

These notes define how the repositioning work should move across current Guild surfaces without overstating implementation changes.

## Narrative Migration

- `README.md`, `docs/how-guild-works.md`, and `docs/project-positioning.md` should move together or be explicitly staged so they do not present competing theses.
- If `docs/project-positioning.md` remains temporarily old-style, it must be clearly marked as historical framing until the guardrail is updated.
- Do not land a new README story that the `project-positioning` check will reject.

## Guardrail Migration

- `crates/guild-draft-truth/src/project_positioning.rs` is part of the user-facing thesis enforcement path.
- Any task that changes the top-level story must either update the guardrail in the same PR or be sequenced after a guardrail-only task.
- Keep the guardrail narrow and wording-focused; do not turn it into a broad prose linter.

## Glossary Migration

- Preferred terms such as trusted operational automation, ops playbook, capability, admission, isolation, receipts, evidence, replay, and inspectability should become the default user-facing language.
- Discouraged terms like artifact, substrate, and reference application should not lead the story unless the sentence is truly about the mechanism layer.
- Mechanism-layer terms may remain where they are necessary to describe the current contract or runtime.

## Capability Migration

- External capability names such as `k8s:restart` and `logs:query` are a docs/UX layer first.
- The internal capability families remain authoritative for current runtime, policy, and contract behavior until explicitly changed in a separate implementation effort.
- Mapping guidance must always show how the operator-facing name relates to the current internal family or host-mediated boundary.

## CLI Migration

- The migration posture is aliases first.
- `run`, `show`, `get`, `why`, and `verify` remain honest current surfaces until replacement commands exist.
- `admit` and `replay` are target-state operator verbs and must be described as such unless implemented.
- CLI docs and help previews may teach the future flow, but they must not imply unsupported binary behavior.

## Playbook Migration

- Playbooks are the target public automation surface, but Guild should not be described as already shipping a broad workflow DSL.
- Playbook docs may define a minimum schema shape and example YAML as a planning target.
- Example surfaces should explain how playbooks compose existing skills and capabilities rather than pretending the skill layer disappeared.

## Example Migration

- `examples/README.md` and `examples/skills/guild-ops-starter/README.md` should shift from substrate demo framing to operator workflow framing.
- Example copy must stay tied to current support claims and current proof commands.
- If executable example scope becomes too large, land docs-first playbook references before adding or changing runnable examples.

## External Site Migration

- There is no in-repo website source to update today.
- Repo-local launch copy can still be prepared for the README, docs, and release notes.
- Any external website rollout should be tracked as a follow-on outside this repository unless ownership and source location become explicit.
