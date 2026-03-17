# ADR 0016: Log-write policy family

Status: Accepted  
Date: 2026-03-17

## Context

Guild's current inspect slice includes an explicit guest `log` host import, but
its semantics are intentionally small.

That smallness is a feature. Logging should not quietly turn into ambient host
output authority or a shadow telemetry subsystem.

## Decision

The `log-write` family authorizes explicit guest log emission through the Guild
host import only.

The current typed policy dimension is:

- `levels`

Today, `levels` are the only policy-relevant logging knob. The current
repository does not treat categories, namespaces, sinks, structured fields, or
message content as separate policy dimensions.

Current runtime behavior is intentionally narrow:

- the guest must call the host `log` import explicitly
- the host first checks `log-write` authorization
- if authorized, the current host implementation accepts the call and does not
  persist a separate durable log record in this milestone

Current denial behavior is host-owned and explicit:

- no `log-write` grant yields `log-write-not-granted`
- a denied severity yields `log-level-not-granted`

This family remains explicit rather than ambient because Guild wants the host to
be able to answer whether the guest was granted permission to emit logs at all.
That matters for least authority, reproducibility, and future sink design, even
though the current runtime does not yet persist or route logs to a durable log
store.

Safe defaults in the current repository are:

- no grant means no guest logging authority
- omitted `levels` on an existing `log-write` grant means any current severity
  level inside this family
- the family does not imply stdout, stderr, file writes, or a general console
  escape hatch

Nested child behavior is subset-only:

- child log levels are reduced from the parent grant and child requirement
- a child cannot widen from `info` to `warn` or `error` if the parent was more
  restrictive

## Consequences

Positive:

- the current inspect slice keeps logging honest and bounded
- future logging work has a clear least-authority starting point
- denial debugging for composite flows is straightforward

Costs and limits:

- Guild does not yet persist or expose guest log output as a durable artifact
- logging remains intentionally less expressive than a full telemetry system

## Explicit invariants

- logging is explicit, not ambient
- `log-write` does not imply filesystem, process, stdout, or stderr authority
- policy-relevant logging state is currently limited to severity levels
- denial of a log call remains host-owned
- child log authority cannot widen beyond the parent grant

## Explicit non-goals / deferred work

- durable log stores
- sink routing and subscriptions
- structured logging contracts
- log categories or namespaces as a policy dimension in this milestone
- treating logs as evidence automatically

## Cross-references

- `SPECS.md`
- `ARCHITECTURE.md`
- `docs/adr/0012-capability-policy-layering-model.md`
- `crates/guild-types/src/lib.rs`
- `crates/guild-runner/src/lib.rs`
- `crates/guild-runner/tests/inspect_slice.rs`
- `crates/guild-runner/tests/composition.rs`
- `examples/skills/hello-inspect/README.md`

