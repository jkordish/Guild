# Guild Effect Kernel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Import Jidoka effect protocol v1 without changing its wire identity, implement it as Guild's deterministic `guild-effect-kernel` leaf crate, and make protocol conformance a required workspace gate before any host mutation integration.

**Architecture:** The crate accepts authenticated values and returns validated bodies, transition bundles, projections, classifications, and sealed one-shot start permits; it performs no I/O and depends on no Guild crate. A content-addressed body graph plus an anchored event chain is the authoritative model, while Guild's host, adapter, persistence, execution-link, CLI, MCP, and `apply` integration remain outside this plan and require their own approved design.

**Tech Stack:** Rust 1.94.0, edition 2024, Cargo workspace, Serde, RFC 8785/JCS canonical JSON, SHA-256, `thiserror`, `proptest`, repo `xtask`, Make, and GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-02-guild-effect-kernel-migration-design.md`; imported normative protocol: `docs/protocol/effect-kernel-v1.md`

## Global Constraints

- The Cargo package is exactly `guild-effect-kernel`; the Rust crate is exactly `guild_effect_kernel`; its path is exactly `crates/guild-effect-kernel`.
- Preserve protocol v1 identifiers and canonical bytes exactly, including `schemaVersion: "jidoka.dev/events/v1"`, all 29 body-kind strings, and all 26 event-type strings.
- Compile under Guild's pinned Rust `1.94.0` toolchain and edition `2024`; protocol conformance wins over the recovered Jidoka workspace's former `1.98.0` pin.
- Pin protocol-sensitive dependencies inside this package to `hex = "=0.4.3"`, `serde = "=1.0.228"`, `serde_jcs = "=0.1.0"`, `serde_json = "=1.0.145"`, `sha2 = "=0.10.9"`, `thiserror = "=2.0.17"`, and dev dependency `proptest = "=1.8.0"`.
- The kernel has no filesystem, network, process, clock, randomness, database, provider, Wasm, MCP, session, AI, or Guild-crate dependency. It accepts values and returns proposed values.
- Do not map `CallerRequest.idempotency_key` to a kernel binding and do not treat Guild `EvidenceRef` as an authoritative kernel observation.
- Keep Axiom planning truth, Guild execution/session truth, and effect/custody truth separate. Keep execution receipts, effect receipts, and future session receipts as distinct types.
- A durable `Started` event is an absolute retry barrier. Recovery may probe and classify; it must never propose or return a second protected mutation permit.
- The first closed effect family is static artifact publication and separation/quarantine. Do not add a generic effect plugin ABI.
- Do not add a public CLI, MCP method, URI, manifest field, WIT surface, adapter, persistence implementation, or active `apply` path in this plan.
- Public construction of bindings, leases, evidence classifications, receipts, proof tokens, deeds, and start permits must be impossible outside their lawful transition APIs.
- Every implementation task is test-first, ends in a focused commit, and is pushed as a durable remote checkpoint before the next task begins.

---

## Scope Boundary

This plan implements migration Phases 1 through 3 only: normative protocol import, the pure kernel, and its conformance gate. Authenticated storage, trusted-clock/principal mapping, execution-record links, session retry barriers, recovery scheduling, the local-file adapter, operator inspection, and active mutation are Phase 4 and must be designed separately after this plan passes. The Phase 5 Jidoka migration pointer and repository disposition are also outside this plan and may begin only after Task 15 proves parity; no task here edits or archives the source repository.

## File Structure

The implementation locks in this structure:

| Path | Responsibility |
| --- | --- |
| `docs/protocol/effect-kernel-v1.md` | Verbatim recovered v1 protocol with a provenance preamble outside the normative body. |
| `docs/protocol/effect-kernel-v1-change-ledger.md` | Exhaustive non-wire migration changes: repository owner, crate name, Rust pin, and Guild terminology. |
| `crates/guild-effect-kernel/src/protocol.rs` | Frozen protocol IDs: 29 body kinds, 26 event types, and event schema version. |
| `crates/guild-effect-kernel/src/scalar.rs` | Validated scalar newtypes, checked counters, and closed validation errors. |
| `crates/guild-effect-kernel/src/canonical.rs` | Strict JSON decoding, duplicate-member rejection, JCS bytes, and SHA-256 identity. |
| `crates/guild-effect-kernel/src/schema.rs` | Closed five-schema registry and exact compiled descriptors. |
| `crates/guild-effect-kernel/src/body.rs` | Typed body references, 29 body payloads/kinds, graph edges, two-pass validation, and immutable insertion. |
| `crates/guild-effect-kernel/src/authority.rs` | Enrollment, immutable policy, warrants, approvals, revocations, expiry, and authority checks. |
| `crates/guild-effect-kernel/src/lease.rs` | Effect/resource identity, permanent bindings, five-second leases, budgets, fences, locks, cancellation, and sequence reserves. |
| `crates/guild-effect-kernel/src/evidence.rs` | Probe evidence, limitations, postconditions, causality, receipts, deed proof, custody derivation, and closed outcome vocabularies. |
| `crates/guild-effect-kernel/src/event.rs` | The 26 closed typed event payloads, canonical preimages/envelopes, and anchored-chain validation. |
| `crates/guild-effect-kernel/src/store.rs` | Immutable body/event maps and atomic compare-and-swap proposal bundles; no persistence implementation. |
| `crates/guild-effect-kernel/src/projection.rs` | Full replay, incremental projection, exhaustive transition relation, address claims, and illegal-history rejection. |
| `crates/guild-effect-kernel/src/publication.rs` | Publication proposal, approval, reservation, preparation, start, cancellation, and live terminalization. |
| `crates/guild-effect-kernel/src/recovery.rs` | Recovery discovery and terminalization of incomplete starts without returning mutation authority. |
| `crates/guild-effect-kernel/src/separation.rs` | Separation proposal, reservation, start, terminalization, recovery, and custody generation. |
| `crates/guild-effect-kernel/src/model.rs` | Dossier validation, summary derivation, legal-history builders used by conformance tests, and protocol facade. |
| `crates/guild-effect-kernel/tests/` | Public-contract, property, corruption, transition, crash-point, and compile-fail doctest coverage grouped by kernel law. |
| `crates/guild-effect-kernel/tests/support/mod.rs` | Deterministic, lawful fixture builders shared by integration tests; fixed values only, no I/O or generated identity. |
| `vectors/effect-kernel-v1/` | Checked canonical-body and complete-dossier JSON vectors plus a digest manifest. |
| `xtask/src/effect_kernel.rs` | Dependency-firewall and vector checks; this host-side tool may depend on the leaf crate. |
| `Makefile`, `.github/workflows/ci.yml` | Required local and CI conformance gates. |

`lib.rs` exports modules by law, not by provider. Test-only builders live in integration-test support or `model`; no filesystem fixture loader enters the kernel crate.

The recovered thirteen-increment order is preserved inside the larger migration plan:

| Recovered increment | Plan task |
| --- | --- |
| 1. pinned Rust workspace | Task 2 |
| 2. scalar types and errors | Task 3 |
| 3. canonicalization and schema descriptors | Task 4 |
| 4. identity and registry graph | Task 5 |
| 5. warrants, leases, bindings, fences, and budgets | Task 6 |
| 6. evidence, causality, receipts, and deeds | Task 7 |
| 7. immutable storage and events | Task 8 |
| 8. replay and projections | Task 9 |
| 9. publication admission | Task 10 |
| 10. publication outcomes | Task 11 |
| 11. recovery | Task 12 |
| 12. separation | Task 13 |
| 13. complete model and golden dossiers | Task 14 |

Task 1 is migration Phase 1, and Task 15 is the Phase 3 conformance gate around those thirteen implementation increments.

---

### Task 1: Import The Normative Protocol And Align Guild Documentation

**Files:**
- Create: `docs/protocol/effect-kernel-v1.md`
- Create: `docs/protocol/effect-kernel-v1-change-ledger.md`
- Modify: `SPECS.md`
- Modify: `ARCHITECTURE.md`
- Modify: `AGENTS.md`
- Modify: `docs/first-honest-mutation-demo.md`
- Verify: `docs/adr/README.md`
- Verify: `docs/adr/0021-adopt-the-effect-kernel-as-guilds-mutation-truth-boundary.md`
- Verify: `docs/superpowers/specs/2026-09-02-guild-effect-kernel-migration-design.md`

**Interfaces:**
- Consumes: source commit `78ace548bdfbf7bd354c0d97e22f71b3dfd6526f`, source path `docs/superpowers/specs/2026-09-01-jidoka-autonomous-change-kernel-recovered-design.md`, source SHA-256 `86df64803cd2da89f6d6499aac4f884184b2799122a8f2e5e4cc7f9f178b177b`.
- Produces: the in-repository normative protocol path used by every remaining task and explicit docs language that does not claim a shipping mutation path.

- [ ] **Step 1: Prove the approved decision is the starting point and the protocol is not yet imported**

Run:

```bash
test ! -e docs/protocol/effect-kernel-v1.md
rg -n 'cache purge with evidence trail' docs/first-honest-mutation-demo.md
rg -n 'Status: accepted|\*\*Status:\*\* Approved|accepted adoption' \
  docs/adr/0021-adopt-the-effect-kernel-as-guilds-mutation-truth-boundary.md \
  docs/superpowers/specs/2026-09-02-guild-effect-kernel-migration-design.md \
  docs/adr/README.md
```

Expected: the first command succeeds, the old cache-purge choice remains to be replaced, and the ADR/design/index already report the approved decision.

- [ ] **Step 2: Verify and import the recovered protocol without editing its normative body**

From a checkout of `jkordish/jidoka` containing the source commit, run:

```bash
guild_repo_dir="$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")"
jidoka_repo_dir="$(dirname "$guild_repo_dir")/jidoka"
git -C "$jidoka_repo_dir" show 78ace548bdfbf7bd354c0d97e22f71b3dfd6526f:docs/superpowers/specs/2026-09-01-jidoka-autonomous-change-kernel-recovered-design.md > /tmp/effect-kernel-v1.source.md
shasum -a 256 /tmp/effect-kernel-v1.source.md
```

Expected SHA-256:

```text
86df64803cd2da89f6d6499aac4f884184b2799122a8f2e5e4cc7f9f178b177b
```

Copy those exact bytes with `cp /tmp/effect-kernel-v1.source.md docs/protocol/effect-kernel-v1.md`, then use `apply_patch` to prepend this non-normative provenance preamble:

```markdown
<!--
Provenance (non-normative): imported from jkordish/jidoka commit
78ace548bdfbf7bd354c0d97e22f71b3dfd6526f at
docs/superpowers/specs/2026-09-01-jidoka-autonomous-change-kernel-recovered-design.md.
The imported source body's SHA-256 is
86df64803cd2da89f6d6499aac4f884184b2799122a8f2e5e4cc7f9f178b177b.
The normative protocol begins at the first H1 below. Guild ownership changes
are recorded separately and do not alter v1 wire identity.
-->

```

Recompute the SHA-256 after stripping the preamble through its terminating blank line:

```bash
tail -n +12 docs/protocol/effect-kernel-v1.md | shasum -a 256
```

Expected: the same source SHA-256.

- [ ] **Step 3: Add the exact migration ledger**

Create `docs/protocol/effect-kernel-v1-change-ledger.md` with these four entries and an explicit “no wire change” verdict for each:

```markdown
# Effect Kernel v1 Guild Migration Ledger

| Concern | Recovered source | Guild implementation | Wire effect |
| --- | --- | --- | --- |
| Product owner | Jidoka | Guild | None; Jidoka remains v1 protocol provenance. |
| Cargo identity | `jidoka-kernel` / `jidoka_kernel` | `guild-effect-kernel` / `guild_effect_kernel` | None; Cargo names are not protocol fields. |
| Rust toolchain | 1.98.0 | 1.94.0 | None; canonical vectors must prove byte parity. |
| Integration terms | standalone coordinator | future Guild host integration | None; host integration is outside protocol v1 and outside this implementation phase. |

The following values are frozen and are not migration-ledger substitutions:

- event schema version `jidoka.dev/events/v1`;
- all 29 body-kind strings;
- all 26 event-type strings;
- canonical JSON bytes, SHA-256 identities, classification tables, and transition laws.
```

- [ ] **Step 4: Align Guild's truth-boundary documentation**

Apply these exact semantic edits while leaving the already accepted ADR/design status unchanged:

- Add a `Planned Effect Truth Boundary` subsection to `SPECS.md` stating that `docs/protocol/effect-kernel-v1.md` is normative only for the pure effect protocol, the live runner still rejects `apply`, and effect receipts do not replace execution receipts.
- Add `guild-effect-kernel` to `ARCHITECTURE.md`'s repository mapping as a pure, disconnected leaf crate and add a three-row truth-layer table for Axiom, Guild execution/session, and effect/custody truth.
- Add repository rules to `AGENTS.md`: preserve v1 IDs, no Guild dependency from the kernel, no host integration claim, and no retry after durable start.
- Replace the cache-purge choice in `docs/first-honest-mutation-demo.md` with static artifact publication plus separation/quarantine, and state that the note is superseded by ADR 0021 for first-effect selection.

Use this exact current-state sentence in every explanatory surface that could otherwise overclaim:

```text
The pure effect protocol is planned and may be implemented in this repository; Guild's live runner still rejects apply, and no host adapter or protected mutation path ships from that fact alone.
```

- [ ] **Step 5: Verify provenance, links, status, and honest positioning**

Run:

```bash
rg -n 'Status: accepted|\*\*Status:\*\* Approved' \
  docs/adr/0021-adopt-the-effect-kernel-as-guilds-mutation-truth-boundary.md \
  docs/superpowers/specs/2026-09-02-guild-effect-kernel-migration-design.md
rg -n 'jidoka.dev/events/v1|guild-effect-kernel|live runner still rejects apply' \
  SPECS.md ARCHITECTURE.md AGENTS.md docs/first-honest-mutation-demo.md \
  docs/protocol/effect-kernel-v1.md
git diff --check
```

Expected: accepted/approved status appears once in each decision document, the frozen protocol ID and honest-state sentence are present, and `git diff --check` exits zero.

- [ ] **Step 6: Commit and checkpoint**

```bash
git add AGENTS.md ARCHITECTURE.md SPECS.md docs/first-honest-mutation-demo.md docs/protocol
git commit -m "docs: adopt the Guild effect kernel protocol"
git push origin HEAD:design/guild-effect-kernel
```

Expected: one docs-only commit; no Rust or runtime behavior changes.

---

### Task 2: Create The Leaf Crate And Mechanical Dependency Firewall

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/guild-effect-kernel/Cargo.toml`
- Create: `crates/guild-effect-kernel/src/lib.rs`
- Create: `crates/guild-effect-kernel/src/protocol.rs`
- Create: `crates/guild-effect-kernel/tests/protocol.rs`
- Create: `xtask/src/effect_kernel.rs`
- Modify: `xtask/src/main.rs`
- Modify: `Makefile`
- Modify: `.github/workflows/ci.yml`
- Modify: `Cargo.lock`

**Interfaces:**
- Consumes: the 29 body-kind and 26 event-type strings from `docs/protocol/effect-kernel-v1.md` §§7 and 10.2.
- Produces: `EVENT_SCHEMA_VERSION: &str`, `BODY_KIND_IDS: [&str; 29]`, `EVENT_TYPE_IDS: [&str; 26]`, and `cargo run -q -p xtask -- effect-kernel check-dependencies`.

