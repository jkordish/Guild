# Contracts

Guild starts from contracts, not implementation folklore.

The canonical contract now lives at [`../SPECS.md`](../SPECS.md).

This file remains in place as a compatibility path for existing links and habits. If this page ever disagrees with the root specification, the root specification wins.

Current contract highlights worth knowing before you follow older notes:

- durable execution IDs are host-minted; caller IDs are correlation only
- `EvidenceRef` points at a host-issued evidence-record URI, while payload blobs remain digest-addressed
- evidence payload and evidence metadata now have distinct canonical resource URIs: `guild://objects/records/{id}` still dereferences payload bytes, while `guild://objects/records/{id}/metadata` returns host-owned `EvidenceRecord` JSON
- requested same-version multi-digest resolution now fails closed as ambiguous
- the active Wasm inspect world is `guild-skill-inspect-v1`, and it only exposes `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`
- the shared host-side contracts still expose broader future vocabulary, but unsupported capability imports are absent from the active inspect guest ABI, broader Guild component imports are rejected as host-owned `unsupported-runtime-surface`, and the host projection into that ABI is explicit, centralized, and fail-closed
- the shared host-side contracts now also expose an explicit typed `filesystem` family, but the active inspect slice still rejects filesystem before execution and the inspect guest ABI does not expose filesystem imports
- the inspect guest `ExecutionContext` is a bounded subset of host execution truth and intentionally omits `mode`
- the current active grant shapes for `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write` project fully into the inspect guest ABI
- durable host records remain the canonical source of truth for requested capabilities, policy decisions, provenance, termination detail, and evidence metadata
- `read-resource` scopes are canonical Guild URI roots, not permissive raw string prefixes
- `http-request` is now a real bounded host capability behind `guild.inspect`, not a separate MCP tool, and its host/domain/path/redirect/private-network controls are enforced by the Rust host boundary rather than guest code
- caller-requested capabilities are now local policy input; the host decides the final granted set before execution
- local policy now selects named profiles by actor and/or tenant and records the host-owned trust tier and verification state used for the decision
- `guild-mcp-server` now exposes one honest stdio MCP tool (`guild.inspect`) plus Guild URI resources and templates
- installed-state portability now has three transport shapes over the same signed payload: the native signed bundle directory, a local OCI image layout, and OCI registry push/pull
- bounded execution-query resources now live under `guild://queries/executions/...` and are exposed through Guild resources and templates rather than new MCP tools

Read next:

- [`../SPECS.md`](../SPECS.md)
- [`../ARCHITECTURE.md`](../ARCHITECTURE.md)
- [`spec-delta-guest-abi-host-record-boundary.md`](spec-delta-guest-abi-host-record-boundary.md)
- [`adr/README.md`](adr/README.md)
