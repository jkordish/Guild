# Explain HTTP Authority

`explain-http-authority` is an inspect-only example skill that reads one stored
Guild execution record and dry-runs a candidate HTTP request against the stored
granted `http-request` authority.

Use `guild why` first for the normal compact persisted-execution explanation
path. Use `guild why -v` when you need the stored requested-versus-granted HTTP
authority diff, authority observations, and family-aware request hints. Use
`explain-http-authority` when you want a richer reusable report that tests a
candidate HTTP request against that same stored execution's grant.

It does not perform the request. Instead it:

- explains whether the candidate method, scheme, host, port, path, and timeout
  fit the stored grant
- reports redirect authority separately from request-shape matching
- stays honest about what it cannot know inside the Wasm guest

This means loopback and IP-literal cases are definitive, while ordinary domain
names can become `indeterminate` if host-side destination resolution would be
required to classify the target safely.

Canonical local proof flow:

```bash
cargo run -p guild-mcp --example inspect_policy_local
```

That flow runs `explain-http-authority` against persisted trusted and
restricted HTTP executions using deterministic loopback candidate URLs, so the
result is fully explained without live network access.
