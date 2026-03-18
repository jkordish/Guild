# Explain Capability Denial

`explain-capability-denial` is an inspect-only example skill that reads one
stored Guild execution record through `read-resource` and explains how
requested capability intent turned into granted, reduced, or denied authority.

It is meant for operator debugging, not demos:

- it reads durable host-owned execution truth instead of inferring from guest output
- it makes requested vs granted capability state explicit
- it surfaces local policy profile, trust tier, verification state, and reason codes
- it keeps the output structured and bounded

Canonical local proof flow:

```bash
cargo run -p guild-mcp --example inspect_policy_local
```

That flow creates a trusted imported HTTP execution and a restricted imported
HTTP denial, then runs `explain-capability-denial` against the persisted denied
execution URI through the same `guild.inspect` path operators would use.
