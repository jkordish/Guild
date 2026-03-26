# Invoke Parent Single Child

Inspect-only deterministic parent skill used by the bounded single-child and exact two-child same-alias `invoke-skill` live-proof slices.

This fixture is intentionally narrower than the general composite examples.
It exists to prove the current honest M8c zero-authority invoke slices only:

- exactly one declared alias, `child`
- exact child identity through the installed dependency snapshot
- fixed child runtime world, `guild-skill-inspect-v1`
- deterministic child input
- zero child-side authority use
- zero nested child executions
- optional exact two-child same-alias fan-out in deterministic order

It is paired with `examples/skills/invoke-child-zero` and drives the checked
`runtime-invoke-skill.*` draft-v1 examples plus the live runner scenarios.

The default fixture input exercises the bounded single-child slice once. The
optional `invoke_twice: true` input exercises the bounded exact two-child
same-alias slice. Broader fan-out, child authority, recursion, and non-inspect
children remain outside this fixture's supported envelope.
