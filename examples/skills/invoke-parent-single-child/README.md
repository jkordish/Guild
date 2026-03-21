# Invoke Parent Single Child

Inspect-only deterministic parent skill used by the bounded single-child `invoke-skill` live-proof slice.

This fixture is intentionally narrower than the general composite examples.
It exists to prove one honest M8c slice only:

- exactly one declared alias, `child`
- exact child identity through the installed dependency snapshot
- fixed child runtime world, `guild-skill-inspect-v1`
- deterministic child input
- zero child-side authority use
- zero nested child executions

It is paired with `examples/skills/invoke-child-zero` and drives the checked
`runtime-invoke-skill.*` draft-v1 examples plus the live runner scenarios.

The default fixture input exercises the supported slice once. The optional
`invoke_twice: true` input exists only to prove the fail-closed boundary for
multi-child execution.
