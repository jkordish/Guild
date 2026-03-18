# Diff Execution Authority

`diff-execution-authority` is an inspect-only example skill that compares two
stored Guild execution records and highlights the authority differences that
changed the outcome.

It focuses on operator questions like:

- did trust tier or verification state change?
- did policy select a different profile?
- which capability families were granted differently?
- did the termination reason change because authority changed?

Canonical local proof flow:

```bash
cargo run -p guild-mcp --example inspect_policy_local
```

That flow creates trusted and restricted imported executions of the same skill,
then runs `diff-execution-authority` over the two persisted execution URIs
through `guild.inspect`.
