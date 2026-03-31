# North Star

Guild is becoming a trusted session substrate for isolated harness execution:
an admission controller, session broker, and receipt engine that lets callers
target durable sessions instead of ephemeral sandboxes.

## Design Principles

- Session is the product abstraction. Users should think in terms of durable
  sessions, not runtime instances.
- Harness is first-class. Isolation, tools, and capability boundaries belong to
  a named execution abstraction, not to vague runtime plumbing.
- Sandbox lifecycle is internal. Warm resume, rehydrate, and cold start are
  host decisions, not user-facing primitives.
- Receipts are host truth. Claims about what happened must resolve to durable
  host-owned records, evidence refs, and provenance.
- Admission stays explicit. Capabilities, secrets, mounts, network policy, and
  isolation choices remain host-mediated and policy-gated.

## Anti-Goals

- Not a generic agent operating system.
- Not a broad workflow engine that hides trust boundaries.
- Not a promise that arbitrary session resume or snapshot restore already
  exists.
- Not a pivot away from capability-gated execution, portable packaging, or
  durable evidence.
- Not a public commitment to sandbox internals as the product surface.
