# ADR 0018: Filesystem policy contract (not yet implemented)

Status: Accepted  
Date: 2026-03-17

## Context

Guild's current inspect slice deliberately does not expose filesystem authority
to guest skills.

That is a strength, not an omission to paper over. Filesystem access is one of
the easiest ways to reintroduce ambient authority, path confusion, and accidental
host escape.

The repository needs a guardrail ADR for filesystem policy shape now, before any
runtime support lands, so future work does not drift into vague path allowlists
or implicit host access.

## Decision

This ADR is design-only. It does not describe a working filesystem capability in
the current repository.

Current truth:

- the active Wasm inspect slice supports `http-request`, `read-resource`,
  `invoke-skill`, `emit-evidence`, and `log-write` only
- the shared Rust contracts now expose an explicit typed `filesystem` family as
  a host-side contract only
- the active inspect guest ABI `guild-skill-inspect-v1` does not expose
  filesystem imports or preopened
  directories
- unsupported families in the active inspect slice are rejected before
  execution, and filesystem now gets that rejection intentionally rather than
  as a vague unknown surface

The current host-side contract uses `CapabilityId::Filesystem` with
`FilesystemConstraints { preopened_roots: Vec<FilesystemRoot> }`, where each
`FilesystemRoot` carries:

- `name`
- `guest_path_prefix`
- `host_path`
- `operations: Vec<FilesystemOperation>`

`FilesystemOperation` is currently frozen as `read`, `write`, `create`, and
`append`.

For that reason, filesystem capability requests are now rejected in practice by
an explicit active-inspect preflight boundary. There is still no executable
filesystem runtime path today.

Before any runtime support lands, Guild freezes the required policy shape for a
future filesystem family:

1. filesystem authority must be host-mediated and explicitly granted
2. authority must be scoped to one or more preopened roots or sandbox roots
3. path handling must canonicalize candidate paths against the host filesystem
   model before access
4. traversal or escape outside the granted root must be rejected fail-closed
5. read, write, create, and append semantics must remain distinct policy
   dimensions rather than one generic "filesystem access" flag
6. future policy may add size, byte, file-count, or write-count ceilings, but
   those limits must remain explicit and typed
7. trust tier and selected profile must be able to reduce or deny filesystem
   authority before guest start

The future contract must preserve these minimum semantics even if the eventual
ABI encoding changes:

- a granted root is a host-owned canonical root, not a guest-relative string
- authorization checks compare canonicalized target paths to canonicalized roots
- `..`, symlink traversal, path normalization quirks, and alternate path forms
  must not allow root escape
- writes must not be authorized simply because reads are authorized
- create/append semantics must not be smuggled under broad write authority

Safe defaults for any future implementation are already decided:

- no explicit filesystem grant means denial
- no implicit current-working-directory access
- no raw host root access
- no guest-chosen root expansion
- no fallback to ambient process filesystem access when a policy value is absent

Host-owned denial behavior is also frozen now:

- preflight rejection must happen before guest execution when the requested
  filesystem authority is unsupported, invalid, or outside policy
- runtime access outside the granted canonical root must remain a host-owned
  denial, not a guest-authored filesystem error masquerading as authorization

Until runtime support exists, this ADR acts as a contract guardrail only. It is
not evidence that filesystem capability is implemented, tested, or grantable.

## Consequences

Positive:

- future filesystem work now has a least-authority contract to implement against
- Guild keeps filesystem clearly separate from `read-resource`
- trust-tier-aware policy can extend to filesystem later without inventing a new
  conceptual model

Costs and limits:

- the ADR still leaves exact guest ABI encoding deferred
- current users still have no filesystem capability in the active inspect slice

## Explicit invariants

- filesystem is not implemented in the current repository
- the active inspect slice does not grant guest filesystem authority
- future filesystem access must be host-mediated and root-scoped
- canonicalization and escape prevention are mandatory
- read, write, create, and append semantics must remain distinct
- unsupported or invalid filesystem requests must fail closed before guest start

## Explicit non-goals / deferred work

- implementing filesystem runtime support
- widening the current WIT guest ABI to expose filesystem imports
- ambient filesystem access for local demos
- broad workflow DSLs around path authorization
- apply-mode mutation policy

## Cross-references

- `AGENTS.md`
- `SPECS.md`
- `ARCHITECTURE.md`
- `docs/adr/0005-capability-schema-and-active-inspect-profile.md`
- `docs/adr/0008-local-policy-evaluator.md`
- `docs/adr/0012-capability-policy-layering-model.md`
- `wit/guild-skill-v1.wit`
- `crates/guild-types/src/lib.rs`
- `crates/guild-runner/src/lib.rs`
