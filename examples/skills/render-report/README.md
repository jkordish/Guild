# Render Report

`render-report` is the zero-authority formatter child for the Guild Ops Starter
Pack.

It is intentionally narrow:

- no declared capabilities
- no dependencies
- deterministic inspect output only
- one job: turn a normalized report object into compact markdown

Direct smoke shape:

```bash
cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/render-report install examples/skills/render-report
cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/render-report run skill://example/render-report@^0.1 --input-json '{"title":"Incident Brief","summary_line":"failed  upper-bound  exec:abc123  example/inspect-http-json@0.1.0","facts":[{"label":"Status","value":"failed"},{"label":"Skill","value":"example/inspect-http-json@0.1.0"}],"sections":[{"title":"Primary reason","lines":["runtime-exec:http-method-not-allowed","POST was requested against a GET-only grant"]}]}'
```

The starter pack uses this child only where composition stays inside the current
honest single-child zero-authority invoke slice.
