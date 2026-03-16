# Inspect HTTP JSON

`inspect-http-json` is the canonical primitive proof skill for Guild's `http-request`
inspect capability family.

It:

- accepts a bounded absolute URL
- performs a host-mediated outbound HTTP request
- parses JSON deterministically
- returns a structured summary plus selected JSON Pointer fields

It does not expose ambient network access. Real authority still comes from the
host-owned execution grant passed to `guild.inspect`, and that grant is now
decided by local host policy rather than by caller intent alone.

Use the documented local proof flow in the repository README or:

```bash
cargo run -p guild-mcp --example inspect_http_json_local
```
