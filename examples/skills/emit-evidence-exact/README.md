# Emit Evidence Exact

Inspect-only deterministic fixture for the exact proof-backed `emit-evidence`
slice.

This skill is intentionally narrower than `hello-inspect`:

- one constant JSON payload
- one fixed local object-store sink emission
- one exact `emit-evidence` required ceiling
- no log-write side path

It exists so the live proof, draft-v1 linkage, and benchmark surfaces can prove
one honest exact `emit-evidence` slice without pretending broader shapes are
supported.