- [ ] **Step 1: Write the failing protocol-freeze test**

Create `crates/guild-effect-kernel/tests/protocol.rs`:

```rust
use guild_effect_kernel::protocol::{BODY_KIND_IDS, EVENT_SCHEMA_VERSION, EVENT_TYPE_IDS};

#[test]
fn protocol_v1_identifiers_are_frozen() {
    assert_eq!(EVENT_SCHEMA_VERSION, "jidoka.dev/events/v1");
    assert_eq!(BODY_KIND_IDS.len(), 29);
    assert_eq!(BODY_KIND_IDS[0], "installation-enrollment/v1");
    assert_eq!(BODY_KIND_IDS[28], "dossier-summary/v1");
    assert_eq!(EVENT_TYPE_IDS.len(), 26);
    assert_eq!(EVENT_TYPE_IDS[0], "installation_enrolled");
    assert_eq!(EVENT_TYPE_IDS[25], "custody_disputed");
}
```

- [ ] **Step 2: Run the test and observe the missing package failure**

Run:

```bash
cargo test -p guild-effect-kernel --test protocol
```

Expected: FAIL because package `guild-effect-kernel` is not yet a workspace member.

- [ ] **Step 3: Add the package with exact pins and no Guild dependencies**

Add `crates/guild-effect-kernel` to the root workspace member list. Create this package manifest:

```toml
[package]
categories.workspace = true
description = "Deterministic mutation admission, evidence, receipt, recovery, and custody kernel for Guild"
edition.workspace = true
keywords.workspace = true
license.workspace = true
name = "guild-effect-kernel"
readme.workspace = true
repository.workspace = true
rust-version.workspace = true
version.workspace = true
publish = false

[dependencies]
hex = "=0.4.3"
serde = { version = "=1.0.228", features = ["derive"] }
serde_jcs = "=0.1.0"
serde_json = "=1.0.145"
sha2 = "=0.10.9"
thiserror = "=2.0.17"

[dev-dependencies]
proptest = "=1.8.0"

[lints]
workspace = true
```

Create `src/lib.rs` and forbid unsafe code:

```rust
#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

//! Pure deterministic effect protocol kernel. This crate performs no I/O.

pub mod protocol;
```

- [ ] **Step 4: Implement the complete frozen identifier arrays**

Create `src/protocol.rs` with `EVENT_SCHEMA_VERSION` plus these arrays in this exact order:

```rust
pub const EVENT_SCHEMA_VERSION: &str = "jidoka.dev/events/v1";

pub const BODY_KIND_IDS: [&str; 29] = [
    "installation-enrollment/v1", "authority-policy/v1", "schema-descriptor/v1",
    "local-file-observation/v1", "xattr-value/v1",
    "static-artifact-publish-input/v1", "static-artifact-publish-precondition/v1",
    "static-artifact-separation-input/v1", "static-artifact-separation-precondition/v1",
    "publication-warrant/v1", "publication-approval/v1", "publication-revocation/v1",
    "effect-lease/v1", "idempotency-binding/v1", "prepared-artifact/v1",
    "publication-evidence/v1", "causality-assessment/v1", "effect-receipt/v1",
    "resource-deed/v1", "separation-warrant/v1", "separation-approval/v1",
    "separation-revocation/v1", "separation-lease/v1", "separation-binding/v1",
    "separation-evidence/v1", "separation-receipt/v1", "custody-record/v1",
    "recovery-assessment/v1", "dossier-summary/v1",
];

pub const EVENT_TYPE_IDS: [&str; 26] = [
    "installation_enrolled", "warrant_proposed", "warrant_approved",
    "warrant_revoked", "warrant_expired", "effect_reserved",
    "effect_cancelled_before_start", "effect_started", "artifact_prepared",
    "artifact_published", "artifact_published_recovered", "effect_verified",
    "effect_failed", "effect_indeterminate", "separation_warrant_proposed",
    "separation_warrant_approved", "separation_warrant_revoked",
    "separation_warrant_expired", "separation_reserved",
    "separation_cancelled_before_start", "separation_started",
    "separation_verified", "separation_failed", "separation_indeterminate",
    "custody_absent", "custody_disputed",
];
```

- [ ] **Step 5: Add the cargo-metadata dependency firewall**

Implement `xtask/src/effect_kernel.rs` with this command interface:

```rust
pub fn run(mut args: impl Iterator<Item = String>) -> anyhow::Result<()>;
```

It accepts only `check-dependencies`, runs `cargo metadata --format-version 1 --no-deps`, locates package `guild-effect-kernel`, and rejects any normal dependency outside this exact set:

```rust
const ALLOWED_NORMAL: &[&str] = &[
    "hex", "serde", "serde_jcs", "serde_json", "sha2", "thiserror",
];
const ALLOWED_DEV: &[&str] = &["proptest"];
const FORBIDDEN_NAME_FRAGMENTS: &[&str] = &[
    "guild-", "tokio", "reqwest", "hyper", "rusqlite", "sqlx", "wasmtime",
    "wit-bindgen", "uuid", "rand", "getrandom",
];
```

The check must also require `rust_version == "1.94"`, `edition == "2024"`, `publish == []`, and exact dependency requirements including the leading `=`. Wire `effect-kernel` through `xtask/src/main.rs` and add its exact usage line.

- [ ] **Step 6: Make the firewall and protocol test pass**

Run:

```bash
cargo test -p guild-effect-kernel --test protocol
cargo run -q -p xtask -- effect-kernel check-dependencies
```

Expected: one passing test and `effect-kernel dependency boundary: ok`.

- [ ] **Step 7: Add the gate to Make and CI**

Add a phony `effect-kernel-boundary` target that runs the xtask command, add it to `verify`, and add a GitHub Actions step named `Effect Kernel Boundary` after Clippy with the same command. Run:

```bash
make effect-kernel-boundary
git diff --check
```

Expected: both commands exit zero.

- [ ] **Step 8: Commit and checkpoint**

```bash
git add Cargo.toml Cargo.lock Makefile .github/workflows/ci.yml xtask crates/guild-effect-kernel
git commit -m "feat(effect-kernel): establish the protocol leaf crate"
git push origin HEAD:design/guild-effect-kernel
```

---

### Task 3: Implement Validated Scalars And Checked Counters

**Files:**
- Create: `crates/guild-effect-kernel/src/scalar.rs`
- Modify: `crates/guild-effect-kernel/src/lib.rs`
- Create: `crates/guild-effect-kernel/tests/scalar.rs`

**Interfaces:**
- Consumes: Serde only; no clock, randomness, or normalization service.
- Produces: `Digest`, `RawDigest`, `Hex256`, `Identifier`, `FieldName`, `XattrName`, `LogicalAddress`, `ArtifactName`, `IdempotencyKey`, `SafeUInt`, `U64Decimal`, `UnixSeconds`, `UnixNanoseconds`, `ByteLength`, `IncarnationId`, `ResourceKey`, `EffectId`, `ValidationError`, and checked arithmetic methods.

- [ ] **Step 1: Write boundary and hostile-deserialization tests**

Create tests that include these exact assertions:

```rust
use guild_effect_kernel::scalar::{
    ArtifactName, IdempotencyKey, LogicalAddress, SafeUInt, U64Decimal,
};

#[test]
fn scalar_boundaries_fail_closed() {
    assert!(LogicalAddress::parse(" local-file:///tmp/a").is_err());
    assert!(ArtifactName::parse(" \t ").is_err());
    assert_eq!(ArtifactName::parse("e\u{301}").unwrap().as_str(), "e\u{301}");
    assert!(IdempotencyKey::parse("fifteen-chars!!").is_err());
    assert!(SafeUInt::new(9_007_199_254_740_991).is_ok());
    assert!(SafeUInt::new(9_007_199_254_740_992).is_err());
}

#[test]
fn decimal_encoding_is_canonical_and_checked() {
    assert!(serde_json::from_str::<U64Decimal>(r#""01""#).is_err());
    assert!(serde_json::from_str::<U64Decimal>("1").is_err());
    let max = U64Decimal::parse("18446744073709551615").unwrap();
    assert!(max.checked_add(1).is_err());
}
```

Add a `proptest!` case that round-trips every `u64` through `U64Decimal::from_u64`, JSON, and `parse`, and a case that inserts one illegal ASCII byte into a valid `IdempotencyKey` and requires rejection.

- [ ] **Step 2: Run the scalar tests and verify the module is missing**

Run:

```bash
cargo test -p guild-effect-kernel --test scalar
```

Expected: FAIL with unresolved import `guild_effect_kernel::scalar`.

- [ ] **Step 3: Define the closed validation model**

