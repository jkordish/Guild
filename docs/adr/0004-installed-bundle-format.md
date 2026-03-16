# ADR 0004: Installed bundle format

## Status

Accepted

## Context

Guild now has a real local bundle export/import path.

That path is no longer speculative:

- source manifests install into executable installed state
- execution resolves from installed state, not source trees
- export/import works for primitive and composite inspect skills
- local signature and trust verification are enforced before import
- imported skills execute without rebuilding

ADR 0001 established that executable identity must be immutable and portable.
ADR 0003 established that the host owns durable platform records and translation boundaries.
What was still missing was an explicit decision for the current local transport unit so future work does not quietly slide back toward source-tree packaging or trust-by-folklore.

## Decision

Guild's current portable bundle format is an installed-state directory bundle, not a source package and not a final remote publication format.

The current local bundle is defined as follows:

- export starts from installed executable state under the local registry, never from a source directory
- the bundle root contains `bundle.json`, `bundle.signature.json`, and one or more copied installed skill directories under `installed/...`
- the bundle index type is `InstalledSkillBundle`
- the signature sidecar type is `BundleSignatureEnvelope`
- the current format version strings are `guild-installed-bundle-v2` and `guild-installed-bundle-signature-v1`

`bundle.json` is the host-built index for the bundle. It contains:

- `format_version`
- `root_skill` as a `ResolvedSkillRef`
- `includes_dependency_closure`
- one bundle-level `publisher`
- `skills`, where each entry maps a bundled `ResolvedSkillRef` to its relative installed directory
- `files`, where each entry records the relative bundled file path and its `sha256` digest

The bundle file list is for bundled installed files only. It does not treat `bundle.json` or `bundle.signature.json` as installed payload files.

Each bundled installed directory is copied from executable installed state and currently includes:

- the installed `manifest.json`
- the executable Wasm artifact referenced by `package.artifact_uri`
- staged support files required by execution or interface use, such as schemas, examples, and any locally staged optional support files

`verification.json` is never exported. Verification metadata is host-owned target-registry state, not bundle payload.

Source and installed lifecycle stages remain distinct:

- `SourceSkillManifest` is human-authored source state
- `SkillManifest` is executable installed state
- export uses the installed manifest with digest-pinned artifact and installed dependency snapshots
- import produces normal installed state in the target registry

Composite export is dependency-closure export only when requested:

- `include_dependencies = false` exports only the selected installed root skill
- `include_dependencies = true` exports the transitive installed dependency closure by exact `ResolvedSkillRef`
- in the current milestone, all bundled installs must share the signing publisher

Signature and trust are local and real:

- `bundle.signature.json` is required for import in the current local signed flow
- the signing scheme is Ed25519
- the signature covers the serialized `bundle.json` bytes
- bundled installed file contents are authenticated transitively through the digest list inside `bundle.json`
- import loads a trusted public key from the target registry trust store and verifies both publisher trust and signature validity before install

Import is source-independent and host-mediated:

- import validates bundle structure, bundle file digests, bundled installed manifests, staged support files, and artifact digests before installation
- new installs are copied into `.bundle-import-staging` and then moved into the target registry
- identical existing installs may be reused instead of overwritten
- imported installs receive host-owned `verification.json` sidecars
- execution after import resolves from installed state exactly like any other local install
- import never rebuilds from source and does not require the original source workspace to exist

## Consequences

Positive:

- Guild now has a concrete local transport unit built from the same executable state that actually runs
- imported execution is source-independent and digest-pinned
- trust verification remains host-owned instead of being delegated to source manifests
- composite portability preserves dependency alias snapshots through installed manifests

Costs and current limits:

- the current bundle is a directory tree, not yet a remote distribution artifact shape
- the current signed closure flow is single-publisher only
- the signature authenticates the bundle index plus listed file digests rather than a packed archive format
- target-registry ambiguity is rejected on import, but the importer does not yet add a separate explicit intra-bundle same-key/version multi-digest rule beyond the existing index-entry and target-state checks

## Explicit invariants

- `RequestedSkillRef` is never executable bundle identity; bundles index installed `ResolvedSkillRef` values
- bundles are built from installed executable state, not source trees
- import does not rebuild from source
- import verifies trust, signature, and bundled file digests before installation
- import does not expose partially validated executable state as installed state
- local verification metadata is host-owned and excluded from exported bundle contents
- imported skills execute through the normal installed resolution path

## Explicit non-goals / deferred work

- defining a remote registry publication format
- defining an OCI mapping
- defining Sigstore or transparency-log integration
- defining remote trust policy or federated publisher trust
- widening the current same-publisher closure rule
- redefining bundles as source packages or rebuild-on-import artifacts

## Cross-references

- `README.md`
- `SPECS.md`
- `ARCHITECTURE.md`
- `MEMORY.md`
- `docs/adr/0001-guild-thesis.md`
- `docs/adr/0003-guest-abi-vs-host-record-boundary.md`
- `crates/guild-manifest/src/lib.rs`
- `crates/guild-registry/src/lib.rs`
- `crates/guild-mcp/examples/export_import_local.rs`
- `crates/guild-mcp/examples/export_import_composite_local.rs`
- `crates/guild-mcp/examples/signed_import_failures_local.rs`
