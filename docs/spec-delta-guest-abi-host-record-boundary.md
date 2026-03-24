# Guest ABI / Host Record Spec Delta

Status: Accepted pre-code delta for the boundary-model contract pass  
Applies to: shared Rust types, manifests, runtime translation, examples, and contract tests

## Boundary rule

- `wit/guild-skill-v1.wit` is canonical for the guest-wire contract.
- Rust host types are canonical for durable platform records.
- Translation between them is explicit and tested.
- Host-owned status, provenance, trust metadata, policy decisions, receipts, and durable identifiers do not cross into WIT `skill-output`.

## Identity and envelopes

| Concern | Contract |
| --- | --- |
| Caller-facing request | `CallerRequest` carries `RequestedSkillRef`, caller identity, input, requested capabilities, budget, and trace metadata. |
| Resolved execution input | `ResolvedExecutionEnvelope` carries `ResolvedSkillRef`, granted capabilities, policy decision, and optional parent linkage. Durable execution identifiers are minted later by the host execution path. |
| Runner entry | The runner accepts only `ResolvedExecutionEnvelope`; no public runner API accepts `RequestedSkillRef`. |
| Durable receipt | `ExecutionReceipt` carries stable execution URI, execution ID, trace ID, and terminal status. |
| Durable provenance | Provenance stores `ResolvedSkillRef`, not a mutable requested ref. |

## Guest ABI

| Concern | Contract |
| --- | --- |
| Entry point | Keep `run(ctx, input) -> result<SkillOutput, SkillError>`. |
| Guest context | The active inspect `ExecutionContext` contains host-minted execution identity, trace/tenant IDs, resolved skill identity, input hash, `now_utc`, budget, and guest-visible granted capabilities only. Inspect mode is implied by the `guild-skill-inspect-v1` world and is not carried as a guest field. |
| Host imports | Keep explicit host-mediated capability imports such as `http-request`, `read-resource`, `emit-evidence`, `invoke-dependency`, and `log`. |
| Child invocation | The guest receives child `SkillOutput` or `SkillError`; host-owned child execution records remain on the host side. |
| Forbidden ABI growth | Do not add execution status, metrics, provenance, policy results, receipts, trust metadata, termination detail, child lineage, or durable URIs to WIT outputs. |

## Durable host records

| Concern | Contract |
| --- | --- |
| Execution record | `ExecutionRecord` stores `ExecutionReceipt`, `CallerRequest`, `ResolvedSkillRef`, `PolicyDecision`, granted capabilities, host-owned termination detail, metrics, provenance, evidence records, and child linkage. |
| Skill output | Guest-authored `SkillOutput` may be embedded inside `ExecutionRecord` as a payload snapshot without changing record ownership. |
| Evidence record | `EvidenceRecord` stores URI, content identity, mime type, size, audience, redaction, freshness, and producer metadata when available. |
| Evidence handle | `EvidenceRef` remains the guest-visible handle returned through WIT and stored in guest output. |
| Policy decision | `PolicyDecision` records allowed, reduced, or rejected outcome plus durable rationale metadata. |

## Manifest version axes

| Concern | Contract |
| --- | --- |
| Manifest schema version | Add top-level `manifest_schema_version`. |
| Skill API version | Add top-level `skill_api_version`. |
| Guest ABI version | Rename `runtime.abi` to `runtime.guest_abi_version`. |
| Executable artifact version | Keep existing `version` for the executable artifact version used in resolution. |

## Resources and authorization

| Concern | Contract |
| --- | --- |
| Execution resources | `ExecutionReceipt.uri` points to the durable Guild execution resource. |
| Evidence resources | Evidence URIs remain Guild object URIs; host code can load durable `EvidenceRecord` metadata for them. |
| MCP transport auth | Separate from Guild runtime capability grants. |
| Guild runtime auth | Expressed as host-evaluated capability grants and policy decisions inside the execution envelope and durable records. |

## Required tests

- compile-fail coverage proving the runner cannot accept `RequestedSkillRef`
- WIT/Rust alignment tests for guest-visible types only
- translation tests proving host-owned fields stay out of WIT outputs
- manifest roundtrip and validation tests for all three version axes
- durable record serialization and resource-read tests for execution and evidence records
