# ADR 0013: Read-resource policy family

Status: Accepted  
Date: 2026-03-17

## Context

Guild already exposes one host-mediated resource path that is shared by guest
`read-resource`, host inspection, and MCP `resources/read`.

That path now covers:

- direct execution resources
- object blob resources
- evidence-record resources
- bounded execution-query resources

The important architectural risk is letting "read-resource" drift into meaning
"read some host thing somewhere," especially once future filesystem work
arrives.

## Decision

The `read-resource` family authorizes reads of canonical Guild resources only.
It does not mean arbitrary filesystem access.

The current resource kinds are:

- `execution` for `guild://executions/{execution_id}`
- `object` for `guild://objects/records/{evidence_record_id}` and
  `guild://objects/sha256/{digest}`
- `query` for `guild://queries/executions/...`

The current scope roots are exact canonical Guild prefixes:

- `guild://executions/`
- `guild://objects/records/`
- `guild://objects/sha256/`
- `guild://queries/executions/`

`ReadResourceConstraints` has two policy dimensions:

- `uri_prefixes`
- `resource_kinds`

Those constraints are host-validated and host-interpreted:

- `uri_prefixes` must parse as canonical Guild scope roots
- non-canonical values are invalid capability constraints and fail before
  execution
- `resource_kinds`, when present, must be compatible with the declared scope
  roots

Authorization is canonical and host-mediated:

1. the host parses the requested URI into a typed `GuildResourceUri`
2. malformed or ambiguous URIs fail closed
3. the host checks that at least one `read-resource` grant covers the parsed
   resource kind
4. the host checks that at least one grant covers the parsed canonical scope
5. only then does the shared resource backend load the resource

Guild compares parsed resources to parsed scopes. It does not authorize by loose
raw string prefix matching.

The current direct-resource versus query-resource distinction is explicit:

- execution and object URIs refer to concrete stored artifacts
- query URIs refer to bounded host-derived summaries over persisted execution
  records
- exact execution or object grants do not implicitly authorize query reads
- query semantics such as bounded limits and deterministic ordering are defined
  in ADR 0011

The current object-record read behavior is also explicit:

- `guild://objects/records/{evidence_record_id}` authorizes against the object
  record scope
- the current resource backend dereferences that record to the underlying payload
  bytes rather than returning serialized `EvidenceRecord` metadata
- direct metadata reads remain a host-side registry concern, not a separate guest
  capability

Safe defaults in the current repository are:

- no `read-resource` grant means no resource read authority
- malformed URIs fail closed
- non-canonical grant scopes fail closed
- if a `read-resource` grant exists and omits one of its optional fields, that
  omission is only unbounded within the current Guild resource universe for that
  family, not outside Guild resources

Nested child behavior is subset-only:

- child `read-resource` authority is reduced from the parent grant against the
  child manifest requirement
- child scopes and kinds may narrow, but they may not widen
- reduction failure produces a host-owned child denial

## Consequences

Positive:

- explain/debug flows can rely on one honest resource model
- query resources remain discoverability tools without turning into a general
  search API
- future filesystem work stays clearly separate from Guild resource reads

Costs and limits:

- guests cannot treat evidence-record URIs as a metadata API in this milestone
- read-resource remains limited to the current local Guild resource backend
- a durable read ledger is still deferred

## Explicit invariants

- `read-resource` is not filesystem access
- resource scopes are canonical Guild roots, not free-form prefixes
- malformed or ambiguous URIs fail closed
- host and guest reads use the same conceptual backend
- query resources require explicit query scope authorization
- child read authority cannot widen beyond the parent grant

## Explicit non-goals / deferred work

- arbitrary host file reads
- remote resource backends
- broader search or full-text query APIs
- persisted read ledgers
- evidence-record metadata as a separate guest-readable JSON resource in this
  milestone

## Cross-references

- `SPECS.md`
- `ARCHITECTURE.md`
- `docs/adr/0011-bounded-artifact-query-resources.md`
- `docs/adr/0012-capability-policy-layering-model.md`
- `docs/adr/0007-evidence-record-schema.md`
- `crates/guild-types/src/lib.rs`
- `crates/guild-registry/src/lib.rs`
- `crates/guild-runner/src/lib.rs`
- `crates/guild-runner/tests/resource_reads.rs`
- `examples/skills/explain-execution/README.md`
- `examples/skills/summarize-execution-query/README.md`

