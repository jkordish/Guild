# Invoke Child Zero

Inspect-only deterministic child skill used by the bounded `invoke-skill` live-proof slice.

This child is intentionally boring:

- no declared capabilities
- no dependencies
- deterministic inspect output only
- fixed `guild-skill-inspect-v1` world

It is paired with `examples/skills/invoke-parent-single-child` so the current
M8c invoke proof can stay honest. The older `hello-inspect` child fixture is a
real runtime example, but it exercises `emit-evidence` and therefore sits
outside the current proof-backed invoke envelope.
