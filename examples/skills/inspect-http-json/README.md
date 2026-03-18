# Inspect HTTP JSON

`inspect-http-json` is the canonical primitive proof skill for Guild's `http-request`
inspect capability family.

It:

- accepts a bounded absolute URL
- performs a host-mediated outbound HTTP request
- inherits only the host-granted redirect and destination-class authority for
  that execution
- parses JSON deterministically
- returns a structured summary plus selected JSON Pointer fields

It does not expose ambient network access. Real authority still comes from the
host-owned execution grant passed to `guild.inspect`, and that grant is now
decided by local host policy rather than by caller intent alone.

Use the documented local proof flow in the repository README or:

```bash
cargo run -p guild-mcp --example inspect_http_json_local
```

That local proof flow uses an explicit loopback + IP-literal grant for the
deterministic local server and also shows a denied host-mismatch execution.

For the trust-tier-aware local policy proof flow that imports the same skill
under different host-owned trust tiers and profile selections, then explains
the persisted denial plus stored authority state through the new authority-debug
skills, use:

```bash
cargo run -p guild-mcp --example inspect_policy_local
```

That policy proof flow now demonstrates trusted imported redirect-following,
restricted-profile redirect denial after the local policy profile caps the
granted HTTP authority, and follow-up inspection through
`explain-execution`, `explain-capability-denial`, `diff-execution-authority`,
and `explain-http-authority`.