Implement these public error categories; include the scalar name and offending index/value where applicable, but never normalize the input:

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("{scalar} must not be empty")]
    Empty { scalar: &'static str },
    #[error("{scalar} length {actual} is outside {min}..={max}")]
    Length { scalar: &'static str, min: usize, max: usize, actual: usize },
    #[error("{scalar} contains a forbidden character at byte {index}")]
    Character { scalar: &'static str, index: usize },
    #[error("{scalar} is not in canonical format")]
    Format { scalar: &'static str },
    #[error("{scalar} value {value} exceeds the admitted maximum {max}")]
    Range { scalar: &'static str, value: u128, max: u128 },
    #[error("{scalar} must be nonzero")]
    Zero { scalar: &'static str },
    #[error("{scalar} arithmetic overflow")]
    Overflow { scalar: &'static str },
}
```

- [ ] **Step 4: Implement each scalar with one fallible constructor and shared Serde validation**

Use private fields. `Deserialize` must call the same `parse`/`new` constructor as normal construction. Implement these signatures:

```rust
impl Digest { pub fn parse(input: &str) -> Result<Self, ValidationError>; pub fn as_str(&self) -> &str; }
impl RawDigest { pub fn parse(input: &str) -> Result<Self, ValidationError>; pub fn as_str(&self) -> &str; }
impl Identifier { pub fn parse(input: &str) -> Result<Self, ValidationError>; pub fn as_str(&self) -> &str; }
impl LogicalAddress { pub fn parse(input: &str) -> Result<Self, ValidationError>; pub fn as_str(&self) -> &str; }
impl ArtifactName { pub fn parse(input: &str) -> Result<Self, ValidationError>; pub fn as_str(&self) -> &str; }
impl SafeUInt { pub const MAX: u64 = 9_007_199_254_740_991; pub fn new(value: u64) -> Result<Self, ValidationError>; pub fn get(self) -> u64; }
impl U64Decimal { pub const MAX: u64 = u64::MAX; pub fn parse(input: &str) -> Result<Self, ValidationError>; pub fn from_u64(value: u64) -> Self; pub fn get(self) -> u64; pub fn checked_add(self, rhs: u64) -> Result<Self, ValidationError>; pub fn checked_sub(self, rhs: u64) -> Result<Self, ValidationError>; }
impl ByteLength { pub fn from_u64(value: u64) -> Self; pub fn get(self) -> u64; }
impl UnixNanoseconds { pub fn parse(input: &str) -> Result<Self, ValidationError>; pub fn get(self) -> u64; pub fn checked_add(self, rhs: u64) -> Result<Self, ValidationError>; }
```

Apply the exact scalar rules in protocol §6.1. `ArtifactName` counts Unicode scalar values and stores the original bytes; `LogicalAddress` and `IdempotencyKey` are byte-for-byte opaque after validation; `U64Decimal` serializes as a string; `SafeUInt` serializes as a JSON number. `ByteLength`, `UnixSeconds`, and `UnixNanoseconds` wrap `U64Decimal`. `Digest`, `RawDigest`, `IncarnationId`, `ResourceKey`, and `EffectId` all encode `sha256:` plus 64 lowercase hex digits but remain distinct Rust types.

- [ ] **Step 5: Run focused and full crate tests**

Run:

```bash
cargo test -p guild-effect-kernel --test scalar
cargo test -p guild-effect-kernel
cargo clippy -p guild-effect-kernel --all-targets --all-features -- -D warnings
```

Expected: all tests pass and Clippy emits no warnings.

- [ ] **Step 6: Commit and checkpoint**

```bash
git add crates/guild-effect-kernel
git commit -m "feat(effect-kernel): add validated protocol scalars"
git push origin HEAD:design/guild-effect-kernel
```

---

### Task 4: Implement Strict Canonical JSON And The Five Schema Descriptors

**Files:**
- Create: `crates/guild-effect-kernel/src/canonical.rs`
- Create: `crates/guild-effect-kernel/src/schema.rs`
- Modify: `crates/guild-effect-kernel/src/lib.rs`
- Create: `crates/guild-effect-kernel/tests/canonical.rs`
- Create: `crates/guild-effect-kernel/tests/schema.rs`

**Interfaces:**
- Consumes: scalar types from Task 3 and exact descriptor rows from protocol §§6.2, 6.3, and 7.3.
- Produces: `canonical_bytes<T: Serialize>(&T) -> Result<Vec<u8>, CanonicalError>`, `canonical_digest<T: Serialize>(&T) -> Result<Digest, CanonicalError>`, `strict_from_slice<T: DeserializeOwned>(&[u8]) -> Result<T, CanonicalError>`, `SchemaId`, `FieldType`, `FieldDescriptor`, `SchemaDescriptor`, and `descriptor(SchemaId) -> &'static SchemaDescriptor`.

- [ ] **Step 1: Write the canonical golden and rejection tests**

```rust
#[test]
fn absent_observation_matches_the_protocol_golden() {
    let value = serde_json::json!({
        "kind": "local-file-observation/v1",
        "body": {
            "state": "absent",
            "logicalAddress": "local-file:///canonical/path",
            "witnessId": "host-probe",
            "observedAt": "1788210000000000000"
        }
    });
    let bytes = guild_effect_kernel::canonical::canonical_bytes(&value).unwrap();
    assert_eq!(bytes, br#"{"body":{"logicalAddress":"local-file:///canonical/path","observedAt":"1788210000000000000","state":"absent","witnessId":"host-probe"},"kind":"local-file-observation/v1"}"#);
    assert_eq!(guild_effect_kernel::canonical::canonical_digest(&value).unwrap().as_str(), "sha256:37acdc8236b6c57c87a7d68b0ed51cf02d9a97ba78edd6d13a3b3f754000cf81");
}

#[test]
fn strict_json_rejects_ambiguous_numbers_and_duplicate_members() {
    assert!(strict_from_slice::<serde_json::Value>(br#"{"a":1,"a":1}"#).is_err());
    assert!(strict_from_slice::<serde_json::Value>(br#"{"a":-1}"#).is_err());
    assert!(strict_from_slice::<serde_json::Value>(br#"{"a":1.0}"#).is_err());
    assert!(strict_from_slice::<serde_json::Value>(br#"{"a":9007199254740992}"#).is_err());
}
```

Add RFC 8785 ordering/escaping fixtures from the RFC that stay within the admitted unsigned-integer model.

- [ ] **Step 2: Run the tests and verify both modules are missing**

Run:

```bash
cargo test -p guild-effect-kernel --test canonical --test schema
```

Expected: FAIL with unresolved `canonical` and `schema` modules.

- [ ] **Step 3: Implement strict parsing before JCS encoding**

Define:

```rust
#[derive(Debug, thiserror::Error)]
pub enum CanonicalError {
    #[error("duplicate JSON member `{key}`")]
    DuplicateMember { key: String },
    #[error("JSON number is outside the canonical SafeUInt model")]
    Number,
    #[error("JSON decode failed: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("JCS encoding failed: {0}")]
    Encode(String),
    #[error("canonical digest was invalid: {0}")]
    Digest(#[from] ValidationError),
}
```

The strict decoder must recursively visit every map into a temporary ordered member list, reject a repeated key before building a `serde_json::Map`, reject negative/fractional/exponent/non-finite/out-of-range numbers, then deserialize `T` from the validated value. `canonical_bytes` must serialize through `serde_jcs::to_vec`; `canonical_digest` hashes only those bytes with SHA-256 and returns lowercase `sha256:<hex>`.

- [ ] **Step 4: Implement the closed schema registry exactly**

Define `SchemaId` with exactly five kebab-case `/v1` variants and `FieldType` with the 17 snake-case values in protocol §7.1. `descriptor` must return the exact sorted rows from §7.3. Use fixed slices, not runtime registration:

```rust
pub fn descriptor(schema_id: SchemaId) -> &'static SchemaDescriptor {
    match schema_id {
        SchemaId::LocalFileObservationV1 => &LOCAL_FILE_OBSERVATION,
        SchemaId::StaticArtifactPublishInputV1 => &STATIC_ARTIFACT_PUBLISH_INPUT,
        SchemaId::StaticArtifactPublishPreconditionV1 => &STATIC_ARTIFACT_PUBLISH_PRECONDITION,
        SchemaId::StaticArtifactSeparationInputV1 => &STATIC_ARTIFACT_SEPARATION_INPUT,
        SchemaId::StaticArtifactSeparationPreconditionV1 => &STATIC_ARTIFACT_SEPARATION_PRECONDITION,
    }
}
```

The schema tests must compare every descriptor entry `(name, field_type, required)` in order and reject an unknown schema ID or unknown field.

- [ ] **Step 5: Run tests, format, and lint**

```bash
cargo test -p guild-effect-kernel --test canonical --test schema
cargo fmt --all --check
cargo clippy -p guild-effect-kernel --all-targets --all-features -- -D warnings
```

Expected: all commands exit zero.

- [ ] **Step 6: Commit and checkpoint**

```bash
git add crates/guild-effect-kernel
git commit -m "feat(effect-kernel): canonicalize protocol values"
git push origin HEAD:design/guild-effect-kernel
```

---

### Task 5: Implement Typed Bodies And The Immutable Graph

**Files:**
- Create: `crates/guild-effect-kernel/src/body.rs`
- Modify: `crates/guild-effect-kernel/src/lib.rs`
- Create: `crates/guild-effect-kernel/tests/body_graph.rs`
- Create: `crates/guild-effect-kernel/tests/support/mod.rs`

**Interfaces:**
- Consumes: canonical/scalar/schema APIs from Tasks 3–4 and all 29 payload/edge rows from protocol §7.
- Produces: `BodyKind`, `BodyTag`, one marker tag/reference alias per body kind, `BodySpec`, `BodyRef<K>`, `ProtocolRef<P, S>`, `ValidatedBody<P>`, `StoredBody`, `BodyGraph`, `BodyBatch`, `BodyError`, `validated_body`, `validate_batch`, `validate_kind_edge_manifest`, and the base observation/input/precondition/xattr payload types.

- [ ] **Step 1: Write graph identity, edge, cycle, and kind-confusion tests**

Create `tests/support/mod.rs` with a deterministic `absent_observation(address: &str) -> ValidatedBody<LocalFileObservation>` helper using witness `host-probe` and observed time `1788210000000000000`. Then create these concrete tests:

```rust
mod support;

use std::collections::BTreeMap;
use guild_effect_kernel::body::{
    BodyBatch, BodyError, BodyGraph, BodyKind, LocalFileObservation,
    LocalFileObservationRef, StaticArtifactPublishInput, XattrEntry, XattrValue,
    validate_batch,
    validate_kind_edge_manifest, validated_body,
};
use guild_effect_kernel::protocol::BODY_KIND_IDS;
use guild_effect_kernel::scalar::{
    ArtifactName, ByteLength, Digest, LogicalAddress, RawDigest, XattrName,
};

#[test]
fn body_map_key_must_equal_canonical_identity() {
    let body = support::absent_observation("local-file:///staging/app");
    let wrong = Digest::parse(&format!("sha256:{}1", "0".repeat(63))).unwrap();
    let entries = BTreeMap::from([(wrong, body.canonical_bytes().to_vec())]);
    assert!(matches!(BodyGraph::from_canonical_entries(entries), Err(BodyError::KeyMismatch { .. })));
}

#[test]
fn typed_reference_rejects_a_body_of_the_wrong_kind() {
    let xattrs = validated_body(XattrValue::new(vec![XattrEntry::new(
        XattrName::parse("com.apple.quarantine").unwrap(),
        RawDigest::parse(&format!("sha256:{}", "1".repeat(64))).unwrap(),
        ByteLength::from_u64(1),
    )]).unwrap()).unwrap();
    let lied_about_kind = LocalFileObservationRef::from_digest(
        xattrs.reference().digest().clone(),
    );
    let input = validated_body(StaticArtifactPublishInput::new(
        ArtifactName::parse("app").unwrap(),
        lied_about_kind,
        LogicalAddress::parse("local-file:///active/app").unwrap(),
    ).unwrap()).unwrap();
    let batch = BodyBatch::new(vec![xattrs.into_stored(), input.into_stored()]).unwrap();
    assert!(matches!(validate_batch(&BodyGraph::empty(), batch), Err(BodyError::WrongTargetKind { .. })));
}

#[test]
fn graph_rejects_missing_edges_and_the_kind_manifest_is_acyclic() {
    let missing = LocalFileObservationRef::from_digest(
        Digest::parse(&format!("sha256:{}", "2".repeat(64))).unwrap(),
    );
    let input = validated_body(StaticArtifactPublishInput::new(
        ArtifactName::parse("app").unwrap(),
        missing,
        LogicalAddress::parse("local-file:///active/app").unwrap(),
    ).unwrap()).unwrap();
    let batch = BodyBatch::new(vec![input.into_stored()]).unwrap();
    assert!(matches!(validate_batch(&BodyGraph::empty(), batch), Err(BodyError::MissingReference { .. })));
    assert_eq!(validate_kind_edge_manifest(), Ok(()));
}

#[test]
fn every_protocol_kind_has_one_manifest_entry() {
    assert_eq!(BodyKind::ALL.len(), 29);
    assert_eq!(BodyKind::ALL.map(BodyKind::as_str), BODY_KIND_IDS);
}
```

- [ ] **Step 2: Run the test and observe the missing body API**

Run:

```bash
cargo test -p guild-effect-kernel --test body_graph
```

Expected: FAIL with unresolved import `guild_effect_kernel::body`.

- [ ] **Step 3: Define the typed body and immutable graph interfaces**

Implement these shapes with private fields and read-only accessors:

```rust
pub trait BodyTag { const KIND: BodyKind; }
pub trait BodySpec: Clone + Serialize {
    type Tag: BodyTag;
    fn edges(&self) -> Vec<TypedEdge>;
    fn validate_local(&self) -> Result<(), BodyError>;
}

pub struct BodyRef<K: BodyTag> { digest: Digest, marker: PhantomData<fn() -> K> }
pub enum ProtocolRef<P: BodyTag, S: BodyTag> {
    Publication { digest: BodyRef<P> },
    Separation { digest: BodyRef<S> },
}
pub struct ValidatedBody<P: BodySpec> { reference: BodyRef<P::Tag>, payload: P, stored: StoredBody }
pub struct StoredBody { digest: Digest, kind: BodyKind, canonical_bytes: Vec<u8>, edges: Vec<TypedEdge> }
pub struct BodyGraph { bodies: BTreeMap<Digest, StoredBody> }
pub struct BodyBatch { bodies: Vec<StoredBody> }
#[derive(Debug, thiserror::Error)]
pub enum BodyError {
    #[error("body-local validation failed: {0}")]
    Local(String),
    #[error("canonical body encoding failed: {0}")]
    Canonical(#[from] CanonicalError),
    #[error("body key does not equal its computed digest")]
    KeyMismatch { key: Digest, computed: Digest },
    #[error("one digest names different canonical bytes")]
    DigestCollision { digest: Digest },
    #[error("unknown body kind `{kind}`")]
    UnknownKind { kind: String },
    #[error("the frozen body kind `{kind}` has no payload decoder in this reviewed increment")]
    PayloadModuleUnavailable { kind: BodyKind },
    #[error("referenced body is missing")]
    MissingReference { source: Digest, target: Digest },
    #[error("typed edge resolves to the wrong body kind")]
    WrongTargetKind { source: BodyKind, expected: BodyKind, actual: BodyKind },
    #[error("body graph contains a cycle")]
    Cycle { digest: Digest },
    #[error("set-like body field is not strictly sorted and unique")]
    NonCanonicalSet,
}

impl<K: BodyTag> BodyRef<K> { pub fn from_digest(digest: Digest) -> Self; pub fn digest(&self) -> &Digest; }
impl<P: BodySpec> ValidatedBody<P> { pub fn reference(&self) -> &BodyRef<P::Tag>; pub fn payload(&self) -> &P; pub fn canonical_bytes(&self) -> &[u8]; pub fn into_stored(self) -> StoredBody; }
impl BodyGraph { pub fn empty() -> Self; pub fn from_canonical_entries(entries: BTreeMap<Digest, Vec<u8>>) -> Result<Self, BodyError>; pub fn get(&self, digest: &Digest) -> Option<&StoredBody>; }
impl BodyBatch { pub fn new(bodies: Vec<StoredBody>) -> Result<Self, BodyError>; }
pub fn validated_body<P: BodySpec>(payload: P) -> Result<ValidatedBody<P>, BodyError>;
pub fn validate_batch(base: &BodyGraph, batch: BodyBatch) -> Result<BodyGraph, BodyError>;
pub fn validate_kind_edge_manifest() -> Result<(), BodyError>;
```

`body.rs` defines an uninhabited marker tag and reference alias for each kind—for example `LocalFileObservationTag` plus `type LocalFileObservationRef = BodyRef<LocalFileObservationTag>` and `ResourceDeedTag` plus `type ResourceDeedRef = BodyRef<ResourceDeedTag>`. This lets an early input payload name a body kind whose sealed payload implementation arrives in a subsequent reviewed increment without weakening typed edges. `BodyRef<K>` serializes as its digest string; `from_digest` expresses an untrusted typed claim and does not prove the target kind. `ProtocolRef<P, S>` uses `#[serde(tag = "protocol", rename_all = "snake_case", deny_unknown_fields)]`; with digest `sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef`, its two exact shapes are `{ "protocol": "publication", "digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" }` and `{ "protocol": "separation", "digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" }`. Only `validated_body` computes identity, over exactly `{"body": <payload>, "kind": P::Tag::KIND}`. `BodyGraph::from_canonical_entries` is the untrusted replay entrypoint: it strict-decodes, recomputes each key, and delegates to the same two-pass graph validator. In Task 5 it decodes the schema/base payloads already implemented and returns `PayloadModuleUnavailable` for the frozen kinds owned by Tasks 6, 7, 10, and 14. Each owning task replaces its exact match arms when its payload lands; Task 14 deletes `PayloadModuleUnavailable`, and Task 15 exhaustively proves that every frozen kind decodes. `validate_batch` first recomputes every key and indexes all bytes, then walks every typed edge from batch roots across the combined base/batch graph, enforcing the §7.2 edge matrix and rejecting all cycles. `validate_kind_edge_manifest` performs a DFS over the complete kind-level edge manifest and fails if that static relation contains a cycle. Identical insertion is idempotent; same key/different bytes is corruption.

- [ ] **Step 4: Add the exact 29-kind enum and base protocol payloads**

`BodyKind` must have 29 exhaustive variants, `ALL: [BodyKind; 29]`, and `as_str()` values identical to `BODY_KIND_IDS`. Implement these closed base payloads now:

- `LocalFileObservation::{Absent, Present}` with the exact fields from §§6.3 and 7.1;
- `XattrValue { entries: NonEmptySortedSet<XattrEntry> }`;
- `StaticArtifactPublishInput`, `StaticArtifactPublishPrecondition`;
- `StaticArtifactSeparationInput`, `StaticArtifactSeparationPrecondition`;
- `OptionalValue<T>`, `ProtocolRef<P, S>`, `ExpectedState`, `PresentExpectedState`, and `AbsentExpectedState`.

Use `#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]` for closed state unions and `#[serde(rename_all = "camelCase", deny_unknown_fields)]` for structs. Implement one reusable `SortedUnique<T>` constructor that compares `canonical_bytes` and rejects out-of-order or duplicate elements; enforce maximum length 1,024 when protocol §7.1 requires it.

- [ ] **Step 5: Test every base payload and graph rule**

Add table-driven tests for each of the first nine body kinds, every base edge in §7.2, unknown fields, absent/present field confusion, unsorted xattrs, a present observation with an invalid optional xattr reference, and source/target address equality. Run:

```bash
cargo test -p guild-effect-kernel --test body_graph
cargo test -p guild-effect-kernel
```

Expected: all tests pass; the canonical absent-observation digest remains unchanged.

- [ ] **Step 6: Commit and checkpoint**

```bash
git add crates/guild-effect-kernel
git commit -m "feat(effect-kernel): validate the immutable body graph"
git push origin HEAD:design/guild-effect-kernel
```

---

### Task 6: Implement Authority, Permanent Bindings, Leases, Budgets, And Fences

**Files:**
- Create: `crates/guild-effect-kernel/src/authority.rs`
- Create: `crates/guild-effect-kernel/src/lease.rs`
- Modify: `crates/guild-effect-kernel/src/body.rs`
- Modify: `crates/guild-effect-kernel/src/lib.rs`
- Create: `crates/guild-effect-kernel/tests/authority_lease.rs`
- Modify: `crates/guild-effect-kernel/tests/support/mod.rs`

**Interfaces:**
- Consumes: typed bodies/graph from Task 5 and authority/identity/temporal/counter laws from protocol §§8.1–8.6.
- Produces: `AuthorityPolicy`, `InstallationEnrollment`, publication/separation warrant/approval/revocation types, `EffectLease`, `SeparationLease`, `IdempotencyBinding`, `SeparationBinding`, `LeaseProjection`, `ReservationMaterial`, `PreStartOutcome`, `AdmissionError`, `derive_resource_key`, `derive_effect_id`, and checked reservation/cancellation primitives used by projection and effect-family APIs.

- [ ] **Step 1: Write failing authority and lease boundary tests**

Add focused tests with these assertions:

```rust
#[test]
fn first_policy_requires_distinct_enrolled_authority() {
    let fixtures = fixtures::authority();
    assert!(fixtures.approve_as(fixtures.proposer_id()).is_err());
    assert!(fixtures.approve_as(fixtures.approver_id()).is_ok());
}

#[test]
fn lease_is_live_strictly_before_but_not_at_expiry() {
    let lease = fixtures::publication_lease_at("1000000000");
    assert!(lease.is_live_at(UnixNanoseconds::parse("5999999999").unwrap()));
    assert!(!lease.is_live_at(UnixNanoseconds::parse("6000000000").unwrap()));
}

```

Extend `tests/support/mod.rs` with `authority() -> AuthorityFixture` and `publication_lease_at(reserved_at: &str) -> EffectLease`. `AuthorityFixture` exposes only `proposer_id`, `approver_id`, and `approve_as`; build it from fixed principals (`proposer`, `approver`, `revoker`, `host-probe`), fixed nanosecond strings, fixed 64-hex nonces/incarnations, and local-file addresses. It must not read the clock or generate IDs.

In `lease.rs`'s private unit-test module, exercise the otherwise unreachable maximum counter directly:

```rust
#[test]
fn first_fence_is_one_and_exhaustion_is_closed() {
    assert_eq!(checked_next_fence(None).unwrap().get(), 1);
    assert_eq!(
        checked_next_fence(Some(U64Decimal::from_u64(u64::MAX))),
        Err(AdmissionError::CounterExhausted),
    );
}
```

Add a property test: reserve an arbitrary valid idempotency key twice against the same warrant/effect and assert the second result reuses the binding without changing budget/fence/lock maps; change one effect-identity field and assert `AdmissionError::IdempotencyConflict`.

- [ ] **Step 2: Run the test and verify authority/lease modules are missing**

```bash
cargo test -p guild-effect-kernel --test authority_lease
```

Expected: FAIL with unresolved imports `authority` and `lease`.

- [ ] **Step 3: Implement immutable policy and authority bodies**

Use type aliases `PrincipalId`, `WitnessId`, `BudgetKey`, `PolicyId`, and `InstallationId = Identifier`; `PolicyGeneration`, `Fence`, and `CustodyGeneration = U64Decimal`. Implement these exact private fields from protocol §7.1:

```rust
pub struct BudgetAmount(SafeUInt);
impl BudgetAmount {
    pub fn new(value: SafeUInt) -> Result<Self, ValidationError>;
    pub fn get(self) -> u64;
}
pub struct BudgetCapacity { key: BudgetKey, capacity: SafeUInt }
pub struct BudgetClaim { key: BudgetKey, amount: BudgetAmount }
pub enum EffectKind { StaticArtifactPublish, StaticArtifactSeparation }

pub struct AuthorityPolicy {
    policy_id: PolicyId,
    generation: PolicyGeneration,
    proposer_ids: SortedUnique<PrincipalId>,
    approver_ids: SortedUnique<PrincipalId>,
    revoker_ids: SortedUnique<PrincipalId>,
    witness_ids: SortedUnique<WitnessId>,
    require_distinct_approval_principal: bool,
    reservation_budgets: SortedUnique<BudgetCapacity>,
    start_budgets: SortedUnique<BudgetCapacity>,
    trusted_clock_id: Identifier,
    trusted_store_id: Identifier,
}
pub struct InstallationEnrollment {
    installation_id: InstallationId,
    incarnation: IncarnationId,
    policy_digest: AuthorityPolicyRef,
    enrolled_at: UnixNanoseconds,
}
pub struct PublicationWarrant {
    installation_digest: InstallationEnrollmentRef,
    policy_digest: AuthorityPolicyRef,
    policy_generation: PolicyGeneration,
    effect_kind: EffectKind,
    proposer_id: PrincipalId,
    input_digest: StaticArtifactPublishInputRef,
    precondition_digest: StaticArtifactPublishPreconditionRef,
    idempotency_key: IdempotencyKey,
    resource_keys: [ResourceKey; 2],
    reservation_budget: BudgetClaim,
    start_budget: BudgetClaim,
    issued_at: UnixNanoseconds,
    expires_at: UnixNanoseconds,
    nonce: Hex256,
}
pub struct PublicationApproval {
    warrant_digest: PublicationWarrantRef,
    approver_id: PrincipalId,
    approved_at: UnixNanoseconds,
}
pub struct PublicationRevocation {
    warrant_digest: PublicationWarrantRef,
    revoker_id: PrincipalId,
    revoked_at: UnixNanoseconds,
    reason: Identifier,
}
pub struct SeparationWarrant {
    installation_digest: InstallationEnrollmentRef,
    policy_digest: AuthorityPolicyRef,
    policy_generation: PolicyGeneration,
    effect_kind: EffectKind,
    proposer_id: PrincipalId,
    input_digest: StaticArtifactSeparationInputRef,
    precondition_digest: StaticArtifactSeparationPreconditionRef,
    idempotency_key: IdempotencyKey,
    resource_keys: [ResourceKey; 2],
    reservation_budget: BudgetClaim,
    start_budget: BudgetClaim,
    issued_at: UnixNanoseconds,
    expires_at: UnixNanoseconds,
    nonce: Hex256,
}
pub struct SeparationApproval {
    warrant_digest: SeparationWarrantRef,
    approver_id: PrincipalId,
    approved_at: UnixNanoseconds,
}
pub struct SeparationRevocation {
    warrant_digest: SeparationWarrantRef,
    revoker_id: PrincipalId,
    revoked_at: UnixNanoseconds,
    reason: Identifier,
}
```

Use `camelCase`/`deny_unknown_fields` wire encoding. `EffectKind` serializes exactly as `static_artifact_publish` or `static_artifact_separation`; `BudgetAmount` rejects zero. Constructors must require each warrant's matching literal `EffectKind`, generation `0`, nonempty sorted principal sets, distinct proposer/approver, enrolled roles, immutable policy identity, `issued_at < expires_at`, approval in `[issued_at, expires_at)`, revocation at or after approval, and event time nondecrease. The caller supplies every timestamp and the nonce.

Implement `BodySpec` and strict replay decoding for enrollment, policy, both warrant/approval/revocation families, both bindings, and both leases; replace exactly those `PayloadModuleUnavailable` match arms and add their §7.2 typed edges.

- [ ] **Step 4: Implement exact effect/resource identity derivation**

Expose only derivation, never arbitrary construction:

```rust
pub fn derive_resource_key(logical_address: &LogicalAddress) -> Result<ResourceKey, CanonicalError>;

pub fn derive_effect_id(
    installation_digest: &Digest,
    warrant_digest: &Digest,
    effect_kind: EffectKind,
    resource_keys: &SortedUnique<ResourceKey>,
    input_digest: &Digest,
    precondition_digest: &Digest,
) -> Result<EffectId, CanonicalError>;
```

The resource preimage is the exact struct `{ effectFamily: "static_artifact", logicalAddress: &LogicalAddress }`, serialized by JCS to the two corresponding lower-camel-case members. The effect preimage is exactly `{ installationDigest, warrantDigest, effectKind, resourceKeys, inputDigest, preconditionDigest }` under JCS ordering. The idempotency key is not a separate effect-ID field because it is already inside the warrant.

- [ ] **Step 5: Implement permanent binding, reservation, budget, fence, and lock laws**

Define the closed errors and state:

```rust
pub struct ResourceFence { resource_key: ResourceKey, fence: Fence }
pub struct BudgetHold { key: BudgetKey, amount: BudgetAmount }
pub struct IdempotencyBinding {
    idempotency_key: IdempotencyKey,
    effect_id: EffectId,
    warrant_digest: PublicationWarrantRef,
}
pub struct EffectLease {
    effect_id: EffectId,
    binding_digest: IdempotencyBindingRef,
    resource_fences: [ResourceFence; 2],
    reservation_budget_hold: BudgetHold,
    start_budget_hold: BudgetHold,
    reserved_at: UnixNanoseconds,
    expires_at: UnixNanoseconds,
}
pub struct SeparationBinding {
    idempotency_key: IdempotencyKey,
    effect_id: EffectId,
    warrant_digest: SeparationWarrantRef,
}
pub struct SeparationLease {
    effect_id: EffectId,
    binding_digest: SeparationBindingRef,
    resource_fences: [ResourceFence; 2],
    reservation_budget_hold: BudgetHold,
    start_budget_hold: BudgetHold,
    reserved_at: UnixNanoseconds,
    expires_at: UnixNanoseconds,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AdmissionError {
    #[error("authority refused")]
    AuthorityRefused,
    #[error("warrant expired")]
    WarrantExpired,
    #[error("warrant revoked")]
    WarrantRevoked,
    #[error("warrant already spent")]
    WarrantSpent,
    #[error("idempotency binding conflicts with the requested effect")]
    IdempotencyConflict,
    #[error("resource is locked or claimed")]
    ResourceConflict,
    #[error("budget is unavailable")]
    BudgetUnavailable,
    #[error("counter exhausted")]
    CounterExhausted,
    #[error("event sequence exhausted")]
    SequenceExhausted,
    #[error("precondition refused")]
    PreconditionRefused,
    #[error("transition time moved backwards")]
    TimeRegression,
    #[error("body graph is invalid: {0}")]
    Body(#[from] BodyError),
}

pub enum BudgetClass { Reservation, Start }
pub enum BudgetState { Available, Held, Consumed }
pub enum PreStartEffectState { Reserved, Prepared }
pub enum PreStartResult { NotAttempted, PreparedOnly }
pub enum PreStartReason {
    RequestDisconnected, ReservationDeadline, AuthorizationIneligible,
    PeerIdentityChanged, PreconditionChanged, RecoveryOrphaned,
    BudgetUnavailable, SeparationPreconditionRefused,
}
pub type BindingRef = ProtocolRef<IdempotencyBindingTag, SeparationBindingTag>;
pub struct PreStartOutcome {
    result: PreStartResult,
    reason: PreStartReason,
    binding_digest: OptionalValue<BindingRef>,
}
pub struct ReservationMaterial<B, L> { binding: ValidatedBody<B>, lease: ValidatedBody<L>, effect_id: EffectId, delta: LeaseDelta }
```

`LeaseProjection` has private maps for permanent idempotency bindings, spent warrant digests, `(BudgetClass, BudgetKey)` accounts, resource fences, resource locks, and terminal sequence reserve. It exposes read-only lookup methods plus crate-private transition application. Bindings and leases have private fields, implement `Serialize` but not public `Deserialize`, and have crate-private validated replay decoders. Reservation holds both namespaced claims, assigns each resource its checked next fence (first is `1`), acquires both locks, permanently spends the warrant, and sets expiry to exactly `reserved_at + 5_000_000_000`. Cancellation releases both holds and locks while retaining binding, spent-warrant state, and fences. Start consumes both holds and retains locks. Terminalization releases locks and never replenishes consumed units.

Implement the terminal sequence-reserve arithmetic from §8.6 exactly: three slots per unterminated publication start and two per unterminated separation start; checked addition/subtraction; one effect terminalized per bundle.

- [ ] **Step 6: Verify all exact boundary cases**

Add tests for equality-at-expiry, effective revocation, same-principal approval, budget class separation, duplicate hold/consume, pre-start cancellation reason vocabulary, first/last fence, generation/sequence overflow, two-key atomic conflict, lock retention through started state, and reserved terminal slots under interleaved ordinary events.

Run:

```bash
cargo test -p guild-effect-kernel --test authority_lease
cargo test -p guild-effect-kernel
cargo clippy -p guild-effect-kernel --all-targets --all-features -- -D warnings
```

Expected: all commands exit zero.

- [ ] **Step 7: Commit and checkpoint**

```bash
git add crates/guild-effect-kernel
git commit -m "feat(effect-kernel): enforce authority leases and fences"
git push origin HEAD:design/guild-effect-kernel
```

---

### Task 7: Implement Evidence, Causality, Receipts, Deeds, And Custody Derivation

**Files:**
- Create: `crates/guild-effect-kernel/src/evidence.rs`
- Modify: `crates/guild-effect-kernel/src/body.rs`
- Modify: `crates/guild-effect-kernel/src/lib.rs`
- Create: `crates/guild-effect-kernel/tests/evidence.rs`
- Modify: `crates/guild-effect-kernel/tests/support/mod.rs`

**Interfaces:**
- Consumes: started-effect facts, before observations, witness enrollment, mutation mode, and the exhaustive tables in protocol §§9.1–9.5.
- Produces: `ObservationAttempt`, `WitnessStatus`, `ObservationEvidence`, `EvidenceLimitation`, `CommandReport`, `MutationMode`, postcondition/causality enums, result/reason/state enums, sealed publication/separation evidence and receipt bodies, `ResourceDeed`, `CustodyRecord`, `RecoveryAssessment`, `derive_publication_evidence`, `classify_publication`, `derive_separation_evidence`, and `classify_separation` as crate-internal primitives.

- [ ] **Step 1: Write table-driven failing classifier tests**

Define one test case struct per classifier:

```rust
struct PublicationCase {
    name: &'static str,
    command_report: CommandReport,
    source_after: ObservationEvidence,
    target_after: ObservationEvidence,
    mutation_mode: MutationMode,
    expected_postcondition: PublicationPostcondition,
    expected_causality: CausalityOutcome,
    expected_state: ReceiptState,
    expected_reason: ReceiptReason,
    deed_expected: bool,
}
```

Instantiate at least one row for each of the 12 publication classification priorities and each of the six separation priorities from §9.4. Add independent postcondition and causality cases so “matching bytes/different incarnation” produces `ExactRequested` plus `DifferentIncarnation`, never `Verified`. Add cases proving `CommandReport::ReportedSuccess` cannot override contradictory evidence and that `NotAvailable` is rejected outside recovery.

- [ ] **Step 2: Run the evidence tests and observe the missing API**

```bash
cargo test -p guild-effect-kernel --test evidence
```

Expected: FAIL with unresolved import `guild_effect_kernel::evidence`.

- [ ] **Step 3: Implement the closed evidence input and vocabulary**

Use exact snake-case serialization:

```rust
pub enum EvidenceLimitation {
    WitnessUnavailable, UnsupportedIdentity, NonAtomicExternalOperation,
    StaleObservation, ConflictingObservation,
}
pub enum CommandReport { ReportedSuccess, ReportedNoEffect, ReportedUncertain, NotAvailable }
pub enum PublicationPostcondition { ExactRequested, AuthoritativeAbsence, PriorStateUnchanged, ContentMismatch, Ambiguous }
pub enum SeparationPostcondition { ExactQuarantine, NoMove, Ambiguous }
pub enum CausalityOutcome { ExactPreparedIncarnation, DifferentIncarnation, DuplicateIncarnation, Ambiguous, Unsupported }
pub enum ReceiptState { Verified, Failed, Indeterminate }
pub enum MutationMode { Conditional, Unconditional }
pub enum WitnessStatus { AuthenticatedEnrolled, Unauthenticated, Unenrolled }
pub enum OperationResult {
    NotAttempted, PreparedOnly,
    PublishReportedSuccess, PublishReportedNoEffect, PublishReportedUncertain,
    PublishRecovered, QuarantineReportedSuccess, QuarantineReportedNoEffect,
    QuarantineReportedUncertain, QuarantineRecovered,
}
pub enum ReceiptReason {
    ArtifactVerified, SeparationVerified, SourceChanged, SourceInvalidAfterStart,
    DigestMismatchAfterStart, PublicationNoEffect, AuthoritativeAbsence,
    SeparationPreconditionRefused, SeparationNoMove, WitnessUnavailable,
    PublicationAmbiguous, IncarnationAmbiguous, DuplicateIncarnation,
    SeparationAmbiguous, UnsupportedIdentity,
}
pub enum ObservationAttempt {
    Observed { observation: ValidatedBody<LocalFileObservation>, witness: WitnessStatus },
    Unavailable { logical_address: LogicalAddress, witness_id: WitnessId, attempted_at: UnixNanoseconds },
    Unsupported { logical_address: LogicalAddress, witness_id: WitnessId, attempted_at: UnixNanoseconds },
    Conflicting { observations: SortedUnique<ValidatedBody<LocalFileObservation>>, witness: WitnessStatus, attempted_at: UnixNanoseconds },
}
#[derive(Debug, thiserror::Error)]
pub enum EvidenceError {
    #[error("effect is not in the required started state")]
    NotStarted,
    #[error("evidence references do not match the durable start")]
    StartReferenceMismatch,
    #[error("an observation occurs after assessment")]
    ObservationAfterAssessment,
    #[error("not_available is legal only during recovery")]
    RecoveryReportOnLivePath,
    #[error("conflicting observations do not name one address")]
    ConflictingAddress,
    #[error("custody generation is exhausted")]
    GenerationExhausted,
    #[error("body validation failed: {0}")]
    Body(#[from] BodyError),
}
```

Implement `ObservationEvidence` as the exact observed/unavailable/unsupported/conflicting union from §7.1. Conflicting observation refs must be sorted, unique, contain at least two entries, and resolve to the same logical address. An unavailable or unsupported observation is not authoritative absence.

- [ ] **Step 4: Derive limitations, postconditions, and causality without caller-selected classifications**

The public transition APIs will accept probe attempts and authenticated witness context only. Keep these functions `pub(crate)`:

```rust
pub(crate) fn derive_publication_evidence(input: PublicationEvidenceInput<'_>) -> Result<PublicationEvidenceMaterial, EvidenceError>;
pub(crate) fn classify_publication(material: PublicationEvidenceMaterial) -> Result<PublicationClassification, EvidenceError>;
pub(crate) fn derive_separation_evidence(input: SeparationEvidenceInput<'_>) -> Result<SeparationEvidenceMaterial, EvidenceError>;
pub(crate) fn classify_separation(material: SeparationEvidenceMaterial) -> Result<SeparationClassification, EvidenceError>;
```

Apply §9.1's limitation derivation exactly, then evaluate the postcondition, causality, and terminal tables top-to-bottom. Reject timestamps after `assessed_at`; derive `StaleObservation` for after-evidence before start. `MutationMode::Unconditional` always derives `NonAtomicExternalOperation`.

- [ ] **Step 5: Seal receipts and deed proof**

Implement the exact `PublicationEvidence`, `CausalityAssessment`, `EffectReceipt`, `ResourceDeed`, `SeparationEvidence`, `SeparationReceipt`, `CustodyRecord`, and `RecoveryAssessment` fields from §7.1. Classification fields, terminal timestamps, next custody generation, and custody fields are derived, never accepted from callers.

Implement `BodySpec` and strict replay decoding for those eight body kinds; replace exactly their `PayloadModuleUnavailable` match arms and add every corresponding §7.2 typed edge.

`ResourceDeed` must have no public constructor and no public deserializer. A private `DeedProof` is minted only for §9.4 publication row 5 after all six §9.3 checks pass. Add this compile-fail doctest to `ResourceDeed`:

```rust,compile_fail
use guild_effect_kernel::evidence::ResourceDeed;
let _forged = ResourceDeed::new();
```

Derive `CustodyRecord` exactly from §9.5: publication gives generation `0` for no prior record or checked `g + 1` from `Absent`; every started separation uses checked `g + 1` and retains the publication deed.

- [ ] **Step 6: Run the matrix, doctest, and lint gates**

```bash
cargo test -p guild-effect-kernel --test evidence
cargo test -p guild-effect-kernel --doc
cargo clippy -p guild-effect-kernel --all-targets --all-features -- -D warnings
```

Expected: every table row passes, the forged deed doctest fails to compile as expected, and Clippy is clean.

- [ ] **Step 7: Commit and checkpoint**

```bash
git add crates/guild-effect-kernel
git commit -m "feat(effect-kernel): derive receipts deeds and custody"
git push origin HEAD:design/guild-effect-kernel
```

---

### Task 8: Implement Typed Events, Anchored Chains, And Atomic Transition Bundles

**Files:**
- Create: `crates/guild-effect-kernel/src/event.rs`
- Create: `crates/guild-effect-kernel/src/store.rs`
- Modify: `crates/guild-effect-kernel/src/lib.rs`
- Create: `crates/guild-effect-kernel/tests/event_chain.rs`
- Modify: `crates/guild-effect-kernel/tests/support/mod.rs`

**Interfaces:**
- Consumes: body graph and scalar/canonical identities; all event payloads/order rules from protocol §§10.1–10.3 and transition-bundle shape from §10.5.
- Produces: `EventType`, 26 typed payload structs, `EventPayload`, `PreviousEvent`, `EventPreimage`, `EventEnvelope`, `TrustedHead`, `ExpectedHead`, non-`Clone` `TrustedCommitOutcome`, `ImmutableStore`, `TransitionBundle`, `ChainError`, `validate_chain`, and `validate_bundle`.

- [ ] **Step 1: Write event identity and corruption tests**

Create deterministic genesis and two-event chains, then assert:

```rust
assert_eq!(EventType::ALL.len(), 26);
assert_eq!(EventType::ALL.map(EventType::as_str), EVENT_TYPE_IDS);
assert_eq!(genesis.preimage().schema_version(), "jidoka.dev/events/v1");
assert_eq!(genesis.preimage().sequence().get(), 0);
assert!(matches!(genesis.preimage().previous_event(), PreviousEvent::Genesis));
```

Add concrete mutations that independently cause `HeadMismatch`, `SequenceDiscontinuity`, `PreviousLinkMismatch`, `TimeRegression`, `DigestMismatch`, `Fork`, `Gap`, `TypeConfusedPayload`, `DuplicateMember`, `UnknownEventType`, and `TruncatedTail`. Test that an all-zero digest string is not accepted as genesis.

- [ ] **Step 2: Run the tests and observe missing event/store modules**

```bash
cargo test -p guild-effect-kernel --test event_chain
```

Expected: FAIL with unresolved imports `event` and `store`.

- [ ] **Step 3: Implement the 26 closed typed payloads and custom event decoding**

Define:

```rust
pub enum PreviousEvent { Genesis, Previous { digest: Digest } }
pub struct EventPreimage {
    schema_version: String,
    sequence: U64Decimal,
    previous_event: PreviousEvent,
    installation_digest: InstallationEnrollmentRef,
    occurred_at: UnixNanoseconds,
    event_type: EventType,
    payload: EventPayload,
}
pub struct EventEnvelope { digest: Digest, preimage: EventPreimage }
pub enum EventPayload {
    InstallationEnrolled(InstallationEnrolledPayload),
    WarrantProposed(WarrantProposedPayload),
    WarrantApproved(WarrantApprovedPayload),
    WarrantRevoked(WarrantRevokedPayload),
    WarrantExpired(WarrantExpiredPayload),
    EffectReserved(EffectReservedPayload),
    EffectCancelledBeforeStart(EffectCancelledBeforeStartPayload),
    EffectStarted(EffectStartedPayload),
    ArtifactPrepared(ArtifactPreparedPayload),
    ArtifactPublished(ArtifactPublishedPayload),
    ArtifactPublishedRecovered(ArtifactPublishedRecoveredPayload),
    EffectVerified(EffectVerifiedPayload),
    EffectFailed(EffectFailedPayload),
    EffectIndeterminate(EffectIndeterminatePayload),
    SeparationWarrantProposed(SeparationWarrantProposedPayload),
    SeparationWarrantApproved(SeparationWarrantApprovedPayload),
    SeparationWarrantRevoked(SeparationWarrantRevokedPayload),
    SeparationWarrantExpired(SeparationWarrantExpiredPayload),
    SeparationReserved(SeparationReservedPayload),
    SeparationCancelledBeforeStart(SeparationCancelledBeforeStartPayload),
    SeparationStarted(SeparationStartedPayload),
    SeparationVerified(SeparationVerifiedPayload),
    SeparationFailed(SeparationFailedPayload),
    SeparationIndeterminate(SeparationIndeterminatePayload),
    CustodyAbsent(CustodyAbsentPayload),
    CustodyDisputed(CustodyDisputedPayload),
}
pub struct TrustedHead {
    installation_digest: InstallationEnrollmentRef,
    head_digest: Digest,
    anchored_at: UnixNanoseconds,
    trusted_store_id: Identifier,
}
```

Serialize `PreviousEvent` as `{ "state": "genesis" }` or, for example, `{ "state": "previous", "digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" }`. `EventPreimage` must serialize exactly `schemaVersion`, `sequence`, `previousEvent`, `installationDigest`, `occurredAt`, `eventType`, and `payload`. Implement custom `Deserialize`: parse `eventType`, then deserialize `payload` into that event's exact closed struct; never retain an open `serde_json::Value` as a trusted payload. Implement every payload field exactly as listed in §10.3.

- [ ] **Step 4: Implement canonical envelopes and anchored chain validation**

Only crate transition code may create an event:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ChainError {
    #[error("event chain is empty")]
    Empty,
    #[error("trusted head does not match the chain")]
    HeadMismatch,
    #[error("event sequence is discontinuous")]
    SequenceDiscontinuity,
    #[error("previous-event link is invalid")]
    PreviousLinkMismatch,
    #[error("event time decreased")]
    TimeRegression,
    #[error("event map key or envelope digest is invalid")]
    DigestMismatch,
    #[error("event history forks")]
    Fork,
    #[error("event history has a gap")]
    Gap,
    #[error("event payload is not valid for its event type")]
    TypeConfusedPayload,
    #[error("event JSON contains a duplicate member")]
    DuplicateMember,
    #[error("unknown event type")]
    UnknownEventType,
    #[error("event tail is truncated relative to the trusted head")]
    TruncatedTail,
    #[error("event chain is not rooted in canonical enrollment genesis")]
    InvalidGenesis,
    #[error("transition bundle is not atomic or internally linked")]
    InvalidBundle,
    #[error("body graph validation failed: {0}")]
    Body(#[from] BodyError),
}

pub(crate) fn seal_event(preimage: EventPreimage) -> Result<EventEnvelope, ChainError>;
pub fn validate_chain(
    bodies: &BodyGraph,
    events: &BTreeMap<Digest, EventEnvelope>,
    expected_head: &TrustedHead,
) -> Result<Vec<EventEnvelope>, ChainError>;
```

Hash canonical `EventPreimage` bytes only; the stored envelope's digest is not self-hashed. Validation starts from the independently supplied expected head, walks backward to explicit genesis, rejects forks/gaps/cycles, reverses into sequence order, and validates enrollment digest, nondecreasing times, checked sequences, typed payload refs, and exact map keys. An internally claimed head without an independent `TrustedHead` may be checked for consistency but must not be called fresh.

- [ ] **Step 5: Implement immutable stores and all-or-nothing proposal bundles**

```rust
pub enum ExpectedHead { Empty, Present(Digest) }
pub struct TransitionBundle {
    expected_head: ExpectedHead,
    new_bodies: Vec<StoredBody>,
    events: Vec<EventEnvelope>,
    new_head: Digest,
}
pub struct ImmutableStore {
    bodies: BodyGraph,
    events: BTreeMap<Digest, EventEnvelope>,
    head: Option<Digest>,
}
pub enum TrustedCommitOutcome {
    Committed { expected_head: ExpectedHead, new_head: Digest },
    HeadMismatch { current_head: Option<Digest> },
    Unknown,
}
```

`validate_bundle` proves first-link/head agreement, internal event links, same transition time across bundle events, sorted unique bodies, all refs resolved in base or bundle, final digest equality, legal genesis-only empty transition, and sequence-capacity reservation. It returns a validated proposal; it does not write storage. `TrustedCommitOutcome` deliberately does not implement `Clone`; an authenticated store adapter must return the actual one-shot CAS outcome to the coordinator, and a later head read is not a substitute. Provide a test-only in-memory `apply_committed_for_test` that applies the entire bundle or none, and keep it out of the public API.

- [ ] **Step 6: Run focused and full tests**

```bash
cargo test -p guild-effect-kernel --test event_chain
cargo test -p guild-effect-kernel
cargo clippy -p guild-effect-kernel --all-targets --all-features -- -D warnings
```

Expected: all commands exit zero.

- [ ] **Step 7: Commit and checkpoint**

```bash
git add crates/guild-effect-kernel
git commit -m "feat(effect-kernel): validate anchored event transitions"
git push origin HEAD:design/guild-effect-kernel
```

---

### Task 9: Implement Exhaustive Replay And Projection

**Files:**
- Create: `crates/guild-effect-kernel/src/projection.rs`
- Modify: `crates/guild-effect-kernel/src/lib.rs`
- Create: `crates/guild-effect-kernel/tests/projection.rs`
- Modify: `crates/guild-effect-kernel/tests/support/mod.rs`

**Interfaces:**
- Consumes: validated ordered events/body graph from Task 8 and the complete transition relation/address-claim rules from protocol §§9.5 and 10.4.
- Produces: `Projection`, `EffectState`, `WarrantState`, `CustodyState`, `AddressClaim`, `ProjectionError`, `replay`, and crate-private `apply_bundle` used to prove incremental/full replay equivalence.

- [ ] **Step 1: Write one legal and one illegal case for every transition row**

Build a table keyed by all 26 `EventType` values. For each entry, construct the smallest legal prior projection and event/bundle, assert the documented state mutation, then mutate exactly one required prior fact and assert the expected closed `ProjectionError`. The table must include adjacency cases for publication report/terminal/custody bundles and separation indeterminate/custody-disputed bundles.

Add these explicit tests:

```rust
mod support;

use guild_effect_kernel::event::EventType;
use guild_effect_kernel::projection::{ProjectionError, replay};

#[test]
fn complete_chain_may_not_end_on_a_required_middle_event() {
    let history = support::publication_failed_without_custody_absent();
    assert_eq!(
        replay(history.graph(), history.events(), history.trusted_head()).unwrap_err(),
        ProjectionError::IncompleteAtomicSequence { expected: EventType::CustodyAbsent },
    );
}

#[test]
fn two_current_records_may_not_claim_the_same_address() {
    let history = support::history_with_two_current_claims_for_one_address();
    assert!(matches!(
        replay(history.graph(), history.events(), history.trusted_head()),
        Err(ProjectionError::AddressClaimConflict { .. }),
    ));
}

#[test]
fn incremental_projection_equals_full_replay() {
    let history = support::publication_verified_history();
    let full = replay(history.graph(), history.events(), history.trusted_head()).unwrap();
    assert_eq!(history.incremental_projection(), &full);
}
```

Extend test support with a `LawfulHistory` that stores committed bundles and exposes `graph`, `events`, `trusted_head`, and `incremental_projection`. Build its normal state through lawful transition APIs. The two corruption helpers start from that lawful state, mutate one canonical body/event fact, recompute the enclosing untrusted bytes/digests needed to reach projection validation, and preserve every unrelated fact.

- [ ] **Step 2: Run the tests and observe the missing projection module**

```bash
cargo test -p guild-effect-kernel --test projection
```

Expected: FAIL with unresolved import `guild_effect_kernel::projection`.

- [ ] **Step 3: Implement the complete projection state**

`Projection` owns, with private maps and read-only queries:

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectionError {
    #[error("chain validation failed: {0}")]
    Chain(String),
    #[error("event is illegal from the projected prior state")]
    IllegalTransition { event_type: EventType },
    #[error("semantic event is duplicated for its subject")]
    DuplicateTransition { event_type: EventType, subject: Digest },
    #[error("atomic event sequence is incomplete")]
    IncompleteAtomicSequence { expected: EventType },
    #[error("budget projection underflowed, overflowed, or changed state illegally")]
    BudgetInvariant,
    #[error("resource fence is stale")]
    StaleFence { resource_key: ResourceKey },
    #[error("custody generation skipped or overflowed")]
    CustodyGeneration,
    #[error("two current custody records claim one address")]
    AddressClaimConflict { resource_key: ResourceKey },
    #[error("receipt or deed lacks its required proof")]
    ProofInvariant,
}

pub struct Projection {
    installation: InstallationProjection,
    publication_warrants: BTreeMap<Digest, WarrantProjection>,
    separation_warrants: BTreeMap<Digest, WarrantProjection>,
    bindings: BTreeMap<IdempotencyKey, BindingProjection>,
    effects: BTreeMap<EffectId, EffectProjection>,
    budgets: BudgetProjection,
    resource_fences: BTreeMap<ResourceKey, Fence>,
    resource_locks: BTreeMap<ResourceKey, ResourceLock>,
    terminal_sequence_reserve: U64Decimal,
    receipts: ReceiptProjection,
    deeds: BTreeMap<Digest, ResourceDeedProjection>,
    custody: BTreeMap<ResourceKey, CustodyProjection>,
    address_claims: BTreeMap<ResourceKey, AddressClaim>,
    head: Digest,
    head_sequence: U64Decimal,
    head_time: UnixNanoseconds,
}

pub fn replay(
    bodies: &BodyGraph,
    events: &BTreeMap<Digest, EventEnvelope>,
    expected_head: &TrustedHead,
) -> Result<Projection, ProjectionError>;
```

Do not cache facts that cannot be recomputed from the chain. `replay` must first validate the body graph and anchored chain, then fold exactly the §10.4.1 table. Crate-private replay decoding may reconstruct sealed values only after canonical identity, kind, graph, and event authority are validated.

- [ ] **Step 4: Enforce atomic adjacency and address-claim replacement**

Validate each `TransitionBundle` against a cloned projection and publish the clone only after the whole ordered bundle succeeds. A required middle event is never visible at an authenticated head. Derive address claims exactly from §9.5, including disputed publication source/target and disputed separation active/quarantine roles; reject claim collisions and generation skips.

- [ ] **Step 5: Add generated legal histories and illegal mutations**

Use `proptest` to generate interleavings of independent proposal/approval/reservation/cancellation histories plus bounded complete publication histories. For every legal history, compare incremental projection with `replay`. Mutate one event link, body ref, fence, budget state, generation, or semantic uniqueness fact and require rejection. Keep a 256-case suite in the normal test and mark a 4,096-case version `#[ignore = "extended property suite"]`; the next step runs both explicitly. Promote any discovered minimal failure into a fixed regression test with explicit input bytes.

- [ ] **Step 6: Run replay and property tests**

```bash
cargo test -p guild-effect-kernel --test projection
cargo test -p guild-effect-kernel --test projection -- --ignored
cargo clippy -p guild-effect-kernel --all-targets --all-features -- -D warnings
```

Expected: normal and extended property suites pass; Clippy is clean.

- [ ] **Step 7: Commit and checkpoint**

```bash
git add crates/guild-effect-kernel
git commit -m "feat(effect-kernel): replay exhaustive effect history"
git push origin HEAD:design/guild-effect-kernel
```

---

### Task 10: Implement Publication Admission Through Durable Start

**Files:**
- Create: `crates/guild-effect-kernel/src/publication.rs`
- Modify: `crates/guild-effect-kernel/src/body.rs`
- Modify: `crates/guild-effect-kernel/src/lib.rs`
- Create: `crates/guild-effect-kernel/tests/publication_admission.rs`
- Modify: `crates/guild-effect-kernel/tests/support/mod.rs`

**Interfaces:**
- Consumes: current `Projection`, trusted caller-supplied times, authenticated before observations, and exact publication admission law from protocol §§8 and 11.1.
- Produces: `propose_publication`, `approve_publication`, `revoke_publication`, `expire_publication`, `reserve_publication`, `prepare_publication`, `start_publication`, `cancel_publication`, `PublicationReservation`, `PublicationPreparation`, `PendingPublicationStart`, and opaque non-`Clone` `PublicationPermit`.

- [ ] **Step 1: Write end-to-start and refusal tests**

Build one lawful chain from enrollment through preparation. Assert proposal, approval, reservation, preparation, and start each return one independently valid `TransitionBundle`. Add one test per admission matrix row: no record allows generation `0`; exact `Absent(g)` allows checked `g + 1`; `Owned`, `Quarantined`, and `Disputed` refuse. Add tests for claimed staging source, source/target equality, stale target expectation, changed source incarnation, expired/revoked warrant, stale fence, exhausted budget, stale observation over five seconds, and lease equality boundary.

Add this start barrier test:

```rust
mod support;

#[test]
fn only_the_exact_successful_start_cas_outcome_yields_a_permit() {
    let winner = support::pending_publication_start();
    let expected_head = winner.bundle().expected_head().clone();
    let new_head = winner.bundle().new_head().clone();
    let loser = support::pending_publication_start();

    let permit = winner.resolve_commit(TrustedCommitOutcome::Committed {
        expected_head,
        new_head: new_head.clone(),
    }).unwrap();
    assert_eq!(permit.effect_id(), support::publication_effect_id());
    assert!(loser.resolve_commit(TrustedCommitOutcome::HeadMismatch {
        current_head: Some(new_head),
    }).is_err());
}
```

Add a compile-fail doctest proving `PublicationPermit` cannot be cloned or directly constructed.

- [ ] **Step 2: Run the tests and observe the missing publication module**

```bash
cargo test -p guild-effect-kernel --test publication_admission
```

Expected: FAIL with unresolved import `guild_effect_kernel::publication`.

- [ ] **Step 3: Implement proposal, approval, reservation, and preparation**

Use explicit request values with no provider or Guild types:

```rust
pub struct PublicationProposal {
    pub proposer_id: PrincipalId,
    pub input: ValidatedBody<StaticArtifactPublishInput>,
    pub precondition: ValidatedBody<StaticArtifactPublishPrecondition>,
    pub idempotency_key: IdempotencyKey,
    pub reservation_budget: BudgetClaim,
    pub start_budget: BudgetClaim,
    pub issued_at: UnixNanoseconds,
    pub expires_at: UnixNanoseconds,
    pub nonce: Hex256,
    pub transition_at: UnixNanoseconds,
}
pub struct PublicationApprovalRequest {
    pub warrant_digest: PublicationWarrantRef,
    pub approver_id: PrincipalId,
    pub approved_at: UnixNanoseconds,
}
pub struct PublicationRevocationRequest {
    pub warrant_digest: PublicationWarrantRef,
    pub revoker_id: PrincipalId,
    pub reason: Identifier,
    pub revoked_at: UnixNanoseconds,
}
pub struct PublicationExpiryRequest {
    pub warrant_digest: PublicationWarrantRef,
    pub transition_at: UnixNanoseconds,
}
pub struct PublicationReservationRequest {
    pub warrant_digest: PublicationWarrantRef,
    pub reserved_at: UnixNanoseconds,
}
pub struct PublicationPreparationRequest {
    pub effect_id: EffectId,
    pub source_observation: ObservationAttempt,
    pub target_observation: ObservationAttempt,
    pub prepared_at: UnixNanoseconds,
}
pub struct PreparedArtifact {
    effect_id: EffectId,
    binding_digest: IdempotencyBindingRef,
    input_digest: StaticArtifactPublishInputRef,
    source_before_observation_digest: LocalFileObservationRef,
    target_before_observation_digest: LocalFileObservationRef,
    content_digest: RawDigest,
    byte_length: ByteLength,
    prepared_incarnation: IncarnationId,
    prepared_at: UnixNanoseconds,
}

pub fn propose_publication(view: &Projection, request: PublicationProposal) -> Result<TransitionBundle, AdmissionError>;
pub fn approve_publication(view: &Projection, request: PublicationApprovalRequest) -> Result<TransitionBundle, AdmissionError>;
pub fn revoke_publication(view: &Projection, request: PublicationRevocationRequest) -> Result<TransitionBundle, AdmissionError>;
pub fn expire_publication(view: &Projection, request: PublicationExpiryRequest) -> Result<TransitionBundle, AdmissionError>;
pub fn reserve_publication(view: &Projection, request: PublicationReservationRequest) -> Result<PublicationReservation, AdmissionError>;
pub fn prepare_publication(view: &Projection, request: PublicationPreparationRequest) -> Result<PublicationPreparation, AdmissionError>;
```

Each function derives every body/event field that can be derived, validates exact refs against the current graph/projection, uses the caller's trusted transition time, and returns values only. Preparation requires fresh authenticated source-present and target observations, commits both observations plus `PreparedArtifact`, and refuses if the prepared incarnation is already visible at target.

Implement `BodySpec` and strict replay decoding for `prepared-artifact/v1`, replacing that exact `PayloadModuleUnavailable` arm and registering its binding/input/two-observation edges.

- [ ] **Step 4: Implement the durable-start barrier**

```rust
pub struct PublicationStartRequest {
    pub effect_id: EffectId,
    pub source_observation: ObservationAttempt,
    pub target_observation: ObservationAttempt,
    pub mutation_mode: MutationMode,
    pub start_at: UnixNanoseconds,
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StartError {
    #[error("store compare-and-swap lost to another head")]
    HeadMismatch,
    #[error("store commit outcome is unknown")]
    Unknown,
    #[error("committed expected/new heads do not equal the proposed start bundle")]
    CommitMismatch,
}

pub fn start_publication(
    view: &Projection,
    request: PublicationStartRequest,
) -> Result<PendingPublicationStart, AdmissionError>;

impl PendingPublicationStart {
    pub fn bundle(&self) -> &TransitionBundle;
    pub fn resolve_commit(self, outcome: TrustedCommitOutcome) -> Result<PublicationPermit, StartError>;
}

impl PublicationPermit {
    pub fn effect_id(&self) -> &EffectId;
    pub fn resource_fences(&self) -> &[ResourceFence; 2];
    pub fn mutation_mode(&self) -> MutationMode;
}
```

`start_publication` rechecks live authorization, exact current fences, lease, both fresh before-observations, observation age, source/incarnation, target precondition, custody generation, terminal-sequence capacity, and nondecreasing time. It returns a proposed start bundle but no permit. `resolve_commit` consumes both the pending value and the authenticated store's non-`Clone` CAS outcome; only `Committed` with the bundle's exact expected and new heads returns a permit. Head mismatch or unknown outcome returns no permit and directs the caller to replay/recovery. A later read that merely observes `new_head` cannot be supplied as a committed outcome. `PublicationPermit` is non-`Clone`, has private fields, and is never serialized.

- [ ] **Step 5: Implement pre-start cancellation without a receipt**

```rust
pub struct PublicationCancellationRequest {
    pub effect_id: EffectId,
    pub reason: PreStartReason,
    pub transition_at: UnixNanoseconds,
}

pub fn cancel_publication(
    view: &Projection,
    request: PublicationCancellationRequest,
) -> Result<(TransitionBundle, PreStartOutcome), AdmissionError>;
```

Admit only the six cancellation-event reasons from §8.4. `reservation_deadline` requires `transition_at >= lease.expires_at`. Return `prepared_only` only if `ArtifactPrepared` committed; otherwise `not_attempted`. Preserve binding, fence, and spent warrant; release held budgets and both locks; mint no receipt.

- [ ] **Step 6: Run admission, doctest, replay, and lint gates**

```bash
cargo test -p guild-effect-kernel --test publication_admission
cargo test -p guild-effect-kernel --doc
cargo test -p guild-effect-kernel --test projection
cargo clippy -p guild-effect-kernel --all-targets --all-features -- -D warnings
```

Expected: all commands exit zero.

- [ ] **Step 7: Commit and checkpoint**

```bash
git add crates/guild-effect-kernel
git commit -m "feat(effect-kernel): admit publication through durable start"
git push origin HEAD:design/guild-effect-kernel
```

---

### Task 11: Implement Live Publication Terminalization

**Files:**
- Modify: `crates/guild-effect-kernel/src/publication.rs`
- Modify: `crates/guild-effect-kernel/src/evidence.rs`
- Create: `crates/guild-effect-kernel/tests/publication_outcomes.rs`

**Interfaces:**
- Consumes: a projected `Started` publication, the exact start-bound observations, authenticated after-evidence values, a non-recovery `CommandReport`, and `assessed_at`.
- Produces: `terminalize_publication_live`, returning `Result<PublicationTerminal, TerminalError>`, where `PublicationTerminal` exposes one atomic `TransitionBundle`, one terminal receipt ref, optional deed ref only for Verified, and derived custody ref.

- [ ] **Step 1: Write exact terminal-bundle tests**

Create one lawful started publication fixture and three after-evidence sets. Assert these exact event sequences:

```rust
assert_eq!(verified.bundle().event_types(), [EventType::ArtifactPublished, EventType::EffectVerified]);
assert_eq!(failed.bundle().event_types(), [EventType::ArtifactPublished, EventType::EffectFailed, EventType::CustodyAbsent]);
assert_eq!(indeterminate.bundle().event_types(), [EventType::ArtifactPublished, EventType::EffectIndeterminate, EventType::CustodyDisputed]);
```

Assert all events in one terminal bundle share `assessed_at`; Verified has one deed and `Owned` custody; Failed has no deed and `Absent` custody; Indeterminate has no deed and `Disputed` custody. Assert all three release both resource locks only at the final event and consume/release the publication's three-slot terminal reserve correctly.

Add refusals for a second terminal proposal, wrong effect/binding/prepared refs, after-observation time after assessment, `NotAvailable` on the live path, and an evidence body whose before-observation refs differ from the start event.

- [ ] **Step 2: Run the outcome tests and observe the missing function**

```bash
cargo test -p guild-effect-kernel --test publication_outcomes
```

Expected: FAIL because `terminalize_publication_live` is not defined.

- [ ] **Step 3: Implement live evidence-to-terminal derivation**

```rust
pub fn terminalize_publication_live(
    view: &Projection,
    request: LivePublicationReport,
) -> Result<PublicationTerminal, TerminalError>;

pub struct LivePublicationReport {
    pub effect_id: EffectId,
    pub command_report: CommandReport,
    pub source_after: ObservationAttempt,
    pub target_after: ObservationAttempt,
    pub assessed_at: UnixNanoseconds,
}
#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("effect is not started")]
    NotStarted,
    #[error("effect already has a terminal receipt")]
    AlreadyTerminal,
    #[error("terminal time regressed or evidence occurs after assessment")]
    Time,
    #[error("terminal evidence is invalid: {0}")]
    Evidence(#[from] EvidenceError),
    #[error("terminal transition is not admissible: {0}")]
    Admission(#[from] AdmissionError),
}
pub struct PublicationTerminal {
    bundle: TransitionBundle,
    receipt_digest: EffectReceiptRef,
    deed_digest: OptionalValue<ResourceDeedRef>,
    custody_record_digest: CustodyRecordRef,
}

impl PublicationTerminal {
    pub fn bundle(&self) -> &TransitionBundle;
    pub fn receipt_digest(&self) -> &EffectReceiptRef;
    pub fn deed_digest(&self) -> &OptionalValue<ResourceDeedRef>;
    pub fn custody_record_digest(&self) -> &CustodyRecordRef;
}
```

`ObservationAttempt` contains only probe facts plus authenticated/enrolled witness status. It contains no postcondition, causality, state, reason, result, deed, generation, or custody input. Resolve the exact started facts from `Projection`, derive evidence and causality through Task 7, classify by §9.4, then derive receipt, optional deed proof, custody record, typed events, and body refs.

- [ ] **Step 4: Build exactly one atomic terminal transition**

The returned bundle must include all newly derived bodies and the complete required event sequence. No externally observable head may end after `ArtifactPublished`, `EffectFailed`, or `EffectIndeterminate`. The transition validates against a clone of the current projection and exposes only the final head. A duplicate request after commit resolves to the existing receipt by replay and must not append or mint a second deed.

- [ ] **Step 5: Run classification, replay, and exactly-once tests**

```bash
cargo test -p guild-effect-kernel --test publication_outcomes
cargo test -p guild-effect-kernel --test evidence --test projection
cargo clippy -p guild-effect-kernel --all-targets --all-features -- -D warnings
```

Expected: all commands exit zero.

- [ ] **Step 6: Commit and checkpoint**

```bash
git add crates/guild-effect-kernel
git commit -m "feat(effect-kernel): terminalize publication from evidence"
git push origin HEAD:design/guild-effect-kernel
```

---

### Task 12: Implement Non-Repeating Recovery

**Files:**
- Create: `crates/guild-effect-kernel/src/recovery.rs`
- Modify: `crates/guild-effect-kernel/src/lib.rs`
- Create: `crates/guild-effect-kernel/tests/recovery.rs`

**Interfaces:**
- Consumes: a completely validated, independently anchored projection plus fresh probe attempts supplied by an outer coordinator.
- Produces: `recovery_candidates`, `next_action`, `recover_publication`, `cancel_orphaned_publication`, `RecoveryCandidate`, `NextAction`, and `RecoveryTerminal`; no function in this module returns `PublicationPermit` or any other protected mutation authority.

- [ ] **Step 1: Write crash-point and no-repeat tests**

Model these fixed crash points: before reservation commit, after reservation, after preparation, after start commit before permit delivery, after adapter command before report, after evidence derivation before terminal commit, and after terminal commit before response. Assert:

- pre-start committed reservations can only cancel with `recovery_orphaned`;
- a committed start always appears in `recovery_candidates` until terminal;
- recovery after any post-start crash produces one terminal bundle using fresh probes;
- no recovery return type contains a permit and no event sequence contains a second `effect_started`;
- replay after an unknown terminal commit either finds the existing receipt or proposes one terminal bundle, never two.

Add this type assertion in the public test:

```rust
fn assert_recovery_result_type(_: guild_effect_kernel::recovery::RecoveryTerminal) {}
```

and a compile-fail doctest showing a `RecoveryTerminal` cannot be converted into `PublicationPermit`.

- [ ] **Step 2: Run the tests and observe the missing recovery module**

```bash
cargo test -p guild-effect-kernel --test recovery
```

Expected: FAIL with unresolved import `guild_effect_kernel::recovery`.

- [ ] **Step 3: Discover candidates only from replayed truth**

```rust
pub enum RecoveryCandidate {
    PublicationStarted { effect_id: EffectId, started_at: UnixNanoseconds },
    PublicationOrphaned { effect_id: EffectId, state: PreStartEffectState },
    SeparationStarted { effect_id: EffectId, started_at: UnixNanoseconds },
    SeparationOrphaned { effect_id: EffectId, state: PreStartEffectState },
}
pub enum NextAction { ProbeAndClassify, CancelOrphan, None }

pub fn recovery_candidates(view: &Projection) -> Vec<RecoveryCandidate>;
pub fn next_action(view: &Projection, effect_id: &EffectId) -> NextAction;
```

Return candidates sorted by `EffectId` canonical bytes. Proposed-only and terminal effects are excluded. Refuse discovery if graph, chain, expected head, binding, fence, or projection validation fails; do not guess from an unanchored dossier.

- [ ] **Step 4: Implement recovered publication classification**

```rust
pub struct PublicationRecoveryReport {
    pub effect_id: EffectId,
    pub source_after: ObservationAttempt,
    pub target_after: ObservationAttempt,
    pub recovered_at: UnixNanoseconds,
}
pub type TerminalReceiptRef = ProtocolRef<EffectReceiptTag, SeparationReceiptTag>;
pub struct RecoveryTerminal {
    bundle: TransitionBundle,
    effect_id: EffectId,
    terminal_receipt: TerminalReceiptRef,
    recovery_assessment: RecoveryAssessmentRef,
}
#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("effect is not a recovery candidate")]
    NotCandidate,
    #[error("history is corrupt or lacks an independently authenticated head")]
    UnanchoredOrCorrupt,
    #[error("store outcome requires authenticated reload and replay")]
    ReloadRequired,
    #[error("terminal classification failed: {0}")]
    Terminal(#[from] TerminalError),
}
pub struct OrphanCancellationRequest {
    pub effect_id: EffectId,
    pub transition_at: UnixNanoseconds,
}

pub fn recover_publication(
    view: &Projection,
    request: PublicationRecoveryReport,
) -> Result<RecoveryTerminal, RecoveryError>;
pub fn cancel_orphaned_publication(
    view: &Projection,
    request: OrphanCancellationRequest,
) -> Result<(TransitionBundle, PreStartOutcome), RecoveryError>;
```

The request includes the candidate effect ID, source/target probe attempts, and `recovered_at`; it has no command report. Derive `CommandReport::NotAvailable`, reuse the live evidence/postcondition/causality classifier, derive one `RecoveryAssessment`, and use `ArtifactPublishedRecovered` followed by the same Verified/Failed/Indeterminate terminal/custody sequence. Require evidence `assessed_at`, receipt `terminal_at`, assessment `recovered_at`, and every event `occurred_at` to equal `recovered_at`.

- [ ] **Step 5: Implement orphan cancellation and unknown-outcome replay rules**

`cancel_orphaned_publication` admits only Reserved or Prepared effects that never started and emits the normal cancellation event with `RecoveryOrphaned`. On a store head mismatch or unknown commit outcome, return a value instructing the caller to reload the authenticated head; never reuse a speculative bundle or permit. If the proposed terminal head is reachable after replay, return the existing receipt.

- [ ] **Step 6: Run crash, replay, doctest, and lint gates**

```bash
cargo test -p guild-effect-kernel --test recovery
cargo test -p guild-effect-kernel --doc
cargo test -p guild-effect-kernel --test publication_outcomes --test projection
cargo clippy -p guild-effect-kernel --all-targets --all-features -- -D warnings
```

Expected: every crash case terminalizes without a second start; all commands exit zero.

- [ ] **Step 7: Commit and checkpoint**

```bash
git add crates/guild-effect-kernel
git commit -m "feat(effect-kernel): recover without repeating mutation"
git push origin HEAD:design/guild-effect-kernel
```

---

### Task 13: Implement Separation And Quarantine

**Files:**
- Create: `crates/guild-effect-kernel/src/separation.rs`
- Modify: `crates/guild-effect-kernel/src/recovery.rs`
- Modify: `crates/guild-effect-kernel/src/lib.rs`
- Create: `crates/guild-effect-kernel/tests/separation.rs`
- Modify: `crates/guild-effect-kernel/tests/support/mod.rs`

**Interfaces:**
- Consumes: exact current `Owned` custody/deed, active and quarantine observations, xattr body, authority/lease/projection primitives, and protocol §§9.4, 9.5, and 13.
- Produces: `propose_separation`, `approve_separation`, `revoke_separation`, `expire_separation`, `reserve_separation`, `start_separation`, `cancel_separation`, `terminalize_separation_live`, `recover_separation`, opaque `PendingSeparationStart`/`SeparationPermit`, and deterministic `Owned`/`Quarantined`/`Disputed` custody transitions.

- [ ] **Step 1: Write admission, start-barrier, and outcome tests**

From a Verified publication fixture, create exact current `Owned` custody and test:

- exact deed/current generation/self-owned active claim plus unclaimed quarantine is admitted;
- stale generation, missing/mismatched deed, non-Owned custody, foreign active claim, claimed/locked quarantine, identical active/quarantine keys, or generation exhaustion refuses before reservation/start;
- start requires fresh exact active-present and quarantine-absent observations, then consumes the exact successful non-`Clone` CAS outcome before returning a non-`Clone` permit;
- exact move plus exact xattr/retained incarnation gives Verified/Quarantined;
- exact no-move gives Failed/Owned;
- duplicate incarnation, partial move/metadata, unsupported identity, conflicting/unavailable evidence, or unconditional uncertainty gives Indeterminate/Disputed;
- every terminal row increments custody generation exactly once and retains the publication deed.

Assert these event shapes:

```rust
assert_eq!(verified.bundle().event_types(), [EventType::SeparationVerified]);
assert_eq!(failed.bundle().event_types(), [EventType::SeparationFailed]);
assert_eq!(indeterminate.bundle().event_types(), [EventType::SeparationIndeterminate, EventType::CustodyDisputed]);
```

- [ ] **Step 2: Run the test and observe the missing separation module**

```bash
cargo test -p guild-effect-kernel --test separation
```

Expected: FAIL with unresolved import `guild_effect_kernel::separation`.

- [ ] **Step 3: Implement separation proposal through reservation**

```rust
pub struct SeparationProposal {
    pub proposer_id: PrincipalId,
    pub deed_digest: ResourceDeedRef,
    pub quarantine_address: LogicalAddress,
    pub quarantine_xattr_digest: XattrValueRef,
    pub idempotency_key: IdempotencyKey,
    pub reservation_budget: BudgetClaim,
    pub start_budget: BudgetClaim,
    pub issued_at: UnixNanoseconds,
    pub expires_at: UnixNanoseconds,
    pub nonce: Hex256,
    pub transition_at: UnixNanoseconds,
}
pub struct SeparationApprovalRequest {
    pub warrant_digest: SeparationWarrantRef,
    pub approver_id: PrincipalId,
    pub approved_at: UnixNanoseconds,
}
pub struct SeparationRevocationRequest {
    pub warrant_digest: SeparationWarrantRef,
    pub revoker_id: PrincipalId,
    pub reason: Identifier,
    pub revoked_at: UnixNanoseconds,
}
pub struct SeparationExpiryRequest {
    pub warrant_digest: SeparationWarrantRef,
    pub transition_at: UnixNanoseconds,
}
pub struct SeparationReservationRequest {
    pub warrant_digest: SeparationWarrantRef,
    pub reserved_at: UnixNanoseconds,
}

pub fn propose_separation(view: &Projection, request: SeparationProposal) -> Result<TransitionBundle, AdmissionError>;
pub fn approve_separation(view: &Projection, request: SeparationApprovalRequest) -> Result<TransitionBundle, AdmissionError>;
pub fn revoke_separation(view: &Projection, request: SeparationRevocationRequest) -> Result<TransitionBundle, AdmissionError>;
pub fn expire_separation(view: &Projection, request: SeparationExpiryRequest) -> Result<TransitionBundle, AdmissionError>;
pub fn reserve_separation(view: &Projection, request: SeparationReservationRequest) -> Result<SeparationReservation, AdmissionError>;
```

Derive active address/artifact/content/incarnation/generation from the deed plus custody; accept only quarantine address and an existing validated xattr body from the input. Use distinct separation warrant/binding/lease body kinds and idempotency namespace. Lock active and quarantine keys, hold both budget classes, assign both checked fences, and reserve two terminal event slots. Check generation successor at reservation.

- [ ] **Step 4: Implement separation durable start and cancellation**

```rust
pub struct SeparationStartRequest {
    pub effect_id: EffectId,
    pub active_observation: ObservationAttempt,
    pub quarantine_observation: ObservationAttempt,
    pub mutation_mode: MutationMode,
    pub start_at: UnixNanoseconds,
}
pub struct SeparationCancellationRequest {
    pub effect_id: EffectId,
    pub reason: PreStartReason,
    pub transition_at: UnixNanoseconds,
}

pub fn start_separation(view: &Projection, request: SeparationStartRequest) -> Result<PendingSeparationStart, AdmissionError>;
impl PendingSeparationStart {
    pub fn bundle(&self) -> &TransitionBundle;
    pub fn resolve_commit(self, outcome: TrustedCommitOutcome) -> Result<SeparationPermit, StartError>;
}
impl SeparationPermit {
    pub fn effect_id(&self) -> &EffectId;
    pub fn resource_fences(&self) -> &[ResourceFence; 2];
    pub fn mutation_mode(&self) -> MutationMode;
}
pub fn cancel_separation(view: &Projection, request: SeparationCancellationRequest) -> Result<(TransitionBundle, PreStartOutcome), AdmissionError>;
```

Recheck generation successor, exact Owned custody/deed, both address claims/locks/fences, warrant/lease/time, and observations at start. Consume budgets but retain locks after start. Cancellation has no receipt and preserves the permanent binding, spent warrant, deed, generation, and fences.

- [ ] **Step 5: Implement live and recovered terminalization**

```rust
pub struct LiveSeparationReport {
    pub effect_id: EffectId,
    pub command_report: CommandReport,
    pub active_after: ObservationAttempt,
    pub quarantine_after: ObservationAttempt,
    pub assessed_at: UnixNanoseconds,
}
pub struct SeparationRecoveryReport {
    pub effect_id: EffectId,
    pub active_after: ObservationAttempt,
    pub quarantine_after: ObservationAttempt,
    pub recovered_at: UnixNanoseconds,
}
pub struct SeparationTerminal {
    bundle: TransitionBundle,
    receipt_digest: SeparationReceiptRef,
    custody_record_digest: CustodyRecordRef,
}

pub fn terminalize_separation_live(view: &Projection, request: LiveSeparationReport) -> Result<SeparationTerminal, TerminalError>;
pub fn recover_separation(view: &Projection, request: SeparationRecoveryReport) -> Result<RecoveryTerminal, RecoveryError>;
```

Use the exact six-row §9.4 classification. Live payload mode is `live`; recovered payload mode is `recovered` and carries the matching `RecoveryAssessment` ref. `NotAvailable` is recovery-only. Derive custody/address claims exactly from §9.5 and release locks only on the final event. A repeated terminal request must return the existing receipt and must not increment generation again.

- [ ] **Step 6: Activate separation candidates without adding mutation authority**

`RecoveryCandidate` already contains `SeparationStarted` and `SeparationOrphaned`; make those variants reachable from replayed separation states and keep all candidates sorted by effect ID. Extend the compile-fail recovery test so neither recovery result can become `SeparationPermit`. Recovered evidence must use the same classifier and must never propose `separation_started` again.

- [ ] **Step 7: Run separation, recovery, property, and lint tests**

```bash
cargo test -p guild-effect-kernel --test separation --test recovery
cargo test -p guild-effect-kernel --test projection
cargo test -p guild-effect-kernel --doc
cargo clippy -p guild-effect-kernel --all-targets --all-features -- -D warnings
```

Expected: all commands exit zero; deed history is retained across every separation outcome.

- [ ] **Step 8: Commit and checkpoint**

```bash
git add crates/guild-effect-kernel
git commit -m "feat(effect-kernel): separate artifacts into quarantine"
git push origin HEAD:design/guild-effect-kernel
```

---

### Task 14: Implement The Complete Dossier Model And Golden Vectors

**Files:**
- Create: `crates/guild-effect-kernel/src/model.rs`
- Modify: `crates/guild-effect-kernel/src/body.rs`
- Modify: `crates/guild-effect-kernel/src/lib.rs`
- Create: `crates/guild-effect-kernel/tests/model.rs`
- Create: `vectors/effect-kernel-v1/README.md`
- Create: `vectors/effect-kernel-v1/manifest.json` via the deterministic writer
- Create: `vectors/effect-kernel-v1/canonical/local-file-observation-absent.json` via the deterministic writer
- Create: `vectors/effect-kernel-v1/dossiers/publication-verified.json` via the deterministic writer
- Create: `vectors/effect-kernel-v1/dossiers/publication-failed.json` via the deterministic writer
- Create: `vectors/effect-kernel-v1/dossiers/publication-indeterminate.json` via the deterministic writer
- Create: `vectors/effect-kernel-v1/dossiers/separation-verified.json` via the deterministic writer
- Create: `vectors/effect-kernel-v1/dossiers/separation-failed.json` via the deterministic writer
- Create: `vectors/effect-kernel-v1/dossiers/separation-indeterminate.json` via the deterministic writer
- Modify: `xtask/Cargo.toml`
- Modify: `xtask/src/effect_kernel.rs`
- Modify: `Cargo.lock`

**Interfaces:**
- Consumes: all kernel modules and protocol §§14–15.
- Produces: `Dossier`, `DossierSummary`, `DossierConsistency`, `validate_dossier`, `derive_summary`, deterministic `conformance_vectors`, and `cargo run -q -p xtask -- effect-kernel vectors <write|check>`.

- [ ] **Step 1: Write dossier consistency/freshness and summary tests**

Test that a complete dossier validates internally against its claimed head, but only reports `Fresh` when the separately supplied `TrustedHead` has the same installation/head/store ID. A mismatched authenticated head must return `StaleOrRolledBack`, not a generic parse success.

For a mixed history, assert exact summary derivation: current custody refs only, all publication/separation receipt refs sorted, counts from unique semantic events, unresolved IDs for Reserved/Prepared/Started only, and claimed head equal to the final validated event digest. Corrupt the summary body and prove replay wins.

- [ ] **Step 2: Run the model tests and observe the missing module**

```bash
cargo test -p guild-effect-kernel --test model
```

Expected: FAIL with unresolved import `guild_effect_kernel::model`.

- [ ] **Step 3: Implement the dossier facade without I/O**

```rust
pub struct Dossier {
    pub bodies: BTreeMap<Digest, Vec<u8>>,
    pub events: BTreeMap<Digest, Vec<u8>>,
    pub claimed_event_head: Digest,
}
pub enum DossierConsistency { InternallyConsistent, Fresh, StaleOrRolledBack }
pub struct DossierCounts {
    proposed: U64Decimal,
    reserved: U64Decimal,
    cancelled: U64Decimal,
    started: U64Decimal,
    verified: U64Decimal,
    failed: U64Decimal,
    indeterminate: U64Decimal,
}
pub struct DossierSummary {
    installation_digest: InstallationEnrollmentRef,
    policy_digest: AuthorityPolicyRef,
    claimed_event_head: RawDigest,
    custody_record_digests: SortedUnique<CustodyRecordRef>,
    publication_receipt_digests: SortedUnique<EffectReceiptRef>,
    separation_receipt_digests: SortedUnique<SeparationReceiptRef>,
    counts: DossierCounts,
    unresolved_effect_ids: SortedUnique<EffectId>,
}
pub struct ValidatedDossier {
    graph: BodyGraph,
    ordered_events: Vec<EventEnvelope>,
    projection: Projection,
    summary: ValidatedBody<DossierSummary>,
}
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("body graph is invalid: {0}")]
    Body(#[from] BodyError),
    #[error("event chain is invalid: {0}")]
    Chain(#[from] ChainError),
    #[error("projection is invalid: {0}")]
    Projection(#[from] ProjectionError),
    #[error("claimed event head is absent from the dossier")]
    MissingClaimedHead,
    #[error("derived dossier summary is inconsistent")]
    SummaryInvariant,
}

pub fn validate_dossier(
    dossier: &Dossier,
    trusted_head: Option<&TrustedHead>,
) -> Result<(ValidatedDossier, DossierConsistency), ModelError>;
pub fn derive_summary(validated: &ValidatedDossier) -> Result<ValidatedBody<DossierSummary>, ModelError>;
```

Strict-decode stored bytes, recompute map keys, validate the complete typed DAG, validate/replay the chain, derive the summary, and compare the independent anchor if supplied. Never let a dossier's embedded claimed head authenticate itself.

Implement `BodySpec` and strict replay decoding for `dossier-summary/v1`, replace its final `PayloadModuleUnavailable` arm, delete the now-unreachable error variant, and make the 29-arm decoder match exhaustive with no wildcard.

- [ ] **Step 4: Define six complete deterministic conformance histories**

Implement `conformance_vectors() -> Result<Vec<ConformanceVector>, ModelError>` with fixed values only: installation `guild-test-installation`, principals `proposer`/`approver`/`revoker`, witness `host-probe`, trusted clock/store `test-clock`/`test-store`, nanosecond times beginning at `1788210000000000000`, fixed 64-hex nonces/incarnations/content digests, and canonical local-file addresses. Build every vector through lawful public transition APIs and the in-test committed-store harness.

Define the value returned to `xtask` without adding I/O to the kernel:

```rust
pub enum ConformanceVectorKind { CanonicalBody, Dossier }
pub struct ConformanceVector {
    pub path: String,
    pub bytes: Vec<u8>,
    pub expected_digest: Digest,
    pub kind: ConformanceVectorKind,
}
pub fn conformance_vectors() -> Result<Vec<ConformanceVector>, ModelError>;
```

The vector set is exactly:

1. canonical absent local-file observation;
2. complete publication Verified dossier;
3. complete publication Failed/no-effect dossier;
4. complete publication Indeterminate/unavailable-witness dossier;
5. complete publication Verified followed by separation Verified dossier;
6. complete publication Verified followed by separation Failed/no-move dossier;
7. complete publication Verified followed by separation Indeterminate/duplicate-incarnation dossier.

- [ ] **Step 5: Implement deterministic vector writing/checking outside the kernel**

Add `guild-effect-kernel = { path = "../crates/guild-effect-kernel" }` to `xtask`; this preserves the allowed dependency direction. Extend the command parser with:

```text
cargo run -q -p xtask -- effect-kernel vectors write
cargo run -q -p xtask -- effect-kernel vectors check
```

`write` serializes canonical bytes from `conformance_vectors`, writes the seven files, and writes `manifest.json` as JCS containing:

```json
{"protocol":"jidoka.dev/events/v1","vectors":[{"expectedDigest":"sha256:37acdc8236b6c57c87a7d68b0ed51cf02d9a97ba78edd6d13a3b3f754000cf81","path":"canonical/local-file-observation-absent.json","sha256":"sha256:37acdc8236b6c57c87a7d68b0ed51cf02d9a97ba78edd6d13a3b3f754000cf81","type":"canonical-body"}]}
```

The writer derives every displayed digest; no digest is manually copied except the already frozen absent-observation identity. `check` regenerates entirely in memory, byte-compares every expected path and the manifest, rejects missing/extra files, and validates every dossier through the public API.

- [ ] **Step 6: Write and verify the golden files**

```bash
cargo run -q -p xtask -- effect-kernel vectors write
cargo run -q -p xtask -- effect-kernel vectors check
cargo test -p guild-effect-kernel --test model
```

Expected: `effect-kernel vectors: 7 files and manifest match`; the absent-observation body has no trailing newline and its digest is `sha256:37acdc8236b6c57c87a7d68b0ed51cf02d9a97ba78edd6d13a3b3f754000cf81`.

- [ ] **Step 7: Document the vector trust boundary**

In `vectors/effect-kernel-v1/README.md`, state that vectors prove canonical/replay parity and internal dossier consistency, not freshness, adapter correctness, principal authentication, durable CAS, or exactly-once external side effects. State that Jidoka v1 identifiers are intentionally retained.

- [ ] **Step 8: Commit and checkpoint**

```bash
git add Cargo.lock xtask crates/guild-effect-kernel vectors/effect-kernel-v1
git commit -m "feat(effect-kernel): freeze v1 dossier vectors"
git push origin HEAD:design/guild-effect-kernel
```

---

### Task 15: Close The Phase 3 Conformance Gate

**Files:**
- Create: `crates/guild-effect-kernel/tests/crash_model.rs`
- Create: `crates/guild-effect-kernel/tests/counter_exhaustion.rs`
- Create: `crates/guild-effect-kernel/tests/ownership.rs`
- Create: `crates/guild-effect-kernel/tests/protocol_conformance.rs`
- Modify: `Makefile`
- Modify: `.github/workflows/ci.yml`
- Modify: `SPECS.md`
- Modify: `ARCHITECTURE.md`
- Modify: `docs/first-honest-mutation-demo.md`
- Modify: `docs/protocol/effect-kernel-v1-change-ledger.md`

**Interfaces:**
- Consumes: the complete pure kernel and vector command.
- Produces: `make effect-kernel-conformance`, an explicit CI gate, and documentation that says the pure kernel is implemented while host mutation remains gated.

- [ ] **Step 1: Add the complete protocol-law inventory test**

`protocol_conformance.rs` must assert that the implementation manifests exactly 29 body kinds, 26 event types, five schema descriptors, every §7.2 permitted edge, every §10.4.1 transition row, all receipt result/reason values, all pre-start reasons, all custody states, and all vector paths. Unknown enum strings, body kinds, schema IDs, fields, edges, or event payload members must fail closed.

- [ ] **Step 2: Add exhaustive counter and ownership tests**

`counter_exhaustion.rs` covers first/last fences, first/no-prior and subsequent custody generations, genesis/last event sequences, multi-key atomic refusal if one counter is exhausted, and interleaved terminal-slot reserves. Each failure must occur before a binding, budget hold, start, or permit.

`ownership.rs` covers two-key publication locks, lock retention through start, release only on cancellation/final terminal event, claimed staging-source refusal, exact self-owned active separation, quarantine claim conflicts, and all Owned/Quarantined/Absent/Disputed address-claim roles from §9.5.

- [ ] **Step 3: Add the crash model across every durable boundary**

`crash_model.rs` enumerates every cut before/after reservation, preparation, start, permit delivery, adapter report, evidence construction, terminal proposal, terminal commit, and response delivery for both publication and separation. For each cut, replay the persisted prefix and assert:

```rust
assert!(history.started_count(effect_id) <= 1);
assert!(history.terminal_receipt_count(effect_id) <= 1);
if history.has_started(effect_id) {
    assert_eq!(recovery.next_action(effect_id), NextAction::ProbeAndClassify);
}
```

If a started effect is terminal in the persisted prefix, the final receipt count is exactly one; if it is unterminated, recovery creates exactly one terminal bundle and no start/permit. Run the same model with `TrustedCommitOutcome::Unknown` at start and terminal CAS boundaries.

- [ ] **Step 4: Run the new tests and fix only law violations**

```bash
cargo test -p guild-effect-kernel --test protocol_conformance
cargo test -p guild-effect-kernel --test counter_exhaustion
cargo test -p guild-effect-kernel --test ownership
cargo test -p guild-effect-kernel --test crash_model
```

Expected: all tests pass. If a failure exposes ambiguity in the approved protocol, stop and amend the design/ADR before changing wire identity or safety law.

- [ ] **Step 5: Add one required local/CI conformance target**

Add:

```make
.PHONY: effect-kernel-conformance
effect-kernel-conformance:
	cargo test -p guild-effect-kernel --all-targets --all-features
	cargo test -p guild-effect-kernel --doc
	cargo test -p guild-effect-kernel --test projection -- --ignored
	cargo run -q -p xtask -- effect-kernel check-dependencies
	cargo run -q -p xtask -- effect-kernel vectors check
```

Add `effect-kernel-conformance` to `verify`. Add a CI step named `Effect Kernel Conformance` running `make effect-kernel-conformance` after the general Test/Clippy steps.

- [ ] **Step 6: Update docs to the exact implemented-but-disconnected state**

Use this sentence:

```text
The pure `guild-effect-kernel` crate and v1 conformance suite are implemented; Guild's live runner still rejects apply, and authenticated storage, adapter execution, execution/session linkage, and operator mutation surfaces remain behind a separate host-integration design.
```

Do not say Guild can publish, separate, deploy, provision, or mutate a real resource. Record vector parity and the Rust 1.94 pin in the change ledger.

- [ ] **Step 7: Run the complete clean-workspace verification**

Run on the pinned toolchain:

```bash
rustc --version
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run -q -p xtask -- effect-kernel check-dependencies
cargo run -q -p xtask -- effect-kernel vectors check
cargo doc -p guild-effect-kernel --no-deps
make verify
git diff --check
git status --short
```

Expected: `rustc 1.94.0`, every command exits zero, vector and dependency checks print `ok`, documentation builds without warnings, and `git status --short` lists only the intended Task 15 files before commit.

- [ ] **Step 8: Commit and checkpoint**

```bash
git add .github/workflows/ci.yml Makefile SPECS.md ARCHITECTURE.md docs/first-honest-mutation-demo.md docs/protocol crates/guild-effect-kernel
git commit -m "test(effect-kernel): enforce v1 conformance"
git push origin HEAD:design/guild-effect-kernel
```

Expected: Phases 1–3 are complete. Stop here; do not begin host integration in the same implementation run.

---

## Final Review Checklist

- [ ] Repository history and provenance preserve the verbatim recovered source body with SHA-256 `86df64803cd2da89f6d6499aac4f884184b2799122a8f2e5e4cc7f9f178b177b`.
- [ ] The current normative body, from the first H1 through EOF and excluding the provenance preamble, has SHA-256 `b38d65617c6922c01c542e5d702aeba9b0866d2119250a4f5e8e83dd4b172f1d` after the ledgered §6.1 clarification.
- [ ] `docs/protocol/effect-kernel-v1-change-ledger.md` enumerates every normative difference from the recovered source body.
- [ ] Cargo metadata shows no Guild or effectful runtime dependency beneath `guild-effect-kernel`.
- [ ] Protocol manifests contain exactly 29 body kinds and 26 event types with unchanged strings.
- [ ] Every sealed authority/proof type lacks a public constructor and public deserializer.
- [ ] Every started effect has at most one start and exactly one terminal receipt after successful recovery.
- [ ] Recovery returns probe/classification work only and cannot return a mutation permit.
- [ ] Full replay equals incremental projection for generated legal histories; every generated illegal mutation fails closed.
- [ ] Publication and separation vectors reproduce byte-for-byte with unchanged digests across clean runs.
- [ ] Documentation distinguishes implemented pure-kernel behavior from unimplemented host integration and still-gated `apply`.
- [ ] No CLI, MCP, URI, WIT, manifest, provider, filesystem adapter, store adapter, session link, or active apply surface was added.
- [ ] The Jidoka source repository was read for provenance only; its migration pointer/archive decision remains a separately gated Phase 5 action.

## Execution Handoff

Implement only after selecting one execution mode and preparing an isolated worktree from `design/guild-effect-kernel`:

1. **Subagent-Driven (recommended):** use `superpowers:subagent-driven-development`; dispatch a fresh worker per task and perform spec-compliance then code-quality review before each checkpoint.
2. **Inline Execution:** use `superpowers:executing-plans`; execute in batches with explicit review checkpoints and stop after Task 15.

The planning shell used to write this document did not have `cargo` installed, so its Rust commands were not executed during planning. Task execution must begin by confirming the pinned Rust 1.94.0 toolchain is available; failure to obtain that toolchain is a blocker, not permission to change the pin.
