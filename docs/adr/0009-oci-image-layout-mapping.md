# ADR 0009: OCI image layout mapping for installed bundles

## Status

Accepted

## Context

ADR 0004 defined Guild's current portable transport unit as a signed installed-state bundle directory.

That remains the canonical signed payload:

- export starts from installed executable state, not source trees
- the signed index is still `bundle.json`
- the detached signature is still `bundle.signature.json`
- import still verifies trusted publisher, signature, and bundled file digests before installation

What was still missing was a standards-shaped local transport mapping that could carry the same installed executable payload without prematurely building remote publication, OCI-native trust policy, or a registry client.

## Decision

Guild adds OCI image layout as an additional local transport mapping for the existing signed installed-bundle payload.

The canonical signed transport semantics do not change:

- `bundle.json` remains the signed installed-bundle index
- `bundle.signature.json` remains the detached Ed25519 signature envelope
- bundled installed content still comes from installed executable state under `installed/...`
- `verification.json` remains host-owned target-registry metadata and is never exported

The OCI layout mapping is defined as follows:

- the export root is a valid OCI image layout directory with `oci-layout`, `index.json`, and `blobs/sha256/...`
- `index.json` contains exactly one root descriptor for the exported Guild unit in this milestone
- the root descriptor points at an OCI image manifest with `artifactType = application/vnd.guild.installed-bundle.oci.v1`
- the manifest config blob is the exact serialized `bundle.json` bytes with media type `application/vnd.guild.installed-bundle.v2+json`
- one manifest layer stores the exact serialized `bundle.signature.json` bytes with media type `application/vnd.guild.installed-bundle.signature.v1+json`
- each bundled installed file is stored as its own OCI blob layer with media type `application/vnd.guild.installed-bundle.file.v1`
- each bundled file layer carries `org.opencontainers.image.title = <bundle-relative installed path>`
- the root descriptor annotations identify the root skill key, version, digest, publisher id, and whether dependency closure was included

Import remains trust-preserving and fail-closed:

- OCI import first validates OCI image layout structure plus descriptor size and digest integrity
- OCI import then reconstructs the same signed bundle payload in staging
- Guild reuses the existing bundle trust/signature/file-digest/import path for final verification and installation
- imported installs become normal installed records and receive the same host-owned `verification.json` metadata as native bundle imports

## Consequences

Positive:

- Guild's local portability story is now standards-aligned without changing the executable or trust boundary
- primitive and composite installed dependency closures can move between Guild roots as OCI image layouts without rebuilding
- future OCI registry publication can build on a real local mapping instead of requiring a format rewrite later

Costs and limits:

- OCI layout is local file-backed only in this milestone
- remote push/pull, Sigstore, transparency logs, and OCI-native remote trust remain deferred
- the native signed-bundle directory remains the canonical signed payload and is still supported alongside OCI layout
- imported execution remains intentionally transport-agnostic after installation; Guild does not persist a different execution model for OCI-imported installs

## Explicit invariants

- `RequestedSkillRef` is never executable transport identity
- execution still resolves only against installed executable state
- OCI layout does not bypass local publisher trust or signature verification
- import does not rebuild from source
- composite alias snapshots remain the installed manifest truth
- import does not expose partially validated executable state as installed state

## Explicit non-goals / deferred work

- OCI registry push/pull
- remote publication workflows
- Sigstore or transparency-log integration
- replacing Guild's current local signature model with OCI-native signing
- widening the single-publisher closure rule
- adding new MCP tools or new execution modes

## Cross-references

- `README.md`
- `SPECS.md`
- `ARCHITECTURE.md`
- `MEMORY.md`
- `docs/adr/0004-installed-bundle-format.md`
- `crates/guild-registry/src/lib.rs`
- `crates/guild-mcp/examples/export_import_oci_local.rs`
- `crates/guild-mcp/examples/export_import_composite_oci_local.rs`
- `crates/guild-mcp/examples/signed_import_oci_failures_local.rs`
