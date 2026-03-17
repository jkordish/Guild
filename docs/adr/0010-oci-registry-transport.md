# ADR 0010: OCI Registry Transport

- Status: accepted
- Date: 2026-03-16

## Context

Guild already had one canonical signed installed-bundle transport unit and one OCI image layout mapping for that same installed executable state. The next portability step is to move that same installed-state artifact through a real OCI registry without turning registry transport into a new trust model.

## Decision

Guild uses OCI registry push/pull as a remote movement layer for the existing signed installed-bundle semantics:

- the signed installed bundle remains the canonical transport unit
- OCI image layout remains the canonical OCI-shaped mapping of that unit
- OCI registry publication pushes that same OCI-mapped artifact through a registry reference
- pull/import reconstructs the same signed installed bundle payload and then runs the existing local trust, signature, and bundled-digest verification before installation
- imported skills remain ordinary installed records that resolve and execute through the normal registry, runner, Wasmtime, persistence, and MCP paths

## Consequences

Positive:

- Guild can move installed executable state across machines through a real OCI registry
- primitive skills and composite dependency closures use one honest artifact mapping instead of separate local and remote package formats
- future remote publication work can build on a standards-shaped transport without reformatting the installed payload again

Constraints:

- OCI registry transport is not a trust bypass
- Guild signatures remain embedded bundle metadata, not OCI-native or Sigstore signatures
- remote trust distribution, transparency logs, discovery, and broader publication workflows remain deferred
